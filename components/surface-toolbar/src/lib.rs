// PINK's surface toolbar (surface_chrome.rs `toolbar` and `input`, files/mod.rs
// `render_header` and `toolbar_button`): the control row a sidebar surface
// carries under its titlebar, as tall as the workbench tab strip, on the
// shared chrome surface with no band or border of its own; the rounded search
// field that takes the row's free width, with its magnifier and query; and
// the glyph a toolbar control carries. The controls themselves are
// `pink-icon-button` squares at `CONTROL_SIZE` and `CONTROL_RADIUS`. Beside
// them: text and labelled controls (the file editor's Preview and save
// status, History's Fetch all), History's compact search field that grows
// out of its magnifier, and the file editor header's artwork and breadcrumb
// (files/preview.rs `render_breadcrumb`). UI rethink decision 54: drawn
// here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the query field, the glyphs (its
// own icon assets), every colour already resolved (a control's wash already
// blended over its hover fade, or held while the control is latched), and
// adds the ids, roles, accessible names, toggled states, focus rings,
// tooltips, hover listeners, occlusion, clicks and the search itself. Plain
// `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{
    AbsoluteLength, Div, ElementId, FontWeight, Hsla, Img, IntoElement, SharedString, Stateful,
    Svg, div, prelude::*, px, rems,
};

/// The row's height, in fixed pixels (PINK `surface_chrome::HEADER_HEIGHT`,
/// the titlebar height), so both sidebars' second rows line up.
pub const HEIGHT: f32 = 38.0;
/// The row's side inset, in fixed pixels (PINK `EDGE_INSET`).
pub const EDGE_INSET: f32 = 8.0;
/// The gap between the row's controls, in fixed pixels (PINK `CONTROL_GAP`).
pub const CONTROL_GAP: f32 = 4.0;
/// A toolbar control's side and corner, in fixed pixels (PINK
/// `CONTROL_SIZE`, `CONTROL_RADIUS`): the control ladder's medium step.
pub const CONTROL_SIZE: f32 = 28.0;
pub const CONTROL_RADIUS: f32 = 6.0;
/// A toolbar control's glyph, in fixed pixels (PINK `ICON_SIZE`).
pub const CONTROL_GLYPH: f32 = 16.0;
/// The search field's height, side pad, gap and type, in pt at the 16 px
/// default: the field grows with the Interface size, its corner stays fixed.
pub const FIELD_HEIGHT: f32 = 28.0;
pub const FIELD_PAD_X: f32 = 8.0;
pub const FIELD_GAP: f32 = 6.0;
pub const FIELD_TEXT: f32 = 12.0;
/// The field's magnifier, in pt at the 16 px default.
pub const FIELD_GLYPH: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The control row: `HEIGHT` tall and full width, its children centred
/// `CONTROL_GAP` apart inside `EDGE_INSET`.
pub fn toolbar() -> Div {
    div()
        .h(px(HEIGHT))
        .w_full()
        .flex_none()
        .px(px(EDGE_INSET))
        .flex()
        .items_center()
        .gap(px(CONTROL_GAP))
}

/// The search field: a `FIELD_HEIGHT` plate on `fill` taking the row's free
/// width, its glyph and query `FIELD_GAP` apart in `FIELD_TEXT` type.
pub fn field(fill: Hsla) -> Div {
    div()
        .h(ui_rems(FIELD_HEIGHT))
        .min_w_0()
        .flex_1()
        .px(ui_rems(FIELD_PAD_X))
        .rounded(px(CONTROL_RADIUS))
        .bg(fill)
        .flex()
        .items_center()
        .gap(ui_rems(FIELD_GAP))
        .text_size(ui_rems(FIELD_TEXT))
}

/// The field's leading magnifier.
pub fn field_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph
        .size(ui_rems(FIELD_GLYPH))
        .flex_none()
        .text_color(color)
}

/// The query's room in the field: it takes what is left.
pub fn field_input(input: impl IntoElement) -> Div {
    div().min_w_0().flex_1().child(input)
}

/// A toolbar control's glyph, `CONTROL_GLYPH` square in `color`.
pub fn control_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(px(CONTROL_GLYPH)).text_color(color)
}

// A flex factor through the style, so the file compiles on crates.io GPUI
// 0.2.2 (whose `flex_shrink` takes no factor).
fn shrink<E: Styled>(mut el: E, factor: f32) -> E {
    el.style().flex_shrink = Some(factor);
    el
}

