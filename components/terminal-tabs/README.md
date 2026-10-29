# Terminal tabs

`pink-terminal-tabs` is PINK's terminal tab strip as a single-GPUI package
(UI rethink decision 54). Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/terminal_tabs.rs`); the Terminal drawer's strip draws
through it. PINK pins the package and patches `gpui` to its vendored
runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `bar(edge)`: the `BAR_HEIGHT` band (the shared header height) over a 1 px
  bottom edge, no fill of its own.
- `tab(&TabStyle, glyph_path, title, close)`: one session tab at the fixed
  `TAB_WIDTH` (the drag maths assume uniform tabs): the `GLYPH` terminal
  glyph, the truncating title and the close control. `TabStyle` carries the
  fill (already blended over the hover fade), the title and glyph colours
  and an exited session's dimming (`EXITED_OPACITY_DARK`,
  `EXITED_OPACITY_LIGHT`).
- `close_button(&CloseStyle, glyph_path)`: the `CLOSE_SIZE` close control;
  only the selected tab shows it, the others keep its room so titles never
  shift.
- `add_button` and `collapse_button(&ButtonStyle, glyph_path)`: the
  `BUTTON_SIZE` plus after the tabs and the chevron pinned right by
  `spacer()`.
- `drag_spacer()`: the room a dragged tab leaves; `ghost(&GhostStyle,
  title)`: the raised tab that follows the pointer.
- `ui_rems(design)`: the tab label is a rem (pt at the 16 px default); the
  band, tabs and controls stay in pixels, so the strip does not grow with
  the Interface size.

The host adds the rest: the resolved colours and hover blends, the glyph
paths and titles, and the ids, hover, click, middle-click and drag
listeners, the drop target, the slide animation and the focus.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-terminal-tabs = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-terminal-tabs" }
```

PINK's `terminal/panel.rs` `render_tab_bar` and `TabGhost` are adapters over
it; `TAB_WIDTH` comes from the package.

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/terminal_tabs.rs` lays the strip out at Interface
  sizes 12, 16 and 20.
