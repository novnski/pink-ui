# User bubble

`pink-user-bubble` is PINK's sent-prompt plate as a single-GPUI package (UI
rethink decision 54): the rounded, padded bubble a user message sits on in
the transcript. Like the other packages under `components/` it depends on
crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/user_bubble.rs`,
whose `plate` the transcript stories share). PINK pins the package and
patches `gpui` to its vendored runtime, so there is one GPUI type graph and
no copied source.

## What it draws

- `bubble(&BubbleStyle)`: the plate, `RADIUS` corners, padded `PAD_X` by
  `PAD_Y`, the prompt at `TEXT` on a `LINE_HEIGHT` line box, in the host's
  fill and text colour, capped at the host's `max_width` (the column width
  times `MAX_WIDTH_FRACTION`). It may shrink below its text's unwrapped
  width, so a long prompt wraps inside the cap.
- The optional hairline `edge`: PINK passes a 1 px inset spread shadow in
  the light appearance. The host builds it because crates.io GPUI 0.2.2 has
  no inset shadows; a shadow costs no layout, so measured heights stay
  exact.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. The
  padding, text and line box grow with the host's Interface size; the
  corners stay fixed.

The host adds the rest: right-aligning the row, the prompt's text and
mention chips, text selection and find highlights, the long-prompt fold and
its expander, attachments and badges above the plate, and the pending clock
beside it.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-user-bubble = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-user-bubble" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/user_bubble.rs` lays a short and a long prompt out
  at Interface sizes 12, 16 and 20.