// The text controls (PINK files/preview.rs `render_breadcrumb`'s Preview and
// save status, history.rs `GitHistoryFetchButton`).

/// A text control's side pad, gap and type, in pt at the 16 px default: the
/// editor header's Preview and save status.
pub const TEXT_CONTROL_PAD_X: f32 = 6.0;
pub const TEXT_CONTROL_GAP: f32 = 4.0;
pub const TEXT_CONTROL_TEXT: f32 = 11.0;
/// A labelled control's side pad and gap, in pt at the 16 px default: the
/// History toolbar's Fetch all.
pub const LABELED_CONTROL_PAD_X: f32 = 8.0;
pub const LABELED_CONTROL_GAP: f32 = 6.0;

/// What a text control draws, resolved by the host.
pub struct TextControlStyle {
    /// The host's interface family.
    pub sans: SharedString,
    /// The label (and, through the host's glyph, the tone it sets).
    pub text: Hsla,
    /// The wash, already blended over the hover fade (transparent at rest
    /// and when the control takes no pointer).
    pub wash: Hsla,
    /// A control with nothing to activate rests at half strength.
    pub enabled: bool,
}

/// A text control: `CONTROL_SIZE` tall with its label in `TEXT_CONTROL_TEXT`
/// type, a host glyph (`control_glyph`) `TEXT_CONTROL_GAP` before it.
pub fn text_control(id: impl Into<ElementId>, style: &TextControlStyle) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(CONTROL_SIZE))
        .px(ui_rems(TEXT_CONTROL_PAD_X))
        .rounded(px(CONTROL_RADIUS))
        .flex()
        .flex_none()
        .items_center()
        .gap(ui_rems(TEXT_CONTROL_GAP))
        .font_family(style.sans.clone())
        .text_size(ui_rems(TEXT_CONTROL_TEXT))
        .text_color(style.text)
        .bg(style.wash)
        .when(!style.enabled, |el| el.opacity(0.5))
}

/// What a labelled control draws, resolved by the host.
pub struct LabeledControlStyle {
    /// The wash, already blended over the hover fade, or held while busy.
    pub wash: Hsla,
    /// An enabled control shows the pointing cursor; a busy one does not.
    pub enabled: bool,
}

/// A labelled control: a host glyph (`control_glyph`, or a spinner) and a
/// `control_label`, `LABELED_CONTROL_GAP` apart on a `CONTROL_SIZE` plate.
pub fn labeled_control(id: impl Into<ElementId>, style: &LabeledControlStyle) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(CONTROL_SIZE))
        .px(ui_rems(LABELED_CONTROL_PAD_X))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .gap(ui_rems(LABELED_CONTROL_GAP))
        .rounded(px(CONTROL_RADIUS))
        .bg(style.wash)
        .when(style.enabled, |el| el.cursor_pointer())
}

/// A labelled control's label, on one line in `TEXT_CONTROL_TEXT` type.
pub fn control_label(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .whitespace_nowrap()
        .text_size(ui_rems(TEXT_CONTROL_TEXT))
        .text_color(color)
        .child(text.into())
}

// The compact search field (PINK history.rs `GitHistorySearchControl`): it
// grows out of a magnifier control to `width` and folds back into it; the
// host animates the width.

/// The compact field's narrowest, in pt at the 16 px default, and its
/// trailing pad, in fixed pixels (the close control overhangs it).
pub const COMPACT_FIELD_MIN: f32 = 80.0;
pub const COMPACT_FIELD_PAD_RIGHT: f32 = 2.0;
/// The status slot (magnifier or spinner) and the query line, in pt.
pub const COMPACT_FIELD_SLOT: f32 = 14.0;
/// The compact field's magnifier, in pt.
pub const COMPACT_FIELD_GLYPH: f32 = 11.0;
/// The close control's hit box and glyph, in pt: a 24 pt target in a 16 pt
/// footprint.
pub const COMPACT_CLOSE: f32 = 24.0;
pub const COMPACT_CLOSE_GLYPH: f32 = 9.0;

/// The compact field: `CONTROL_SIZE` tall and `width` wide (never under
/// `COMPACT_FIELD_MIN`, shrinking before the other controls) on `fill`.
pub fn compact_field(id: impl Into<ElementId>, width: f32, fill: Hsla) -> Stateful<Div> {
    shrink(
        div()
            .id(id)
            .h(px(CONTROL_SIZE))
            .w(px(width))
            .min_w(ui_rems(COMPACT_FIELD_MIN)),
        1.0,
    )
    .overflow_hidden()
    .flex()
    .items_center()
    .gap(ui_rems(FIELD_GAP))
    .pl(px(EDGE_INSET))
    .pr(px(COMPACT_FIELD_PAD_RIGHT))
    .rounded(px(CONTROL_RADIUS))
    .bg(fill)
}

