# File row

`pink-file-row` is PINK's Files pane rows as a single-GPUI package (UI
rethink decision 54): the tree entry, the name search result, a lazily
listed folder's status rows and a text search's file and matching-line
rows. Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/file_row.rs`); File Tree draws
its rows through it. PINK pins the package and patches `gpui` to its
vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `tree_entry(base, depth, ignored)`: shapes the host's base as a tree
  entry, `ROW_HEIGHT` tall, an inset pill (`PILL_INSET` from each side,
  `PILL_RADIUS` corners) indented `INDENT` a level from `LEAD`, dimmed to
  `IGNORED_OPACITY` for an ignored entry.
- `search_entry(base, depth)`: the same row at full width for a name
  search result.
- `disclosure(chevron)`: the `SLOT` square before the artwork, with a
  folder's `tree_chevron(glyph, expanded, color)` (turned a quarter while
  open) or a result's `search_chevron(glyph, color)`, or empty.
- `artwork(art)` and `name(text, color, font)`: the file-type artwork and
  the truncated name.
- `status_row(base, depth, kind)` with `status_label`, `error_message` and
  `retry_label`: Loading..., Empty folder, a failed listing with its Retry,
  and Load more..., one level deeper than the folder's entries.
- `text_row(base, kind, font)` with `text_file_name`, `text_file_folder`,
  `match_count`, `line_number` and `match_line`: a text search's file row
  and matching-line row.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. The
  slots, glyphs, text and gaps grow with the host's Interface size; the row
  height, indents and pill inset stay fixed, because the host's virtualized
  lists measure rows by `ROW_HEIGHT`.

The host adds the rest: each row's base with its selected fill or eased
hover wash, the glyph and artwork assets, every resolved colour, the font
families, the highlighted match text, the ids, roles, accessible names,
selection, the click, drag and keyboard handling and the lists.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-file-row = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-file-row" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/file_row.rs` lays out a tree entry, a search
  result, a status row and a text match at Interface sizes 12, 16 and 20.
