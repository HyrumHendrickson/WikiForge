'use strict';

/* ==========================================================================
   WikiForge article viewer.
   Loads ../pages/<slug>.md (slug comes from ?p=slug), converts it to HTML and
   brings the interactive widgets (code windows, sheets, maps, Desmos, 3D) to life.
   ========================================================================== */

const CONFIG = {
  manifest: '../pages/pages.json',
  pagesDir: '../pages/',
  // Desmos' public demo key. Replace with your own key from https://www.desmos.com/api
  desmosKey: 'dcb31709b452b1cf9dc26972add0fda6',
  canvasWidth: 480,
  canvasHeight: 320,
  // Remote runners used for compiled languages (they run on someone else's server).
  rustPlayground: 'https://play.rust-lang.org/execute',
  godbolt: 'https://godbolt.org/api',
  libs: {
    katexCss: 'https://cdnjs.cloudflare.com/ajax/libs/KaTeX/0.16.11/katex.min.css',
    katex: 'https://cdnjs.cloudflare.com/ajax/libs/KaTeX/0.16.11/katex.min.js',
    cmCss: 'https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/codemirror.min.css',
    cm: [
      'https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/codemirror.min.js',
      'https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/addon/mode/simple.min.js',
      'https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/mode/clike/clike.min.js',
      'https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/mode/python/python.min.js',
      'https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/mode/rust/rust.min.js',
      'https://cdnjs.cloudflare.com/ajax/libs/codemirror/5.65.16/mode/javascript/javascript.min.js',
    ],
    leafletCss: 'https://cdnjs.cloudflare.com/ajax/libs/leaflet/1.9.4/leaflet.min.css',
    leaflet: 'https://cdnjs.cloudflare.com/ajax/libs/leaflet/1.9.4/leaflet.min.js',
    pyodide: 'https://cdn.jsdelivr.net/pyodide/v0.26.4/full/pyodide.js',
    modelViewer: 'https://ajax.googleapis.com/ajax/libs/model-viewer/4.0.0/model-viewer.min.js',
  },
};

/* ---------------------------------------------------------------- helpers */

const $ = (id) => document.getElementById(id);
const esc = (s) =>
  String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const slugify = (s) =>
  String(s).trim().toLowerCase().replace(/[^\p{L}\p{N}\s_-]/gu, '').replace(/\s+/g, '-') || 'section';

const loaded = new Map();
function loadScript(url, asModule) {
  if (!loaded.has(url)) {
    loaded.set(url, new Promise((resolve, reject) => {
      const s = document.createElement('script');
      s.src = url;
      if (asModule) s.type = 'module';
      s.onload = resolve;
      s.onerror = () => reject(new Error('Failed to load ' + url));
      document.head.appendChild(s);
    }));
  }
  return loaded.get(url);
}
function loadCss(url) {
  if (loaded.has(url)) return;
  loaded.set(url, Promise.resolve());
  const l = document.createElement('link');
  l.rel = 'stylesheet';
  l.href = url;
  document.head.appendChild(l);
}
async function loadSeq(urls) { for (const u of urls) await loadScript(u); }

// Run `fn` the first time `el` approaches the viewport.
function lazy(el, fn) {
  if (!('IntersectionObserver' in window)) { fn(); return; }
  const io = new IntersectionObserver((entries) => {
    if (entries.some((e) => e.isIntersecting)) { io.disconnect(); fn(); }
  }, { rootMargin: '300px' });
  io.observe(el);
}

// key=value, key="value with spaces", or a bare flag.
function parseAttrs(s) {
  const out = {};
  const re = /([\w-]+)(?:=(?:"([^"]*)"|(\S+)))?/g;
  let m;
  while ((m = re.exec(s || ''))) out[m[1]] = m[2] !== undefined ? m[2] : m[3] !== undefined ? m[3] : '1';
  return out;
}

// Body lines of "key: value" pairs (blank lines and # comments ignored).
function parseKeyed(text) {
  const kv = [];
  const rest = [];
  for (const raw of text.split('\n')) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    const m = /^([A-Za-z][\w -]*?)\s*:\s*(.*)$/.exec(line);
    if (m) kv.push([m[1].toLowerCase(), m[2]]); else rest.push(line);
  }
  return { kv, rest };
}

function resolveMedia(u) {
  if (/^(https?:|data:|blob:|\/\/|\/|\.\.\/)/i.test(u)) return u;
  return '../' + u.replace(/^\.\//, '');
}

const VIDEO_RE = /\.(mp4|webm|ogv|mov|m4v)(\?.*)?$/i;
function youtubeId(u) {
  const m = /(?:youtube\.com\/(?:watch\?v=|embed\/)|youtu\.be\/)([\w-]{6,})/.exec(u);
  return m ? m[1] : null;
}

function mediaHtml(url, alt) {
  const yt = youtubeId(url);
  if (yt) return `<iframe src="https://www.youtube-nocookie.com/embed/${yt}" allowfullscreen loading="lazy" title="${esc(alt || 'Video')}"></iframe>`;
  const src = esc(resolveMedia(url));
  if (VIDEO_RE.test(url)) return `<video controls preload="metadata" src="${src}"></video>`;
  return `<img src="${src}" alt="${esc(alt || '')}" loading="lazy">`;
}

/* ---------------------------------------------------------------- links */

function classifyLink(url) {
  if (/^(https?:\/\/|mailto:)/i.test(url)) return { href: url, cls: 'wl-ext', ext: true };
  if (url.startsWith('#')) return { href: url, cls: 'wl-anchor' };
  let m;
  if ((m = /^wiki:([^#]+)(#.*)?$/.exec(url)) || (m = /^([\w-]+)(#[\w-]+)?$/.exec(url))) {
    return { href: '?p=' + encodeURIComponent(slugify(m[1])) + (m[2] || ''), cls: 'wl-int' };
  }
  return { href: resolveMedia(url), cls: 'wl-file' };
}

/* ---------------------------------------------------------------- inline markdown */

function inline(src, ctx) {
  const stash = [];
  const keep = (h) => { stash.push(h); return '\u0000' + (stash.length - 1) + '\u0000'; };

  src = src.replace(/\\([\\`*_{}\[\]()#+\-.!$|~<>])/g, (m, c) => keep(esc(c)));
  src = src.replace(/(`+)([\s\S]*?[^`])\1(?!`)/g, (m, a, code) => keep('<code>' + esc(code.trim()) + '</code>'));
  src = src.replace(/\$\$([^$]+?)\$\$/g, (m, t) => keep(`<span class="math" data-display="1" data-tex="${esc(t)}"></span>`));
  src = src.replace(/\$(?!\s)([^$\n]+?)(?<!\s)\$(?!\d)/g, (m, t) => keep(`<span class="math" data-tex="${esc(t)}"></span>`));
  src = esc(src);
  src = src.replace(/\u0001/g, '<br>');

  const emph = (t) => t
    .replace(/\*\*(?=\S)([\s\S]*?\S)\*\*/g, '<strong>$1</strong>')
    .replace(/\*(?=\S)([\s\S]*?\S)\*/g, '<em>$1</em>')
    .replace(/(^|[^\w])__(?=\S)([\s\S]*?\S)__(?=[^\w]|$)/g, '$1<strong>$2</strong>')
    .replace(/(^|[^\w])_(?=\S)([\s\S]*?\S)_(?=[^\w]|$)/g, '$1<em>$2</em>')
    .replace(/~~(?=\S)([\s\S]*?\S)~~/g, '<s>$1</s>');

  // images (also videos and YouTube links)
  src = src.replace(/!\[([^\]]*)\]\(([^)\s]+)(?:\s+&quot;([^&]*?)&quot;)?\)/g,
    (m, alt, url) => keep(mediaHtml(url.replace(/&amp;/g, '&'), alt)));

  // [[Page name|label]]
  src = src.replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (m, page, label) => {
    const info = classifyLink('wiki:' + page.trim());
    return keep(`<a class="${info.cls}" href="${esc(info.href)}">${emph(label || page.trim())}</a>`);
  });

  // [label](url "title")
  src = src.replace(/\[([^\]]+)\]\(([^)\s]+)(?:\s+&quot;([^&]*?)&quot;)?\)/g, (m, label, url, title) => {
    const info = classifyLink(url.replace(/&amp;/g, '&'));
    const extra = info.ext ? ' target="_blank" rel="noopener noreferrer"' : '';
    const t = title ? ` title="${title}"` : '';
    return keep(`<a class="${info.cls}" href="${info.href.replace(/&/g, '&amp;')}"${t}${extra}>${emph(label)}</a>`);
  });

  // bare URLs
  src = src.replace(/(^|[\s(])(https?:\/\/[^\s<)]+[^\s<).,;:!?])/g,
    (m, pre, url) => pre + keep(`<a class="wl-ext" href="${url}" target="_blank" rel="noopener noreferrer">${url}</a>`));

  // footnote references
  src = src.replace(/\[\^([^\]\s]+)\]/g, (m, id) => {
    if (!ctx.fnNumber.has(id)) { ctx.fnNumber.set(id, ctx.fnNumber.size + 1); ctx.fnCount.set(id, 0); }
    const n = ctx.fnNumber.get(id);
    const k = ctx.fnCount.get(id) + 1;
    ctx.fnCount.set(id, k);
    const safe = esc(id);
    return keep(`<sup class="fnref" id="fnref-${safe}-${k}"><a href="#fn-${safe}">[${n}]</a></sup>`);
  });

  src = emph(src);

  let guard = 0;
  while (/\u0000\d+\u0000/.test(src) && guard++ < 6) src = src.replace(/\u0000(\d+)\u0000/g, (m, i) => stash[+i]);
  return src;
}

/* ---------------------------------------------------------------- block markdown */

const LANGS = {
  python: ['Python', 'python'], py: ['Python', 'python'],
  c: ['C', 'c'],
  cpp: ['C++', 'cpp'], 'c++': ['C++', 'cpp'],
  rust: ['Rust', 'rust'], rs: ['Rust', 'rust'],
  javascript: ['JavaScript', 'javascript'], js: ['JavaScript', 'javascript'],
  java: ['Java', 'java'],
};

const FONT_MAP = {
  serif: 'Georgia, "Times New Roman", serif',
  sans: 'system-ui, "Segoe UI", Arial, sans-serif',
  'sans-serif': 'system-ui, "Segoe UI", Arial, sans-serif',
  monospace: 'ui-monospace, Menlo, Consolas, monospace',
  mono: 'ui-monospace, Menlo, Consolas, monospace',
};

function styleFromAttrs(a) {
  const css = [];
  if (a.font) {
    const f = FONT_MAP[a.font.toLowerCase()] || `"${a.font.replace(/[^\w \-]/g, '')}", serif`;
    css.push('font-family:' + f);
  }
  if (a.size && +a.size > 0) css.push('font-size:' + (+a.size) + 'px');
  if (a.color && /^#?[\w(),. %-]+$/.test(a.color)) css.push('color:' + a.color);
  if (a.align && /^(left|right|center|justify)$/.test(a.align)) css.push('text-align:' + a.align);
  return css.join(';');
}

function splitRow(line) {
  let s = line.trim();
  if (s.startsWith('|')) s = s.slice(1);
  if (s.endsWith('|') && !s.endsWith('\\|')) s = s.slice(0, -1);
  const cells = [];
  let cur = '';
  for (let i = 0; i < s.length; i++) {
    if (s[i] === '\\' && s[i + 1] === '|') { cur += '|'; i++; }
    else if (s[i] === '|') { cells.push(cur.trim()); cur = ''; }
    else cur += s[i];
  }
  cells.push(cur.trim());
  return cells;
}

const isTableSep = (l) => !!l && /[|]/.test(l) && /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?\s*$/.test(l);
const isListLine = (l) => /^(\s*)([-*+]|\d+[.)])\s+/.test(l);

