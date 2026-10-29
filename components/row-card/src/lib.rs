// PINK's row cards (transcript/agent_cards.rs `AgentCard::render`,
// automations.rs `render_card`): the bordered card a transcript shows for a
// helper agent or an automation. It follows the settings row anatomy: a
// 28 pt tile (`pink-settings-parts` `row_tile`), a title line over a detail
// line, and the actions in one trailing column. UI rethink decision 54:
// drawn here, consumed by PINK through a pinned revision.
//
// Two layouts. `Layout::Wrapping` is the agent card: when the card is too
// narrow for the actions beside a readable title they wrap under the text,
// still at the right. `Layout::Inline` is the automation card, one line
// whose text gives way.
//
// The package draws only. The host supplies the resolved colours, the tile,
// the copy and the buttons; it adds the ids, roles and accessible names,
// tooltips and the actions. Plain `//` comments: the pink-ui gallery
// compiles this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The card's padding, in pt at the 16 px default.
pub const PAD_X: f32 = 12.0;
pub const PAD_Y: f32 = 10.0;
/// A wrapping card's corners, and the gaps between its tile, text and
/// actions across and, once the actions wrap, down.
pub const WRAPPING_RADIUS: f32 = 10.0;
pub const WRAPPING_GAP_X: f32 = 12.0;
pub const WRAPPING_GAP_Y: f32 = 8.0;
/// The narrowest a wrapping card's text gets before the actions wrap.
pub const WRAPPING_TEXT_MIN: f32 = 180.0;
pub const WRAPPING_ACTION_GAP: f32 = 4.0;
/// An inline card's corners and gap.
pub const INLINE_RADIUS: f32 = 12.0;
pub const INLINE_GAP: f32 = 10.0;
pub const INLINE_ACTION_GAP: f32 = 6.0;
/// The title line over the detail line.
pub const LINE_GAP: f32 = 2.0;
pub const TITLE_TEXT: f32 = 13.0;
pub const TITLE_LINE_HEIGHT: f32 = 18.0;
/// The title to its type badge.
pub const TITLE_GAP: f32 = 8.0;
pub const DETAIL_TEXT: f32 = 12.0;
pub const DETAIL_LINE_HEIGHT: f32 = 16.0;
/// The facts on a meta line sit this far apart: space, never a separator.
pub const META_GAP: f32 = 12.0;
/// A status dot and the gap to its word.
pub const STATUS_DOT: f32 = 6.0;
pub const STATUS_GAP: f32 = 6.0;
/// The quiet type badge after a title.
pub const BADGE_HEIGHT: f32 = 16.0;
pub const BADGE_PAD_X: f32 = 5.0;
pub const BADGE_RADIUS: f32 = 4.0;
pub const BADGE_TEXT: f32 = 11.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// How the card lays out its tile, text and actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// The actions wrap under the text when the card is narrow.
    Wrapping,
    /// One line; the text gives way.
    Inline,
}

/// What the card draws.
pub struct CardStyle {
    pub border: Hsla,
    pub background: Hsla,
}

/// The card: bordered, rounded and padded, its tile, text column and
/// actions in a row. The host adds the children.
pub fn card(layout: Layout, style: &CardStyle) -> Div {
    let card = div().w_full();
    match layout {
        Layout::Wrapping => card
            .min_w_0()
            .px(ui_rems(PAD_X))
            .py(ui_rems(PAD_Y))
            .rounded(px(WRAPPING_RADIUS))
            .border_1()
            .border_color(style.border)
            .bg(style.background)
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x(ui_rems(WRAPPING_GAP_X))
            .gap_y(ui_rems(WRAPPING_GAP_Y)),
        Layout::Inline => card
            .flex()
            .flex_row()
            .items_center()
            .gap(ui_rems(INLINE_GAP))
            .px(ui_rems(PAD_X))
            .py(ui_rems(PAD_Y))
            .rounded(px(INLINE_RADIUS))
            .border_1()
            .border_color(style.border)
            .bg(style.background),
    }
}

/// The text column between the tile and the actions: the title line over
/// the detail line.
pub fn text_column(layout: Layout) -> Div {
    let column = div().flex_1();
    match layout {
        Layout::Wrapping => column.min_w(ui_rems(WRAPPING_TEXT_MIN)),
        Layout::Inline => column.min_w_0(),
    }
    .flex()
    .flex_col()
    .gap(ui_rems(LINE_GAP))
}

/// A title line that carries a badge after the title.
pub fn title_line() -> Div {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(ui_rems(TITLE_GAP))
}

/// The title: medium, on one line.
pub fn title(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(TITLE_TEXT))
        .line_height(ui_rems(TITLE_LINE_HEIGHT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text.into())
}

/// What a type badge draws.
pub struct BadgeStyle {
    pub background: Hsla,
    pub text: Hsla,
}

/// A quiet type badge after the title: a label, never a selection or status
/// cue, and it never shrinks.
pub fn type_badge(style: &BadgeStyle, label: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .h(ui_rems(BADGE_HEIGHT))
        .px(ui_rems(BADGE_PAD_X))
        .flex()
        .items_center()
        .rounded(px(BADGE_RADIUS))
        .bg(style.background)
        .text_size(ui_rems(BADGE_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
        .child(label.into())
}

/// A one-line detail under the title.
pub fn detail(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .truncate()
        .text_size(ui_rems(DETAIL_TEXT))
        .line_height(ui_rems(DETAIL_LINE_HEIGHT))
        .text_color(color)
        .child(text.into())
}

/// A detail line of separate facts ([`status`], [`fact`], [`preview`]) set
/// apart by space.
pub fn meta_line(color: Hsla) -> Div {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(ui_rems(META_GAP))
        .text_size(ui_rems(DETAIL_TEXT))
        .line_height(ui_rems(DETAIL_LINE_HEIGHT))
        .text_color(color)
}

/// A status read like the sidebar: a coloured dot and quiet words, tinted
/// only when `tone` is given (a failure).
pub fn status(dot: Hsla, tone: Option<Hsla>, label: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(ui_rems(STATUS_GAP))
        .child(
            div()
                .flex_none()
                .size(ui_rems(STATUS_DOT))
                .rounded_full()
                .bg(dot),
        )
        .child(
            div()
                .flex_none()
                .when_some(tone, |label, tone| label.text_color(tone))
                .child(label.into()),
        )
}

/// One fact on a meta line, which never shrinks.
pub fn fact(text: impl Into<SharedString>) -> Div {
    div().flex_none().child(text.into())
}

/// The line's preview, which alone gives way when the line runs out.
pub fn preview(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .text_color(color)
        .child(text.into())
}

/// The trailing column of actions.
pub fn actions(layout: Layout) -> Div {
    match layout {
        Layout::Wrapping => div()
            .flex_none()
            .ml_auto()
            .flex()
            .items_center()
            .gap(ui_rems(WRAPPING_ACTION_GAP)),
        Layout::Inline => div()
            .flex_none()
            .flex()
            .flex_row()
            .gap(ui_rems(INLINE_ACTION_GAP)),
    }
}
