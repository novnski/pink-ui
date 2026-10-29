// PINK's diff row chrome (changes.rs `notice_row`, `hunk_header_row`,
// `meta_line_row`, `diff_line_row`, `split_line_cell`, `split_filler` and
// `split_row`): the notice under a file header ("New file", "Binary file"),
// the `@@ ... @@` band over each hunk, the italic note a line can carry
// ("\ No newline at end of file"), and the columns of a diff line: the
// accent bar spacer, the line-number gutters, the `+ - .` marker column and
// the room for the code, unified on one row or split around a hairline.
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host supplies the code itself (shaped,
// syntax-highlighted, word-washed, clipped or scrolled sideways), every
// colour already resolved, the mono family and the sizes it derives from the
// code font setting; it adds the comment adder, hover tracking, folding,
// the virtualized list and the sticky file header. Plain `//` comments: the
// pink-ui gallery compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, AlignItems, AnyElement, Div, Hsla, SharedString, div, prelude::*, px, rems,
};

/// A hunk's `@@` band height, in fixed pixels (PINK `HUNK_HEADER_HEIGHT`).
pub const HUNK_HEADER_HEIGHT: f32 = 28.0;
/// A notice row's height, in fixed pixels (PINK `NOTICE_HEIGHT`).
pub const NOTICE_HEIGHT: f32 = 24.0;
/// A notice's side inset, in fixed pixels (PINK `Theme::SPACE_LG`).
pub const NOTICE_PAD_X: f32 = 16.0;
/// A notice's text, in pt at the 16 px default.
pub const NOTICE_TEXT: f32 = 11.0;
/// The `@@` band's side inset, in fixed pixels (PINK `CONTENT_INSET`).
pub const HUNK_PAD_X: f32 = 12.0;
/// The accent bar column before the gutters, in fixed pixels.
pub const ACCENT_BAR_WIDTH: f32 = 3.0;
/// The narrowest line-number gutter, in fixed pixels (PINK `GUTTER_WIDTH`).
pub const GUTTER_WIDTH: f32 = 36.0;
/// A gutter's inset after its number, in fixed pixels.
pub const GUTTER_PAD_RIGHT: f32 = 8.0;
/// The marker column on a unified row and on a split half, in fixed pixels.
pub const MARKER_WIDTH: f32 = 16.0;
pub const SPLIT_MARKER_WIDTH: f32 = 18.0;
/// The hairline between a split row's halves, in fixed pixels.
pub const SPLIT_DIVIDER_WIDTH: f32 = 1.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

// Stretch on the cross axis, spelled through the style so the file compiles
// on crates.io GPUI 0.2.2 (which has no `self_stretch`/`items_stretch`).
fn self_stretch<E: Styled>(mut el: E) -> E {
    el.style().align_self = Some(AlignItems::Stretch);
    el
}

fn items_stretch<E: Styled>(mut el: E) -> E {
    el.style().align_items = Some(AlignItems::Stretch);
    el
}

/// The host's code metrics and the chrome colours that do not depend on the
/// row. Sizes are in fixed pixels: they follow the code font setting, not
/// the Interface size.
#[derive(Clone, Debug)]
pub struct DiffStyle {
    /// The mono family for numbers, markers and the `@@` band.
    pub mono: SharedString,
    /// The code's size (PINK: 12 at the default code font size).
    pub text_size: f32,
    /// The gutters' and the `@@` band's size (PINK: 11 at the default).
    pub meta_size: f32,
    /// A note row's size (PINK: 10.5 at the default).
    pub note_size: f32,
    /// A diff line's height (PINK: 21 at the default).
    pub line_height: f32,
    /// The `@@` band's fill (PINK `diff_hunk_bg`).
    pub hunk_fill: Hsla,
    /// The `@@` band's text, floored on that fill.
    pub hunk_text: Hsla,
    /// Notices and notes (PINK `text_faint`).
    pub faint: Hsla,
    /// The empty half of a one-sided split row (PINK `ink(0.03)`).
    pub filler: Hsla,
    /// The hairline between split halves (PINK `hairline(0.06)`).
    pub divider: Hsla,
}

