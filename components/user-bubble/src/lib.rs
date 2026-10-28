// PINK's user message bubble (transcript.rs `RowKind::User`): the plate a
// sent prompt sits on, right-aligned in the transcript and capped at a
// fraction of the content column. UI rethink decision 54: drawn here,
// consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the plate's fill, its optional
// hairline edge (light appearance) as an inset shadow it builds itself
// (crates.io GPUI 0.2.2 has no inset shadows), the text colour and the
// column's width cap; it adds the prompt's text and mention chips, text
// selection, the long-prompt fold and its expander, attachments and the
// pending clock.
// Plain `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{AbsoluteLength, BoxShadow, Div, Hsla, Pixels, div, prelude::*, px, rems};

/// The plate's corner radius (PINK `Theme::BUBBLE_RADIUS`).
pub const RADIUS: f32 = 16.0;
/// Horizontal padding, in pt at the 16 px default.
pub const PAD_X: f32 = 16.0;
/// Vertical padding, in pt at the 16 px default.
pub const PAD_Y: f32 = 10.0;
/// The prompt's text size, in pt at the 16 px default.
pub const TEXT: f32 = 14.0;
/// The prompt's line box, in pt at the 16 px default.
pub const LINE_HEIGHT: f32 = 22.0;
/// The widest a bubble grows, as a fraction of the content column.
pub const MAX_WIDTH_FRACTION: f32 = 0.8;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the bubble draws.
pub struct BubbleStyle {
    pub fill: Hsla,
    /// The hairline edge: PINK draws a 1 px inset spread shadow (no layout
    /// cost, so measured heights stay exact); `None` where the fill
    /// separates on its own.
    pub edge: Option<Vec<BoxShadow>>,
    pub text: Hsla,
    /// The width cap, usually the column width times `MAX_WIDTH_FRACTION`.
    pub max_width: Pixels,
}

/// The bubble: a rounded, padded plate holding the host's prompt. It may
/// shrink below its text's unwrapped width (`min_w_0`), so a long prompt
/// wraps inside the cap rather than overflowing a right-aligned row.
pub fn bubble(style: &BubbleStyle) -> Div {
    div()
        .min_w_0()
        .max_w(style.max_width)
        .bg(style.fill)
        .when_some(style.edge.clone(), |plate, edge| plate.shadow(edge))
        .rounded(px(RADIUS))
        .px(ui_rems(PAD_X))
        .py(ui_rems(PAD_Y))
        .text_size(ui_rems(TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(style.text)
}
