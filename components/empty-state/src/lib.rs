// PINK's empty and error states, eight shapes:
//
// - The list note (shell.rs `render_chat_sidebar` "sidebar-empty-state"): a
//   list with nothing in it says so in a centred medium title and names the
//   next step under it ("No threads yet", "Start one with ⌘N.").
// - The pane state (browser view.rs `empty_body`): a surface before it has
//   content, or after it failed, centres a tile holding the place's glyph,
//   a title, a description and one bordered action ("Enter an address" with
//   its shortcut; "Try again" after an error).
// - The place state (right_rail.rs `render_place_empty`): a right-rail
//   place with nothing open centres its faint glyph, one line saying so
//   ("No diff open."), an optional notice and the host's button that opens
//   one ("Diff the working tree").
// - The guide (related_chats.rs `render_related_empty`): a list that fills
//   only as the user works says so, then explains each kind of row it will
//   hold beside that kind's glyph, then offers the host's button that starts
//   one ("No related chats yet").
// - The popup note (pickers.rs `empty_list_note`): a popover list with no
//   rows centres one muted line ("No models found").
// - The surface state (surface_chrome.rs `surface_state`): a sidebar
//   surface or document with nothing to show centres its faint glyph (or
//   the host's loader), a title, an optional secondary line and an optional
//   action ("Files couldn't load" with Retry; "Loading files…").
// - The surface message (files/preview.rs `centered_state`): a surface
//   whose state is one line centres the surface state's secondary line
//   alone ("Loading file…", "This file cannot be previewed.").
// - The error note (popover.rs `error_row`): a list that failed to load
//   says why in the danger ink, with room under it for the host's Retry.
//
// UI rethink decision 54: drawn here, consumed by PINK through a pinned
// revision. The package draws only. The host supplies the resolved colours,
// the glyph and the copy; it adds the ids, the status role and accessible
// names, the hover fill and the listeners. Plain `//` comments: the pink-ui
// gallery compiles this file in place with `include!` for its story.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, IntoElement, SharedString, div, prelude::*, px, rems,
};

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

/// The place state's padding (px), the gap between its parts (pt), its
/// glyph (px, fixed like the rail's chrome), its message and notice sizes
/// (pt) and the notice's widest measure (px).
pub const PLACE_PAD: f32 = 16.0;
pub const PLACE_GAP: f32 = 10.0;
pub const PLACE_GLYPH: f32 = 22.0;
pub const PLACE_MESSAGE: f32 = 12.5;
pub const PLACE_NOTICE: f32 = 11.5;
pub const PLACE_NOTICE_WIDTH: f32 = 280.0;

/// The guide's side padding (px, PINK `Theme::SPACE_SM`, matching the list
/// it stands in for), its top and bottom padding, the gap between its lines
/// and its text size (pt).
pub const GUIDE_PAD_X: f32 = 8.0;
pub const GUIDE_PAD_Y: f32 = 10.0;
pub const GUIDE_GAP: f32 = 8.0;
pub const GUIDE_TEXT: f32 = 12.0;
/// A guide line's kind glyph (pt), the gap after it (pt) and how far it
/// drops to sit on the first line of its text (px).
pub const GUIDE_GLYPH: f32 = 13.0;
pub const GUIDE_GLYPH_GAP: f32 = 8.0;
pub const GUIDE_GLYPH_DROP: f32 = 1.0;
/// The extra space above the guide's action, in px.
pub const GUIDE_ACTION_SPACE: f32 = 2.0;

/// The popup note's padding and text size, in pt at the 16 px default.
pub const POPUP_NOTE_PAD_X: f32 = 8.0;
pub const POPUP_NOTE_PAD_Y: f32 = 24.0;
pub const POPUP_NOTE_TEXT: f32 = 12.0;

/// The surface state's side padding, in px (PINK `CONTENT_INSET`, where
/// every surface's content starts). Fixed, like the surface's own insets.
pub const SURFACE_PAD_X: f32 = 12.0;
/// The gap between the surface state's parts, the glyph's extra space
/// under it and the glyph's size, in pt at the 16 px default.
pub const SURFACE_GAP: f32 = 6.0;
pub const SURFACE_GLYPH_SPACE: f32 = 4.0;
pub const SURFACE_GLYPH: f32 = 28.0;
/// The surface state's title size.
pub const SURFACE_TITLE: f32 = 13.0;
/// The secondary line's size, line height and widest measure.
pub const SURFACE_BODY: f32 = 12.0;
pub const SURFACE_BODY_LINE: f32 = 18.0;
pub const SURFACE_BODY_WIDTH: f32 = 280.0;
/// The extra space above the surface state's action.
pub const SURFACE_ACTION_SPACE: f32 = 6.0;

/// The error note's padding, the gap between the message and the host's
/// Retry, and the message's size, in pt at the 16 px default.
pub const ERROR_PAD: f32 = 8.0;
pub const ERROR_GAP: f32 = 6.0;
pub const ERROR_TEXT: f32 = 12.0;

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

/// The place state's frame: fills the place and centres its glyph
/// ([`place_glyph`]), its [`place_message`], an optional [`place_notice`]
/// and the host's button. The host adds the id.
pub fn place() -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(ui_rems(PLACE_GAP))
        .p(px(PLACE_PAD))
}

/// Sizes and tints the host's place glyph (an `svg()` of the place's icon).
pub fn place_glyph<E: Styled>(glyph: E, color: Hsla) -> E {
    glyph.size(px(PLACE_GLYPH)).text_color(color)
}

