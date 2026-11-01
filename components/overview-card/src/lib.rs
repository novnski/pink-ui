// PINK's thread overview card (after T3 Code's ThreadDetailsCard,
// threadDetailsCardLayout and chatCanvasLayout): the card that sits beside
// the conversation when a readable chat lane still fits, its sections and
// rows, and the canvas geometry the conversation and the card share. When
// the lane does not fit, the host shows the same content in a popover.
//
// The package draws only. The host supplies the colours (already blended
// over its hover fade), the font, the glyph paths and the text; it adds the
// ids, listeners, actions, focus, tooltips, scrolling and accessibility, and
// decides which sections show at each density.
// Plain `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{
    AbsoluteLength, BoxShadow, Div, FontWeight, Hsla, SharedString, Svg, div, prelude::*, px, rems,
    svg,
};

// ---------------------------------------------------------------------------
// Canvas geometry (logical px, not Interface-scaled: it divides the window)
// ---------------------------------------------------------------------------

/// The card's width beside the conversation.
pub const CARD_WIDTH: f32 = 280.0;
/// The card's inset from the canvas's top and right edges (and its bottom).
pub const CARD_GAP: f32 = 12.0;
/// The clear space between the chat column's right edge and the card.
pub const CHAT_CLEARANCE: f32 = 32.0;
/// The canvas's side padding while the card is beside the chat.
pub const LANE_PADDING: f32 = 48.0;
/// The widest the chat column grows (the host may pass its own reading
/// width instead).
pub const PREFERRED_CHAT_WIDTH: f32 = 768.0;
/// The narrowest chat column the card may leave; below it the card turns
/// into a popover.
pub const MIN_CHAT_WIDTH: f32 = 640.0;
/// The shortest the card may be inline; shorter canvases use the popover.
pub const MIN_CARD_HEIGHT: f32 = 160.0;

/// Where the card sits inside the canvas, in logical px from its top-left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardPlacement {
    /// The card's left edge.
    pub left: f32,
    /// The card's inset from the canvas's right edge.
    pub right: f32,
    pub top: f32,
    pub width: f32,
    /// The most height the card may take (it scrolls past it).
    pub height: f32,
}

/// The card's place in a `width` by `height` canvas whose chat may not get
/// narrower than `min_chat_width`, `top_inset` below the canvas's top (a
/// find bar or other chrome above it): `None` when a readable lane does not
/// fit beside it, or the card would be too short — then the host shows the
/// card as a popover instead.
pub fn card_placement(
    width: f32,
    height: f32,
    min_chat_width: f32,
    top_inset: f32,
) -> Option<CardPlacement> {
    let left = width - CARD_WIDTH - CARD_GAP;
    if left - CHAT_CLEARANCE - LANE_PADDING < min_chat_width {
        return None;
    }
    let top = CARD_GAP + top_inset;
    let card_height = height - top - CARD_GAP;
    if card_height < MIN_CARD_HEIGHT {
        return None;
    }
    Some(CardPlacement {
        left,
        right: CARD_GAP,
        top,
        width: CARD_WIDTH,
        height: card_height,
    })
}

/// The chat column in a canvas `width` wide: its left edge and width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChatLane {
    pub left: f32,
    pub width: f32,
}

/// The chat column for a canvas `width` wide whose chat grows to
/// `preferred_width`, with the card's left edge at `card_left` (when it is
/// inline). The chat stays centred in the canvas while it can; it shifts
/// left only as far as it must to keep `CHAT_CLEARANCE` from the card, and
/// narrows only once it reaches the canvas's left padding.
pub fn chat_lane(width: f32, preferred_width: f32, card_left: Option<f32>) -> ChatLane {
    let centred = preferred_width.min(width - LANE_PADDING * 2.0).max(0.0);
    let lane_right = card_left.map_or(width - LANE_PADDING, |left| left - CHAT_CLEARANCE);
    let lane_width = centred.min(lane_right - LANE_PADDING).max(0.0);
    let centre_left = (width - lane_width) / 2.0;
    let left = centre_left.min(lane_right - lane_width).max(LANE_PADDING);
    ChatLane {
        left,
        width: lane_width,
    }
}

