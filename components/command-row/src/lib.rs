// PINK's command palette rows (command_palette.rs `render_command_palette`
// and its row helpers): an action's glyph, highlighted label and shortcut,
// an extension command's name beside its label, a matching thread's
// two-line summary, the "show more" row, the quiet section headings, the
// separator spacing between sections, the status line under the results
// and the footer's key hints. UI rethink decision 54: drawn here, consumed
// by PINK through a pinned revision.
//
// The package draws only. A row's base is the host's menu row (pink-menu-card
// `row` with its selected wash or hover fade); this package shapes it for
// the palette. The host supplies the colours, the glyphs, the highlighted
// label, the kbd hints and the text; it adds the ids, the role, accessible
// names, the click and keyboard handling, the scroller and its fades. Plain
// `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, Svg, div, prelude::*, px, rems};

/// A row's corner radius: concentric with the 16 pt palette across its
/// 8 pt inset.
pub const ITEM_RADIUS: f32 = 16.0 - 8.0;
/// The results' inset from the palette's edge, and the room above the first
/// and below the last result, in pt at the 16 px default.
pub const RESULTS_INSET: f32 = 8.0;
/// An action row's minimum height and vertical padding.
pub const ACTION_MIN_HEIGHT: f32 = 32.0;
pub const ACTION_PAD_Y: f32 = 4.0;
/// A matching thread's two-line row.
pub const DETAIL_MIN_HEIGHT: f32 = 44.0;
pub const DETAIL_PAD_Y: f32 = 5.0;
/// The gap between a thread's summary and its attribution, in px.
pub const DETAIL_LINE_GAP: f32 = 2.0;
/// A row's leading glyph.
pub const GLYPH: f32 = 16.0;
/// A thread's summary and attribution.
pub const SUMMARY_TEXT: f32 = 12.0;
pub const META_TEXT: f32 = 11.0;
/// A section heading's inset, its room below and its label.
pub const HEADING_PAD_X: f32 = 10.0;
pub const HEADING_PAD_BOTTOM: f32 = 4.0;
pub const HEADING_TEXT: f32 = 11.0;
/// The room above and below the separator between sections.
pub const SEPARATOR_MARGIN: f32 = 7.0;
/// The status line under the results.
pub const STATUS_PAD_X: f32 = 16.0;
pub const STATUS_PAD_Y: f32 = 8.0;
pub const STATUS_TEXT: f32 = 11.0;
/// A footer key hint: the gap between its keys and its label.
pub const HINT_GAP: f32 = 5.0;
pub const HINT_TEXT: f32 = 11.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// Which row a menu row becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A command or an extension command: one line.
    Action,
    /// A matching thread: its summary over its attribution.
    Detail,
    /// The "show more" row: one line, its label centred.
    More,
}

/// Shapes the host's menu row as a palette row: the palette's concentric
/// radius and the kind's height and padding.
pub fn row<E: Styled>(base: E, kind: RowKind) -> E {
    let row = base.rounded(px(ITEM_RADIUS));
    match kind {
        RowKind::Action => row
            .min_h(ui_rems(ACTION_MIN_HEIGHT))
            .py(ui_rems(ACTION_PAD_Y)),
        RowKind::Detail => row
            .min_h(ui_rems(DETAIL_MIN_HEIGHT))
            .py(ui_rems(DETAIL_PAD_Y)),
        RowKind::More => row.min_h(ui_rems(ACTION_MIN_HEIGHT)).justify_center(),
    }
}

/// One result's slot in the list: the room above the first and below the
/// last result scrolls with the content. The host adds the id, the heading
/// and the inset row.
pub fn slot(first: bool, last: bool) -> Div {
    let slot = div().flex_none();
    let slot = if first {
        slot.pt(ui_rems(RESULTS_INSET))
    } else {
        slot
    };
    if last {
        slot.pb(ui_rems(RESULTS_INSET))
    } else {
        slot
    }
}

/// The row's inset inside its slot.
pub fn inset(row: impl IntoElement) -> Div {
    div().px(ui_rems(RESULTS_INSET)).child(row)
}

/// A row's leading glyph.
pub fn glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(ui_rems(GLYPH)).flex_none().text_color(color)
}

/// The label's room: it takes what is left. The host adds the highlighted
/// label.
pub fn label() -> Div {
    div().flex_1().min_w_0()
}

/// A note after an extension command's label: its extension's name.
pub fn note(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(META_TEXT))
        .text_color(color)
        .child(text.into())
}

/// What a matching thread's lines draw.
pub struct DetailStyle {
    pub summary: Hsla,
    pub meta: Hsla,
}

/// A matching thread's summary over its attribution, each truncating.
pub fn detail(
    summary: impl Into<SharedString>,
    meta: impl Into<SharedString>,
    style: &DetailStyle,
) -> Div {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(DETAIL_LINE_GAP))
        .child(
            div()
                .truncate()
                .text_size(ui_rems(SUMMARY_TEXT))
                .text_color(style.summary)
                .child(summary.into()),
        )
        .child(
            div()
                .truncate()
                .text_size(ui_rems(META_TEXT))
                .text_color(style.meta)
                .child(meta.into()),
        )
}

/// A section's quiet heading. The host adds the id and the label.
pub fn heading(color: Hsla) -> Div {
    div()
        .px(ui_rems(HEADING_PAD_X))
        .pb(ui_rems(HEADING_PAD_BOTTOM))
        .text_size(ui_rems(HEADING_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
}

/// Spaces the host's separator between two sections.
pub fn separator<E: Styled>(separator: E) -> E {
    separator.w_full().my(ui_rems(SEPARATOR_MARGIN))
}

/// The status line under the results (searching, unavailable, nothing
/// found). The host adds the id and the text.
pub fn status(color: Hsla) -> Div {
    div()
        .px(ui_rems(STATUS_PAD_X))
        .py(ui_rems(STATUS_PAD_Y))
        .text_size(ui_rems(STATUS_TEXT))
        .text_color(color)
}

/// A footer key hint: the host's kbd hint and what it does.
pub fn key_hint(keys: impl IntoElement, label: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex()
        .items_center()
        .gap(ui_rems(HINT_GAP))
        .child(keys)
        .child(
            div()
                .text_size(ui_rems(HINT_TEXT))
                .text_color(color)
                .child(label.into()),
        )
}
