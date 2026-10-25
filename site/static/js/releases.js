/* Ferrite site: downloads. Asks GitHub for the releases once (cached for ten
   minutes, because unauthenticated API calls are limited to 60 an hour), then
   fills the latest-build card, the per-platform buttons and the list of every
   earlier build. With JavaScript off, or if GitHub can't be reached, every
   button still links to the releases page. */
(function () {
  'use strict';
  var REPO = 'rayanjainn/Ferrite-Browser';
  var PAGE = 'https://github.com/' + REPO + '/releases';
  var API = 'https://api.github.com/repos/' + REPO + '/releases?per_page=60';
  var CACHE = 'ferrite-releases-v2';
  var OS = {
    macos: { name: 'macOS', arch: 'Apple silicon', key: 'macos-arm64' },
    windows: { name: 'Windows', arch: 'x64', key: 'windows-x64' },
    linux: { name: 'Linux', arch: 'x64', key: 'linux-x64' }
  };
  var ASSET = /^ferrite-([0-9a-f]{7,})-(macos-arm64|windows-x64|linux-x64)(-media)?\.(zip|tar\.gz)$/;

  var wants = document.querySelector('[data-releases]');
  if (!wants) return;

  function mine() {
    var ua = (navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || navigator.userAgent || '';
    if (/iphone|ipad|android/i.test(navigator.userAgent)) return '';
    return /mac/i.test(ua) ? 'macos' : /win/i.test(ua) ? 'windows' : /linux|x11/i.test(ua) ? 'linux' : '';
  }
  function mb(n) { return n >= 1e8 ? Math.round(n / 1048576) + ' MB' : (n / 1048576).toFixed(1) + ' MB'; }
  function day(iso) { try { return new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' }); } catch (e) { return iso.slice(0, 10); } }
  function esc(s) { return String(s).replace(/[&<>"]/g, function (c) { return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]; }); }
  function all(sel) { return Array.prototype.slice.call(document.querySelectorAll(sel)); }
  function set(sel, text) { all(sel).forEach(function (el) { el.textContent = text; }); }

  function load() {
    try {
      var hit = JSON.parse(sessionStorage.getItem(CACHE) || 'null');
      if (hit && Date.now() - hit.at < 600000) return Promise.resolve(hit.data);
    } catch (e) {}
    return fetch(API, { headers: { Accept: 'application/vnd.github+json' } })
      .then(function (r) {
        if (r.status === 403 || r.status === 429) throw new Error('rate');
        if (!r.ok) throw new Error(String(r.status));
        return r.json();
      })
      .then(function (data) {
        try { sessionStorage.setItem(CACHE, JSON.stringify({ at: Date.now(), data: data })); } catch (e) {}
        return data;
      });
  }

  // One release, reduced to what the page shows.
  function shape(rel) {
    var files = {}, sha = '';
    (rel.assets || []).forEach(function (a) {
      var m = ASSET.exec(a.name);
      if (!m) return;
      sha = m[1];
      files[m[2] + (m[3] ? '-media' : '')] = { name: a.name, url: a.browser_download_url, size: a.size };
    });
    var num = /build-(\d+)/.exec(rel.tag_name) || /build (\d+)/.exec(rel.name || '');
    return { tag: rel.tag_name, name: rel.name || rel.tag_name, num: num ? Number(num[1]) : null, sha: sha, date: rel.published_at, url: rel.html_url, files: files, zip: rel.zipball_url, tar: rel.tarball_url };
  }

  function status(kind, text) {
    all('[data-release-status]').forEach(function (el) { el.className = 'status-line ' + kind; el.innerHTML = '<i></i>' + esc(text); });
  }

  function fileButton(f, label, primary) {
    if (!f) return '<a class="file-btn" aria-disabled="true" href="' + PAGE + '">' + esc(label) + '<small>not in this build</small></a>';
    return '<a class="file-btn' + (primary ? ' primary' : '') + '" href="' + esc(f.url) + '">' + esc(label) + '<small>' + mb(f.size) + '</small></a>';
  }

  function fillLatest(latest) {
    var here = mine();
    set('[data-latest="build"]', latest.num ? 'Build ' + latest.num : latest.name);
    set('[data-latest="date"]', day(latest.date));
    set('[data-latest="commit"]', latest.sha || '—');
    all('[data-latest="commit-link"]').forEach(function (a) { if (latest.sha) a.href = 'https://github.com/' + REPO + '/commit/' + latest.sha; });

    Object.keys(OS).forEach(function (key) {
      var card = document.querySelector('[data-os-card="' + key + '"]');
      if (!card) return;
      card.classList.toggle('is-mine', key === here);
      var files = card.querySelector('[data-files]');
      if (files) {
        files.innerHTML = fileButton(latest.files[OS[key].key], 'Download for ' + OS[key].name, key === here) +
          fileButton(latest.files[OS[key].key + '-media'], 'Media build (experimental)', false);
      }
    });

    var big = document.getElementById('dl-big');
    if (big) {
      var f = here && latest.files[OS[here].key];
      var label = big.querySelector('[data-big-label]');
      if (f) {
        big.href = f.url;
        label.textContent = 'Download for ' + OS[here].name;
        set('[data-big-meta]', f.name + ' · ' + mb(f.size) + ' · ' + OS[here].arch);
      } else {
        big.href = '#platforms';
        label.textContent = 'Choose your platform';
        set('[data-big-meta]', 'macOS · Windows · Linux');
      }
    }
  }

  function fillHistory(builds) {
    var box = document.getElementById('history-list');
    if (!box) return;
    var labels = [['macos-arm64', 'macOS · Apple silicon'], ['windows-x64', 'Windows · x64'], ['linux-x64', 'Linux · x64'],
      ['macos-arm64-media', 'macOS · media'], ['windows-x64-media', 'Windows · media'], ['linux-x64-media', 'Linux · media']];
    box.innerHTML = builds.map(function (b, i) {
      var assets = labels.filter(function (l) { return b.files[l[0]]; }).map(function (l) { return fileButton(b.files[l[0]], l[1], false); }).join('');
      assets += '<a class="file-btn" href="' + esc(b.zip) + '">Source code<small>zip</small></a>' +
        '<a class="file-btn" href="' + esc(b.url) + '" rel="noopener">Release notes<small>GitHub</small></a>';
      return '<details class="rel" data-q="' + esc((b.name + ' ' + b.tag + ' ' + b.sha).toLowerCase()) + '"' + (i === 0 ? ' open' : '') + '>' +
        '<summary><span class="tag">' + esc(b.tag) + '</span><b>' + esc(b.num ? 'Build ' + b.num : b.name) +
        (i === 0 ? '<span class="badge">newest</span>' : '') + '</b><span class="when">' + day(b.date) + (b.sha ? ' · ' + esc(b.sha) : '') + '</span>' +
        '<svg class="chev" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6"/></svg></summary>' +
        '<div class="assets">' + assets + '</div></details>';
    }).join('');
    set('[data-history-count]', String(builds.length));
    var filter = document.querySelector('[data-history-filter]');
    if (filter) {
      filter.addEventListener('input', function () {
        var q = filter.value.trim().toLowerCase();
        all('#history-list .rel').forEach(function (el) { el.hidden = q && el.getAttribute('data-q').indexOf(q) < 0; });
      });
    }
  }

  load().then(function (data) {
    var rels = (data || []).filter(function (r) { return !r.draft; }).map(shape).filter(function (r) { return Object.keys(r.files).length; });
    if (!rels.length) { status('bad', 'No build is published yet. Build from source below.'); return; }
    var latest = rels.filter(function (r) { return r.tag === 'latest'; })[0] || rels[0];
    // "latest" is a moving copy of the newest numbered build; show its number.
    var twin = rels.filter(function (r) { return r.tag !== 'latest' && r.sha === latest.sha; })[0];
    if (twin) { latest.num = twin.num; latest.name = twin.name; }
    var builds = rels.filter(function (r) { return r.tag !== 'latest'; }).sort(function (a, b) { return (b.num || 0) - (a.num || 0) || (b.date > a.date ? 1 : -1); });
    fillLatest(latest);
    fillHistory(builds);
    status('ok', 'Latest build · ' + day(latest.date));
  }).catch(function (err) {
    status('bad', err && err.message === 'rate'
      ? 'GitHub’s hourly limit for your network was reached. Use the releases page.'
      : 'Could not reach GitHub. Use the releases page.');
    set('[data-latest="build"]', 'See releases');
  });
})();
