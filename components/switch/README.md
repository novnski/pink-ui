# Switch

`pink-switch` is PINK's toggle switch as a single-GPUI package (UI rethink
decision 54). Like the other packages under `components/` it depends on
crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/switch.rs`), and
the kit's `pink::toggle_switch` (so `switch_control`) draws through it. PINK
pins the package and patches `gpui` to its vendored runtime, so there is one
GPUI type graph and no copied source.

## What it draws

- `switch(&SwitchStyle, progress)`: a `TRACK_W` by `TRACK_H` pill with a
  round `KNOB` inset `KNOB_INSET` from its ends and lifted by a small shadow,
  at `progress` (0 off, 1 on) along its travel. It is `track(..)` holding
  `knob(..)`; `knob_left(progress)` gives the knob's edge.
- `ui_rems(design)`: sizes are rems (pt at the 16 px default), so the switch
  follows the host's Interface size.

The host owns the motion: it eases `progress` when the value flips and
passes the track already mixed between its off and on tones. It adds the
id, the Switch role and toggled state, the focus ring, the cursor and the
click.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-switch = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-switch" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/switch.rs` lays the switch out at Interface sizes
  12, 16 and 20 at three points of its travel.
