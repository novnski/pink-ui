// PINK's review comment cards (comment_ui.rs, files/preview.rs): the notes a
// reviewer pins to a line of the Changes diff, a Markdown preview or the
// code editor before sending them to the agent. A posted card is a flat band
// with a 3 pt accent bar, the comment glyph and the `path:line` it cites,
// edit and remove revealed under the pointer, and the note; the open draft
// is a firmer band holding the host's text field and Cancel and Comment
// (Save when editing), as tall as its contents. The code editor floats the same
// header and note on the host's popover card instead of a band. The adder is
// the small `+` plate a line shows to start a draft. UI rethink decision
// 54: drawn here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies every resolved colour (a ghost
// action's label and wash already blended over its hover fade), the glyphs
// (its own icon assets), the location and note text, the text field and,
// for a floating card, the popover card itself with its frost; it adds the
// ids, the hover group's name, roles and accessible names, focus, keys and
// every action. Comment state, editing and the analytic body-line estimate
// stay with the host. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, IntoElement, SharedString, Svg, div, prelude::*, px,
    rems,
};

/// The accent bar, in fixed pixels. A bar, not a border: PINK's diff rows
/// carry the same column (`pink-diff-rows` `ACCENT_BAR_WIDTH`), so a card's
/// edge never steps in or out of the diff.
pub const ACCENT_BAR_WIDTH: f32 = 3.0;
/// The content's side inset and the vertical padding, split above and
/// below, in fixed pixels (PINK `Theme::SPACE_LG`, comments.rs
/// `CARD_PAD_V`).
pub const PAD_X: f32 = 16.0;
pub const PAD_V: f32 = 20.0;
/// The header row, in pt at the 16 px default (comments.rs
/// `CARD_HEADER_HEIGHT`).
pub const HEADER_HEIGHT: f32 = 22.0;
/// The glyph to the location and the controls, in fixed pixels.
pub const HEADER_GAP: f32 = 6.0;
/// The comment, pen and remove glyphs, in fixed pixels.
pub const GLYPH: f32 = 12.0;
/// The location's mono text, in pt at the 16 px default.
pub const LOCATION_TEXT: f32 = 11.0;
/// The note's text and line height, in pt at the 16 px default
/// (comments.rs `CARD_LINE_HEIGHT`).
pub const BODY_TEXT: f32 = 12.0;
pub const LINE_HEIGHT: f32 = 18.0;
/// The slack under a posted card's note, in fixed pixels (comments.rs
/// `CARD_GAP`).
pub const GAP: f32 = 6.0;
/// The open draft's height at the 16 px default (comments.rs
/// `DRAFT_CARD_HEIGHT`); `draft_height` gives it at other Interface sizes.
pub const DRAFT_HEIGHT: f32 = PAD_V + HEADER_HEIGHT + FIELD_HEIGHT + ACTIONS_HEIGHT;
/// The draft's field slot and its action row, in fixed pixels.
pub const FIELD_HEIGHT: f32 = 46.0;
pub const ACTIONS_HEIGHT: f32 = 28.0;
pub const ACTIONS_GAP: f32 = 6.0;
/// A draft action: height, side padding and corners in fixed pixels, its
/// label in pt at the 16 px default (`scaled_action` scales the box too).
pub const ACTION_HEIGHT: f32 = 22.0;
pub const ACTION_PAD_X: f32 = 10.0;
pub const ACTION_RADIUS: f32 = 6.0;
pub const ACTION_TEXT: f32 = 11.0;
/// The corners of the adder, edit and remove, in fixed pixels.
pub const CONTROL_RADIUS: f32 = 4.0;
/// The adder plate and its plus, in fixed pixels (comments.rs
/// `COMMENT_ADDER_SIZE`).
pub const ADDER: f32 = 16.0;
pub const ADDER_GLYPH: f32 = 11.0;
/// Edit's box, in fixed pixels.
pub const EDIT: f32 = 16.0;
/// Remove's hit box and the margin that keeps its footprint at 16 pt
/// (PINK DS-19), in fixed pixels.
pub const REMOVE_HIT: f32 = 24.0;
pub const REMOVE_INSET: f32 = 4.0;
/// The floating draft (the code editor's): its vertical padding, and its
/// framed field's height, corners and insets, in fixed pixels.
pub const FLOATING_DRAFT_PAD_V: f32 = 16.0;
pub const FLOATING_FIELD_HEIGHT: f32 = 48.0;
pub const FLOATING_FIELD_RADIUS: f32 = 7.0;
pub const FLOATING_FIELD_PAD_X: f32 = 8.0;
pub const FLOATING_FIELD_PAD_Y: f32 = 5.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// A posted card's height for a note of `body_lines` lines at the host's
/// Interface size (comments.rs `card_height_for`): the padding and the
/// slack stay fixed, the header and lines scale. Worked out rather than
/// measured, so a diff can tween a fold over it.
pub fn card_height(body_lines: usize, ui_font_size: f32) -> f32 {
    PAD_V + GAP + (HEADER_HEIGHT + body_lines as f32 * LINE_HEIGHT) * (ui_font_size / 16.0)
}

