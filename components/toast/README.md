# Toast

`pink-toast` is PINK's feedback toast as a single-GPUI package (UI rethink
decision 54): the capsule centered just above the composer that reports a
status or an error. Like the other packages under `components/` it depends
on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/toaster.rs`, the
`package` module). PINK pins the package and patches `gpui` to its vendored
runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `lane()`: the absolutely placed lane `TOAST_HEIGHT + ANCHOR_GAP` above its
  `.relative()` anchor, centering the capsule across it.
- `capsule(&ToastStyle)`: the fully rounded `TOAST_HEIGHT` capsule, capped at
  `MAX_WIDTH`, padded `PAD_LEFT`/`PAD_RIGHT`, its parts `GAP` apart, on the
  host's surface, edge, shadows and font. Its copy is `TEXT` in rems; the
  capsule itself stays fixed.
- `message(text)`: the one line of copy, truncating.
- `dismiss()`: the round `DISMISS_SIZE` Dismiss frame (`DISMISS_GLYPH` glyph).
- `arrive(wrapper, progress)`: the arrival's fade and `RISE`, for the host's
  animation.

The host adds the rest: the glyphs and their tone, the ids, the status role
and accessible names, the focus ring, the hover wash, the dismiss listener,
the animation timing and the notice's lifetime.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-toast = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-toast" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/toast.rs` floats the toast at Interface sizes 12, 16
  and 20.
