//! Pink's loaders, ported 1:1 from the Pink app: the cell loaders and the
//! upload ring from `crates/ui/src/loaders.rs`, the text shimmer from
//! `transcript.rs` (`shimmer_overlay`), the pulsing skeletons from
//! `popover.rs`, and the building blocks of Pink's working trailer (the
//! "thinking" line under the last transcript row).
//!
//! All motion routes through [`crate::motion`]: the phase math is Pink's
//! (`pulse_opacity`, `gspin_opacity`, ...) and every loop runs on the shared
//! pulse clock ([`motion::pulse_phase`]), ~30fps (15Hz for the gradient
//! spinner), phase-locked across instances, parked when nothing is mounted.
//! Cells animate inside fixed-size slots – opacity and inner size are
//! paint-local and never move surrounding layout. Reduced motion rests every
//! loader at its static frame and schedules nothing.
//!
//! Colors come in as parameters, as Pink's do from its theme:
//!
//! | Loader | Pink passes |
//! |---|---|
//! | [`breathing_orb`] | `theme.accent` |
//! | [`gradient_spinner`], [`mini_glyph_spinner`] | the accent glyph palette `[light, mid, deep]` |
//! | [`mini_mono_spinner`] | `text_muted` (or `on_solid` on a solid button) |
//! | [`pink_loader`], [`pink_mark_loader`] | `theme.text` |
//! | [`shimmer_text`] | base `content_muted` (or `warning`), peak `text` |
//! | [`skeleton_rows`] / [`skeleton_bar`] | Quaternary / Tertiary fill |
//!
//! Loaders are builders: drop them in any element tree, no `Window` or `App`
//! needed. Sizes are px at Pink's default UI scale.

use gpui::{
    AnyElement, App, AppContext, Bounds, ContentMask, Context, Div, Entity, FontFeatures,
    FontWeight, Hsla, IntoElement, ParentElement, PathBuilder, Render, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, StyleRefinement, Styled, TextAlign, TextRun, Window,
    canvas, div, hsla, point, prelude::FluentBuilder, px, size,
};
use web_time::Instant;

use crate::components::working_trailer;
use crate::motion::{
    self, GRADIENT_SPIN, ORB_BREATH, PINK_PULSE, PULSE_STAGGER, STATUS_SHIMMER, speed_scale,
};

pub use crate::motion::{
    GSPIN_DIM, GSPIN_ROW_TINTS, MARK_CELLS, MATRIX_SIDE, PINK_CELLS, mark_cell_stagger,
};

/// Pink's control corner radius (Pink `Theme::CONTROL_RADIUS`), used by the
/// skeleton rows.
const CONTROL_RADIUS: f32 = 6.0;

// ---------------------------------------------------------------------------
// Pink mark loader
// ---------------------------------------------------------------------------

/// The animated Pink mark (Pink `pink_mark_loader`): the full logo pixel grid
/// with a light wave sweeping tail → head. Each cell rests dim (opacity 0.08,
/// scale 0.9) and flares to full as the crest passes, over [`PINK_PULSE`]
/// (2.4s). `height_px` sets the mark's height; the width follows the 820:940
/// canvas. Pink paints it in `theme.text`.
pub fn pink_mark_loader(height_px: f32, color: Hsla) -> PinkMarkLoader {
    PinkMarkLoader { height_px, color }
}

/// See [`pink_mark_loader`].
#[derive(IntoElement)]
pub struct PinkMarkLoader {
    height_px: f32,
    color: Hsla,
}

impl RenderOnce for PinkMarkLoader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let color = self.color;
        let scale = self.height_px / 940.0;
        let cell = 100.0 * scale;
        let delta = motion::pulse_phase(&PINK_PULSE, window, cx);
        div()
            .relative()
            .w(px(820.0 * scale))
            .h(px(self.height_px))
            .children(MARK_CELLS.iter().map(move |&(x, y)| {
                let stagger = mark_cell_stagger(x, y);
                // Fixed slot; the animated cell breathes inside it.
                div()
                    .absolute()
                    .left(px(x * scale))
                    .top(px(y * scale))
                    .size(px(cell))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child({
                        // A negative CSS delay starts the cell mid-cycle: the
                        // stagger ADDS phase.
                        let phase = (delta + stagger).rem_euclid(1.0);
                        div()
                            .rounded(px(16.0 * scale))
                            .bg(color)
                            .opacity(motion::pulse_opacity(phase))
                            .size(px(cell * motion::pulse_scale(phase)))
                    })
            }))
    }
}

// ---------------------------------------------------------------------------
// Pink wave loader
// ---------------------------------------------------------------------------

