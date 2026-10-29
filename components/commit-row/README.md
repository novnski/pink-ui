# Commit row

`pink-commit-row` is PINK's commit history row chrome (the History surface) as
a single-GPUI package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own workspace.
The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/commit_row.rs`); Commit History draws its rows, column
header and graph cells through it.
PINK pins the package and patches `gpui` to its vendored runtime, so there is
one GPUI type graph and no copied source.

## What it draws

- `row(id, wash)`: one commit, `ROW_HEIGHT` tall in fixed pixels (the list
  and the lane graph share the pitch).
- `subject_lane(opacity)`, `subject(text, color)`, `ref_area(available)`,
  `ref_badge(id, label, svg, style)` and `ref_overflow(id, text, color)`: the
  subject with its branch, remote and tag badges, capped at
  `REF_BADGE_MAX_WIDTH`, and the count of refs past the room.
- `cells()`, `cell`, `author_avatar_cell`, `author_name_cell`, `avatar`,
  `avatar_image`, `avatar_initial`, `date_cell`, `sha_cell` and `sha_pill`:
  the optional author, date and short-sha columns.
- `load_more_row()` and `load_more(id, label, svg, style)`: Load more,
  Retry or Loading... under the last commit.
- `column_header(id, group, style)`, `commit_header(text)` and
  `header_cell(id, width, min_width, centered)`: the `HEADER_HEIGHT` strip
  naming the columns, its optional headers sized like the row cells under
  them (inside `cells()`).
- `resize_handle(id, fill)`, `drop_line(after, color)`,
  `columns_button(id, group, open, wash)` and `columns_glyph(svg, color)`:
  the `RESIZE_HANDLE` divider on a header's leading edge, the line where a
  dragged header lands, and the checklist button that shows while the strip
  named `group` is pointed at or its menu is open.
- `graph_cell(id, group, width)`, `graph_node(x, radius, color)`,
  `head_ring(x, radius, edge, fill)`, `fold_control(id, group, x, collapsed,
  style)` and `fold_glyph(svg, color)`: a row's slice of the lane graph, in
  fixed pixels at the row's middle: the node, HEAD's ring
  `HEAD_RING_PADDING` outside it, and a branch tip's `FOLD_SIZE` fold
  control `FOLD_GAP` after it, shown while the cell is pointed at or while
  the branch is collapsed.

The host adds the rest: the lane graph's strokes and geometry (lane
positions, the node's radius under focus), glyph and avatar assets, every
resolved colour (the row's and the sha's washes blended over their hover
fades, the graph focus's dimming as an opacity), the families and text, the
container query that sizes the ref area, column widths and their
persistence, tooltips, the fold transition, copying a sha and opening a
commit.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-commit-row = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-commit-row" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/commit_row.rs` lays a row, its badge and cells, the
  column header and a graph cell out at Interface sizes 12, 16 and 20.
