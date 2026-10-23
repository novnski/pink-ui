// Copies the site's pages into the build, filling in the placeholders:
// __CATALOG__ (index.html), __BUILD__ (cache-busting id), __REPO__ and
// __WASM_BYTES__ (the module's size, for the download progress bar).
//
//   node scripts/site/pages.mjs site site/dist <build-id> <repo-url>

import fs from 'node:fs';
import path from 'node:path';

const [src, out, build, repo] = process.argv.slice(2);
const catalog = fs.readFileSync(path.join(out, 'catalog.json'), 'utf8');
const wasmBytes = fs.statSync(path.join(out, 'pkg', 'pink_ui_web_bg.wasm')).size;
const values = {
  // Inside <script type="application/json">, `</` would end the element.
  __CATALOG__: catalog.replaceAll('</', '<\\/'),
  __BUILD__: build,
  __REPO__: repo,
  __WASM_BYTES__: String(wasmBytes),
};

for (const name of fs.readdirSync(src)) {
  const from = path.join(src, name);
  if (fs.statSync(from).isDirectory() || name.startsWith('.')) continue;
  let text = fs.readFileSync(from, 'utf8');
  for (const [key, value] of Object.entries(values)) text = text.replaceAll(key, value);
  fs.writeFileSync(path.join(out, name), text);
}
