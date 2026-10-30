// PINK's prompt library (composer.rs `render_stash_menu` and the template
// fill panel): a saved template's row says how many blanks it has, and the
// open template is a sentence whose blanks are typed straight into, painted
// like the composer's inline chips. Drawn here, consumed by PINK through a
// pinned revision.
//
// The package draws only. The host parses its template format into
// `Part`s, supplies the text fields that sit inside the blanks, the colours
// already resolved (each blank's fill eased over its focus and filled
// fades), and adds the roles, accessible names, focus, keyboard handling
// and the actions. Plain `//` comments: the pink-ui gallery compiles this
// file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, Pixels, SharedString, div, prelude::*, px, rems};

/// The saved-prompt list's height cap (PINK `render_stash_menu`).
pub const STASH_LIST_MAX_H: f32 = 220.0;
/// The blank count at the end of a template's menu row.
pub const COUNT_TEXT: f32 = 11.0;
/// Inline chip geometry (PINK composer.rs chip quads): the wash sits 2 px
/// inside the line box top and bottom, at radius 5.
pub const CHIP_INSET_Y: f32 = 2.0;
pub const CHIP_RADIUS: f32 = 5.0;
/// Side room inside an editable blank, so the caret never touches its edge.
pub const CHIP_PAD_X: f32 = 4.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// One piece of a template as the host parsed it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part<'a> {
    Text(&'a str),
    Blank(&'a str),
}

/// One piece of the sentence: a word, or a blank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Word(String),
    Blank(String),
}

/// The sentence as unbreakable groups: words wrap freely, and a blank stays
/// glued to the punctuation right before or after it. Line breaks in the
/// template read as spaces.
pub fn atoms<'a>(parts: impl IntoIterator<Item = Part<'a>>) -> Vec<Vec<Piece>> {
    let mut atoms: Vec<Vec<Piece>> = vec![Vec::new()];
    for part in parts {
        match part {
            Part::Text(text) => {
                for (i, chunk) in text.split([' ', '\n', '\t']).enumerate() {
                    if i > 0 {
                        atoms.push(Vec::new());
                    }
                    if !chunk.is_empty()
                        && let Some(atom) = atoms.last_mut()
                    {
                        atom.push(Piece::Word(chunk.to_string()));
                    }
                }
            }
            Part::Blank(name) => {
                if let Some(atom) = atoms.last_mut() {
                    atom.push(Piece::Blank(name.to_string()));
                }
            }
        }
    }
    atoms.retain(|atom| !atom.is_empty());
    atoms
}

/// "1 blank", "3 blanks".
pub fn blank_count_label(count: usize) -> SharedString {
    format!("{count} blank{}", if count == 1 { "" } else { "s" }).into()
}

/// The quiet blank count at the end of a template's menu row.
pub fn blank_count(color: Hsla, count: usize) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(COUNT_TEXT))
        .text_color(color)
        .child(blank_count_label(count))
}

/// The wrapping sentence; `space` is one space at the input's size.
pub fn sentence(space: Pixels) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(space)
}

/// One unbreakable group of the sentence, a line box tall.
pub fn group(line_height: Pixels) -> Div {
    div()
        .min_h(line_height)
        .flex()
        .flex_row()
        .items_center()
}

/// A blank's colours, already eased by the host between the empty,
/// focused and filled looks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChipStyle {
    pub fill: Hsla,
    pub ink: Hsla,
}

/// The composer's inline folder chip: `accent` ink on accent at 0.17 dark
/// or 0.10 light.
pub fn filled_palette(accent: Hsla, dark: bool) -> ChipStyle {
    ChipStyle {
        fill: accent.opacity(if dark { 0.17 } else { 0.10 }),
        ink: accent,
    }
}

/// A blank as wide as its text (or its name while empty), with the side
/// room and the caret.
pub fn chip_width(text_width: Pixels, caret: Pixels, pad_x: Pixels) -> Pixels {
    text_width + pad_x * 2.0 + caret
}

fn chip(style: &ChipStyle, line_height: Pixels, pad_x: Pixels) -> Div {
    let height = line_height - px(2.0 * CHIP_INSET_Y);
    div()
        .flex_none()
        .h(height)
        .px(pad_x)
        .flex()
        .items_center()
        .rounded(px(CHIP_RADIUS))
        .bg(style.fill)
        .text_color(style.ink)
        .line_height(height)
}

/// An editable blank: the host puts its text field inside and adds focus
/// on click. `width` comes from `chip_width`.
pub fn blank_chip(style: &ChipStyle, width: Pixels, line_height: Pixels, pad_x: Pixels) -> Div {
    chip(style, line_height, pad_x).w(width).overflow_hidden()
}

/// A repeated blank: the same chip, showing what the first one holds.
pub fn echo_chip(
    style: &ChipStyle,
    text: impl Into<SharedString>,
    line_height: Pixels,
    pad_x: Pixels,
) -> Div {
    chip(style, line_height, pad_x).child(text.into())
}
