# Welcome screen

`pink-welcome-screen` is PINK's first-run welcome as a single-GPUI package
(novnski/pink#259): PINK's mark, the question under it, a line on what to
do, the spacing of the one action that starts a project and its shortcut.
Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/welcome_screen.rs`), and the
Welcome Screen story draws with it. PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied
source.

The mark replaced the earlier text-drawn moon once PINK left its lunar
theme. It is the app icon drawn live: a deep pink rounded tile holding a
6 x 6 field of rounded cells, lightest at the heart and deepest at the
corners, with the icon's own geometry (16 pt gaps on an 824 pt tile).

## What it draws

- `mark_frame(build, ripple)`: one frame of the mark. `tile` is the tile's
  own build (0 to 1); `cells` are the `GRID` x `GRID` cells row by row, each
  with its `distance` from the centre (which picks its shade), how far it
  has `appear`ed in the build and how much a passing ripple `lift`s it. At
  rest (`None`, `None`) everything is in and nothing is lifted.
- `build_at(progress)` and `ripple_at(progress)`: the build when the mark
  first appears (`BUILD_SECONDS`: the tile settles in from 94% as it fades
  up, and the cells pop in from the centre outwards with a small overshoot)
  and one ripple (`RIPPLE_SECONDS`, eased out from the centre). Both are
  `None` once finished, when the host stops asking for frames.
- `mark(frame, side, &MarkStyle)`: the tile and its cells painted in a
  `side` square. The four corner cells are clipped to the tile's rounded
  corners (`clip_to_disc`), as the icon's mask clips them. `shade_index`,
  `cell_color`, `appear_scale`, `cell_rect` and `mix` are the pieces it
  paints with.
- `mark_side(width, height)`: `MARK_SIDE` from a 900 tall, 440 wide column,
  down to `MARK_MIN_SCALE` of it.
- `heading(color, copy)`, `subheading(color, copy)`, `action(button)`,
  `hint(&HintStyle, lead, keys)` and `welcome()`: the question
  (`HEADING_SIZE` semibold, `HEADING_GAP` under the mark), the line under it
  (`SUBHEADING_SIZE`, wrapped at `SUBHEADING_WIDTH`), the host's button set
  `ACTION_GAP` under that with its padding and `ACTION_TEXT` label, the
  shortcut in a keycap `HINT_GAP` under the button, and the centred column
  holding them. The text is rems, so it follows the host's Interface size;
  the mark follows the space it has.

The host adds the rest: the clocks (when the build and a ripple started,
reduced motion, animation frames), the resolved colours (the icon's tile,
seven shades and highlight) and mono family, the mark's role, accessible
name and hover listener, the button with its id, role, accessible name and
click, and the real shortcut for the hint.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-welcome-screen = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-welcome-screen" }
```

`welcome_mark.rs` keeps PINK's mark view (its clock and fixture readback)
and draws with `mark_cells` and `mark`; the onboarding canvas in `shell.rs`
lays the mark, `heading`, `subheading`, `action` and `hint` out in
`welcome()`.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/welcome_screen.rs` lays the welcome out at Interface
  sizes 12, 16 and 20; the story module's tests check the icon's geometry
  and shades, the build, the ripple, the corner clipping, the colours and
  the mark's scale.
