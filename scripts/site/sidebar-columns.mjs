// Audits the Sidebar story's shared columns, the way Pink's
// scripts/qa-sidebar-replay.py column audit does (novnski/pink#185): every
// element draws its first pixel on the title column and its last on the time
// column, within 1.5 pt, in both themes at Interface sizes 12 and 20.
//
//   node scripts/site/sidebar-columns.mjs site/dist [out-dir]
//
// Needs Playwright (PLAYWRIGHT=/path/to/playwright to point at a copy, and
// CHROMIUM=/path/to/chrome for a specific browser build). Renders the player
// at 1x, then, for each sidebar in the story (All projects and a single
// project, side by side), splits its drawn pixels into horizontal bands (a
// row title, a meta line, a header, the footer) and takes each band's first
// and last drawn column against the sidebar's own surface. The columns are
// where the bands near the 16 pt insets actually start and end (text has its
// own side bearing), and they must sit within 1.5 pt of the 16 pt insets.
// Bands that start or end more than 24 pt in (a meta line that fades before
// the time) are not on a column and are skipped, as in Pink; anything nearer
// is an indent and fails.
// The Activity bell's unread dot may overhang the time column like a badge.
//
// Writes one PNG and one JSON per case, result.json and a SHA-256 manifest
// to out-dir. Exit status 0 means every case passed.

import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';

const { chromium } = await import(process.env.PLAYWRIGHT ?? 'playwright');
const root = path.resolve(process.argv[2] ?? 'site/dist');
const out = path.resolve(process.argv[3] ?? 'shots/sidebar-columns');
fs.mkdirSync(out, { recursive: true });
const VIEW = { width: 900, height: 800 };
const TOLERANCE = 1.5;
const INSET = 16;
// Bands that start (or end) within this of a column count as on it: wide
// enough to catch an indent, short of the 40 pt navigation label column.
const NEAR = 24;

const types = {
  '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json',
  '.wasm': 'application/wasm', '.svg': 'image/svg+xml', '.ttf': 'font/ttf', '.jpg': 'image/jpeg',
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

// Runs in the page: decodes the screenshot and measures it.
function audit({ png, insets, near, tolerance }) {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement('canvas');
      canvas.width = img.width;
      canvas.height = img.height;
      const g = canvas.getContext('2d');
      g.drawImage(img, 0, 0);
      const { data, width, height } = g.getImageData(0, 0, img.width, img.height);
      const at = (x, y) => {
        const i = (y * width + x) * 4;
        return [data[i], data[i + 1], data[i + 2]];
      };
      const diff = (a, b) => Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]) + Math.abs(a[2] - b[2]);
      // Accent-tinted ink (the unread dot and its antialiased edge): blue
      // leads and the pixel is clearly chromatic. Text and glyphs are grey.
      const accentish = ([r, g, b]) => b - Math.max(r, g) > 24 && b - Math.min(r, g) > 48;
      const page = at(2, 2);
      // Sidebars: runs of the surface across the middle row.
      const mid = Math.round(height / 2);
      const runs = [];
      let start = null;
      for (let x = 0; x < width; x++) {
        const on = diff(at(x, mid), page) > 6;
        if (on && start === null) start = x;
        if (!on && start !== null) {
          if (x - start > 150) runs.push([start, x]);
          start = null;
        }
      }
      const sidebars = runs.map(([left, right]) => {
        const probe = left + 4;
        let top = mid;
        while (top > 0 && diff(at(probe, top - 1), page) > 6) top--;
        let bottom = mid;
        while (bottom < height - 1 && diff(at(probe, bottom + 1), page) > 6) bottom++;
        // Past the rounded corners.
        top += 12;
        bottom -= 12;
        const surface = at(left + 4, top);
        const bands = [];
        let band = null;
        for (let y = top; y <= bottom; y++) {
          // The row's own fill (a selected row) or the surface.
          const back = at(left + 12, y);
          let first = null;
          let last = null;
          let lastPlain = null;
          for (let x = left + 9; x < right - 9; x++) {
            const c = at(x, y);
            if (diff(c, back) <= 24) continue;
            if (first === null) first = x;
            last = x;
            if (!accentish(c)) lastPlain = x;
          }
          if (first === null) {
            if (band) bands.push(band);
            band = null;
            continue;
          }
          if (!band) band = { top: y, bottom: y, left: first, right: last + 1, rightPlain: null };
          band.bottom = y;
          band.left = Math.min(band.left, first);
          band.right = Math.max(band.right, last + 1);
          if (lastPlain !== null) {
            band.rightPlain = Math.max(band.rightPlain ?? 0, lastPlain + 1);
          }
        }
        if (band) bands.push(band);
        const nominalLeft = left + insets;
        const nominalRight = right - insets;
        // A band's trailing edge without accent ink beyond the column (the
        // bell's unread dot overhangs like a badge).
        const trail = (b) => (b.right > nominalRight + tolerance && b.rightPlain ? b.rightPlain : b.right);
        const median = (xs) => {
          const s = [...xs].sort((a, b) => a - b);
          return s.length ? s[Math.floor(s.length / 2)] : null;
        };
        // A band that starts inside the gutter is a row's own wash (the
        // selected row's rounded edges), not an element: Pink measures each
        // element against its own background.
        const elements = bands.filter((b) => b.bottom - b.top >= 3 && b.left >= left + insets - 4);
        const leading = elements.filter((b) => b.left < nominalLeft + near);
        const trailing = elements.filter((b) => trail(b) > nominalRight - near);
        const leftColumn = median(leading.map((b) => b.left)) ?? nominalLeft;
        const rightColumn = median(trailing.map(trail)) ?? nominalRight;
        const row = (b) => ({ y: b.top, h: b.bottom - b.top + 1, left: b.left - left, right: trail(b) - left });
        return {
          frame: { left, right, top: top - 12, bottom: bottom + 12 },
          nominal: { left: insets, right: right - left - insets },
          columns: { left: leftColumn - left, right: rightColumn - left },
          columnsOnInsets:
            Math.abs(leftColumn - nominalLeft) <= tolerance && Math.abs(rightColumn - nominalRight) <= tolerance,
          bands: elements.map(row),
          offLeft: leading.filter((b) => Math.abs(b.left - leftColumn) > tolerance).map(row),
          offRight: trailing.filter((b) => Math.abs(trail(b) - rightColumn) > tolerance).map(row),
        };
      });
      resolve({ width, height, sidebars });
    };
    img.src = `data:image/png;base64,${png}`;
  });
}

