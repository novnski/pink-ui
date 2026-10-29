// PINK's floating find bar (transcript/find.rs `render_find_bar`): the
// plate that floats over the top-right of a conversation, `BAR_RIGHT` from
// its edge, as one row: the magnifier, the query field, the status ("3 of
// 7", "No results", "Searching...") and the host's Match case, Previous,
// Next and Close buttons. UI rethink decision 54: drawn here, consumed by
// PINK through a pinned revision.
//
// The package draws only. The host supplies the plate (its popover card,
// with the card's edge, corners and shadow), the opaque fill under the
// query, the glyph (its own icon asset), the query field, every colour
// already resolved, the status text and the buttons; it adds the ids, key
// context, occlusion, the Enter, Shift+Enter and Escape handling, focus and
// the search itself. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, IntoElement, SharedString, Svg, div, prelude::*, px, rems};

/// The plate's width, in pt at the 16 px default (PINK `FIND_BAR_WIDTH`).
pub const BAR_WIDTH: f32 = 420.0;
/// The room a revealed match keeps clear under the bar, in fixed pixels
/// (PINK `FIND_BAR_HEIGHT`).
pub const BAR_HEIGHT: f32 = 40.0;
/// The bar's distance from the conversation's right edge, in fixed pixels.
pub const BAR_RIGHT: f32 = 16.0;
/// The gap between the bar's parts and the plate's insets, in fixed pixels.
pub const BAR_GAP: f32 = 4.0;
pub const BAR_PAD_X: f32 = 8.0;
pub const BAR_PAD_Y: f32 = 4.0;
/// The magnifier, in pt at the 16 px default.
pub const GLYPH: f32 = 14.0;
/// The status text, in pt at the 16 px default.
pub const STATUS_TEXT: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// Where the bar floats: `top` below the conversation's top edge,
/// `BAR_RIGHT` from its right edge. The host adds the id, key context,
/// occlusion and listeners, then the plate.
pub fn anchor(top: f32) -> Div {
    div().absolute().top(px(top)).right(px(BAR_RIGHT))
}

/// Shapes the host's plate as the bar: `BAR_WIDTH` wide, filled with
/// `fill` (opaque, so the conversation never shows through the query), one
/// row `BAR_GAP` apart inside `BAR_PAD_X` / `BAR_PAD_Y`.
pub fn plate<E: Styled>(plate: E, fill: Hsla) -> E {
    plate
        .bg(fill)
        .w(ui_rems(BAR_WIDTH))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(BAR_GAP))
        .px(px(BAR_PAD_X))
        .py(px(BAR_PAD_Y))
}

/// The magnifier before the query.
pub fn glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(ui_rems(GLYPH)).flex_none().text_color(color)
}

/// The query field's room: it takes what is left.
pub fn field(input: impl IntoElement) -> Div {
    div().flex_1().min_w_0().child(input)
}

/// Where the search stands, after the query.
pub fn status(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(STATUS_TEXT))
        .text_color(color)
        .child(text.into())
}