/// One notice row under a file header, in the Interface size.
pub fn notice_row(text: impl Into<SharedString>, style: &DiffStyle) -> Div {
    div()
        .h(px(NOTICE_HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .px(px(NOTICE_PAD_X))
        .text_size(ui_rems(NOTICE_TEXT))
        .text_color(style.faint)
        .child(text.into())
}

/// One `@@ ... @@` band over a hunk.
pub fn hunk_header_row(header: impl Into<SharedString>, style: &DiffStyle) -> Div {
    div()
        .h(px(HUNK_HEADER_HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .px(px(HUNK_PAD_X))
        .bg(style.hunk_fill)
        .font_family(style.mono.clone())
        .text_size(px(style.meta_size))
        .text_color(style.hunk_text)
        .child(header.into())
}

/// A note about a row rather than code, indented `pad_left` past the
/// columns and never tinted. In split mode it spans both halves.
pub fn meta_line_row(text: impl Into<SharedString>, pad_left: f32, style: &DiffStyle) -> Div {
    div()
        .h(px(style.line_height))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .pl(px(pad_left))
        .text_size(px(style.note_size))
        .text_color(style.faint)
        .italic()
        .child(text.into())
}

/// The note's indent on a unified row: past the bar, both gutters, the
/// marker and the code's lead.
pub fn unified_meta_pad(gutter: f32, code_lead: f32) -> f32 {
    ACCENT_BAR_WIDTH + 2.0 * gutter + MARKER_WIDTH + code_lead
}

/// The note's indent on a split row: past both halves' bars and gutters.
pub fn split_meta_pad(gutter: f32) -> f32 {
    2.0 * (ACCENT_BAR_WIDTH + gutter)
}

/// A unified row: one line tall (at least, when wrapping), on the row's
/// wash. The host adds `accent_bar`, the two gutters, the marker and the
/// code.
pub fn unified_row(wash: Option<Hsla>, wrapped: bool, style: &DiffStyle) -> Div {
    let row = div();
    let row = if wrapped {
        row.min_h(px(style.line_height))
    } else {
        row.h(px(style.line_height))
    };
    let row = row
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_start();
    match wash {
        Some(wash) => row.bg(wash),
        None => row,
    }
}

/// The accent bar column: solid on a changed row when the host paints one,
/// an invisible spacer otherwise so the columns always align.
pub fn accent_bar(color: Option<Hsla>) -> Div {
    let bar = self_stretch(div().w(px(ACCENT_BAR_WIDTH))).flex_none();
    match color {
        Some(color) => bar.bg(color),
        None => bar,
    }
}

/// One line-number gutter, `width` wide (the host fits it to the file's
/// largest number), the number right-aligned.
pub fn gutter(number: Option<u32>, color: Hsla, width: f32, style: &DiffStyle) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .font_family(style.mono.clone())
        .text_size(px(style.meta_size))
        .line_height(px(style.line_height))
        .text_color(color)
        .flex()
        .justify_end()
        .pr(px(GUTTER_PAD_RIGHT))
        .child(SharedString::from(
            number.map(|n| n.to_string()).unwrap_or_default(),
        ))
}

/// The `+ - .` marker column (`MARKER_WIDTH` unified, `SPLIT_MARKER_WIDTH`
/// on a split half).
pub fn marker(text: impl Into<SharedString>, color: Hsla, width: f32, style: &DiffStyle) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .flex()
        .justify_center()
        .text_size(px(style.text_size))
        .line_height(px(style.line_height))
        .text_color(color)
        .font_family(style.mono.clone())
        .child(text.into())
}

/// One half of a split row, on its line's wash. The host adds
/// `accent_bar`, its one gutter, the marker and the code.
pub fn split_half(wash: Option<Hsla>) -> Div {
    let half = self_stretch(div().flex_1().min_w_0())
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_start();
    match wash {
        Some(wash) => half.bg(wash),
        None => half,
    }
}

/// The empty half of a one-sided split row.
pub fn split_filler(style: &DiffStyle) -> Div {
    self_stretch(div().flex_1().min_w_0()).bg(style.filler)
}

/// A split row: the two halves around the hairline.
pub fn split_row(left: AnyElement, right: AnyElement, wrapped: bool, style: &DiffStyle) -> Div {
    let row = div();
    let row = if wrapped {
        row.min_h(px(style.line_height))
    } else {
        row.h(px(style.line_height))
    };
    items_stretch(row.w_full().flex_none().flex().flex_row())
        .child(left)
        .child(
            self_stretch(div().w(px(SPLIT_DIVIDER_WIDTH)))
                .flex_none()
                .bg(style.divider),
        )
        .child(right)
}
