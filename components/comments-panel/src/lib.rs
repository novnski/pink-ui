// PINK's Comments Panel (issue #387): a document's review threads listed in
// a side pane. A toolbar carries the title, the open count and the host's
// Open / Resolved switch; under it every thread is an entry on PINK's
// comment card band (`pink-comment-card`: the 3 pt bar, the comment glyph
// and the `path:line` it is pinned to) with a byline (who opened it and
// when), the opening note clipped to its lines, and a replies line. The
// gallery's slash-command edit cards and draft card stay in the gallery:
// a host shows only the threads it has.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies every resolved colour (hover
// fills already blended), the glyphs, the names, times and text, the band,
// the switch and the actions; it adds the ids, roles and accessible names,
// focus, keys, tooltips, scrolling and every listener. Which tab is shown
// and the threads themselves stay with the host. Plain `//` comments: the
// pink-ui gallery compiles this file in place with `include!` for its
// story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The toolbar row (PINK surface chrome `toolbar`), its inset and the gap
/// between its controls, in fixed pixels.
pub const TOOLBAR_HEIGHT: f32 = 38.0;
pub const TOOLBAR_INSET: f32 = 8.0;
pub const TOOLBAR_GAP: f32 = 4.0;
/// The title starts where the entries' text does: past the 3 px bar and
/// the card's 16 px inset, measured from the toolbar's own inset.
pub const TITLE_INSET: f32 = 3.0 + 16.0 - TOOLBAR_INSET;
/// The title (medium) and the open count beside it, in pt at the 16 px
/// default, and the gap between them (fixed).
pub const TITLE_TEXT: f32 = 13.0;
pub const TITLE_LINE: f32 = 18.0;
pub const COUNT_TEXT: f32 = 11.0;
pub const COUNT_LINE: f32 = 14.0;
pub const TITLE_GAP: f32 = 6.0;
/// The host's two-option Open / Resolved switch, in fixed pixels.
pub const SWITCH_WIDTH: f32 = 150.0;
/// Above the first entry and between entries, in fixed pixels.
pub const LIST_TOP: f32 = 4.0;
pub const ENTRY_GAP: f32 = 8.0;
/// The band's vertical padding (split above and below), the slack under
/// the last row, and the header row, as the comment card's
/// (`pink-comment-card` `PAD_V`, `GAP`, `HEADER_HEIGHT`).
pub const PAD_V: f32 = 20.0;
pub const SLACK: f32 = 6.0;
pub const HEADER_HEIGHT: f32 = 22.0;
/// The byline: the author's name (medium) and the time, on one row, in pt
/// at the 16 px default; the gap between them is fixed.
pub const BYLINE_HEIGHT: f32 = 18.0;
pub const NAME_TEXT: f32 = 12.0;
pub const TIME_TEXT: f32 = 11.0;
pub const BYLINE_GAP: f32 = 6.0;
/// The opening note, as the comment card's note, in pt at the 16 px
/// default.
pub const BODY_TEXT: f32 = 12.0;
pub const LINE_HEIGHT: f32 = 18.0;
/// The replies line under the note, in pt at the 16 px default.
pub const REPLIES_HEIGHT: f32 = 18.0;
pub const REPLIES_TEXT: f32 = 11.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// An entry's band height for a note of `body_lines` lines, with or
/// without its replies line, at the host's Interface size: the padding and
/// the slack stay fixed, the header, byline, note and replies scale.
/// Worked out rather than measured, so a host can fold an entry over it.
pub fn entry_height(body_lines: usize, replies: bool, ui_font_size: f32) -> f32 {
    let rows = HEADER_HEIGHT
        + BYLINE_HEIGHT
        + body_lines as f32 * LINE_HEIGHT
        + if replies { REPLIES_HEIGHT } else { 0.0 };
    PAD_V + SLACK + rows * (ui_font_size / 16.0)
}

/// The pane's paint: its fill, text colour and the host's sans family.
pub struct PaneStyle {
    pub background: Hsla,
    pub text: Hsla,
    pub font_family: SharedString,
}

/// The pane: a full-height column the host fills with the `toolbar` and
/// its scrolling `list`. The host adds its border.
pub fn pane(style: &PaneStyle) -> Div {
    div()
        .size_full()
        .min_w_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(style.background)
        .font_family(style.font_family.clone())
        .text_color(style.text)
}

/// The toolbar row; the host adds the `heading`, then its switch.
pub fn toolbar() -> Div {
    div()
        .h(px(TOOLBAR_HEIGHT))
        .w_full()
        .flex_none()
        .px(px(TOOLBAR_INSET))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(TOOLBAR_GAP))
}

/// The title, truncating, lined up with the entries' text; the host adds
/// the `count` after it.
pub fn heading(title: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .pl(px(TITLE_INSET))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(TITLE_GAP))
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(ui_rems(TITLE_TEXT))
                .line_height(ui_rems(TITLE_LINE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(title.into()),
        )
}

/// The open threads' count beside the title; it never wraps.
pub fn count(label: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_size(ui_rems(COUNT_TEXT))
        .line_height(ui_rems(COUNT_LINE))
        .text_color(color)
        .child(label.into())
}

/// The entries' column under the toolbar. The host gives it an id and
/// makes it scroll.
pub fn list() -> Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .pt(px(LIST_TOP))
}

/// One entry's slot in the list: the host's band, then the gap to the next.
pub fn entry() -> Div {
    div().w_full().flex_none().pb(px(ENTRY_GAP))
}

/// The column inside an entry's band: the header, byline, note and
/// replies inside the card's `PAD_X` (16 px) and half of `PAD_V`, with
/// the slack under the last row.
pub fn content() -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .px(px(16.0))
        .pt(px(PAD_V / 2.0))
        .pb(px(PAD_V / 2.0 + SLACK))
}

/// The byline row; the host adds the `name` and, when known, the `time`.
pub fn byline() -> Div {
    div()
        .h(ui_rems(BYLINE_HEIGHT))
        .flex_none()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(BYLINE_GAP))
}

/// Who opened the thread, medium and truncating.
pub fn name(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(NAME_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text.into())
}

/// When, already formatted ("12m", "1h"); it never wraps.
pub fn time(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_size(ui_rems(TIME_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The opening note, exactly `lines` tall and clipping past it, so an
/// entry keeps its worked-out `entry_height`.
pub fn note(text: impl Into<SharedString>, color: Hsla, lines: usize) -> Div {
    div()
        .h(ui_rems(lines as f32 * LINE_HEIGHT))
        .flex_none()
        .min_w_0()
        .overflow_hidden()
        .text_size(ui_rems(BODY_TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(color)
        .child(text.into())
}

/// The replies line under the note ("1 reply", "3 replies"; see
/// `replies_label`), truncating.
pub fn replies(label: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .h(ui_rems(REPLIES_HEIGHT))
        .flex_none()
        .min_w_0()
        .flex()
        .items_center()
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(ui_rems(REPLIES_TEXT))
                .text_color(color)
                .child(label.into()),
        )
}

/// "1 reply", "3 replies".
pub fn replies_label(count: usize) -> String {
    if count == 1 {
        "1 reply".to_string()
    } else {
        format!("{count} replies")
    }
}
