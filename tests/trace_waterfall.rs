//! The `pink-trace-waterfall` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays a finished turn's calls out. Failure modes: a bar or
//! its label that stops following the call's start and end on the shared
//! axis, or stays at its 16 px size while the text grows; a call with no
//! recorded end drawn as if it ran to the end of the turn (an invented
//! duration) instead of a mark at its start; a call with no recorded start
//! given a bar at all; an axis with too many ticks to read, ticks that do
//! not start at zero, or hairlines that miss them; an explicit zero cost
//! that reads as unknown, or an unknown one shown as zero; a label that
//! runs past the cell's edge at the run's end instead of flipping to the
//! bar's left, or one flipped where it fits after the bar (or has no room
//! before it).

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::trace_waterfall::package::{
        BAR_H, BarStyle, LABEL_GAP, LabelSide, MAX_TICKS, PAD_L, PAD_R, ROW_H, TraceAxis,
        axis_ticks, bar_cell, bar_cell_on, bar_label, fmt_cost, fmt_secs, fmt_tick, label_side,
        tick_step,
    },
    pink::Pink,
};

const WIDTH: f32 = 480.0;
/// A finished 20 s turn: a closed call from 2 s to 12 s, one that never
/// closed from 5 s and one with no recorded start.
const TOTAL: f32 = 20.0;
/// A label wider than the room after the axis ("1m 00s, $0.031" and
/// "Slowest"), in pt at the 16 px default.
const FLIPPED_W: f32 = 120.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let axis = TraceAxis::finished(TOTAL);
        let style = BarStyle {
            fill: t.text_muted,
            label: t.content_faint,
        };
        let row = |name: &'static str, start, end| {
            div()
                .w(px(WIDTH))
                .h(px(ROW_H * self.ui / 16.0))
                .flex()
                .child(
                    bar_cell(
                        &axis,
                        start,
                        end,
                        Some(
                            div()
                                .debug_selector(move || format!("{name}-label"))
                                .child(name)
                                .into_any_element(),
                        ),
                        &style,
                    )
                    .debug_selector(move || format!("{name}-cell")),
                )
        };
        // The run's last call, ending at its end, with a label wider than
        // the room after the axis: drawn before its bar.
        let flipped = div()
            .w(px(WIDTH))
            .h(px(ROW_H * self.ui / 16.0))
            .flex()
            .child(
                bar_cell_on(
                    &axis,
                    Some(14.0),
                    Some(TOTAL),
                    Some(
                        div()
                            .debug_selector(|| "flipped-label".into())
                            .w(px(FLIPPED_W * self.ui / 16.0))
                            .into_any_element(),
                    ),
                    &style,
                    LabelSide::Before,
                )
                .debug_selector(|| "flipped-cell".into()),
            );
        div()
            .flex()
            .flex_col()
            .child(row("closed", Some(2.0), Some(12.0)))
            .child(flipped)
            .child(row("open", Some(5.0), None))
            .child(row("untimed", None, None))
            .child(
                div()
                    .w(px(WIDTH))
                    .h(px(30.0))
                    .flex()
                    .child(axis_ticks(&axis, t.text_muted).debug_selector(|| "axis".into())),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    cx.debug_bounds(Box::leak(name.to_owned().into_boxed_str()))
        .unwrap_or_else(|| panic!("{name} was not drawn"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.75
}

#[gpui::test]
fn bars_follow_the_axis_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let axis = TraceAxis::finished(TOTAL);
        let cell = bounds(cx, "closed-cell");
        assert!(
            near(f32::from(cell.size.width), WIDTH)
                && near(f32::from(cell.size.height), ROW_H * scale),
            "cell at {ui}: {:?}",
            cell.size
        );
        // The plot is the cell less its rem paddings.
        let plot_x = f32::from(cell.origin.x) + PAD_L * scale;
        let plot_w = WIDTH - (PAD_L + PAD_R) * scale;
        let at = |s: f32| plot_x + axis.fraction(s) * plot_w;

        // A closed call: its label sits LABEL_GAP after the bar's end.
        let closed = bounds(cx, "closed-label");
        assert!(
            near(f32::from(closed.origin.x), at(12.0) + LABEL_GAP * scale),
            "closed label at {ui}: {} vs {}",
            f32::from(closed.origin.x),
            at(12.0) + LABEL_GAP * scale
        );
        // An open call draws only a BAR_H mark at its start, never a bar
        // to the end of the turn.
        let open = bounds(cx, "open-label");
        assert!(
            near(
                f32::from(open.origin.x),
                at(5.0) + (BAR_H + LABEL_GAP) * scale
            ),
            "open label at {ui}: {}",
            f32::from(open.origin.x)
        );
        // No recorded start: no bar, the label at the axis's zero.
        let untimed = bounds(cx, "untimed-label");
        assert!(
            near(f32::from(untimed.origin.x), plot_x),
            "untimed label at {ui}: {}",
            f32::from(untimed.origin.x)
        );
        // A flipped label ends LABEL_GAP before its bar's start, inside the
        // cell.
        let flipped = bounds(cx, "flipped-label");
        assert!(
            near(f32::from(flipped.right()), at(14.0) - LABEL_GAP * scale)
                && near(f32::from(flipped.size.width), FLIPPED_W * scale),
            "flipped label at {ui}: {flipped:?} vs bar start {}",
            at(14.0)
        );
        // The label's line grows with the Interface size.
        assert!(
            near(f32::from(closed.size.height), 14.0 * scale),
            "label line at {ui}: {:?}",
            closed.size
        );
    }
}

