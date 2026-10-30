// PINK's review comment thread (issue #373): an inline review comment grown
// into a conversation. The thread sits on PINK's comment card band
// (`pink-comment-card`: the 3 pt bar, the comment glyph and the `path:line`
// it is pinned to); this package draws what grows inside it. The header
// adds the message count and, once resolved, a Resolved chip. Every message
// is a meta row (the author's avatar disc, name and time) over its note,
// indented under the name; an agent's reply can carry a summary plate (the
// transcript's tool-group plate) that leads back to the turn it came from.
// A footer row holds the thread's actions (Reply, Resolve or Reopen).
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies every resolved colour (hover
// fills already blended), the glyphs (its own icon assets), the names,
// times and text, the band and the actions themselves; it adds the ids,
// roles and accessible names, focus, keys, tooltips and every listener.
// Thread state, replies and resolution stay with the host. Plain `//`
// comments: the pink-ui gallery compiles this file in place with `include!`
// for its story.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, SharedString, Svg, div, prelude::*, px, rems,
};

/// The author disc, in pt at the 16 px default (PINK history.rs author
/// avatar), and the initial or mark inside it.
pub const AVATAR: f32 = 20.0;
pub const AVATAR_TEXT: f32 = 11.0;
pub const AVATAR_MARK: f32 = 12.0;
/// The disc to the name and the name to the time, in fixed pixels.
pub const META_GAP: f32 = 8.0;
/// The author's name (medium) and the time, in pt at the 16 px default.
pub const NAME_TEXT: f32 = 12.0;
pub const NAME_LINE: f32 = 16.0;
pub const TIME_TEXT: f32 = 11.0;
pub const TIME_LINE: f32 = 14.0;
/// The note under the meta row: its gap (fixed), text and line height (pt
/// at the 16 px default, the comment card's note). It is indented by the
/// disc and its gap, so it lines up under the name.
pub const BODY_GAP: f32 = 2.0;
pub const BODY_TEXT: f32 = 12.0;
pub const LINE_HEIGHT: f32 = 18.0;
pub const TEXT_INDENT: f32 = AVATAR + META_GAP;
/// Between messages and above the first, in fixed pixels.
pub const MESSAGE_GAP: f32 = 12.0;
pub const LIST_TOP: f32 = 4.0;
/// The summary plate (the transcript's tool-group header plate): its row,
/// the plate inside it, the gap above it (fixed) and its glyph, label,
/// insets and corners.
pub const PLATE_ROW: f32 = 26.0;
pub const PLATE_HEIGHT: f32 = 24.0;
pub const PLATE_TOP: f32 = 4.0;
pub const PLATE_GLYPH: f32 = 14.0;
pub const PLATE_TEXT: f32 = 12.0;
pub const PLATE_PAD_L: f32 = 5.5;
pub const PLATE_PAD_R: f32 = 8.0;
pub const PLATE_GAP: f32 = 6.0;
pub const PLATE_RADIUS: f32 = 6.0;
/// The header's count and the Resolved chip, in pt at the 16 px default,
/// with the chip's insets, corners and glyph.
pub const COUNT_TEXT: f32 = 11.0;
pub const CHIP_HEIGHT: f32 = 18.0;
pub const CHIP_TEXT: f32 = 11.0;
pub const CHIP_PAD_X: f32 = 6.0;
pub const CHIP_GAP: f32 = 4.0;
pub const CHIP_RADIUS: f32 = 9.0;
pub const CHIP_GLYPH: f32 = 10.0;
/// The footer: the gap above it and its row, in fixed pixels (the comment
/// draft's action row).
pub const FOOTER_TOP: f32 = 8.0;
pub const FOOTER_HEIGHT: f32 = 28.0;
pub const FOOTER_GAP: f32 = 6.0;
/// The band's vertical padding, split above and below, and the header row,
/// as the comment card's (`pink-comment-card` `PAD_V`, `HEADER_HEIGHT`).
pub const PAD_V: f32 = 20.0;
pub const HEADER_HEIGHT: f32 = 22.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// One message's share of a worked-out thread height: its note's line
/// count and whether it carries a summary plate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MessageLines {
    pub lines: usize,
    pub plate: bool,
}

/// One message's height at the host's Interface size: the fixed gaps stay,
/// the meta row, note lines and plate scale.
pub fn message_height(message: MessageLines, ui_font_size: f32) -> f32 {
    let scale = ui_font_size / 16.0;
    let plate = if message.plate {
        PLATE_TOP + PLATE_ROW * scale
    } else {
        0.0
    };
    (AVATAR + message.lines.max(1) as f32 * LINE_HEIGHT) * scale + BODY_GAP + plate
}

/// The whole thread band's height (header, messages, optional footer) at
/// the host's Interface size. Worked out rather than measured, so a diff
/// can tween a fold over it; `body` clips each note to its `lines`.
pub fn thread_height(messages: &[MessageLines], footer: bool, ui_font_size: f32) -> f32 {
    let list: f32 = messages
        .iter()
        .map(|message| message_height(*message, ui_font_size))
        .sum();
    let gaps = MESSAGE_GAP * messages.len().saturating_sub(1) as f32;
    let footer = if footer {
        FOOTER_TOP + FOOTER_HEIGHT
    } else {
        0.0
    };
    PAD_V + HEADER_HEIGHT * (ui_font_size / 16.0) + LIST_TOP + list + gaps + footer
}

/// The first letter of a name, upper-cased (PINK history.rs
/// `history_author_initial`); "?" for a blank name.
pub fn initial(name: &str) -> SharedString {
    name.chars()
        .find(|c| !c.is_whitespace())
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".to_string())
        .into()
}

