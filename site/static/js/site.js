/* Ferrite site: theme, menu, copy buttons, on-page nav and bar animation.
   Downloads live in releases.js and motion in motion.js. Everything degrades:
   with JavaScript off the page is fully readable. */
(function () {
  'use strict';
  var doc = document.documentElement;
  var REPO = 'rayanjainn/Ferrite-Browser';

  function store(key, value) { try { localStorage.setItem(key, value); } catch (e) {} }

  // ── Theme ──────────────────────────────────────────────────────────────
  var toggle = document.getElementById('theme-toggle');
  if (toggle) {
    toggle.addEventListener('click', function () {
      var explicit = doc.getAttribute('data-theme');
      var dark = explicit ? explicit === 'dark' : window.matchMedia('(prefers-color-scheme: dark)').matches;
      var next = dark ? 'light' : 'dark';
      doc.setAttribute('data-theme', next);
      store('ferrite-theme', next);
    });
  }

  // ── Mobile menu ────────────────────────────────────────────────────────
  var menuBtn = document.getElementById('menu-btn');
  var links = document.getElementById('nav-links');
  if (menuBtn && links) {
    menuBtn.addEventListener('click', function () {
      var open = links.classList.toggle('open');
      menuBtn.setAttribute('aria-expanded', open ? 'true' : 'false');
    });
    links.addEventListener('click', function (e) {
      if (e.target.tagName === 'A') { links.classList.remove('open'); menuBtn.setAttribute('aria-expanded', 'false'); }
    });
  }

  // ── Copy buttons on code blocks ────────────────────────────────────────
  document.querySelectorAll('pre').forEach(function (pre) {
    if (!navigator.clipboard) return;
    var b = document.createElement('button');
    b.type = 'button'; b.className = 'copy'; b.textContent = 'Copy';
    b.addEventListener('click', function () {
      var code = pre.querySelector('code');
      navigator.clipboard.writeText((code || pre).innerText.replace(/^\$ /gm, '')).then(function () {
        b.textContent = 'Copied';
        setTimeout(function () { b.textContent = 'Copy'; }, 1400);
      });
    });
    pre.appendChild(b);
  });

  // ── "On this page" highlight ───────────────────────────────────────────
  var tocLinks = Array.prototype.slice.call(document.querySelectorAll('.toc a'));
  if (tocLinks.length && 'IntersectionObserver' in window) {
    var byId = {};
    tocLinks.forEach(function (a) { byId[a.getAttribute('href').slice(1)] = a; });
    var heads = Array.prototype.slice.call(document.querySelectorAll('article h2[id], article h3[id]'));
    var current = null;
    var spy = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) { if (en.isIntersecting) current = en.target.id; });
      if (!current || !byId[current]) return;
      tocLinks.forEach(function (a) { a.classList.remove('active'); });
      byId[current].classList.add('active');
    }, { rootMargin: '-80px 0px -70% 0px' });
    heads.forEach(function (h) { spy.observe(h); });
  }

  // ── Bars grow when they scroll into view ───────────────────────────────
  var groups = document.querySelectorAll('.bars');
  if ('IntersectionObserver' in window) {
    var io = new IntersectionObserver(function (entries) {
      entries.forEach(function (en) { if (en.isIntersecting) { en.target.classList.add('in-view'); io.unobserve(en.target); } });
    }, { threshold: 0.25 });
    groups.forEach(function (g) { io.observe(g); });
  } else {
    groups.forEach(function (g) { g.classList.add('in-view'); });
  }
})();
