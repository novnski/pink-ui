# Place tab

`pink-place-tab` is PINK's right-pane place tab row as a single-GPUI package
(UI rethink decision 54). Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/place_tab.rs`). PINK pins the package and patches `gpui` to
its vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `bar()`: the `BAR_HEIGHT` (36) row, `BAR_PAD_X` inset, its parts
  `BAR_GAP` apart.
- `strip()`: the line the tabs sit in, `BAR_GAP` apart.
- `tab(&TabStyle)`: one tab, fixed at `TAB_WIDTH` (112) by `TAB_HEIGHT`
  (28) so the strip's drop index and slide offsets can step by `TAB_SLOT`;
  `PAD_LEFT`/`PAD_RIGHT`, `PART_GAP`, `TAB_RADIUS`. The selected fill is
  optional, so a resting tab leaves its background to the host's hover wash.
- `slot()`, `glyph_layer(group)`, `close_layer(group)`: the 20 pt leading
  slot whose glyph turns into the close control while the pointer is over
  the tab's hover group, with no width jump.
- `title(text, color)`: the 12.5 pt truncating title.
- `dirty_dot(color)`: the 6 pt unsaved dot.
- `drag_spacer()`: the room a dragged tab leaves behind.
- `home_tab(&HomeStyle)`: the Agents place's pinned home tab.
- `add_button(fill)`: the 28 pt add button; the fill is already blended
  over the host's hover fade.
- `edge_fade(Edge, surface)`: the painted fade over a scrolled strip's edge.
- Glyph sizes (`GLYPH`, `FILE_GLYPH`, `HOME_GLYPH`, `ADD_GLYPH`) and
  `ui_rems(design)`: labels grow with the host's Interface size; the row,
  tabs and controls stay fixed.

The host adds the rest: the glyphs, the spinner and favicon, the colours,
the ids, the scroller, the hover washes, the roles, accessible names and
focus rings, the tooltips, the click, middle-click, menu and drag
listeners, the slide animation and measurement.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-place-tab = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-place-tab" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/place_tab.rs` lays the row out at Interface sizes
  12, 16 and 20.