/// The Pink wave loader (Pink `pink_loader`): a row of 5 cells pulsing
/// opacity 0.08 → 1 and scale 0.9 → 1 over [`PINK_PULSE`] (2.4s), each 0.15s
/// behind the last. Cells are `cell_px` slots `cell_px / 2` apart, rounded
/// `cell_px / 4`. Pink paints it in `theme.text`.
pub fn pink_loader(cell_px: f32, color: Hsla) -> PinkLoader {
    PinkLoader { cell_px, color }
}

/// See [`pink_loader`].
#[derive(IntoElement)]
pub struct PinkLoader {
    cell_px: f32,
    color: Hsla,
}

impl RenderOnce for PinkLoader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (slot, color) = (self.cell_px, self.color);
        let delta = motion::pulse_phase(&PINK_PULSE, window, cx);
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(slot / 2.0))
            .children((0..PINK_CELLS).map(move |i| {
                // Fixed slot; the animated cell breathes inside it.
                div()
                    .size(px(slot))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child({
                        let phase = motion::staggered_phase(delta, i, PULSE_STAGGER);
                        div()
                            .rounded(px(slot / 4.0))
                            .bg(color)
                            .opacity(motion::pulse_opacity(phase))
                            .size(px(slot * motion::pulse_scale(phase)))
                    })
            }))
    }
}

// ---------------------------------------------------------------------------
// Breathing orb
// ---------------------------------------------------------------------------

/// The orb's box: it sits on a 12–13px text line (Pink `ORB_BOX`).
pub const ORB_BOX: f32 = working_trailer::ORB_BOX;
/// The solid core's size at full breath (Pink `ORB_CORE`).
pub const ORB_CORE: f32 = working_trailer::ORB_CORE;

/// The working orb (Pink `breathing_orb`): a solid `accent` core inside a
/// soft halo, breathing on one slow sine over [`ORB_BREATH`] (1600ms) – core
/// 5.1 → 6px (`6 * (0.85 + 0.15 * breath)`), halo `accent` at 16% → 34%.
/// A 12px box, `flex_none`. Reduced motion rests at full breath. Pink uses it
/// for the working line and the composer's "Sending" strip.
pub fn breathing_orb(accent: Hsla) -> BreathingOrb {
    BreathingOrb { accent }
}

/// See [`breathing_orb`].
#[derive(IntoElement)]
pub struct BreathingOrb {
    accent: Hsla,
}

/// The orb's breath now: 0 (rest) to 1 (full), resting at 1 under reduced
/// motion.
fn orb_breath(window: &mut Window, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        1.0
    } else {
        // `0.5 - 0.5 * cos(2πt)`: Pink's breath is the pulse wave.
        motion::pulse_wave(motion::pulse_phase(&ORB_BREATH, window, cx))
    }
}

impl RenderOnce for BreathingOrb {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Drawn by the `pink-working-trailer` package.
        working_trailer::orb(self.accent, orb_breath(window, cx))
    }
}

// ---------------------------------------------------------------------------
// Gradient matrix spinner (3×3)
// ---------------------------------------------------------------------------

/// The phase offset of a gradient-spinner cell (Pink `gradient_spinner`):
/// its distance from the wave origin at the bottom center, `(2 - row) +
/// |col - 1|`, over 4. Row 0 is the top.
pub fn gradient_cell_phase(row: usize, col: usize) -> f32 {
    let center = (MATRIX_SIDE as f32 - 1.0) / 2.0;
    let max = MATRIX_SIDE as f32 - 1.0 + center;
    let d = MATRIX_SIDE as f32 - 1.0 - row as f32 + (col as f32 - center).abs();
    if max == 0.0 { 0.0 } else { d / (max + 1.0) }
}

/// The gradient matrix spinner (Pink `gradient_spinner`, the
/// "WorkingIndicator"): a 3×3 grid of round `cell_px` cells, `cell_px / 2`
/// apart (so `4 * cell_px` square), each row tinted from `tints` (`[light,
/// mid, deep]`, top to bottom – Pink's accent glyph palette). Every cell
/// pulses once per [`GRADIENT_SPIN`] (750ms) on [`motion::gspin_opacity`];
/// the per-cell phase enters at the bottom edge and converges on the top
/// center, so the wave reads as travelling upward. Runs at 15Hz. Pink sizes:
/// 2.5 (boot splash, empty panes), 3 (history, settings), 2 (workbench).
pub fn gradient_spinner(cell_px: f32, tints: [Hsla; 3]) -> GradientSpinner {
    GradientSpinner { cell_px, tints }
}

/// See [`gradient_spinner`].
#[derive(IntoElement)]
pub struct GradientSpinner {
    cell_px: f32,
    tints: [Hsla; 3],
}

