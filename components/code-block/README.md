# Code block

`pink-code-block` is PINK's fenced-code chrome as a single-GPUI package (UI
rethink decision 54): the frame a fenced code block, generated diagram or
tool output sits in, its header strip and the two action shapes. Like the
other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/code_block.rs`); Markdown
Message's fences draw through it. PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `frame(id, header, &FrameStyle)`: full width, `FRAME_RADIUS` corners, a
  1 px edge, clipping and positioned, with the header (if any) above the
  host's body.
- `header(language, actions, &FrameStyle)`: the `HEADER_HEIGHT` strip over
  a 1 px rule, the `LABEL_TEXT` language `HEADER_PAD_LEFT` in and the
  actions `ACTION_GAP` apart, `HEADER_PAD_RIGHT` from the edge. `None` when
  there is neither, so a bare block has no strip.
- `icon_action(id, glyph, wash)`: an `ACTION_SIZE` square for an
  `ACTION_ICON` glyph (wrap, diagram/source toggles).
- `copy_action(id, glyph, label, color, wash)`: the Copy pill, its
  `COPY_ICON` glyph then, once copied, its `COPY_TEXT` label.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Only
  the language and Copied labels grow with the host's Interface size; the
  strip, actions, glyphs and corners stay fixed.

The host adds the rest: the body and its syntax-highlighted lines,
scrollbar and selection, the glyph assets, every resolved colour, each
action's hover fade, the button roles, accessible names, focus rings,
tooltips and listeners, and the copy, wrap and diagram behaviour.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-code-block = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-code-block" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/code_block.rs` frames a block at rest, one just
  copied and a bare block at Interface sizes 12, 16 and 20.
