# Menu card

`pink-menu-card` is PINK's floating card, its rows and the chrome around
them as a single-GPUI package (UI rethink decision 54). Like the other packages under `components/`
it depends on crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui
gallery compiles this same `src/lib.rs` in place
(`src/components/menu_card.rs`). PINK pins the package and patches `gpui` to
its vendored runtime, so there is one GPUI type graph and no copied source.

## What it draws

- `card(&CardStyle)`: the bordered surface every menu, picker and popover
  opens on, `CARD_RADIUS` corners, rows stacked `MENU_GAP` apart inside
  `CARD_INSET`, with the host's colours, font and optional shadow.
- `row(&RowStyle)`: one item, its parts in a line `ROW_GAP` apart, padded
  `ROW_PAD_X` by `ROW_PAD_Y` and rounded to `ITEM_RADIUS` so it sits
  concentric in the card. Text and fill come from the host.
- `heading(color, label)`: a small uppercase heading over a group of rows,
  tracked with hair spaces (`tracked_upper`).
- `separator(color)`: the 1 px rule between groups, border to border across
  the card's inset.
- `section(rule)`: a trailing block of rows under a 1 px top rule.
- `search_frame(fill)`: the borderless search field at the top of a picker;
  the host adds its text input.
- `palette_card(&PaletteStyle, width, corner_radius)` and
  `search_glyph(glyph, color)`: the command palette's card and its search
  glyph in a row glyph's slot.
- `key_cap(fill)`, `key_hint`, `key_hint_text`, `key_hint_pair` and
  `kbd_hint` with a `KeyHintStyle`: the key legends in palette and picker
  footers (a glyph, a word or two glyphs on a cap, then a verb) and the
  accelerator chip inside a row.
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Row,
  heading, field and key padding and text grow with the host's Interface
  size; the card's inset, gap, rules and corners stay fixed.

The host adds the rest: the id, the hover listener and its fade, the
selected wash, the actions, the text input, the glyph paths, keyboard
navigation and accessibility, scrolling and where the card opens.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-menu-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-menu-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/menu_card.rs` lays the card, its chrome and its key
  hints out at Interface sizes 12, 16 and 20.
