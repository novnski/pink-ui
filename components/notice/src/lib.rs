// PINK's in-context notice (notice.rs `notice_chip` and `notice_action`):
// the opaque plate a composer notice floats on above the pill, its title row
// (the tone glyph, the medium title, the actions and the Copy message
// button), the message under it, and the capsule action a notice offers.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the colours already resolved
// (the tone flattened over its canvas, the edge, the readable ink, the
// action's border and text), its glyphs and the copy; it adds the ids, the
// roles, accessible names and focus rings, the hover washes, the clipboard
// write and the listeners. Plain `//` comments: the pink-ui gallery compiles
// this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The plate's corner radius: the stack ladder's notice step above the pill
/// (PINK popover.rs `STACK_NOTICE_RADIUS`). Fixed, like every radius.
pub const PLATE_RADIUS: f32 = 14.0;
/// The gap between the title row and the message.
pub const PLATE_GAP: f32 = 6.0;
/// The plate's padding.
pub const PLATE_PAD_X: f32 = 10.0;
pub const PLATE_PAD_Y: f32 = 8.0;
/// The notice's copy, in pt at the 16 px default.
pub const TEXT: f32 = 12.0;
pub const LINE_HEIGHT: f32 = 16.0;
/// The gap between the title row's parts.
pub const ROW_GAP: f32 = 8.0;
/// The tone glyph before the title.
pub const GLYPH: f32 = 14.0;
/// The Copy message button, its radius and glyph.
pub const COPY_SIZE: f32 = 20.0;
pub const COPY_RADIUS: f32 = 6.0;
pub const COPY_GLYPH: f32 = 12.0;
/// A notice action: a capsule in the title row.
pub const ACTION_HEIGHT: f32 = 24.0;
pub const ACTION_PAD_X: f32 = 10.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a notice plate draws, already resolved by the host.
pub struct NoticeStyle {
    /// The opaque plate: the tone's light tint flattened over the canvas, so
    /// rows scrolling under it cannot print through.
    pub plate: Hsla,
    /// The plate's edge.
    pub edge: Hsla,
    /// The copy and the glyph: the muted tone held readable.
    pub ink: Hsla,
}

/// The plate: the title row over the message, padded and rounded on the
/// notice step. The host adds the title row and the message.
pub fn plate(style: &NoticeStyle) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(ui_rems(PLATE_GAP))
        .rounded(px(PLATE_RADIUS))
        .border_1()
        .border_color(style.edge)
        .bg(style.plate)
        .px(ui_rems(PLATE_PAD_X))
        .py(ui_rems(PLATE_PAD_Y))
        .text_size(ui_rems(TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(style.ink)
}

/// The title row. A narrow pane or a large Interface size wraps the actions
/// under the title instead of pushing them past the plate. The host adds the
/// glyph (`GLYPH` rems), the [`title`], a [`spacer`], the actions and the
/// [`copy_button`].
pub fn title_row() -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(ui_rems(ROW_GAP))
}

/// The notice's title, medium weight.
pub fn title(label: impl Into<SharedString>) -> Div {
    div().font_weight(FontWeight::MEDIUM).child(label.into())
}

/// Pushes the actions to the row's trailing edge.
pub fn spacer() -> Div {
    div().flex_1()
}

/// The Copy message button's frame. The host adds the id, the role and
/// name, the hover fill, the click and its glyph (`COPY_GLYPH` rems).
pub fn copy_button() -> Div {
    div()
        .flex_none()
        .size(ui_rems(COPY_SIZE))
        .rounded(px(COPY_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
}

/// The message on its own line, wrapping inside the plate. The host adds
/// the id and the alert role that announces title and message once.
pub fn message(text: impl Into<SharedString>) -> Div {
    div().min_w_0().w_full().child(text.into())
}

/// What a notice action draws.
pub struct ActionStyle {
    /// The capsule's edge.
    pub border: Hsla,
    /// The label's colour.
    pub text: Hsla,
}

/// A notice action: a 24 pt capsule in the title row. The host adds the id,
/// the role, name and tab stop, the hover wash, the focus ring, the click
/// and the label.
pub fn action(style: &ActionStyle) -> Div {
    div()
        .flex_none()
        .h(ui_rems(ACTION_HEIGHT))
        .px(ui_rems(ACTION_PAD_X))
        .flex()
        .items_center()
        .rounded_full()
        .border_1()
        .border_color(style.border)
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
        .cursor_pointer()
}