impl RenderOnce for GradientSpinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let (cell_px, tints) = (self.cell_px, self.tints);
        let delta = motion::pulse_phase_slow(&GRADIENT_SPIN, window, cx);
        div()
            .flex()
            .flex_col()
            .gap(px(cell_px / 2.0))
            .children((0..MATRIX_SIDE).map(move |row| {
                let tint = tints[row];
                div()
                    .flex()
                    .flex_row()
                    .gap(px(cell_px / 2.0))
                    .children((0..MATRIX_SIDE).map(move |col| {
                        div()
                            .size(px(cell_px))
                            .rounded(px(cell_px / 2.0))
                            .bg(tint)
                            .opacity(motion::gspin_opacity(
                                delta + gradient_cell_phase(row, col),
                                GSPIN_DIM,
                            ))
                    }))
            }))
    }
}

// ---------------------------------------------------------------------------
// Mini spinners (2×3)
// ---------------------------------------------------------------------------

/// Clockwise ring position of each `(row, col)` cell of the mini spinner,
/// top-left first: (0,0) → (0,1) → (1,1) → (2,1) → (2,0) → (1,0) (Pink
/// `mini_spinner_cells` `RING`). A cell's phase is its position over 6.
pub const MINI_RING: [[usize; 2]; 3] = [[0, 1], [5, 2], [4, 3]];

/// The 2×3 activity glyph for compact status slots (Pink
/// `mini_glyph_spinner`, Pink's default "running item" mark): three rows of
/// two round `cell_px` cells, `cell_px / 2` apart (`2.5 × 4` cells, 5×8px at
/// cell 2), rows tinted `[light, mid, deep]` from the accent glyph palette.
/// Brightness snakes around the ring over [`GRADIENT_SPIN`] (750ms) at 30Hz.
///
/// Each spinner renders as its own cached view keyed by `key`, so only its
/// six cells re-render per tick; keep `key` unique among siblings. Pink
/// sizes: cell 2 (running tool rows, sidebar rows, tabs), 1.75 and 1.5
/// (history), 3 (attachment sending).
pub fn mini_glyph_spinner(
    key: impl Into<SharedString>,
    cell_px: f32,
    tints: [Hsla; 3],
) -> MiniSpinner {
    MiniSpinner {
        key: key.into(),
        cell_px,
        row_tints: tints,
    }
}

/// The grayscale [`mini_glyph_spinner`] (Pink `mini_mono_spinner`) for
/// surfaces where an accent would pull focus: same grid, snake and timing,
/// one `tint` (Pink: `text_muted` for "Reconnecting…").
pub fn mini_mono_spinner(key: impl Into<SharedString>, cell_px: f32, tint: Hsla) -> MiniSpinner {
    mini_glyph_spinner(key, cell_px, [tint; 3])
}

/// See [`mini_glyph_spinner`].
#[derive(IntoElement)]
pub struct MiniSpinner {
    key: SharedString,
    cell_px: f32,
    row_tints: [Hsla; 3],
}

impl RenderOnce for MiniSpinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Keep pulse invalidation apart from the host's state so cached
        // sibling rows are reused while these six cells animate.
        let view = window.with_global_id(self.key.clone().into(), |id, window| {
            window.with_element_state(id, |previous: Option<Entity<MiniSpinnerView>>, _| {
                let view = previous.unwrap_or_else(|| {
                    cx.new(|_| MiniSpinnerView {
                        cell_px: self.cell_px,
                        row_tints: self.row_tints,
                    })
                });
                view.update(cx, |view, cx| {
                    if view.cell_px != self.cell_px || view.row_tints != self.row_tints {
                        view.cell_px = self.cell_px;
                        view.row_tints = self.row_tints;
                        cx.notify();
                    }
                });
                (view.clone(), view)
            })
        });
        view.cached(
            StyleRefinement::default()
                .flex_none()
                .w(px(self.cell_px * 2.5))
                .h(px(self.cell_px * 4.0)),
        )
    }
}

struct MiniSpinnerView {
    cell_px: f32,
    row_tints: [Hsla; 3],
}

impl Render for MiniSpinnerView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (cell_px, row_tints) = (self.cell_px, self.row_tints);
        let delta = motion::pulse_delta(&GRADIENT_SPIN, cx.entity_id(), cx);
        div()
            .flex()
            .flex_col()
            .gap(px(cell_px / 2.0))
            .children((0..3).map(move |row| {
                let tint = row_tints[row];
                div()
                    .flex()
                    .flex_row()
                    .gap(px(cell_px / 2.0))
                    .children((0..2).map(move |col| {
                        let phase = MINI_RING[row][col] as f32 / 6.0;
                        div()
                            .size(px(cell_px))
                            .rounded(px(cell_px / 2.0))
                            .bg(tint)
                            .opacity(motion::gspin_opacity(delta + phase, GSPIN_DIM))
                    }))
            }))
    }
}

