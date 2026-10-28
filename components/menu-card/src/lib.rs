// PINK's floating card and its rows (popover.rs `popover_card` and
// `menu_row`): the surface every menu, picker and popover opens on, and the
// one row shape its items share. UI rethink decision 54: drawn here,
// consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the card's colours, its shadow
// (none on a frosted surface) and font, and each row's text and fill already
// blended over its hover fade; it adds the id, the hover listener, the
// actions and the accessibility. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its story.

use gpui::{AbsoluteLength, BoxShadow, Div, Hsla, SharedString, div, prelude::*, px, rems};

/// The card's corner radius.
pub const CARD_RADIUS: f32 = 12.0;
/// The card's inset from its edge to its rows.
pub const CARD_INSET: f32 = 4.0;
/// The gap between rows.
pub const MENU_GAP: f32 = 2.0;
/// A row's corner radius: concentric with the card across its inset.
pub const ITEM_RADIUS: f32 = CARD_RADIUS - CARD_INSET;
/// Text size in the card and its rows, in pt at the 16 px default.
pub const TEXT: f32 = 13.0;
/// A row's gap between its parts, in pt at the 16 px default.
pub const ROW_GAP: f32 = 10.0;
/// A row's horizontal padding, in pt at the 16 px default.
pub const ROW_PAD_X: f32 = 8.0;
/// A row's vertical padding, in pt at the 16 px default.
pub const ROW_PAD_Y: f32 = 6.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the card draws.
pub struct CardStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub text: Hsla,
    pub font_family: SharedString,
    /// The card's lift; `None` on a surface that carries its own depth.
    pub shadows: Option<Vec<BoxShadow>>,
}

/// The floating card: a bordered, rounded surface with its rows stacked
/// inside `CARD_INSET`. The host chains its layout, size and children.
pub fn card(style: &CardStyle) -> Div {
    let card = div()
        .border_1()
        .border_color(style.border)
        .rounded(px(CARD_RADIUS));
    let card = match &style.shadows {
        Some(shadows) => card.shadow(shadows.clone()),
        None => card,
    };
    card.bg(style.background)
        .p(px(CARD_INSET))
        .gap(px(MENU_GAP))
        .overflow_hidden()
        .text_size(ui_rems(TEXT))
        .font_family(style.font_family.clone())
        .text_color(style.text)
}

/// What one row draws: its text colour and fill, each already resolved for
/// the selected state or blended over the hover fade.
pub struct RowStyle {
    pub text: Hsla,
    pub fill: Hsla,
}

/// One row: its parts in a line, padded and rounded to sit concentric in the
/// card. The host adds the id, the hover listener and the action.
pub fn row(style: &RowStyle) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .px(ui_rems(ROW_PAD_X))
        .py(ui_rems(ROW_PAD_Y))
        .rounded(px(ITEM_RADIUS))
        .text_size(ui_rems(TEXT))
        .cursor_pointer()
        .bg(style.fill)
        .text_color(style.text)
}
