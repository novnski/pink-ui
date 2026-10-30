// PINK's Decision Inbox (Activity, issue #266): the questions and approvals
// waiting in other chats, answered from one list. The decision being
// answered opens in the question panel (`pink-question-card` inside the
// `pink-composer-pill` chrome); this package draws the rest: each waiting
// decision's one-line card (the transcript's `async_question_card` row: a
// tile with the question glyph, the question and its status) and the fact
// lines under a question (its expiry, what the provider did not share, the
// engine's refusal).
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the resolved colours, the icon
// path and the text; it adds the ids, roles and accessible names, focus,
// keyboard, hover fills and the actions. Plain `//` comments: the pink-ui
// gallery compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, rems, svg,
};

// The card (the transcript's `async_question_card`), in pt at the 16 px
// default: a `CARD_H` row with `CARD_RADIUS` corners, `CARD_GAP_Y` above and
// below it in a list.
pub const CARD_H: f32 = 34.0;
pub const CARD_GAP_Y: f32 = 4.0;
pub const CARD_PAD_X: f32 = 8.0;
pub const CARD_RADIUS: f32 = 10.0;
pub const CARD_PART_GAP: f32 = 8.0;
pub const CARD_TEXT: f32 = 12.0;
// The tile holding the question glyph.
pub const TILE: f32 = 20.0;
pub const TILE_RADIUS: f32 = 6.0;
pub const ICON: f32 = 12.0;
// The question stops growing here; the status takes the rest.
pub const TITLE_MAX_W: f32 = 320.0;

// Fact lines under a question: 11.5 pt, `FACT_GAP` apart.
pub const FACT_TEXT: f32 = 11.5;
pub const FACT_GAP: f32 = 2.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The card's resolved colours. An open decision takes an accent edge and an
/// accent status; a settled one a hairline and a muted status.
#[derive(Clone, Copy, Debug)]
pub struct CardStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub tile: Hsla,
    pub icon: Hsla,
    pub title: Hsla,
    pub status: Hsla,
}

/// One waiting decision as a one-line card: the glyph tile, the question
/// (medium weight, cut with an ellipsis once it reaches `TITLE_MAX_W` or runs
/// out of room) and its status, which takes the rest of the row. The host
/// adds the id, `Role::Button`, the accessible name, focus ring, hover fill
/// and the click that opens the decision.
pub fn card(
    style: &CardStyle,
    icon: impl Into<SharedString>,
    title: impl Into<SharedString>,
    status: impl Into<SharedString>,
) -> Div {
    div()
        .h(ui_rems(CARD_H))
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(CARD_PART_GAP))
        .overflow_hidden()
        .rounded(ui_rems(CARD_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .px(ui_rems(CARD_PAD_X))
        .text_size(ui_rems(CARD_TEXT))
        .child(
            div()
                .flex_none()
                .size(ui_rems(TILE))
                .rounded(ui_rems(TILE_RADIUS))
                .bg(style.tile)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(icon.into())
                        .flex_none()
                        .size(ui_rems(ICON))
                        .text_color(style.icon),
                ),
        )
        .child(
            div()
                .min_w_0()
                .max_w(ui_rems(TITLE_MAX_W))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.title)
                .child(title.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_color(style.status)
                .child(status.into()),
        )
}

/// The column of fact lines under a question.
pub fn facts() -> Div {
    div().w_full().flex().flex_col().gap(ui_rems(FACT_GAP))
}

/// One fact about a question in `color`: its expiry, a detail the provider
/// did not share, or the engine's refusal. It wraps; the host words it as a
/// sentence rather than a "label: value" pair.
pub fn fact(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .w_full()
        .text_size(ui_rems(FACT_TEXT))
        .text_color(color)
        .child(text.into())
}
