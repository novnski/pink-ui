// PINK's composer discs (composer.rs `render_send_button`,
// `render_stop_button` and the Attach button): the round primary button
// that sends, queues or steers (and stops when nothing else is offered),
// the separate Stop beside it while a run is live, and the quiet Attach. UI rethink decision 54: drawn here,
// consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the glyph (its own arrow
// icons, or `stop_glyph`), the fill already blended over its hover fade and
// whether the disc can act, and adds what crates.io GPUI 0.2.2 can't
// express (role, accessible name, focus and its ring), the tooltip, the
// glyph's cross-fade and the behaviour (click, keyboard).

use gpui::{AbsoluteLength, AnyElement, Hsla, SharedString, Stateful, div, prelude::*, px, rems};

/// The arrow glyphs' side (Send, Queue, Steer), in pt at the 16 px default.
pub const GLYPH: f32 = 14.0;
/// The stop square's side, in pt at the 16 px default.
pub const STOP_GLYPH: f32 = 11.0;
/// The stop square's corner radius.
pub const STOP_RADIUS: f32 = 3.0;
/// How far a pressed disc dims.
pub const PRESSED_OPACITY: f32 = 0.76;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What one disc draws.
pub struct DiscStyle {
    /// The disc's diameter.
    pub size: AbsoluteLength,
    /// The fill, already resolved by the host (solid, recessed or quiet).
    pub fill: Hsla,
    /// The glyph colour, inherited by a glyph that takes the text colour.
    pub glyph: Hsla,
    /// A disc that can act shows the pointing cursor; a recessed one keeps
    /// its shape without it.
    pub actionable: bool,
    /// An actionable disc that dims to `PRESSED_OPACITY` while pressed
    /// (Send and Stop do; the quiet Attach doesn't).
    pub press_dim: bool,
}

/// A round disc with `glyph` centred. The host chains its semantics,
/// tooltip, focus ring and actions on the returned element. The disc
/// carries its id as a debug selector for geometry checks.
pub fn disc(
    id: impl Into<SharedString>,
    glyph: AnyElement,
    style: &DiscStyle,
) -> Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let selector = id.to_string();
    div()
        .id(id)
        .debug_selector(move || selector)
        .size(style.size)
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(style.glyph)
        .bg(style.fill)
        .when(style.actionable, |disc| disc.cursor_pointer())
        .when(style.actionable && style.press_dim, |disc| {
            disc.active(|pressed| pressed.opacity(PRESSED_OPACITY))
        })
        .child(glyph)
}

/// The stop square: `STOP_GLYPH` in rems with `STOP_RADIUS` corners.
pub fn stop_glyph(color: Hsla) -> gpui::Div {
    div()
        .size(ui_rems(STOP_GLYPH))
        .rounded(px(STOP_RADIUS))
        .bg(color)
}
