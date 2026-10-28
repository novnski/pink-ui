# Composer pill

`pink-composer-pill` is PINK's composer surface as a single-GPUI package (UI
rethink decision 54). Like the other packages under `components/` it depends
on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/composer_pill.rs`).
PINK pins the package and patches `gpui` to its vendored runtime, so there is
one GPUI type graph and no copied source.

## What it draws

- `pill(&PillStyle)`: the rounded surface the composer, the question wizard
  and the queue tray behind them share, with a 1 px hairline (`BORDER_V` top
  plus bottom at every Interface size), the host's background and, when
  `lifted`, its large shadow.
- `radius(dock_amount)`: `RADIUS` while the pill floats, tightening by
  `DOCK_TIGHTEN` as it docks to the pane's edge.

The host adds the rest: the frosted or opaque background and whether it lifts,
the dock morph, the height morph and clipping, the input, the controls row,
the notices and trays above the pill, and their behaviour.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-composer-pill = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-composer-pill" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/composer_pill.rs` lays the pill out at Interface
  sizes 12, 16 and 20.
