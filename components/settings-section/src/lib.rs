// PINK's settings section anatomy (settings/widgets.rs `section_card`,
// `section_header`, `card_row`, `row_text`, `row_title`, `row_description`,
// `row_controls`, `row_block` and `section_footer`): one rounded card per
// section, its title inside it, then rows divided by hairlines, each row a
// wrapping text column and a trailing control group. UI rethink decision 54:
// drawn here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the resolved colours (the card
// tone already thinned over glass, the readable secondary tone) and adds the
// search destination plate, the controls, their listeners and the
// accessibility. Every size next to text is a rem, so a page follows the
// host's Interface size; hairlines and corners stay in pixels. Plain `//`
// comments: the pink-ui gallery compiles this file in place with `include!`
// for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// Space between sections on a page, and above the first one.
pub const SECTION_GAP: f32 = 24.0;
/// The section card's corner radius, in pixels.
pub const CARD_RADIUS: f32 = 12.0;
/// The horizontal padding of section headers and rows.
pub const ROW_PAD_X: f32 = 16.0;
/// The vertical padding of rows.
pub const ROW_PAD_Y: f32 = 12.0;
/// A row's minimum height.
pub const ROW_MIN_HEIGHT: f32 = 48.0;
/// A row's (and a header's) text column holds this width before the
/// controls wrap under it.
pub const ROW_TEXT_MIN: f32 = 220.0;
/// The horizontal gap between a row's tile, text and controls.
pub const ROW_GAP_X: f32 = 12.0;
/// The vertical gap when a row wraps its controls under its text.
pub const ROW_GAP_Y: f32 = 10.0;
/// The gap between a row's title and description.
pub const ROW_TEXT_GAP: f32 = 2.0;
/// The gap between the controls in a row's trailing group.
pub const CONTROLS_GAP: f32 = 8.0;
/// A section header's top padding.
pub const HEADER_PAD_TOP: f32 = 14.0;
/// A section header's bottom padding.
pub const HEADER_PAD_BOTTOM: f32 = 12.0;
/// The horizontal gap between a header's text and its actions.
pub const HEADER_GAP_X: f32 = 12.0;
/// The vertical gap when a header wraps its actions under its text.
pub const HEADER_GAP_Y: f32 = 8.0;
/// The gap between a header's title and description.
pub const HEADER_TEXT_GAP: f32 = 3.0;
/// The section title's size and line height (Title3, semibold).
pub const HEADER_TITLE_SIZE: f32 = 15.0;
pub const HEADER_TITLE_LINE: f32 = 20.0;
/// A row title's size and line height (medium).
pub const ROW_TITLE_SIZE: f32 = 13.0;
pub const ROW_TITLE_LINE: f32 = 18.0;
/// A row description's size and line height; also the section footer's.
pub const ROW_DESCRIPTION_SIZE: f32 = 12.0;
pub const ROW_DESCRIPTION_LINE: f32 = 16.0;
/// The space between a section card and its footer.
pub const FOOTER_GAP: f32 = 8.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the section card draws.
pub struct CardStyle {
    pub border: Hsla,
    /// The card tone, already thinned over glass by the host.
    pub background: Hsla,
}

/// The section card: `SECTION_GAP` above it, a 1 px border, `CARD_RADIUS`
/// corners, clipped, a column. Put a `section_header` inside, then
/// `card_row`s.
pub fn section_card(style: &CardStyle) -> Div {
    div()
        .mt(ui_rems(SECTION_GAP))
        .rounded(px(CARD_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .overflow_hidden()
        .flex()
        .flex_col()
}

/// A section's title block inside its card: a wrapping line holding a
/// `header_text` column and, optionally, a `row_controls` group of actions
/// on the title's line. Follow it with rows that draw their divider.
pub fn section_header() -> Div {
    div()
        .w_full()
        .px(ui_rems(ROW_PAD_X))
        .pt(ui_rems(HEADER_PAD_TOP))
        .pb(ui_rems(HEADER_PAD_BOTTOM))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_start()
        .gap_x(ui_rems(HEADER_GAP_X))
        .gap_y(ui_rems(HEADER_GAP_Y))
}

/// A header's text column: the `header_title`, then an optional
/// `row_description`, holding `ROW_TEXT_MIN` before its actions wrap.
pub fn header_text() -> Div {
    div()
        .flex_1()
        .min_w(ui_rems(ROW_TEXT_MIN))
        .flex()
        .flex_col()
        .gap(ui_rems(HEADER_TEXT_GAP))
}

/// A section title: 15/20 semibold.
pub fn header_title(color: Hsla, title: impl Into<SharedString>) -> Div {
    div()
        .text_size(ui_rems(HEADER_TITLE_SIZE))
        .line_height(ui_rems(HEADER_TITLE_LINE))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(title.into())
}

/// One card row: at least `ROW_MIN_HEIGHT` tall, padded, wrapping, with a
/// 1 px hairline on top in `divider` (none for the card's first row). Rows
/// have no hover: they aren't controls.
pub fn card_row(divider: Option<Hsla>) -> Div {
    let row = div()
        .w_full()
        .min_h(ui_rems(ROW_MIN_HEIGHT))
        .px(ui_rems(ROW_PAD_X))
        .py(ui_rems(ROW_PAD_Y));
    let row = match divider {
        Some(divider) => row.border_t_1().border_color(divider),
        None => row,
    };
    row.flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(ui_rems(ROW_GAP_X))
        .gap_y(ui_rems(ROW_GAP_Y))
}

/// A row's text column: a `row_title` and an optional `row_description`,
/// holding `ROW_TEXT_MIN` so the controls wrap below it rather than
/// squeezing the text.
pub fn row_text() -> Div {
    div()
        .flex_1()
        .min_w(ui_rems(ROW_TEXT_MIN))
        .flex()
        .flex_col()
        .gap(ui_rems(ROW_TEXT_GAP))
}

/// A row title: 13/18 medium, wrapping rather than truncating.
pub fn row_title(color: Hsla, title: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .text_size(ui_rems(ROW_TITLE_SIZE))
        .line_height(ui_rems(ROW_TITLE_LINE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(title.into())
}

/// A row description: 12/16 in the host's secondary tone, wrapping.
pub fn row_description(color: Hsla, copy: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .text_size(ui_rems(ROW_DESCRIPTION_SIZE))
        .line_height(ui_rems(ROW_DESCRIPTION_LINE))
        .text_color(color)
        .child(copy.into())
}

/// A row's trailing controls: one line pushed right, vertically centred.
// The group itself never wraps: a wrapping box that doesn't grow measures
// at its narrowest and would stack its controls; the row moves the whole
// group under the text instead, and `ml_auto` keeps it right-aligned there.
pub fn row_controls() -> Div {
    div()
        .flex_none()
        .ml_auto()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(ui_rems(CONTROLS_GAP))
}

/// A full-width block under a row's text for controls too wide for the
/// trailing group (chip groups, lists, editors). The row wraps it onto its
/// own line and its gap spaces it.
pub fn row_block() -> Div {
    div()
        .w_full()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(ui_rems(CONTROLS_GAP))
}

/// Fine print under a section card, aligned with the card's content edge.
pub fn section_footer(color: Hsla, copy: impl Into<SharedString>) -> Div {
    div()
        .mt(ui_rems(FOOTER_GAP))
        .px(ui_rems(ROW_PAD_X))
        .text_size(ui_rems(ROW_DESCRIPTION_SIZE))
        .line_height(ui_rems(ROW_DESCRIPTION_LINE))
        .text_color(color)
        .child(copy.into())
}
