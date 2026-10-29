# Loaders

`pink-loaders` is PINK's loading indicators as a single-GPUI package (UI
rethink decision 54): the gradient matrix, the glyph and mono spinners, the
upload progress ring and the boot splash's content, drawn as PINK's
`loaders.rs` draws them. Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place (`src/components/loaders.rs`);
the kit's `pink::loaders` spinners, ring and splash draw through it. PINK
pins the package and patches `gpui` to its vendored runtime, so there is one
GPUI type graph and no copied source. The breathing orb is
`pink-working-trailer`'s `orb`, not drawn twice.

## What it draws

- `gradient_spinner(cell, tints, phase, pulse)`: the 3×3 matrix of round
  `cell` px cells, `cell / 2` apart, rows tinted `[light, mid, deep]` top to
  bottom. Each cell's opacity is `pulse(phase + gradient_cell_phase(row,
  col))`, so the wave enters at the bottom edge and converges on the top
  centre.
- `mini_spinner(cell, tints, phase, pulse)`: the 2×3 glyph, the same cells
  in three rows of two. Its offsets (`mini_cell_phase`, `MINI_RING`) snake
  clockwise round the perimeter. One tint for all three rows is the mono
  spinner. `mini_spinner_box(cell)` is the 2.5 × 4 cell box a host reserves.
- `upload_progress_ring(percent, diameter)`: a `diameter` pt box with a
  white 22% track and a white 95% arc growing clockwise from twelve o'clock,
  `RING_STROKE` polylines, and the percent centred, SEMIBOLD at
  `RING_LABEL_RATIO` of the diameter.
- `splash_overlay(bg, tints, phase, pulse, label, label_color)`: the boot
  splash's `inset_0` layer of `bg`, a `SPLASH_CELL` gradient matrix above the
  `SPLASH_TEXT` label, `SPLASH_GAP` apart and centred.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. The
  ring and the splash label grow with the host's Interface size; spinner
  cells and the splash gap stay fixed pixels, as in PINK.

The host owns the clock and the motion: each spinner's phase (0..1 of its
period, PINK's shared pulse clock, 0 under reduced motion) and the pulse
curve that turns a cell's phase into its opacity (PINK:
`gspin_opacity(t, GSPIN_DIM)`, shared with the terminal viewport). It also
keeps each mini spinner's own cached view and its re-render lease, the
upload percent, the resolved colours and label, and the splash's fade out
and removal.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-loaders = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-loaders" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/loaders.rs` lays the spinners, the ring and the
  splash out at Interface sizes 12, 16 and 20.
