'use strict';

// Home page: lists the articles registered in pages/pages.json and provides search.
(async function () {
  const MANIFEST = 'pages/pages.json';
  const $ = (id) => document.getElementById(id);

  let data = { siteTitle: 'Wiki', pages: [] };
  try {
    const res = await fetch(MANIFEST, { cache: 'no-store' });
    if (res.ok) data = await res.json();
  } catch (e) {
    if (location.protocol === 'file:') {
      $('empty').hidden = false;
      $('empty').textContent =
        'This site loads its articles with fetch(), which browsers block on file:// URLs. ' +
        'Serve the folder over HTTP (for example: python3 -m http.server) or publish it with GitHub Pages.';
      return;
    }
  }

  const pages = (data.pages || []).slice().sort((a, b) => a.title.localeCompare(b.title));
  document.title = data.siteTitle || 'Wiki';
  $('site-title').textContent = data.siteTitle || 'Wiki';
  $('home-title').textContent = 'Welcome to ' + (data.siteTitle || 'the wiki');

  const href = (p) => 'page/?p=' + encodeURIComponent(p.slug);
  const fill = (ul) => {
    ul.innerHTML = '';
    for (const p of pages) {
      const li = document.createElement('li');
      const a = document.createElement('a');
      a.href = href(p);
      a.textContent = p.title;
      li.appendChild(a);
      ul.appendChild(li);
    }
  };
  fill($('article-list'));
  fill($('sidebar-list'));
  $('empty').hidden = pages.length > 0;

  // ----- search -----
  const input = $('search');
  const box = $('search-results');
  let active = -1;
  const show = () => {
    const q = input.value.trim().toLowerCase();
    box.innerHTML = '';
    active = -1;
    if (!q) { box.style.display = 'none'; return; }
    const hits = pages.filter((p) => p.title.toLowerCase().includes(q) || p.slug.includes(q)).slice(0, 12);
    for (const p of hits) {
      const a = document.createElement('a');
      a.href = href(p);
      a.textContent = p.title;
      box.appendChild(a);
    }
    if (!hits.length) {
      const d = document.createElement('div');
      d.className = 'muted';
      d.style.padding = '0.5rem 0.75rem';
      d.textContent = 'No matching articles';
      box.appendChild(d);
    }
    box.style.display = 'block';
  };
  input.addEventListener('input', show);
  input.addEventListener('keydown', (e) => {
    const items = [...box.querySelectorAll('a')];
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!items.length) return;
      active = (active + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
      items.forEach((a, i) => a.classList.toggle('active', i === active));
    } else if (e.key === 'Enter') {
      const target = items[active >= 0 ? active : 0];
      if (target) location.href = target.href;
    } else if (e.key === 'Escape') {
      box.style.display = 'none';
    }
  });
  document.addEventListener('click', (e) => { if (!e.target.closest('.wk-search')) box.style.display = 'none'; });
})();
