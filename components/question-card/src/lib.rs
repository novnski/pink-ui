// PINK's question card (composer.rs `render_wizard`): the panel an agent's
// question, approval or permission request takes in the composer's place.
// Its body stacks the tracked header with the page counter, the question,
// the approval context as label-over-value rows, the multi-select hint, the
// numbered option rows and the free answer field; its footer holds the notes
// (countdown, async note) and the actions. UI rethink decision 54: drawn
// here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the surface the card sits on
// (PINK's composer pill), the colours already resolved (each option's fill
// blended over its hover fade, the picked wash), the text already cased and
// the buttons; it adds the roles, accessible names, focus rings, hover
// listeners, keyboard handling and the actions. Plain `//` comments: the
// pink-ui gallery compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, ElementId, FontWeight, Hsla, SharedString, Stateful, div, prelude::*, px,
    rems,
};

/// The body's and the footer's inset from the card's edges.
pub const PAD: f32 = 16.0;
/// The header's text, and its gap to the page counter.
pub const HEADER_TEXT: f32 = 11.0;
pub const HEADER_GAP: f32 = 10.0;
/// The page counter chip ("1 of 2") beside the header.
pub const COUNTER_HEIGHT: f32 = 20.0;
pub const COUNTER_PAD_X: f32 = 6.0;
/// The question under the header: its gap, type and scroll cap.
pub const QUESTION_GAP: f32 = 6.0;
pub const QUESTION_TEXT: f32 = 15.0;
pub const QUESTION_LINE_HEIGHT: f32 = 20.0;
pub const QUESTION_MAX_HEIGHT: f32 = 160.0;
/// The approval context under the question: its gap, scroll cap and the
/// gap between its label-over-value rows.
pub const CONTEXT_GAP: f32 = 8.0;
pub const CONTEXT_MAX_HEIGHT: f32 = 220.0;
pub const CONTEXT_ROW_GAP: f32 = 6.0;
pub const CONTEXT_LABEL_TEXT: f32 = 11.0;
pub const CONTEXT_VALUE_TEXT: f32 = 12.0;
/// The multi-select hint: its gap above and its text.
pub const HINT_GAP: f32 = 4.0;
pub const HINT_TEXT: f32 = 12.0;
/// The option list: its gap, scroll cap and the gap between rows.
pub const OPTIONS_GAP: f32 = 12.0;
pub const OPTIONS_MAX_HEIGHT: f32 = 220.0;
pub const OPTION_GAP: f32 = 4.0;
/// One option row: its padding, the gap between its text and number chip,
/// and its type.
pub const OPTION_PAD_X: f32 = 14.0;
pub const OPTION_PAD_Y: f32 = 10.0;
pub const OPTION_PART_GAP: f32 = 12.0;
pub const OPTION_TEXT: f32 = 13.5;
pub const OPTION_DETAIL_TEXT: f32 = 11.5;
pub const OPTION_LINE_GAP: f32 = 3.0;
/// An option row's corner radius. Fixed, like every radius.
pub const OPTION_RADIUS: f32 = 12.0;
/// The number chip that names an option's 1 to 9 key.
pub const NUMBER_SIZE: f32 = 22.0;
pub const NUMBER_TEXT: f32 = 11.0;
/// The number chip's and the page counter's corner radius.
pub const CHIP_RADIUS: f32 = 6.0;
/// The answer field: its gap above, its inset around the host's surface and
/// the field's padding inside it.
pub const ANSWER_GAP: f32 = 12.0;
pub const ANSWER_INSET: f32 = 4.0;
pub const ANSWER_PAD_X: f32 = 8.0;
pub const ANSWER_PAD_Y: f32 = 12.0;
/// The footer: the gap between its notes and actions, and its top pad.
pub const FOOTER_GAP: f32 = 12.0;
pub const FOOTER_TOP: f32 = 4.0;
/// The gap between the footer's notes and secondary actions.
pub const NOTES_GAP: f32 = 8.0;
pub const NOTE_TEXT: f32 = 12.0;
/// The primary action's horizontal padding (the host's button).
pub const ACTION_PAD_X: f32 = 16.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The card's text colours, resolved by the host.
pub struct Palette {
    /// The question and each context value.
    pub text: Hsla,
    /// The header and the page counter's text.
    pub header: Hsla,
    /// The page counter's fill.
    pub counter_fill: Hsla,
    /// Context labels and the multi-select hint.
    pub detail: Hsla,
    /// The footer's notes (countdown, async note).
    pub note: Hsla,
}

/// The card's body: the header, question, context, hint, options and answer
/// stacked inside `PAD`. The host chains its children.
pub fn body() -> Div {
    div()
        .px(ui_rems(PAD))
        .pt(ui_rems(PAD))
        .flex()
        .flex_col()
}

/// The header: the host's tracked uppercase title and, on a paged request,
/// the page counter chip.
pub fn header(
    palette: &Palette,
    title: impl Into<SharedString>,
    counter: Option<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(HEADER_GAP))
        .child(
            div()
                .text_size(ui_rems(HEADER_TEXT))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.header)
                .child(title.into()),
        )
        .when_some(counter, |el, counter| {
            el.child(
                div()
                    .h(ui_rems(COUNTER_HEIGHT))
                    .px(ui_rems(COUNTER_PAD_X))
                    .flex()
                    .items_center()
                    .rounded(px(CHIP_RADIUS))
                    .bg(palette.counter_fill)
                    .text_size(ui_rems(HEADER_TEXT))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(palette.header)
                    .child(counter),
            )
        })
}

