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
// The card and its parts (Interface-scaled)
// ---------------------------------------------------------------------------

/// The card's corner radius.
pub const CARD_RADIUS: f32 = 16.0;
/// The card's inner inset from its edge to its sections.
pub const CARD_INSET: f32 = 4.0;
/// A section's vertical padding, in pt at the 16 px default.
pub const SECTION_PAD_Y: f32 = 4.0;
/// A row's corner radius: concentric with the card across its inset.
pub const ROW_RADIUS: f32 = CARD_RADIUS - CARD_INSET - 4.0;
/// A row's least height, in pt at the 16 px default.
pub const ROW_HEIGHT: f32 = 30.0;
/// A row's horizontal padding, in pt at the 16 px default.
pub const ROW_PAD_X: f32 = 8.0;
/// The gap between a row's parts, in pt at the 16 px default.
pub const ROW_GAP: f32 = 8.0;
/// A row glyph's side, in pt at the 16 px default.
pub const GLYPH: f32 = 14.0;
/// Text in the card's rows, in pt at the 16 px default.
pub const TEXT: f32 = 13.0;
/// A row's quiet detail and a section heading, in pt at the 16 px default.
pub const DETAIL_TEXT: f32 = 11.5;

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

/// The card: a bordered, rounded surface with its sections stacked inside
/// `CARD_INSET`. The host chains its size, position, scrolling and
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

/// A group of rows. `rule` draws a 1 px rule above it, border to border
/// (the first section has none).
pub fn section(rule: Option<Hsla>) -> Div {
    let section = div()
        .flex()
        .flex_col()
        .px(px(CARD_INSET))
        .py(ui_rems(SECTION_PAD_Y));
    match rule {
        Some(color) => section.border_t_1().border_color(color),
        None => section,
    }
}

/// A section's small heading, with an optional trailing count.
pub fn heading(color: Hsla, label: impl Into<SharedString>, count: Option<usize>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(6.0))
        .px(ui_rems(ROW_PAD_X))
        .pt(ui_rems(4.0))
        .pb(ui_rems(2.0))
        .text_size(ui_rems(DETAIL_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
        .children(count.map(|count| div().flex_none().child(count.to_string())))
}

/// What one row draws: its text and its fill, already resolved for its
/// selected state or blended over the hover fade.
pub struct RowStyle {
    pub text: Hsla,
    pub fill: Hsla,
}

/// One row: its parts in a line `ROW_GAP` apart, at least `ROW_HEIGHT`
/// tall, rounded concentric with the card. The host adds a glyph, a label
/// and details.
pub fn row(style: &RowStyle) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .min_h(ui_rems(ROW_HEIGHT))
        .px(ui_rems(ROW_PAD_X))
        .py(ui_rems(4.0))
        .rounded(px(ROW_RADIUS))
        .bg(style.fill)
        .text_color(style.text)
}

/// A row's leading glyph.
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

/// A row's label over a quiet second line (a folder under its machine, a
/// worktree under its branch).
pub fn label_with_detail(
    text: impl Into<SharedString>,
    detail: Option<SharedString>,
    detail_color: Hsla,
) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(div().truncate().child(text.into()))
        .children(detail.map(|detail| {
            div()
                .truncate()
                .text_size(ui_rems(DETAIL_TEXT))
                .text_color(detail_color)
                .child(detail)
        }))
}

/// A quiet detail at a row's end (a count, a status, a key).
pub fn detail(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(DETAIL_TEXT))
        .text_color(color)
        .child(text.into())
}

/// A status dot (running, failed, idle) in a row glyph's slot.
pub fn dot(color: Hsla) -> Div {
    div()
        .flex_none()
        .size(ui_rems(GLYPH))
        .flex()
        .items_center()
        .justify_center()
        .child(div().size(ui_rems(6.0)).rounded_full().bg(color))
}

/// A section's state line in place of its rows: loading, empty or an error.
pub fn note(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .px(ui_rems(ROW_PAD_X))
        .py(ui_rems(4.0))
        .text_size(ui_rems(12.0))
        .text_color(color)
        .child(text.into())
}

/// Small controls in a line under a row (editor choices, Git actions); the
/// host adds its buttons.
pub fn actions() -> Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(ui_rems(4.0))
        .px(ui_rems(ROW_PAD_X))
        .pb(ui_rems(4.0))
}

/// The quiet text control that unfolds a longer list ("Show more").
pub fn show_more(color: Hsla, text: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .min_h(ui_rems(24.0))
        .px(ui_rems(ROW_PAD_X))
        .rounded(px(ROW_RADIUS))
        .text_size(ui_rems(12.0))
        .text_color(color)
        .child(text.into())
}
