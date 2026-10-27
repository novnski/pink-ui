// PINK's icon button (shell.rs `window_control_button`,
// `nav_history_button`): the titlebar's sidebar toggle, Back and Forward.
// A square at the scaled control size with the control radius, the host's
// hover wash and a centred glyph. Decision 54: the first bridge primitive,
// drawn here and consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the glyph (its own icon
// assets) and the wash already blended over its hover fade, and adds the
// semantics crates.io GPUI 0.2.2 can't express (role, accessible name,
// focus and its ring), the tooltip, and the behaviour (click, keyboard,
// occluding a titlebar drag strip).

use gpui::{AnyElement, Hsla, SharedString, Stateful, div, prelude::*, px};

/// The 24 pt minimum hit area.
pub const MIN_HIT: f32 = 24.0;
/// Control steps: small, medium (the titlebar's default) and large.
pub const CONTROL_S: f32 = 20.0;
pub const CONTROL_M: f32 = 28.0;
pub const CONTROL_L: f32 = 32.0;
/// Glyph steps.
pub const ICON_S: f32 = 12.0;
pub const ICON_M: f32 = 16.0;
pub const ICON_L: f32 = 18.0;
/// The control radius (PINK `surface_chrome::RADIUS_M`).
pub const RADIUS: f32 = 6.0;

/// A control step grown with the Interface size (`scale` = size / 16),
/// never under the 24 pt minimum hit nor over the large step (PINK
/// `surface_chrome::scaled_control`).
pub fn scaled_control(step: f32, scale: f32) -> f32 {
    (step * scale).clamp(MIN_HIT, CONTROL_L)
}

/// A glyph step grown with the Interface size, between the small and large
/// glyphs (PINK `surface_chrome::scaled_icon`).
pub fn scaled_icon(step: f32, scale: f32) -> f32 {
    (step * scale).clamp(ICON_S, ICON_L)
}

/// What one button draws.
pub struct IconButtonStyle {
    /// The square's side.
    pub size: f32,
    /// Corner radius (usually [`RADIUS`]).
    pub radius: f32,
    /// The wash, already resolved by the host (transparent at rest).
    pub wash: Hsla,
    /// An enabled button shows the pointing cursor; a disabled one keeps
    /// its size and glyph but takes no pointer cursor.
    pub enabled: bool,
}

/// An icon button: `glyph` centred in a `size` square. The host chains its
/// semantics, tooltip, focus ring and actions on the returned element. The
/// button carries its id as a debug selector for geometry checks.
pub fn icon_button(
    id: impl Into<SharedString>,
    glyph: AnyElement,
    style: &IconButtonStyle,
) -> Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let selector = id.to_string();
    div()
        .id(id)
        .debug_selector(move || selector)
        .size(px(style.size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(style.radius))
        .bg(style.wash)
        .when(style.enabled, |button| button.cursor_pointer())
        .child(glyph)
}