function renderBlocks(lines, ctx) {
  const out = [];
  let i = 0;
  const startsSpecial = (l) =>
    /^```/.test(l) || /^:::/.test(l) || /^#{1,6}\s/.test(l) || /^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(l) ||
    /^\$\$/.test(l) || /^>/.test(l) || isListLine(l) || /^\[\^[^\]]+\]:/.test(l);

  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) { i++; continue; }

    // fenced block
    let m = /^```\s*([^\s`]*)\s*(.*)$/.exec(line);
    if (m) {
      const lang = m[1].toLowerCase();
      const attrs = parseAttrs(m[2]);
      const body = [];
      i++;
      while (i < lines.length && !/^```\s*$/.test(lines[i])) body.push(lines[i++]);
      i++;
      out.push(renderFence(lang, attrs, body.join('\n'), ctx));
      continue;
    }

    // directive
    m = /^:::\s*([\w-]+)\s*(.*)$/.exec(line);
    if (m) {
      const name = m[1].toLowerCase();
      const attrs = parseAttrs(m[2]);
      const body = [];
      i++;
      let depth = 1;
      while (i < lines.length) {
        if (/^:::\s*[\w-]+/.test(lines[i])) depth++;
        else if (/^:::\s*$/.test(lines[i])) { depth--; if (depth === 0) break; }
        body.push(lines[i++]);
      }
      i++;
      out.push(renderDirective(name, attrs, body, ctx));
      continue;
    }

    // display math
    if (/^\$\$/.test(line)) {
      let tex;
      const one = /^\$\$(.+)\$\$\s*$/.exec(line);
      if (one) { tex = one[1]; i++; }
      else {
        const body = [];
        const first = line.replace(/^\$\$/, '');
        if (first.trim()) body.push(first);
        i++;
        while (i < lines.length && !/\$\$\s*$/.test(lines[i])) body.push(lines[i++]);
        if (i < lines.length) { const last = lines[i].replace(/\$\$\s*$/, ''); if (last.trim()) body.push(last); i++; }
        tex = body.join('\n');
      }
      out.push(`<div class="math-block" data-display="1" data-tex="${esc(tex.trim())}"></div>`);
      continue;
    }

    // heading
    m = /^(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
    if (m) {
      let level = m[1].length;
      const text = m[2];
      if (level === 1 && !ctx.title) { ctx.title = text; i++; continue; }
      if (level === 1) level = 2;
      let id = slugify(text.replace(/[*_`\[\]]/g, ''));
      let n = 2; const base = id;
      while (ctx.ids.has(id)) id = base + '-' + n++;
      ctx.ids.add(id);
      ctx.toc.push({ level, id, text });
      out.push(`<h${level} id="${id}">${inline(text, ctx)}<a class="anchor" href="#${id}">#</a></h${level}>`);
      i++;
      continue;
    }

    // footnote definition
    m = /^\[\^([^\]\s]+)\]:\s*(.*)$/.exec(line);
    if (m) {
      const parts = [m[2]];
      i++;
      while (i < lines.length && /^\s{2,}\S/.test(lines[i])) parts.push(lines[i++].trim());
      ctx.fnDefs.set(m[1], parts.join(' '));
      continue;
    }

    // horizontal rule
    if (/^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(line)) { out.push('<hr>'); i++; continue; }

    // blockquote
    if (/^>/.test(line)) {
      const body = [];
      while (i < lines.length && /^>/.test(lines[i])) body.push(lines[i++].replace(/^>\s?/, ''));
      out.push(`<blockquote>${renderBlocks(body, ctx)}</blockquote>`);
      continue;
    }

    // table
    if (line.includes('|') && isTableSep(lines[i + 1])) {
      const head = splitRow(line);
      const aligns = splitRow(lines[i + 1]).map((c) =>
        /^:-+:$/.test(c) ? 'center' : /^-+:$/.test(c) ? 'right' : /^:-+$/.test(c) ? 'left' : '');
      i += 2;
      const rows = [];
      while (i < lines.length && lines[i].trim() && lines[i].includes('|')) rows.push(splitRow(lines[i++]));
      const cell = (tag, c, k) => `<${tag}${aligns[k] ? ` style="text-align:${aligns[k]}"` : ''}>${inline(c, ctx)}</${tag}>`;
      let h = '<div class="table-wrap"><table class="wikitable"><thead><tr>' + head.map((c, k) => cell('th', c, k)).join('') + '</tr></thead><tbody>';
      for (const r of rows) h += '<tr>' + head.map((_, k) => cell('td', r[k] || '', k)).join('') + '</tr>';
      out.push(h + '</tbody></table></div>');
      continue;
    }

    // list
    if (isListLine(line)) {
      const items = [];
      while (i < lines.length && lines[i].trim()) {
        const lm = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/.exec(lines[i]);
        if (lm) { items.push({ indent: lm[1].replace(/\t/g, '    ').length, ordered: /\d/.test(lm[2]), text: lm[3] }); i++; }
        else if (/^\s+\S/.test(lines[i]) && items.length) { items[items.length - 1].text += ' ' + lines[i].trim(); i++; }
        else break;
      }
      out.push(listHtml(items, ctx));
      continue;
    }

    // paragraph
    const para = [line];
    i++;
    while (i < lines.length && lines[i].trim() && !startsSpecial(lines[i]) && !(lines[i].includes('|') && isTableSep(lines[i + 1]))) {
      para.push(lines[i++]);
    }
    const text = para.map((l, k) => (k < para.length - 1 && / {2,}$/.test(l) ? l.replace(/ +$/, '') + '\u0001' : l.trim())).join(' ').replace(/\u0001 /g, '\u0001');
    out.push(`<p>${inline(text, ctx)}</p>`);
  }
  return out.join('\n');
}

function listHtml(items, ctx) {
  let html = '';
  const stack = [];
  for (const it of items) {
    while (stack.length && it.indent < stack[stack.length - 1].indent) html += `</li></${stack.pop().tag}>`;
    const top = stack[stack.length - 1];
    if (!top || it.indent > top.indent) {
      const tag = it.ordered ? 'ol' : 'ul';
      stack.push({ indent: it.indent, tag });
      html += `<${tag}><li>`;
    } else html += '</li><li>';
    html += inline(it.text, ctx);
  }
  while (stack.length) html += `</li></${stack.pop().tag}>`;
  return html;
}

// `interactive=false` (or 0 / off / no) switches a widget to its plain, non-interactive form.
const isOff = (v) => /^(false|0|off|no)$/i.test(String(v === undefined ? '' : v).trim());

/* ---------------------------------------------------------------- timelines
   One event per line:  date | title | description
   Dates may be years (-44, 44 BC, 1066 AD), YYYY-MM-DD, or any text. When every date is
   numeric the events are sorted and spaced in proportion to their dates. */

function parseWhen(str) {
  const t = String(str).trim().replace(/^(?:c\.|ca\.|circa)\s*/i, '');
  const m = /^(-?\d{1,6})(?:-(\d{1,2})(?:-(\d{1,2}))?)?\s*(BCE|BC|CE|AD)?$/i.exec(t);
  if (!m) return null;
  let year = parseInt(m[1], 10);
  if (m[4] && /^b/i.test(m[4])) return -Math.abs(year);
  if (m[2] && year >= 0) year += (parseInt(m[2], 10) - 1) / 12 + (m[3] ? (parseInt(m[3], 10) - 1) / 372 : 0);
  return year;
}

function parseTimeline(text) {
  const events = [];
  for (const raw of String(text).split('\n')) {
    const line = raw.trim();
    if (!line || line.startsWith('#')) continue;
    const parts = line.split('|').map((x) => x.trim());
    events.push({ date: parts[0], title: parts[1] || '', desc: parts.slice(2).join(' | '), when: parseWhen(parts[0]) });
  }
  const numeric = events.length > 0 && events.every((e) => e.when !== null);
  if (numeric) events.sort((a, b) => a.when - b.when);
  return { events, numeric };
}

function timelineStaticHtml(events, ctx) {
  return '<ol class="tl-static">' + events.map((e) =>
    `<li><span class="tl-date">${esc(e.date)}</span><div class="tl-body"><strong>${inline(e.title, ctx)}</strong>` +
    (e.desc ? `<p>${inline(e.desc, ctx)}</p>` : '') + '</div></li>').join('') + '</ol>';
}

function renderFence(lang, attrs, body, ctx) {
  const text = esc(body);
  if (LANGS[lang]) {
    const [label, key] = LANGS[lang];
    return `<div class="cw" data-lang="${key}" data-label="${label}" data-canvas="${attrs.canvas ? 1 : 0}"><textarea class="cw-src" hidden>${text}</textarea></div>`;
  }
  switch (lang) {
    case 'sheet':
      return `<div class="sheet" data-height="${esc(attrs.height || '')}"><textarea class="sheet-src" hidden>${text}</textarea></div>`;
    case 'map':
      return `<div class="map-widget" data-height="${esc(attrs.height || '')}" data-interactive="${isOff(attrs.interactive) ? 0 : 1}"><textarea class="map-src" hidden>${text}</textarea></div>`;
    case 'timeline': {
      const tl = parseTimeline(body);
      if (!tl.events.length) return '<div class="widget-note">This timeline has no events.</div>';
      if (isOff(attrs.interactive)) return `<div class="timeline-widget tl-plain">${timelineStaticHtml(tl.events, ctx)}</div>`;
      return `<div class="timeline-widget" data-timeline="${esc(JSON.stringify(tl))}"></div>`;
    }
    case 'csvsearch':
      return `<div class="csv-widget" data-attrs="${esc(JSON.stringify(attrs))}"><textarea class="csv-src" hidden>${text}</textarea></div>`;
    case 'desmos':
      return `<div class="desmos-widget" data-height="${esc(attrs.height || '')}"><textarea class="desmos-src" hidden>${text}</textarea></div>`;
    case 'model':
      return `<div class="model-widget" data-attrs="${esc(JSON.stringify(attrs))}"></div>`;
    case 'math':
    case 'latex':
      return `<div class="math-block" data-display="1" data-tex="${text}"></div>`;
    default:
      return `<pre class="plain-code"><code>${text}</code></pre>`;
  }
}

function renderDirective(name, attrs, body, ctx) {
  switch (name) {
    case 'text': {
      const css = styleFromAttrs(attrs);
      return `<div class="styled-text"${css ? ` style="${esc(css)}"` : ''}>${renderBlocks(body, ctx)}</div>`;
    }
    case 'figure': {
      const align = /^(left|right|center|none)$/.test(attrs.align || '') ? attrs.align : 'right';
      const cls = { left: 'tleft', right: 'tright', center: 'tcenter', none: 'tnone' }[align];
      const w = +attrs.width > 0 ? `width:${+attrs.width}px;` : (align === 'center' || align === 'none' ? 'max-width:100%;' : 'width:300px;');
      const cap = attrs.caption ? `<figcaption>${inline(attrs.caption, ctx)}</figcaption>` : '';
      return `<figure class="thumb ${cls}" style="${w}">${mediaHtml(attrs.src || '', attrs.alt || attrs.caption || '')}${cap}</figure>`;
    }
    case 'audio': {
      if (!attrs.src) return '<div class="widget-note">Audio: missing src</div>';
      const title = attrs.title ? `<div class="au-title">${inline(attrs.title, ctx)}</div>` : '';
      return `<div class="audio-widget">${title}<audio controls preload="metadata" src="${esc(resolveMedia(attrs.src))}">` +
        `Your browser does not support audio playback. <a href="${esc(resolveMedia(attrs.src))}">Download the file</a>.</audio></div>`;
    }
    case 'infobox': {
      const { kv, rest } = parseKeyed(body.join('\n'));
      let title = '';
      let image = '';
      const rows = [];
      for (const [k, v] of kv) {
        if (k === 'title') title = v;
        else if (k === 'image') image = v;
        else rows.push([k, v]);
      }
      let h = '<aside class="infobox">';
      if (title) h += `<div class="ib-title">${inline(title, ctx)}</div>`;
      if (image) h += `<div class="ib-image">${mediaHtml(image, title)}</div>`;
      h += '<table>';
      for (const [k, v] of rows) h += `<tr><th>${esc(k.replace(/\b\w/g, (c) => c.toUpperCase()))}</th><td>${inline(v, ctx)}</td></tr>`;
      for (const r of rest) h += `<tr><td colspan="2">${inline(r, ctx)}</td></tr>`;
      return h + '</table></aside>';
    }
    case 'note':
    case 'tip':
    case 'warning':
      return `<div class="callout ${name}">${renderBlocks(body, ctx)}</div>`;
    default:
      return `<div class="callout">${renderBlocks(body, ctx)}</div>`;
  }
}

function renderMarkdown(md) {
  const ctx = { title: '', ids: new Set(), toc: [], fnNumber: new Map(), fnCount: new Map(), fnDefs: new Map() };
  const lines = md.replace(/\r\n?/g, '\n').split('\n');
  let html = renderBlocks(lines, ctx);

  // table of contents
  if (ctx.toc.length >= 3) {
    const counters = [0, 0, 0, 0, 0, 0, 0];
    let toc = '<div class="toc" id="toc"><div class="toc-title">Contents</div><ul>';
    let prev = 2;
    const stack = [];
    for (const h of ctx.toc) {
      const lvl = Math.max(2, h.level);
      if (lvl > prev) { while (stack.length < lvl - 2) { toc += '<ul>'; stack.push(1); } }
      while (lvl < prev && stack.length) { toc += '</ul>'; stack.pop(); prev--; }
      prev = lvl;
      counters[lvl]++;
      for (let k = lvl + 1; k < counters.length; k++) counters[k] = 0;
      const num = counters.slice(2, lvl + 1).join('.');
      toc += `<li><a class="wl-anchor" href="#${h.id}"><span class="tocnum">${num}</span>${esc(h.text.replace(/[*_`]/g, ''))}</a></li>`;
    }
    while (stack.length) { toc += '</ul>'; stack.pop(); }
    toc += '</ul></div>';
    const at = html.indexOf('<h2');
    if (at >= 0) html = html.slice(0, at) + toc + html.slice(at);
  }

  // footnotes
  if (ctx.fnNumber.size) {
    const order = [...ctx.fnNumber.entries()].sort((a, b) => a[1] - b[1]);
    let notes = '<h2 id="notes">Notes</h2><ol class="references">';
    for (const [id] of order) {
      const text = ctx.fnDefs.has(id) ? inline(ctx.fnDefs.get(id), ctx) : '<em>(missing footnote)</em>';
      const safe = esc(id);
      const backs = [];
      for (let k = 1; k <= ctx.fnCount.get(id); k++) backs.push(`<a href="#fnref-${safe}-${k}">${k === 1 ? '\u2191' : '\u2191' + k}</a>`);
      notes += `<li id="fn-${safe}"><span class="backlink">${backs.join(' ')}</span>${text}</li>`;
    }
    html += notes + '</ol>';
  }
  return { html, title: ctx.title };
}

/* ==========================================================================
   Canvas engine: rect / ellipse / line / text + a turtle.
   Every language can drive it. JavaScript and Python call it directly;
   C, C++, Rust and Java print command lines that start with "@" to stdout:
       @rect 10 10 80 40 #3366cc        (x y w h [color] [fill|stroke])
       @ellipse 100 60 40 25 red        (cx cy rx ry [color] [fill|stroke])
       @circle 100 60 30 #0a0           (cx cy r [color] [fill|stroke])
       @line 0 0 100 50 black           (x1 y1 x2 y2 [color])
       @text 20 30 Hello world          (x y text...)
       @forward 50   @back 50   @left 90   @right 90
       @penup   @pendown   @goto 0 0   @setheading 0   @home
       @color red   @pensize 3   @dot 5   @arc 40 [extent]
       @hideturtle   @showturtle   @clear [color]   @size 480 320
   Pixel commands use a top-left origin. Turtle commands use a centre origin with y up.
   Colors must not contain spaces (use #rrggbb, names, or rgb(1,2,3)).
   ========================================================================== */

class CanvasEngine {
  constructor(host, w, h) {
    this.screen = document.createElement('canvas');
    host.appendChild(this.screen);
    this.buf = document.createElement('canvas');
    this.ctx = this.buf.getContext('2d');
    this.pending = false;
    this.resize(w, h);
  }
  resize(w, h) {
    this.w = w; this.h = h;
    this.screen.width = this.buf.width = w;
    this.screen.height = this.buf.height = h;
    this.reset();
  }
  reset() {
    const c = this.ctx;
    c.setTransform(1, 0, 0, 1, 0, 0);
    c.fillStyle = '#ffffff';
    c.fillRect(0, 0, this.w, this.h);
    c.lineCap = 'round'; c.lineJoin = 'round';
    c.textBaseline = 'alphabetic';
    c.font = '14px sans-serif';
    this.t = { x: 0, y: 0, h: 0, pen: true, color: '#000000', size: 1, visible: true };
    this.present();
  }
  present() {
    if (this.pending) return;
    this.pending = true;
    requestAnimationFrame(() => {
      this.pending = false;
      const s = this.screen.getContext('2d');
      s.clearRect(0, 0, this.w, this.h);
      s.drawImage(this.buf, 0, 0);
      if (this.t.visible) this.drawTurtle(s);
    });
  }
  drawTurtle(s) {
    const t = this.t, r = (t.h * Math.PI) / 180;
    s.save();
    s.translate(this.w / 2 + t.x, this.h / 2 - t.y);
    s.rotate(-r);
    s.beginPath(); s.moveTo(11, 0); s.lineTo(-7, 6); s.lineTo(-7, -6); s.closePath();
    s.fillStyle = 'rgba(30,110,40,0.85)'; s.strokeStyle = '#124'; s.lineWidth = 1;
    s.fill(); s.stroke();
    s.restore();
  }
  px(x) { return this.w / 2 + x; }
  py(y) { return this.h / 2 - y; }

  // --- turtle
  moveTo(nx, ny) {
    const t = this.t, c = this.ctx;
    if (t.pen) {
      c.beginPath(); c.moveTo(this.px(t.x), this.py(t.y)); c.lineTo(this.px(nx), this.py(ny));
      c.strokeStyle = t.color; c.lineWidth = t.size; c.stroke();
    }
    t.x = nx; t.y = ny; this.present();
  }
  forward(d) { const a = (this.t.h * Math.PI) / 180; this.moveTo(this.t.x + d * Math.cos(a), this.t.y + d * Math.sin(a)); }
  back(d) { this.forward(-d); }
  left(a) { this.t.h = (this.t.h + a) % 360; this.present(); }
  right(a) { this.left(-a); }
  penup() { this.t.pen = false; }
  pendown() { this.t.pen = true; }
  goto(x, y) { this.moveTo(x, y); }
  setheading(a) { this.t.h = a % 360; this.present(); }
  home() { this.moveTo(0, 0); this.t.h = 0; }
  color(c) { if (c) this.t.color = c; }
  pensize(w) { this.t.size = Math.max(0.1, w); }
  dot(r) {
    const c = this.ctx; c.beginPath(); c.arc(this.px(this.t.x), this.py(this.t.y), Math.max(0.5, r), 0, Math.PI * 2);
    c.fillStyle = this.t.color; c.fill(); this.present();
  }
  arc(r, extent = 360) {
    const steps = Math.max(8, Math.ceil(Math.abs(extent) / 6));
    const stepAngle = extent / steps * (r < 0 ? -1 : 1);
    const stepLen = (2 * Math.PI * Math.abs(r) * Math.abs(extent / 360)) / steps;
    for (let i = 0; i < steps; i++) { this.left(stepAngle / 2); this.forward(stepLen); this.left(stepAngle / 2); }
  }
  hideturtle() { this.t.visible = false; this.present(); }
  showturtle() { this.t.visible = true; this.present(); }

  // --- shapes (pixel coordinates, top-left origin)
  style(color, fill) {
    const col = color || this.t.color;
    this.ctx.fillStyle = col; this.ctx.strokeStyle = col; this.ctx.lineWidth = this.t.size;
    return fill !== false;
  }
  rect(x, y, w, h, color, fill = true) {
    const f = this.style(color, fill);
    f ? this.ctx.fillRect(x, y, w, h) : this.ctx.strokeRect(x, y, w, h);
    this.present();
  }
  ellipse(cx, cy, rx, ry, color, fill = true) {
    const f = this.style(color, fill);
    this.ctx.beginPath(); this.ctx.ellipse(cx, cy, Math.abs(rx), Math.abs(ry), 0, 0, Math.PI * 2);
    f ? this.ctx.fill() : this.ctx.stroke(); this.present();
  }
  disc(cx, cy, r, color, fill = true) { this.ellipse(cx, cy, r, r, color, fill); }
  line(x1, y1, x2, y2, color) {
    this.style(color, false);
    this.ctx.beginPath(); this.ctx.moveTo(x1, y1); this.ctx.lineTo(x2, y2); this.ctx.stroke(); this.present();
  }
  text(x, y, str, color) { this.style(color, true); this.ctx.fillText(String(str), x, y); this.present(); }
  pixel(x, y, color) { this.style(color, true); this.ctx.fillRect(x, y, 1, 1); this.present(); }
  clear(color) { this.ctx.fillStyle = color || '#ffffff'; this.ctx.fillRect(0, 0, this.w, this.h); this.present(); }

  // --- text protocol
  exec(line) {
    line = String(line).trim();
    if (!line) return;
    if (line[0] === '@') line = line.slice(1);
    const tok = line.split(/\s+/);
    const cmd = tok.shift().toLowerCase();
    const n = tok.map(Number);
    const num = (k, d = 0) => (Number.isFinite(n[k]) ? n[k] : d);
    const opts = (from) => {
      let color; let fill = true;
      for (const t of tok.slice(from)) {
        if (/^(0|false|stroke|outline)$/i.test(t)) fill = false;
        else if (/^(1|true|fill)$/i.test(t)) fill = true;
        else color = t;
      }
      return [color, fill];
    };
    switch (cmd) {
      case 'size': this.resize(Math.min(2000, Math.max(16, num(0, 480))), Math.min(2000, Math.max(16, num(1, 320)))); break;
      case 'clear': case 'background': this.clear(tok[0]); break;
      case 'rect': this.rect(num(0), num(1), num(2), num(3), ...opts(4)); break;
      case 'ellipse': this.ellipse(num(0), num(1), num(2), num(3), ...opts(4)); break;
      case 'circle': this.disc(num(0), num(1), num(2), ...opts(3)); break;
      case 'line': this.line(num(0), num(1), num(2), num(3), tok[4]); break;
      case 'text': this.text(num(0), num(1), tok.slice(2).join(' ')); break;
      case 'pixel': this.pixel(num(0), num(1), tok[2]); break;
      case 'forward': case 'fd': this.forward(num(0)); break;
      case 'back': case 'backward': case 'bk': this.back(num(0)); break;
      case 'left': case 'lt': this.left(num(0)); break;
      case 'right': case 'rt': this.right(num(0)); break;
      case 'penup': case 'pu': this.penup(); break;
      case 'pendown': case 'pd': this.pendown(); break;
      case 'goto': case 'setpos': this.goto(num(0), num(1)); break;
      case 'setheading': case 'seth': this.setheading(num(0)); break;
      case 'home': this.home(); break;
      case 'color': this.color(tok[0]); break;
      case 'pensize': case 'width': this.pensize(num(0, 1)); break;
      case 'dot': this.dot(num(0, 5)); break;
      case 'arc': this.arc(num(0, 30), num(1, 360)); break;
      case 'hideturtle': case 'ht': this.hideturtle(); break;
      case 'showturtle': case 'st': this.showturtle(); break;
      default: throw new Error('Unknown canvas command: ' + cmd);
    }
  }

  // --- API handed to JavaScript code
  jsApi() {
    const e = this;
    const api = {
      canvas: e.buf, ctx: e.ctx, width: e.w, height: e.h,
      forward: (d) => e.forward(d), back: (d) => e.back(d), left: (a) => e.left(a), right: (a) => e.right(a),
      penup: () => e.penup(), pendown: () => e.pendown(), goto: (x, y) => e.goto(x, y),
      setheading: (a) => e.setheading(a), home: () => e.home(), color: (c) => e.color(c),
      pensize: (w) => e.pensize(w), dot: (r) => e.dot(r === undefined ? 5 : r), arc: (r, x) => e.arc(r, x),
      hideturtle: () => e.hideturtle(), showturtle: () => e.showturtle(),
      circle: (a, b, c, d, f) => (b === undefined ? e.arc(a) : e.disc(a, b, c, d, f)),
      rect: (...a) => e.rect(...a), ellipse: (...a) => e.ellipse(...a), line: (...a) => e.line(...a),
      text: (...a) => e.text(...a), pixel: (...a) => e.pixel(...a), clear: (c) => e.clear(c),
      background: (c) => e.clear(c), size: (w, h) => { e.resize(w, h); api.width = w; api.height = h; },
      sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
    };
    api.fd = api.forward; api.bk = api.back; api.backward = api.back; api.lt = api.left; api.rt = api.right;
    api.pu = api.penup; api.pd = api.pendown; api.setpos = api.goto; api.update = () => e.present();
    api.turtle = {};
    for (const k of ['forward', 'fd', 'back', 'bk', 'backward', 'left', 'lt', 'right', 'rt', 'penup', 'pu', 'pendown', 'pd',
      'goto', 'setpos', 'setheading', 'home', 'color', 'pensize', 'dot', 'circle', 'hideturtle', 'showturtle', 'clear']) {
      api.turtle[k] = api[k];
    }
    return api;
  }
}

/* ==========================================================================
   Spreadsheet engine (formulas, ranges, a few dozen functions)
   ========================================================================== */

class SheetErr { constructor(code) { this.code = code; } }

const colName = (n) => { let s = ''; n++; while (n > 0) { const m = (n - 1) % 26; s = String.fromCharCode(65 + m) + s; n = Math.floor((n - 1) / 26); } return s; };
const colIndex = (s) => { let n = 0; for (const ch of s.toUpperCase()) n = n * 26 + ch.charCodeAt(0) - 64; return n - 1; };

function parseCSV(text) {
  const rows = []; let row = []; let cur = ''; let q = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (q) {
      if (ch === '"') { if (text[i + 1] === '"') { cur += '"'; i++; } else q = false; } else cur += ch;
    } else if (ch === '"' && cur === '') q = true;
    else if (ch === ',') { row.push(cur); cur = ''; }
    else if (ch === '\n' || ch === '\r') { if (ch === '\r' && text[i + 1] === '\n') i++; row.push(cur); rows.push(row); row = []; cur = ''; }
    else cur += ch;
  }
  if (cur !== '' || row.length) { row.push(cur); rows.push(row); }
  while (rows.length && rows[rows.length - 1].every((c) => c === '')) rows.pop();
  return rows;
}
const toCSV = (rows) => rows.map((r) => r.map((c) => (/[",\n]/.test(c) ? '"' + c.replace(/"/g, '""') + '"' : c)).join(',')).join('\n');

function tokenize(s) {
  const t = []; let i = 0; let m;
  while (i < s.length) {
    const ch = s[i];
    if (/\s/.test(ch)) { i++; continue; }
    const rest = s.slice(i);
    if ((m = /^(\d+\.?\d*(?:[eE][+-]?\d+)?|\.\d+)/.exec(rest))) { t.push({ t: 'num', v: parseFloat(m[1]) }); i += m[0].length; continue; }
    if (ch === '"') {
      let j = i + 1; let str = '';
      while (j < s.length) { if (s[j] === '"') { if (s[j + 1] === '"') { str += '"'; j += 2; continue; } break; } str += s[j++]; }
      t.push({ t: 'str', v: str }); i = j + 1; continue;
    }
    if ((m = /^(TRUE|FALSE)(?![A-Za-z0-9_])/i.exec(rest))) { t.push({ t: 'bool', v: m[1].toUpperCase() === 'TRUE' }); i += m[0].length; continue; }
    if ((m = /^([A-Za-z_][A-Za-z0-9_.]*)\s*\(/.exec(rest))) { t.push({ t: 'fn', v: m[1].toUpperCase() }); i += m[0].length; continue; }
    if ((m = /^\$?([A-Za-z]{1,3})\$?(\d+)(?::\$?([A-Za-z]{1,3})\$?(\d+))?(?![A-Za-z0-9_])/.exec(rest))) {
      const a = { c: colIndex(m[1]), r: +m[2] - 1 };
      t.push(m[3] ? { t: 'range', a, b: { c: colIndex(m[3]), r: +m[4] - 1 } } : { t: 'ref', a });
      i += m[0].length; continue;
    }
    if ((m = /^(<=|>=|<>|[-+*\/^&=<>(),;%])/.exec(rest))) { t.push({ t: 'op', v: m[1] === ';' ? ',' : m[1] }); i += m[0].length; continue; }
    throw new SheetErr('#NAME?');
  }
  return t;
}

class Sheet {
  constructor(raw) { this.raw = raw; this.cache = new Map(); this.stack = new Set(); }
  invalidate() { this.cache.clear(); }
  value(r, c) {
    const key = r + ',' + c;
    if (this.cache.has(key)) return this.cache.get(key);
    if (this.stack.has(key)) throw new SheetErr('#CYCLE!');
    this.stack.add(key);
    let v;
    try {
      const raw = (this.raw[r] && this.raw[r][c]) || '';
      if (raw[0] === '=') v = this.evalFormula(raw.slice(1));
      else if (raw.trim() !== '' && isFinite(raw)) v = parseFloat(raw);
      else v = raw;
    } catch (e) { v = e instanceof SheetErr ? e : new SheetErr('#ERROR!'); }
    finally { this.stack.delete(key); }
    this.cache.set(key, v);
    return v;
  }
  display(r, c) {
    const v = this.value(r, c);
    if (v instanceof SheetErr) return v.code;
    if (typeof v === 'number') return Number.isInteger(v) ? String(v) : String(parseFloat(v.toPrecision(10)));
    if (typeof v === 'boolean') return v ? 'TRUE' : 'FALSE';
    return String(v);
  }
  cell(r, c) {
    const v = this.value(r, c);
    if (v instanceof SheetErr) throw v;
    return v;
  }
  evalFormula(src) {
    const toks = tokenize(src); let p = 0;
    const self = this;
    const peek = () => toks[p];
    const isOp = (v) => toks[p] && toks[p].t === 'op' && toks[p].v === v;
    const num = (v) => {
      if (typeof v === 'number') return v;
      if (typeof v === 'boolean') return v ? 1 : 0;
      if (v === '' || v === undefined) return 0;
      if (typeof v === 'string' && isFinite(v)) return parseFloat(v);
      throw new SheetErr('#VALUE!');
    };
    const flat = (args) => { const o = []; for (const a of args) Array.isArray(a) ? o.push(...a) : o.push(a); return o; };
    const nums = (args) => flat(args.map((a) => a)).filter((v) => typeof v === 'number' || typeof v === 'boolean' || (typeof v === 'string' && v !== '' && isFinite(v) && !Array.isArray(v))).map(num);
    const scalar = (v) => { if (Array.isArray(v)) return v.length ? v[0] : ''; return v; };
    const FN = {
      SUM: (a) => nums(a).reduce((x, y) => x + y, 0),
      PRODUCT: (a) => nums(a).reduce((x, y) => x * y, 1),
      AVERAGE: (a) => { const n = nums(a); if (!n.length) throw new SheetErr('#DIV/0!'); return n.reduce((x, y) => x + y, 0) / n.length; },
      MIN: (a) => { const n = nums(a); return n.length ? Math.min(...n) : 0; },
      MAX: (a) => { const n = nums(a); return n.length ? Math.max(...n) : 0; },
      COUNT: (a) => flat(a).filter((v) => typeof v === 'number').length,
      COUNTA: (a) => flat(a).filter((v) => v !== '').length,
      MEDIAN: (a) => { const n = nums(a).sort((x, y) => x - y); if (!n.length) throw new SheetErr('#NUM!'); const h = n.length >> 1; return n.length % 2 ? n[h] : (n[h - 1] + n[h]) / 2; },
      ABS: (a) => Math.abs(num(scalar(a[0]))),
      SQRT: (a) => { const v = num(scalar(a[0])); if (v < 0) throw new SheetErr('#NUM!'); return Math.sqrt(v); },
      POWER: (a) => Math.pow(num(scalar(a[0])), num(scalar(a[1]))),
      MOD: (a) => { const d = num(scalar(a[1])); if (!d) throw new SheetErr('#DIV/0!'); const n = num(scalar(a[0])); return n - d * Math.floor(n / d); },
      ROUND: (a) => { const f = Math.pow(10, a[1] === undefined ? 0 : num(scalar(a[1]))); return Math.round(num(scalar(a[0])) * f) / f; },
      FLOOR: (a) => Math.floor(num(scalar(a[0]))),
      CEILING: (a) => Math.ceil(num(scalar(a[0]))),
      INT: (a) => Math.floor(num(scalar(a[0]))),
      EXP: (a) => Math.exp(num(scalar(a[0]))),
      LN: (a) => Math.log(num(scalar(a[0]))),
      LOG10: (a) => Math.log10(num(scalar(a[0]))),
      SIN: (a) => Math.sin(num(scalar(a[0]))), COS: (a) => Math.cos(num(scalar(a[0]))), TAN: (a) => Math.tan(num(scalar(a[0]))),
      PI: () => Math.PI,
      IF: (a) => (scalar(a[0]) && scalar(a[0]) !== '0' ? (a[1] === undefined ? true : scalar(a[1])) : (a[2] === undefined ? false : scalar(a[2]))),
      AND: (a) => flat(a).every((v) => !!v), OR: (a) => flat(a).some((v) => !!v), NOT: (a) => !scalar(a[0]),
      CONCAT: (a) => flat(a).join(''), LEN: (a) => String(scalar(a[0])).length,
      UPPER: (a) => String(scalar(a[0])).toUpperCase(), LOWER: (a) => String(scalar(a[0])).toLowerCase(),
    };

    function primary() {
      const tk = toks[p++];
      if (!tk) throw new SheetErr('#ERROR!');
      switch (tk.t) {
        case 'num': case 'str': case 'bool': return tk.v;
        case 'ref': return self.cell(tk.a.r, tk.a.c);
        case 'range': {
          const out = [];
          for (let r = Math.min(tk.a.r, tk.b.r); r <= Math.max(tk.a.r, tk.b.r); r++)
            for (let c = Math.min(tk.a.c, tk.b.c); c <= Math.max(tk.a.c, tk.b.c); c++) out.push(self.cell(r, c));
          return out;
        }
        case 'fn': {
          const args = [];
          if (isOp(')')) p++;
          else {
            for (;;) { args.push(cmp()); if (isOp(',')) { p++; continue; } if (isOp(')')) { p++; break; } throw new SheetErr('#ERROR!'); }
          }
          if (!FN[tk.v]) throw new SheetErr('#NAME?');
          return FN[tk.v](args);
        }
        case 'op':
          if (tk.v === '(') { const v = cmp(); if (!isOp(')')) throw new SheetErr('#ERROR!'); p++; return v; }
          throw new SheetErr('#ERROR!');
        default: throw new SheetErr('#ERROR!');
      }
    }
    function postfix() { let v = primary(); while (isOp('%')) { p++; v = num(scalar(v)) / 100; } return v; }
    function unary() { if (isOp('-')) { p++; return -num(scalar(unary())); } if (isOp('+')) { p++; return num(scalar(unary())); } return postfix(); }
    function pow() { let v = unary(); while (isOp('^')) { p++; v = Math.pow(num(scalar(v)), num(scalar(unary()))); } return v; }
    function mul() {
      let v = pow();
      while (isOp('*') || isOp('/')) {
        const op = toks[p++].v; const r = num(scalar(pow())); const l = num(scalar(v));
        if (op === '/' && r === 0) throw new SheetErr('#DIV/0!');
        v = op === '*' ? l * r : l / r;
      }
      return v;
    }
    function add() { let v = mul(); while (isOp('+') || isOp('-')) { const op = toks[p++].v; const r = num(scalar(mul())); v = op === '+' ? num(scalar(v)) + r : num(scalar(v)) - r; } return v; }
    function cat() { let v = add(); while (isOp('&')) { p++; v = String(scalar(v)) + String(scalar(add())); } return v; }
    function cmp() {
      let v = cat();
      while (peek() && peek().t === 'op' && ['=', '<>', '<', '>', '<=', '>='].includes(peek().v)) {
        const op = toks[p++].v; const l = scalar(v); const r = scalar(cat());
        const a = typeof l === 'string' && typeof r === 'string' ? l.toLowerCase() : l;
        const b = typeof l === 'string' && typeof r === 'string' ? r.toLowerCase() : r;
        v = op === '=' ? a === b : op === '<>' ? a !== b : op === '<' ? a < b : op === '>' ? a > b : op === '<=' ? a <= b : a >= b;
      }
      return v;
    }
    const result = cmp();
    if (p < toks.length) throw new SheetErr('#ERROR!');
    return scalar(result);
  }
}

function initSheet(el) {
  const original = parseCSV(el.querySelector('textarea.sheet-src').value);
  let raw = original.map((r) => r.slice());
  const width = () => Math.max(1, ...raw.map((r) => r.length));
  const normalize = () => { const w = width(); raw.forEach((r) => { while (r.length < w) r.push(''); }); if (!raw.length) raw.push(['']); };
  normalize();
  const sheet = new Sheet(raw);
  if (el.dataset.height) el.querySelector('textarea').insertAdjacentHTML('afterend', '');

  el.innerHTML = '<div class="sheet-scroll"><table class="sheet-table"></table></div>' +
    '<div class="sheet-bar"><span class="widget-note">Interactive sheet. Cells starting with = are formulas (SUM, AVERAGE, IF, ROUND, ...).</span>' +
    '<button type="button" data-a="row">Add row</button><button type="button" data-a="col">Add column</button>' +
    '<button type="button" data-a="reset">Reset</button><button type="button" data-a="csv">Download CSV</button></div>';
  if (el.dataset.height) el.querySelector('.sheet-scroll').style.maxHeight = +el.dataset.height + 'px';
  const table = el.querySelector('table');
  let inputs = [];

  const paint = (inp) => {
    const r = +inp.dataset.r, c = +inp.dataset.c;
    const v = sheet.value(r, c);
    inp.classList.toggle('err', v instanceof SheetErr);
    inp.classList.toggle('num', typeof v === 'number');
    if (document.activeElement !== inp) inp.value = sheet.display(r, c);
  };
  const repaint = () => { sheet.invalidate(); inputs.forEach(paint); };

  const build = () => {
    normalize();
    sheet.raw = raw; sheet.invalidate();
    const w = width();
    let h = '<thead><tr><th></th>';
    for (let c = 0; c < w; c++) h += `<th>${colName(c)}</th>`;
    h += '</tr></thead><tbody>';
    raw.forEach((row, r) => {
      h += `<tr><th>${r + 1}</th>`;
      for (let c = 0; c < w; c++) h += `<td><input data-r="${r}" data-c="${c}" spellcheck="false" autocomplete="off"></td>`;
      h += '</tr>';
    });
    table.innerHTML = h + '</tbody>';
    inputs = [...table.querySelectorAll('input')];
    inputs.forEach(paint);
  };
  build();

  table.addEventListener('focusin', (e) => { const i = e.target; if (i.matches('input')) { i.value = raw[i.dataset.r][i.dataset.c]; i.classList.remove('num'); } });
  table.addEventListener('input', (e) => { const i = e.target; if (i.matches('input')) { raw[i.dataset.r][i.dataset.c] = i.value; sheet.invalidate(); inputs.forEach((x) => { if (x !== i) paint(x); }); } });
  table.addEventListener('focusout', (e) => { const i = e.target; if (i.matches('input')) { repaint(); } });
  table.addEventListener('keydown', (e) => {
    const i = e.target; if (!i.matches('input')) return;
    let r = +i.dataset.r, c = +i.dataset.c;
    if (e.key === 'Enter' || e.key === 'ArrowDown') r++; else if (e.key === 'ArrowUp') r--; else return;
    const next = inputs.find((x) => +x.dataset.r === r && +x.dataset.c === c);
    if (next) { e.preventDefault(); next.focus(); next.select(); }
  });
  el.querySelector('.sheet-bar').addEventListener('click', (e) => {
    const a = e.target.dataset && e.target.dataset.a; if (!a) return;
    if (a === 'row') { raw.push(new Array(width()).fill('')); build(); }
    else if (a === 'col') { raw.forEach((r) => r.push('')); build(); }
    else if (a === 'reset') { raw = original.map((r) => r.slice()); build(); }
    else if (a === 'csv') {
      const blob = new Blob([toCSV(raw)], { type: 'text/csv' });
      const link = document.createElement('a'); link.href = URL.createObjectURL(blob); link.download = 'sheet.csv'; link.click();
      setTimeout(() => URL.revokeObjectURL(link.href), 2000);
    }
  });
}

/* ==========================================================================
   Code windows
   ========================================================================== */

const PY_PRELUDE = `
import sys, types
from wikicanvas import _cmd

def _col(c):
    if c is None: return ''
    if isinstance(c, (tuple, list)): return 'rgb(%s)' % ','.join(str(int(v)) for v in c)
    return str(c).replace(' ', '')

def _c(*a):
    _cmd(' '.join(str(x) for x in a if x != ''))

def forward(d): _c('forward', d)
def backward(d): _c('back', d)
def left(a): _c('left', a)
def right(a): _c('right', a)
def penup(): _c('penup')
def pendown(): _c('pendown')
def goto(x, y=None):
    if y is None: x, y = x
    _c('goto', x, y)
def setheading(a): _c('setheading', a)
def home(): _c('home')
def color(c): _c('color', _col(c))
def pensize(w): _c('pensize', w)
def dot(r=5): _c('dot', r)
def hideturtle(): _c('hideturtle')
def showturtle(): _c('showturtle')
def clear(c=None): _c('clear', _col(c))
def background(c): _c('clear', _col(c))
def size(w, h): _c('size', w, h)
def rect(x, y, w, h, color=None, fill=True): _c('rect', x, y, w, h, _col(color), 1 if fill else 0)
def ellipse(cx, cy, rx, ry, color=None, fill=True): _c('ellipse', cx, cy, rx, ry, _col(color), 1 if fill else 0)
def line(x1, y1, x2, y2, color=None): _c('line', x1, y1, x2, y2, _col(color))
def text(x, y, s, color=None): _c('text', x, y, s)
def pixel(x, y, color=None): _c('pixel', x, y, _col(color))
def circle(*a, color=None, fill=True):
    if len(a) == 1: _c('arc', a[0])
    else: _c('circle', a[0], a[1], a[2], _col(color), 1 if fill else 0)

fd, bk, back, lt, rt, pu, pd, setpos = forward, backward, backward, left, right, penup, pendown, goto
ht, st, width = hideturtle, showturtle, pensize

_names = ['forward','backward','left','right','penup','pendown','goto','setheading','home','color','pensize','dot',
          'hideturtle','showturtle','clear','background','size','rect','ellipse','line','text','pixel','circle',
          'fd','bk','back','lt','rt','pu','pd','setpos','ht','st','width']
_t = types.ModuleType('turtle')
for _n in _names:
    setattr(_t, _n, globals()[_n])
sys.modules['turtle'] = _t
`;

let pyodidePromise = null;
function getPyodide() {
  if (!pyodidePromise) {
    pyodidePromise = (async () => { await loadScript(CONFIG.libs.pyodide); return loadPyodide(); })();
    pyodidePromise.catch(() => { pyodidePromise = null; });
  }
  return pyodidePromise;
}

const fmtArg = (v) => (typeof v === 'string' ? v : (() => { try { return typeof v === 'object' ? JSON.stringify(v) : String(v); } catch (e) { return String(v); } })());

async function runJavaScript(code, ui) {
  const api = ui.engine ? ui.engine.jsApi() : {};
  const cons = {
    log: (...a) => ui.out(a.map(fmtArg).join(' ')), info: (...a) => ui.out(a.map(fmtArg).join(' ')),
    warn: (...a) => ui.out(a.map(fmtArg).join(' '), 'err'), error: (...a) => ui.out(a.map(fmtArg).join(' '), 'err'),
  };
  const scope = Object.assign({ console: cons, print: cons.log }, api);
  const names = Object.keys(scope);
  const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
  await new AsyncFunction(...names, code)(...names.map((k) => scope[k]));
}

async function runPython(code, ui) {
  const first = !pyodidePromise;
  if (first) ui.out('Loading Python runtime (first run only)...', 'dim');
  const py = await getPyodide();
  if (first) ui.clear();
  py.setStdout({ batched: (s) => ui.out(s) });
  py.setStderr({ batched: (s) => ui.out(s, 'err') });
  const g = py.globals.get('dict')();
  try {
    if (ui.engine) {
      py.registerJsModule('wikicanvas', { _cmd: (s) => ui.engine.exec(String(s)) });
      py.runPython(PY_PRELUDE, { globals: g });
    }
    await py.runPythonAsync(code, { globals: g });
  } catch (e) {
    const lines = String(e.message || e).split('\n');
    const at = lines.findIndex((l) => l.includes('File "<exec>"'));
    throw new Error(at >= 0 ? 'Traceback (most recent call last):\n' + lines.slice(at).join('\n').trim() : lines.join('\n').trim());
  } finally { g.destroy(); }
}

async function runRust(code, ui) {
  ui.out('Compiling and running on play.rust-lang.org ...', 'dim');
  const r = await fetch(CONFIG.rustPlayground, {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ channel: 'stable', mode: 'debug', edition: '2021', crateType: 'bin', tests: false, code, backtrace: false }),
  });
  if (!r.ok) throw new Error('Rust Playground returned HTTP ' + r.status);
  const j = await r.json();
  ui.clear();
  if (j.stdout) ui.out(j.stdout.replace(/\n$/, ''));
  if (j.stderr) ui.out(j.stderr.replace(/\n$/, ''), j.success ? 'dim' : 'err');
}

const godboltIds = {};
function godboltCompiler(lang) {
  if (!godboltIds[lang]) {
    godboltIds[lang] = (async () => {
      const r = await fetch(`${CONFIG.godbolt}/compilers/${encodeURIComponent(lang)}?fields=id,name,semver`, { headers: { Accept: 'application/json' } });
      if (!r.ok) throw new Error('Compiler Explorer returned HTTP ' + r.status);
      const list = await r.json();
      const pat = lang === 'java' ? /^(jdk|java|openjdk)\s*[\d.]+$/i : /^x86-64 gcc [\d.]+$/i;
      const ver = (c) => ((/([\d.]+)$/.exec(c.name) || [0, '0'])[1]).split('.').map(Number);
      const cmpv = (a, b) => { for (let i = 0; i < 4; i++) { const d = (a[i] || 0) - (b[i] || 0); if (d) return d; } return 0; };
      const cands = list.filter((c) => pat.test(c.name)).sort((a, b) => cmpv(ver(b), ver(a)));
      const pick = cands[0] || list[0];
      if (!pick) throw new Error('No compiler found for ' + lang);
      return pick.id;
    })();
    godboltIds[lang].catch(() => { delete godboltIds[lang]; });
  }
  return godboltIds[lang];
}

async function runGodbolt(key, code, ui) {
  const lang = key === 'cpp' ? 'c++' : key;
  const nice = { c: 'C', 'c++': 'C++', java: 'Java' }[lang];
  ui.out(`Compiling and running ${nice} on godbolt.org ...`, 'dim');
  const id = await godboltCompiler(lang);
  // Compiler Explorer does not name the file after the class, so a public top-level
  // class (public class Main) is rejected. Drop the modifier; line numbers are unchanged.
  if (lang === 'java') {
    code = code.replace(/^([ \t]*)public([ \t]+(?:(?:final|abstract|strictfp)[ \t]+)*(?:class|interface|enum|record)\b)/gm, '$1$2');
  }
  const args = lang === 'c' ? '-O1 -lm' : lang === 'c++' ? '-std=c++20 -O1' : '';
  const r = await fetch(`${CONFIG.godbolt}/compiler/${encodeURIComponent(id)}/compile`, {
    method: 'POST', headers: { 'Content-Type': 'application/json', Accept: 'application/json' },
    body: JSON.stringify({
      source: code, compiler: id, lang,
      options: {
        userArguments: args, executeParameters: { args: [], stdin: '' },
        compilerOptions: { executorRequest: true }, filters: { execute: true }, tools: [], libraries: [],
      },
      allowStoreCodeDebug: false,
    }),
  });
  if (!r.ok) throw new Error('Compiler Explorer returned HTTP ' + r.status);
  const j = await r.json();
  const txt = (a) => (a || []).map((x) => x.text).join('\n');
  ui.clear();
  const build = j.buildResult;
  if (build && build.code !== 0) { ui.out(txt(build.stderr) || txt(build.stdout) || 'Build failed.', 'err'); return; }
  const ex = j.execResult || j;
  const so = txt(ex.stdout), se = txt(ex.stderr);
  if (so) ui.out(so);
  if (se) ui.out(se, 'err');
  if (typeof ex.code === 'number' && ex.code !== 0) ui.out('[exit code ' + ex.code + ']', 'dim');
  if (!so && !se && ex.code === 0) ui.out('(no output)', 'dim');
}

const CM_MODES = { python: 'python', c: 'text/x-csrc', cpp: 'text/x-c++src', java: 'text/x-java', rust: 'rust', javascript: 'javascript' };
const REMOTE = { c: true, cpp: true, java: true, rust: true };

function initCodeWidget(el) {
  const lang = el.dataset.lang, label = el.dataset.label, hasCanvas = el.dataset.canvas === '1';
  const original = el.querySelector('textarea.cw-src').value;
  el.innerHTML =
    `<div class="cw-bar"><span class="cw-lang">${esc(label)}</span>` +
    (hasCanvas ? '<span class="cw-tag">canvas</span>' : '') +
    `<span class="cw-tag">${REMOTE[lang] ? 'runs on a remote server' : 'runs in your browser'}</span><span class="cw-spacer"></span>` +
    '<button class="run" type="button">Run</button><button class="reset" type="button">Reset</button></div>' +
    '<div class="cw-editor"></div>' +
    '<div class="cw-out">' + (hasCanvas ? '<div class="cw-canvas-wrap"></div>' : '') + '<pre class="cw-console"></pre></div>';

  const editorHost = el.querySelector('.cw-editor');
  const consoleEl = el.querySelector('.cw-console');
  const runBtn = el.querySelector('button.run');
  let getCode = () => original, setCode = () => {};

  const fallback = () => {
    const ta = document.createElement('textarea');
    ta.className = 'cw-fallback'; ta.spellcheck = false; ta.value = original;
    ta.addEventListener('keydown', (e) => {
      if (e.key === 'Tab') { e.preventDefault(); const s = ta.selectionStart; ta.setRangeText('    ', s, ta.selectionEnd, 'end'); }
    });
    editorHost.appendChild(ta);
    getCode = () => ta.value; setCode = (v) => { ta.value = v; };
    ta.addEventListener('keydown', (e) => { if (e.ctrlKey && e.key === 'Enter') run(); });
  };
  loadCss(CONFIG.libs.cmCss);
  loadSeq(CONFIG.libs.cm).then(() => {
    const cm = CodeMirror(editorHost, {
      value: original, mode: CM_MODES[lang], lineNumbers: true, indentUnit: 4, viewportMargin: Infinity,
      extraKeys: { Tab: (c) => c.execCommand('insertSoftTab'), 'Ctrl-Enter': () => run(), 'Cmd-Enter': () => run() },
    });
    getCode = () => cm.getValue(); setCode = (v) => cm.setValue(v);
  }).catch(fallback);

  let engine = null;
  if (hasCanvas) engine = new CanvasEngine(el.querySelector('.cw-canvas-wrap'), CONFIG.canvasWidth, CONFIG.canvasHeight);

  const ui = {
    engine,
    clear() { consoleEl.textContent = ''; },
    out(text, cls) {
      for (const line of String(text).split('\n')) {
        if (engine && line.startsWith('@')) {
          try { engine.exec(line); } catch (e) { this.print(e.message, 'err'); }
        } else this.print(line, cls);
      }
    },
    print(line, cls) {
      const s = document.createElement('span');
      if (cls) s.className = cls;
      s.textContent = line + '\n';
      consoleEl.appendChild(s);
      consoleEl.scrollTop = consoleEl.scrollHeight;
    },
  };

  async function run() {
    if (runBtn.disabled) return;
    runBtn.disabled = true; ui.clear();
    if (engine) engine.reset();
    const code = getCode();
    try {
      if (lang === 'javascript') await runJavaScript(code, ui);
      else if (lang === 'python') await runPython(code, ui);
      else if (lang === 'rust') await runRust(code, ui);
      else await runGodbolt(lang, code, ui);
    } catch (e) {
      ui.print(String((e && e.message) || e), 'err');
      if (REMOTE[lang] && /fetch|network|Failed to/i.test(String(e && e.message))) ui.print('The remote compiler service could not be reached.', 'dim');
    } finally { runBtn.disabled = false; }
  }
  runBtn.addEventListener('click', run);
  el.querySelector('button.reset').addEventListener('click', () => { setCode(original); ui.clear(); if (engine) engine.reset(); });
}

/* ==========================================================================
   Maps, Desmos, 3D models, math
   ========================================================================== */

function initMap(el) {
  const text = el.querySelector('textarea.map-src').value;
  const height = +el.dataset.height || 360;
  const interactive = el.dataset.interactive !== '0';
  el.innerHTML = `<div class="map-canvas${interactive ? '' : ' map-static'}" style="height:${height}px"><div class="widget-loading">Loading map...</div></div>` +
    `<div class="widget-note">${interactive ? 'Interactive map. Click the map to enable scroll-zoom.' : 'Static map.'}</div>`;
  const host = el.querySelector('.map-canvas');
  loadCss(CONFIG.libs.leafletCss);
  loadScript(CONFIG.libs.leaflet).then(() => {
    host.innerHTML = '';
    // A non-interactive map is a fixed picture: no panning, zooming, keyboard or popups.
    // Labels are printed next to the shapes instead, because there is nothing to click.
    const map = L.map(host, interactive ? { scrollWheelZoom: false } : {
      dragging: false, touchZoom: false, doubleClickZoom: false, scrollWheelZoom: false,
      boxZoom: false, keyboard: false, zoomControl: false, tap: false,
    });
    if (interactive) map.on('click', () => map.scrollWheelZoom.enable());
    const shapeOpts = interactive ? {} : { interactive: false };
    const mark = (layer, label) => {
      if (!label) return;
      if (interactive) layer.bindPopup(label);
      else layer.bindTooltip(label, { permanent: true, direction: 'right' });
    };
    let tiles = 'https://tile.openstreetmap.org/{z}/{x}/{y}.png';
    let center = null, zoom = 4;
    const pts = [];
    const ll = (s) => s.split(',').map((x) => parseFloat(x));
    const { kv } = parseKeyed(text);
    for (const [k, v] of kv) {
      if (k === 'center') center = ll(v);
      else if (k === 'zoom') zoom = parseFloat(v);
      else if (k === 'tiles') tiles = v;
    }
    L.tileLayer(tiles, { maxZoom: 19, attribution: '&copy; OpenStreetMap contributors' }).addTo(map);
    for (const [k, v] of kv) {
      const parts = v.split('|').map((x) => x.trim());
      const spec = parts[0];
      let label = parts[1];
      if (k === 'marker') {
        const p = ll(spec); if (p.some(isNaN)) continue;
        const m = L.marker(p, interactive ? {} : { interactive: false, keyboard: false }).addTo(map); mark(m, label); pts.push(p);
      } else if (k === 'circle') {
        const a = ll(spec); if (a.slice(0, 2).some(isNaN)) continue;
        let radius = a[2];
        if (!isFinite(radius)) { radius = parseFloat(parts[1]); label = parts[2]; }
        if (!isFinite(radius) || radius <= 0) radius = 500;
        const c = L.circle([a[0], a[1]], Object.assign({ radius }, shapeOpts)).addTo(map); mark(c, label); pts.push([a[0], a[1]]);
      } else if (k === 'line') {
        const path = spec.split(';').map(ll).filter((p) => p.length === 2 && !p.some(isNaN));
        if (path.length > 1) { const l = L.polyline(path, Object.assign({ color: '#3366cc', weight: 4 }, shapeOpts)).addTo(map); mark(l, label); pts.push(...path); }
      }
    }
    if (center && !center.some(isNaN)) map.setView(center, zoom);
    else if (pts.length > 1) map.fitBounds(pts, { padding: [30, 30] });
    else if (pts.length === 1) map.setView(pts[0], 10);
    else map.setView([20, 0], 2);
    setTimeout(() => map.invalidateSize(), 200);
  }).catch((e) => { host.innerHTML = `<div class="widget-loading">${esc(e.message)}</div>`; });
}

function initDesmos(el) {
  const { kv, rest } = (() => {
    const lines = el.querySelector('textarea.desmos-src').value.split('\n');
    const opts = []; const exprs = [];
    for (const raw of lines) {
      const l = raw.trim();
      if (!l || l.startsWith('#')) continue;
      const m = /^(bounds|height|expressions|keypad)\s*:\s*(.*)$/i.exec(l);
      if (m) opts.push([m[1].toLowerCase(), m[2]]); else exprs.push(l);
    }
    return { kv: opts, rest: exprs };
  })();
  const opt = Object.fromEntries(kv);
  const height = +opt.height || el.dataset.height || 400;
  el.innerHTML = `<div class="desmos-canvas" style="height:${height}px"><div class="widget-loading">Loading Desmos...</div></div>` +
    '<div class="widget-note">Interactive Desmos graph.</div>';
  const host = el.querySelector('.desmos-canvas');
  loadScript(`https://www.desmos.com/api/v1.11/calculator.js?apiKey=${CONFIG.desmosKey}`).then(() => {
    host.innerHTML = '';
    const calc = Desmos.GraphingCalculator(host, {
      expressions: opt.expressions !== 'false', keypad: opt.keypad !== 'false', settingsMenu: false,
    });
    rest.forEach((latex, i) => calc.setExpression({ id: 'e' + i, latex }));
    if (opt.bounds) {
      const [left, right, bottom, top] = opt.bounds.split(',').map(parseFloat);
      if ([left, right, bottom, top].every(isFinite)) calc.setMathBounds({ left, right, bottom, top });
    }
  }).catch((e) => { host.innerHTML = `<div class="widget-loading">${esc(e.message)}</div>`; });
}

