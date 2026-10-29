# Commit row

`pink-commit-row` is PINK's commit history row chrome (the History surface) as
a single-GPUI package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own workspace.
The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/commit_row.rs`); Commit History draws its rows through it.
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

The host adds the rest: the graph cell, glyph and avatar assets, every
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
- In the gallery: `tests/commit_row.rs` lays a row, its badge and cells out
  at Interface sizes 12, 16 and 20.
