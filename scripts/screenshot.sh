#!/usr/bin/env bash
# Renders one component's demo stage headlessly and saves a PNG.
#
#   scripts/screenshot.sh <slug> [light|dark] [out.png] [WxH] [clean|classic]
#
# Needs Xvfb (xvfb-run), a window manager (openbox), xdotool and ImageMagick.
# The default size matches the website's preview stage for side-by-side checks.
set -euo pipefail

slug=${1:?usage: screenshot.sh <slug> [light|dark] [out.png] [WxH]}
theme=${2:-light}
out=${3:-shots/$slug.gpui.$theme.png}
size=${4:-894x520}
look=${5:-clean}
wait_secs=${WAIT:-6}

cd "$(dirname "$0")/.."
target_dir=${CARGO_TARGET_DIR:-target}
mkdir -p "$(dirname "$out")"
out=$(realpath "$out")

# Several checkouts may share one CARGO_TARGET_DIR; build and take a private
# copy of the binary under a lock so nobody runs another checkout's gallery.
# Cargo fingerprints this crate the same way in every checkout, so a newer build
# elsewhere looks "fresh" here: with a shared target dir, bump src/lib.rs first
# to make sure the binary is built from this checkout.
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
bin="$tmp/gallery"
mkdir -p "$target_dir"
flock "$target_dir/.gallery-build.lock" bash -c \
  '[ -n "$3" ] && touch src/lib.rs; cargo build --quiet --bin gallery && cp "$1/debug/gallery" "$2"' \
  _ "$target_dir" "$bin" "${CARGO_TARGET_DIR:-}"

xvfb-run -a -s "-screen 0 1400x1100x24" bash -c '
  openbox >/dev/null 2>&1 &
  sleep 0.5
  "$0" "$1" --theme "$2" --look "$6" --stage-only --size "$3" >/dev/null 2>&1 &
  pid=$!
  sleep "$5"
  wid=$(xdotool search --pid "$pid" | tail -1)
  import -window "$wid" "$4"
  kill "$pid"
' "$bin" "$slug" "$theme" "$size" "$out" "$wait_secs" "$look"
echo "$out"