function initModel(el) {
  const a = JSON.parse(el.dataset.attrs || '{}');
  if (!a.src) { el.innerHTML = '<div class="widget-loading">3D model: missing src</div>'; return; }
  loadScript(CONFIG.libs.modelViewer, true).catch(() => {});
  const mv = document.createElement('model-viewer');
  mv.setAttribute('src', resolveMedia(a.src));
  mv.setAttribute('alt', a.alt || '3D model');
  mv.setAttribute('camera-controls', '');
  mv.setAttribute('touch-action', 'pan-y');
  mv.setAttribute('shadow-intensity', '1');
  if (a.rotate !== 'false') mv.setAttribute('auto-rotate', '');
  if (a.poster) mv.setAttribute('poster', resolveMedia(a.poster));
  mv.style.height = (+a.height || 420) + 'px';
  el.appendChild(mv);
  const note = document.createElement('div');
  note.className = 'widget-note';
  note.textContent = 'Interactive 3D model. Drag to rotate, scroll or pinch to zoom.';
  el.appendChild(note);
}

function initTimeline(el) {
  const data = JSON.parse(el.dataset.timeline);
  const events = data.events, n = events.length;
  const ctx = { fnNumber: new Map(), fnCount: new Map() };
  const PAD = 80, GAP = 52, PER = 150;
  el.innerHTML =
    '<div class="tl-viewport" tabindex="0" aria-label="Timeline. Use the left and right arrow keys to move between events."><div class="tl-track"><div class="tl-axis"></div></div></div>' +
    '<div class="tl-detail" aria-live="polite"></div>' +
    '<div class="tl-nav"><button type="button" data-a="prev">Previous</button><span class="tl-count"></span><button type="button" data-a="next">Next</button>' +
    '<span class="widget-note">Click an event, use the buttons or the arrow keys. Drag the timeline to scroll.</span></div>';
  const viewport = el.querySelector('.tl-viewport'), track = el.querySelector('.tl-track');
  const detail = el.querySelector('.tl-detail'), count = el.querySelector('.tl-count');
  const prev = el.querySelector('[data-a=prev]'), next = el.querySelector('[data-a=next]');

  // x positions: proportional to the dates when all are numeric, otherwise evenly spaced;
  // neighbours are always at least GAP px apart so the labels cannot pile up
  const avail = viewport.clientWidth || 640;
  let xs;
  if (data.numeric && n > 1) {
    const min = events[0].when, span = events[n - 1].when - min || 1;
    const natural = Math.max(n * PER, avail);
    xs = events.map((e) => PAD + ((e.when - min) / span) * (natural - 2 * PAD));
  } else xs = events.map((e, i) => PAD + i * PER);
  for (let i = 1; i < n; i++) if (xs[i] < xs[i - 1] + GAP) xs[i] = xs[i - 1] + GAP;
  track.style.width = Math.max(avail, xs[n - 1] + PAD) + 'px';

  const lanes = [['up', ''], ['down', ''], ['up', ' far'], ['down', ' far']];
  const markers = events.map((e, i) => {
    const [dir, far] = lanes[i % 4];
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'tl-marker ' + dir + far;
    b.style.left = xs[i] + 'px';
    b.innerHTML = `<span class="tl-dot"></span><span class="tl-card"><span class="tl-date">${esc(e.date)}</span><span class="tl-name">${esc(e.title)}</span></span>`;
    b.addEventListener('click', () => select(i, true));
    track.appendChild(b);
    return b;
  });

  let sel = -1;
  function select(i, scroll) {
    sel = Math.max(0, Math.min(n - 1, i));
    markers.forEach((m, k) => { m.classList.toggle('active', k === sel); m.setAttribute('aria-pressed', k === sel ? 'true' : 'false'); });
    const e = events[sel];
    detail.innerHTML = `<div class="tl-detail-date">${esc(e.date)}</div><h4 class="tl-detail-title">${inline(e.title, ctx)}</h4>` +
      (e.desc ? `<div class="tl-detail-text">${inline(e.desc, ctx)}</div>` : '');
    if (/\$/.test(e.title + e.desc)) renderMath(detail);
    count.textContent = (sel + 1) + ' / ' + n;
    prev.disabled = sel === 0; next.disabled = sel === n - 1;
    if (scroll) viewport.scrollTo({ left: Math.max(0, xs[sel] - viewport.clientWidth / 2), behavior: 'smooth' });
  }
  prev.addEventListener('click', () => select(sel - 1, true));
  next.addEventListener('click', () => select(sel + 1, true));
  viewport.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowRight' || e.key === 'ArrowDown') { e.preventDefault(); select(sel + 1, true); }
    else if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') { e.preventDefault(); select(sel - 1, true); }
    else if (e.key === 'Home') { e.preventDefault(); select(0, true); }
    else if (e.key === 'End') { e.preventDefault(); select(n - 1, true); }
  });

  // drag to scroll with a mouse (touch and trackpads scroll natively)
  let drag = null;
  viewport.addEventListener('pointerdown', (e) => {
    if (e.pointerType !== 'mouse' || e.button !== 0 || e.target.closest('.tl-marker')) return;
    drag = { x: e.clientX, left: viewport.scrollLeft };
    viewport.setPointerCapture(e.pointerId);
    viewport.classList.add('dragging');
  });
  viewport.addEventListener('pointermove', (e) => { if (drag) viewport.scrollLeft = drag.left - (e.clientX - drag.x); });
  const stop = () => { drag = null; viewport.classList.remove('dragging'); };
  viewport.addEventListener('pointerup', stop);
  viewport.addEventListener('pointercancel', stop);

  select(0, false);
}