// ---------------------------------------------------------------------------
// Upload progress ring
// ---------------------------------------------------------------------------

/// Stroke width of [`upload_progress_ring`] (Pink `RING_STROKE`).
const RING_STROKE: f32 = 2.5;
/// Polyline segments for a full circle (Pink `RING_SEGMENTS`).
const RING_SEGMENTS: f32 = 64.0;
/// The percent label's size as a share of the diameter: 9 in the 34 ring
/// (Pink `RING_LABEL_RATIO`).
const RING_LABEL_RATIO: f32 = 9.0 / 34.0;

/// Radial upload progress with the percent centered (Pink
/// `upload_progress_ring`), overlaid on an attachment thumbnail while it
/// uploads: a faint full track (white 22%) and a bright arc (white 95%)
/// growing clockwise from 12 o'clock, 2.5px strokes; the label is
/// `diameter * 9/34` SEMIBOLD white 95%. Fixed white-on-wash: the caller
/// dims the image behind it. Pink uses a 34px ring.
pub fn upload_progress_ring(percent: u8, diameter: f32) -> AnyElement {
    let frac = f32::from(percent.min(100)) / 100.0;
    let ring = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let center = bounds.center();
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
            paint_arc(1.0, hsla(0.0, 0.0, 1.0, 0.22));
            paint_arc(frac, hsla(0.0, 0.0, 1.0, 0.95));
        },
    )
    .absolute()
    .inset_0();
    div()
        .relative()
        .size(px(diameter))
        .flex()
        .items_center()
        .justify_center()
        .child(ring)
        .child(
            div()
                .text_size(px(diameter * RING_LABEL_RATIO))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(hsla(0.0, 0.0, 1.0, 0.95))
                .child(SharedString::from(format!("{percent}%"))),
        )
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Boot splash
// ---------------------------------------------------------------------------

/// Pink's full-window boot splash (Pink `splash_overlay`): an absolute
/// `inset_0` layer of `bg` (Pink: the glass tint; pass `t.chrome_surface()`,
/// the glass baked over the canvas) with a [`gradient_spinner`] at cell 2.5 above a 12px
/// `label` in `label_color` (Pink: "Setting up PINK environment" in
/// `readable_muted(0.7)`), 12px apart, centered. While `fading` it plays
/// [`motion::splash_out`]: 150ms hold, then a 500ms fade and 6px lift.
pub fn splash_overlay(
    bg: Hsla,
    tints: [Hsla; 3],
    label: impl Into<SharedString>,
    label_color: Hsla,
    fading: bool,
) -> AnyElement {
    let content = div()
        .absolute()
        .inset_0()
        .bg(bg)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.0))
        .child(gradient_spinner(2.5, tints))
        .child(
            div()
                .text_size(px(12.0))
                .text_color(label_color)
                .child(label.into()),
        );
    if fading {
        motion::splash_out("boot-splash-out", content).into_any_element()
    } else {
        content.into_any_element()
    }
}

// ---------------------------------------------------------------------------
// Skeletons (Pink `popover.rs`)
// ---------------------------------------------------------------------------

/// Per-row stagger of the skeleton pulse, as a fraction of [`PINK_PULSE`]
/// (Pink `skeleton_rows`).
pub const SKELETON_STAGGER: f32 = 0.08;

/// A skeleton's opacity for a phase: `0.35 + 0.4 * pulse_wave` (Pink).
pub fn skeleton_opacity(phase: f32) -> f32 {
    0.35 + 0.4 * motion::pulse_wave(phase)
}

/// Pulsing skeleton rows shown while a list loads (Pink `skeleton_rows`):
/// `count` 28px bars, rounded 6, 6px apart with 4px above and below, in
/// `wash` (Pink: the Quaternary fill), pulsing on [`PINK_PULSE`] with
/// [`SKELETON_STAGGER`] per row.
pub fn skeleton_rows(count: usize, wash: Hsla) -> Skeleton {
    Skeleton {
        kind: SkeletonKind::Rows(count),
        wash,
    }
}

/// [`skeleton_rows`] shaped like a loading menu (Pink
/// `skeleton_menu_rows`): 14px bars rounded 7 at 42/58/48/66% width, 8px
/// apart, inset 6px vertically and 4px horizontally.
pub fn skeleton_menu_rows(count: usize, wash: Hsla) -> Skeleton {
    Skeleton {
        kind: SkeletonKind::MenuRows(count),
        wash,
    }
}

/// One pulsing ghost label (Pink `skeleton_bar`): `width` × 11px, rounded
/// 5.5, in `wash` (Pink: the Tertiary fill).
pub fn skeleton_bar(width: f32, wash: Hsla) -> Skeleton {
    Skeleton {
        kind: SkeletonKind::Bar(width),
        wash,
    }
}

