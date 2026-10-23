#!/usr/bin/env bash
# Builds the showcase site into site/dist: the components compiled to
# WebAssembly, the catalog, the component sources, fonts and pages.
#
#   scripts/build-site.sh                   # what GitHub Pages serves
#   PROFILE=release scripts/build-site.sh   # quicker to build, larger
#
# Needs the wasm32-unknown-unknown target, wasm-bindgen-cli at the version in
# Cargo.lock and Node. The card thumbnails are committed in site/thumbs; see
# scripts/site/thumbnails.mjs to refresh them. There is no wasm-opt pass: on
# this module it trims the raw size but grows the gzipped download.
set -euo pipefail

cd "$(dirname "$0")/.."
profile=${PROFILE:-web}
repo=${SITE_REPO:-https://github.com/novnski/pink-ui}
target_dir=${CARGO_TARGET_DIR:-target}
out=site/dist

want=$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/^version = "\(.*\)"$/\1/p')
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [ "$want" != "$have" ]; then
  echo "build-site: need wasm-bindgen-cli $want (found ${have:-none}):" >&2
  echo "  cargo install wasm-bindgen-cli --version $want" >&2
  exit 1
fi

cargo build -p pink-ui-web --lib --target wasm32-unknown-unknown --profile "$profile"
[ "$profile" = dev ] && profile_dir=debug || profile_dir=$profile

rm -rf "$out"
mkdir -p "$out/pkg" "$out/fonts" "$out/src/components" "$out/thumbs"
wasm-bindgen --target web --no-typescript --out-dir "$out/pkg" \
  "$target_dir/wasm32-unknown-unknown/$profile_dir/pink_ui_web.wasm"

node scripts/site/catalog.mjs "$out/pkg" > "$out/catalog.json"
cp src/components/*.rs "$out/src/components/"
cp assets/fonts/Geist*.ttf assets/fonts/Geist-OFL.txt "$out/fonts/"
cp site/thumbs/*.jpg "$out/thumbs/"
build=$(sha256sum "$out/pkg/pink_ui_web_bg.wasm" | cut -c1-12)
node scripts/site/pages.mjs site "$out" "$build" "$repo"
touch "$out/.nojekyll"

wasm_mb=$(du -m "$out/pkg/pink_ui_web_bg.wasm" | cut -f1)
gz_mb=$(gzip -9 -c "$out/pkg/pink_ui_web_bg.wasm" | wc -c | awk '{printf "%.1f", $1 / 1048576}')
echo "build-site: $out ready (module ${wasm_mb} MB, ${gz_mb} MB gzipped, build $build)"
