# Welcome screen

`pink-welcome-screen` is PINK's first-run welcome as a single-GPUI package
(novnski/pink#259): the lit moon drawn in text, the question under it and the
spacing of the one action that starts a project. Like the other packages
under `components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/welcome_screen.rs`), and the Welcome Screen story draws with
it. PINK pins the package and patches `gpui` to its vendored runtime, so
there is one GPUI type graph and no copied source.

## What it draws

- `moon_rows(turn, light)`: the `COLUMNS` x `ROWS` glyph grid of a sphere
  whose near side is PINK's NASA-derived artwork (`assets/moon-full-ascii.txt`)
  and whose far side is procedural, lit from the upper left, with the surface
  turned by `turn` (0..1 of a revolution). A light appearance inks it like
  paper; a dark one lights it.
- `moon(rows, diameter, &MoonStyle)`: the grid in a `diameter` square, the
  glyph size set from the mono family's advance so `COLUMNS` glyphs span it.
  `moon_ink(text, turn, light)` is the ink, brightening through a turn in a
  dark appearance.
- `moon_diameter(width, height)`: `MOON_DIAMETER` from a 900 tall, 440 wide
  column, down to `MOON_MIN_SCALE` of it.
- `turn_at(progress)`: smoothstep through one turn of `TURN_SECONDS`, `None`
  once it has finished, when the host parks the moon.
- `heading(color, copy)`, `action(button)` and `welcome()`: the question
  (`HEADING_SIZE` semibold, `HEADING_GAP` under the moon), the host's button
  set `ACTION_GAP` under it with its padding and `ACTION_TEXT` label, and the
  centred column holding the three. The question and the label are rems, so
  they follow the host's Interface size; the moon follows the space it has.

The host adds the rest: the clock (when a turn started, reduced motion,
animation frames), the resolved colours and mono family, the moon's role,
accessible name and hover listener, and the button with its id, role,
accessible name and click.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-welcome-screen = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-welcome-screen" }
```

`empty_project_moon.rs` keeps PINK's moon view (its clock and fixture
readback) and draws with `moon_rows` and `moon`; the onboarding canvas in
`shell.rs` lays the moon, `heading` and `action` out in `welcome()`.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/welcome_screen.rs` lays the welcome out at Interface
  sizes 12, 16 and 20; the story module's tests check the grid, the paper
  inking, a full turn and the moon's scale.
