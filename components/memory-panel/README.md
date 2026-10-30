# Memory panel

`pink-memory-panel` draws the pink-ui gallery's Memory Panel as a
single-GPUI package. Like the other packages under `components/` it depends
on crates.io `gpui = "=0.2.2"` in its own workspace. The gallery compiles
this same `src/lib.rs` in place (`src/components/memory_panel.rs`), and PINK
pins the package for the Memory card in Providers settings, patching `gpui`
to its vendored runtime.

## What it draws

- `row(&RowStyle)`: a memory, `ROW_H` tall, padded `ROW_PAD_X` by
  `ROW_PAD_Y`, `ROW_RADIUS` corners, on the host's hover-blended wash.
- `title_line(fade_to, title, corner)`: the memory on one 13/18 line that
  fades out over its last `TITLE_FADE` instead of ending in an ellipsis,
  then the host's `corner(opacity)` of `action(icon, tone)` tiles
  (`ACTION` square, `ACTION_GLYPH` glyph).
- `meta_line(color, glyph, text)`: the quiet 11/14 line under the title.
- `heading(label, count, label_text, count, gap)`: a section heading,
  `HEADING_H` tall, `SECTION_GAP` above every section after the first.
- `strip(&StripStyle, icon, text)`: the note strip that folds open under
  the header (paused, applies at next launch, an error).
- `list(border)`, `footer(border)` and `summary(color, text)`: the list
  frame below its rule and the footer bar with its summary.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems, so
  every piece grows with the host's Interface size.

## What the host adds

Ids, roles and labels, hover listeners and the hover fade, tooltips,
focus rings, clicks, text inputs, motion and the data. The package keeps no
state.