/* ---------------------------------------------------------------- CSV search
   Fetches a CSV file and shows it as a table with one search box per column, a search box
   for all columns, and click-to-sort headers.
   Column boxes:  text  = contains (any case)   !text = does not contain   =text = exactly
                  >100  <=5  =7  = compare numbers      10..20 = number range             */

const CSV_NUM = '-?\\d[\\d,]*(?:\\.\\d+)?';
const csvNum = (c) => {
  const t = String(c).replace(/[,$%\s]/g, '');
  return /^[-+]?(\d+\.?\d*|\.\d+)$/.test(t) ? parseFloat(t) : NaN;
};

function csvMatcher(term) {
  term = term.trim();
  if (!term) return null;
  let m = new RegExp('^(>=|<=|>|<|=)\\s*(' + CSV_NUM + ')$').exec(term);
  if (m) {
    const op = m[1], v = csvNum(m[2]);
    return (c) => {
      const x = csvNum(c);
      if (isNaN(x)) return false;
      return op === '>' ? x > v : op === '<' ? x < v : op === '>=' ? x >= v : op === '<=' ? x <= v : x === v;
    };
  }
  m = new RegExp('^(' + CSV_NUM + ')\\s*\\.\\.\\s*(' + CSV_NUM + ')$').exec(term);
  if (m) {
    let lo = csvNum(m[1]), hi = csvNum(m[2]);
    if (lo > hi) [lo, hi] = [hi, lo];
    return (c) => { const x = csvNum(c); return !isNaN(x) && x >= lo && x <= hi; };
  }
  if (term[0] === '=' && term.length > 1) { const q = term.slice(1).trim().toLowerCase(); return (c) => c.trim().toLowerCase() === q; }
  if (term[0] === '!' && term.length > 1) { const q = term.slice(1).trim().toLowerCase(); return (c) => !c.toLowerCase().includes(q); }
  const q = term.toLowerCase();
  return (c) => c.toLowerCase().includes(q);
}

