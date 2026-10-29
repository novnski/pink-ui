// PINK's file rows (files/tree.rs `render_tree_row` and `status_row`,
// files/search.rs `render_search_row`, files/text_search.rs
// `render_text_search_row`): the Files pane's tree entry as an inset,
// rounded pill indented a level at a time, the full-width row a name search
// result takes, the Loading..., Empty folder, error and Load more... rows a
// lazily listed folder shows, and a text search's file and matching-line
// rows. UI rethink decision 54: drawn here, consumed by PINK through a
// pinned revision.
//
// The package draws only. A row's base is the host's stateful element with
// its selected fill or its eased hover wash already applied; this package
// shapes it. The host supplies each glyph and its file-type artwork (its own
// assets), each colour already resolved, the font families and the text; it
// adds the ids, roles, accessible names, selection state, the click, drag
// and keyboard handling and the virtualized list. Plain `//` comments: the
// pink-ui gallery compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, SharedString, Svg, Transformation, div, prelude::*, px,
    radians, rems,
};

/// Every row's height, in fixed pixels (PINK `TREE_ROW_HEIGHT`,
/// `SEARCH_ROW_HEIGHT`): the virtualized lists measure rows by it.
pub const ROW_HEIGHT: f32 = 27.0;
/// The indent per tree level, in fixed pixels (PINK `TREE_INDENT`,
/// `SEARCH_TREE_INDENT`).
pub const INDENT: f32 = 14.0;
/// The lead before a level-0 row's content, in fixed pixels.
pub const LEAD: f32 = 8.0;
/// The tree pill's inset from each side of the pane, in fixed pixels (PINK
/// `TREE_ROW_INSET`).
pub const PILL_INSET: f32 = 6.0;
/// The tree pill's corner radius, in fixed pixels (PINK `radius::SM`).
pub const PILL_RADIUS: f32 = 6.0;
/// An ignored or hidden entry's opacity.
pub const IGNORED_OPACITY: f32 = 0.52;
/// The gap between a row's slot, artwork and name, in pt at the 16 px
/// default.
pub const GAP: f32 = 4.0;
/// A full-width row's trailing inset, in pt at the 16 px default.
pub const PAD_RIGHT: f32 = 8.0;
/// The disclosure slot and the file-type artwork, in pt at the 16 px
/// default.
pub const SLOT: f32 = 14.0;
/// A tree folder's chevron, turned a quarter while open.
pub const TREE_CHEVRON: f32 = 12.0;
/// A search row's down or right chevron.
pub const SEARCH_CHEVRON: f32 = 11.0;
/// An entry's name.
pub const NAME_TEXT: f32 = 11.5;
/// A status row's label, and the gap between an error and its Retry.
pub const STATUS_TEXT: f32 = 11.0;
pub const STATUS_GAP: f32 = 6.0;
/// A text search row's gap, a file row's lead and a match row's lead.
pub const TEXT_GAP: f32 = 5.0;
pub const TEXT_FILE_LEAD: f32 = 8.0;
pub const TEXT_MATCH_LEAD: f32 = 26.0;
/// A text search file's longest name, its match count, and a match's line
/// number column.
pub const TEXT_NAME_MAX: f32 = 180.0;
pub const COUNT_TEXT: f32 = 10.5;
pub const LINE_COLUMN: f32 = 28.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The lead before a row's content at `depth`, in fixed pixels.
pub fn lead(depth: usize) -> f32 {
    LEAD + depth as f32 * INDENT
}

/// Shapes the host's base as a tree entry: `ROW_HEIGHT` tall, an inset pill
/// (`PILL_INSET` from each side, `PILL_RADIUS` corners) indented `INDENT` a
/// level, dimmed when the entry is ignored. The host adds the disclosure
/// slot, the artwork and the name.
pub fn tree_entry<E: Styled>(base: E, depth: usize, ignored: bool) -> E {
    let row = base
        .h(px(ROW_HEIGHT))
        .mx(px(PILL_INSET))
        .rounded(px(PILL_RADIUS))
        .flex_none()
        .pl(px((lead(depth) - PILL_INSET).max(2.0)))
        .pr(px(LEAD - PILL_INSET / 2.0))
        .flex()
        .items_center()
        .gap(ui_rems(GAP))
        .cursor_pointer();
    if ignored {
        row.opacity(IGNORED_OPACITY)
    } else {
        row
    }
}

/// Shapes the host's base as a name search result: full width, the tree's
/// indent and slots, no pill.
pub fn search_entry<E: Styled>(base: E, depth: usize) -> E {
    base.h(px(ROW_HEIGHT))
        .w_full()
        .flex_none()
        .pl(px(lead(depth)))
        .pr(ui_rems(PAD_RIGHT))
        .flex()
        .items_center()
        .gap(ui_rems(GAP))
        .cursor_pointer()
}

