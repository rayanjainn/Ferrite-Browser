/* Ferrite site: motion. Reveals on scroll, counts numbers up, and runs the
   page's explainers on a loop: the attack story, the four stages, the legend
   beside the figure and the audit chain. Nothing waits for a click. Each loop
   pauses while it is off screen, and under prefers-reduced-motion every
   explainer is shown complete and still. */
(function () {
  'use strict';
  var still = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  var hasIO = 'IntersectionObserver' in window;

  // Calls tick(step) every `ms` while `el` is on screen.
  function loop(el, ms, tick) {
    var step = 0, timer = 0, on = false;
    function run() { tick(step++); timer = setTimeout(run, typeof ms === 'function' ? ms(step) : ms); }
    function start() { if (!on) { on = true; run(); } }
    function stop() { on = false; clearTimeout(timer); }
    if (!hasIO) { start(); return; }
    new IntersectionObserver(function (es) { es[es.length - 1].isIntersecting ? start() : stop(); }, { rootMargin: '80px 0px' }).observe(el);
  }

  // ── Reveal ─────────────────────────────────────────────────────────────
  var reveals = document.querySelectorAll('[data-reveal], .stat');
  if (!hasIO || still) {
    reveals.forEach(function (el) { el.classList.add('is-in'); });
  } else {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) {
        if (!en.isIntersecting) return;
        en.target.classList.add('is-in');
        io.unobserve(en.target);
      });
    }, { threshold: 0.12, rootMargin: '0px 0px -6% 0px' });
    reveals.forEach(function (el) { io.observe(el); });
  }
  // Children of [data-stagger] reveal one after another.
  document.querySelectorAll('[data-stagger]').forEach(function (group) {
    Array.prototype.forEach.call(group.children, function (child, i) {
      child.setAttribute('data-reveal', child.getAttribute('data-reveal') || '');
      child.style.setProperty('--d', (i * 110) + 'ms');
      if (still || !hasIO) child.classList.add('is-in'); else io.observe(child);
    });
  });

  // ── Count up ───────────────────────────────────────────────────────────
  function countUp(el) {
    var to = parseFloat(el.getAttribute('data-count'));
    var dec = (el.getAttribute('data-count').split('.')[1] || '').length;
    var pre = el.getAttribute('data-pre') || '', post = el.getAttribute('data-post') || '';
    var fmt = function (v) { return pre + v.toLocaleString('en-US', { minimumFractionDigits: dec, maximumFractionDigits: dec }) + post; };
    if (still) { el.textContent = fmt(to); return; }
    var t0 = 0;
    function frame(t) {
      if (!t0) t0 = t;
      var p = Math.min(1, (t - t0) / 1600), e = 1 - Math.pow(1 - p, 4);
      el.textContent = fmt(to * e);
      if (p < 1) requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
  }
  var counters = document.querySelectorAll('[data-count]');
  if (hasIO) {
    var cio = new IntersectionObserver(function (es) {
      es.forEach(function (en) { if (en.isIntersecting) { countUp(en.target); cio.unobserve(en.target); } });
    }, { threshold: 0.4 });
    counters.forEach(function (el) { cio.observe(el); });
  } else counters.forEach(countUp);

  // ── The story: one task, one hostile page ──────────────────────────────
  document.querySelectorAll('[data-story]').forEach(function (story) {
    var beats = story.querySelectorAll('.beat');
    var bar = story.querySelector('.story-bar i');
    if (still) { beats.forEach(function (b) { b.classList.add('on'); }); if (bar) bar.style.width = '100%'; return; }
    // Step 0 clears the card, steps 1..n bring in one beat each, step n+1 holds.
    var n = beats.length;
    loop(story, function (step) { var s = (step - 1) % (n + 2); return s === 0 ? 700 : s === n + 1 ? 3200 : 2000; }, function (step) {
      var s = step % (n + 2);
      beats.forEach(function (b, i) {
        b.classList.toggle('on', s > 0 && i < s);
        b.classList.toggle('now', s > 0 && s <= n && i === s - 1);
      });
      if (bar) {
        bar.style.transition = s === 0 ? 'none' : 'width 2s linear';
        bar.style.width = (s === 0 ? 0 : Math.min(100, (s / n) * 100)) + '%';
      }
    });
  });

  // ── Stages, legends: highlight one item after another ──────────────────
  document.querySelectorAll('[data-cycle]').forEach(function (group) {
    var items = group.querySelectorAll('[data-cycle-item]');
    var ms = parseInt(group.getAttribute('data-cycle'), 10) || 2000;
    if (still) return;
    loop(group, ms, function (step) {
      items.forEach(function (it, i) { it.classList.toggle('is-active', i === step % items.length); });
    });
  });

  // ── The audit chain: one entry is altered, every later link breaks ────
  document.querySelectorAll('[data-chain]').forEach(function (wrap) {
    var blocks = wrap.querySelectorAll('.blk'), links = wrap.querySelectorAll('.link');
    var status = wrap.querySelector('.chain-status'), text = status && status.querySelector('span');
    var ok = text ? text.textContent : '';
    if (still) return;
    loop(wrap, 1500, function (step) {
      var s = step % 8, hit = 2;
      blocks.forEach(function (b, i) {
        b.classList.toggle('bad', s >= 2 && s <= 5 && i === hit);
        b.classList.toggle('broken', s >= 3 && s <= 5 && i > hit);
      });
      links.forEach(function (l, i) { l.classList.toggle('broken', s >= 3 && s <= 5 && i >= hit); });
      if (status) {
        status.classList.toggle('bad', s >= 2 && s <= 5);
        text.textContent = s === 2 ? 'Entry 3 was edited after it was written…'
          : s >= 3 && s <= 5 ? 'Verification fails: entry 3 no longer matches the hash in entry 4.'
          : ok;
      }
    });
  });
})();
