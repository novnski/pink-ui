# Trigger chip

`pink-trigger-chip` is PINK's composer controls-row chip (the model, access
and similar pickers) as a single-GPUI package (UI rethink decision 54). Like
the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this same
`src/lib.rs` in place (`src/components/trigger_chip.rs`). PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI type
graph and no copied source.

## What it draws

- `trigger_chip(id, TriggerChipContent, &TriggerChipStyle)`: a ghost pill
  `HEIGHT` tall with no border or caret. In order: the host's icon (with an
  optional corner dot, `attention`), the label (text that truncates, or the
  host's own skeleton), a badge and a muted suffix, on the host's wash.
  `icon_only` makes it a `HEIGHT` square with the icon alone. A labelled chip
  stops growing at `MAX_WIDTH` and truncates its label.
- The design sizes (`HEIGHT`, `PAD_X`, `GAP`, `ICON`, `BADGE`, `TEXT`, `DOT`,
  `MAX_WIDTH`) are in pt at the 16 px default. `Units::Rems` turns them into
  rems so the chip grows with the host's Interface size (PINK sets the
  window's rem to it); `Units::Pixels` keeps them fixed.

The host adds what the package can't express with crates.io GPUI: the role,
accessible name and description, Tab focus and its ring, the tooltip, and the
press, click and keyboard behaviour. It also builds the icon and badge
elements from its own assets, sized with `Units::len(ICON)` and
`Units::len(BADGE)`.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-trigger-chip = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-trigger-chip" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- Geometry: `tests/trigger_chip.rs` in the pink-ui crate lays chips out in
  rems at Interface sizes 12, 16 and 20 and checks the height, the square
  icon-only form and the width cap.
