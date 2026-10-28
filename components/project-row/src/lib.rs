// PINK's project picker rows (project_picker.rs, the sidebar's project
// filter): a project's name with its device tag and the disconnected glyph
// over its folder, its activity counts (approvals waiting, tasks running) or
// when it was last active, the footer's action rows, the pinned footer's
// full-bleed divider and the empty line. UI rethink decision 54: drawn here,
// consumed by PINK through a pinned revision.
//
// The package draws only. A row's base is the host's keyboard-navigable menu
// row (pink-menu-card `row` with its highlight and selection check) and its
// leading tile is the host's project monogram; this package lays out what
// follows. The host supplies the colours already held to their contrast
// floors, the glyphs, the mono family and the text; it adds the ids, the
// listbox roles and accessible names, the pointer and keyboard cursor, the
// clicks and the context menu, and the counts' tabular figures. Plain `//`
// comments: the pink-ui gallery compiles this file in place with `include!`
// for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, Svg, div, prelude::*, px, rems};

/// The gap between rows, in px.
pub const LIST_GAP: f32 = 2.0;
/// The list's scroll budget, in pt at the 16 px default.
pub const LIST_MAX_HEIGHT: f32 = 224.0;
/// A project's leading tile, and the All projects folder glyph in it.
pub const TILE: f32 = 28.0;
pub const ALL_GLYPH: f32 = 20.0;
/// The gap between a project's name, its tag and the disconnected glyph.
pub const NAME_GAP: f32 = 6.0;
/// The tag's widest share of the name line.
pub const TAG_MAX_SHARE: f32 = 0.5;
/// The tag, the folder line and the counts.
pub const DETAIL_TEXT: f32 = 11.0;
/// The disconnected glyph, a count's glyph and a footer action's glyph.
pub const GLYPH: f32 = 12.0;
/// The gap between counts, and between a count's glyph and its number.
pub const COUNTS_GAP: f32 = 8.0;
pub const COUNT_GAP: f32 = 3.0;
/// The running task's mark: a small square, the task loader at rest.
pub const WORKING_MARK: f32 = 6.0;
pub const WORKING_MARK_RADIUS: f32 = 1.5;
/// The footer divider's thickness, its room above and below, and how far it
/// bleeds through the card's inset to its edges, in px.
pub const DIVIDER: f32 = 1.0;
pub const DIVIDER_MARGIN: f32 = 2.0;
pub const DIVIDER_BLEED: f32 = 4.0;
/// The empty line's padding (px) and text.
pub const EMPTY_PAD_X: f32 = 8.0;
pub const EMPTY_PAD_Y: f32 = 6.0;
pub const EMPTY_TEXT: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The text column after the tile: the name line over the folder line. It
/// takes the room that is left.
pub fn text_column() -> Div {
    div().flex_1().min_w_0().flex().flex_col()
}

/// The name line: the name, then the host's tag and disconnected glyph.
pub fn name_line(name: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(NAME_GAP))
        .font_weight(FontWeight::MEDIUM)
        .child(div().min_w_0().truncate().child(name.into()))
}

/// The device tag after the name: at most half the line, truncating.
pub fn tag(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .max_w(gpui::relative(TAG_MAX_SHARE))
        .min_w_0()
        .truncate()
        .text_size(ui_rems(DETAIL_TEXT))
        .font_weight(FontWeight::NORMAL)
        .text_color(color)
        .child(text.into())
}

/// A small glyph in a row: the disconnected mark, a footer action's glyph.
pub fn glyph(glyph: Svg, color: Hsla) -> Svg {
    glyph.size(ui_rems(GLYPH)).flex_none().text_color(color)
}

/// The folder (mono, `font_family`) or All projects' sentence under the
/// name, truncating.
pub fn subtitle(
    text: impl Into<SharedString>,
    color: Hsla,
    font_family: Option<SharedString>,
) -> Div {
    let line = div()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(DETAIL_TEXT))
        .font_weight(FontWeight::NORMAL);
    let line = match font_family {
        Some(family) => line.font_family(family),
        None => line,
    };
    line.text_color(color).child(text.into())
}

/// The trailing activity counts. The host adds the counts (or the last
/// active time) and their tabular figures.
pub fn counts() -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(COUNTS_GAP))
        .text_size(ui_rems(DETAIL_TEXT))
        .font_weight(FontWeight::MEDIUM)
}

/// One count: the host's glyph, then the number, in its tone.
pub fn count(glyph: impl IntoElement, n: usize, color: Hsla) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(COUNT_GAP))
        .text_color(color)
        .child(glyph)
        .child(SharedString::from(n.to_string()))
}

/// The running task's mark in the busy tone.
pub fn working_mark(color: Hsla) -> Div {
    div()
        .size(ui_rems(WORKING_MARK))
        .rounded(px(WORKING_MARK_RADIUS))
        .bg(color)
}

/// When the project was last active, where it has nothing waiting or
/// running.
pub fn last_active(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .font_weight(FontWeight::NORMAL)
        .text_color(color)
        .child(text.into())
}

/// A footer action's label: it takes the room that is left and truncates.
pub fn action_label(text: impl Into<SharedString>) -> Div {
    div().flex_1().min_w_0().truncate().child(text.into())
}

/// The divider over the pinned footer: full-bleed through the card's inset,
/// since a divider stopping short of the edges reads as a mistake.
pub fn divider(color: Hsla) -> Div {
    div()
        .my(px(DIVIDER_MARGIN))
        .mx(px(-DIVIDER_BLEED))
        .h(px(DIVIDER))
        .flex_none()
        .bg(color)
}

/// The line shown when no project matches.
pub fn empty(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .px(px(EMPTY_PAD_X))
        .py(px(EMPTY_PAD_Y))
        .text_size(ui_rems(EMPTY_TEXT))
        .text_color(color)
        .child(text.into())
}
