# Composer disc

`pink-composer-disc` is PINK's composer Send, Queue, Steer, Stop and Attach
buttons as a single-GPUI package (UI rethink decision 54). Like the other
packages under `components/` it depends on crates.io `gpui = "=0.2.2"` in its
own workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/composer_disc.rs`). PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `disc(id, glyph, &DiscStyle)`: a round disc `size` across with the host's
  `glyph` centred on its `fill`. `glyph` sets the text colour a glyph
  inherits. An `actionable` disc shows the pointing cursor, and with
  `press_dim` it dims to `PRESSED_OPACITY` while pressed (Send and Stop do,
  the quiet Attach doesn't). A recessed disc takes neither.
- `stop_glyph(color)`: the stop square, `STOP_GLYPH` with `STOP_RADIUS`
  corners.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems, so a
  disc and its glyph grow with the host's Interface size. Arrow glyphs are
  `GLYPH` across.

The host adds what the package can't express with crates.io GPUI: the role and
accessible name, Tab focus and its ring, the tooltip, the glyph's cross-fade
between Send, Queue, Steer and Stop, and the click and keyboard behaviour. It
also resolves the fill (solid, recessed or quiet, blended over its hover fade)
and builds arrow glyphs from its own assets.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-composer-disc = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-composer-disc" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- Geometry: `tests/composer_disc.rs` in the pink-ui crate lays a Send and a
  Stop disc out in rems at Interface sizes 12, 16 and 20 and checks the size,
  the round shape's square bounds and the centred glyphs.