const browser = await chromium.launch({
  executablePath: process.env.CHROMIUM || undefined,
  args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'],
});
const results = [];
let failed = false;
for (const theme of ['light', 'dark']) {
  for (const ui of [12, 20]) {
    const page = await browser.newPage({ viewport: VIEW, deviceScaleFactor: 1 });
    await page.goto(`${base}/play.html?c=sidebar&theme=${theme}&ui=${ui}`);
    await page.waitForFunction(() => window.__pinkuiReady, null, { timeout: 120000 });
    await page.waitForTimeout(Number(process.env.WAIT ?? 2500));
    const name = `sidebar-${theme}-${ui}`;
    const png = await page.screenshot({ path: path.join(out, `${name}.png`) });
    const measured = await page.evaluate(audit, {
      png: png.toString('base64'),
      insets: INSET,
      near: NEAR,
      tolerance: TOLERANCE,
    });
    const ok =
      measured.sidebars.length === 2 &&
      measured.sidebars.every((s) => s.columnsOnInsets && !s.offLeft.length && !s.offRight.length);
    failed ||= !ok;
    fs.writeFileSync(path.join(out, `${name}.json`), JSON.stringify(measured, null, 2));
    results.push({
      case: name,
      ok,
      sidebars: measured.sidebars.map((s) => ({
        columns: s.columns,
        nominal: s.nominal,
        bands: s.bands.length,
        offLeft: s.offLeft,
        offRight: s.offRight,
      })),
    });
    console.log(
      `${ok ? 'ok  ' : 'FAIL'} ${name}: ` +
        measured.sidebars
          .map((s) => `columns ${s.columns.left}/${s.columns.right} (insets ${s.nominal.left}/${s.nominal.right}), ` +
            `${s.bands.length} bands, ${s.offLeft.length + s.offRight.length} off`)
          .join('; '),
    );
    await page.close();
  }
}
await browser.close();
server.close();
fs.writeFileSync(path.join(out, 'result.json'), JSON.stringify({ ok: !failed, tolerance: TOLERANCE, results }, null, 2));
const manifest = fs
  .readdirSync(out)
  .filter((f) => f !== 'SHA256SUMS')
  .sort()
  .map((f) => `${crypto.createHash('sha256').update(fs.readFileSync(path.join(out, f))).digest('hex')}  ${f}`)
  .join('\n');
fs.writeFileSync(path.join(out, 'SHA256SUMS'), `${manifest}\n`);
process.exit(failed ? 1 : 0);