/// The leading slot holding the magnifier (`compact_field_glyph`) or the
/// host's spinner, centred.
pub fn compact_field_status(status: impl IntoElement) -> Div {
    div()
        .size(ui_rems(COMPACT_FIELD_SLOT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(status)
}

/// The compact field's magnifier.
pub fn compact_field_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph
        .size(ui_rems(COMPACT_FIELD_GLYPH))
        .flex_none()
        .text_color(color)
}

/// The query's line in the compact field: it takes what is left.
pub fn compact_field_input(input: impl IntoElement) -> Div {
    div()
        .h(ui_rems(COMPACT_FIELD_SLOT))
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .overflow_hidden()
        .child(input)
}

/// The control that clears and folds the field, on `wash` (already blended
/// over its hover fade); the host adds `compact_close_glyph`.
pub fn compact_close(id: impl Into<ElementId>, wash: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .size(ui_rems(COMPACT_CLOSE))
        .m(px(-4.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CONTROL_RADIUS))
        .cursor_pointer()
        .bg(wash)
}

/// The close control's glyph.
pub fn compact_close_glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(ui_rems(COMPACT_CLOSE_GLYPH)).text_color(color)
}

// The file editor header (PINK files/preview.rs `render_breadcrumb`): the
// file's artwork, then its path as a breadcrumb whose leaf leads, then the
// header's controls.

/// The file artwork, in pt at the 16 px default.
pub const ARTWORK: f32 = 14.0;
/// A crumb separator's glyph and side margin, in pt.
pub const CRUMB_SEPARATOR: f32 = 10.0;
pub const CRUMB_SEPARATOR_MARGIN: f32 = 3.0;
/// A parent folder's narrowest, in pt: parents shrink first.
pub const CRUMB_MIN: f32 = 16.0;
/// The leaf's type and a parent folder's, in pt.
pub const CRUMB_LEAF_TEXT: f32 = 12.0;
pub const CRUMB_TEXT: f32 = 11.0;

/// The editor header: the toolbar row with its trailing inset tightened to
/// `CONTROL_GAP`, so the last control sits near the edge.
pub fn editor_header() -> Div {
    toolbar().pr(px(CONTROL_GAP))
}

/// The file's artwork, `ARTWORK` square.
pub fn file_artwork(artwork: Img) -> Img {
    artwork.size(ui_rems(ARTWORK)).flex_none()
}

/// The breadcrumb: the path's crumbs in a line taking the header's free
/// width, clipping what does not fit.
pub fn breadcrumb(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .min_w_0()
        .flex_1()
        .flex()
        .items_center()
        .overflow_hidden()
}

/// The quiet chevron between two crumbs.
pub fn crumb_separator(glyph: Svg, color: Hsla) -> Div {
    div()
        .flex_none()
        .mx(ui_rems(CRUMB_SEPARATOR_MARGIN))
        .child(glyph.size(ui_rems(CRUMB_SEPARATOR)).text_color(color))
}

/// What the crumbs draw, resolved by the host.
pub struct CrumbStyle {
    /// The host's interface family.
    pub sans: SharedString,
    /// The leaf (the file name), in the primary tone.
    pub leaf: Hsla,
    /// A parent folder, in the faint tone.
    pub parent: Hsla,
}

/// One crumb. The leaf keeps its full width in `CRUMB_LEAF_TEXT` medium
/// type; a parent folder shrinks first, down to `CRUMB_MIN`, in
/// `CRUMB_TEXT`. Both truncate.
pub fn crumb(text: impl Into<SharedString>, leaf: bool, style: &CrumbStyle) -> Div {
    let el = if leaf {
        div().flex_shrink_0().max_w_full()
    } else {
        shrink(div().min_w(ui_rems(CRUMB_MIN)), 1.0)
    };
    el.truncate()
        .font_family(style.sans.clone())
        .text_size(ui_rems(if leaf { CRUMB_LEAF_TEXT } else { CRUMB_TEXT }))
        .when(leaf, |el| el.font_weight(FontWeight::MEDIUM))
        .text_color(if leaf { style.leaf } else { style.parent })
        .child(text.into())
}
