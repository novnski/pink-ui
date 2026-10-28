# Message badge

`pink-message-badge` is PINK's message badge as a single-GPUI package (UI
rethink decision 54): the pill a sent message draws in place of structured
context it carries, and the rows of the hover card that opens over it. Like
the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/message_badge.rs`). PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI type
graph and no copied source.

## What it draws

- `badge(&BadgeStyle)`: the pill, at least `BADGE_HEIGHT` tall, padded
  `BADGE_PAD_X`, glyph (`BADGE_GLYPH`) and label `BADGE_GAP` apart,
  `BADGE_RADIUS` corners, 12 medium copy.
- `card(surface)`: lays the hover card's rows out on the host's card,
  `CARD_WIDTH` wide, inset `CARD_PAD`, rows `CARD_GAP` apart.
- `detail_row(&DetailStyle, location, tag, body)`: one row, an accent bar
  beside the truncating mono location, its optional tag and the body.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Every
  size grows with the host's Interface size; the radii and the bar stay
  fixed.

The host adds the rest: the colours, the mono font, the glyph, the id, the
label role and accessible name, the hover card's tooltip and delay, the
card's surface and its frost.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-message-badge = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-message-badge" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/message_badge.rs` lays the pill and its card out at
  Interface sizes 12, 16 and 20.
