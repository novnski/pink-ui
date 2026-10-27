# Icon button

`pink-icon-button` is PINK's titlebar icon button (the sidebar toggle, Back
and Forward) as a single-GPUI package, the first bridge primitive of UI
rethink decision 54. Like `components/list-row` and `components/tool-activity`
it depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place (`src/components/icon_button.rs`).
PINK pins the package and patches `gpui` to its vendored runtime, so there is
one GPUI type graph and no copied source.

## What it draws

- `icon_button(id, glyph, &IconButtonStyle)`: the host's `glyph` centred in a
  `size` square with `radius` corners and the host's `wash`. An enabled
  button shows the pointing cursor.
- `scaled_control(step, scale)` and `scaled_icon(step, scale)`: control and
  glyph sizes grown with the Interface size (`scale` = size / 16). Controls
  stay between the 24 pt minimum hit and 32 pt; glyphs between 12 and 18 pt.
  The titlebar uses `scaled_control(CONTROL_M, scale)` (24, 28 and 32 pt at
  sizes 12, 16 and 20) and `scaled_icon(ICON_M, scale)`.

The host adds what the package can't express with crates.io GPUI: the role and
accessible name, Tab focus and its ring, the tooltip, the click and keyboard
activation, and occluding a titlebar drag strip.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-icon-button = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-icon-button" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- Geometry: `tests/icon_button.rs` in the pink-ui crate lays the titlebar
  cluster out at Interface sizes 12, 16 and 20 and checks the sizes, the
  centred glyph and the 24 pt minimum.