#[derive(Clone, Copy)]
enum SkeletonKind {
    Rows(usize),
    MenuRows(usize),
    Bar(f32),
}

/// See [`skeleton_rows`], [`skeleton_menu_rows`] and [`skeleton_bar`].
#[derive(IntoElement)]
pub struct Skeleton {
    kind: SkeletonKind,
    wash: Hsla,
}

impl RenderOnce for Skeleton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        const WIDTHS: [f32; 4] = [0.42, 0.58, 0.48, 0.66];
        let wash = self.wash;
        let delta = motion::pulse_phase(&PINK_PULSE, window, cx);
        let phase = move |i: usize| motion::staggered_phase(delta, i, SKELETON_STAGGER);
        match self.kind {
            SkeletonKind::Rows(count) => {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .py(px(4.0))
                    .children((0..count).map(move |i| {
                        div()
                            .h(px(28.0))
                            .rounded(px(CONTROL_RADIUS))
                            .bg(wash)
                            .opacity(skeleton_opacity(phase(i)))
                    }))
            }
            SkeletonKind::MenuRows(count) => div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .py(px(6.0))
                .px(px(4.0))
                .children((0..count).map(move |i| {
                    div()
                        .h(px(14.0))
                        .w(gpui::relative(WIDTHS[i % WIDTHS.len()]))
                        .rounded(px(7.0))
                        .bg(wash)
                        .opacity(skeleton_opacity(phase(i)))
                })),
            SkeletonKind::Bar(width) => div()
                .w(px(width))
                .h(px(11.0))
                .rounded(px(5.5))
                .bg(wash)
                .opacity(skeleton_opacity(phase(0))),
        }
    }
}

// ---------------------------------------------------------------------------
// Text shimmer (Pink `transcript.rs` `shimmer_overlay`)
// ---------------------------------------------------------------------------

/// Half-width of the shimmer's highlight band, as a share of the text width
/// (Pink `TOOL_GROUP_SHIMMER_HALF_WIDTH`).
pub const SHIMMER_HALF_WIDTH: f32 = 0.36;
/// Width of the clip strips the shimmer repaints the line through (Pink
/// `TOOL_GROUP_SHIMMER_STRIP_WIDTH`).
pub const SHIMMER_STRIP_WIDTH: f32 = 2.0;

/// How lit the text is at `x` (0..1 of its width) for a sweep `phase`
/// (Pink `tool_title_shimmer_amount`): a 300%-wide repeating gradient moving
/// from 200% to −100%. The primary center runs `-2.5 + 6 * phase`, copies sit
/// three widths apart, and each lights a ±0.36 shoulder. The band crosses the
/// label twice per period.
pub fn shimmer_amount(x: f32, phase: f32) -> f32 {
    let primary_center = -2.5 + phase.clamp(0.0, 1.0) * 6.0;
    (-2..=2)
        .map(|copy| primary_center + copy as f32 * 3.0)
        .map(|center| (1.0 - (x - center).abs() / SHIMMER_HALF_WIDTH).clamp(0.0, 1.0))
        .fold(0.0, f32::max)
}

/// The sweep phase of a shimmer that started at `start` (Pink
/// `tool_title_shimmer_phase`): elapsed over [`STATUS_SHIMMER`] (3400ms).
pub fn shimmer_phase_since(start: Instant, now: Instant) -> f32 {
    let period = STATUS_SHIMMER.total().mul_f32(speed_scale()).as_secs_f32();
    let elapsed = now.saturating_duration_since(start).as_secs_f32();
    (elapsed / period).fract()
}

/// The shimmer itself (Pink `shimmer_overlay`): an absolute `inset_0` canvas
/// to lay over a single-line text element of the same `size` / `line_height`
/// on a `relative` parent. It shapes `text` once to measure it, cuts the
/// width into 2px strips, and repaints the whole shaped line through each lit
/// strip in `mix(base, peak, shimmer_amount(x, phase))` – the native
/// equivalent of CSS `background-clip: text`, without splitting the line's
/// shaping. The font is the inherited one (Pink: Geist regular). Most callers
/// want [`shimmer_text`].
pub fn shimmer_overlay(
    text: SharedString,
    size_px: f32,
    line_height: f32,
    base: Hsla,
    peak: Hsla,
    phase: f32,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let font = window.text_style().font();
            let run = |color: Hsla| TextRun {
                len: text.len(),
                font: font.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let probe =
                window
                    .text_system()
                    .shape_line(text.clone(), px(size_px), &[run(peak)], None);
            let text_width = f32::from(probe.width()).min(f32::from(bounds.size.width));
            let strip_count = (text_width / SHIMMER_STRIP_WIDTH).ceil().max(0.0) as usize;
            let mut strips = Vec::with_capacity(strip_count);
            for ix in 0..strip_count {
                let left = ix as f32 * SHIMMER_STRIP_WIDTH;
                let right = ((ix + 1) as f32 * SHIMMER_STRIP_WIDTH).min(text_width);
                let x = (left + right) * 0.5 / text_width.max(1.0);
                let amount = shimmer_amount(x, phase);
                if amount <= 0.001 {
                    continue;
                }
                let line = window.text_system().shape_line(
                    text.clone(),
                    px(size_px),
                    &[run(motion::mix(base, peak, amount))],
                    None,
                );
                strips.push((left, right, line));
            }
            strips
        },
        move |bounds, strips, window, cx| {
            for (left, right, line) in strips {
                let mask = ContentMask {
                    bounds: Bounds {
                        origin: point(bounds.origin.x + px(left), bounds.origin.y),
                        size: size(px(right - left), bounds.size.height),
                    },
                };
                window.with_content_mask(Some(mask), |window| {
                    let _ = line.paint(
                        bounds.origin,
                        px(line_height),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                });
            }
        },
    )
    .absolute()
    .inset_0()
}

