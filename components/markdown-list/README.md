# Markdown List

`pink-markdown-list` is PINK's Markdown list markers as a single-GPUI
package. Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/markdown_list.rs`), and the
Markdown Message story draws its lists through it. PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

## What it draws

- `list()` stacks items `ITEM_GAP` apart; `item(marker, body)` puts the
  marker column `MARKER_GAP` left of `body()`, the item's blocks.
- `disc(color, line_height)`: a real `DISC` centred on the first line, for
  unordered items. `number(label, color, text_size, line_height)`: a tinted
  number for ordered items.
- `task_box(&TaskStyle, checked, toggleable, check)`: a task item's
  checkbox in place of its `[ ]` / `[x]` text. `TASK_BOX` with a 1 px edge,
  open in `edge`, done as a `plate` holding the host's `check` icon at
  `TASK_CHECK` in `check`. A box the reader cannot toggle (an assistant
  reply) rests at `READ_ONLY_OPACITY`. Place it in `marker_slot(line_height)`.
- `ui_rems(design)`: sizes are rems (pt at the 16 px default), so lists follow
  the host's Interface size.

The host passes resolved colours, its body text metrics and its check icon.
It adds the id, the CheckBox role, the toggled state, the item's text as the
accessible name, and, where the source can be rewritten, the tab stop, the
focus ring, the cursor and the click. It keeps the `[ ]` marker in the text
it copies and searches.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-markdown-list = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-markdown-list" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/markdown_list.rs` lays out task, done read-only and
  bullet items at Interface sizes 12, 16 and 20.
