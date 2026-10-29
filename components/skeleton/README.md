# Skeleton

`pink-skeleton` is PINK's pulsing skeletons as a single-GPUI package (UI
rethink decision 54). Like the other packages under `components/` it depends
on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/skeleton.rs`). PINK
pins the package and patches `gpui` to its vendored runtime, so there is one
GPUI type graph and no copied source.

## What it draws

- `rows(count, wash, phase, wave)`: the ghost rows a list or settings page
  shows while it loads, full-width `ROW_HEIGHT` bars `ROW_GAP` apart.
- `menu_rows(count, wash, phase, wave)`: a loading menu, pill bars at the
  `MENU_ROW_WIDTHS` ladder so they read as ghost labels.
- `bar(width, wash, phase, wave)`: one ghost label in a label's slot, the
  trigger chip's placeholder while its value resolves.
- `row_phase`, `opacity` and `STAGGER`: each row trails the one above it by
  `STAGGER` of the period and pulses between `OPACITY_REST` and
  `OPACITY_REST + OPACITY_SWING`.

Sizes are rems (`ui_rems`), so the rows grow with the host's Interface size;
corner radii stay fixed.

The host adds the rest: the wash, its pulse clock's `phase` (0..1, shared by
every skeleton on screen so they breathe together, 0 under reduced motion),
the `wave` that turns a phase into brightness, and the repaint schedule.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-skeleton = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-skeleton" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/skeleton.rs` lays the rows out at Interface sizes
  12, 16 and 20.