async function initCsvSearch(el) {
  const a = JSON.parse(el.dataset.attrs || '{}');
  const inlineText = (el.querySelector('textarea.csv-src') || { value: '' }).value;
  const height = +a.height || 360;
  const PAGE = 100;
  el.innerHTML = '<div class="widget-loading">Loading data...</div>';
  let text = inlineText;
  if (a.src) {
    try {
      const r = await fetch(resolveMedia(a.src));
      if (!r.ok) throw new Error('HTTP ' + r.status);
      text = await r.text();
    } catch (e) {
      el.innerHTML = `<div class="widget-loading">Could not load ${esc(a.src)} (${esc(e.message)}).` +
        (location.protocol === 'file:' ? ' Serve the site over HTTP; browsers block fetch() on file:// pages.' : '') + '</div>';
      return;
    }
  }
  const rows = parseCSV(text.replace(/^\uFEFF/, ''));
  if (!text.trim() || !rows.length) { el.innerHTML = '<div class="widget-loading">This CSV file is empty.</div>'; return; }
  const header = rows[0], ncol = header.length;
  const data = rows.slice(1).map((r) => { const c = r.slice(0, ncol); while (c.length < ncol) c.push(''); return c; });
  const hay = data.map((r) => r.join('\u0001').toLowerCase());
  const isNum = header.map((_, c) => {
    let seen = 0, ok = 0;
    for (const r of data) if (r[c].trim() !== '') { seen++; if (!isNaN(csvNum(r[c]))) ok++; }
    return seen > 0 && ok / seen >= 0.9;
  });

  el.innerHTML =
    '<div class="csvs"><div class="csvs-top"><input type="search" class="csvs-all" placeholder="Search all columns" aria-label="Search all columns">' +
    '<span class="csvs-count"></span><button type="button" class="csvs-clear">Clear</button></div>' +
    `<div class="csvs-scroll" style="max-height:${height}px"><table class="csvs-table"><thead>` +
    '<tr class="csvs-heads">' + header.map((h, c) => `<th data-c="${c}" class="${isNum[c] ? 'num' : ''}" title="Click to sort">${esc(h)}<span class="csvs-arrow"></span></th>`).join('') + '</tr>' +
    '<tr class="csvs-filters">' + header.map((h, c) => `<th><input type="search" data-c="${c}" placeholder="${esc(h)}" aria-label="Search ${esc(h)}"></th>`).join('') + '</tr>' +
    '</thead><tbody></tbody></table></div><div class="csvs-more"></div>' +
    '<div class="widget-note">Type in any box; the boxes combine. Text matches anywhere in a cell, <code>!text</code> excludes it, ' +
    '<code>&gt;100</code> or <code>&lt;=5</code> compare numbers, <code>10..20</code> is a range. Click a column title to sort.</div></div>';

  // the filter row sticks just below the title row, whatever its rendered height
  const headH = el.querySelector('.csvs-heads').offsetHeight;
  if (headH) el.querySelectorAll('.csvs-filters th').forEach((t) => { t.style.top = headH + 'px'; });
  const $all = el.querySelector('.csvs-all'), $count = el.querySelector('.csvs-count'), $more = el.querySelector('.csvs-more');
  const $body = el.querySelector('tbody'), $inputs = [...el.querySelectorAll('.csvs-filters input')];
  const $heads = [...el.querySelectorAll('.csvs-heads th')];
  let sortCol = -1, sortDir = 1, shown = PAGE, matches = [];

  function apply() {
    const ms = $inputs.map((i) => csvMatcher(i.value));
    const toks = $all.value.toLowerCase().split(/\s+/).filter(Boolean);
    matches = [];
    for (let i = 0; i < data.length; i++) {
      if (toks.length && !toks.every((t) => hay[i].includes(t))) continue;
      let ok = true;
      for (let c = 0; c < ncol && ok; c++) if (ms[c] && !ms[c](data[i][c])) ok = false;
      if (ok) matches.push(i);
    }
    if (sortCol >= 0) {
      const c = sortCol, d = sortDir;
      if (isNum[c]) {
        matches.sort((x, y) => {
          const p = csvNum(data[x][c]), q = csvNum(data[y][c]);
          if (isNaN(p) && isNaN(q)) return 0;
          if (isNaN(p)) return 1;
          if (isNaN(q)) return -1;
          return (p - q) * d;
        });
      } else {
        matches.sort((x, y) => data[x][c].localeCompare(data[y][c], undefined, { numeric: true, sensitivity: 'base' }) * d);
      }
    }
    shown = PAGE;
    paint();
  }

  function paint() {
    const part = matches.slice(0, shown);
    $body.innerHTML = part.length
      ? part.map((i) => '<tr>' + data[i].map((v, c) => `<td${isNum[c] ? ' class="num"' : ''}>${esc(v)}</td>`).join('') + '</tr>').join('')
      : `<tr><td colspan="${ncol}" class="csvs-none">No matching rows.</td></tr>`;
    $count.textContent = matches.length === data.length ? `${data.length} rows` : `${matches.length} of ${data.length} rows`;
    $more.innerHTML = matches.length > shown
      ? `<button type="button">Show ${Math.min(PAGE, matches.length - shown)} more</button> <span class="widget-note">Showing ${shown} of ${matches.length}. Narrow the search to see the rest.</span>` : '';
    $heads.forEach((h, c) => { h.querySelector('.csvs-arrow').textContent = c === sortCol ? (sortDir === 1 ? ' \u25B2' : ' \u25BC') : ''; });
  }

  let timer = 0;
  const schedule = () => { clearTimeout(timer); timer = setTimeout(apply, data.length > 2000 ? 150 : 0); };
  $all.addEventListener('input', schedule);
  $inputs.forEach((i) => i.addEventListener('input', schedule));
  $heads.forEach((h, c) => h.addEventListener('click', () => {
    if (sortCol === c) { if (sortDir === 1) sortDir = -1; else { sortCol = -1; sortDir = 1; } } else { sortCol = c; sortDir = 1; }
    apply();
  }));
  el.querySelector('.csvs-clear').addEventListener('click', () => { $all.value = ''; $inputs.forEach((i) => { i.value = ''; }); sortCol = -1; sortDir = 1; apply(); });
  $more.addEventListener('click', (e) => { if (e.target.closest('button')) { shown += PAGE; paint(); } });
  apply();
}

