# Surface toolbar

`pink-surface-toolbar` is PINK's surface toolbar chrome (the Files search
toolbar) as a single-GPUI package (UI rethink decision 54). Like the other
packages under `components/` it depends on crates.io `gpui = "=0.2.2"` in its
own workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/surface_toolbar.rs`); File Tree draws its toolbar through
it and Diff Viewer its header toggles' glyphs. PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and no
copied source.

## What it draws

- `toolbar()`: the control row, `HEIGHT` tall, its children `CONTROL_GAP`
  apart inside `EDGE_INSET`, with no band or border of its own.
- `field(fill)`: the rounded search field taking the row's free width,
  `FIELD_HEIGHT` tall with `FIELD_PAD_X`, `FIELD_GAP` and `FIELD_TEXT`.
- `field_glyph(svg, color)` and `field_input(input)`: the `FIELD_GLYPH`
  magnifier and the query's room.
- `control_glyph(svg, color)`: a toolbar control's `CONTROL_GLYPH` glyph.
- `CONTROL_SIZE` and `CONTROL_RADIUS`: the controls are `pink-icon-button`
  squares at this size and corner, so the package adds no second button.
- `ui_rems(design)`: a design size in rems; the field grows with the host's
  Interface size, the row, insets, controls and corners stay fixed.

The host adds the rest: the query field and its text metrics, the glyph
assets, every resolved colour (each control's wash blended over its hover
fade, or held while it is latched), the ids, roles, accessible names,
toggled states, focus rings, tooltips, hover listeners, occlusion, clicks and
the search.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-surface-toolbar = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-surface-toolbar" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/surface_toolbar.rs` lays the toolbar out at
  Interface sizes 12, 16 and 20.
