'use strict';

/* ==========================================================================
   Home page.
   Reads pages/pages.json and provides:
     - the featured articles (sidebar and list),
     - a paged, filterable list of every article (the site can have thousands),
     - an interactive site map: articles are nodes, [[cross references]] are edges,
     - header search.
   pages.json: { siteTitle, logo, featured: [slug], pages: [{ slug, title, links: [slug] }] }
   ========================================================================== */

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/* ---------------------------------------------------------------- graph layout
   A small force-directed layout (no libraries): nodes repel each other, edges act as springs,
   and a weak pull toward the centre keeps separate clusters on screen. Repulsion only looks at
   nearby nodes (a spatial grid), so thousands of articles stay fast. */

class GraphLayout {
  constructor(nodes, edges) {
    this.nodes = nodes;
    this.edges = edges;       // [{ s, t, k }] with node indices
    this.alpha = 1;           // "temperature": shrinks as the layout settles
    this.seed = 12345;
    // start on a spiral so that no two nodes share a position
    nodes.forEach((nd, i) => {
      const r = 16 * Math.sqrt(i + 0.5), a = i * 2.399963;
      nd.x = r * Math.cos(a); nd.y = r * Math.sin(a); nd.vx = 0; nd.vy = 0; nd.pinned = false;
    });
  }
  rand() { this.seed = (this.seed * 1664525 + 1013904223) % 4294967296; return this.seed / 4294967296; }
  get settled() { return this.alpha < 0.03; }
  reheat(a) { this.alpha = Math.max(this.alpha, a); }

  tick() {
    const nodes = this.nodes, n = nodes.length, alpha = this.alpha;
    const CELL = 110, REP = 520, LINK = 42, SPRING = 0.09, GRAVITY = 0.012, DAMP = 0.62, MAXV = 28;
    const grid = new Map();
    for (let i = 0; i < n; i++) {
      const p = nodes[i];
      p.fx = 0; p.fy = 0;
      const key = Math.floor(p.x / CELL) * 100003 + Math.floor(p.y / CELL);
      const cell = grid.get(key);
      if (cell) cell.push(i); else grid.set(key, [i]);
    }
    for (let i = 0; i < n; i++) {
      const p = nodes[i], cx = Math.floor(p.x / CELL), cy = Math.floor(p.y / CELL);
      for (let gx = cx - 1; gx <= cx + 1; gx++) {
        for (let gy = cy - 1; gy <= cy + 1; gy++) {
          const cell = grid.get(gx * 100003 + gy);
          if (!cell) continue;
          for (let c = 0; c < cell.length; c++) {
            const j = cell[c];
            if (j <= i) continue;
            const q = nodes[j];
            let dx = p.x - q.x, dy = p.y - q.y, d2 = dx * dx + dy * dy;
            if (d2 > CELL * CELL) continue;
            if (d2 < 0.01) { dx = this.rand() - 0.5; dy = this.rand() - 0.5; d2 = dx * dx + dy * dy + 0.01; }
            const f = REP / d2;
            p.fx += dx * f; p.fy += dy * f; q.fx -= dx * f; q.fy -= dy * f;
          }
        }
      }
    }
    for (const e of this.edges) {
      const a = nodes[e.s], b = nodes[e.t];
      const dx = b.x - a.x, dy = b.y - a.y, d = Math.sqrt(dx * dx + dy * dy) || 0.01;
      const f = ((d - LINK) * SPRING * e.k) / d;
      a.fx += dx * f; a.fy += dy * f; b.fx -= dx * f; b.fy -= dy * f;
    }
    for (let i = 0; i < n; i++) {
      const p = nodes[i];
      if (p.pinned) { p.vx = 0; p.vy = 0; continue; }
      p.fx -= p.x * GRAVITY; p.fy -= p.y * GRAVITY;
      p.vx = (p.vx + p.fx * alpha) * DAMP; p.vy = (p.vy + p.fy * alpha) * DAMP;
      const v = Math.sqrt(p.vx * p.vx + p.vy * p.vy);
      if (v > MAXV) { p.vx *= MAXV / v; p.vy *= MAXV / v; }
      p.x += p.vx; p.y += p.vy;
    }
    this.alpha *= 0.985;
  }

