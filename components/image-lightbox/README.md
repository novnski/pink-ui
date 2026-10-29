# Image Lightbox

`pink-image-lightbox` is PINK's image lightbox chrome as a single-GPUI
package (UI rethink decision 54): the window-wide layer a transcript or
composer thumbnail, a Markdown image and the Files image preview open. Like
the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this same
`src/lib.rs` in place (`src/components/image_lightbox.rs`). PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI type
graph and no copied source.

## What it draws

- `layer(viewport, &LightboxStyle)`: the whole window under the scrim
  (`SCRIM_ALPHA_DARK` in the dark appearance), its parts centred in a column
  `GAP` px apart.
- `frame(viewport)` and `frame_size(viewport)`: the image frame,
  `FRAME_WIDTH` by `FRAME_HEIGHT` of the window, that the host's viewer fills.
- `rise(frame, progress)`: the frame's entrance, fading in while it rises
  `RISE` px. The host runs it over `RISE_MS` and fades the layer in over
  `FADE_MS` with its own motion curves.
- `caption(viewport, &LightboxStyle)`: the file name, `CAPTION_TEXT` rems, cut
  to one line no wider than the frame.
- `close_button()`: the `CLOSE_SIZE` px capsule `CLOSE_INSET` px in from the
  top right corner (glyph `CLOSE_GLYPH` px).
- `place(layer)`: the layer over everything in the window from its top left
  corner, at `LAYER_PRIORITY`.
- `ViewGeometry`: the viewer's zoom and pan maths (no drawing). The image
  starts fitted inside the viewport, never upscaled, and centred;
  `zoom(scale, anchor)` keeps the image point under the anchor in place
  between `MIN_ZOOM` and `MAX_ZOOM` (less for an image whose zoomed side
  would pass `MAX_EXTENT` px); `pan_by(delta)` keeps the pan inside the
  image's overhang; `resize` refits a fitted image and clamps a zoomed one;
  `image_origin()` and `contains(point)` place the image and hit-test it.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. The
  caption grows with the host's Interface size; the frame follows the window
  and the close capsule stays fixed.

The host adds the rest: the resolved scrim and caption colours, the viewer
that places the image by `ViewGeometry`, the file name, the ids, the focus
handle, the close capsule's role, accessible name, glass hover fill and
glyph, the animations and every listener and gesture (Escape, a plain
click, the wheel, a pinch, a drag) that drives the geometry.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-image-lightbox = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-image-lightbox" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/image_lightbox.rs` lays the lightbox out at Interface
  sizes 12, 16 and 20 and checks the view geometry's fit, anchored zoom and
  bounds.
