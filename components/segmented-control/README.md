# Segmented control

`pink-segmented-control` is PINK's segmented control (`button.rs`
`segmented`) as a single-GPUI package (novnski/pink#258). Like the other
packages under `components/` it depends on crates.io `gpui = "=0.2.2"` in
its own workspace. The pink-ui gallery compiles this same `src/lib.rs` in
place for its Segmented Control's tray shape
(`src/components/segmented_control.rs`), so the gallery and PINK draw the
tray from one source. PINK pins the package and patches `gpui` to its
vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `tray`: the quiet track with a hairline edge, `INSET` px around the
  content box, and the options in a row over the thumb. Its `Track` takes
  the width (or `None` to fill the container), height and type size in
  design pt, and the corner in px.
- `thumb`: one thumb under the chosen option, placed by a fraction of the
  content box, so nothing is measured. The host animates `position`. In
  the light appearance a `Lift` adds the hairline edge and soft shadow.
- `option` and `label`: an option's equal share, its content centred, the
  chosen one in medium weight, and a label that truncates.

Every design size is in pt at the 16 px default and becomes rems through
`ui_rems`, the same formula as PINK's `ui_rems`, so the control grows with
the Interface size (PINK sets the window's rem to it).

## What the host adds

The package draws only. The host supplies the text, glyph elements and
colours, already resolved and blended over its hover fade, and adds what
crates.io GPUI 0.2.2 can't express: the radio group and radio roles, names
and checked state, focus and its ring, the hover tone, clicks and the
keyboard (Tab to an option, arrow keys, Home and End to move and select).

## Checks

- `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- `tests/segmented_control.rs` in the gallery: the track's size, the equal
  shares, the thumb under the chosen share and a long label truncating, at
  Interface sizes 12, 16 and 20.
