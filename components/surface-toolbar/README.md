# Surface toolbar

`pink-surface-toolbar` is PINK's surface toolbar chrome (every sidebar
surface's control row, the Files search toolbar, the file editor header and
History's toolbar controls) as a single-GPUI package (UI rethink decision
54). Like the other
packages under `components/` it depends on crates.io `gpui = "=0.2.2"` in its
own workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/surface_toolbar.rs`); File Tree draws its toolbar through
it, Diff Viewer its header toggles' glyphs and Commit History its toolbar's
search, Fetch all and view toggle. PINK pins the package and
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
- `text_control(id, style)`: a `CONTROL_SIZE` text control (the file
  editor's Preview and save status) in `TEXT_CONTROL_TEXT` type, its glyph
  `TEXT_CONTROL_GAP` before the label, at half strength while disabled.
- `labeled_control(id, style)` and `control_label(text, color)`: a glyph and
  one-line label on a `CONTROL_SIZE` plate (History's Fetch all).
- `compact_field(id, width, fill)`, `compact_field_status`,
  `compact_field_glyph`, `compact_field_input`, `compact_close` and
  `compact_close_glyph`: History's search field, `width` wide but never under
  `COMPACT_FIELD_MIN`, with its status slot, query line and a
  `COMPACT_CLOSE` close target. The host animates the width as it grows out
  of the magnifier and folds back.
- `editor_header()`, `file_artwork(img)`, `breadcrumb(id)`,
  `crumb_separator(svg, color)` and `crumb(text, leaf, style)`: the file
  editor header with the file's `ARTWORK` and its path; the leaf keeps its
  width in `CRUMB_LEAF_TEXT` medium type while parent folders shrink first,
  down to `CRUMB_MIN`, in `CRUMB_TEXT`.
- `ui_rems(design)`: a design size in rems; the field, text and artwork grow
  with the host's Interface size, the row, insets, controls and corners stay
  fixed.

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
- In the gallery: `tests/surface_toolbar.rs` lays the toolbar, the editor
  header and History's controls out at Interface sizes 12, 16 and 20.
