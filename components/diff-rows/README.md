# Diff rows

`pink-diff-rows` is PINK's diff row chrome as a single-GPUI package (UI
rethink decision 54): the notice under a file header, the `@@` band over a
hunk, the note a line can carry and the columns around a line's code,
unified or split. Like the other packages under `components/` it depends on
crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/diff_rows.rs`);
Diff Viewer and Inline Diff draw their rows through it. PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI
type graph and no copied source.

## What it draws

- `notice_row(text, &DiffStyle)`: `NOTICE_HEIGHT` tall, `NOTICE_PAD_X` in,
  `NOTICE_TEXT` in the faint tone ("New file", "Binary file").
- `hunk_header_row(header, &DiffStyle)`: the `HUNK_HEADER_HEIGHT` band on
  the hunk fill, `HUNK_PAD_X` in, mono at the meta size.
- `meta_line_row(text, pad_left, &DiffStyle)`: an italic note one line
  tall, indented past the columns (`unified_meta_pad`, `split_meta_pad`).
- `unified_row(wash, wrapped, &DiffStyle)` with `accent_bar`, `gutter` and
  `marker`: one line (at least, while wrapping) on the row's wash, the
  `ACCENT_BAR_WIDTH` bar column, line-number gutters the host fits to the
  file and the `MARKER_WIDTH` marker column.
- `split_half(wash)`, `split_filler(&DiffStyle)` and
  `split_row(left, right, wrapped, &DiffStyle)`: a split row's halves
  (`SPLIT_MARKER_WIDTH` marker), the flat empty half and the
  `SPLIT_DIVIDER_WIDTH` hairline between them.
- `ui_rems(design)`: a design size in rems. Only the notice follows the
  Interface size; the rest follows the code font sizes in `DiffStyle`.

The host adds the rest: the code itself (shaped, syntax-highlighted,
word-washed, clipped, wrapped or scrolled sideways), every resolved colour,
the mono family and the code sizes, the comment adder, hover tracking,
folding, the virtualized list and the sticky file header.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-diff-rows = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-diff-rows" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/diff_rows.rs` lays out a notice, a band, a unified
  line, a note and a split row at Interface sizes 12, 16 and 20.