async function renderMath(root) {
  const nodes = root.querySelectorAll('[data-tex]');
  if (!nodes.length) return;
  loadCss(CONFIG.libs.katexCss);
  try { await loadScript(CONFIG.libs.katex); } catch (e) { return; }
  nodes.forEach((n) => {
    try { katex.render(n.dataset.tex, n, { displayMode: n.dataset.display === '1', throwOnError: false }); }
    catch (e) { n.textContent = n.dataset.tex; }
  });
}

function initWidgets(root) {
  renderMath(root);
  root.querySelectorAll('.cw').forEach((el) => lazy(el, () => initCodeWidget(el)));
  root.querySelectorAll('.sheet').forEach((el) => initSheet(el));
  root.querySelectorAll('.map-widget').forEach((el) => lazy(el, () => initMap(el)));
  root.querySelectorAll('.desmos-widget').forEach((el) => lazy(el, () => initDesmos(el)));
  root.querySelectorAll('.model-widget').forEach((el) => lazy(el, () => initModel(el)));
  root.querySelectorAll('.timeline-widget[data-timeline]').forEach((el) => lazy(el, () => initTimeline(el)));
  root.querySelectorAll('.csv-widget').forEach((el) => lazy(el, () => initCsvSearch(el)));
}

/* ==========================================================================
   Boot: manifest, sidebar, search, article
   ========================================================================== */