  // Runs ticks until settled or until `ms` milliseconds have passed.
  run(ms) {
    const end = Date.now() + ms;
    while (!this.settled && Date.now() < end) this.tick();
  }
}

// Builds nodes and (undirected, de-duplicated) edges from the manifest.
function buildGraph(pages, featuredSet) {
  const index = new Map();
  const nodes = pages.map((p, i) => { index.set(p.slug, i); return { slug: p.slug, title: p.title, deg: 0, featured: !featuredSet || featuredSet.has(p.slug), nbrs: [] }; });
  const seen = new Set();
  const raw = [];
  pages.forEach((p, i) => {
    for (const l of p.links || []) {
      const j = index.get(l);
      if (j === undefined || j === i) continue;
      const key = i < j ? i + ':' + j : j + ':' + i;
      if (seen.has(key)) continue;
      seen.add(key);
      raw.push([i, j]);
      nodes[i].deg++; nodes[j].deg++;
      nodes[i].nbrs.push(j); nodes[j].nbrs.push(i);
    }
  });
  // springs on hubs are weaker so that a node with many links does not collapse its neighbours
  const edges = raw.map(([s, t]) => ({ s, t, k: 1 / Math.min(Math.max(nodes[s].deg, 1), Math.max(nodes[t].deg, 1)) ** 0.5 }));
  return { nodes, edges };
}

/* ---------------------------------------------------------------- graph view (canvas) */

class GraphView {
  constructor(host, graph, hrefFor) {
    this.graph = graph;
    this.hrefFor = hrefFor;
    this.layout = new GraphLayout(graph.nodes, graph.edges);
    this.k = 1; this.tx = 0; this.ty = 0;       // world -> screen: screen = world * k + t
    this.w = 600; this.h = 420; this.dpr = window.devicePixelRatio || 1;
    this.selected = -1; this.hover = -1;
    this.touched = false;                       // the reader has panned or zoomed: stop auto-fitting
    this.scrollZoom = false;                    // wheel zoom only after a click, so the page can still scroll
    this.scheduled = false;

    host.innerHTML =
      '<div class="gv-wrap"><canvas class="gv-canvas" tabindex="0" role="img" aria-label="Site map: articles as dots, cross references as lines. Click a dot for its article."></canvas>' +
      '<div class="gv-pop" hidden></div>' +
      '<div class="gv-ctl"><button type="button" data-a="in" aria-label="Zoom in">+</button><button type="button" data-a="out" aria-label="Zoom out">&minus;</button><button type="button" data-a="fit">Fit</button></div></div>' +
      `<div class="widget-note">${graph.nodes.length} article${graph.nodes.length === 1 ? '' : 's'}, ${graph.edges.length} cross reference${graph.edges.length === 1 ? '' : 's'}. ` +
      'Click a dot to see the article and its link; drag a dot to move it, drag the background to pan, click the map and scroll to zoom. Darker dots are featured articles.</div>';
    this.wrap = host.querySelector('.gv-wrap');
    this.canvas = host.querySelector('canvas');
    this.pop = host.querySelector('.gv-pop');
    this.ctx = this.canvas.getContext('2d');
    this.measure();

    const reduce = window.matchMedia && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    this.layout.run(reduce ? 4000 : 350);       // most of the layout happens before the first paint
    this.fit();
    this.bind();
    if (window.ResizeObserver) new ResizeObserver(() => { this.measure(); this.kick(); }).observe(this.wrap);
    this.kick();
  }

  measure() {
    const r = this.wrap.getBoundingClientRect();
    this.w = Math.max(200, r.width || 600); this.h = Math.max(200, r.height || 420);
    this.dpr = window.devicePixelRatio || 1;
    this.canvas.width = Math.round(this.w * this.dpr); this.canvas.height = Math.round(this.h * this.dpr);
    if (!this.touched) this.fit();
  }

  radius(nd) { return 4 + Math.sqrt(nd.deg) * 1.7; }

