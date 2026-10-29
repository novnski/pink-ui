# Scroll rail

`pink-scroll-rail` is PINK's floating scroll rail as a single-GPUI package
(UI rethink decision 54). Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/scroll_rail.rs`). PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `rail(faint, fade, active, thumb_top, thumb_height)`: an invisible
  `HIT_WIDTH` strip on its parent's right edge, faded to `fade`, holding the
  thumb `TRACK_INSET + thumb_top` from the top and `THUMB_INSET` from the
  edge. The thumb is a capsule `THUMB_WIDTH` wide at `THUMB_ALPHA` of the
  host's faint tone, `ACTIVE_THUMB_WIDTH` at `ACTIVE_THUMB_ALPHA` while the
  rail is hovered or dragged, and `HOVER_THUMB_ALPHA` under the pointer.
  Widening happens inside the fixed strip, so rows never reflow.
- `horizontal_rail(faint, fade, active, thumb_left, thumb_length)`: the
  same rail along its parent's bottom edge, for a viewport that scrolls
  sideways (PINK's code fences). A full-width `HIT_WIDTH` strip holds the
  thumb `TRACK_INSET + thumb_left` from the leading edge and `THUMB_INSET`
  above the bottom, `THUMB_WIDTH` thick, `ACTIVE_THUMB_WIDTH` while active.
  The whole strip is the pointer target, so the thumb keeps the active
  paint under the pointer rather than brightening further.
- `thumb_width`, `thumb_color` and `MIN_THUMB`, for hosts that measure the
  thumb.

The host adds the rest: the thumb's measurements, when the rail shows and
how far it has faded, whether it is active, the rail's id, its hover,
press, drag and release listeners, and the scrolling itself.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-scroll-rail = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-scroll-rail" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/scroll_rail.rs` lays both rails out at Interface
  sizes 12, 16 and 20.
