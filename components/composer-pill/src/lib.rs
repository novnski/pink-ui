// PINK's composer pill (composer.rs `composer_surface`): the rounded,
// bordered surface the composer, the question wizard and the queue tray
// behind them share. UI rethink decision 54: drawn here, consumed by PINK
// through a pinned revision.
//
// The package draws only. The host supplies the colours, whether the pill
// lifts (a frosted surface carries its own depth) and the dock morph's
// progress, and lays out the input, the controls row and everything above
// the pill. Plain `//` comments: the pink-ui gallery compiles this file in
// place with `include!` for its story.

use gpui::{Div, Hsla, div, prelude::*, px};

/// The pill's corner radius when it floats.
pub const RADIUS: f32 = 26.0;
/// How much the corners tighten when the pill docks to the pane's edge.
pub const DOCK_TIGHTEN: f32 = 4.0;
/// The pill's hairline, top plus bottom.
pub const BORDER_V: f32 = 2.0;

/// The corner radius at a dock morph's progress, 0 floating to 1 docked.
pub fn radius(dock_amount: f32) -> f32 {
    RADIUS - DOCK_TIGHTEN * dock_amount
}

/// What the pill draws.
pub struct PillStyle {
    /// The corner radius, usually [`radius`] of the dock morph.
    pub radius: f32,
    pub border: Hsla,
    pub background: Hsla,
    /// Lift the pill with its large shadow; off on a frosted surface.
    pub lifted: bool,
}

/// The pill: a rounded, hairline-bordered surface. The host chains its
/// layout, size and children.
pub fn pill(style: &PillStyle) -> Div {
    div()
        .rounded(px(style.radius))
        .border_1()
        .border_color(style.border)
        .bg(style.background)
        .when(style.lifted, |el| el.shadow_lg())
}
