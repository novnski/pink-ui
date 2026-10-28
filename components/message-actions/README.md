# Message actions

`pink-message-actions` is PINK's message hover lane as a single-GPUI package
(UI rethink decision 54): the band reserved under a settled transcript entry
and the timestamp and icon actions (Side chat or Send to main, Fork from
here, Edit message, Rewind, Copy) that fade in while the entry is hovered.
Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/message_actions.rs`). PINK pins
the package and patches `gpui` to its vendored runtime, so there is one GPUI
type graph and no copied source.

## What it draws

- `lane(align_end)`: the `LANE_HEIGHT` band, flush with the content column,
  holding the strip on its bottom `ACTION_SIZE`. `align_end` puts the strip
  under a user bubble's right edge; otherwise it starts on a reply's first
  character. The band is always reserved, so revealing the strip never
  moves the layout.
- `strip()`: a `STRIP_LEAD` lead, then the host's children in a line
  `STRIP_GAP` (fixed pixels) apart.
- `timestamp(text, color)`: the entry's time at `TIMESTAMP_TEXT`.
- `action(id, glyph, wash)`: one action, the host's glyph (sized
  `ACTION_ICON`) centred in an `ACTION_SIZE` square with `ACTION_RADIUS`
  corners and the host's wash.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. The
  lane, actions, glyphs, lead and timestamp grow with the host's Interface
  size; the gap between parts and the corners stay fixed.

The host adds the rest: which actions an entry offers, their glyphs and
labels (Copy's check after copying), the hover fade that blends each wash,
role, accessible name, focus ring and tooltip, the listeners and their
behaviour, and the strip's fade-in while the entry is hovered.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-message-actions = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-message-actions" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/message_actions.rs` lays a reply's and a user
  message's lane out at Interface sizes 12, 16 and 20.
