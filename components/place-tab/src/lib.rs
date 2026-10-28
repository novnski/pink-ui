// PINK's right-pane place tab row (shell.rs `render_place_tabs`): the row
// above what a place shows, one fixed-width tab per open instance (its glyph
// that turns into the close control under the pointer, the truncating title
// and the unsaved dot), the Agents place's pinned home tab, the add button
// and the edge fades over a scrolled strip. UI rethink decision 54: drawn
// here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the text and glyph colours, the
// selected fill (a resting tab leaves its background to the host's hover
// wash), the add button's fill already blended over its hover fade, the
// glyphs, the spinner, the favicon and the titles; it adds the ids, the
// scroller, the role, accessible names and focus rings, the tooltips, the
// click, menu, middle-click and drag listeners, the slide animation and the
// measurement. Plain `//` comments: the pink-ui gallery compiles this file in
// place with `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, SharedString, div, prelude::*, px, rems};

/// The row's height and horizontal padding. Fixed: the row does not grow
/// with the Interface size.
pub const BAR_HEIGHT: f32 = 36.0;
pub const BAR_PAD_X: f32 = 8.0;
/// The gap between the row's parts, and between tabs in the strip.
pub const BAR_GAP: f32 = 4.0;
/// A tab's fixed width: the strip's drag mechanics (drop-index quantisation
/// and slide offsets) assume uniform tabs.
pub const TAB_WIDTH: f32 = 112.0;
/// One tab's slot along the strip: the tab and the gap after it.
pub const TAB_SLOT: f32 = TAB_WIDTH + BAR_GAP;
/// A tab's height and corner radius (the canvas tabs' recipe).
pub const TAB_HEIGHT: f32 = 28.0;
pub const TAB_RADIUS: f32 = 8.0;
/// A tab's leading and trailing padding: the glyph slot carries the rest of
/// the leading inset.
pub const PAD_LEFT: f32 = 5.0;
pub const PAD_RIGHT: f32 = 8.0;
/// The gap between a tab's parts.
pub const PART_GAP: f32 = 3.0;
/// A tab's label, in pt at the 16 px default.
pub const TAB_TEXT: f32 = 12.5;
/// The leading slot (the glyph, the close control under the pointer): the
/// small control step and its radius.
pub const SLOT_SIZE: f32 = 20.0;
pub const SLOT_RADIUS: f32 = 4.0;
/// A place's glyph and the close glyph in the slot.
pub const GLYPH: f32 = 12.0;
/// A file's type icon in the slot.
pub const FILE_GLYPH: f32 = 14.0;
/// An inactive file icon's opacity.
pub const FILE_GLYPH_MUTED: f32 = 0.78;
/// The unsaved dot's diameter.
pub const DIRTY_DOT: f32 = 6.0;
/// The spacer a dragged tab leaves behind (the ghost carries the tab).
pub const DRAG_SPACER_HEIGHT: f32 = 24.0;
/// The Agents place's pinned home tab: its padding, gap and glyph.
pub const HOME_PAD_X: f32 = 8.0;
pub const HOME_GAP: f32 = 6.0;
pub const HOME_GLYPH: f32 = 13.0;
/// The add button: the medium control step, its radius and glyph.
pub const ADD_SIZE: f32 = 28.0;
pub const ADD_RADIUS: f32 = 6.0;
pub const ADD_GLYPH: f32 = 13.0;
/// The fade over whichever edge of the strip hides tabs.
pub const FADE_WIDTH: f32 = 36.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The place's tab row: the home tab, the strip's region and the add
/// button in a line. The host adds the id and the children.
pub fn bar() -> Div {
    div()
        .h(px(BAR_HEIGHT))
        .flex_none()
        .w_full()
        .min_w_0()
        .px(px(BAR_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(BAR_GAP))
}

/// The strip the tabs sit in. The host adds the id, the horizontal scroller
/// and the drag and drop listeners.
pub fn strip() -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(BAR_GAP))
        .min_w_0()
}

/// What one tab draws.
pub struct TabStyle {
    /// The selected fill; `None` leaves the background to the host's hover
    /// wash.
    pub fill: Option<Hsla>,
}

