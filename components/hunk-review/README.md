# Hunk Review

`pink-hunk-review` is PINK's per-hunk review of an agent's file edit as a
single-GPUI package: the band under each changed block of the transcript's
inline diff (novnski/pink#310) with Keep and Drop, and the review model
behind it. Like the other packages under `components/` it depends on
crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place for its Inline Diff story
(`src/components/inline_diff.rs`, module `review`). PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

## What it draws

- `band(&BandStyle, lines)`: the `height(lines)` strip on the host's wash,
  holding the `bar`, the `content` and the `actions` in a row.
- `bar(&BandStyle, focus)`: the 3 pt leading bar, firming from `BAR_REST` to
  `BAR_ACTIVE` while the block holds keyboard focus.
- `content()`, `header()`, `location_label(&style, text)` and
  `chip(ink, plate, word)`: the `PAD_X` by `PAD_Y` content, the 22 pt header
  with the 11 pt `path:line` and the state chip.
- `headline(&style, text)` and `detail(&style, text)`: the 12/18 status line
  and the refusal's reason line, each one truncated line so the band's
  height stays analytic.
- `actions()`: the column for the host's `Sm` Drop and Keep buttons, centred
  on the header and the first status line, `ACTIONS_INSET` from the edge.
- `legend()`, `legend_item(&KeyStyle, keys, verb)` and `LEGEND`: the
  "Y Keep, N Drop, J K Move" key legend the focused block shows.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Every
  size grows with the host's Interface size; the bar and radii stay fixed.

## The review model

`BlockState` mirrors the engine's `pending`, `kept` and `dropped`. `Status`
adds what the host knows beyond the record: `Reading`, `Deciding` (a
decision in flight) and `Refused` with the engine's reason. The recorded
state always stays the engine's, so a drop never reads as done before the
engine confirms it. `headline()`, `detail()`, `chip()` and `tone()` word it;
`offers(decision)` says which buttons are live (none while reading or in
flight, never the state the block already has; Keep on a dropped block is the
undo). `accessible_name` and `action_name` name each hunk by path and line,
`progress` counts "2 of 3 reviewed", `refusal_reason` strips the engine's
`edit_review_*:` code, `key_command` maps Y, N, J and K (and the arrows) and
leaves modified keys to the app, and `step` moves between blocks without
wrapping.

The host adds the rest: the colours, the fonts, the ids, the roles and
accessible names, the focus handles and rings, the buttons, the tooltips and
the listeners.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-hunk-review = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-hunk-review" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/hunk_review.rs` lays a band out at Interface sizes
  12, 16 and 20 and checks the review model.
