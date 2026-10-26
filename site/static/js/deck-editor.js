/* Deck editor: a Canva-style edit mode for the project-review deck
   (site/src/presentation.html). Plain JavaScript, no dependencies, no
   requests other than the page's own files when building a download.

   Present mode is untouched until the viewer presses E or the Edit button.
   The site is static, so edits are kept in this browser (localStorage), and
   can leave it as a standalone .html download or a JSON export.

   This file is loaded before the deck's own script so a saved version of the
   slides is in place before the deck first shows a slide. */
(function () {
  'use strict';

  var stage = document.getElementById('stage');
  if (!stage || window.DeckEditor) return;
  var body = document.body;
  var DECK_ID = stage.getAttribute('data-deck-id') || 'ferrite-review';
  var KEY = 'ferrite-deck-edit:v1:' + DECK_ID;
  var W = 1600, H = 900;
  var ORIGINAL_TITLE = document.title;

  // ── Small helpers ─────────────────────────────────────────────────────────
  function $(sel, root) { return (root || document).querySelector(sel); }
  function $$(sel, root) { return Array.prototype.slice.call((root || document).querySelectorAll(sel)); }
  function slides() { return $$(':scope > .slide', stage); }
  function clamp(v, a, b) { return Math.max(a, Math.min(b, v)); }
  function deck() { return window.Deck; }
  function curIndex() { var d = deck(); return d ? Math.max(0, d.cur()) : 0; }
  function curSlide() { return slides()[curIndex()] || null; }
  function scale() { var d = deck(); return d ? d.scale() : 1; }
  function show(i) { var d = deck(); if (d) d.show(i, true); }
  function hashStr(s) { var h = 5381; for (var i = 0; i < s.length; i++) h = ((h << 5) + h + s.charCodeAt(i)) | 0; return (h >>> 0).toString(36); }
  function isSvg(el) { return el instanceof SVGElement; }
  function ownText(n) {
    for (var c = n.firstChild; c; c = c.nextSibling) if (c.nodeType === 3 && c.nodeValue.trim()) return true;
    return false;
  }
  function hasText(el) { return !!el && !isSvg(el) && el.tagName !== 'IMG' && el.textContent.trim().length > 0; }
  function countText(n) {
    return Math.round(parseFloat(n.getAttribute('data-count'))).toLocaleString('en-US') + (n.getAttribute('data-suffix') || '');
  }
  function cssVar(name) { return getComputedStyle(document.documentElement).getPropertyValue(name).trim(); }
  function resolveColor(c) { var m = /^var\((--[\w-]+)\)$/.exec(c); return m ? cssVar(m[1]) : c; }

  // ── Serialising the slides ────────────────────────────────────────────────
  // The saved form of the deck is the stage's inner HTML with every runtime
  // trace removed (active/past classes, aria-hidden, count-up progress, the
  // editor's own attributes), so it compares equal to the original until a
  // real edit is made.
  function cleanTree(root) {
    [root].concat($$('*', root)).forEach(function (n) {
      if (n.classList) n.classList.remove('ed-typing');
      n.removeAttribute('contenteditable');
      if (n.getAttribute('class') === '') n.removeAttribute('class');
      if (n.getAttribute('style') === '') n.removeAttribute('style');
    });
    $$('[data-count]', root).forEach(function (n) { n.textContent = countText(n); });
  }
  function cleanSlide(s) {
    s.classList.remove('active', 'past');
    s.removeAttribute('aria-hidden');
    s.removeAttribute('inert');
    cleanTree(s);
  }
  function serialize() {
    var c = stage.cloneNode(true);
    $$(':scope > .slide', c).forEach(cleanSlide);
    return c.innerHTML;
  }

  // Imported or restored HTML is reduced to slides with no script in them.
  var BAD_TAGS = 'script,iframe,object,embed,link,meta,base,form,input,textarea,select,button,frame,frameset,style,template';
  function sanitizeSlides(html) {
    var t = document.createElement('template');
    t.innerHTML = html;
    $$(BAD_TAGS, t.content).forEach(function (n) { n.remove(); });
    $$('*', t.content).forEach(function (n) {
      Array.prototype.slice.call(n.attributes).forEach(function (a) {
        var name = a.name.toLowerCase();
        if (name.indexOf('on') === 0 || name === 'contenteditable') n.removeAttribute(a.name);
        else if (/^(href|src|xlink:href|srcset|action|formaction)$/.test(name) && /^\s*(javascript|vbscript):/i.test(a.value)) n.removeAttribute(a.name);
      });
    });
    Array.prototype.slice.call(t.content.childNodes).forEach(function (n) {
      if (n.nodeType === 1 && !(n.tagName === 'SECTION' && n.classList.contains('slide'))) n.remove();
    });
    return t.content.querySelector('.slide') ? t.innerHTML : null;
  }

  // ── Restore a saved version before the deck starts ───────────────────────
  var originalHTML = serialize();
  var originalHash = hashStr(originalHTML);
  var restoredStale = false;
  (function restore() {
    var raw = null;
    try { raw = window.localStorage.getItem(KEY); } catch (e) { return; }
    if (!raw) return;
    try {
      var rec = JSON.parse(raw);
      var html = rec && typeof rec.html === 'string' ? sanitizeSlides(rec.html) : null;
      if (!html) return;
      stage.innerHTML = html;
      restoredStale = !!(rec.base && rec.base !== originalHash);
    } catch (e) { /* a corrupt record: present the original */ }
  })();

  // ── Editor state ──────────────────────────────────────────────────────────
  var editing = false;
  var root, ctx, thumbs, selBox, hoverBox, guideV, guideH, statusEl, noticeEl, imgInput, jsonInput;
  var sel = null;            // the selected element
  var typing = null;         // the element being typed into (contenteditable)
  var typingBefore = '';
  var state = null;          // serialised slides as of the last commit
  var undoStack = [], redoStack = [];
  var saveTimer = null, nudgeTimer = null, statusTimer = null;
  var openAnchor = null;
  var menuIndex = 0;

  // ── Icons ─────────────────────────────────────────────────────────────────
  var ICONS = {
    undo: '<path d="M9 14L4 9l5-5"/><path d="M4 9h10.5a5.5 5.5 0 0 1 0 11H11"/>',
    redo: '<path d="M15 14l5-5-5-5"/><path d="M20 9H9.5a5.5 5.5 0 0 0 0 11H13"/>',
    text: '<path d="M5 7V5h14v2"/><path d="M12 5v14"/><path d="M9 19h6"/>',
    heading: '<path d="M6 4v16M17 4v16M6 12h11"/>',
    image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="M21 16l-5-5-9 9"/>',
    shape: '<rect x="3" y="3" width="10" height="10" rx="1.5"/><circle cx="16.5" cy="16.5" r="4.5"/>',
    stat: '<path d="M4 20V11M10 20V5M16 20v-6M3 20h18"/>',
    file: '<path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><path d="M14 3v5h5"/>',
    play: '<path d="M7 5l12 7-12 7z" fill="currentColor" stroke="none"/>',
    chev: '<path d="M6 9l6 6 6-6"/>',
    minus: '<path d="M6 12h12"/>',
    plus: '<path d="M12 6v12M6 12h12"/>',
    up: '<path d="M12 19V6"/><path d="M6 11l6-6 6 6"/>',
    left: '<path d="M4 6h16M4 10h10M4 14h16M4 18h10"/>',
    center: '<path d="M4 6h16M7 10h10M4 14h16M7 18h10"/>',
    right: '<path d="M4 6h16M10 10h10M4 14h16M10 18h10"/>',
    opacity: '<circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 0 16z" fill="currentColor"/>',
    fwd: '<rect x="9" y="9" width="11" height="11" rx="2" fill="currentColor" fill-opacity=".3"/><path d="M15 5V4.5A1.5 1.5 0 0 0 13.5 3h-9A1.5 1.5 0 0 0 3 4.5v9A1.5 1.5 0 0 0 4.5 15H5"/>',
    back: '<rect x="4" y="4" width="11" height="11" rx="2" fill="currentColor" fill-opacity=".3"/><path d="M19 9h.5a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 9 19.5V19"/>',
    dup: '<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>',
    trash: '<path d="M4 7h16"/><path d="M10 11v6M14 11v6"/><path d="M6 7l1 13h10l1-13"/><path d="M9 7V4h6v3"/>',
    pencil: '<path d="M4 20h4L19 9l-4-4L4 16v4z"/><path d="M13.5 6.5l4 4"/>',
    fill: '<path d="M5 12l7-7 7 7-7 7z"/><path d="M20 16s2 2.3 2 3.6a2 2 0 0 1-4 0c0-1.3 2-3.6 2-3.6z" fill="currentColor"/>',
    more: '<circle cx="5" cy="12" r="1.6" fill="currentColor"/><circle cx="12" cy="12" r="1.6" fill="currentColor"/><circle cx="19" cy="12" r="1.6" fill="currentColor"/>',
    download: '<path d="M12 4v11"/><path d="M7 10l5 5 5-5"/><path d="M4 20h16"/>',
    json: '<path d="M8 4H7a2 2 0 0 0-2 2v4l-2 2 2 2v4a2 2 0 0 0 2 2h1M16 4h1a2 2 0 0 1 2 2v4l2 2-2 2v4a2 2 0 0 1-2 2h-1"/>',
    upload: '<path d="M12 20V9"/><path d="M7 13l5-5 5 5"/><path d="M4 4h16"/>',
    reset: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5"/>',
    slideUp: '<path d="M12 19V5"/><path d="M6 11l6-6 6 6"/>',
    slideDown: '<path d="M12 5v14"/><path d="M6 13l6 6 6-6"/>',
    addSlide: '<rect x="3" y="5" width="18" height="12" rx="2"/><path d="M12 8v6M9 11h6"/>'
  };
  function icon(n, cls) {
    return '<svg' + (cls ? ' class="' + cls + '"' : '') + ' viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">' + ICONS[n] + '</svg>';
  }

  var TEXT_COLORS = [
    ['var(--text)', 'Text'], ['var(--text-2)', 'Soft text'], ['var(--text-3)', 'Muted'], ['var(--accent)', 'Accent'],
    ['var(--accent-2)', 'Accent light'], ['#f3dccb', 'Sand'], ['var(--good)', 'Good'], ['var(--warn)', 'Warning'],
    ['var(--bad)', 'Bad'], ['#ffffff', 'White'], ['#0c0b0a', 'Black'], ['#8fb8ff', 'Blue']
  ];
  var FILL_COLORS = [
    ['transparent', 'None'], ['var(--card)', 'Card'], ['var(--card-2)', 'Card, raised'], ['var(--bg-2)', 'Panel'],
    ['var(--accent)', 'Accent'], ['rgba(255,122,61,.14)', 'Accent, soft'], ['var(--good)', 'Good'], ['var(--good-soft)', 'Good, soft'],
    ['var(--warn)', 'Warning'], ['var(--warn-soft)', 'Warning, soft'], ['var(--bad)', 'Bad'], ['var(--bad-soft)', 'Bad, soft']
  ];
  function swatches(list, kind) {
    return list.map(function (c) {
      return '<button class="ed-sw' + (c[0] === 'transparent' ? ' is-none' : '') + '" data-act="' + kind + '" data-color="' + c[0] + '" title="' + c[1] + '" aria-label="' + c[1] + '" style="background:' + c[0] + '"></button>';
    }).join('');
  }

  // ── Building the editor's chrome ─────────────────────────────────────────
  function buildUI() {
    root = document.createElement('div');
    root.id = 'ed-root';
    root.setAttribute('data-ed-ui', '');
    root.innerHTML =
      '<header class="ed-top" role="toolbar" aria-label="Deck editor">' +
        '<div class="ed-brand"><span class="ed-mark">F</span><div><b>Project review</b><small>Editing · changes save in this browser</small></div></div>' +
        '<button class="ed-btn ed-icon" data-act="undo" title="Undo (Ctrl+Z)" aria-label="Undo">' + icon('undo') + '</button>' +
        '<button class="ed-btn ed-icon" data-act="redo" title="Redo (Ctrl+Shift+Z)" aria-label="Redo">' + icon('redo') + '</button>' +
        '<span class="ed-sep"></span><span class="ed-label">Insert</span>' +
        '<button class="ed-btn" data-act="ins-text" title="Insert a text box">' + icon('text') + 'Text</button>' +
        '<button class="ed-btn" data-act="ins-heading" title="Insert a heading">' + icon('heading') + 'Heading</button>' +
        '<button class="ed-btn" data-act="ins-image" title="Insert an image from your computer">' + icon('image') + 'Image</button>' +
        '<button class="ed-btn" data-pop="shape" title="Insert a shape">' + icon('shape') + 'Shape' + icon('chev', 'ed-chev') + '</button>' +
        '<button class="ed-btn" data-act="ins-stat" title="Insert a big-number box">' + icon('stat') + 'Stat</button>' +
        '<span class="ed-spacer"></span>' +
        '<span class="ed-notice" hidden><span class="ed-notice-t">Edited version</span><button data-act="reset">Reset to original</button></span>' +
        '<span class="ed-status" aria-live="polite"></span>' +
        '<button class="ed-btn" data-pop="file" title="Download, export, import, reset">' + icon('file') + 'File' + icon('chev', 'ed-chev') + '</button>' +
        '<span class="ed-sep"></span>' +
        '<button class="ed-btn ed-primary" data-act="present" title="Back to present mode (E)">' + icon('play') + 'Present</button>' +
      '</header>' +

      '<div class="ed-ctx" role="toolbar" aria-label="Selected element">' +
        '<div class="ed-ctx-empty"><span><b style="color:var(--text-2)">Click</b> an element to select it</span><span><b style="color:var(--text-2)">Double-click</b> text to type</span><span><b style="color:var(--text-2)">Drag</b> to move · corners to resize</span><span><kbd>Ctrl Z</kbd> undo</span><span><kbd>PgUp</kbd> <kbd>PgDn</kbd> slides</span></div>' +
        '<div class="ed-ctx-tools">' +
          '<span class="ed-kind">Text</span>' +
          '<button class="ed-btn ed-icon" data-act="parent" title="Select the box around this element" aria-label="Select parent">' + icon('up') + '</button>' +
          '<span class="ed-sep"></span>' +
          '<div class="ed-size" title="Font size"><button class="ed-btn" data-act="font-down" data-text aria-label="Smaller text">' + icon('minus') + '</button><span class="ed-size-v">—</span><button class="ed-btn" data-act="font-up" data-text aria-label="Larger text">' + icon('plus') + '</button></div>' +
          '<span class="ed-sep"></span>' +
          '<button class="ed-btn ed-icon" data-act="bold" data-text title="Bold (Ctrl+B)" aria-label="Bold"><span class="ed-glyph b">B</span></button>' +
          '<button class="ed-btn ed-icon" data-act="italic" data-text title="Italic (Ctrl+I)" aria-label="Italic"><span class="ed-glyph i">I</span></button>' +
          '<button class="ed-btn ed-icon" data-act="underline" data-text title="Underline (Ctrl+U)" aria-label="Underline"><span class="ed-glyph u">U</span></button>' +
          '<span class="ed-sep"></span>' +
          '<button class="ed-btn ed-colorbtn" data-pop="color" data-text title="Text colour" aria-label="Text colour"><span class="ed-glyph">A</span><span class="ed-bar ed-bar-text"></span></button>' +
          '<button class="ed-btn ed-colorbtn" data-pop="fill" title="Fill colour" aria-label="Fill colour">' + icon('fill') + '<span class="ed-bar ed-bar-fill"></span></button>' +
          '<span class="ed-sep"></span>' +
          '<button class="ed-btn ed-icon" data-act="align" data-align="left" data-text title="Align left" aria-label="Align left">' + icon('left') + '</button>' +
          '<button class="ed-btn ed-icon" data-act="align" data-align="center" data-text title="Align centre" aria-label="Align centre">' + icon('center') + '</button>' +
          '<button class="ed-btn ed-icon" data-act="align" data-align="right" data-text title="Align right" aria-label="Align right">' + icon('right') + '</button>' +
          '<span class="ed-sep"></span>' +
          '<button class="ed-btn ed-icon" data-pop="opacity" title="Opacity" aria-label="Opacity">' + icon('opacity') + '</button>' +
          '<button class="ed-btn ed-icon" data-act="forward" title="Bring forward" aria-label="Bring forward">' + icon('fwd') + '</button>' +
          '<button class="ed-btn ed-icon" data-act="backward" title="Send backward" aria-label="Send backward">' + icon('back') + '</button>' +
          '<span class="ed-sep"></span>' +
          '<button class="ed-btn ed-icon" data-act="dup" title="Duplicate (Ctrl+D)" aria-label="Duplicate">' + icon('dup') + '</button>' +
          '<button class="ed-btn ed-icon ed-danger" data-act="del" title="Delete (Del)" aria-label="Delete">' + icon('trash') + '</button>' +
          '<span class="ed-spacer"></span>' +
          '<button class="ed-btn" data-act="edit-text" data-text title="Type in this element (Enter or double-click)">' + icon('pencil') + 'Edit text</button>' +
        '</div>' +
      '</div>' +

      '<aside class="ed-rail" aria-label="Slides"><div class="ed-rail-head"><b>Slides</b><span class="ed-count"></span></div>' +
        '<div class="ed-thumbs"></div>' +
        '<button class="ed-add" data-act="add-slide">' + icon('plus') + 'Add slide</button></aside>' +

      '<div class="ed-hover"></div>' +
      '<div class="ed-sel"><span class="ed-sel-tag"></span><i class="ed-h" data-h="nw"></i><i class="ed-h" data-h="ne"></i><i class="ed-h" data-h="sw"></i><i class="ed-h" data-h="se"></i></div>' +
      '<div class="ed-guide ed-guide-v"></div><div class="ed-guide ed-guide-h"></div>' +

      '<div class="ed-pop" data-pop-id="shape" hidden><div class="ed-pop-title">Shapes</div><div class="ed-shapes">' +
        '<button class="ed-btn" data-act="ins-shape" data-shape="rect" title="Rectangle" aria-label="Rectangle"><i style="width:26px;height:18px"></i></button>' +
        '<button class="ed-btn" data-act="ins-shape" data-shape="rounded" title="Rounded rectangle" aria-label="Rounded rectangle"><i style="width:26px;height:18px;border-radius:6px"></i></button>' +
        '<button class="ed-btn" data-act="ins-shape" data-shape="circle" title="Circle" aria-label="Circle"><i style="width:22px;height:22px;border-radius:50%"></i></button>' +
        '<button class="ed-btn" data-act="ins-shape" data-shape="line" title="Line" aria-label="Line"><i style="width:28px;height:3px;border-radius:2px"></i></button>' +
      '</div></div>' +

      '<div class="ed-pop" data-pop-id="color" hidden><div class="ed-pop-title">Text colour</div><div class="ed-swatches">' + swatches(TEXT_COLORS, 'text-color') + '</div>' +
        '<label class="ed-custom">Custom colour <input type="color" class="ed-color-text" value="#edeae4" aria-label="Custom text colour"></label></div>' +
      '<div class="ed-pop" data-pop-id="fill" hidden><div class="ed-pop-title">Fill colour</div><div class="ed-swatches">' + swatches(FILL_COLORS, 'fill-color') + '</div>' +
        '<label class="ed-custom">Custom colour <input type="color" class="ed-color-fill" value="#ff7a3d" aria-label="Custom fill colour"></label></div>' +
      '<div class="ed-pop" data-pop-id="opacity" hidden><div class="ed-pop-title">Opacity</div><div class="ed-range"><input type="range" min="5" max="100" step="1" value="100" class="ed-opacity" aria-label="Opacity"><output class="ed-opacity-v">100%</output></div></div>' +

      '<div class="ed-pop ed-menu" data-pop-id="file" hidden>' +
        '<button class="ed-btn" data-act="download">' + icon('download') + 'Download as HTML<small>one file</small></button>' +
        '<button class="ed-btn" data-act="export">' + icon('json') + 'Export slides (JSON)</button>' +
        '<button class="ed-btn" data-act="import">' + icon('upload') + 'Import slides (JSON)…</button>' +
        '<hr><button class="ed-btn ed-danger" data-act="reset">' + icon('reset') + 'Reset to original…</button>' +
      '</div>' +
      '<div class="ed-pop ed-menu" data-pop-id="slide" hidden>' +
        '<button class="ed-btn" data-act="slide-dup">' + icon('dup') + 'Duplicate slide</button>' +
        '<button class="ed-btn" data-act="slide-add">' + icon('addSlide') + 'Add blank slide below</button>' +
        '<button class="ed-btn" data-act="slide-up">' + icon('slideUp') + 'Move up</button>' +
        '<button class="ed-btn" data-act="slide-down">' + icon('slideDown') + 'Move down</button>' +
        '<hr><button class="ed-btn ed-danger" data-act="slide-del">' + icon('trash') + 'Delete slide</button>' +
      '</div>' +

      '<input type="file" accept="image/*" class="ed-file-img" hidden>' +
      '<input type="file" accept="application/json,.json" class="ed-file-json" hidden>';
    body.appendChild(root);

    ctx = $('.ed-ctx', root);
    thumbs = $('.ed-thumbs', root);
    selBox = $('.ed-sel', root);
    hoverBox = $('.ed-hover', root);
    guideV = $('.ed-guide-v', root);
    guideH = $('.ed-guide-h', root);
    statusEl = $('.ed-status', root);
    noticeEl = $('.ed-notice', root);
    imgInput = $('.ed-file-img', root);
    jsonInput = $('.ed-file-json', root);
    var drop = document.createElement('div');
    drop.className = 'ed-drop';
    thumbs.appendChild(drop);

    // Toolbar buttons must not take focus away from text being typed.
    root.addEventListener('mousedown', function (e) {
      if (e.target.closest('button') && !e.target.closest('input')) e.preventDefault();
    });
    root.addEventListener('click', onRootClick);
    $$('.ed-h', selBox).forEach(function (h) { h.addEventListener('pointerdown', startResize); });

    var ct = $('.ed-color-text', root), cf = $('.ed-color-fill', root), op = $('.ed-opacity', root);
    ct.addEventListener('input', function () { applyTextColor(ct.value, false); });
    ct.addEventListener('change', function () { applyTextColor(ct.value, true); });
    cf.addEventListener('input', function () { applyFill(cf.value, false); });
    cf.addEventListener('change', function () { applyFill(cf.value, true); });
    op.addEventListener('input', function () {
      if (!sel) return;
      var v = +op.value;
      sel.style.opacity = v >= 100 ? '' : String(v / 100);
      $('.ed-opacity-v', root).textContent = v + '%';
      updateOverlay();
    });
    op.addEventListener('change', function () { commit(); });

    imgInput.addEventListener('change', function () {
      var f = imgInput.files && imgInput.files[0];
      imgInput.value = '';
      if (f) readImage(f).then(insertImage, function () { setStatus('Could not read that image', true); });
    });
    jsonInput.addEventListener('change', function () {
      var f = jsonInput.files && jsonInput.files[0];
      jsonInput.value = '';
      if (f) f.text().then(importJSON, function () { setStatus('Could not read that file', true); });
    });

    thumbs.addEventListener('pointerdown', onThumbDown);
    thumbs.addEventListener('contextmenu', function (e) {
      var item = e.target.closest('.ed-thumb');
      if (!item) return;
      e.preventDefault();
      openSlideMenu(+item.dataset.index, { left: e.clientX, right: e.clientX, top: e.clientY, bottom: e.clientY }, null);
    });
  }

  // ── Entering and leaving edit mode ───────────────────────────────────────
  function enter() {
    if (editing) return;
    var d = deck();
    if (d && d.isOverview()) d.toggleOverview();
    if (!root) buildUI();
    editing = true;
    body.classList.add('editing');
    if (state === null) state = serialize();
    $$('[data-count]', stage).forEach(function (n) { n.textContent = countText(n); });
    if (d) { d.fit(); d.show(curIndex(), true); }
    rebuildThumbs();
    refreshUI();
  }
  function exit() {
    if (!editing) return;
    finishTyping();
    flushNudge();
    select(null);
    closePops();
    editing = false;
    body.classList.remove('editing');
    hoverBox.style.display = 'none';
    thumbs.querySelectorAll('.ed-thumb').forEach(function (t) { t.remove(); });
    saveNow();
    var d = deck();
    if (d) { d.fit(); d.show(curIndex(), true); }
  }

  // ── Picking elements on the slide ────────────────────────────────────────
  var SKIP_TAGS = { TBODY: 1, THEAD: 1, TFOOT: 1, TR: 1, BR: 1 };
  function isSelectable(el) {
    if (el.tagName === 'IMG' || el.tagName.toLowerCase() === 'svg') return true;
    if (SKIP_TAGS[el.tagName]) return false;
    var d = getComputedStyle(el).display;
    return d !== 'inline' && d !== 'contents' && d !== 'none';
  }
  function pickAt(t) {
    if (!t || !stage.contains(t) || t === stage) return null;
    if (isSvg(t)) { // an svg's inner parts select the whole drawing
      var o = t.closest('svg');
      while (o && o.parentElement && o.parentElement.closest('svg')) o = o.parentElement.closest('svg');
      t = o || t;
    }
    while (t && t !== stage) {
      if (t.classList.contains('slide') && t.parentElement === stage) return null;
      if (isSelectable(t)) return t;
      t = t.parentElement;
    }
    return null;
  }
  function parentOf(el) {
    var p = el.parentElement;
    while (p && p !== stage && !(p.classList.contains('slide') && p.parentElement === stage)) {
      if (isSelectable(p)) return p;
      p = p.parentElement;
    }
    return null;
  }
  function kindOf(el) {
    var tag = el.tagName.toLowerCase(), c = el.classList;
    if (c.contains('ed-shape')) return 'Shape';
    if (tag === 'img') return 'Image';
    if (tag === 'svg') return 'Graphic';
    if (/^h[1-6]$/.test(tag)) return 'Heading';
    if (c.contains('stat')) return 'Stat';
    if (c.contains('card') || c.contains('stage-card') || c.contains('crate') || c.contains('side')) return 'Card';
    if (tag === 'td' || tag === 'th') return 'Table cell';
    if (tag === 'li') return 'List item';
    if (tag === 'table' || c.contains('tbl')) return 'Table';
    if (tag === 'ul' || tag === 'ol') return 'List';
    if (c.contains('tag') || c.contains('pill') || c.contains('kicker') || c.contains('dir')) return 'Label';
    if (c.contains('takeaway') || c.contains('step')) return 'Callout';
    if (c.contains('shot')) return 'Screenshot';
    if (c.contains('num') || c.contains('big')) return 'Number';
    if (tag === 'p' || c.contains('ed-text') || (hasText(el) && !el.children.length)) return 'Text';
    return 'Group';
  }

  function select(el) {
    if (typing && typing !== el) finishTyping();
    sel = el && el.isConnected ? el : null;
    closePops();
    refreshUI();
  }

  // ── Overlay: selection box, hover box, guides ────────────────────────────
  function updateOverlay() {
    if (!root) return;
    if (!editing || !sel || !sel.isConnected || !curSlide() || !curSlide().contains(sel)) {
      selBox.style.display = 'none';
      return;
    }
    var r = sel.getBoundingClientRect();
    selBox.style.display = 'block';
    selBox.style.left = (r.left - 1) + 'px';
    selBox.style.top = (r.top - 1) + 'px';
    selBox.style.width = (r.width + 2) + 'px';
    selBox.style.height = (r.height + 2) + 'px';
    selBox.classList.toggle('is-typing', !!typing);
    selBox.classList.toggle('is-low', r.top < 140);
    if (!resizing) $('.ed-sel-tag', selBox).textContent = kindOf(sel) + (typing ? ' · typing' : '');
  }
  function showHover(el) {
    if (!el || el === sel || typing || dragging) { hoverBox.style.display = 'none'; return; }
    var r = el.getBoundingClientRect();
    hoverBox.style.display = 'block';
    hoverBox.style.left = r.left + 'px';
    hoverBox.style.top = r.top + 'px';
    hoverBox.style.width = r.width + 'px';
    hoverBox.style.height = r.height + 'px';
  }
  function showGuides(gx, gy) {
    var st = stage.getBoundingClientRect(), s = scale();
    if (gx === null) guideV.style.display = 'none';
    else {
      guideV.style.display = 'block';
      guideV.style.left = (st.left + gx * s) + 'px';
      guideV.style.top = st.top + 'px';
      guideV.style.height = st.height + 'px';
    }
    if (gy === null) guideH.style.display = 'none';
    else {
      guideH.style.display = 'block';
      guideH.style.top = (st.top + gy * s) + 'px';
      guideH.style.left = st.left + 'px';
      guideH.style.width = st.width + 'px';
    }
  }

  // ── Toolbar state ─────────────────────────────────────────────────────────
  function refreshUI() {
    if (!root) return;
    $('[data-act="undo"]', root).disabled = !undoStack.length;
    $('[data-act="redo"]', root).disabled = !redoStack.length;
    var edited = state !== null && state !== originalHTML;
    noticeEl.hidden = !edited;
    $('.ed-notice-t', noticeEl).textContent = restoredStale ? 'Edited version (of an older deck)' : 'Edited version';
    $('.ed-count', root).textContent = slides().length + ' slides';
    ctx.classList.toggle('has-sel', !!sel);
    updateOverlay();
    if (!sel) return;
    var cs = getComputedStyle(sel);
    var text = hasText(sel);
    $('.ed-kind', ctx).textContent = kindOf(sel);
    $$('[data-text]', ctx).forEach(function (b) { b.disabled = !text; });
    $('[data-act="parent"]', ctx).disabled = !parentOf(sel);
    $('.ed-size-v', ctx).textContent = text ? (Math.round(parseFloat(cs.fontSize) * 10) / 10) + '' : '—';
    $('[data-act="bold"]', ctx).classList.toggle('is-on', text && parseInt(cs.fontWeight, 10) >= 600);
    $('[data-act="italic"]', ctx).classList.toggle('is-on', text && cs.fontStyle === 'italic');
    $('[data-act="underline"]', ctx).classList.toggle('is-on', text && /underline/.test(cs.textDecorationLine || cs.textDecoration));
    var align = cs.textAlign === 'start' ? 'left' : cs.textAlign === 'end' ? 'right' : cs.textAlign;
    $$('[data-act="align"]', ctx).forEach(function (b) { b.classList.toggle('is-on', text && b.dataset.align === align); });
    $('.ed-bar-text', ctx).style.background = cs.color;
    $('.ed-bar-fill', ctx).style.background = cs.backgroundColor;
  }

  var statusIsBad = false;
  function setStatus(msg, bad) {
    if (!statusEl) return;
    statusEl.textContent = msg;
    statusEl.classList.toggle('is-bad', !!bad);
    statusIsBad = !!bad;
    clearTimeout(statusTimer);
    if (!bad) statusTimer = setTimeout(function () { statusEl.textContent = ''; }, 2600);
  }

  // ── Commit, undo, redo, autosave ─────────────────────────────────────────
  function commit() {
    clearTimeout(nudgeTimer); nudgeTimer = null;
    var html = serialize();
    if (state === null) state = originalHTML;
    if (html === state) { refreshUI(); return; }
    undoStack.push({ html: state, cur: curIndex() });
    if (undoStack.length > 150) undoStack.shift();
    redoStack.length = 0;
    state = html;
    scheduleSave();
    rebuildThumbs();
    refreshUI();
  }
  function flushNudge() { if (nudgeTimer) commit(); }
  function load(html, cur) {
    sel = null;
    stage.innerHTML = html;
    stage.scrollTop = 0; stage.scrollLeft = 0;
    state = serialize();
    show(clamp(cur || 0, 0, slides().length - 1));
    scheduleSave();
    rebuildThumbs();
    refreshUI();
  }
  function undo() {
    finishTyping(); flushNudge();
    if (!undoStack.length) return;
    redoStack.push({ html: state, cur: curIndex() });
    var s = undoStack.pop();
    load(s.html, s.cur);
    setStatus('Undone');
  }
  function redo() {
    finishTyping(); flushNudge();
    if (!redoStack.length) return;
    undoStack.push({ html: state, cur: curIndex() });
    var s = redoStack.pop();
    load(s.html, s.cur);
    setStatus('Redone');
  }
  function scheduleSave() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(saveNow, 400);
  }
  function saveNow() {
    clearTimeout(saveTimer); saveTimer = null;
    if (state === null) return;
    try {
      if (state === originalHTML) window.localStorage.removeItem(KEY);
      else window.localStorage.setItem(KEY, JSON.stringify({ v: 1, base: originalHash, savedAt: new Date().toISOString(), html: state }));
      if (statusIsBad || editing) setStatus('Saved in this browser');
    } catch (e) {
      setStatus('Not saved: browser storage is off or full. Use File › Download.', true);
    }
  }
  window.addEventListener('pagehide', function () { if (saveTimer) saveNow(); });

  // ── Typing ────────────────────────────────────────────────────────────────
  function startTyping(el, x, y) {
    if (!hasText(el)) return;
    if (typing === el) return;
    finishTyping();
    sel = el;
    typing = el;
    typingBefore = el.innerHTML;
    el.setAttribute('contenteditable', 'true');
    el.classList.add('ed-typing');
    el.focus({ preventScroll: true });
    var range = null;
    if (x != null && document.caretRangeFromPoint) {
      range = document.caretRangeFromPoint(x, y);
      if (range && !el.contains(range.startContainer)) range = null;
    }
    if (!range) { range = document.createRange(); range.selectNodeContents(el); }
    var s = window.getSelection();
    s.removeAllRanges(); s.addRange(range);
    hoverBox.style.display = 'none';
    refreshUI();
  }
  function finishTyping() {
    if (!typing) return;
    var el = typing;
    typing = null;
    el.removeAttribute('contenteditable');
    el.classList.remove('ed-typing');
    if (!el.getAttribute('class')) el.removeAttribute('class');
    var s = window.getSelection();
    if (s && s.anchorNode && el.contains(s.anchorNode)) s.removeAllRanges();
    stage.scrollTop = 0; stage.scrollLeft = 0;
    if (el.innerHTML !== typingBefore) {
      syncCounts(el);
      if (el.classList.contains('ed-item') && !el.textContent.trim() && !el.querySelector('img,svg')) {
        if (sel === el) sel = null;
        el.remove();
      }
      commit();
    } else refreshUI();
  }
  // A number that counts up keeps counting to whatever was typed, if it is
  // still a whole number; otherwise it stops counting and shows the text.
  function syncCounts(el) {
    var nodes = $$('[data-count]', el);
    if (el.hasAttribute('data-count')) nodes.unshift(el);
    nodes.forEach(function (n) {
      var m = /^(\d{1,3}(?:,\d{3})+|\d+)([^\d.,]*)$/.exec(n.textContent.trim());
      if (m) {
        n.setAttribute('data-count', m[1].replace(/,/g, ''));
        if (m[2]) n.setAttribute('data-suffix', m[2]); else n.removeAttribute('data-suffix');
      } else {
        n.removeAttribute('data-count');
        n.removeAttribute('data-suffix');
      }
    });
  }
  function rangeInside(el) {
    var s = window.getSelection();
    return !!(s && s.rangeCount && !s.isCollapsed && el.contains(s.getRangeAt(0).commonAncestorContainer));
  }

  // ── Moving, resizing, snapping ───────────────────────────────────────────
  function getT(el) {
    var p = (el.style.translate || '').split(/\s+/).filter(Boolean).map(parseFloat);
    return { x: p[0] || 0, y: p[1] || 0 };
  }
  function setT(el, x, y) {
    x = Math.round(x); y = Math.round(y);
    el.style.translate = (x || y) ? x + 'px ' + y + 'px' : '';
  }
  function stageBox(el) {
    var r = el.getBoundingClientRect(), st = stage.getBoundingClientRect(), s = scale();
    return { x: (r.left - st.left) / s, y: (r.top - st.top) / s, w: r.width / s, h: r.height / s };
  }
  var SNAP_X = [0, 96, W / 2, W - 96, W], SNAP_Y = [0, 78, H / 2, H - 78, H];
  function snapAxis(start, size, targets, th) {
    var pts = [start, start + size / 2, start + size], best = null;
    targets.forEach(function (t) {
      pts.forEach(function (p) {
        var d = t - p;
        if (Math.abs(d) <= th && (!best || Math.abs(d) < Math.abs(best.d))) best = { d: d, g: t };
      });
    });
    return best;
  }

  var dragging = false, resizing = false;
  function startDrag(e, el, onClick) {
    var x0 = e.clientX, y0 = e.clientY, s = scale(), moved = false;
    var t0 = getT(el), base = stageBox(el);
    function mv(ev) {
      if (!moved && Math.abs(ev.clientX - x0) + Math.abs(ev.clientY - y0) < 4) return;
      if (!moved) { moved = true; dragging = true; body.classList.add('ed-dragging'); hoverBox.style.display = 'none'; }
      var dx = (ev.clientX - x0) / s, dy = (ev.clientY - y0) / s, gx = null, gy = null;
      if (!ev.altKey) {
        var th = 7 / Math.max(s, .3);
        var sx = snapAxis(base.x + dx, base.w, SNAP_X, th), sy = snapAxis(base.y + dy, base.h, SNAP_Y, th);
        if (sx) { dx += sx.d; gx = sx.g; }
        if (sy) { dy += sy.d; gy = sy.g; }
      }
      setT(el, t0.x + dx, t0.y + dy);
      showGuides(gx, gy);
      updateOverlay();
    }
    function up() {
      document.removeEventListener('pointermove', mv);
      document.removeEventListener('pointerup', up);
      document.removeEventListener('pointercancel', up);
      showGuides(null, null);
      body.classList.remove('ed-dragging');
      dragging = false;
      if (moved) commit();
      else if (onClick) onClick();
    }
    document.addEventListener('pointermove', mv);
    document.addEventListener('pointerup', up);
    document.addEventListener('pointercancel', up);
  }

  function startResize(e) {
    if (!sel || e.button !== 0) return;
    e.preventDefault(); e.stopPropagation();
    finishTyping();
    var el = sel, h = e.currentTarget.dataset.h, s = scale();
    var r = el.getBoundingClientRect(), w0 = r.width / s, h0 = r.height / s, t0 = getT(el);
    var keep = el.tagName === 'IMG' || isSvg(el) || el.classList.contains('ed-circle');
    var left = h.indexOf('w') >= 0, top = h.indexOf('n') >= 0;
    var x0 = e.clientX, y0 = e.clientY, moved = false;
    resizing = true;
    var tag = $('.ed-sel-tag', selBox);
    function mv(ev) {
      moved = true;
      var dx = (ev.clientX - x0) / s, dy = (ev.clientY - y0) / s;
      var w = w0 + (left ? -dx : dx), hh = h0 + (top ? -dy : dy);
      if (keep || ev.shiftKey) {
        var ratio = h0 / Math.max(w0, 1);
        if (Math.abs(dx) * ratio >= Math.abs(dy)) hh = w * ratio; else w = hh / ratio;
      }
      w = Math.max(12, w); hh = Math.max(2, hh);
      el.style.width = Math.round(w) + 'px';
      el.style.height = Math.round(hh) + 'px';
      el.style.maxWidth = 'none';
      el.style.minWidth = '0';
      setT(el, t0.x + (left ? w0 - w : 0), t0.y + (top ? h0 - hh : 0));
      tag.textContent = Math.round(w) + ' × ' + Math.round(hh);
      updateOverlay();
    }
    function up() {
      document.removeEventListener('pointermove', mv);
      document.removeEventListener('pointerup', up);
      document.removeEventListener('pointercancel', up);
      resizing = false;
      if (moved) commit(); else refreshUI();
    }
    document.addEventListener('pointermove', mv);
    document.addEventListener('pointerup', up);
    document.addEventListener('pointercancel', up);
  }

  function nudge(dx, dy) {
    if (!sel) return;
    var t = getT(sel);
    setT(sel, t.x + dx, t.y + dy);
    updateOverlay();
    clearTimeout(nudgeTimer);
    nudgeTimer = setTimeout(commit, 400);
  }

  // ── Canvas pointer handling ──────────────────────────────────────────────
  function onStageDown(e) {
    if (!editing || e.button !== 0) return;
    var t = e.target;
    if (typing && typing.contains(t)) return; // place the caret / select text
    if (typing) finishTyping();
    closePops();
    var hit = pickAt(t);
    e.preventDefault();
    if (!hit) { select(null); return; }
    if (sel && sel !== hit && sel.contains(hit)) {
      // Pressing inside the selected box drags the box; a plain click goes one level in.
      var inner = hit;
      startDrag(e, sel, function () { select(inner); });
    } else {
      select(hit);
      startDrag(e, hit, null);
    }
  }
  function onStageDblClick(e) {
    if (!editing) return;
    var hit = pickAt(e.target);
    if (!hit) return;
    // Type into the nearest box that has text (an image's caption, a card's line).
    while (hit && !hasText(hit)) hit = parentOf(hit);
    if (hit) startTyping(hit, e.clientX, e.clientY);
  }
  function onStageMove(e) {
    if (!editing || dragging || resizing) return;
    showHover(pickAt(e.target));
  }

  // ── Selection commands ───────────────────────────────────────────────────
  function fontStep(dir) {
    if (!sel || !hasText(sel)) return;
    finishTyping();
    var base = parseFloat(getComputedStyle(sel).fontSize);
    var step = base < 20 ? 1 : base < 40 ? 2 : 4;
    var f = Math.max(6, base + dir * step) / base;
    var nodes = [sel].concat($$('*', sel).filter(function (n) { return !isSvg(n); }));
    var sizes = nodes.map(function (n) { return parseFloat(getComputedStyle(n).fontSize); });
    nodes.forEach(function (n, i) { n.style.fontSize = (Math.round(sizes[i] * f * 2) / 2) + 'px'; });
    commit();
  }
  function toggleStyle(kind) {
    if (!sel || !hasText(sel)) return;
    if (typing && rangeInside(typing)) {
      document.execCommand(kind === 'bold' ? 'bold' : kind === 'italic' ? 'italic' : 'underline');
      refreshUI();
      return;
    }
    finishTyping();
    var cs = getComputedStyle(sel);
    if (kind === 'bold') sel.style.fontWeight = parseInt(cs.fontWeight, 10) >= 600 ? '400' : '700';
    if (kind === 'italic') sel.style.fontStyle = cs.fontStyle === 'italic' ? 'normal' : 'italic';
    if (kind === 'underline') sel.style.textDecoration = /underline/.test(cs.textDecorationLine || cs.textDecoration) ? 'none' : 'underline';
    commit();
  }
  function applyTextColor(c, doCommit) {
    if (!sel || !hasText(sel)) return;
    if (typing && rangeInside(typing)) {
      document.execCommand('styleWithCSS', false, true);
      document.execCommand('foreColor', false, resolveColor(c));
      return;
    }
    finishTyping();
    sel.style.color = c;
    $$('*', sel).forEach(function (n) { if (!isSvg(n) && ownText(n)) n.style.color = c; });
    if (doCommit) commit(); else refreshUI();
  }
  function applyFill(c, doCommit) {
    if (!sel) return;
    finishTyping();
    if (isSvg(sel)) sel.style.color = c;
    else sel.style.background = c;
    if (doCommit) commit(); else refreshUI();
  }
  function align(v) {
    if (!sel || !hasText(sel)) return;
    finishTyping();
    sel.style.textAlign = v;
    var d = getComputedStyle(sel).display;
    if (d === 'flex' || d === 'inline-flex') sel.style.justifyContent = v === 'left' ? 'flex-start' : v === 'right' ? 'flex-end' : 'center';
    commit();
  }
  function zStep(d) {
    if (!sel) return;
    finishTyping();
    var cs = getComputedStyle(sel);
    if (cs.position === 'static') sel.style.position = 'relative';
    var z = parseInt(cs.zIndex, 10);
    if (isNaN(z)) z = 0;
    sel.style.zIndex = String(z + d);
    commit();
  }
  function duplicate() {
    if (!sel) return;
    finishTyping();
    var c = sel.cloneNode(true);
    c.classList.remove('ed-typing');
    c.removeAttribute('contenteditable');
    sel.after(c);
    if (getComputedStyle(sel).position === 'absolute') { var t = getT(sel); setT(c, t.x + 24, t.y + 24); }
    sel = c;
    commit();
  }
  function del() {
    if (!sel) return;
    finishTyping();
    var el = sel;
    sel = null;
    el.remove();
    commit();
  }

  // ── Inserting ─────────────────────────────────────────────────────────────
  function place(el) {
    var slide = curSlide();
    if (!slide) return;
    finishTyping();
    el.classList.add('ed-item', 'r');
    el.style.setProperty('--i', '3');
    slide.appendChild(el);
    var w = el.offsetWidth, h = el.offsetHeight;
    el.style.left = Math.round((W - w) / 2) + 'px';
    el.style.top = Math.round((H - h) / 2) + 'px';
    sel = el;
    commit();
  }
  function fromHTML(html) { var t = document.createElement('template'); t.innerHTML = html.trim(); return t.content.firstElementChild; }
  function insert(kind) {
    if (kind === 'text') place(fromHTML('<div class="ed-text" style="width:640px">Your text here. Double-click to edit.</div>'));
    else if (kind === 'heading') place(fromHTML('<h2 style="width:900px">A new heading</h2>'));
    else if (kind === 'stat') place(fromHTML('<div class="card stat" style="width:320px"><div class="num grad">42%</div><div class="lbl">What this number means</div></div>'));
  }
  function insertShape(shape) {
    var st = { rect: 'width:360px;height:220px', rounded: 'width:360px;height:220px;border-radius:28px', circle: 'width:240px;height:240px;border-radius:50%', line: 'width:480px;height:4px;border-radius:2px' }[shape];
    var el = fromHTML('<div class="ed-shape' + (shape === 'circle' ? ' ed-circle' : '') + '" style="' + st + '"></div>');
    place(el);
  }
  function blobToDataURL(blob) {
    return new Promise(function (res, rej) { var r = new FileReader(); r.onload = function () { res(r.result); }; r.onerror = rej; r.readAsDataURL(blob); });
  }
  // Images are embedded as data: URLs so they survive autosave and download;
  // very large ones are scaled down to keep the saved deck small.
  function readImage(file) {
    return blobToDataURL(file).then(function (url) {
      return new Promise(function (res, rej) {
        var im = new Image();
        im.onload = function () {
          var w = im.naturalWidth || 400, h = im.naturalHeight || 300, max = 1600;
          if ((w > max || h > max) && !/svg/.test(file.type)) {
            var k = max / Math.max(w, h), c = document.createElement('canvas');
            c.width = Math.round(w * k); c.height = Math.round(h * k);
            c.getContext('2d').drawImage(im, 0, 0, c.width, c.height);
            url = c.toDataURL(file.type === 'image/jpeg' ? 'image/jpeg' : 'image/png', .9);
            w = c.width; h = c.height;
          }
          res({ url: url, w: w, h: h, name: file.name });
        };
        im.onerror = rej;
        im.src = url;
      });
    });
  }
  function insertImage(img) {
    var k = Math.min(1, 640 / img.w, 480 / img.h);
    var el = document.createElement('img');
    el.src = img.url;
    el.alt = (img.name || '').replace(/\.[a-z0-9]+$/i, '');
    el.style.width = Math.round(img.w * k) + 'px';
    el.style.height = Math.round(img.h * k) + 'px';
    place(el);
  }

  // ── Slides: thumbnails, order, duplicate, delete ─────────────────────────
  function rebuildThumbs() {
    if (!editing || !thumbs) return;
    var drop = $('.ed-drop', thumbs);
    $$('.ed-thumb', thumbs).forEach(function (t) { t.remove(); });
    var cur = curIndex(), frag = document.createDocumentFragment();
    slides().forEach(function (s, i) {
      var item = document.createElement('div');
      item.className = 'ed-thumb' + (i === cur ? ' is-current' : '');
      item.dataset.index = i;
      item.title = 'Slide ' + (i + 1) + ' · drag to reorder, right-click for more';
      item.innerHTML = '<span class="ed-thumb-n">' + (i + 1) + '</span><div class="ed-thumb-frame"><div class="ed-thumb-canvas"></div>' +
        '<button class="ed-thumb-more" aria-label="Slide ' + (i + 1) + ' actions" title="Slide actions">' + icon('more') + '</button></div>';
      var c = s.cloneNode(true);
      c.classList.remove('past');
      c.classList.add('active');
      c.removeAttribute('aria-hidden');
      c.setAttribute('inert', '');
      $$('[contenteditable]', c).forEach(function (n) { n.removeAttribute('contenteditable'); });
      $('.ed-thumb-canvas', item).appendChild(c);
      frag.appendChild(item);
    });
    thumbs.insertBefore(frag, drop);
    $('.ed-count', root).textContent = slides().length + ' slides';
  }
  function markCurrentThumb(i) {
    if (!thumbs) return;
    $$('.ed-thumb', thumbs).forEach(function (t, k) {
      t.classList.toggle('is-current', k === i);
      if (k === i) {
        var tr = t.getBoundingClientRect(), lr = thumbs.getBoundingClientRect();
        if (tr.top < lr.top || tr.bottom > lr.bottom) t.scrollIntoView({ block: 'nearest' });
      }
    });
  }
  function onThumbDown(e) {
    if (e.button !== 0) return;
    var item = e.target.closest('.ed-thumb');
    if (!item) return;
    var from = +item.dataset.index;
    var more = e.target.closest('.ed-thumb-more');
    if (more) { e.preventDefault(); openSlideMenu(from, more.getBoundingClientRect(), more); return; }
    e.preventDefault();
    finishTyping();
    var y0 = e.clientY, moved = false, to = from;
    var drop = $('.ed-drop', thumbs), scrollTimer = null, lastY = y0;
    function gapAt(y) {
      var items = $$('.ed-thumb', thumbs), k = items.length;
      for (var i = 0; i < items.length; i++) {
        var r = items[i].getBoundingClientRect();
        if (y < r.top + r.height / 2) { k = i; break; }
      }
      return k;
    }
    function placeDrop(k) {
      var items = $$('.ed-thumb', thumbs);
      var top = k < items.length ? items[k].offsetTop : items[items.length - 1].offsetTop + items[items.length - 1].offsetHeight;
      drop.style.top = (top - 2) + 'px';
      drop.style.display = (k === from || k === from + 1) ? 'none' : 'block';
    }
    function autoScroll() {
      var r = thumbs.getBoundingClientRect();
      var d = lastY < r.top + 40 ? -12 : lastY > r.bottom - 40 ? 12 : 0;
      if (d) { thumbs.scrollTop += d; to = gapAt(lastY); placeDrop(to); }
    }
    function mv(ev) {
      lastY = ev.clientY;
      if (!moved && Math.abs(ev.clientY - y0) < 6) return;
      if (!moved) { moved = true; item.classList.add('is-dragging'); body.classList.add('ed-dragging'); scrollTimer = setInterval(autoScroll, 30); }
      to = gapAt(ev.clientY);
      placeDrop(to);
    }
    function up() {
      document.removeEventListener('pointermove', mv);
      document.removeEventListener('pointerup', up);
      document.removeEventListener('pointercancel', up);
      clearInterval(scrollTimer);
      drop.style.display = 'none';
      item.classList.remove('is-dragging');
      body.classList.remove('ed-dragging');
      if (moved) moveSlide(from, to);
      else { select(null); show(from); }
    }
    document.addEventListener('pointermove', mv);
    document.addEventListener('pointerup', up);
    document.addEventListener('pointercancel', up);
  }
  // Moves slide `from` into the gap before slide `gap` (0..n).
  function moveSlide(from, gap) {
    var list = slides();
    if (gap === from || gap === from + 1 || from < 0 || from >= list.length) return;
    var s = list[from];
    stage.insertBefore(s, gap < list.length ? list[gap] : null);
    select(null);
    show(gap > from ? gap - 1 : gap);
    commit();
  }
  function blankSlide() {
    return fromHTML('<section class="slide"><div class="kicker r" style="--i:0">Section</div><h2 class="r" style="--i:1">New slide</h2></section>');
  }
  function slideAction(act, i) {
    finishTyping();
    var list = slides(), s = list[i];
    if (!s) return;
    select(null);
    if (act === 'slide-dup') {
      var c = s.cloneNode(true);
      cleanSlide(c);
      s.after(c);
      show(i + 1);
    } else if (act === 'slide-add') {
      s.after(blankSlide());
      show(i + 1);
    } else if (act === 'slide-del') {
      if (list.length < 2) { setStatus('A deck needs at least one slide', true); return; }
      s.remove();
      show(Math.min(i, list.length - 2));
    } else if (act === 'slide-up') {
      if (i === 0) return;
      stage.insertBefore(s, list[i - 1]);
      show(i - 1);
    } else if (act === 'slide-down') {
      if (i >= list.length - 1) return;
      list[i + 1].after(s);
      show(i + 1);
    }
    commit();
  }
  function openSlideMenu(i, rect, anchor) {
    menuIndex = i;
    var n = slides().length;
    var pop = $('[data-pop-id="slide"]', root);
    $('[data-act="slide-del"]', pop).disabled = n < 2;
    $('[data-act="slide-up"]', pop).disabled = i === 0;
    $('[data-act="slide-down"]', pop).disabled = i >= n - 1;
    openPopAt('slide', rect, anchor);
  }

  // ── Popovers ──────────────────────────────────────────────────────────────
  function openPopAt(id, r, anchor) {
    closePops();
    var p = $('[data-pop-id="' + id + '"]', root);
    if (!p) return;
    p.hidden = false;
    var w = p.offsetWidth, h = p.offsetHeight;
    var left = clamp(r.left, 8, window.innerWidth - w - 8), top = r.bottom + 6;
    if (top + h > window.innerHeight - 8) top = Math.max(8, r.top - h - 6);
    p.style.left = left + 'px';
    p.style.top = top + 'px';
    if (anchor) { anchor.classList.add('is-open'); openAnchor = anchor; }
  }
  function togglePop(id, anchor) {
    var p = $('[data-pop-id="' + id + '"]', root);
    if (p && !p.hidden) { closePops(); return; }
    if (id === 'opacity' && sel) {
      var v = Math.round(parseFloat(getComputedStyle(sel).opacity) * 100);
      $('.ed-opacity', root).value = v;
      $('.ed-opacity-v', root).textContent = v + '%';
    }
    openPopAt(id, anchor.getBoundingClientRect(), anchor);
  }
  function closePops() {
    if (!root) return;
    $$('.ed-pop', root).forEach(function (p) { p.hidden = true; });
    if (openAnchor) { openAnchor.classList.remove('is-open'); openAnchor = null; }
  }

  // ── Toolbar dispatch ──────────────────────────────────────────────────────
  function onRootClick(e) {
    var popBtn = e.target.closest('[data-pop]');
    if (popBtn && root.contains(popBtn)) { if (!popBtn.disabled) togglePop(popBtn.dataset.pop, popBtn); return; }
    var b = e.target.closest('[data-act]');
    if (!b || b.disabled || b.closest('.ed-thumbs')) return;
    var act = b.dataset.act;
    var keepPop = act === 'text-color' || act === 'fill-color';
    if (!keepPop) closePops();
    switch (act) {
      case 'undo': undo(); break;
      case 'redo': redo(); break;
      case 'present': exit(); break;
      case 'ins-text': insert('text'); break;
      case 'ins-heading': insert('heading'); break;
      case 'ins-stat': insert('stat'); break;
      case 'ins-image': finishTyping(); imgInput.click(); break;
      case 'ins-shape': insertShape(b.dataset.shape); break;
      case 'parent': if (sel) { finishTyping(); select(parentOf(sel)); } break;
      case 'font-down': fontStep(-1); break;
      case 'font-up': fontStep(1); break;
      case 'bold': case 'italic': case 'underline': toggleStyle(act); break;
      case 'text-color': applyTextColor(b.dataset.color, true); break;
      case 'fill-color': applyFill(b.dataset.color, true); break;
      case 'align': align(b.dataset.align); break;
      case 'forward': zStep(1); break;
      case 'backward': zStep(-1); break;
      case 'dup': duplicate(); break;
      case 'del': del(); break;
      case 'edit-text': if (sel) startTyping(sel); break;
      case 'add-slide': slideAction('slide-add', curIndex()); break;
      case 'slide-dup': case 'slide-add': case 'slide-del': case 'slide-up': case 'slide-down': slideAction(act, menuIndex); break;
      case 'download': download(); break;
      case 'export': exportJSON(); break;
      case 'import': finishTyping(); jsonInput.click(); break;
      case 'reset': resetDeck(); break;
    }
  }

  // ── Keyboard ──────────────────────────────────────────────────────────────
  document.addEventListener('keydown', function (e) {
    var t = e.target;
    var inField = t && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));
    var mod = e.ctrlKey || e.metaKey, k = e.key;
    if (!editing) {
      if (!inField && !mod && !e.altKey && (k === 'e' || k === 'E')) { e.preventDefault(); enter(); }
      return;
    }
    if (typing) {
      if (k === 'Escape') { e.preventDefault(); finishTyping(); }
      return; // everything else is text
    }
    if (inField) return;
    if (mod && (k === 'z' || k === 'Z')) { e.preventDefault(); if (e.shiftKey) redo(); else undo(); return; }
    if (mod && (k === 'y' || k === 'Y')) { e.preventDefault(); redo(); return; }
    if (mod && (k === 'd' || k === 'D')) { e.preventDefault(); duplicate(); return; }
    if (mod || e.altKey) return;
    if (k === 'Escape') { e.preventDefault(); if (openAnchor || $$('.ed-pop:not([hidden])', root).length) closePops(); else select(null); }
    else if (k === 'e' || k === 'E') { e.preventDefault(); exit(); }
    else if ((k === 'Delete' || k === 'Backspace') && sel) { e.preventDefault(); del(); }
    else if (k === 'Enter' && sel && hasText(sel)) { e.preventDefault(); startTyping(sel); }
    else if (/^Arrow/.test(k)) {
      e.preventDefault();
      var n = e.shiftKey ? 10 : 1;
      if (sel) nudge(k === 'ArrowLeft' ? -n : k === 'ArrowRight' ? n : 0, k === 'ArrowUp' ? -n : k === 'ArrowDown' ? n : 0);
      else if (k === 'ArrowLeft' || k === 'ArrowUp') show(curIndex() - 1);
      else show(curIndex() + 1);
    }
    else if (k === 'PageDown') { e.preventDefault(); select(null); show(curIndex() + 1); }
    else if (k === 'PageUp') { e.preventDefault(); select(null); show(curIndex() - 1); }
    else if (k === 'Home') { e.preventDefault(); select(null); show(0); }
    else if (k === 'End') { e.preventDefault(); select(null); show(slides().length - 1); }
  });

  // ── Download, export, import, reset ──────────────────────────────────────
  function fetchOk(url) { return fetch(url).then(function (r) { if (!r.ok) throw new Error(r.status + ' ' + url); return r; }); }
  function fetchDataURL(url) { return fetchOk(url).then(function (r) { return r.blob(); }).then(blobToDataURL); }
  function fetchText(url) { return fetchOk(url).then(function (r) { return r.text(); }); }
  function inlineable(u) { return !!u && !/^(data:|blob:|#|javascript:|mailto:)/i.test(u.trim()); }
  function absUrl(u, base) { try { return new URL(u, base || document.baseURI).href; } catch (e) { return u; } }
  function inlineCssUrls(css, base) {
    var re = /url\(\s*(['"]?)([^'")]+)\1\s*\)/g, found = {}, m;
    while ((m = re.exec(css))) if (inlineable(m[2])) found[m[2]] = null;
    return Promise.all(Object.keys(found).map(function (u) {
      return fetchDataURL(absUrl(u, base)).then(function (d) { found[u] = d; }, function () {});
    })).then(function () {
      return css.replace(re, function (all, q, u) { return found[u] ? 'url("' + found[u] + '")' : all; });
    });
  }
  // A copy of this page with the edited slides, every style sheet, script,
  // font and image inlined, so the file opens anywhere with no network.
  function buildStandalone() {
    finishTyping(); flushNudge();
    var doc = document.documentElement.cloneNode(true);
    $$('[data-ed-ui]', doc).forEach(function (n) { n.remove(); });
    var b = $('body', doc);
    b.removeAttribute('class');
    var st = $('#stage', doc);
    st.removeAttribute('style');
    st.innerHTML = serialize();
    st.setAttribute('data-deck-id', 'ferrite-review-copy-' + Date.now().toString(36));
    var pr = $('#progress', doc); if (pr) pr.removeAttribute('style');
    var hint = $('#hint', doc); if (hint) hint.classList.remove('gone');
    var fsOn = $('#fs-on', doc), fsOff = $('#fs-off', doc);
    if (fsOn) fsOn.removeAttribute('style');
    if (fsOff) fsOff.setAttribute('style', 'display:none');
    var title = $('title', doc); if (title) title.textContent = ORIGINAL_TITLE;
    var jobs = [];
    $$('style', doc).forEach(function (s) {
      jobs.push(inlineCssUrls(s.textContent, document.baseURI).then(function (css) { s.textContent = css; }));
    });
    $$('link[rel~="stylesheet"][href]', doc).forEach(function (l) {
      var url = absUrl(l.getAttribute('href'));
      jobs.push(fetchText(url).then(function (css) { return inlineCssUrls(css, url); }).then(function (css) {
        var s = document.createElement('style'); s.textContent = css; l.replaceWith(s);
      }, function () { l.setAttribute('href', url); }));
    });
    $$('script[src]', doc).forEach(function (sc) {
      var url = absUrl(sc.getAttribute('src'));
      jobs.push(fetchText(url).then(function (js) {
        var n = document.createElement('script'); n.textContent = js.replace(/<\/script/gi, '<\\/script'); sc.replaceWith(n);
      }, function () { sc.setAttribute('src', url); }));
    });
    $$('link[rel~="icon"][href], img[src]', doc).forEach(function (el) {
      var attr = el.tagName === 'IMG' ? 'src' : 'href', u = el.getAttribute(attr);
      if (!inlineable(u)) return;
      jobs.push(fetchDataURL(absUrl(u)).then(function (d) { el.setAttribute(attr, d); }, function () { el.setAttribute(attr, absUrl(u)); }));
    });
    $$('a[href]', doc).forEach(function (a) { var u = a.getAttribute('href'); if (inlineable(u)) a.setAttribute('href', absUrl(u)); });
    return Promise.all(jobs).then(function () { return '<!doctype html>\n' + doc.outerHTML; });
  }
  function saveFile(name, text, type) {
    var url = URL.createObjectURL(new Blob([text], { type: type }));
    var a = document.createElement('a');
    a.href = url; a.download = name; a.style.display = 'none';
    a.setAttribute('data-ed-ui', '');
    body.appendChild(a);
    a.click();
    setTimeout(function () { URL.revokeObjectURL(url); a.remove(); }, 2000);
  }
  function stamp() { return new Date().toISOString().slice(0, 10); }
  function download() {
    setStatus('Preparing download…');
    buildStandalone().then(function (html) {
      saveFile('ferrite-review-edited-' + stamp() + '.html', html, 'text/html');
      setStatus('Downloaded');
    }, function () { setStatus('Download failed', true); });
  }
  function exportJSON() {
    finishTyping(); flushNudge();
    var c = stage.cloneNode(true);
    var data = {
      format: 'ferrite-deck', version: 1, exportedAt: new Date().toISOString(), title: ORIGINAL_TITLE,
      slides: $$(':scope > .slide', c).map(function (s) {
        cleanSlide(s);
        var h = $('h1, h2', s);
        return { title: s.getAttribute('data-title') || (h ? h.textContent.trim() : ''), html: s.outerHTML };
      })
    };
    saveFile('ferrite-review-slides-' + stamp() + '.json', JSON.stringify(data, null, 2), 'application/json');
    setStatus('Exported ' + data.slides.length + ' slides');
  }
  function importJSON(text) {
    var data;
    try { data = JSON.parse(text); } catch (e) { setStatus('Import failed: not a JSON file', true); return; }
    var list = data && Array.isArray(data.slides) ? data.slides : null;
    if (!list || !list.length || data.format !== 'ferrite-deck') { setStatus('Import failed: not an exported deck', true); return; }
    var html = sanitizeSlides(list.map(function (s) { return s && typeof s.html === 'string' ? s.html : ''; }).join('\n'));
    if (!html) { setStatus('Import failed: no slides in the file', true); return; }
    finishTyping(); flushNudge();
    undoStack.push({ html: state, cur: curIndex() });
    redoStack.length = 0;
    restoredStale = false;
    load(html, 0);
    setStatus('Imported ' + slides().length + ' slides');
  }
  function resetDeck() {
    if (!window.confirm('Reset to the original deck?\n\nThis removes the edited version saved in this browser. You can still undo it until you leave the page.')) return;
    finishTyping(); flushNudge();
    if (state !== originalHTML) { undoStack.push({ html: state, cur: curIndex() }); redoStack.length = 0; }
    restoredStale = false;
    load(originalHTML, curIndex());
    saveNow();
    setStatus('Back to the original');
  }

  // ── Wiring ────────────────────────────────────────────────────────────────
  function init() {
    if (state === null) state = serialize();
    var btn = document.getElementById('edit');
    if (btn) btn.addEventListener('click', function (e) { e.stopPropagation(); if (editing) exit(); else enter(); });
    stage.addEventListener('pointerdown', onStageDown);
    stage.addEventListener('dblclick', onStageDblClick);
    stage.addEventListener('pointermove', onStageMove);
    stage.addEventListener('pointerleave', function () { if (hoverBox) hoverBox.style.display = 'none'; });
    stage.addEventListener('dragstart', function (e) { if (editing) e.preventDefault(); });
    stage.addEventListener('input', function () { if (typing) updateOverlay(); });
    stage.addEventListener('paste', function (e) {
      if (!typing) return;
      e.preventDefault();
      var text = (e.clipboardData || window.clipboardData).getData('text/plain');
      document.execCommand('insertText', false, text);
    });
    stage.addEventListener('focusout', function (e) {
      if (typing && e.target === typing && !(e.relatedTarget && typing.contains(e.relatedTarget))) finishTyping();
    });
    document.addEventListener('pointerdown', function (e) {
      if (!root || !editing) return;
      if (!e.target.closest('.ed-pop') && !e.target.closest('[data-pop]') && !e.target.closest('.ed-thumb-more')) closePops();
    }, true);
    document.addEventListener('deck:show', function (e) {
      if (!editing) return;
      if (sel && !(curSlide() && curSlide().contains(sel))) select(null);
      markCurrentThumb(e.detail.index);
      updateOverlay();
    });
    window.addEventListener('resize', function () { if (editing) requestAnimationFrame(updateOverlay); });
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();

  window.DeckEditor = {
    enter: enter, exit: exit, undo: undo, redo: redo,
    isEditing: function () { return editing; },
    serialize: serialize
  };
})();
