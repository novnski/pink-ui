# Overview card

`pink-overview-card` is PINK's thread overview card and the canvas geometry
it shares with the conversation, as a single-GPUI package (UI rethink
decision 54), after T3 Code's `ThreadDetailsCard`, `threadDetailsCardLayout`
and `chatCanvasLayout`. Like the other packages under `components/` it
depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/overview_card.rs`). PINK pins the package and patches
`gpui` to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `card(&CardStyle)`: the rounded, bordered surface (`CARD_RADIUS`), its
  sections stacked in a column, with the host's colours, font and optional
  shadow (none inline: an in-flow card has no elevation).
- `section(rule)`: a group of rows inside `CARD_INSET`, optionally under a
  1 px rule that runs border to border.
- `heading(color, label, count)`: a section's small heading with an
  optional trailing count.
- `row(&RowStyle)`, `glyph`, `label`, `label_with_detail`, `detail` and
  `dot`: one row at least `ROW_HEIGHT` tall, padded `ROW_PAD_X`, its parts
  `ROW_GAP` apart, rounded concentric with the card, with a leading glyph or
  status dot, a truncated label (optionally over a quiet second line) and a
  quiet trailing detail.
- `note(color, text)`: a section's loading, empty or error line.
- `actions()` and `show_more(color, text)`: a wrapping line of the host's
  small buttons under a row, and the quiet control that unfolds a longer
  list.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Rows,
  glyphs, text and padding grow with the host's Interface size; the card's
  inset, rules and corners stay fixed.

## Canvas geometry

Logical pixels, not Interface-scaled: they divide the conversation canvas.

- `card_placement(width, height, min_chat_width, top_inset)`: the inline
  card's place (`CARD_WIDTH` wide, `CARD_GAP` from the canvas's top, right
  and bottom edges, below `top_inset`), or `None` when the chat beside it
  would be narrower than `min_chat_width` after `CHAT_CLEARANCE` and
  `LANE_PADDING`, or the card shorter than `MIN_CARD_HEIGHT`. The host then
  shows the same card as a popover.
- `chat_lane(width, preferred_width, card_left)`: the chat column. It stays
  centred while it fits, shifts left only as far as it must to keep
  `CHAT_CLEARANCE` from the card, and narrows only once it reaches
  `LANE_PADDING`.
- `density(available, full, compact)`: `Full`, `Compact` or `Essential` for
  the height the card has, from the content's measured heights at full and
  compact density (0 = not measured yet).

## What the host adds

The ids, hover listeners and their fades, clicks, drags, focus, tooltips,
scrolling, accessibility names and roles, and every action. It decides which
sections show at each density and keeps the transcript and composer inside
the lane.

## Pinning it in PINK

```toml
pink-overview-card = { git = "https://github.com/novnski/pink-ui", rev = "<merge commit on main>", package = "pink-overview-card" }
```
