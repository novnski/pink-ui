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
theme. It is the app icon's idea at hero size: a pink tile whose shades
deepen towards its edges.

## What it draws

- `mark_cells(ripple)`: the `GRID` x `GRID` tile, row by row. Each `Cell`
  has a `shade` from the light heart (0) to the deep edge (1) in softened
  square rings, and a `lift` while a ripple passes (0 at rest). The four
  corner cells are `None`, which rounds the tile like the app icon.
- `mark(cells, side, &MarkStyle)`: the cells painted as rounded quads in a
  `side` square, `CELL_GAP` apart with `CELL_ROUND` corners. A lifted cell
  grows by up to `RIPPLE_GROW` and moves towards the highlight.
  `cell_color` and `mix` are the colour it paints.
- `mark_side(width, height)`: `MARK_SIDE` from a 900 tall, 440 wide column,
  down to `MARK_MIN_SCALE` of it.
- `ripple_at(progress)`: one ripple of `RIPPLE_SECONDS`, eased out from the
  centre; `None` once it has finished, when the host stops asking for
  frames.
- `heading(color, copy)`, `subheading(color, copy)`, `action(button)`,
  `hint(&HintStyle, lead, keys)` and `welcome()`: the question
  (`HEADING_SIZE` semibold, `HEADING_GAP` under the mark), the line under it
  (`SUBHEADING_SIZE`, wrapped at `SUBHEADING_WIDTH`), the host's button set
  `ACTION_GAP` under that with its padding and `ACTION_TEXT` label, the
  shortcut in a keycap `HINT_GAP` under the button, and the centred column
  holding them. The text is rems, so it follows the host's Interface size;
  the mark follows the space it has.

The host adds the rest: the clock (when a ripple started, reduced motion,
animation frames), the resolved colours (the brand pinks for the mark) and
mono family, the mark's role, accessible name and hover listener, the
button with its id, role, accessible name and click, and the real shortcut
for the hint.

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
  sizes 12, 16 and 20; the story module's tests check the tile's shape and
  shades, the ripple, the colour mix and the mark's scale.