/// The `SLOT` square before the artwork: a folder's chevron, or empty so
/// files line up with folders.
pub fn disclosure(chevron: Option<Svg>) -> Div {
    div()
        .size(ui_rems(SLOT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .children(chevron)
}

/// A tree folder's chevron: one right-pointing glyph, turned a quarter
/// while the folder is open.
pub fn tree_chevron(glyph: Svg, expanded: bool, color: Hsla) -> Svg {
    glyph
        .with_transformation(Transformation::rotate(radians(if expanded {
            std::f32::consts::FRAC_PI_2
        } else {
            0.0
        })))
        .size(ui_rems(TREE_CHEVRON))
        .text_color(color)
}

/// A search or text search row's chevron: the host passes the down glyph
/// while expanded and the right one otherwise.
pub fn search_chevron(glyph: Svg, color: Hsla) -> Svg {
    glyph
        .size(ui_rems(SEARCH_CHEVRON))
        .flex_none()
        .text_color(color)
}

/// The file-type artwork, `SLOT` square.
pub fn artwork<E: Styled>(art: E) -> E {
    art.size(ui_rems(SLOT)).flex_none()
}

/// An entry's name, truncated.
pub fn name(text: impl Into<SharedString>, color: Hsla, font: SharedString) -> Div {
    div()
        .min_w_0()
        .truncate()
        .font_family(font)
        .text_size(ui_rems(NAME_TEXT))
        .text_color(color)
        .child(text.into())
}

/// What a lazily listed folder's status row says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    /// Loading... or Empty folder: a quiet note.
    Note,
    /// A failed listing and its Retry: clickable, the two `STATUS_GAP` apart.
    Error,
    /// Load more...: clickable.
    More,
}

/// Shapes the host's base as a folder's status row: full width, one level
/// deeper than the folder's entries.
pub fn status_row<E: Styled>(base: E, depth: usize, kind: StatusKind) -> E {
    let row = base
        .h(px(ROW_HEIGHT))
        .w_full()
        .flex_none()
        .pl(px(lead(depth) + INDENT))
        .pr(ui_rems(PAD_RIGHT))
        .flex()
        .items_center();
    match kind {
        StatusKind::Note => row,
        StatusKind::Error => row.gap(ui_rems(STATUS_GAP)).cursor_pointer(),
        StatusKind::More => row.cursor_pointer(),
    }
}

/// A status row's label (Loading..., Empty folder, Load more...).
pub fn status_label(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_size(ui_rems(STATUS_TEXT))
        .text_color(color)
        .child(text.into())
}

/// A failed listing's message, truncated before its Retry.
pub fn error_message(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(STATUS_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The Retry after a failed listing.
pub fn retry_label(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(STATUS_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(text.into())
}

/// Which text search row a base becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextRowKind {
    /// A file holding matches: chevron, artwork, name, folder, count.
    File,
    /// One matching line: its number, then the line.
    Match,
}

/// Shapes the host's base as a text search row: full width, `NAME_TEXT` in
/// the host's sans family, a file at `TEXT_FILE_LEAD`, a match at
/// `TEXT_MATCH_LEAD`.
pub fn text_row<E: Styled>(base: E, kind: TextRowKind, font: SharedString) -> E {
    base.h(px(ROW_HEIGHT))
        .w_full()
        .flex_none()
        .pr(ui_rems(PAD_RIGHT))
        .flex()
        .items_center()
        .gap(ui_rems(TEXT_GAP))
        .cursor_pointer()
        .font_family(font)
        .text_size(ui_rems(NAME_TEXT))
        .pl(ui_rems(match kind {
            TextRowKind::File => TEXT_FILE_LEAD,
            TextRowKind::Match => TEXT_MATCH_LEAD,
        }))
}

/// A text search file's name, at most `TEXT_NAME_MAX` wide.
pub fn text_file_name(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .max_w(ui_rems(TEXT_NAME_MAX))
        .truncate()
        .text_color(color)
        .child(text.into())
}

/// A text search file's folder, taking what is left.
pub fn text_file_folder(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .min_w_0()
        .flex_1()
        .truncate()
        .text_color(color)
        .child(text.into())
}

/// A text search file's match count.
pub fn match_count(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(COUNT_TEXT))
        .text_color(color)
        .child(text.into())
}

/// A matching line's number, in its `LINE_COLUMN` column.
pub fn line_number(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .min_w(ui_rems(LINE_COLUMN))
        .text_size(ui_rems(COUNT_TEXT))
        .text_color(color)
        .child(text.into())
}

/// The matching line's room, on one line in the host's mono family. The host
/// adds the line with its hit highlighted.
pub fn match_line(color: Hsla, font: SharedString) -> Div {
    div()
        .min_w_0()
        .flex_1()
        .truncate()
        .whitespace_nowrap()
        .font_family(font)
        .text_color(color)
}
