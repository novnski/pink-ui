// PINK's loading indicators (loaders.rs): the 3×3 gradient matrix, the 2×3
// glyph and mono spinners, the upload progress ring and the boot splash's
// visuals. UI rethink decision 54: drawn here, consumed by PINK through a
// pinned revision.
//
// The package draws only. The host owns the clock and the motion: it passes
// each spinner's phase (0..1 of its period, 0 under reduced motion) and the
// pulse curve that turns a cell's phase into its opacity (PINK:
// `gspin_opacity(t, GSPIN_DIM)`), the upload percent, the resolved colours
// and the label. It keeps the timers, each mini spinner's own cached view,
// the splash's fade and the reduced-motion rest frame. The breathing orb is
// `pink-working-trailer`'s `orb`.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, PathBuilder, SharedString, canvas, div, hsla, point,
    prelude::*, px, rems,
};

/// A design size (pt at the 16 px default) in rems, so a loader grows with
/// the host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

// ---------------------------------------------------------------------------
// Gradient matrix (3×3)
// ---------------------------------------------------------------------------

/// The gradient matrix's side, in cells.
pub const MATRIX_SIDE: usize = 3;

/// A gradient-matrix cell's phase offset: its distance from the wave's
/// origin at the bottom centre, `(2 - row) + |col - 1|`, over 4 (row 0 is
/// the top), so the pulse enters at the bottom edge and converges on the
/// top centre (gradient-spin.tsx `--gspin-phase`).
pub fn gradient_cell_phase(row: usize, col: usize) -> f32 {
    let center = (MATRIX_SIDE as f32 - 1.0) / 2.0;
    let max = MATRIX_SIDE as f32 - 1.0 + center;
    let d = MATRIX_SIDE as f32 - 1.0 - row as f32 + (col as f32 - center).abs();
    if max == 0.0 { 0.0 } else { d / (max + 1.0) }
}

/// The gradient matrix spinner (PINK `gradient_spinner`, the
/// "WorkingIndicator"): a 3×3 grid of round `cell` px cells, `cell / 2`
/// apart (so `4 * cell` square), each row tinted from `tints` (`[light, mid,
/// deep]`, top to bottom). Each cell's opacity is `pulse(phase + offset)`
/// with its [`gradient_cell_phase`] offset, so the wave reads as travelling
/// upward. PINK sizes: 2.5 (boot splash, empty panes), 3 (history,
/// settings), 2 (workbench).
pub fn gradient_spinner(
    cell: f32,
    tints: [Hsla; MATRIX_SIDE],
    phase: f32,
    pulse: impl Fn(f32) -> f32,
) -> Div {
    let pulse = &pulse;
    div()
        .flex()
        .flex_col()
        .gap(px(cell / 2.0))
        .children((0..MATRIX_SIDE).map(move |row| {
            let tint = tints[row];
            div()
                .flex()
                .flex_row()
                .gap(px(cell / 2.0))
                .children((0..MATRIX_SIDE).map(move |col| {
                    dot(cell, tint, pulse(phase + gradient_cell_phase(row, col)))
                }))
        }))
}

// ---------------------------------------------------------------------------
// Mini spinner (2×3)
// ---------------------------------------------------------------------------

/// Clockwise ring position of each `(row, col)` mini-spinner cell, top-left
/// first: (0,0) → (0,1) → (1,1) → (2,1) → (2,0) → (1,0).
pub const MINI_RING: [[usize; 2]; 3] = [[0, 1], [5, 2], [4, 3]];

/// A mini-spinner cell's phase offset: its ring position over 6.
pub fn mini_cell_phase(row: usize, col: usize) -> f32 {
    MINI_RING[row][col] as f32 / 6.0
}

/// The mini spinner's `(width, height)` in px for `cell`: 2.5 × 4 cells
/// (5×8 at cell 2), the box a host reserves for it.
pub fn mini_spinner_box(cell: f32) -> (f32, f32) {
    (cell * 2.5, cell * 4.0)
}

/// The 2×3 activity glyph for compact status slots (PINK
/// `mini_glyph_spinner`, and `mini_mono_spinner` with one tint for all
/// three rows): three rows of two round `cell` px cells, `cell / 2` apart,
/// rows tinted from `tints`. Each cell's opacity is `pulse(phase + offset)`
/// with its [`mini_cell_phase`] offset, so brightness snakes clockwise round
/// the grid's perimeter. PINK sizes: cell 2 (running tool rows, sidebar
/// rows, tabs), 1.75 and 1.5 (history), 3 (attachment sending).
pub fn mini_spinner(cell: f32, tints: [Hsla; 3], phase: f32, pulse: impl Fn(f32) -> f32) -> Div {
    let pulse = &pulse;
    div()
        .flex()
        .flex_col()
        .gap(px(cell / 2.0))
        .children((0..3).map(move |row| {
            let tint = tints[row];
            div()
                .flex()
                .flex_row()
                .gap(px(cell / 2.0))
                .children(
                    (0..2).map(move |col| dot(cell, tint, pulse(phase + mini_cell_phase(row, col)))),
                )
        }))
}

