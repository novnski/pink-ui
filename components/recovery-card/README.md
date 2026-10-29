# Recovery Card

`pink-recovery-card` is PINK's timeline notice as a single-GPUI package: the
card a run error, a turn recovery (novnski/pink#193) or an undelivered send
shows in the transcript, and the recovery model behind it. Like the other
packages under `components/` it depends on crates.io `gpui = "=0.2.2"` in
its own workspace. The pink-ui gallery compiles this same `src/lib.rs` in
place (`src/components/recovery_card.rs`). PINK pins the package and patches
`gpui` to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `card(&NoticeStyle)`: the `RADIUS` card tinted `tone@FILL_ALPHA` with a
  `tone@EDGE_ALPHA` edge, padded `PAD_X` by `PAD_Y`, the title row over the
  detail `GAP` apart.
- `title_row()`, `glyph_size()`, `title(&style, label)`: the `ROW_MIN_HEIGHT`
  row holding the `GLYPH` tone glyph, the 13 pt semibold title and the
  actions.
- `detail(&style)`: the 12/17 detail, indented `DETAIL_INSET` so it starts on
  the title's edge, held to `DETAIL_MAX_WIDTH`.
- `action(&ActionStyle)`: a notice action, `ACTION_HEIGHT` tall with
  `ACTION_RADIUS` corners and the host's edge and text.
- `copy_button()` and `run_error_frame()`: the run error's `COPY_SIZE` Copy
  error frame and its timeline margin.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Every
  size grows with the host's Interface size; the radii stay fixed.

## The recovery model

`Recovery` mirrors PINK's `RecoveryStatus` for the states that show a card.
`headline()` words it ("Retrying in 5s, attempt 2 of 3"), `tone()` and
`glyph()` pick warning or danger and a clock, refresh or danger glyph, and
`actions(observe_only, sign_in_expired)` lists Send now (Continue while
held), Open Providers and Cancel in title-row order with the primary marked.
`recovery_cause` picks the engine's detail, else the run's error while the
recovery is active. `spoken(title, detail)` is what the alert announces.

The host adds the rest: the colours, the glyphs, the ids, the roles and
accessible names, the focus rings, the hover fills, the clipboard write and
the listeners.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-recovery-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-recovery-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/recovery_card.rs` lays the notice out at Interface
  sizes 12, 16 and 20 and checks the recovery model.
