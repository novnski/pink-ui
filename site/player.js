// Hosts the pink-ui wasm module: one GPUI window drawn into a canvas that
// fills this page. Opened on its own (play.html?c=button&theme=dark) it is a
// full-page player; embedded by the site, it takes commands over postMessage
// and reports progress, readiness and the demo's size back.
//
// Messages are objects with a `pinkui` field naming the kind:
//   in:  show {slug}, theme {dark}, replay
//   out: progress {loaded, total}, ready, resize {width, height},
//        wheel {dx, dy}, error {message}

import init, { start, show, set_theme, replay } from './pkg/pink_ui_web.js?v=__BUILD__';

const params = new URLSearchParams(location.search);
const embedded = window.parent !== window;
const state = {
  slug: params.get('c') || 'button',
  dark: params.get('theme') === 'dark',
};
let ready = false;
// The demo's laid-out size, once it reports one (demos that fill the stage
// don't).
let size = null;

function tell(kind, data = {}) {
  if (embedded) window.parent.postMessage({ pinkui: kind, ...data }, '*');
}

function apply(message) {
  switch (message.pinkui) {
    case 'show':
      state.slug = message.slug;
      size = null;
      if (ready) show(message.slug);
      break;
    case 'theme':
      state.dark = !!message.dark;
      document.documentElement.classList.toggle('dark', state.dark);
      if (ready) set_theme(state.dark);
      break;
    case 'replay':
      if (ready) replay();
      break;
  }
}

window.addEventListener('message', (event) => {
  if (event.source === window.parent && event.data && event.data.pinkui) apply(event.data);
});

// The canvas keeps every wheel event for itself. Embedded, let the page
// scroll past the preview until the visitor clicks into it, the way map
// embeds do; hovering works either way.
if (embedded) {
  let engaged = false;
  window.addEventListener('pointerdown', () => { engaged = true; }, true);
  document.documentElement.addEventListener('mouseleave', () => { engaged = false; });
  window.addEventListener('blur', () => { engaged = false; });
  window.addEventListener('wheel', (event) => {
    if (engaged) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? window.innerHeight : 1;
    tell('wheel', { dx: event.deltaX * unit, dy: event.deltaY * unit });
  }, { capture: true, passive: false });
}

// Fetch the module ourselves to report progress. Pages serves it compressed,
// so the total comes from the build (the uncompressed size), not the headers.
async function download(url, total) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  if (!response.body) return new Uint8Array(await response.arrayBuffer());
  const reader = response.body.getReader();
  const chunks = [];
  let loaded = 0;
  let last = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    loaded += value.length;
    const now = performance.now();
    if (now - last > 60) {
      last = now;
      tell('progress', { loaded, total });
    }
  }
  tell('progress', { loaded: total, total });
  const bytes = new Uint8Array(loaded);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

try {
  const bytes = await download('pkg/pink_ui_web_bg.wasm?v=__BUILD__', __WASM_BYTES__);
  await init({ module_or_path: bytes });
  const timeout = setTimeout(() => {
    tell('error', { message: 'This browser has neither WebGPU nor WebGL2.' });
  }, 15000);
  const initial = { ...state };
  start(initial.slug, initial.dark, () => {
    // Called from inside GPUI's update: call back into it on the next task.
    setTimeout(() => {
      clearTimeout(timeout);
      ready = true;
      // Catch up on anything that arrived while graphics were starting.
      if (state.slug !== initial.slug) show(state.slug);
      if (state.dark !== initial.dark) set_theme(state.dark);
      window.__pinkuiReady = true;
      tell('ready');
    }, 0);
  }, (width, height) => {
    size = { width, height };
    tell('resize', { width, height });
  });
} catch (error) {
  console.error(error);
  tell('error', { message: String(error && error.message || error) });
}

// For scripts driving the player directly (the thumbnail renderer).
window.pinkui = {
  show: (slug) => apply({ pinkui: 'show', slug }),
  theme: (dark) => apply({ pinkui: 'theme', dark }),
  replay: () => apply({ pinkui: 'replay' }),
  size: () => size,
};