/// The line saying the place is empty. The host adds the copy.
pub fn place_message(color: Hsla) -> Div {
    div().text_size(ui_rems(PLACE_MESSAGE)).text_color(color)
}

/// A notice under the message (why the place could not fill), centred and
/// wrapping inside `PLACE_NOTICE_WIDTH`. The host adds the copy.
pub fn place_notice(color: Hsla) -> Div {
    div()
        .max_w(px(PLACE_NOTICE_WIDTH))
        .text_center()
        .text_size(ui_rems(PLACE_NOTICE))
        .text_color(color)
}

/// What a guide draws, already resolved by the host.
pub struct GuideStyle {
    /// The heading.
    pub title: Hsla,
    /// The explanations and their glyphs.
    pub text: Hsla,
}

/// The guide's column: the [`guide_title`], a [`guide_line`] per kind and
/// the [`guide_action`]. The host adds the id.
pub fn guide(style: &GuideStyle) -> Div {
    div()
        .px(px(GUIDE_PAD_X))
        .py(ui_rems(GUIDE_PAD_Y))
        .flex()
        .flex_col()
        .gap(ui_rems(GUIDE_GAP))
        .text_size(ui_rems(GUIDE_TEXT))
        .text_color(style.text)
}

/// The guide's heading. The host adds the copy.
pub fn guide_title(style: &GuideStyle) -> Div {
    div().text_color(style.title)
}

/// One kind explained: the host's glyph (an `svg()` of the kind's icon,
/// sized and tinted here) beside its wrapping explanation.
pub fn guide_line<E: Styled + IntoElement>(
    style: &GuideStyle,
    glyph: E,
    text: impl Into<SharedString>,
) -> Div {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(ui_rems(GUIDE_GLYPH_GAP))
        .child(
            glyph
                .size(ui_rems(GUIDE_GLYPH))
                .flex_none()
                .mt(px(GUIDE_GLYPH_DROP))
                .text_color(style.text),
        )
        .child(div().flex_1().min_w_0().child(text.into()))
}

/// The slot for the guide's action. The host adds its button.
pub fn guide_action() -> Div {
    div().pt(px(GUIDE_ACTION_SPACE))
}

/// The popup note: one muted line centred in an empty popover list. The
/// host adds the id and the status role where it has them.
pub fn popup_note(color: Hsla, copy: impl Into<SharedString>) -> Div {
    div()
        .px(ui_rems(POPUP_NOTE_PAD_X))
        .py(ui_rems(POPUP_NOTE_PAD_Y))
        .text_size(ui_rems(POPUP_NOTE_TEXT))
        .text_color(color)
        .text_center()
        .child(copy.into())
}

/// The surface state's frame: takes the surface's free height and centres
/// the [`surface_glyph_slot`], the [`surface_title`], an optional
/// [`surface_body`] and an optional [`surface_action`], `SURFACE_GAP`
/// apart, inside the fixed `SURFACE_PAD_X` side padding. The host adds the
/// id and the debug selector.
pub fn surface() -> Div {
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(ui_rems(SURFACE_GAP))
        .px(px(SURFACE_PAD_X))
}

/// The slot holding the surface state's glyph, `SURFACE_GLYPH_SPACE` above
/// the title. The host adds its [`surface_glyph`] or its loader.
pub fn surface_glyph_slot() -> Div {
    div().mb(ui_rems(SURFACE_GLYPH_SPACE))
}

/// Sizes and tints the host's surface glyph (an `svg()` of the surface's
/// icon) to `SURFACE_GLYPH`.
pub fn surface_glyph<E: Styled>(glyph: E, color: Hsla) -> E {
    glyph.size(ui_rems(SURFACE_GLYPH)).text_color(color)
}

/// The surface state's title, medium weight. The host adds the copy.
pub fn surface_title(color: Hsla) -> Div {
    div()
        .text_size(ui_rems(SURFACE_TITLE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
}

/// The secondary line under the title (why the surface is empty, or the
/// error), centred and wrapping inside `SURFACE_BODY_WIDTH`. The host adds
/// the copy.
pub fn surface_body(color: Hsla) -> Div {
    div()
        .max_w(ui_rems(SURFACE_BODY_WIDTH))
        .text_center()
        .text_size(ui_rems(SURFACE_BODY))
        .line_height(ui_rems(SURFACE_BODY_LINE))
        .text_color(color)
}

/// The slot for the surface state's action, `SURFACE_ACTION_SPACE` lower.
/// The host adds its button or buttons.
pub fn surface_action() -> Div {
    div().mt(ui_rems(SURFACE_ACTION_SPACE))
}

/// The surface message: a surface whose state is one line centres it alone
/// as a [`surface_body`] in the surface's free space, inside the fixed
/// `SURFACE_PAD_X` side padding.
pub fn surface_message(color: Hsla, copy: impl Into<SharedString>) -> Div {
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .px(px(SURFACE_PAD_X))
        .child(surface_body(color).child(copy.into()))
}

/// The error note: why a list failed to load, in the host's danger ink
/// (held readable), padded `ERROR_PAD`. The host adds its Retry after the
/// message, `ERROR_GAP` lower, and the listener on it.
pub fn error_note(color: Hsla, message: impl Into<SharedString>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(ui_rems(ERROR_GAP))
        .p(ui_rems(ERROR_PAD))
        .text_size(ui_rems(ERROR_TEXT))
        .text_color(color)
        .child(message.into())
}