/// One line of text with Pink's live shimmer: a soft highlight sweeping from
/// `base` to `peak` and back, through [`shimmer_overlay`]. Pink's two uses:
/// the working-line verb (12/16, `content_muted` → `text`) and the live
/// tool-group title (12/18, same colors). By default the sweep runs on the
/// shared clock at [`STATUS_SHIMMER`] (3400ms); [`started_at`] times it from
/// a start instead (Pink's tool groups), [`phase`] drives it by hand. Not
/// [`live`], or under reduced motion, it is plain `base` text. The wrapper
/// is `relative`; style it like any div (`.flex_none()` for a status verb,
/// `.flex_1().min_w_0().overflow_hidden()` for a title). Set size and line
/// height through the constructor, which the overlay shapes with.
///
/// [`started_at`]: ShimmerText::started_at
/// [`phase`]: ShimmerText::phase
/// [`live`]: ShimmerText::live
pub fn shimmer_text(
    text: impl Into<SharedString>,
    size_px: f32,
    line_height: f32,
    base: Hsla,
    peak: Hsla,
) -> ShimmerText {
    let text: SharedString = text.into();
    // `shape_line` takes one line.
    let text = if text.contains('\n') {
        text.replace('\n', " ").into()
    } else {
        text
    };
    ShimmerText {
        root: div()
            .relative()
            .text_size(px(size_px))
            .line_height(px(line_height))
            .text_color(base),
        text,
        size_px,
        line_height,
        base,
        peak,
        phase: None,
        started: None,
        live: true,
    }
}

/// See [`shimmer_text`].
#[derive(IntoElement)]
pub struct ShimmerText {
    root: Div,
    text: SharedString,
    size_px: f32,
    line_height: f32,
    base: Hsla,
    peak: Hsla,
    phase: Option<f32>,
    started: Option<Instant>,
    live: bool,
}

impl ShimmerText {
    /// Drive the sweep by hand (0..1); the caller keeps frames coming.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Time the sweep from `start` (Pink's tool-group title, which starts
    /// when the group first appears), on the pulse clock.
    pub fn started_at(mut self, start: Instant) -> Self {
        self.started = Some(start);
        self
    }

    /// Whether the shimmer runs; settled text is plain (default `true`).
    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }
}

impl Styled for ShimmerText {
    fn style(&mut self) -> &mut StyleRefinement {
        self.root.style()
    }
}

impl RenderOnce for ShimmerText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let phase = if !self.live || cx.reduce_motion() {
            None
        } else if let Some(phase) = self.phase {
            Some(phase)
        } else if let Some(start) = self.started {
            motion::pulse_keep(window, cx);
            Some(shimmer_phase_since(start, Instant::now()))
        } else {
            Some(motion::pulse_phase(&STATUS_SHIMMER, window, cx))
        };
        let label = div().whitespace_nowrap().child(self.text.clone());
        self.root.child(label).when_some(phase, |el, phase| {
            el.child(shimmer_overlay(
                self.text,
                self.size_px,
                self.line_height,
                self.base,
                self.peak,
                phase,
            ))
        })
    }
}

// ---------------------------------------------------------------------------
// Working trailer building blocks (Pink `transcript.rs`)
// ---------------------------------------------------------------------------
//
// Pink's thinking indicator is one row under the last transcript row:
// `pt 16, gap 8, items_start`, holding [`orb_slot`], [`working_status`]
// (verb + object, swapping in on [`motion::STATUS_SWAP`]), [`working_timer`]
// ("12s") and a hover-only "Stop" (`ml 4, px 6, rounded 6`, 11/16
// `content_muted`, opacity 0 → 1 on group hover, `hover_wash`).

