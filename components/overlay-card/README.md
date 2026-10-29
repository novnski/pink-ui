# Overlay card

`pink-overlay-card` is PINK's overlay cards as a single-GPUI package (UI
rethink decision 54): the tooltip card, the dialog card a modal lays on its
scrim with its warning paint, and the full-width popup above the composer
(PINK popover.rs). Like the other packages under `components/` it depends on
crates.io `gpui = "=0.2.2"` in its own workspace. The pink-ui gallery
compiles this same `src/lib.rs` in place (`src/components/overlay_card.rs`),
and the kit's `pink::tooltip_card`, `pink::TextTooltip`, `pink::dialog_card`
and `pink::stack_popup_card` draw from it. PINK pins the package and patches
`gpui` to its vendored runtime, so there is one GPUI type graph and no copied
source.

## What it draws

- `tooltip_card(&TooltipStyle)`: the card every tooltip and rich hover card
  draws on, `TOOLTIP_RADIUS` corners, a 1 px edge, an opaque fill, the
  host's shadows and the font named on the card (a tooltip lays out as its
  own root).
- `text_tooltip(&TooltipStyle, text, label)`: the plain-text tooltip, padded
  `TOOLTIP_PAD_X` by `TOOLTIP_PAD_Y`, on one line up to
  `TOOLTIP_WRAP_CHARS` characters and wrapped at `TOOLTIP_MAX_WIDTH` past
  it (`tooltip_wraps`). The host chains its caption type style.
- `above(card)`: the zero-size root of a tooltip shown above its trigger,
  centring the card over the anchor point and ending it there.
- `dialog_card(&DialogStyle)`: the modal dialog's card, `DIALOG_WIDTH` wide,
  padded `DIALOG_PAD`, `MODAL_RADIUS` corners with a 1 px edge, a column,
  with a large drop shadow only when the host says the plate is opaque.
- `ModalWarning`, `warning_wash`, `warning_border` and
  `warning_notice(&ModalWarning)`: a warning notice's paint inside a
  dialog, the tone at `WARNING_WASH` and `WARNING_BORDER`, with copy the
  host has floored for contrast.
- `stack_popup(card)`: the host's menu card on the stack ladder's popup
  step, `STACK_POPUP_RADIUS` corners with its rows `STACK_POPUP_INSET` in
  (rows round to `STACK_POPUP_ROW_RADIUS`).
- `ui_rems(design)`: a design size (pt at the 16 px default) in rems.
  Tooltip padding and wrap width and the dialog's width and padding grow
  with the host's Interface size; corners, edges and the popup inset stay
  fixed.

The host adds the rest: the colours, the caption type, the scrim, the
frost and its corner, the dialog entrance, where each card opens, the ids,
listeners, focus and accessibility.

## How PINK pins and consumes it

In `crates/ui/Cargo.toml`, beside the other pink-ui packages:

```toml
pink-overlay-card = { git = "https://github.com/novnski/pink-ui", rev = "<merged main commit>", package = "pink-overlay-card" }
```

## Checks

- Alone: `cargo clippy --features gpui/runtime_shaders -- -D warnings` in this
  directory.
- In the gallery: `tests/overlay_card.rs` lays the cards out at Interface
  sizes 12, 16 and 20 and checks the above-the-trigger root.