/// One tab: its slot, title and dot in a line, at the fixed width. The host
/// adds the id, the hover group and wash and everything interactive.
pub fn tab(style: &TabStyle) -> Div {
    let tab = div()
        .h(px(TAB_HEIGHT))
        .w(px(TAB_WIDTH))
        .flex_none()
        .pl(px(PAD_LEFT))
        .pr(px(PAD_RIGHT))
        .rounded(px(TAB_RADIUS))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(PART_GAP))
        .cursor_pointer();
    match style.fill {
        Some(fill) => tab.bg(fill),
        None => tab,
    }
}

/// The leading slot: the glyph's and the close control's shared box. The
/// host adds the id, the close control's hover fill and click, and the two
/// layers.
pub fn slot() -> Div {
    div()
        .flex_none()
        .size(px(SLOT_SIZE))
        .rounded(px(SLOT_RADIUS))
        .relative()
}

/// The slot's glyph layer: shown at rest, hidden while the pointer is over
/// the tab's hover `group`. The host adds the glyph.
pub fn glyph_layer(group: impl Into<SharedString>) -> Div {
    layer().group_hover(group, |style| style.opacity(0.0))
}

/// The slot's close layer: hidden at rest, shown while the pointer is over
/// the tab's hover `group`. The host adds the close glyph.
pub fn close_layer(group: impl Into<SharedString>) -> Div {
    layer()
        .opacity(0.0)
        .group_hover(group, |style| style.opacity(1.0))
}

fn layer() -> Div {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
}

/// The tab's title: it takes the room that is left and truncates.
pub fn title(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .min_w_0()
        .truncate()
        .text_size(ui_rems(TAB_TEXT))
        .text_color(color)
        .child(text.into())
}

/// Unsaved changes: a small dot after the title.
pub fn dirty_dot(color: Hsla) -> Div {
    div()
        .flex_none()
        .size(px(DIRTY_DOT))
        .rounded_full()
        .bg(color)
}

/// The room a dragged tab leaves in the strip.
pub fn drag_spacer() -> Div {
    div().w(px(TAB_WIDTH)).h(px(DRAG_SPACER_HEIGHT)).flex_none()
}

/// What the home tab draws.
pub struct HomeStyle {
    /// The label's colour, already resolved for the selection.
    pub text: Hsla,
    /// The selected fill; `None` leaves the background to the host's hover
    /// wash.
    pub fill: Option<Hsla>,
}

/// The Agents place's pinned home tab: never closed, dragged or scrolled
/// away. The host adds the id, the glyph, the label and everything
/// interactive.
pub fn home_tab(style: &HomeStyle) -> Div {
    let tab = div()
        .h(px(TAB_HEIGHT))
        .px(px(HOME_PAD_X))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HOME_GAP))
        .rounded(px(TAB_RADIUS))
        .cursor_pointer()
        .text_size(ui_rems(TAB_TEXT))
        .text_color(style.text);
    match style.fill {
        Some(fill) => tab.bg(fill),
        None => tab,
    }
}

/// The add button pinned after the strip: `fill` is already blended over
/// the host's hover fade. The host adds the id, the glyph and everything
/// interactive.
pub fn add_button(fill: Hsla) -> Div {
    div()
        .size(px(ADD_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(ADD_RADIUS))
        .cursor_pointer()
        .bg(fill)
}

/// Which edge of the strip a fade covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
}

/// A painted fade over one edge of the strip, from the row's surface tone
/// to clear, for a surface that cannot fade its glyphs in place.
pub fn edge_fade(edge: Edge, surface: Hsla) -> Div {
    let fade = div().absolute();
    let (fade, angle) = match edge {
        Edge::Left => (fade.left_0(), 90.0),
        Edge::Right => (fade.right_0(), 270.0),
    };
    fade.top_0()
        .bottom_0()
        .w(px(FADE_WIDTH))
        .bg(gpui::linear_gradient(
            angle,
            gpui::linear_color_stop(surface, 0.0),
            gpui::linear_color_stop(surface.opacity(0.0), 1.0),
        ))
}