#[test]
fn ticks_and_figures_stay_readable() {
    for total in [0.4_f32, 8.0, 25.0, 45.0, 400.0, 5_000.0] {
        let axis = TraceAxis::finished(total);
        let ticks = axis.ticks();
        assert_eq!(ticks[0], 0.0, "ticks start at zero for {total}");
        assert!(
            ticks.len() as f32 <= MAX_TICKS + 1.0,
            "{} ticks for {total}",
            ticks.len()
        );
        assert!(ticks.last().copied().unwrap() <= axis.scale + 0.001);
    }
    assert_eq!(tick_step(8.), 2.);
    assert_eq!(tick_step(25.), 5.);
    assert_eq!(tick_step(45.), 10.);
    assert_eq!(fmt_tick(150.), "2m 30s");
    assert_eq!(fmt_secs(3.24), "3.2s");
    assert_eq!(fmt_secs(83.), "1m 23s");
    assert_eq!(fmt_secs(3_725.), "1h 02m");
    assert_eq!(fmt_cost(0.0421), "$0.042");
    assert_eq!(fmt_cost(12.345), "$12.35");
    // A provider-reported zero is a price; no cost is no claim.
    assert_eq!(bar_label(3.0, Some(0.0)).as_ref(), "3.0s, $0.000");
    assert_eq!(bar_label(3.0, None).as_ref(), "3.0s");
    // Nonsense never reaches the screen as NaN.
    assert_eq!(fmt_secs(f32::NAN), "0.0s");
    assert_eq!(TraceAxis::new(f32::NAN).scale, 1.0);
}

/// The host's measured label goes after its bar while it fits before the
/// cell's edge, before the bar when it would run past and the room before
/// holds it, and after anyway when neither side has room.
#[test]
fn labels_flip_before_the_bar_only_when_they_would_run_past() {
    let axis = TraceAxis::finished(TOTAL);
    for scale in [0.75_f32, 1.0, 1.25] {
        let cell = WIDTH * scale;
        let short = 40.0 * scale;
        let wide = FLIPPED_W * scale;
        assert_eq!(
            label_side(&axis, 14.0, Some(TOTAL), short, cell, scale),
            LabelSide::After,
            "a time label fits after the run's last bar at {scale}"
        );
        assert_eq!(
            label_side(&axis, 14.0, Some(TOTAL), wide, cell, scale),
            LabelSide::Before,
            "a wide label at the run's end flips at {scale}"
        );
        assert_eq!(
            label_side(&axis, 2.0, Some(12.0), wide, cell, scale),
            LabelSide::After,
            "a wide label with room after a mid-run bar stays after it at {scale}"
        );
        assert_eq!(
            label_side(&axis, 0.0, Some(TOTAL), wide, cell, scale),
            LabelSide::After,
            "no room before a bar from zero at {scale}"
        );
    }
}