/// The open draft's height at the host's Interface size: the padding, the
/// field and the action row stay fixed and only the header scales, so the
/// band always holds them. Worked out rather than measured, so an open
/// draft never fights a fold tween.
pub fn draft_height(ui_font_size: f32) -> f32 {
    PAD_V + FIELD_HEIGHT + ACTIONS_HEIGHT + HEADER_HEIGHT * (ui_font_size / 16.0)
}

/// Centres a card's content under a document column (the Markdown
/// preview's) while the band stays full width: `gutter` either side, the
/// content at most `max_width` wide, the accent bar on the band's edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub max_width: f32,
    pub gutter: f32,
}

/// A band's paint: the fill (PINK: `ink` 0.05 posted, 0.08 drafting) and
/// the accent bar (`solid` at 0.35 posted, 0.7 drafting).
pub struct BandStyle {
    pub background: Hsla,
    pub accent: Hsla,
}

/// The accent bar, stretched to the band's height.
pub fn accent_bar(color: Hsla) -> Div {
    div().w(px(ACCENT_BAR_WIDTH)).h_full().flex_none().bg(color)
}

/// The band as one row with its accent bar first; the host sets the
/// height (see `card` and `draft`) and adds `content`. Under a `column`,
/// the bar sits on the band's edge and the content centres between the
/// gutters.
pub fn band(style: &BandStyle, column: Option<Column>) -> Div {
    div()
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .bg(style.background)
        .when_some(column, |el, column| {
            el.relative().justify_center().px(px(column.gutter))
        })
        .child(accent_bar(style.accent).when(column.is_some(), |el| {
            el.absolute().left(px(0.0)).top(px(0.0))
        }))
}

/// A posted card's band, `height` tall (`card_height`).
pub fn card(style: &BandStyle, height: f32, column: Option<Column>) -> Div {
    band(style, column).h(px(height))
}

/// The open draft's band, `height` tall (`draft_height`).
pub fn draft(style: &BandStyle, height: f32, column: Option<Column>) -> Div {
    band(style, column).h(px(height))
}

/// The column inside a band: the header, then the note or the field and
/// actions, inside `PAD_X` and half of `PAD_V` above and below (no side
/// inset under a `column`).
pub fn content(column: Option<Column>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .px(px(PAD_X))
        .when_some(column, |el, column| {
            el.max_w(px(column.max_width)).px(px(0.0))
        })
        .py(px(PAD_V / 2.0))
}

/// Shapes the host's popover card as a floating comment (the code
/// editor's): one column inside `PAD_X` and half of `PAD_V`.
pub fn floating<E: Styled>(card: E) -> E {
    card.flex().flex_col().px(px(PAD_X)).py(px(PAD_V / 2.0))
}

/// Shapes the host's popover card as a floating draft: one column inside
/// `PAD_X` and half of `FLOATING_DRAFT_PAD_V`.
pub fn floating_draft<E: Styled>(card: E) -> E {
    card.flex()
        .flex_col()
        .px(px(PAD_X))
        .py(px(FLOATING_DRAFT_PAD_V / 2.0))
}

