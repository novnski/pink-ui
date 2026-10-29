// PINK's file badge (transcript/file_badges.rs `render_file_badge`): the
// chip a tool header shows for the file a tool read or wrote, with the
// file-type artwork in a counter-shaded well and the file's name. UI
// rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours (the
// chip's fill already blended over its hover fade when the badge opens the
// file), the file-type artwork and the name; it adds the id, the hover
// listener, the role and accessible name, the tooltip, the frosted backdrop
// and the action. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{Div, Hsla, IntoElement, SharedString, div, prelude::*, px};

/// The chip's height and corners.
pub const HEIGHT: f32 = 22.0;
pub const RADIUS: f32 = 6.0;
/// The well sits 1 pt in from the chip's leading edge; the name ends 6 pt
/// from its trailing edge.
pub const PAD_LEFT: f32 = 1.0;
pub const PAD_RIGHT: f32 = 6.0;
/// The well to the name.
pub const GAP: f32 = 6.0;
/// The well and its corners.
pub const WELL: f32 = 20.0;
pub const WELL_RADIUS: f32 = 4.0;
/// The file-type artwork the host puts in the well.
pub const GLYPH: f32 = 14.0;

/// What the chip draws.
pub struct BadgeStyle {
    /// The chip's fill, blended over the hover fade when it is a control.
    pub background: Hsla,
    /// The name's colour.
    pub text: Hsla,
}

/// The chip: the well and the name in a line, clipped to its corners. It
/// may shrink, truncating the name.
pub fn badge(style: &BadgeStyle) -> Div {
    div()
        .min_w_0()
        .h(px(HEIGHT))
        .flex()
        .items_center()
        .overflow_hidden()
        .gap(px(GAP))
        .rounded(px(RADIUS))
        .bg(style.background)
        .pl(px(PAD_LEFT))
        .pr(px(PAD_RIGHT))
        .text_color(style.text)
}

/// The well: the host's file-type artwork (sized [`GLYPH`]) centred on its
/// counter-shade.
pub fn well(background: Hsla, artwork: impl IntoElement) -> Div {
    div()
        .size(px(WELL))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(WELL_RADIUS))
        .bg(background)
        .child(artwork)
}

/// The file's name, on one line.
pub fn name(text: impl Into<SharedString>) -> Div {
    div().min_w_0().truncate().child(text.into())
}