/// One round spinner cell.
fn dot(cell: f32, tint: Hsla, opacity: f32) -> Div {
    div()
        .size(px(cell))
        .rounded(px(cell / 2.0))
        .bg(tint)
        .opacity(opacity)
}

// ---------------------------------------------------------------------------
// Upload progress ring
// ---------------------------------------------------------------------------

/// The ring's stroke, in px.
pub const RING_STROKE: f32 = 2.5;
/// Polyline segments for a full circle: plenty for a ring up to 40 pt.
pub const RING_SEGMENTS: f32 = 64.0;
/// The percent label's size as a share of the diameter (9 pt in the 34 pt
/// ring), so label and ring scale together with the Interface size.
pub const RING_LABEL_RATIO: f32 = 9.0 / 34.0;

/// The track's tone: white at 22%.
pub fn ring_track() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.22)
}

/// The arc's and the label's tone: white at 95%.
pub fn ring_arc() -> Hsla {
    hsla(0.0, 0.0, 1.0, 0.95)
}

/// The radial upload ring (PINK `upload_progress_ring`) over a sending
/// attachment's thumbnail: a `diameter` pt box (rems) with a faint full
/// track and a bright arc growing clockwise from twelve o'clock for
/// `percent` (clamped to 100), both `RING_STROKE` polylines inset by the
/// stroke, and the percent centred, SEMIBOLD at `diameter * RING_LABEL_RATIO`.
/// Fixed white on a wash: the host dims the image behind it, which reads in
/// both themes. PINK uses a 34 pt ring.
pub fn upload_progress_ring(percent: u8, diameter: f32) -> Div {
    let frac = f32::from(percent.min(100)) / 100.0;
    let ring = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let center = bounds.center();
            // The ring is rem-sized; paint to the laid-out box.
            let radius = f32::from(bounds.size.width.min(bounds.size.height)) / 2.0 - RING_STROKE;
            let mut paint_arc = |sweep: f32, color: Hsla| {
                if sweep <= 0.0 {
                    return;
                }
                let steps = ((RING_SEGMENTS * sweep).ceil() as usize).max(2);
                let at = |i: usize| {
                    // Clockwise from 12 o'clock.
                    let theta = -std::f32::consts::FRAC_PI_2
                        + std::f32::consts::TAU * sweep * (i as f32 / steps as f32);
                    point(
                        center.x + px(radius * theta.cos()),
                        center.y + px(radius * theta.sin()),
                    )
                };
                let mut builder = PathBuilder::stroke(px(RING_STROKE));
                builder.move_to(at(0));
                for i in 1..=steps {
                    builder.line_to(at(i));
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            };
            paint_arc(1.0, ring_track());
            paint_arc(frac, ring_arc());
        },
    )
    .absolute()
    .inset_0();
    div()
        .relative()
        .size(ui_rems(diameter))
        .flex()
        .items_center()
        .justify_center()
        .child(ring)
        .child(
            div()
                .text_size(ui_rems(diameter * RING_LABEL_RATIO))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ring_arc())
                .child(SharedString::from(format!("{percent}%"))),
        )
}

// ---------------------------------------------------------------------------
// Boot splash
// ---------------------------------------------------------------------------

/// The splash's gradient-matrix cell, the size every other surface runs
/// that spinner at (the Sending strip, the working trailer).
pub const SPLASH_CELL: f32 = 2.5;
/// The gap between the splash's spinner and its label, in px.
pub const SPLASH_GAP: f32 = 12.0;
/// The splash label's type size.
pub const SPLASH_TEXT: f32 = 12.0;

/// The boot splash's content (PINK `splash_overlay`): an absolute `inset_0`
/// layer of `bg` (PINK: the frost tint, `theme.glass()`) with a
/// [`gradient_spinner`] at [`SPLASH_CELL`] (its `tints`, `phase` and
/// `pulse`) above `label` at [`SPLASH_TEXT`] in `label_color`, `SPLASH_GAP`
/// apart and centred. The host plays its fade out (PINK `splash_out`) and
/// removes the layer.
pub fn splash_overlay(
    bg: Hsla,
    tints: [Hsla; MATRIX_SIDE],
    phase: f32,
    pulse: impl Fn(f32) -> f32,
    label: impl Into<SharedString>,
    label_color: Hsla,
) -> Div {
    div()
        .absolute()
        .inset_0()
        .bg(bg)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(SPLASH_GAP))
        .child(gradient_spinner(SPLASH_CELL, tints, phase, pulse))
        .child(
            div()
                .text_size(ui_rems(SPLASH_TEXT))
                .text_color(label_color)
                .child(label.into()),
        )
}
