# Diff header

`pink-diff-header` is PINK's Changes pane chrome around the diff rows as a
single-GPUI package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own workspace.
The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/diff_header.rs`); Diff Viewer draws its file headers,
sticky header, summary strip, pane header controls and menu rows through it.
PINK pins the package and patches `gpui` to its vendored runtime, so there is
one GPUI type graph and no copied source.

## What it draws

- `file_header(id, placement, style)`: a file's header, `FILE_HEADER_HEIGHT`
  tall in fixed pixels (the virtualized list measures it), as a list row
  (`Row { first }`, a rule over every header after the first) or the sticky
  copy (`Sticky`, a rule underneath).
- `disclosure(svg, open, color)`, `artwork(img)`, `path(text, mono, color)`,
  `binary_tag(text, color)` and `count(text, mono, color)`: the header's
  turning chevron, file-type artwork, mono path, BIN and `+N` / `-N`.
- `summary_strip(rule)`, `summary_label`, `summary_count`, `partial_badge`
  and `spacer`: the strip over the list.
- `header_row()`, `scope_trigger`, `history_title`, `history_count`,
  `trailing`, `history_trailing`, `git_chip`, `commit_sha` and
  `commit_subject`: the pane header's controls.
- `ref_selector(branch, arrow, trigger, style)` and `ref_trigger(...)`:
  `{branch} -> {base}`, each side shrinking by `ref_shrink` (its length
  squared) so the longer name gives way first.
- `scope_list`, `scope_row_label`, `ref_list`, `ref_row_label` and
  `ref_list_empty`: the content of the scope and base menus. Their cards and
  rows are `pink-menu-card`; the header's icon toggles are `pink-icon-button`.

The host adds the rest: glyph and artwork assets, every resolved colour (each
fill blended over its hover fade, the sticky copy's opaque plate), the ids'
listeners, roles, accessible names, focus, tooltips, occlusion, the fold and
its tween, the sticky header's push offset and frost, the menus and their
placement, and the diff itself.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-diff-header = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-diff-header" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/diff_header.rs` lays the headers, strip and ref
  selector out at Interface sizes 12, 16 and 20.