  fit() {
    const ns = this.graph.nodes;
    if (!ns.length) return;
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const n of ns) { x0 = Math.min(x0, n.x); y0 = Math.min(y0, n.y); x1 = Math.max(x1, n.x); y1 = Math.max(y1, n.y); }
    const pad = 36, bw = Math.max(x1 - x0, 1), bh = Math.max(y1 - y0, 1);
    this.k = Math.max(0.03, Math.min((this.w - 2 * pad) / bw, (this.h - 2 * pad) / bh, 2.2));
    this.tx = this.w / 2 - ((x0 + x1) / 2) * this.k;
    this.ty = this.h / 2 - ((y0 + y1) / 2) * this.k;
  }

  toWorld(sx, sy) { return [(sx - this.tx) / this.k, (sy - this.ty) / this.k]; }
  pointer(e) { const r = this.canvas.getBoundingClientRect(); return [e.clientX - r.left, e.clientY - r.top]; }

  hit(sx, sy) {
    const ns = this.graph.nodes;
    let best = -1, bd = Infinity;
    for (let i = 0; i < ns.length; i++) {
      const dx = ns[i].x * this.k + this.tx - sx, dy = ns[i].y * this.k + this.ty - sy;
      const d = dx * dx + dy * dy, r = this.screenRadius(ns[i]) + 5;
      if (d <= r * r && d < bd) { bd = d; best = i; }
    }
    return best;
  }
  screenRadius(nd) { return this.radius(nd) * Math.max(0.7, Math.min(1.8, Math.sqrt(this.k))); }

  bind() {
    const c = this.canvas;
    let drag = null;
    c.addEventListener('pointerdown', (e) => {
      if (e.button !== 0) return;
      const [sx, sy] = this.pointer(e);
      const i = this.hit(sx, sy);
      this.scrollZoom = true;
      c.setPointerCapture(e.pointerId);
      drag = { node: i, sx, sy, moved: false, tx: this.tx, ty: this.ty };
      if (i >= 0) this.graph.nodes[i].pinned = true;
    });
    c.addEventListener('pointermove', (e) => {
      const [sx, sy] = this.pointer(e);
      if (!drag) {
        const i = this.hit(sx, sy);
        if (i !== this.hover) { this.hover = i; c.style.cursor = i >= 0 ? 'pointer' : 'grab'; this.kick(); }
        return;
      }
      if (Math.abs(sx - drag.sx) + Math.abs(sy - drag.sy) > 3) drag.moved = true;
      if (!drag.moved) return;
      this.touched = true;
      if (drag.node >= 0) {
        const nd = this.graph.nodes[drag.node];
        [nd.x, nd.y] = this.toWorld(sx, sy);
        this.layout.reheat(0.3);
      } else {
        this.tx = drag.tx + (sx - drag.sx); this.ty = drag.ty + (sy - drag.sy);
      }
      this.kick();
    });
    const end = () => {
      if (!drag) return;
      const d = drag; drag = null;
      if (d.node >= 0) this.graph.nodes[d.node].pinned = false;
      if (!d.moved) this.select(d.node);       // a click on a dot selects it; on the background it clears
      this.kick();
    };
    c.addEventListener('pointerup', end);
    c.addEventListener('pointercancel', end);
    c.addEventListener('pointerleave', () => { this.scrollZoom = false; if (this.hover !== -1) { this.hover = -1; this.kick(); } });
    c.addEventListener('wheel', (e) => {
      if (!this.scrollZoom && !e.ctrlKey && !e.metaKey) return;
      e.preventDefault();
      const [sx, sy] = this.pointer(e);
      this.zoomAt(sx, sy, Math.exp(-e.deltaY * 0.0015));
    }, { passive: false });
    c.addEventListener('keydown', (e) => {
      const n = this.graph.nodes.length;
      if (!n) return;
      if (e.key === 'ArrowRight' || e.key === 'ArrowDown') { e.preventDefault(); this.select(this.selected < 0 ? 0 : (this.selected + 1) % n); }
      else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') { e.preventDefault(); this.select(this.selected < 0 ? n - 1 : (this.selected - 1 + n) % n); }
      else if (e.key === 'Escape') this.select(-1);
    });
    this.wrap.querySelector('.gv-ctl').addEventListener('click', (e) => {
      const a = e.target.dataset && e.target.dataset.a;
      if (a === 'in') this.zoomAt(this.w / 2, this.h / 2, 1.4);
      else if (a === 'out') this.zoomAt(this.w / 2, this.h / 2, 1 / 1.4);
      else if (a === 'fit') { this.touched = false; this.fit(); this.kick(); }
    });
    this.pop.addEventListener('click', (e) => { if (e.target.closest('.gv-x')) this.select(-1); });
  }

  zoomAt(sx, sy, f) {
    const k = Math.max(0.03, Math.min(10, this.k * f));
    const [wx, wy] = this.toWorld(sx, sy);
    this.k = k; this.tx = sx - wx * k; this.ty = sy - wy * k;
    this.touched = true;
    this.kick();
  }

  select(i) {
    this.selected = i;
    if (i < 0) { this.pop.hidden = true; this.kick(); return; }
    const nd = this.graph.nodes[i];
    this.pop.innerHTML =
      '<button type="button" class="gv-x" aria-label="Close">&times;</button>' +
      `<strong>${esc(nd.title)}</strong>` +
      `<a href="${esc(this.hrefFor(nd))}">Open article &rarr;</a>` +
      `<span class="gv-meta">${nd.deg} cross reference${nd.deg === 1 ? '' : 's'}${nd.featured ? ', featured' : ''}</span>`;
    this.pop.hidden = false;
    this.kick();
  }

  kick() {
    if (this.scheduled) return;
    this.scheduled = true;
    requestAnimationFrame(() => {
      this.scheduled = false;
      if (!this.layout.settled) {
        for (let i = 0; i < 2 && !this.layout.settled; i++) this.layout.tick();
        if (!this.touched) this.fit();
      }
      this.draw();
      if (!this.layout.settled) this.kick();
    });
  }

  draw() {
    const ctx = this.ctx, ns = this.graph.nodes, es = this.graph.edges, k = this.k;
    const css = getComputedStyle(document.documentElement);
    const accent = css.getPropertyValue('--accent').trim() || '#3366cc';
    ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    ctx.clearRect(0, 0, this.w, this.h);
    const sel = this.selected >= 0 ? ns[this.selected] : null;
    const near = new Set(sel ? [this.selected, ...sel.nbrs] : []);

    // edges: all in one path, then the selected node's edges on top
    ctx.lineWidth = 1;
    ctx.strokeStyle = sel ? 'rgba(120,130,150,0.22)' : 'rgba(110,120,140,0.42)';
    ctx.beginPath();
    for (const e of es) {
      if (sel && (e.s === this.selected || e.t === this.selected)) continue;
      const a = ns[e.s], b = ns[e.t];
      ctx.moveTo(a.x * k + this.tx, a.y * k + this.ty); ctx.lineTo(b.x * k + this.tx, b.y * k + this.ty);
    }
    ctx.stroke();
    if (sel) {
      ctx.strokeStyle = accent; ctx.lineWidth = 2;
      ctx.beginPath();
      for (const e of es) {
        if (e.s !== this.selected && e.t !== this.selected) continue;
        const a = ns[e.s], b = ns[e.t];
        ctx.moveTo(a.x * k + this.tx, a.y * k + this.ty); ctx.lineTo(b.x * k + this.tx, b.y * k + this.ty);
      }
      ctx.stroke();
    }

    // nodes
    for (let i = 0; i < ns.length; i++) {
      const nd = ns[i], x = nd.x * k + this.tx, y = nd.y * k + this.ty;
      if (x < -20 || y < -20 || x > this.w + 20 || y > this.h + 20) continue;
      const r = this.screenRadius(nd);
      ctx.beginPath(); ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fillStyle = nd.featured ? '#1f4fa8' : '#8fa9dc';
      ctx.globalAlpha = sel && !near.has(i) ? 0.35 : 1;
      ctx.fill();
      ctx.globalAlpha = 1;
      if (i === this.selected || i === this.hover) { ctx.lineWidth = 2.5; ctx.strokeStyle = '#d33'; ctx.stroke(); }
    }

    // labels: the selected node and its neighbours, the hovered node, and everything once zoomed in
    ctx.font = '12px system-ui, sans-serif'; ctx.textAlign = 'left'; ctx.textBaseline = 'middle';
    ctx.lineJoin = 'round'; ctx.lineWidth = 3.5; 
    for (let i = 0; i < ns.length; i++) {
      const show = i === this.hover || near.has(i) || k >= 1.6;
      if (!show) continue;
      const nd = ns[i], x = nd.x * k + this.tx, y = nd.y * k + this.ty;
      if (x < -20 || y < -20 || x > this.w + 20 || y > this.h + 20) continue;
      const label = nd.title.length > 28 ? nd.title.slice(0, 27) + '\u2026' : nd.title;
      const lx = x + this.screenRadius(nd) + 4;
      ctx.strokeStyle = 'rgba(255,255,255,0.92)'; ctx.strokeText(label, lx, y);
      ctx.fillStyle = '#202122'; ctx.fillText(label, lx, y);
    }

    // popup follows the selected node
    if (sel) {
      const x = sel.x * k + this.tx, y = sel.y * k + this.ty;
      const pw = this.pop.offsetWidth || 180, ph = this.pop.offsetHeight || 70;
      let left = x + this.screenRadius(sel) + 8, top = y - ph / 2;
      if (left + pw > this.w - 4) left = x - this.screenRadius(sel) - 8 - pw;
      left = Math.max(4, Math.min(this.w - pw - 4, left));
      top = Math.max(4, Math.min(this.h - ph - 4, top));
      this.pop.style.left = left + 'px'; this.pop.style.top = top + 'px';
    }
  }
}