/// The question, scrolling past `QUESTION_MAX_HEIGHT`.
pub fn question(
    id: impl Into<ElementId>,
    palette: &Palette,
    text: impl Into<SharedString>,
) -> Stateful<Div> {
    div()
        .id(id)
        .max_h(ui_rems(QUESTION_MAX_HEIGHT))
        .overflow_y_scroll()
        .mt(ui_rems(QUESTION_GAP))
        .text_size(ui_rems(QUESTION_TEXT))
        .line_height(ui_rems(QUESTION_LINE_HEIGHT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(palette.text)
        .child(text.into())
}

/// The approval context: each `(label, value)` as a quiet label over its
/// value, scrolling past `CONTEXT_MAX_HEIGHT`.
pub fn context(
    id: impl Into<ElementId>,
    palette: &Palette,
    rows: impl IntoIterator<Item = (SharedString, SharedString)>,
) -> Stateful<Div> {
    let (detail, text) = (palette.detail, palette.text);
    div()
        .id(id)
        .mt(ui_rems(CONTEXT_GAP))
        .max_h(ui_rems(CONTEXT_MAX_HEIGHT))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(ui_rems(CONTEXT_ROW_GAP))
        .children(rows.into_iter().map(move |(label, value)| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(ui_rems(CONTEXT_LABEL_TEXT))
                        .text_color(detail)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(ui_rems(CONTEXT_VALUE_TEXT))
                        .text_color(text)
                        .child(value),
                )
        }))
}

/// The multi-select hint under the question.
pub fn hint(palette: &Palette, text: impl Into<SharedString>) -> Div {
    div()
        .mt(ui_rems(HINT_GAP))
        .text_size(ui_rems(HINT_TEXT))
        .text_color(palette.detail)
        .child(text.into())
}

/// The option list, scrolling past `OPTIONS_MAX_HEIGHT`. The host chains
/// its rows.
pub fn options(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .max_h(ui_rems(OPTIONS_MAX_HEIGHT))
        .overflow_y_scroll()
        .mt(ui_rems(OPTIONS_GAP))
        .flex()
        .flex_col()
        .gap(ui_rems(OPTION_GAP))
}

/// What one option row draws, each colour already resolved for the picked
/// state or blended over the hover fade.
pub struct OptionStyle {
    /// The row's edge: a faint ink when picked, transparent at rest (the
    /// host's focus ring recolours it).
    pub border: Hsla,
    pub fill: Hsla,
    pub label: Hsla,
    /// The detail lines under the label.
    pub detail: Hsla,
    pub number_fill: Hsla,
    pub number_text: Hsla,
}

/// One option: its label over its detail lines and, when it has a key, the
/// number chip naming it. The host adds the id, role, accessible state,
/// focus ring, hover listener and the pick.
pub fn option(
    style: &OptionStyle,
    label: impl Into<SharedString>,
    details: impl IntoIterator<Item = SharedString>,
    number: Option<usize>,
) -> Div {
    let detail = style.detail;
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(OPTION_PART_GAP))
        .px(ui_rems(OPTION_PAD_X))
        .py(ui_rems(OPTION_PAD_Y))
        .rounded(px(OPTION_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.fill)
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(ui_rems(OPTION_TEXT))
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.label)
                .flex()
                .flex_col()
                .gap(ui_rems(OPTION_LINE_GAP))
                .child(label.into())
                .children(details.into_iter().map(move |line| {
                    div()
                        .text_size(ui_rems(OPTION_DETAIL_TEXT))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(detail)
                        .child(line)
                })),
        )
        .when_some(number, |el, number| {
            el.child(
                div()
                    .flex_none()
                    .size(ui_rems(NUMBER_SIZE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(CHIP_RADIUS))
                    .bg(style.number_fill)
                    .text_size(ui_rems(NUMBER_TEXT))
                    .text_color(style.number_text)
                    .child(SharedString::from(number.to_string())),
            )
        })
}

/// The free answer: the host's surface (PINK's composer pill, nested in the
/// card) inset by `ANSWER_INSET`, holding the host's field padded by
/// `ANSWER_PAD_X` by `ANSWER_PAD_Y`.
pub fn answer<S>(surface: S, field: impl IntoElement) -> Div
where
    S: Styled + ParentElement + IntoElement,
{
    div()
        .mt(ui_rems(ANSWER_GAP))
        .pb(ui_rems(ANSWER_INSET))
        .px(ui_rems(ANSWER_INSET))
        .child(
            surface.w_full().overflow_hidden().child(
                div()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .px(ui_rems(ANSWER_PAD_X))
                    .py(ui_rems(ANSWER_PAD_Y))
                    .child(field),
            ),
        )
}

/// The footer: the notes on the leading edge, the primary action on the
/// trailing one. The host chains `notes()` and its button (padded by
/// `ACTION_PAD_X`).
pub fn footer() -> Div {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .gap(ui_rems(FOOTER_GAP))
        .px(ui_rems(PAD))
        .pb(ui_rems(PAD))
        .pt(ui_rems(FOOTER_TOP))
}

/// The footer's leading slot: notes and secondary actions in a line. It
/// fills the footer, so the primary action keeps the trailing edge even when
/// the slot is empty.
pub fn notes() -> Div {
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(ui_rems(NOTES_GAP))
}

/// One footer note (the countdown, the async note). The host adds its id.
pub fn note(palette: &Palette, text: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .text_size(ui_rems(NOTE_TEXT))
        .text_color(palette.note)
        .child(text.into())
}
