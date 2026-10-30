// The time side of the pink-ui Trace Waterfall: a run's calls laid out
// against one shared axis. The axis spans the run (with a little room after
// its end), carries at most six readable ticks with a hairline under each,
// and every call draws as Pink's 6 px usage-meter capsule from its start to
// its end with its time (and, when the host knows it, its cost) in Pink's
// 11/14 timer type just after the bar.
//
// What a call has not recorded is never drawn as if it had: a call with no
// end draws only a capsule-sized mark at its start, and a call with no
// start draws no bar at all, so the host's label says what is missing.
//
// The package draws only. The host supplies the resolved colours (the bar
// tone for each call's state, the label and tick text, the grid hairline),
// the text of every label and the step column beside the bars, and adds the
// ids, roles, accessible names, focus and listeners. Every size is in rems
// (pt at the 16 px default), so the waterfall follows the host's Interface
// size; positions on the axis are fractions of the plot, so it fits any
// column width. Plain `//` comments: the pink-ui gallery compiles this file
// in place with `include!` for its story.

use gpui::{
    AbsoluteLength, AnyElement, Div, Hsla, SharedString, div, prelude::*, px, relative, rems,
};

/// The axis header row (Pink's usage table header).
pub const AXIS_H: f32 = 30.0;
/// One call's row in a host's step list.
pub const ROW_H: f32 = 28.0;
/// A call's bar: Pink's 6 pt usage-meter capsule.
pub const BAR_H: f32 = 6.0;
/// Before the axis's zero.
pub const PAD_L: f32 = 12.0;
/// After the axis's end: room for the last bar's label.
pub const PAD_R: f32 = 88.0;
/// Between a bar's end and its label.
pub const LABEL_GAP: f32 = 6.0;
/// Pink's meta and timer type.
pub const META_SIZE: f32 = 11.0;
pub const META_LINE: f32 = 14.0;
/// Most ticks an axis draws.
pub const MAX_TICKS: f32 = 6.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// A tick step that gives at most [`MAX_TICKS`] ticks over `scale` seconds.
pub fn tick_step(scale: f32) -> f32 {
    [
        1., 2., 5., 10., 15., 20., 30., 60., 120., 300., 600., 1200., 1800.,
    ]
    .into_iter()
    .find(|step| scale / step <= MAX_TICKS)
    .unwrap_or(3600.)
}

/// A call's duration: tenths under 10 s, whole seconds to a minute, then
/// `1m 05s`, then `1h 02m` (Pink `native_agents::duration_label`).
pub fn fmt_secs(s: f32) -> String {
    let s = if s.is_finite() { s.max(0.) } else { 0. };
    if s < 10. {
        format!("{s:.1}s")
    } else if s < 60. {
        format!("{s:.0}s")
    } else if s < 3600. {
        let whole = s as u32;
        format!("{}m {:02}s", whole / 60, whole % 60)
    } else {
        let mins = s as u32 / 60;
        format!("{}h {:02}m", mins / 60, mins % 60)
    }
}

/// A tick's time: `0s`, `30s`, `2m`, `2m 30s`.
pub fn fmt_tick(s: f32) -> String {
    let whole = s.max(0.).round() as u32;
    match (whole / 60, whole % 60) {
        (0, secs) => format!("{secs}s"),
        (mins, 0) => format!("{mins}m"),
        (mins, secs) => format!("{mins}m {secs}s"),
    }
}

/// A reported cost (Pink `usage_live::live_money`): mills under a dollar,
/// cents from there. An explicit zero reads `$0.000`.
pub fn fmt_cost(dollars: f64) -> String {
    let dollars = if dollars.is_finite() {
        dollars.max(0.)
    } else {
        0.
    };
    if dollars < 1. {
        format!("${dollars:.3}")
    } else {
        let cents = (dollars * 100.).round() as u64;
        format!("${}.{:02}", cents / 100, cents % 100)
    }
}

/// The time axis: seconds from the run's start to a fraction of the plot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TraceAxis {
    /// The seconds the plot's full width stands for.
    pub scale: f32,
}

impl TraceAxis {
    pub fn new(scale: f32) -> Self {
        let scale = if scale.is_finite() && scale > 0. {
            scale
        } else {
            1.
        };
        Self { scale }
    }