/// How much of the card's content shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Density {
    /// Every section.
    Full,
    /// The workspace and version control, without the longer sections.
    Compact,
    /// Only what the thread needs at hand (its branch and Git actions).
    Essential,
}

/// The density for a card `available` tall, given the content's measured
/// heights at full and compact density (0 = not measured yet, which tries
/// that density so it gets measured).
pub fn density(available: f32, full: f32, compact: f32) -> Density {
    if full == 0.0 || full <= available {
        Density::Full
    } else if compact == 0.0 || compact <= available {
        Density::Compact
    } else {
        Density::Essential
    }
}

// ---------------------------------------------------------------------------
// The card and its parts (Interface-scaled), in T3's row grammar: one
// 32 pt row shape for every control, 16 pt glyphs leading, 13 pt medium
// labels, quiet trailing meta, and trailing chevrons for anything that
// opens more. Split controls pair a primary action with its options.
// ---------------------------------------------------------------------------

/// The card's corner radius (T3's `rounded-3xl` on its 10 px radius
/// scale).
pub const CARD_RADIUS: f32 = 22.0;
/// A section's side padding, in pt at the 16 px default.
pub const SECTION_PAD_X: f32 = 8.0;
/// A section's top and bottom padding, in pt at the 16 px default.
pub const SECTION_PAD_TOP: f32 = 8.0;
pub const SECTION_PAD_BOTTOM: f32 = 10.0;
/// A section heading's height and its space below, in pt.
pub const HEADING_HEIGHT: f32 = 32.0;
pub const HEADING_GAP: f32 = 4.0;
/// Every control's height: rows, split halves and select rows, in pt.
pub const ROW_HEIGHT: f32 = 32.0;
/// A row's side padding and the gap between its parts, in pt.
pub const ROW_PAD_X: f32 = 10.0;
pub const ROW_GAP: f32 = 10.0;
/// A row's corner radius (T3's `rounded-lg`).
pub const ROW_RADIUS: f32 = 10.0;
/// A row glyph's side, in pt.
pub const GLYPH: f32 = 16.0;
/// A row's label size, in pt.
pub const TEXT: f32 = 13.0;
/// A lineage row's title size, in pt.
pub const TITLE_TEXT: f32 = 14.0;
/// Headings, trailing meta and statuses, in pt.
pub const META_TEXT: f32 = 11.0;
/// Diff counts and other figures, in pt.
pub const FIGURE_TEXT: f32 = 12.0;
/// The trailing chevron slot of a select row or a split control, in pt.
pub const CHEVRON_SLOT: f32 = 32.0;
/// A small icon action's side and corner radius, in pt.
pub const ICON_ACTION: f32 = 24.0;
pub const ICON_ACTION_RADIUS: f32 = 8.0;
/// The tallest a list (lineage, related chats, artifacts) grows before it
/// scrolls inside the card, in pt.
pub const LIST_MAX_HEIGHT: f32 = 216.0;
/// One nesting level of a list row (a chat nested under the chat it came
/// from), in pt.
pub const INDENT: f32 = 16.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What the card draws.
pub struct CardStyle {
    pub border: Hsla,
    pub background: Hsla,
    pub text: Hsla,
    pub font_family: SharedString,
    /// The card's lift; `None` inline (an in-flow card has none) or on a
    /// frosted surface that carries its own depth.
    pub shadows: Option<Vec<BoxShadow>>,
}

/// The card: a bordered surface with `CARD_RADIUS` corners, its sections
/// stacked in a column. The host chains its size, position, scrolling and
/// children.
pub fn card(style: &CardStyle) -> Div {
    let card = div()
        .flex()
        .flex_col()
        .border_1()
        .border_color(style.border)
        .rounded(px(CARD_RADIUS));
    let card = match &style.shadows {
        Some(shadows) => card.shadow(shadows.clone()),
        None => card,
    };
    card.bg(style.background)
        .overflow_hidden()
        .text_size(ui_rems(TEXT))
        .font_family(style.font_family.clone())
        .text_color(style.text)
}

