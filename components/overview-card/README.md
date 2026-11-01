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

T3 Code's thread details grammar: every control is one 32 pt row with a
16 pt glyph, a 13 pt medium label and quiet trailing meta or a chevron.

- `card(&CardStyle)`: the bordered surface (`CARD_RADIUS`, T3's
  `rounded-3xl` on its 10 px radius scale), sections in a column, with the
  host's colours, font and optional shadow (none inline).
- `section(rule)`: a group of controls inset 8 pt at the sides and top and
  10 pt below, optionally under a 1 px rule that runs border to border.
- `heading(color, label, status)`: a section's small heading with an
  optional trailing status (`1 running`).
- `row(&RowStyle)` with `glyph`, `label`, `title` and `meta`: one control
  `ROW_HEIGHT` tall, padded `ROW_PAD_X` with its parts `ROW_GAP` apart and
  `ROW_RADIUS` corners.
- `select_row`, `select_label` and `select_chevron`: a row that opens a
  menu, its label half and its trailing `CHEVRON_SLOT` chevron.
- `split_group`, `split_primary`, `split_separator` and `split_secondary`:
  an action and its options (Open in, Run, Commit) in one row shape, with a
  16 pt hairline between them only while the pair is hovered.
- `diff_counts(&DiffCountsStyle, additions, deletions)`: `+12 -3` in
  figures, green and red when non-zero.
- `fold_chevron(path, color, expanded)`: a group's chevron, turned to point
  right while it is folded (Previous agents).
- `list()`, `show_more(style, glyph, text)`, `note(color, text)`,
  `icon_action(fill)` and `dot(color)`: a list that scrolls past
  `LIST_MAX_HEIGHT` (rows nest by `INDENT`), the row that unfolds more of
  it, a loading, empty or error line, a 24 pt icon action and a status dot.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems.
  Everything above grows with the host's Interface size.

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
