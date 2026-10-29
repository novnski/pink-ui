// PINK's empty and error states, two shapes:
//
// - The list note (shell.rs `render_chat_sidebar` "sidebar-empty-state"): a
//   list with nothing in it says so in a centred medium title and names the
//   next step under it ("No threads yet", "Start one with ⌘N.").
// - The pane state (browser view.rs `empty_body`): a surface before it has
//   content, or after it failed, centres a tile holding the place's glyph,
//   a title, a description and one bordered action ("Enter an address" with
//   its shortcut; "Try again" after an error).
//
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision. The package draws only. The host supplies the resolved colours,
// the glyph and the copy; it adds the ids, the status role and accessible
// names, the hover fill and the listeners. Plain `//` comments: the pink-ui
// gallery compiles this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The list note's padding and the gap between its lines, in px (PINK
/// `Theme::SPACE_SM` / `SPACE_LG`). Fixed, like the list's own insets.
pub const NOTE_PAD_X: f32 = 8.0;
pub const NOTE_PAD_Y: f32 = 16.0;
pub const NOTE_GAP: f32 = 2.0;
/// The list note's title and hint, in pt at the 16 px default.
pub const NOTE_TITLE: f32 = 13.0;
pub const NOTE_HINT: f32 = 12.0;

/// The pane state's padding, its column's widest measure and the gap
/// between the column's parts, in pt at the 16 px default.
pub const PANE_PAD: f32 = 24.0;
pub const PANE_MAX_WIDTH: f32 = 300.0;
pub const PANE_GAP: f32 = 12.0;
/// The glyph tile, its glyph (pt) and its fixed corner radius (px).
pub const TILE: f32 = 44.0;
pub const TILE_GLYPH: f32 = 22.0;
pub const TILE_RADIUS: f32 = 12.0;
/// The title: its size and the extra space above it.
pub const TITLE: f32 = 14.0;
pub const TITLE_SPACE: f32 = 4.0;
/// The description's size and line height.
pub const DESCRIPTION: f32 = 12.0;
pub const DESCRIPTION_LINE: f32 = 19.0;
/// The action: the extra space above it, its height, padding, the gap
/// before its shortcut, its label and shortcut sizes, and its fixed corner
/// radius (px).
pub const ACTION_SPACE: f32 = 6.0;
pub const ACTION_HEIGHT: f32 = 28.0;
pub const ACTION_PAD_X: f32 = 10.0;
pub const ACTION_GAP: f32 = 8.0;
pub const ACTION_TEXT: f32 = 12.0;
pub const SHORTCUT_TEXT: f32 = 11.0;
pub const ACTION_RADIUS: f32 = 6.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a list note draws, already resolved by the host.
pub struct NoteStyle {
    /// The title: the muted ink held readable.
    pub title: Hsla,
    /// The hint: the faint ink held readable.
    pub hint: Hsla,
}

/// The list note: the title over the hint, centred. The host adds the id,
/// the status role and the accessible name that reads both.
pub fn note(
    style: &NoteStyle,
    title: impl Into<SharedString>,
    hint: impl Into<SharedString>,
) -> Div {
    div()
        .px(px(NOTE_PAD_X))
        .py(px(NOTE_PAD_Y))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(NOTE_GAP))
        .text_center()
        .child(
            div()
                .text_size(ui_rems(NOTE_TITLE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(style.title)
                .child(title.into()),
        )
        .child(
            div()
                .text_size(ui_rems(NOTE_HINT))
                .text_color(style.hint)
                .child(hint.into()),
        )
}

/// The pane state's frame: fills the surface and centres its [`column`].
pub fn pane() -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(ui_rems(PANE_PAD))
}

/// The centred column: the [`tile`], the [`title`], the [`description`] and
/// the [`action`], never wider than `PANE_MAX_WIDTH`.
pub fn column() -> Div {
    div()
        .w_full()
        .max_w(ui_rems(PANE_MAX_WIDTH))
        .flex()
        .flex_col()
        .items_center()
        .gap(ui_rems(PANE_GAP))
}

/// What the glyph tile draws, already resolved by the host.
pub struct TileStyle {
    pub edge: Hsla,
    pub fill: Hsla,
}

/// The glyph tile. The host adds the place's glyph at `TILE_GLYPH` rems.
pub fn tile(style: &TileStyle) -> Div {
    div()
        .size(ui_rems(TILE))
        .rounded(px(TILE_RADIUS))
        .border_1()
        .border_color(style.edge)
        .bg(style.fill)
        .flex()
        .items_center()
        .justify_center()
}

/// The pane state's title, medium weight. The host adds the copy.
pub fn title(color: Hsla) -> Div {
    div()
        .mt(ui_rems(TITLE_SPACE))
        .text_size(ui_rems(TITLE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
}

/// The description under the title, centred and wrapping inside the column.
/// The host adds the copy (or the error that replaced it).
pub fn description(color: Hsla) -> Div {
    div()
        .text_center()
        .text_size(ui_rems(DESCRIPTION))
        .line_height(ui_rems(DESCRIPTION_LINE))
        .text_color(color)
}

/// What the action draws, already resolved by the host.
pub struct ActionStyle {
    pub edge: Hsla,
    pub text: Hsla,
}

/// The pane state's action: a bordered row holding the label and, when it
/// has one, the [`shortcut`]. The host adds the id, the pointer, the role
/// and name, the hover fill, the click and the label.
pub fn action(style: &ActionStyle) -> Div {
    div()
        .mt(ui_rems(ACTION_SPACE))
        .h(ui_rems(ACTION_HEIGHT))
        .px(ui_rems(ACTION_PAD_X))
        .rounded(px(ACTION_RADIUS))
        .border_1()
        .border_color(style.edge)
        .flex()
        .items_center()
        .gap(ui_rems(ACTION_GAP))
        .text_size(ui_rems(ACTION_TEXT))
        .text_color(style.text)
}

/// The action's keyboard shortcut after its label. The host adds the keys.
pub fn shortcut(color: Hsla) -> Div {
    div().text_size(ui_rems(SHORTCUT_TEXT)).text_color(color)
}