/// The header row with the host's comment glyph; the host adds the
/// `location`, then edit and remove on a posted card.
pub fn header(glyph: Svg, color: Hsla) -> Div {
    div()
        .h(ui_rems(HEADER_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HEADER_GAP))
        .child(glyph.flex_none().size(px(GLYPH)).text_color(color))
}

/// The `path:line` the note cites, in the host's mono `font`, truncating.
pub fn location(text: impl Into<SharedString>, color: Hsla, font: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .font_family(font.into())
        .text_size(ui_rems(LOCATION_TEXT))
        .text_color(color)
        .child(text.into())
}

/// Edit: the host's pen in an `EDIT` box, shown while the pointer is over
/// the card's hover `group`.
pub fn edit(glyph: Svg, color: Hsla, group: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .size(px(EDIT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .cursor_pointer()
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
        .child(glyph.size(px(GLYPH)).text_color(color))
}

/// Remove: the host's glyph in a `REMOVE_HIT` box whose negative margin
/// keeps its footprint at 16 pt, shown while the pointer is over the card's
/// hover `group`.
pub fn remove(glyph: Svg, color: Hsla, group: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .size(px(REMOVE_HIT))
        .m(px(-REMOVE_INSET))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .cursor_pointer()
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
        .child(glyph.size(px(GLYPH)).text_color(color))
}

/// The note. Its height is the host's analytic one, so an over-long note
/// clips inside the card rather than past it.
pub fn body(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .text_size(ui_rems(BODY_TEXT))
        .line_height(ui_rems(LINE_HEIGHT))
        .text_color(color)
        .child(text.into())
}

/// The draft's `FIELD_HEIGHT` slot for the host's text field.
pub fn field(input: impl IntoElement) -> Div {
    div()
        .h(px(FIELD_HEIGHT))
        .flex_none()
        .overflow_hidden()
        .text_size(ui_rems(BODY_TEXT))
        .child(input)
}

/// A floating draft's field paint: the field's glass and its edge.
pub struct FieldStyle {
    pub background: Hsla,
    pub border: Hsla,
}

/// A floating draft's framed field around the host's text field.
pub fn framed_field(style: &FieldStyle, input: impl IntoElement) -> Div {
    div()
        .h(px(FLOATING_FIELD_HEIGHT))
        .flex_none()
        .overflow_hidden()
        .rounded(px(FLOATING_FIELD_RADIUS))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .px(px(FLOATING_FIELD_PAD_X))
        .py(px(FLOATING_FIELD_PAD_Y))
        .text_size(ui_rems(BODY_TEXT))
        .child(input)
}

/// The draft's action row: the host's actions at its trailing end.
pub fn actions() -> Div {
    div()
        .h(px(ACTIONS_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(px(ACTIONS_GAP))
}

/// An action's paint. The primary (Comment, Save) is PINK's `solid` plate
/// with `on_solid`; Cancel is a ghost whose label and wash the host blends
/// over its hover fade.
pub struct ActionStyle {
    pub background: Hsla,
    pub text: Hsla,
}

fn action_base(style: &ActionStyle) -> Div {
    div()
        .flex()
        .items_center()
        .rounded(px(ACTION_RADIUS))
        .text_size(ui_rems(ACTION_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .bg(style.background)
        .text_color(style.text)
}

/// A band draft's action: a fixed `ACTION_HEIGHT` box with its label.
pub fn action(style: &ActionStyle, label: impl Into<SharedString>) -> Div {
    action_base(style)
        .h(px(ACTION_HEIGHT))
        .px(px(ACTION_PAD_X))
        .child(label.into())
}

/// A floating draft's action: the same plate, its box scaled with the
/// Interface size too.
pub fn scaled_action(style: &ActionStyle, label: impl Into<SharedString>) -> Div {
    action_base(style)
        .h(ui_rems(ACTION_HEIGHT))
        .px(ui_rems(ACTION_PAD_X))
        .child(label.into())
}

/// The adder: the host's plus centred on an `ADDER` plate (PINK: `solid`
/// under `on_solid`). The host places and reveals it and adds the action.
pub fn adder(fill: Hsla, glyph: Svg, color: Hsla) -> Div {
    div()
        .size(px(ADDER))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .bg(fill)
        .cursor_pointer()
        .child(glyph.size(px(ADDER_GLYPH)).text_color(color))
}
