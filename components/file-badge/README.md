# File badge

`pink-file-badge` is PINK's tool-header file badge as a single-GPUI package
(UI rethink decision 54): the chip that names the file a tool read or wrote.
Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/file_badge.rs`; the Tool Call
Card story's badges draw with it). PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `badge(&BadgeStyle)`: the `HEIGHT` chip, `RADIUS` corners, its well and
  name `GAP` apart between `PAD_LEFT` and `PAD_RIGHT`, clipped, shrinking
  with its header.
- `well(background, artwork)`: the `WELL` counter-shaded seat, `WELL_RADIUS`
  corners, centring the host's file-type artwork (sized `GLYPH`).
- `name(text)`: the file's name on one line, truncating.

Sizes are fixed so the badge fills the tool header's detail slot at every
Interface size. The host adds the rest: the resolved colours (the chip's
fill blended over its hover fade when it opens the file), the artwork and
name, the id, hover listener, role and accessible name, tooltip, frosted
backdrop and the open action.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-file-badge = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-file-badge" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/file_badge.rs` lays a short and a truncated badge
  out at Interface sizes 12, 16 and 20.
