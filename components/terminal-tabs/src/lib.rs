// PINK's terminal tab strip (terminal/panel.rs `render_tab_bar` and
// `TabGhost`): the band above the terminal drawer with one fixed-width tab
// per shell session (its terminal glyph, the truncating title and the close
// control the selected tab shows), the add button, the collapse chevron
// pinned right, the room a dragged tab leaves and the ghost that follows the
// pointer. UI rethink decision 54: drawn here, consumed by PINK through a
// pinned revision.
//
// The package draws only. The host supplies the text and glyph colours, the
// tab and button fills already blended over its hover fade, the exited
// tab's dimming, the glyphs and the titles; it adds the ids, the hover,
// click, middle-click and drag listeners, the drop target, the slide
// animation and the focus. Plain `//` comments: the pink-ui gallery compiles
// this file in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, Hsla, SharedString, div, prelude::*, px, rems, svg};

/// The band's height, its leading and trailing padding and the gap between
/// its parts. Fixed: the band is the shared header height and does not grow
/// with the Interface size.
pub const BAR_HEIGHT: f32 = 38.0;
pub const BAR_PAD_LEFT: f32 = 8.0;
pub const BAR_PAD_RIGHT: f32 = 6.0;
pub const BAR_GAP: f32 = 4.0;
/// A tab's fixed width: the strip's drag mechanics (drop-index quantisation
/// and slide offsets) assume uniform tabs.
pub const TAB_WIDTH: f32 = 118.0;
/// A tab's height and corner radius.
pub const TAB_HEIGHT: f32 = 28.0;
pub const TAB_RADIUS: f32 = 8.0;
/// A tab's leading and trailing padding: the close control's own inset
/// carries the rest of the trailing room.
pub const TAB_PAD_LEFT: f32 = 8.0;
pub const TAB_PAD_RIGHT: f32 = 4.0;
/// The gap between a tab's parts.
pub const PART_GAP: f32 = 6.0;
/// A tab's label, in pt at the 16 px default.
pub const TAB_TEXT: f32 = 12.0;
/// The terminal glyph that leads a tab.
pub const GLYPH: f32 = 16.0;
/// The close control: its box, corner radius and glyph.
pub const CLOSE_SIZE: f32 = 20.0;
pub const CLOSE_RADIUS: f32 = 6.0;
pub const CLOSE_GLYPH: f32 = 12.0;
/// The add and collapse buttons: their box and corner radius.
pub const BUTTON_SIZE: f32 = 28.0;
pub const BUTTON_RADIUS: f32 = 8.0;
/// The add button's plus and the collapse button's chevron.
pub const ADD_GLYPH: f32 = 16.0;
pub const COLLAPSE_GLYPH: f32 = 13.0;
/// An exited session's tab dims to this opacity on a dark and on a light
/// appearance.
pub const EXITED_OPACITY_DARK: f32 = 0.55;
pub const EXITED_OPACITY_LIGHT: f32 = 0.8;
/// The drag ghost: its horizontal padding, corner radius and opacity.
pub const GHOST_PAD_X: f32 = 8.0;
pub const GHOST_RADIUS: f32 = 6.0;
pub const GHOST_OPACITY: f32 = 0.85;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The band: the tabs, the add button, `spacer` and the collapse button in
/// a line over a 1 px `edge` along the bottom. No fill of its own: the
/// drawer's plate shows through. The host adds the id, the drag-move and
/// drop listeners and the children.
pub fn bar(edge: Hsla) -> Div {
    div()
        .h(px(BAR_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(BAR_GAP))
        .pl(px(BAR_PAD_LEFT))
        .pr(px(BAR_PAD_RIGHT))
        .border_b_1()
        .border_color(edge)
}

/// What one tab draws.
pub struct TabStyle {
    /// The selected or resting fill, already blended over the host's hover
    /// fade.
    pub fill: Hsla,
    /// The title's colour.
    pub text: Hsla,
    /// The terminal glyph's colour.
    pub glyph: Hsla,
    /// An exited session's dimming (`EXITED_OPACITY_DARK` or
    /// `EXITED_OPACITY_LIGHT`); `None` for a live one.
    pub opacity: Option<f32>,
}

/// One tab at the fixed width: the terminal glyph at `glyph_path`, the
/// title that takes the room left and truncates, then `close` (a
/// `close_button`). The host adds the id and everything interactive.
pub fn tab(
    style: &TabStyle,
    glyph_path: impl Into<SharedString>,
    title: impl Into<SharedString>,
    close: impl IntoElement,
) -> Div {
    let tab = div()
        .w(px(TAB_WIDTH))
        .h(px(TAB_HEIGHT))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(PART_GAP))
        .pl(px(TAB_PAD_LEFT))
        .pr(px(TAB_PAD_RIGHT))
        .rounded(px(TAB_RADIUS))
        .bg(style.fill)
        .text_size(ui_rems(TAB_TEXT))
        .text_color(style.text)
        .cursor_pointer();
    let tab = match style.opacity {
        Some(opacity) => tab.opacity(opacity),
        None => tab,
    };
    tab.child(
        svg()
            .path(glyph_path)
            .flex_none()
            .size(px(GLYPH))
            .text_color(style.glyph),
    )
    .child(div().flex_1().min_w_0().truncate().child(title.into()))
    .child(close)
}

