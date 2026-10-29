// PINK's file badge (transcript/file_badges.rs `render_file_badge`): the
// chip a tool header shows for the file a tool read or wrote, with the
// file-type artwork in a counter-shaded well and the file's name. The same
// well, in Interface-scaled units, sits beside a Markdown paragraph that is
// one workspace file (markdown/render.rs): see `scaled_well` and
// `file_link`. UI rethink decision 54: drawn here, consumed by PINK through
// a pinned revision.
//
// The package draws only. The host supplies the resolved colours (the
// chip's fill already blended over its hover fade when the badge opens the
// file), the file-type artwork and the name; it adds the id, the hover
// listener, the role and accessible name, the tooltip, the frosted backdrop
// and the action. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, IntoElement, SharedString, div, prelude::*, px, rems};

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
/// A Markdown file link's well to its text, in pt at the 16 px default.
pub const LINK_GAP: f32 = 4.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

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

/// The well in Interface-scaled units, for a Markdown file link: `WELL`
/// and the host's artwork (sized `ui_rems(GLYPH)`) grow with the Interface
/// size beside the paragraph's text; the `WELL_RADIUS` corners stay fixed.
pub fn scaled_well(background: Hsla, artwork: impl IntoElement) -> Div {
    div()
        .size(ui_rems(WELL))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(WELL_RADIUS))
        .bg(background)
        .child(artwork)
}

/// A Markdown paragraph that is one workspace file: the host's
/// `scaled_well`, then the paragraph's text, `LINK_GAP` apart. The text
/// takes the rest of the width and may wrap; the well stays centred on it.
pub fn file_link(well: impl IntoElement, text: impl IntoElement) -> Div {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(ui_rems(LINK_GAP))
        .child(well)
        .child(div().min_w_0().flex_1().child(text))
}
