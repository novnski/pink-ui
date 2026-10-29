# Find bar

`pink-find-bar` is PINK's floating conversation Find bar chrome as a
single-GPUI package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/find_bar.rs`, module `chrome`); Find Bar draws its plate
through it. PINK pins the package and patches `gpui` to its vendored
runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `anchor(top)`: where the bar floats, `top` below the conversation's top
  edge and `BAR_RIGHT` from its right edge.
- `plate(plate, fill)`: shapes the host's popover card as the bar,
  `BAR_WIDTH` wide, filled opaque, one row `BAR_GAP` apart inside
  `BAR_PAD_X` / `BAR_PAD_Y`.
- `glyph(svg, color)`, `field(input)` and `status(text, color)`: the
  `GLYPH` magnifier, the query field taking what is left, and the
  `STATUS_TEXT` status ("3 of 7", "No results", "Searching...").
- `BAR_HEIGHT`: the room a revealed match keeps clear under the bar.
- `ui_rems(design)`: a design size in rems; the width, glyph and status
  grow with the host's Interface size, the insets stay fixed.

The host adds the rest: the popover card itself, the query field and its
text metrics, the Match case, Previous, Next and Close buttons, every
resolved colour, the ids, key context, occlusion, the Enter, Shift+Enter
and Escape handling, focus and the search.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-find-bar = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-find-bar" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/find_bar.rs` floats the bar at Interface sizes 12,
  16 and 20.