async function boot() {
  const params = new URLSearchParams(location.search);
  let manifest = { siteTitle: 'Wiki', pages: [] };
  try {
    const r = await fetch(CONFIG.manifest, { cache: 'no-store' });
    if (r.ok) manifest = await r.json();
  } catch (e) { /* handled below */ }

  const pages = (manifest.pages || []).slice().sort((a, b) => a.title.localeCompare(b.title));
  $('site-title').textContent = manifest.siteTitle || 'Wiki';

  let slug = params.get('p');
  if (!slug) {
    const main = pages.find((p) => p.slug === 'main-page') || pages[0];
    slug = main ? main.slug : '';
  }

  // The sidebar lists only the featured articles (older manifests have no list: show everything).
  // Every other article is reachable through search, the home page and the site map.
  const featured = Array.isArray(manifest.featured) ? new Set(manifest.featured) : null;
  const sidebar = pages.filter((p) => !featured || featured.has(p.slug));
  const here = pages.find((p) => p.slug === slug);
  if (here && !sidebar.includes(here)) sidebar.push(here);
  const list = $('sidebar-list');
  for (const p of sidebar) {
    const li = document.createElement('li');
    if (p.slug === slug) li.className = 'current';
    const a = document.createElement('a');
    a.href = '?p=' + encodeURIComponent(p.slug);
    a.textContent = p.title;
    li.appendChild(a); list.appendChild(li);
  }
  setupSearch(pages);

  const content = $('content');
  if (!slug) { $('firstHeading').textContent = 'No articles'; content.innerHTML = '<p>This wiki has no articles yet.</p>'; return; }

  let md;
  try {
    const r = await fetch(CONFIG.pagesDir + encodeURIComponent(slug) + '.md', { cache: 'no-store' });
    if (!r.ok) throw new Error('HTTP ' + r.status);
    md = await r.text();
  } catch (e) {
    $('firstHeading').textContent = location.protocol === 'file:' ? 'Cannot load articles from file://' : 'Article not found';
    content.innerHTML = location.protocol === 'file:'
      ? '<p>Browsers block <code>fetch()</code> on <code>file://</code> pages. Serve this folder over HTTP, for example with <code>python3 -m http.server</code>, or publish it with GitHub Pages.</p>'
      : `<p>There is no article named <code>${esc(slug)}</code>.</p>`;
    return;
  }

  const { html, title } = renderMarkdown(md);
  const heading = title || slug.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
  $('firstHeading').textContent = heading;
  document.title = heading + ' - ' + (manifest.siteTitle || 'Wiki');
  content.innerHTML = html;
  initWidgets(content);
  if (location.hash) setTimeout(() => { const t = document.getElementById(decodeURIComponent(location.hash.slice(1))); if (t) t.scrollIntoView(); }, 150);
}

