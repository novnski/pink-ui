# Context window

`pink-context-window` is the map of a model's context window from the pink-ui
Context Window component, as a single-GPUI package (novnski/pink#257). Like
the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/context_window.rs`, module
`package`), and PINK pins it for its Context menu, so there is one GPUI type
graph and no copied source.

## What it draws

- `cells(capacity, parts, reserve)`: lays a window out over `CELLS` (100)
  cells, one per percent, column by column: each part in order (a part with
  items gives each item its own run, then its tokens beyond them), free
  room, then the reserve's hollow cells at the end. The occupied cells
  always total the parts' share of the window, rounded as the host prints
  its percentage (ties to even), so the map agrees with the ring; the runs
  share them by largest remainder, and a run with any tokens keeps at least
  one cell. Usage that runs into the reserve stays occupied: the reserve is
  hollow only over the cells usage hasn't reached (novnski/pink#277), and an
  overfull window fills every cell. The auto-compaction rule still marks
  where compaction kicks in.
- `cell(id, index, CellPaint)` and `field(id, cells, marker)`: the field of
  `CELL` cells `CELL_GAP` apart (`FIELD_W` x `FIELD_H`), each filled or, for
  the reserve, edged, at the host's opacity, and the auto-compaction rule in
  the gap before `marker_column(fraction)`.
- `part_row(...)`, `item_row(...)`, `swatch(...)`, `token_count(...)` and
  `breakdown(rows)`: the breakdown, a `ROW_H` row per part (swatch, label,
  meta that truncates first, count) and an `ITEM_H` row per item (its name
  in the host's mono family on the labels' column, its count, and an
  optional action that replaces the count under the item's `item_group`
  hover). `open` folds a removed item away.
- `note(text, colour)` and `section()`: the caption and the card section
  (`PAD`, `GAP`) the three sit in. `CARD_W` is a card that hugs the field.
- `ui_rems(design)`: every size is in rems, so the map grows with the
  Interface size.

The host decides what fills the window and resolves every colour: the
occupancy tone at each part's strength, unlit cells dimmed, the sweep and
the compaction wave mixed in, and washes blended over their hover fade. It
adds the hover listeners that light a part's cells, item actions with their
role, label and focus, tabular figures (crates.io GPUI has no font
features) and the animation clock.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-context-window = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-context-window" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- Fill and geometry: `tests/context_window.rs` in the pink-ui crate checks
  the fill order, items, slivers and overflow, that filled cells equal the
  rounded share at 25%, 80% and 95% (past where the reserve begins), and the
  field, cells, rule, rows and folded items at Interface sizes 12, 16 and 20.
