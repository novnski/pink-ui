# Welcome screen

`pink-welcome-screen` is PINK's first-run welcome as a single-GPUI package
(novnski/pink#259): a pulsing field of PINK's pink cells, the question under
its heart, a line on what to do, the spacing of the one action that starts a
project and its shortcut. Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/welcome_screen.rs`), and the Welcome Screen story draws
with it. PINK pins the package and patches `gpui` to its vendored runtime,
so there is one GPUI type graph and no copied source.

The field replaced the earlier text-drawn moon once PINK left its lunar
theme. It is the app icon grown into the room: rounded cells on the icon's
grid (16 pt gaps on an 824 pt tile), lightest and largest at a four-cell
heart and deepening, shrinking and fading as they spread, until they
dissolve at the room's side and top edges. Below the heart's row they fade
four times faster, so the words under it stay clear.

## What it draws

- `field_layout(width, height)`: the `FieldLayout` for a room: the heart's
  `side` in the column and the grid `pitch` (full size from 900 tall and
  440 wide, down to `FIELD_MIN_SCALE`), and how far the field reaches across
  (`reach_x`, half the width) and up (`reach_y`, to the top edge above the
  heart), each bounded by `FIELD_REACH_MIN` and `FIELD_REACH_MAX`.
- `field_cells(layout, build, pulse)`: every `FieldCell` with its offset
  from the heart, its side at rest, its `distance` from the heart (0 to 1 at
  the reach), how far it has `appear`ed in the build and how much the pulse
  `lift`s it.
- `build_at(progress)` and `pulse_at(elapsed)`: the build when the field
  first appears (`BUILD_SECONDS`: the cells pop in from the heart outwards
  with a small overshoot) and the pulse's phase. Pulses roll from the heart
  to the reach in `PULSE_SECONDS`, two always on their way, so one leaves
  the heart every half cycle (`pulse_lift`).
- `hero(layout, cells, &FieldStyle)`: the heart's square in the column, with
  the field painted around its centre and spilling past it; the host's
  canvas clips it, and the question and action paint over it.
  `shade_index`, `field_alpha`, `cell_color`, `appear_scale` and `mix` are
  the pieces it paints with.
- `heading(color, copy)`, `subheading(color, copy)`, `action(button)`,
  `hint(&HintStyle, lead, keys)` and `welcome()`: the question
  (`HEADING_SIZE` semibold, `HEADING_GAP` under the heart), the line under it
  (`SUBHEADING_SIZE`, wrapped at `SUBHEADING_WIDTH`), the host's button set
  `ACTION_GAP` under that with its padding and `ACTION_TEXT` label, the
  shortcut in a keycap `HINT_GAP` under the button, and the centred column
  holding them. The text is rems, so it follows the host's Interface size;
  the field follows the space it has.

The host adds the rest: the clocks (when the build and the pulse started,
reduced motion, animation frames), the resolved colours (the icon's seven
shades and the highlight) and mono family, the field's role and accessible
name, the button with its id, role, accessible name and click, and the real
shortcut for the hint.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-welcome-screen = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-welcome-screen" }
```

`welcome_mark.rs` keeps PINK's field view (its clocks and fixture readback)
and draws with `field_cells` and `hero`; the onboarding canvas in `shell.rs`
lays the hero, `heading`, `subheading`, `action` and `hint` out in
`welcome()` and clips the field.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/welcome_screen.rs` lays the welcome out at Interface
  sizes 12, 16 and 20; the story module's tests check the four-cell heart,
  the reach and the faster fade below it, the shading, shrinking and
  fading outwards, the build, the pulses, the colours and the fit to the
  room.
