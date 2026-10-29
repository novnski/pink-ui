# Comment card

`pink-comment-card` is PINK's review comment cards as a single-GPUI package
(UI rethink decision 54): the notes a reviewer pins to a line of the Changes
diff, a Markdown preview or the code editor before sending them to the
agent. Like the other packages under `components/` it depends on crates.io
`gpui = "=0.2.2"` in its own workspace. The pink-ui gallery compiles this
same `src/lib.rs` in place (`src/components/comment_card.rs`; Diff Viewer,
Comment Thread and Comments Panel draw their cards with it). PINK pins the
package and patches `gpui` to its vendored runtime, so there is one GPUI
type graph and no copied source.

## What it draws

- `card(&BandStyle, height, column)` and `draft(&BandStyle, height, column)`:
  the posted band (`card_height(body_lines, ui_font_size)` tall) and the
  open draft (`draft_height(ui_font_size)`), each a row with its
  `ACCENT_BAR_WIDTH` bar first.
  `band` is the same row without a height, and `accent_bar` the bar alone.
  Under a `Column` the content centres at most `max_width` wide between
  the gutters and the bar sits on the band's edge.
- `content(column)`: the column inside a band, `PAD_X` in and half of
  `PAD_V` above and below.
- `header(glyph, color)` and `location(text, color, font)`: the
  `HEADER_HEIGHT` row with the comment glyph and the mono `path:line`,
  truncating.
- `edit(glyph, color, group)` and `remove(glyph, color, group)`: the `EDIT`
  box and the `REMOVE_HIT` box with its 16 pt footprint, shown while the
  pointer is over the card's hover group.
- `body(text, color)`: the note at `BODY_TEXT` / `LINE_HEIGHT`, clipped to
  the card.
- `field(input)`, `actions()` and `action(&ActionStyle, label)`: the draft's
  `FIELD_HEIGHT` slot, its trailing action row and the `ACTION_HEIGHT`
  plates (the primary on `solid`, Cancel a ghost).
- `floating(card)`, `floating_draft(card)`, `framed_field(&FieldStyle,
  input)` and `scaled_action(&ActionStyle, label)`: the code editor's
  floating card and draft on the host's popover card, its framed field and
  its actions, whose box scales with the Interface size.
- `adder(fill, glyph, color)`: the `ADDER` plate with its plus.
- `ui_rems(design)`: a design size in rems. The header, note, location,
  draft height and labels grow with the host's Interface size; the bar,
  padding, controls and the band draft's field and actions stay fixed.

The host adds the rest: every resolved colour (a ghost action's label and
wash blended over its hover fade), the glyphs, the location and note text,
the text field, a floating card's popover card and frost, the ids, the hover
group's name, roles and accessible names, focus, keys and every action.
Comment state, editing and the body-line estimate stay with the host.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-comment-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-comment-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/comment_card.rs` lays posted cards (full width,
  under a column and narrow), the adder, a draft and a floating card out at
  Interface sizes 12, 16 and 20.
