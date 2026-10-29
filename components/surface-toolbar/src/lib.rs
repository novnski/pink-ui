// PINK's surface toolbar (surface_chrome.rs `toolbar` and `input`, files/mod.rs
// `render_header` and `toolbar_button`): the control row a sidebar surface
// carries under its titlebar, as tall as the workbench tab strip, on the
// shared chrome surface with no band or border of its own; the rounded search
// field that takes the row's free width, with its magnifier and query; and
// the glyph a toolbar control carries. The controls themselves are
// `pink-icon-button` squares at `CONTROL_SIZE` and `CONTROL_RADIUS`. UI
// rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the query field, the glyphs (its
// own icon assets), every colour already resolved (a control's wash already
// blended over its hover fade, or held while the control is latched), and
// adds the ids, roles, accessible names, toggled states, focus rings,
// tooltips, hover listeners, occlusion, clicks and the search itself. Plain
// `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, IntoElement, Svg, div, prelude::*, px, rems};

/// The row's height, in fixed pixels (PINK `surface_chrome::HEADER_HEIGHT`,
/// the titlebar height), so both sidebars' second rows line up.
pub const HEIGHT: f32 = 38.0;
/// The row's side inset, in fixed pixels (PINK `EDGE_INSET`).
pub const EDGE_INSET: f32 = 8.0;
/// The gap between the row's controls, in fixed pixels (PINK `CONTROL_GAP`).
pub const CONTROL_GAP: f32 = 4.0;
/// A toolbar control's side and corner, in fixed pixels (PINK
/// `CONTROL_SIZE`, `CONTROL_RADIUS`): the control ladder's medium step.
pub const CONTROL_SIZE: f32 = 28.0;
pub const CONTROL_RADIUS: f32 = 6.0;
/// A toolbar control's glyph, in fixed pixels (PINK `ICON_SIZE`).
pub const CONTROL_GLYPH: f32 = 16.0;
/// The search field's height, side pad, gap and type, in pt at the 16 px
/// default: the field grows with the Interface size, its corner stays fixed.
pub const FIELD_HEIGHT: f32 = 28.0;
pub const FIELD_PAD_X: f32 = 8.0;
pub const FIELD_GAP: f32 = 6.0;
pub const FIELD_TEXT: f32 = 12.0;
/// The field's magnifier, in pt at the 16 px default.
pub const FIELD_GLYPH: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The control row: `HEIGHT` tall and full width, its children centred
/// `CONTROL_GAP` apart inside `EDGE_INSET`.
pub fn toolbar() -> Div {
    div()
        .h(px(HEIGHT))
        .w_full()
        .flex_none()
        .px(px(EDGE_INSET))
        .flex()
        .items_center()
        .gap(px(CONTROL_GAP))
}

/// The search field: a `FIELD_HEIGHT` plate on `fill` taking the row's free
/// width, its glyph and query `FIELD_GAP` apart in `FIELD_TEXT` type.
pub fn field(fill: Hsla) -> Div {
    div()
        .h(ui_rems(FIELD_HEIGHT))
        .min_w_0()
        .flex_1()
        .px(ui_rems(FIELD_PAD_X))
        .rounded(px(CONTROL_RADIUS))
        .bg(fill)
        .flex()
        .items_center()
        .gap(ui_rems(FIELD_GAP))
        .text_size(ui_rems(FIELD_TEXT))
}

/// The field's leading magnifier.
pub fn field_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph
        .size(ui_rems(FIELD_GLYPH))
        .flex_none()
        .text_color(color)
}

/// The query's room in the field: it takes what is left.
pub fn field_input(input: impl IntoElement) -> Div {
    div().min_w_0().flex_1().child(input)
}

/// A toolbar control's glyph, `CONTROL_GLYPH` square in `color`.
pub fn control_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(px(CONTROL_GLYPH)).text_color(color)
}
