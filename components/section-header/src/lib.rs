// PINK's sidebar section headers (shell/spaces.rs): the quiet disclosure
// header over a group of rows (`sidebar_disclosure_header`, audit SH-15) and
// a bottom shelf's header (`sidebar_shelf_header`, #185) with its glyph,
// label, count and chevron. UI rethink decision 54: drawn here, consumed by
// PINK through a pinned revision.
//
// The package draws only. The host supplies the Interface scale, the
// colours, the glyphs and the chevron (and its rotation), the label's edge
// fade and the count's tabular figures; it adds the id, the hover fill and
// its fade, the role, accessible name, focus and the toggle. Plain `//`
// comments: the pink-ui gallery compiles this file in place with `include!`
// for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// A header's height at the 16 px default.
pub const HEADER_HEIGHT: f32 = 28.0;
/// The 24 pt minimum hit area: a header never drops below it.
pub const MIN_HIT: f32 = 24.0;
/// A header's horizontal padding and corner radius (fixed pixels).
pub const HEADER_PAD_X: f32 = 8.0;
pub const HEADER_RADIUS: f32 = 6.0;
/// A disclosure header's gap between its label and chevron (fixed pixels).
pub const DISCLOSURE_GAP: f32 = 8.0;
/// A disclosure header's label, in pt at the 16 px default.
pub const DISCLOSURE_TEXT: f32 = 11.0;
/// A shelf header's gap between its parts, in pt at the 16 px default.
pub const SHELF_GAP: f32 = 8.0;
/// The mark slot every sidebar row kind shares, in pt at the 16 px
/// default, so the label starts on the rows' title column.
pub const MARK_SLOT: f32 = 16.0;
/// A shelf's glyph, label and count, in pt at the 16 px default.
pub const SHELF_GLYPH: f32 = 13.0;
pub const SHELF_TEXT: f32 = 12.0;
pub const COUNT_TEXT: f32 = 11.0;
/// The disclosure chevron, in pt at the 16 px default.
pub const CHEVRON: f32 = 12.0;
/// The chevron glyph's empty space at a trailing edge, pulled back so the
/// arrow ends on the time column of the rows above it.
pub const CHEVRON_TRAILING_BEARING: f32 = 4.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// A header's height at an Interface `scale` (size / 16): it grows with the
/// text but never drops below the minimum hit target.
pub fn header_height(scale: f32) -> f32 {
    (HEADER_HEIGHT * scale).max(MIN_HIT)
}

/// The disclosure header's row: a small label, a spacer and the chevron
/// the host reveals under the pointer. No rule.
pub fn disclosure_header(scale: f32) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(DISCLOSURE_GAP))
        .h(px(header_height(scale)))
        .px(px(HEADER_PAD_X))
        .rounded(px(HEADER_RADIUS))
        .cursor_pointer()
}

/// The disclosure header's label: small, semibold and quiet, in the host's
/// colour (held to its text floor).
pub fn disclosure_label(text: Hsla, label: impl Into<SharedString>) -> Div {
    div()
        .text_size(ui_rems(DISCLOSURE_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(text)
        .child(label.into())
}

/// A trailing part shown only while the pointer is over the header's hover
/// `group` (the disclosure chevron).
pub fn reveal_on_hover(group: impl Into<SharedString>) -> Div {
    div()
        .flex_none()
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
}

/// A shelf header's row: the glyph on the title column, the label, the
/// count and the chevron. The host blends its text over the hover fade.
pub fn shelf_header(scale: f32) -> Div {
    div()
        .w_full()
        .flex_none()
        .h(px(header_height(scale)))
        .px(px(HEADER_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(SHELF_GAP))
        .rounded(px(HEADER_RADIUS))
        .cursor_pointer()
}

/// The mark slot: the host's glyph starts on the rows' title column.
pub fn mark_slot() -> Div {
    div()
        .w(ui_rems(MARK_SLOT))
        .flex_none()
        .flex()
        .justify_start()
}

/// The shelf's label: it takes the room that is left.
pub fn shelf_label(label: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .text_size(ui_rems(SHELF_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .child(label.into())
}

/// The shelf's count, in the host's colour. The host adds tabular figures.
pub fn shelf_count(text: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(COUNT_TEXT))
        .text_color(text)
}

/// The chevron's slot at a trailing edge, its glyph's empty space pulled
/// back onto the time column.
pub fn trailing_chevron() -> Div {
    div()
        .flex_none()
        .mr(-rems(CHEVRON_TRAILING_BEARING / 16.0))
}
