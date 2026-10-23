// Prints the component catalog (JSON) from the built player, so the site
// lists exactly what the Rust registry holds. No browser needed: `catalog()`
// is plain Rust.
//
//   node scripts/site/catalog.mjs site/dist/pkg

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const pkg = path.resolve(process.argv[2] ?? 'site/dist/pkg');
const glue = await import(pathToFileURL(path.join(pkg, 'pink_ui_web.js')).href);
glue.initSync({ module: fs.readFileSync(path.join(pkg, 'pink_ui_web_bg.wasm')) });
process.stdout.write(glue.catalog());
