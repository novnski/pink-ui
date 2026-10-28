// PINK's context ring (context_usage.rs `render`): the composer controls
// row's context-window chip, a 16 pt ring whose arc fills with the share of
// the window in use, and the share beside it while the row has room. UI
// rethink decision 54: drawn here, consumed by PINK through a pinned
// revision.
//
// The package draws only. The host resolves the tones (muted, warning,
// danger or the compaction accent, floored for contrast on its composer),
// the wash already blended over its hover fade and the label, and adds what
// crates.io GPUI 0.2.2 can't express (role, accessible name, expanded
// state, focus and its ring), the hover card and the behaviour.

use gpui::{
    AbsoluteLength, Hsla, PathBuilder, SharedString, Stateful, canvas, div, point, prelude::*, px,
    rems,
};

/// The ring's side, in pt at the 16 px default.
pub const RING: f32 = 16.0;
/// The ring's radius and stroke, in pixels.
pub const RADIUS: f32 = 6.0;
pub const STROKE: f32 = 1.8;
/// The gap between the ring and the share.
pub const RING_GAP: f32 = 5.0;
/// The chip's horizontal padding.
pub const PAD_X: f32 = 6.0;
/// The chip's height (the controls row's control height).
pub const HEIGHT: f32 = 28.0;
/// The share's type size.
pub const TEXT: f32 = 12.0;

/// A design size (pt at the 16 px default) in rems, so the chip grows with
/// the host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// The chip's `(with label, ring only)` widths at Interface `scale`, given
/// the label's measured width, so a host's row can give the label up before
/// it truncates anything.
pub fn chip_widths(label_width: f32, scale: f32) -> (f32, f32) {
    (
        (2.0 * PAD_X + RING + RING_GAP) * scale + label_width,
        (2.0 * PAD_X + RING) * scale,
    )
}

/// The ring alone: the full `track`, then the arc for `fraction` (clamped
/// to 0..=1, starting at twelve o'clock and running clockwise) in `color`.
pub fn ring(fraction: Option<f32>, track: Hsla, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let center = bounds.center();
            let mut arc = |fraction: f32, color| {
                if fraction <= 0.0 {
                    return;
                }
                let steps = (64.0 * fraction).ceil().max(2.0) as usize;
                let mut path = PathBuilder::stroke(px(STROKE));
                for i in 0..=steps {
                    let angle = -std::f32::consts::FRAC_PI_2
                        + std::f32::consts::TAU * fraction * i as f32 / steps as f32;
                    let p = point(
                        center.x + px(RADIUS * angle.cos()),
                        center.y + px(RADIUS * angle.sin()),
                    );
                    if i == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            };
            arc(1.0, track);
            arc(fraction.unwrap_or(0.0).clamp(0.0, 1.0), color);
        },
    )
    .size(ui_rems(RING))
}

/// What one chip draws.
pub struct ContextRingStyle {
    /// The share's colour.
    pub text: Hsla,
    /// The filled arc's colour.
    pub arc: Hsla,
    /// The unfilled track's colour.
    pub track: Hsla,
    /// The wash (transparent at rest, the hover or open fill otherwise).
    pub wash: Hsla,
}

/// The context-window chip: the ring and, when given, the share. The host chains its semantics, hover card, focus ring and
/// actions on the returned element. The chip carries its id as a debug
/// selector for geometry checks.
pub fn context_ring(
    id: impl Into<SharedString>,
    fraction: Option<f32>,
    label: Option<SharedString>,
    style: &ContextRingStyle,
) -> Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let selector = id.to_string();
    div()
        .id(id)
        .debug_selector(move || selector)
        .cursor_pointer()
        .flex_none()
        .flex()
        .items_center()
        .gap(ui_rems(RING_GAP))
        .h(ui_rems(HEIGHT))
        .px(ui_rems(PAD_X))
        .rounded_full()
        .text_size(ui_rems(TEXT))
        .text_color(style.text)
        .bg(style.wash)
        .child(ring(fraction, style.track, style.arc))
        // The host sets tabular figures on the chip when it shows the
        // share (crates.io GPUI 0.2.2 has no font features), so the share
        // never shifts as it ticks.
        .when_some(label, |chip, label| chip.child(label))
}