/// A group of controls. `rule` draws a 1 px rule above it, border to
/// border (the first section, and sections in compact density, have none).
pub fn section(rule: Option<Hsla>) -> Div {
    let section = div()
        .flex()
        .flex_col()
        .px(ui_rems(SECTION_PAD_X))
        .pt(ui_rems(SECTION_PAD_TOP))
        .pb(ui_rems(SECTION_PAD_BOTTOM));
    match rule {
        Some(color) => section.border_t_1().border_color(color),
        None => section,
    }
}

/// A section's heading: its small medium-weight name and an optional
/// trailing status ("2 running"), aligned with the rows' labels.
pub fn heading(
    color: Hsla,
    label: impl Into<SharedString>,
    status: Option<(SharedString, Hsla)>,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(ui_rems(8.0))
        .min_h(ui_rems(HEADING_HEIGHT))
        .mb(ui_rems(HEADING_GAP))
        .px(ui_rems(ROW_PAD_X))
        .text_size(ui_rems(META_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(div().min_w_0().truncate().child(label.into()))
        .children(status.map(|(text, color)| div().flex_none().text_color(color).child(text)))
}

/// What a control draws: its label colour and its fill, already resolved
/// for its state or blended over the hover fade.
pub struct RowStyle {
    pub text: Hsla,
    pub fill: Hsla,
}

/// One control: `ROW_HEIGHT` tall across the section, its parts `ROW_GAP`
/// apart inside `ROW_PAD_X`, a 13 pt medium label. The host adds a glyph,
/// a label and trailing meta or a chevron.
pub fn row(style: &RowStyle) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .w_full()
        .min_w_0()
        .h(ui_rems(ROW_HEIGHT))
        .gap(ui_rems(ROW_GAP))
        .px(ui_rems(ROW_PAD_X))
        .rounded(px(ROW_RADIUS))
        .bg(style.fill)
        .text_size(ui_rems(TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
}

/// A row's leading glyph (or a trailing one: a workspace's folder).
pub fn glyph(path: impl Into<SharedString>, color: Hsla) -> Svg {
    svg()
        .path(path.into())
        .flex_none()
        .size(ui_rems(GLYPH))
        .text_color(color)
}

/// A row's label: one truncated line that takes the row's free width.
pub fn label(text: impl Into<SharedString>) -> Div {
    div().flex_1().min_w_0().truncate().child(text.into())
}

/// A lineage row's title: a chat's name, a size up and a touch brighter
/// than control labels.
pub fn title(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(TITLE_TEXT))
        .text_color(color)
        .child(text.into())
}

/// Quiet trailing meta: a status, a relationship, a count.
pub fn meta(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(META_TEXT))
        .font_weight(FontWeight::NORMAL)
        .text_color(color)
        .child(text.into())
}

/// The colours of a diff count.
pub struct DiffCountsStyle {
    pub addition: Hsla,
    pub deletion: Hsla,
    /// A zero count.
    pub muted: Hsla,
    pub mono: SharedString,
}

/// Lines added and removed, `+12 -3`, in figures: green and red when there
/// are any, muted at zero.
pub fn diff_counts(style: &DiffCountsStyle, additions: u64, deletions: u64) -> Div {
    let color = |count: u64, tone: Hsla| if count > 0 { tone } else { style.muted };
    div()
        .flex()
        .flex_none()
        .flex_row()
        .items_center()
        .gap(ui_rems(6.0))
        .font_family(style.mono.clone())
        .text_size(ui_rems(FIGURE_TEXT))
        .font_weight(FontWeight::NORMAL)
        .child(
            div()
                .text_color(color(additions, style.addition))
                .child(format!("+{additions}")),
        )
        .child(
            div()
                .text_color(color(deletions, style.deletion))
                .child(format!("-{deletions}")),
        )
}

/// A select control: a row whose label part and trailing chevron slot
/// are separate halves (the host puts `select_label` and `select_chevron`
/// in it; both open the same menu).
pub fn select_row(style: &RowStyle) -> Div {
    row(style).gap_0().px_0()
}

/// A select control's label half.
pub fn select_label() -> Div {
    div()
        .flex()
        .flex_row()
        .flex_1()
        .min_w_0()
        .h_full()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .px(ui_rems(ROW_PAD_X))
}

/// A select control's (or a split control's) trailing chevron, centred in
/// its `CHEVRON_SLOT`. `expanded` turns it up; folded it points down.
pub fn select_chevron(path: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(ui_rems(CHEVRON_SLOT))
        .h_full()
        .child(glyph(path, color))
}

/// A split control: a primary action and its options side by side in one
/// row shape. `group` names the pair for its hover hairline.
pub fn split_group(group: impl Into<SharedString>) -> Div {
    div()
        .group(group.into())
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .w_full()
        .min_w_0()
        .h(ui_rems(ROW_HEIGHT))
        .rounded(px(ROW_RADIUS))
}

/// A split control's primary half: the action itself.
pub fn split_primary(style: &RowStyle) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_1()
        .min_w_0()
        .h_full()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .px(ui_rems(ROW_PAD_X))
        .rounded_l(px(ROW_RADIUS))
        .bg(style.fill)
        .text_size(ui_rems(TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
}

/// The hairline between a split control's halves: it shows only while the
/// pair is hovered, so a resting pair reads as one plain row.
pub fn split_separator(group: impl Into<SharedString>, hairline: Hsla) -> Div {
    div()
        .flex_none()
        .w(px(1.0))
        .h(ui_rems(16.0))
        .group_hover(group.into(), move |style| style.bg(hairline))
}

/// A split control's options half: the chevron.
pub fn split_secondary(style: &RowStyle) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(ui_rems(CHEVRON_SLOT))
        .h_full()
        .rounded_r(px(ROW_RADIUS))
        .bg(style.fill)
        .text_color(style.text)
}

/// A small square icon action at a row's end (reveal, stop), `fill`
/// already blended over its hover fade.
pub fn icon_action(fill: Hsla) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(ui_rems(ICON_ACTION))
        .rounded(px(ICON_ACTION_RADIUS))
        .bg(fill)
}