    /// The axis of a finished run of `total` seconds: the run with 2% of
    /// room after it, never under a second.
    pub fn finished(total: f32) -> Self {
        Self::new(total.max(1.) * 1.02)
    }

    /// Where `secs` falls on the plot, from 0 (its start) to 1 (its end);
    /// a little past the end still draws, in the room the labels keep.
    pub fn fraction(&self, secs: f32) -> f32 {
        (secs / self.scale).clamp(0., 1.2)
    }

    /// The ticks, from zero, [`tick_step`] apart, up to the scale.
    pub fn ticks(&self) -> Vec<f32> {
        let step = tick_step(self.scale);
        let count = (self.scale / step + 0.001).floor() as usize;
        (0..=count).map(|k| k as f32 * step).collect()
    }
}

/// The plot inside a timeline cell: the cell less [`PAD_L`] and [`PAD_R`].
fn plot() -> Div {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .left(ui_rems(PAD_L))
        .right(ui_rems(PAD_R))
}

/// The axis header's timeline cell: each tick's time just after its line,
/// in `text`. It fills its row's free width; the host draws the step
/// column's header beside it and the row's [`AXIS_H`] height and border.
pub fn axis_ticks(axis: &TraceAxis, text: Hsla) -> Div {
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .h_full()
        .child(plot().children(axis.ticks().into_iter().map(move |s| {
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(axis.fraction(s)))
                .pl(ui_rems(4.))
                .flex()
                .items_center()
                .whitespace_nowrap()
                .text_size(ui_rems(META_SIZE))
                .line_height(ui_rems(META_LINE))
                .text_color(text)
                .child(fmt_tick(s))
        })))
}

/// The grid behind the bars: a 1 px hairline in `color` at every tick but
/// zero. Absolute: put it in the timeline column's box, which it fills.
pub fn grid(axis: &TraceAxis, color: Hsla) -> Div {
    div()
        .absolute()
        .inset_0()
        .child(
            plot().children(axis.ticks().into_iter().skip(1).map(move |s| {
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(axis.fraction(s)))
                    .w(px(1.))
                    .bg(color)
            })),
        )
}

/// What one bar draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarStyle {
    /// The capsule.
    pub fill: Hsla,
    /// The label after it.
    pub label: Hsla,
}

/// One call's timeline cell: its bar from `start` to `end` seconds on the
/// axis, then `label` [`LABEL_GAP`] after it. A bar is never shorter than
/// [`BAR_H`], so a call with no `end` draws only that mark at its start;
/// with no `start` there is no bar and the label sits at the axis's zero.
/// The cell fills its row's free width and height.
pub fn bar_cell(
    axis: &TraceAxis,
    start: Option<f32>,
    end: Option<f32>,
    label: Option<AnyElement>,
    style: &BarStyle,
) -> Div {
    // The bar and its label run from the call's start to the axis's far
    // edge (`fraction` stops at 1.2), so the bar's share of that run is its
    // share of the axis.
    let from = start.map_or(0., |s| axis.fraction(s));
    let reach = (1.2 - from).max(f32::EPSILON);
    let bar = start.map(|_| {
        let to = end.map_or(from, |e| axis.fraction(e).max(from));
        div()
            .flex_none()
            .w(relative((to - from) / reach))
            .min_w(ui_rems(BAR_H))
            .h(ui_rems(BAR_H))
            .rounded_full()
            .bg(style.fill)
    });
    let has_bar = bar.is_some();
    let label = label.map(|label| {
        div()
            .flex_none()
            .when(has_bar, |label| label.ml(ui_rems(LABEL_GAP)))
            .whitespace_nowrap()
            .text_size(ui_rems(META_SIZE))
            .line_height(ui_rems(META_LINE))
            .text_color(style.label)
            .child(label)
    });
    div().relative().flex_1().min_w_0().h_full().child(
        plot().child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(from))
                .w(relative(reach))
                .flex()
                .items_center()
                .children(bar)
                .children(label),
        ),
    )
}

/// A call's label text: its time, then its cost when the host knows it
/// (`12.3s, $0.031`). `None` cost stays out: unknown is the host's to say.
pub fn bar_label(secs: f32, cost: Option<f64>) -> SharedString {
    match cost {
        Some(cost) => format!("{}, {}", fmt_secs(secs), fmt_cost(cost)).into(),
        None => fmt_secs(secs).into(),
    }
}
