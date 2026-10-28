// PINK's message badge (badges.rs `render` and its hover card): the pill a
// sent message draws in place of structured context it carries (review
// comments, say), and the detail rows of the card that opens over it. UI
// rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the colours already resolved,
// the mono font, the glyph and the copy; it adds the id, the label role and
// accessible name, the hover card's tooltip and delay, the card's surface
// and its frost. Plain `//` comments: the pink-ui gallery compiles this file
// in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The pill's height floor, in pt at the 16 px default. A host that sizes a
/// strip of pills arithmetically scales this by its Interface size.
pub const BADGE_HEIGHT: f32 = 24.0;
/// The pill's padding and the gap between its glyph and label.
pub const BADGE_PAD_X: f32 = 8.0;
pub const BADGE_GAP: f32 = 6.0;
/// The pill's corner radius. Fixed.
pub const BADGE_RADIUS: f32 = 8.0;
/// The label, in pt at the 16 px default.
pub const BADGE_TEXT: f32 = 12.0;
/// The leading glyph.
pub const BADGE_GLYPH: f32 = 12.0;
/// The hover card: its width, inset and the gap between rows.
pub const CARD_WIDTH: f32 = 320.0;
pub const CARD_PAD: f32 = 6.0;
pub const CARD_GAP: f32 = 4.0;
/// One detail row: the gap after its bar, its padding and radius.
pub const ROW_GAP: f32 = 8.0;
pub const ROW_PAD: f32 = 8.0;
pub const ROW_RADIUS: f32 = 6.0;
/// The accent bar down a row's leading edge.
pub const BAR_WIDTH: f32 = 2.0;
pub const BAR_RADIUS: f32 = 1.0;
/// The gap between a row's location line and its body.
pub const COLUMN_GAP: f32 = 4.0;
/// The location line: its gap and mono size.
pub const LOCATION_GAP: f32 = 6.0;
pub const LOCATION_TEXT: f32 = 11.0;
/// The tag beside a location (a diff side, a status, a count).
pub const TAG_PAD_X: f32 = 4.0;
pub const TAG_RADIUS: f32 = 3.0;
/// The row's body.
pub const BODY_TEXT: f32 = 12.0;
pub const BODY_LINE_HEIGHT: f32 = 16.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a pill draws.
pub struct BadgeStyle {
    /// The pill's plate.
    pub fill: Hsla,
    /// The label's colour.
    pub text: Hsla,
}

/// One pill: its glyph and label in a line. The host adds the id, the role
/// and name, the glyph (`BADGE_GLYPH` rems), the label and the hover card.
pub fn badge(style: &BadgeStyle) -> Div {
    div()
        .min_h(ui_rems(BADGE_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(BADGE_GAP))
        .px(ui_rems(BADGE_PAD_X))
        .rounded(px(BADGE_RADIUS))
        .bg(style.fill)
        .text_size(ui_rems(BADGE_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
}

/// Lays the hover card's rows out on the host's card `surface` (its own
/// popover card, already filled): `CARD_WIDTH` wide, inset `CARD_PAD`, rows
/// `CARD_GAP` apart.
pub fn card(surface: Div) -> Div {
    surface
        .w(ui_rems(CARD_WIDTH))
        .p(ui_rems(CARD_PAD))
        .flex()
        .flex_col()
        .gap(ui_rems(CARD_GAP))
}

/// What a detail row draws, already resolved by the host.
pub struct DetailStyle {
    /// The row's plate.
    pub plate: Hsla,
    /// The accent bar down its leading edge.
    pub bar: Hsla,
    /// The location line's colour.
    pub location: Hsla,
    /// The tag's plate.
    pub tag: Hsla,
    /// The body's colour.
    pub body: Hsla,
    /// The location line's mono font.
    pub mono: SharedString,
}

/// One row of the hover card: a bar, then the location (truncating) with an
/// optional tag over the body.
pub fn detail_row(
    style: &DetailStyle,
    location: impl Into<SharedString>,
    tag: Option<SharedString>,
    body: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_row()
        .gap(ui_rems(ROW_GAP))
        .p(ui_rems(ROW_PAD))
        .rounded(px(ROW_RADIUS))
        .bg(style.plate)
        .child(
            div()
                .flex_none()
                .w(px(BAR_WIDTH))
                .rounded(px(BAR_RADIUS))
                .bg(style.bar),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(ui_rems(COLUMN_GAP))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(ui_rems(LOCATION_GAP))
                        .font_family(style.mono.clone())
                        .text_size(ui_rems(LOCATION_TEXT))
                        .text_color(style.location)
                        .child(div().flex_1().min_w_0().truncate().child(location.into()))
                        .children(tag.map(|tag| {
                            div()
                                .flex_none()
                                .px(ui_rems(TAG_PAD_X))
                                .rounded(px(TAG_RADIUS))
                                .bg(style.tag)
                                .child(tag)
                        })),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_size(ui_rems(BODY_TEXT))
                        .line_height(ui_rems(BODY_LINE_HEIGHT))
                        .text_color(style.body)
                        .child(body.into()),
                ),
        )
}