/* ---------------------------------------------------------------- page */

if (typeof document !== 'undefined') {
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
        $('map').hidden = true; $('all').hidden = true;
        return;
      }
    }

    const pages = (data.pages || []).slice().sort((a, b) => a.title.localeCompare(b.title));
    // Older manifests have no featured list: treat every article as featured.
    const featuredSet = Array.isArray(data.featured) ? new Set(data.featured) : null;
    const featured = pages.filter((p) => !featuredSet || featuredSet.has(p.slug));
    document.title = data.siteTitle || 'Wiki';
    $('site-title').textContent = data.siteTitle || 'Wiki';
    if (data.logo) {
      $('site-logo').src = data.logo;
      $('site-logo').hidden = false;
      $('site-globe').hidden = true;
      $('favicon').href = data.logo;
    }
    $('home-title').textContent = 'Welcome to ' + (data.siteTitle || 'the wiki');

    const href = (p) => 'page/?p=' + encodeURIComponent(p.slug);
    const fill = (ul, list) => {
      ul.innerHTML = '';
      for (const p of list) {
        const li = document.createElement('li');
        const a = document.createElement('a');
        a.href = href(p);
        a.textContent = p.title;
        li.appendChild(a);
        ul.appendChild(li);
      }
    };
    fill($('article-list'), featured);
    fill($('sidebar-list'), featured);
    $('empty').hidden = pages.length > 0;
    $('no-featured').hidden = pages.length === 0 || featured.length > 0;

    // ----- all articles: filter + pages of 100 -----
    const PAGE = 100;
    let allQuery = '', allPage = 0;
    const renderAll = () => {
      const q = allQuery.trim().toLowerCase();
      const hits = q ? pages.filter((p) => p.title.toLowerCase().includes(q) || p.slug.includes(q)) : pages;
      const count = Math.max(1, Math.ceil(hits.length / PAGE));
      allPage = Math.min(allPage, count - 1);
      fill($('all-list'), hits.slice(allPage * PAGE, (allPage + 1) * PAGE));
      $('all-info').textContent = `${hits.length} article${hits.length === 1 ? '' : 's'}` + (count > 1 ? `, page ${allPage + 1} of ${count}` : '');
      $('all-prev').disabled = allPage === 0;
      $('all-next').disabled = allPage >= count - 1;
      $('all-prev').hidden = $('all-next').hidden = count === 1;
    };
    $('all-filter').addEventListener('input', (e) => { allQuery = e.target.value; allPage = 0; renderAll(); });
    $('all-prev').addEventListener('click', () => { allPage--; renderAll(); });
    $('all-next').addEventListener('click', () => { allPage++; renderAll(); });
    renderAll();

    // ----- site map -----
    if (pages.length) {
      new GraphView($('graph'), buildGraph(pages, featuredSet), href);
    } else {
      $('map').hidden = true;
    }

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
}

if (typeof module !== 'undefined' && module.exports) module.exports = { GraphLayout, buildGraph };