/// "1 comment", "3 comments" (PINK comments.rs `chip_label`).
pub fn count_label(count: usize) -> String {
    if count == 1 {
        "1 comment".to_string()
    } else {
        format!("{count} comments")
    }
}

/// The disc's paint: its hairline edge, its wash and the initial or mark.
pub struct AvatarStyle {
    pub edge: Hsla,
    pub fill: Hsla,
    pub content: Hsla,
}

fn disc(style: &AvatarStyle) -> Div {
    div()
        .size(ui_rems(AVATAR))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .rounded_full()
        .border_1()
        .border_color(style.edge)
        .bg(style.fill)
}

/// A person's disc holding their initial (`initial`).
pub fn initial_avatar(initial: impl Into<SharedString>, style: &AvatarStyle) -> Div {
    disc(style).child(
        div()
            .text_size(ui_rems(AVATAR_TEXT))
            .line_height(ui_rems(AVATAR))
            .text_color(style.content)
            .child(initial.into()),
    )
}

/// The agent's disc holding the host's mark glyph.
pub fn mark_avatar(mark: Svg, style: &AvatarStyle) -> Div {
    disc(style).child(
        mark.flex_none()
            .size(ui_rems(AVATAR_MARK))
            .text_color(style.content),
    )
}

/// The name and time colours.
pub struct MetaStyle {
    pub name: Hsla,
    pub time: Hsla,
}

/// The meta row: the host's disc, the name (medium, truncating) and the
/// time, which never wraps.
pub fn meta(
    avatar: Div,
    name: impl Into<SharedString>,
    time: Option<SharedString>,
    style: &MetaStyle,
) -> Div {
    div()
        .h(ui_rems(AVATAR))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(META_GAP))
        .min_w_0()
        .child(avatar)
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(ui_rems(NAME_TEXT))
                .line_height(ui_rems(NAME_LINE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.name)
                .child(name.into()),
        )
        .when_some(time, |el, time| {
            el.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(ui_rems(TIME_TEXT))
                    .line_height(ui_rems(TIME_LINE))
                    .text_color(style.time)
                    .child(time),
            )
        })
}

/// The note, indented under the name and `lines` tall (the host's estimate,
/// as `thread_height` counts it), clipping anything past it.
pub fn body(text: impl Into<SharedString>, color: Hsla, lines: usize) -> Div {
    div()
        .mt(px(BODY_GAP))
        .ml(ui_rems(TEXT_INDENT))
        .h(ui_rems(lines.max(1) as f32 * LINE_HEIGHT))
        .overflow_hidden()
        .min_w_0()
        .text_size(ui_rems(BODY_TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(color)
        .child(text.into())
}

/// One message: the host adds `meta`, `body` and, for an agent's reply,
/// `plate`.
pub fn message() -> Div {
    div().flex_none().flex().flex_col().min_w_0()
}

/// The conversation under the header, `MESSAGE_GAP` apart.
pub fn messages() -> Div {
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(MESSAGE_GAP))
        .pt(px(LIST_TOP))
}

/// The summary plate's paint (the host blends its hover fill and label).
pub struct PlateStyle {
    pub background: Hsla,
    pub glyph: Hsla,
    pub text: Hsla,
}

/// The agent's summary plate, indented under the name: the host's glyph and
/// the label, truncating. The host makes it a button that opens the turn.
pub fn plate(glyph: Svg, label: impl Into<SharedString>, style: &PlateStyle) -> Div {
    div()
        .mt(px(PLATE_TOP))
        .ml(ui_rems(TEXT_INDENT))
        .h(ui_rems(PLATE_ROW))
        .flex()
        .flex_row()
        .items_center()
        .min_w_0()
        .child(
            div()
                .h(ui_rems(PLATE_HEIGHT))
                .max_w_full()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(PLATE_GAP))
                .pl(px(PLATE_PAD_L))
                .pr(px(PLATE_PAD_R))
                .rounded(px(PLATE_RADIUS))
                .bg(style.background)
                .text_size(ui_rems(PLATE_TEXT))
                .line_height(ui_rems(LINE_HEIGHT))
                .text_color(style.text)
                .child(
                    glyph
                        .flex_none()
                        .size(ui_rems(PLATE_GLYPH))
                        .text_color(style.glyph),
                )
                .child(div().min_w_0().truncate().child(label.into())),
        )
}

/// The header's message count (`count_label`), which never wraps.
pub fn count(label: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .whitespace_nowrap()
        .text_size(ui_rems(COUNT_TEXT))
        .line_height(ui_rems(TIME_LINE))
        .text_color(color)
        .child(label.into())
}

/// The Resolved chip's paint.
pub struct ChipStyle {
    pub background: Hsla,
    pub text: Hsla,
}

/// The header's state chip: the host's glyph and label on a pill.
pub fn chip(glyph: Svg, label: impl Into<SharedString>, style: &ChipStyle) -> Div {
    div()
        .flex_none()
        .h(ui_rems(CHIP_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(CHIP_GAP))
        .px(px(CHIP_PAD_X))
        .rounded(px(CHIP_RADIUS))
        .bg(style.background)
        .whitespace_nowrap()
        .text_size(ui_rems(CHIP_TEXT))
        .line_height(ui_rems(TIME_LINE))
        .text_color(style.text)
        .child(
            glyph
                .flex_none()
                .size(ui_rems(CHIP_GLYPH))
                .text_color(style.text),
        )
        .child(label.into())
}

/// The footer row under the conversation; the host adds its actions
/// (`pink-comment-card` `action`), trailing.
pub fn footer() -> Div {
    div()
        .mt(px(FOOTER_TOP))
        .h(px(FOOTER_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(px(FOOTER_GAP))
}
