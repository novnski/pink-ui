# Attachment thumb

`pink-attachment-thumb` is PINK's attachment thumbnails as a single-GPUI
package (UI rethink decision 54): the image tiles above a sent message and
the squares staged above the composer's input. Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own
workspace. The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/attachment_thumb.rs`; the Dropzone and Image Lightbox
stories draw their thumbnails with it). PINK pins the package and patches
`gpui` to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `Kind::Sent` (`SENT_WIDTH` by `SENT_HEIGHT`) and `Kind::Staged`
  (`STAGED_SIZE` square), both with `RADIUS` corners.
- `frame(kind)`: the bare clipped frame.
- `plate(kind, &PlateStyle)`: a tile holding an image, its 1 px edge over
  its plate, positioned for the sending veil.
- `image(kind, source)`: the image covering the tile inside its edge, with
  explicit dimensions and its own corners (`SENT_IMAGE_RADIUS`,
  `STAGED_IMAGE_RADIUS`).
- `sending(pulse)`: the black veil (`SENDING_ALPHA`, breathing by
  `SENDING_PULSE`) over a tile whose bytes are still crossing, centring the
  host's `RING` upload ring or `SPINNER_CELL` spinner.
- `missing(kind, &PlateStyle)`: the dashed tile left where a file could not
  load.
- `skeleton(kind, &PlateStyle, pulse)`: the tile while its image loads, at
  `SKELETON_REST` plus `SKELETON_PULSE` opacity.
- `sent_strip()`: the right-aligned, wrapping strip above a user message,
  `SENT_STRIP_GAP` apart.
- `staged_strip()`: the wrapping strip above the composer's input,
  `STAGED_STRIP_GAP` apart.
- `remove_disc(&RemoveStyle)`: the `REMOVE_SIZE` disc overhanging a staged
  square's corner by `REMOVE_OVERHANG`, hidden until the host reveals it,
  for a `REMOVE_GLYPH` glyph.

Sizes are fixed px: thumbnails keep their size at every Interface size. The
host adds the rest: the resolved colours, the image, the pulse, the
indicator, the ids, the pointer, the role and accessible names, the focus
handling, the hover group that reveals the remove disc, its glyph and
frosted layer, and the clicks that open the lightbox or remove the file.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-attachment-thumb = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-attachment-thumb" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/attachment_thumb.rs` lays the sent strip and a
  staged square with its remove disc out at Interface sizes 12, 16 and 20.