/// Working-line verb and object size (Pink `WORKING_STATUS_SIZE`).
pub const WORKING_STATUS_SIZE: f32 = working_trailer::STATUS_TEXT;
/// The working line's shared line box (Pink `WORKING_STATUS_LINE_HEIGHT`).
pub const WORKING_STATUS_LINE_HEIGHT: f32 = working_trailer::LINE_HEIGHT;
/// The elapsed timer, one step quieter (Pink `WORKING_TIMER_SIZE`).
pub const WORKING_TIMER_SIZE: f32 = working_trailer::TIMER_TEXT;
/// Tool-group title size (Pink `TOOL_LABEL_SIZE`).
pub const TOOL_LABEL_SIZE: f32 = 12.0;
/// Tool-group title line height (Pink `TOOL_LABEL_LINE_HEIGHT`).
pub const TOOL_LABEL_LINE_HEIGHT: f32 = 18.0;
/// A live status object is ellipsized past this many chars (Pink
/// `LIVE_STATUS_OBJECT_CHARS`).
pub const LIVE_STATUS_OBJECT_CHARS: usize = 44;

/// "1m 32s"-style elapsed time (Pink `format_elapsed`): `"12s"` under a
/// minute, `"1m 5s"` from 60s. Negative input reads as 0.
pub fn format_elapsed(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

/// Truncates `text` to `max` chars with a trailing `…` (Pink
/// `ellipsize_chars`).
pub fn ellipsize_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Tabular (fixed-width) figures for counters and timers, so ticking digits
/// never shift the text after them (Pink `typography::tabular_figures`).
/// Pass to `.font_features(..)`; gpui-pre forwards OpenType features to the
/// shaper.
pub fn tabular_figures() -> FontFeatures {
    FontFeatures(std::sync::Arc::new(vec![("tnum".into(), 1)]))
}

/// 64-bit FNV-1a (Pink `transcript.rs` `fnv1a`), for ids keyed on text.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x1_0000_01b3);
    }
    hash
}

/// An element id derived from a label, `"{prefix}-{fnv1a:x}"` (Pink's
/// `"working-status-in-{:x}"`): a new label gets a new id, so a
/// [`motion::status_swap`] keyed on it replays.
pub fn label_id(prefix: &str, label: &str) -> SharedString {
    format!("{prefix}-{:x}", fnv1a(label.as_bytes())).into()
}

/// The working line's orb slot (Pink `render_working_trailer`): a
/// `flex_none` 16px line box centering the [`breathing_orb`], so the orb
/// stays on the first line when the status wraps.
pub fn orb_slot(accent: Hsla) -> OrbSlot {
    OrbSlot { accent }
}

/// See [`orb_slot`].
#[derive(IntoElement)]
pub struct OrbSlot {
    accent: Hsla,
}

impl RenderOnce for OrbSlot {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Drawn by the `pink-working-trailer` package.
        working_trailer::orb_slot(self.accent, orb_breath(window, cx))
    }
}

/// The working line's timer (Pink `render_working_trailer`): `"12s"` at
/// 11/16 with tabular figures, `flex_none`, no wrap. Pink colors it
/// `content_faint`.
pub fn working_timer(elapsed_secs: i64, color: Hsla) -> Div {
    working_trailer::timer(format_elapsed(elapsed_secs), color).font_features(tabular_figures())
}

/// The working line's status (Pink `render_working_trailer` status line):
/// a 12/16 row, 4px gap, of the `verb` in `tone` (Pink: `content_muted`, or
/// `warning` for queued and notices) shimmering toward `text` while live,
/// then an optional object in `text` that truncates (Pink ellipsizes it at
/// [`LIVE_STATUS_OBJECT_CHARS`]). Each new label fades in and settles 2px on
/// [`motion::STATUS_SWAP`]. A [`sentence`](WorkingStatus::sentence) (queued,
/// provider notices) wraps instead and does not shimmer.
pub fn working_status(verb: impl Into<SharedString>, tone: Hsla, text: Hsla) -> WorkingStatus {
    WorkingStatus {
        verb: verb.into(),
        object: None,
        tone,
        text,
        sentence: false,
        live: true,
    }
}

/// See [`working_status`].
#[derive(IntoElement)]
pub struct WorkingStatus {
    verb: SharedString,
    object: Option<SharedString>,
    tone: Hsla,
    text: Hsla,
    sentence: bool,
    live: bool,
}

impl WorkingStatus {
    /// What the verb acts on (a file, command or host), in `text`.
    pub fn object(mut self, object: impl Into<SharedString>) -> Self {
        self.object = Some(object.into());
        self
    }

    /// A sentence rather than a status word: it wraps and does not shimmer.
    pub fn sentence(mut self, sentence: bool) -> Self {
        self.sentence = sentence;
        self
    }

