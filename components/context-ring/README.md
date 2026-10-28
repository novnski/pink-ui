# Context ring

`pink-context-ring` is PINK's composer context-window chip as a single-GPUI
package (UI rethink decision 54). Like the other packages under
`components/` it depends on crates.io `gpui = "=0.2.2"` in its own workspace.
The pink-ui gallery compiles this same `src/lib.rs` in place
(`src/components/context_ring.rs`). PINK pins the package and patches `gpui`
to its vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `context_ring(id, fraction, label, &ContextRingStyle)`: a `HEIGHT` capsule
  with the ring and, when given, the share. The ring is `RING` across: the
  full `track`, then an arc for `fraction` from twelve o'clock, clockwise, in
  `arc`, with a `STROKE` line at `RADIUS`. No fraction shows the bare track.
- `ring(fraction, track, color)`: the ring alone.
- `chip_widths(label_width, scale)`: the chip's `(with label, ring only)`
  widths at Interface `scale`, so a controls row can give the share up before
  anything truncates.
- `ui_rems(design)`: sizes in rems, so the chip grows with the Interface size.

The host resolves the tones (muted, warning from 75%, danger from 90%, or the
compaction accent, floored for contrast on its composer), the wash over its
hover fade and the label. It sets tabular figures on the chip while the share
shows (crates.io GPUI has no font features). It also adds the role, accessible
name, expanded state, Tab focus and its ring, the hover card and the
behaviour.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-context-ring = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-context-ring" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- Geometry: `tests/context_ring.rs` in the pink-ui crate checks that the drawn
  ring-only chip matches `chip_widths` and the height at Interface sizes 12,
  16 and 20.
