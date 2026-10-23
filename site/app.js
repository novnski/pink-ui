// pink-ui showcase: the catalog, the component pages and the bridge to the
// live player (play.html, which hosts the wasm build of the components).
//
// Routes live in the hash: `#button` opens a component, anything else is the
// catalog. The player iframe is created on the first component page and then
// kept, so moving between components only swaps the demo inside it.

(function () {
  'use strict';

  const REPO = '__REPO__';
  const BUILD = '__BUILD__';
  // The player's design size; narrower stages scale it down.
  const STAGE_WIDTH = 900;
  const STAGE_MIN_HEIGHT = 600;
  // Room around the demo when it is taller than the default stage.
  const STAGE_PADDING = 96;

  const catalog = JSON.parse(document.getElementById('catalog-data').textContent);
  const components = catalog.components;
  const bySlug = new Map(components.map((c, i) => [c.slug, { ...c, index: i }]));
  const $ = (selector) => document.querySelector(selector);

  const store = {
    get(key) { try { return localStorage.getItem(key); } catch (e) { return null; } },
    set(key, value) { try { localStorage.setItem(key, value); } catch (e) { /* private mode */ } },
  };

  function escapeHtml(text) {
    return text.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
  }

  function toast(message) {
    const el = $('#toast');
    el.textContent = message;
    el.classList.add('show');
    clearTimeout(toast.timer);
    toast.timer = setTimeout(() => el.classList.remove('show'), 1600);
  }

  async function copy(text) {
    try {
      await navigator.clipboard.writeText(text);
      toast('Copied');
    } catch (e) {
      toast('Copy failed');
    }
  }

  function thumb(slug) {
    return `thumbs/${slug}-${theme()}.jpg?v=${BUILD}`;
  }

  // Theme and look ------------------------------------------------------

  function theme() {
    return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
  }

  function setTheme(next) {
    document.documentElement.dataset.theme = next;
    store.set('pinkui-theme', next);
    player.post({ pinkui: 'theme', dark: next === 'dark' });
    for (const img of document.querySelectorAll('img[data-thumb]')) img.src = thumb(img.dataset.thumb);
    syncControls();
  }

  let look = store.get('pinkui-look') === 'classic' ? 'classic' : 'clean';

  function setLook(next) {
    look = next;
    store.set('pinkui-look', next);
    player.post({ pinkui: 'look', classic: next === 'classic' });
    syncControls();
  }

  function syncControls() {
    for (const b of document.querySelectorAll('#mode button')) {
      b.setAttribute('aria-pressed', String(b.dataset.mode === theme()));
    }
    for (const b of document.querySelectorAll('#look button')) {
      b.setAttribute('aria-pressed', String(b.dataset.look === look));
    }
    const current = bySlug.get(currentSlug);
    if (current) $('#open').href = playerUrl(current.slug);
  }

  $('#theme-toggle').addEventListener('click', () => setTheme(theme() === 'dark' ? 'light' : 'dark'));
  $('#mode').addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) setTheme(b.dataset.mode); });
  $('#look').addEventListener('click', (e) => { const b = e.target.closest('button'); if (b) setLook(b.dataset.look); });

  // Catalog -------------------------------------------------------------

  const counts = {
    all: components.length,
    core: components.filter((c) => c.kind === 'core').length,
    pink: components.filter((c) => c.kind === 'pink').length,
  };
  for (const img of document.querySelectorAll('.hero-art img')) {
    img.src = thumb(img.dataset.thumb);
    // Without thumbnails (a local build), leave the hero plain.
    img.onerror = () => { img.closest('.hero-art').hidden = true; };
  }
  for (const el of document.querySelectorAll('[data-count]')) el.textContent = counts.all;
  for (const el of document.querySelectorAll('[data-count-core]')) el.textContent = counts.core;
  for (const el of document.querySelectorAll('[data-count-pink]')) el.textContent = counts.pink;

  let category = 'All';
  let query = '';

  function renderChips() {
    const chips = ['All', ...catalog.categories].map((name) => {
      const n = name === 'All' ? components.length : components.filter((c) => c.category === name).length;
      return `<button class="chip" type="button" data-category="${escapeHtml(name)}" aria-pressed="${name === category}">${escapeHtml(name)}<small>${n}</small></button>`;
    });
    $('#chips').innerHTML = chips.join('');
  }

  function kindBadge(c) {
    return c.kind === 'core'
      ? '<span class="badge pink">Clean · Classic</span>'
      : '<span class="badge">pink</span>';
  }

  function card(c) {
    return `<a class="card" href="#${c.slug}">
      <div class="card-media">
        <span class="ph">${escapeHtml(c.name)}</span>
        <img data-thumb="${c.slug}" src="${thumb(c.slug)}" alt="${escapeHtml(c.name)} preview" loading="lazy" decoding="async" onerror="this.remove()">
      </div>
      <div class="card-body">
        <div class="card-head"><h3>${escapeHtml(c.name)}</h3>${kindBadge(c)}</div>
        <p>${escapeHtml(c.description)}</p>
      </div>
    </a>`;
  }

  function matches(c) {
    if (category !== 'All' && c.category !== category) return false;
    if (!query) return true;
    const hay = `${c.name} ${c.category} ${c.description} ${c.kind}`.toLowerCase();
    return query.split(/\s+/).every((word) => hay.includes(word));
  }

  function renderCatalog() {
    const shown = components.filter(matches);
    let html;
    if (!shown.length) {
      html = `<p class="empty">No component matches “${escapeHtml(query)}”.</p>`;
    } else if (category === 'All' && !query) {
      html = catalog.categories.map((name) => {
        const list = shown.filter((c) => c.category === name);
        return `<div class="section-title"><h2>${escapeHtml(name)}</h2><span>${list.length}</span></div>
          <div class="grid">${list.map(card).join('')}</div>`;
      }).join('');
    } else {
      html = `<div class="section-title"><h2>${escapeHtml(query ? 'Results' : category)}</h2><span>${shown.length}</span></div>
        <div class="grid">${shown.map(card).join('')}</div>`;
    }
    $('#catalog').innerHTML = html;
  }

  $('#chips').addEventListener('click', (e) => {
    const b = e.target.closest('button');
    if (!b) return;
    category = b.dataset.category;
    renderChips();
    renderCatalog();
  });
  $('#search').addEventListener('input', (e) => {
    query = e.target.value.trim().toLowerCase();
    renderCatalog();
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === '/' && !$('#home').hidden && document.activeElement !== $('#search')) {
      e.preventDefault();
      $('#search').focus();
    }
  });
  document.addEventListener('click', (e) => {
    const b = e.target.closest('[data-copy]');
    if (b) copy(document.querySelector(b.dataset.copy).textContent);
  });

  // Player --------------------------------------------------------------

  function playerUrl(slug) {
    return `play.html?c=${slug}&theme=${theme()}&look=${look}`;
  }

  const stage = $('#stage');
  const player = {
    frame: null,
    // The component the player shows, and the one it was opened with.
    slug: null,
    initialSlug: null,
    ready: false,
    height: STAGE_MIN_HEIGHT,

    post(message) {
      if (this.frame && this.frame.contentWindow) this.frame.contentWindow.postMessage(message, '*');
    },

    show(slug) {
      if (this.slug === slug) return;
      // The new demo reports its own size once it lays out.
      this.height = STAGE_MIN_HEIGHT;
      layoutStage();
      if (!this.frame) {
        const frame = document.createElement('iframe');
        frame.title = 'Live component preview';
        frame.src = playerUrl(slug);
        frame.setAttribute('allow', 'clipboard-read; clipboard-write');
        stage.appendChild(frame);
        this.frame = frame;
        this.initialSlug = slug;
        this.slug = slug;
        layoutStage();
      } else {
        this.slug = slug;
        this.post({ pinkui: 'show', slug });
      }
    },
  };

  function layoutStage() {
    const width = stage.clientWidth || STAGE_WIDTH;
    const scale = Math.min(1, width / STAGE_WIDTH);
    stage.style.height = `${Math.round(player.height * scale)}px`;
    if (player.frame) {
      player.frame.style.width = `${width / scale}px`;
      player.frame.style.height = `${player.height}px`;
      player.frame.style.transform = scale < 1 ? `scale(${scale})` : '';
    }
  }
  new ResizeObserver(layoutStage).observe(stage);

  function setLoading(state, text, fraction) {
    const loading = $('#loading');
    loading.classList.toggle('done', state === 'ready');
    loading.classList.toggle('error', state === 'error');
    $('#live').classList.toggle('on', state === 'ready');
    $('#live-text').textContent = state === 'ready' ? 'Live' : state === 'error' ? 'Still image' : 'Loading';
    if (text) $('#loading-label').textContent = text;
    if (fraction !== undefined) {
      $('#loading-bar').style.width = `${Math.round(fraction * 100)}%`;
      $('#loading-size').textContent = `${Math.round(fraction * 100)}%`;
    }
    $('#poster').style.opacity = state === 'ready' ? '0' : state === 'error' ? '1' : '';
  }

  window.addEventListener('message', (event) => {
    const data = event.data;
    if (!player.frame || event.source !== player.frame.contentWindow || !data || !data.pinkui) return;
    switch (data.pinkui) {
      case 'progress':
        if (!player.ready) setLoading('loading', 'Loading WebAssembly', data.total ? data.loaded / data.total : 0);
        if (data.loaded >= data.total) setLoading('loading', 'Starting GPUI');
        break;
      case 'ready':
        player.ready = true;
        setLoading('ready');
        // Anything sent before the player was listening is lost: resync.
        if (player.slug !== player.initialSlug) player.post({ pinkui: 'show', slug: player.slug });
        player.post({ pinkui: 'theme', dark: theme() === 'dark' });
        player.post({ pinkui: 'look', classic: look === 'classic' });
        break;
      case 'resize': {
        const wanted = Math.max(STAGE_MIN_HEIGHT, Math.ceil(data.height) + STAGE_PADDING);
        if (wanted > player.height) {
          player.height = wanted;
          layoutStage();
        }
        break;
      }
      case 'wheel':
        window.scrollBy(data.dx, data.dy);
        break;
      case 'error':
        setLoading('error', `Live preview unavailable: ${data.message}`);
        break;
    }
  });

  $('#replay').addEventListener('click', () => player.post({ pinkui: 'replay' }));

  // Rust highlighting -----------------------------------------------------

  const KEYWORDS = new Set(('as async await break const continue crate dyn else enum extern false fn for if impl in let loop match mod move mut pub ref return self Self static struct super trait true type unsafe use where while').split(' '));
  const TOKEN = /(\/\/[^\n]*)|(\/\*[\s\S]*?\*\/)|(b?r(#*)"[\s\S]*?"\4)|(b?"(?:\\[\s\S]|[^"\\])*")|('(?:\\[^\n']{1,10}|[^\\'\n])')|('[A-Za-z_]\w*)|(#!?\[[^\]\n]*\])|(\b\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?(?:[a-z]\w*)?)|([A-Za-z_]\w*!)|([A-Za-z_]\w*)/g;

  // Splits Rust source into [class, text] pieces ('' for plain text).
  function tokens(source) {
    const out = [];
    let last = 0;
    for (const m of source.matchAll(TOKEN)) {
      if (m.index > last) out.push(['', source.slice(last, m.index)]);
      const text = m[0];
      let cls = '';
      if (m[1] || m[2]) cls = 'c';
      else if (m[3] || m[5] || m[6]) cls = 's';
      else if (m[7]) cls = 'l';
      else if (m[8]) cls = 'a';
      else if (m[9]) cls = 'n';
      else if (m[10]) cls = 'm';
      else if (m[11]) {
        if (KEYWORDS.has(text)) cls = 'k';
        else if (/^[A-Z]/.test(text)) cls = 't';
        else if (source[m.index + text.length] === '(') cls = 'f';
      }
      out.push([cls, text]);
      last = m.index + text.length;
    }
    if (last < source.length) out.push(['', source.slice(last)]);
    return out;
  }

  // Highlighted HTML, one string per line, so line numbers can go in front.
  function highlightLines(source) {
    const lines = [''];
    for (const [cls, text] of tokens(source)) {
      text.split('\n').forEach((part, i) => {
        if (i > 0) lines.push('');
        if (!part) return;
        const html = escapeHtml(part);
        lines[lines.length - 1] += cls ? `<span class="tok-${cls}">${html}</span>` : html;
      });
    }
    return lines;
  }

  function codeBlock(source, { name = 'Rust', numbered = false, extraClass = '' } = {}) {
    const lines = highlightLines(source.replace(/\s+$/, ''));
    const body = numbered
      ? lines.map((l, i) => `<span class="ln">${i + 1}</span>${l}`).join('\n')
      : lines.join('\n');
    const id = `code-${Math.random().toString(36).slice(2, 9)}`;
    return `<div class="code ${extraClass}">
      <div class="code-bar"><span class="name">${escapeHtml(name)}</span>
        <button class="btn small ghost" type="button" data-copy-code="${id}">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><rect x="8" y="8" width="12" height="12" rx="2.5"/><path d="M16 8V6.5A2.5 2.5 0 0 0 13.5 4h-7A2.5 2.5 0 0 0 4 6.5v7A2.5 2.5 0 0 0 6.5 16H8"/></svg>Copy</button>
      </div>
      <pre><code>${body}</code></pre>
      <textarea id="${id}" hidden>${escapeHtml(source)}</textarea>
    </div>`;
  }

  document.addEventListener('click', (e) => {
    const b = e.target.closest('[data-copy-code]');
    if (b) copy(document.getElementById(b.dataset.copyCode).value);
  });

  // Module docs -----------------------------------------------------------

  // The `//!` header of a component file, as lines.
  function moduleDocs(source) {
    const lines = [];
    for (const line of source.split('\n')) {
      if (line.startsWith('//!')) lines.push(line.replace(/^\/\/! ?/, ''));
      else if (lines.length) break;
    }
    return lines;
  }

  function inline(text) {
    return escapeHtml(text)
      .replace(/\[`([^`]+)`\](\([^)\s]*\))?/g, '<code>$1</code>')
      .replace(/`([^`]+)`/g, '<code>$1</code>')
      .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
      .replace(/\[([^\]]+)\]\((https?:[^)\s]+)\)/g, '<a href="$2">$1</a>');
  }

  // A small Markdown subset: paragraphs, "- " lists and fenced code.
  function renderDocs(lines, name) {
    const html = [];
    let para = [];
    let list = null;
    let fence = null;
    const flushPara = () => {
      if (para.length) html.push(`<p>${inline(para.join(' '))}</p>`);
      para = [];
    };
    const flushList = () => {
      if (list) html.push(`<ul>${list.map((item) => `<li>${inline(item)}</li>`).join('')}</ul>`);
      list = null;
    };
    for (const line of lines) {
      if (fence) {
        if (line.startsWith('```')) {
          html.push(codeBlock(fence.lines.join('\n'), { name: fence.lang === 'text' ? 'Shell' : 'Rust' }));
          fence = null;
        } else {
          fence.lines.push(line);
        }
      } else if (line.startsWith('```')) {
        flushPara();
        flushList();
        fence = { lang: line.slice(3).trim(), lines: [] };
      } else if (!line.trim()) {
        flushPara();
        flushList();
      } else if (/^\s*- /.test(line)) {
        flushPara();
        list = list || [];
        list.push(line.replace(/^\s*- /, ''));
      } else if (list && /^\s{2,}/.test(line)) {
        list[list.length - 1] += ` ${line.trim()}`;
      } else {
        flushList();
        para.push(line.trim());
      }
    }
    flushPara();
    flushList();
    // The first paragraph restates the name and description shown above.
    if (html.length && html[0].startsWith(`<p>${escapeHtml(name)}`)) html.shift();
    return html.join('');
  }

  // The `story` function: a complete, compiling example of the component.
  function storySource(source) {
    const start = source.search(/^pub fn story\(/m);
    if (start < 0) return null;
    let depth = 0;
    for (let i = source.indexOf('{', start); i < source.length; i++) {
      if (source[i] === '{') depth++;
      else if (source[i] === '}' && --depth === 0) return source.slice(start, i + 1);
    }
    return null;
  }

  const sources = new Map();
  function fetchSource(c) {
    if (!sources.has(c.slug)) {
      sources.set(c.slug, fetch(`${c.source}?v=${BUILD}`).then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.text();
      }));
    }
    return sources.get(c.slug);
  }

  function moduleName(c) {
    return c.source.split('/').pop().replace(/\.rs$/, '');
  }

  async function renderPanels(c) {
    $('#usage').innerHTML = '<p class="prose">Loading…</p>';
    $('#source').innerHTML = '';
    let source;
    try {
      source = await fetchSource(c);
    } catch (e) {
      $('#usage').innerHTML = `<p class="prose">Couldn't load the source: ${escapeHtml(e.message)}. <a href="${REPO}/blob/main/${c.source}">Read it on GitHub</a>.</p>`;
      return;
    }
    if (currentSlug !== c.slug) return;
    const story = storySource(source);
    const setup = `use pink_ui::components::${moduleName(c)}::*;

// Once at startup, with \`pink_ui::Assets\` as the app's asset source:
pink_ui::init(cx);`;
    $('#usage').innerHTML = `
      <div class="prose">${renderDocs(moduleDocs(source), c.name)}</div>
      <div class="prose"><h3>Setup</h3><p>Every component is a builder: build it and drop it into <code>.child(…)</code>. Root views call <code>motion::frame</code> and add <code>motion::driver()</code> so hovers and glides animate; see the <a href="${REPO}#using-it-in-an-app">README</a>.</p></div>
      ${codeBlock(setup)}
      ${story ? `<div class="prose"><h3>This demo</h3></div>${codeBlock(story, { name: `${c.source} · story` })}` : ''}`;
    $('#source').innerHTML = `
      ${codeBlock(source, { name: c.source, numbered: true, extraClass: 'source' })}
      <p class="prose"><a href="${REPO}/blob/main/${c.source}">View ${escapeHtml(c.source)} on GitHub →</a></p>`;
  }

  function selectTab(which) {
    for (const tab of ['usage', 'source']) {
      $(`#tab-${tab}`).setAttribute('aria-selected', String(tab === which));
      $(`#${tab}`).hidden = tab !== which;
    }
  }
  $('#tab-usage').addEventListener('click', () => selectTab('usage'));
  $('#tab-source').addEventListener('click', () => selectTab('source'));

  // Component page ----------------------------------------------------------

  function renderSide() {
    $('#side').innerHTML = catalog.categories.map((name) => `<h4>${escapeHtml(name)}</h4>${components
      .filter((c) => c.category === name)
      .map((c) => `<a href="#${c.slug}" data-slug="${c.slug}">${escapeHtml(c.name)}</a>`)
      .join('')}`).join('');
  }

  let currentSlug = null;

  function showPage(c) {
    const changed = currentSlug !== c.slug;
    currentSlug = c.slug;
    $('#home').hidden = true;
    $('#page').hidden = false;
    document.title = `${c.name} – pink-ui`;
    $('#crumb').textContent = c.category;
    $('#name').textContent = c.name;
    $('#description').textContent = c.description;
    $('#badges').innerHTML = `<span class="badge">${escapeHtml(c.category)}</span>${kindBadge(c)}`;
    $('#look').hidden = c.kind !== 'core';
    for (const a of document.querySelectorAll('#side a')) {
      if (a.dataset.slug === c.slug) {
        a.setAttribute('aria-current', 'page');
        a.scrollIntoView({ block: 'nearest' });
      } else {
        a.removeAttribute('aria-current');
      }
    }
    const prev = components[(c.index + components.length - 1) % components.length];
    const next = components[(c.index + 1) % components.length];
    $('#pager').innerHTML = `
      <a href="#${prev.slug}"><small>← Previous</small><span>${escapeHtml(prev.name)}</span></a>
      <a class="next" href="#${next.slug}"><small>Next →</small><span>${escapeHtml(next.name)}</span></a>`;
    if (!player.ready) {
      const poster = $('#poster');
      poster.src = thumb(c.slug);
      poster.onerror = () => poster.removeAttribute('src');
    }
    syncControls();
    if (changed) {
      selectTab('usage');
      renderPanels(c);
      window.scrollTo(0, 0);
    }
    player.show(c.slug);
  }

  function showHome(anchor) {
    currentSlug = null;
    document.title = 'pink-ui – animated components for GPUI';
    $('#page').hidden = true;
    $('#home').hidden = false;
    if (anchor) document.getElementById(anchor)?.scrollIntoView();
  }

  function route() {
    const slug = decodeURIComponent(location.hash.replace(/^#\/?/, ''));
    const c = bySlug.get(slug);
    if (c) showPage(c);
    else showHome(slug || null);
  }

  renderChips();
  renderCatalog();
  renderSide();
  syncControls();
  window.addEventListener('hashchange', route);
  route();
})();