function setupSearch(pages) {
  const input = $('search'), box = $('search-results');
  let active = -1;
  const show = () => {
    const q = input.value.trim().toLowerCase();
    box.innerHTML = ''; active = -1;
    if (!q) { box.style.display = 'none'; return; }
    const hits = pages.filter((p) => p.title.toLowerCase().includes(q) || p.slug.includes(q)).slice(0, 12);
    for (const p of hits) {
      const a = document.createElement('a');
      a.href = '?p=' + encodeURIComponent(p.slug); a.textContent = p.title; box.appendChild(a);
    }
    if (!hits.length) { const d = document.createElement('div'); d.className = 'muted'; d.style.padding = '0.5rem 0.75rem'; d.textContent = 'No matching articles'; box.appendChild(d); }
    box.style.display = 'block';
  };
  input.addEventListener('input', show);
  input.addEventListener('keydown', (e) => {
    const items = [...box.querySelectorAll('a')];
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault(); if (!items.length) return;
      active = (active + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
      items.forEach((a, i) => a.classList.toggle('active', i === active));
    } else if (e.key === 'Enter') { const t = items[active >= 0 ? active : 0]; if (t) location.href = t.href; }
    else if (e.key === 'Escape') box.style.display = 'none';
  });
  document.addEventListener('click', (e) => { if (!e.target.closest('.wk-search')) box.style.display = 'none'; });
}

if (typeof document !== 'undefined') boot();