/// A chevron that folds a group: down when `expanded`, turned to point
/// right when folded.
pub fn fold_chevron(path: impl Into<SharedString>, color: Hsla, expanded: bool) -> Svg {
    let chevron = glyph(path, color);
    if expanded {
        chevron
    } else {
        chevron.with_transformation(gpui::Transformation::rotate(gpui::radians(
            -std::f32::consts::FRAC_PI_2,
        )))
    }
}

/// A list's rows: they scroll inside the card past `LIST_MAX_HEIGHT` (the
/// host adds the id and the scrolling).
pub fn list() -> Div {
    div().flex().flex_col().max_h(ui_rems(LIST_MAX_HEIGHT))
}

/// The row that unfolds more of a list: a plus and `Show N more`, quieter
/// than the rows above it.
pub fn show_more(
    style: &RowStyle,
    plus: impl Into<SharedString>,
    text: impl Into<SharedString>,
) -> Div {
    let color = style.text;
    row(style).child(glyph(plus, color)).child(label(text))
}

/// A section's state line in place of its rows: loading, empty or an
/// error, aligned with the rows' labels.
pub fn note(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .items_center()
        .min_h(ui_rems(ROW_HEIGHT))
        .px(ui_rems(ROW_PAD_X))
        .text_size(ui_rems(FIGURE_TEXT))
        .text_color(color)
        .child(text.into())
}

/// A status dot (running, failed, idle) in a glyph's slot.
pub fn dot(color: Hsla) -> Div {
    div()
        .flex_none()
        .size(ui_rems(GLYPH))
        .flex()
        .items_center()
        .justify_center()
        .child(div().size(ui_rems(6.0)).rounded_full().bg(color))
}
