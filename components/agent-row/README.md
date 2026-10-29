# Agent row

`pink-agent-row` is PINK's Agents list row as a single-GPUI package (UI
rethink decision 54). Like the other packages under `components/` it depends
on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/agent_row.rs`; the
Agent Tree story draws its rows with it). PINK pins the package and patches
`gpui` to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `row()`: the row, padded `PAD_X` by `PAD_Y`, `RADIUS` corners, its two
  lines `LINE_GAP` apart, no outlined box. Rows of a list sit `LIST_GAP`
  apart. A row is `ROW_HEIGHT` tall at the 16 px default (line heights are
  set, so an outline can size its slots and rails).
- `dot(color)`: the `MARK` status dot. While an agent runs the host passes
  its working spinner instead.
- `title_line(color, mark, title, status)`: the mark centred in its slot, the
  13 pt title, which truncates, and the host's `status_word(color, text)`
  (11 pt medium), which it may wrap in a swap transition. A host may add a
  trailing control (the gallery's fold tile).
- `meta_line(color, activity)` and `figures(color, text)`: line two, indented
  `META_INDENT` under the title: what the agent is doing, which gives way
  first, then its figures (model, tokens, tool uses, runtime) ending on the
  row's trailing edge. The figures truncate at `FIGURES_MAX` (half) of the
  line, so long figures never hide the activity.
- `agent_row(&RowStyle, mark, RowText)`: the whole row.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems.
  Padding, gaps, the mark and text grow with the host's Interface size; the
  corners stay fixed.

The host adds the rest: the resolved colours (the title's danger tone for a
failed agent), the mark, the copy, ids, roles and accessible names, focus,
hover and selection fills, an outline's indent and the actions.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-agent-row = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-agent-row" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/agent_row.rs` lays short, long and narrow rows out
  at Interface sizes 12, 16 and 20.
