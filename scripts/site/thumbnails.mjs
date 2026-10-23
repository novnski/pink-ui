// Renders a light and a dark thumbnail of every component with the built
// player in headless Chromium, and fails if any component panics in the
// browser – so this doubles as the web build's smoke test.
//
//   node scripts/site/thumbnails.mjs site/dist
//
// Needs Playwright with Chromium (`npm install playwright && npx playwright
// install chromium`). Set PLAYWRIGHT to load it from elsewhere, WAIT to change
// how long each demo plays (ms) before its picture is taken.

import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const { chromium } = await import(process.env.PLAYWRIGHT ?? 'playwright');
const root = path.resolve(process.argv[2] ?? 'site/dist');
const wait = Number(process.env.WAIT ?? 3000);
// The player's viewport, and the cards' aspect ratio (site/styles.css).
const VIEW = { width: 900, height: 640 };
const ASPECT = 900 / 640;
const catalog = JSON.parse(fs.readFileSync(path.join(root, 'catalog.json'), 'utf8'));
const out = path.join(root, 'thumbs');
fs.mkdirSync(out, { recursive: true });

const types = {
  '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css',
  '.json': 'application/json', '.wasm': 'application/wasm', '.svg': 'image/svg+xml',
  '.ttf': 'font/ttf', '.jpg': 'image/jpeg', '.png': 'image/png', '.rs': 'text/plain',
};
const server = http.createServer((req, res) => {
  const file = path.join(root, decodeURIComponent(new URL(req.url, 'http://x').pathname));
  if (!file.startsWith(root) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { 'content-type': types[path.extname(file)] ?? 'application/octet-stream' });
  fs.createReadStream(file).pipe(res);
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${server.address().port}`;

// The demo with a margin, in the cards' aspect ratio. Stories center their
// demo; one that fills the stage reports no size and keeps the whole frame.
// At least 600px wide, so the ~300px cards get two pixels per point.
function crop(size) {
  if (!size) return { x: 0, y: 0, ...VIEW };
  let width = Math.max(size.width + 80, 600);
  let height = Math.max(size.height + 80, width / ASPECT);
  width = Math.max(width, height * ASPECT);
  const scale = Math.min(1, VIEW.width / width, VIEW.height / height);
  width *= scale;
  height *= scale;
  return { x: (VIEW.width - width) / 2, y: (VIEW.height - height) / 2, width, height };
}

// SwiftShader gives headless Chromium a software WebGL2 to fall back on.
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const panics = [];
let page;
let panicked = false;

async function open(slug, dark) {
  if (page) await page.close();
  // At 1x: Chromium's emulated device scale factor reports a devicePixelRatio
  // that its device-pixel-content-box sizes don't follow, which GPUI's web
  // platform reads as a smaller window.
  page = await browser.newPage({ viewport: VIEW });
  page.on('console', (m) => {
    if (m.type() === 'error' && /panicked at/.test(m.text())) {
      panicked = true;
      panics.push(m.text().split('\n')[0]);
    }
  });
  page.on('pageerror', (e) => { panicked = true; panics.push(String(e.message).split('\n')[0]); });
  await page.goto(`${base}/play.html?c=${slug}&theme=${dark ? 'dark' : 'light'}`);
  await page.waitForFunction(() => window.__pinkuiReady === true, null, { timeout: 90000 });
}

let failures = 0;
for (const dark of [false, true]) {
  const theme = dark ? 'dark' : 'light';
  await open(catalog.components[0].slug, dark);
  for (const c of catalog.components) {
    panicked = false;
    try {
      await page.evaluate((slug) => window.pinkui.show(slug), c.slug);
      await page.waitForTimeout(wait);
      if (!panicked) {
        const clip = crop(await page.evaluate(() => window.pinkui.size()));
        await page.screenshot({ path: path.join(out, `${c.slug}-${theme}.jpg`), type: 'jpeg', quality: 85, clip });
      }
    } catch (error) {
      failures++;
      console.error(`${c.slug} (${theme}): ${error.message.split('\n')[0]}`);
    }
    if (panicked) {
      console.error(`${c.slug} (${theme}) panicked: ${panics.at(-1)}`);
      // A panic leaves the module unusable: start over for the rest.
      await open(c.slug, dark).catch(() => {});
    } else {
      console.log(`${c.slug} (${theme})`);
    }
  }
}

await browser.close();
server.close();
if (panics.length) {
  console.error(`\n${panics.length} panic(s) in the browser:\n${panics.join('\n')}`);
  process.exit(1);
}
if (failures) console.error(`\n${failures} thumbnail(s) could not be taken.`);
