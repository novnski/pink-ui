# Menu card

`pink-menu-card` is PINK's floating card and its rows as a single-GPUI
package (UI rethink decision 54). Like the other packages under `components/`
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
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems. Row
  padding and text grow with the host's Interface size; the card's inset,
  gap and corners stay fixed.

The host adds the rest: the id, the hover listener and its fade, the
selected wash, the actions, keyboard navigation and accessibility, scrolling
and where the card opens.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-menu-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-menu-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/menu_card.rs` lays the card out at Interface sizes
  12, 16 and 20.