/// What a tab's close control draws.
pub struct CloseStyle {
    /// The fill, already blended over the host's hover fade.
    pub fill: Hsla,
    pub glyph: Hsla,
    /// Only the selected tab shows its close control; the others keep the
    /// room, so titles never shift.
    pub visible: bool,
}

/// A tab's close control: the glyph at `glyph_path` in a `CLOSE_SIZE`
/// box. The host adds the id, the hover listener and the click.
pub fn close_button(style: &CloseStyle, glyph_path: impl Into<SharedString>) -> Div {
    let close = div()
        .size(px(CLOSE_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(CLOSE_RADIUS));
    let close = if style.visible {
        close
    } else {
        close.invisible()
    };
    close.cursor_pointer().bg(style.fill).child(
        svg()
            .path(glyph_path)
            .flex_none()
            .size(px(CLOSE_GLYPH))
            .text_color(style.glyph),
    )
}

/// What the add and collapse buttons draw.
pub struct ButtonStyle {
    /// The fill, already blended over the host's hover fade.
    pub fill: Hsla,
    pub glyph: Hsla,
}

/// The add button after the tabs: the plus at `glyph_path`. The host adds
/// the id, the hover listener and the click.
pub fn add_button(style: &ButtonStyle, glyph_path: impl Into<SharedString>) -> Div {
    button(style, glyph_path.into(), ADD_GLYPH)
}

/// The collapse button pinned right: the chevron at `glyph_path`. The host
/// adds the id, the hover listener and the click.
pub fn collapse_button(style: &ButtonStyle, glyph_path: impl Into<SharedString>) -> Div {
    button(style, glyph_path.into(), COLLAPSE_GLYPH)
}

fn button(style: &ButtonStyle, glyph_path: SharedString, glyph: f32) -> Div {
    div()
        .size(px(BUTTON_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(BUTTON_RADIUS))
        .cursor_pointer()
        .bg(style.fill)
        .child(
            svg()
                .path(glyph_path)
                .flex_none()
                .size(px(glyph))
                .text_color(style.glyph),
        )
}

/// The room between the add button and the collapse button, which pins the
/// collapse button right.
pub fn spacer() -> Div {
    div().flex_1()
}

/// The room a dragged tab leaves in the strip (the ghost carries the tab).
pub fn drag_spacer() -> Div {
    div().w(px(TAB_WIDTH)).h(px(TAB_HEIGHT)).flex_none()
}

/// What the drag ghost draws.
pub struct GhostStyle {
    /// The raised surface it floats on.
    pub fill: Hsla,
    /// Its 1 px edge.
    pub edge: Hsla,
    pub text: Hsla,
}

/// The tab that follows the pointer while dragging: the title on a raised
/// plate at `GHOST_OPACITY`.
pub fn ghost(style: &GhostStyle, title: impl Into<SharedString>) -> Div {
    div()
        .w(px(TAB_WIDTH))
        .h(px(TAB_HEIGHT))
        .px(px(GHOST_PAD_X))
        .flex()
        .items_center()
        .rounded(px(GHOST_RADIUS))
        .bg(style.fill)
        .border_1()
        .border_color(style.edge)
        .text_size(ui_rems(TAB_TEXT))
        .text_color(style.text)
        .opacity(GHOST_OPACITY)
        .child(div().truncate().child(title.into()))
}
