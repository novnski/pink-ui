# Notice

`pink-notice` is PINK's in-context notice as a single-GPUI package (UI
rethink decision 54): the plate a composer notice floats on above the pill
and the capsule action a notice offers. Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/notice.rs`). PINK pins the package and patches `gpui` to its
vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `plate(&NoticeStyle)`: the opaque plate, `PLATE_RADIUS` corners (the stack
  ladder's notice step), the title row over the message `PLATE_GAP` apart,
  padded `PLATE_PAD_X` by `PLATE_PAD_Y`, with 12/16 copy in the host's ink.
- `title_row()`, `title(label)`, `spacer()`: the row that holds the tone
  glyph (`GLYPH`), the medium title and the actions, wrapping them under the
  title in a narrow pane.
- `copy_button()`: the `COPY_SIZE` Copy message frame, `COPY_RADIUS` corners.
- `message(text)`: the message on its own line, wrapping inside the plate.
- `action(&ActionStyle)`: a notice action, an `ACTION_HEIGHT` capsule with
  the host's border and text.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Every
  size grows with the host's Interface size; the radii stay fixed.

The host adds the rest: the colours (the tone flattened over its canvas),
the glyphs, the ids, the roles and accessible names, the focus rings, the
hover washes, the clipboard write and the listeners.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-notice = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-notice" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/notice.rs` lays the notice out at Interface sizes
  12, 16 and 20.
