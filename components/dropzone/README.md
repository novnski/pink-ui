# Dropzone

`pink-dropzone` is PINK's attachment drop veil as a single-GPUI package (UI
rethink decision 54): hidden at rest, it covers its surface with a scrim and a
centred "Drop to attach" while files hover over it. Like the other packages
under `components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/dropzone.rs`), where the story adds the attachments card and
its thumbnails. PINK pins the package and patches `gpui` to its vendored
runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `veil(&VeilStyle)`: the veil over its relatively positioned surface,
  transparent until the host's `drag_over` style sets its opacity to 1, with
  the scrim (`VEIL_ALPHA_DARK` in the dark appearance) and a centred
  `LABEL_TEXT` label colour.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. The
  label grows with the host's Interface size.

The host adds the rest: the resolved colours, the label, the id, the rounded
corners of a card it covers, and the `drag_over` styles for the payload types
it accepts (GPUI matches them by type, so a resize handle or a tab drag never
reveals the veil).

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-dropzone = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-dropzone" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/dropzone.rs` lays the veil out at Interface sizes 12,
  16 and 20.