    /// Whether the verb shimmers (default `true`).
    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }
}

impl RenderOnce for WorkingStatus {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let label: SharedString = match &self.object {
            Some(object) => format!("{} {object}", self.verb).into(),
            None => self.verb.clone(),
        };
        let verb = if self.sentence {
            working_trailer::verb(self.verb, self.tone, true).into_any_element()
        } else {
            shimmer_text(
                self.verb,
                WORKING_STATUS_SIZE,
                WORKING_STATUS_LINE_HEIGHT,
                self.tone,
                self.text,
            )
            .live(self.live)
            .flex_none()
            .into_any_element()
        };
        let text = self.text;
        let line = working_trailer::status_line(label_id("working-status", &label))
            .role(Role::Status)
            .aria_label(label.clone())
            .child(verb)
            .when_some(self.object, |el, object| {
                el.child(working_trailer::object(object, text))
            });
        motion::status_swap(label_id("working-status-in", &label), line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    #[test]
    fn elapsed_and_ellipsis_match_pink() {
        assert_eq!(format_elapsed(-3), "0s");
        assert_eq!(format_elapsed(12), "12s");
        assert_eq!(format_elapsed(65), "1m 5s");
        assert_eq!(ellipsize_chars("abc", 3), "abc");
        assert_eq!(ellipsize_chars("abcdef", 4), "abc…");
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_ne!(label_id("s", "Reading"), label_id("s", "Writing"));
    }

    #[test]
    fn gradient_phases_rise_from_the_bottom_center() {
        let map: Vec<Vec<f32>> = (0..3)
            .map(|row| (0..3).map(|col| gradient_cell_phase(row, col)).collect())
            .collect();
        assert_eq!(
            map,
            vec![
                vec![0.75, 0.5, 0.75],
                vec![0.5, 0.25, 0.5],
                vec![0.25, 0.0, 0.25]
            ]
        );
    }

    #[test]
    fn shimmer_band_sweeps_twice_per_period() {
        // Centered on the label at phase 0.5 (primary) and 0 (the next copy).
        assert!((shimmer_amount(0.5, 0.5) - 1.0).abs() < 1e-5);
        assert!((shimmer_amount(0.5, 0.0) - 1.0).abs() < 1e-5);
        assert_eq!(shimmer_amount(0.5, 0.25), 0.0, "dark between crossings");
        // The band touches the label only for phase 0.357..0.643 (and the
        // copy's 0.857..0.143).
        assert_eq!(shimmer_amount(0.0, 0.35), 0.0);
        assert!(shimmer_amount(0.0, 0.37) > 0.0);
        // The shoulder is 0.36 of the width either side of the peak.
        assert!((shimmer_amount(0.5 + 0.18, 0.5) - 0.5).abs() < 1e-4);
    }

    struct Host;

    impl Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let accent = hsla(0.69, 0.88, 0.73, 1.0);
            let tints = [accent, accent, accent];
            div()
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(breathing_orb(accent))
                .child(gradient_spinner(2.5, tints))
                .child(mini_glyph_spinner("mini", 2.0, tints))
                .child(mini_mono_spinner("mono", 2.0, accent))
                .child(pink_loader(4.0, accent))
                .child(pink_mark_loader(24.0, accent))
                .child(upload_progress_ring(42, 34.0))
                .child(skeleton_rows(3, accent))
                .child(skeleton_menu_rows(4, accent))
                .child(skeleton_bar(48.0, accent))
                .child(shimmer_text("Thinking", 12.0, 16.0, accent, gpui::white()))
                .child(
                    shimmer_text("Read 3 files", 12.0, 18.0, accent, gpui::white())
                        .started_at(Instant::now()),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.0))
                        .child(orb_slot(accent))
                        .child(working_status("Reading", accent, gpui::white()).object("main.rs"))
                        .child(working_timer(65, accent)),
                )
                .child(splash_overlay(
                    gpui::black(),
                    tints,
                    "Setting up",
                    accent,
                    true,
                ))
        }
    }

    #[gpui::test]
    fn loaders_render_animated_and_reduced(cx: &mut TestAppContext) {
        cx.update(crate::init);
        let (_, cx) = cx.add_window_view(|_, _| Host);
        for reduced in [false, true] {
            for _ in 0..2 {
                cx.update(|window, cx| {
                    motion::set_reduced_motion(cx, reduced);
                    window.refresh();
                });
                cx.run_until_parked();
            }
            if !reduced {
                // The host view (orb, gradient, wave, mark, skeletons,
                // shimmer) and each mini spinner's own view hold a lease.
                let leases = cx.update(|_, cx| motion::pulse_leases(cx));
                assert!(leases >= 3, "loaders drive the pulse clock: {leases}");
            }
        }
    }
}
