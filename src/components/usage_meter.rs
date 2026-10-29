//! Usage Meter – a plan's credits against its allowance, laid out like
//! Pink's Usage page.
//!
//! One settings section card. The header names the workspace with its plan
//! badge (Pink: usage.rs `compact_limit_row`), a cycle switch (Pink's
//! segmented control) and the plan action. The quota row is Pink's limit row
//! (usage.rs `pooled_limit_sections`): the credits used with the reset and
//! even-pace reading, "NN% left" at Title2 and Pink's shared 6 px meter
//! (`usage_meter`), which turns warning at 75% and danger at 90%. When the
//! cycle is headed over its allowance, a warning strip speaks up under it
//! (settings/widgets.rs `warning_strip`) and the header action becomes
//! "Raise limit". Then one row per category with its series dot and amount
//! (the Usage page's provider rows), the daily chart that layers each
//! category's spend (`usage_chart`), and the plan's other limits as compact
//! meter lines (`compact_limit_row`).
//!
//! ```ignore
//! UsageMeter::new("usage", "Northwind Labs", "Team", 10_000.)
//!     .category(UsageCategory::new("Agents"))
//!     .cycle(UsageCycle::new("This cycle", days).series(agents_daily))
//!     .limit(UsageLimit::new("Seats", 18., 20.))
//!     .on_raise_limit(|_, _| {})
//! ```

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyElement, AnyView, App, ElementId, Entity, FontWeight, Hsla, PathBuilder, SharedString,
    TextAlign, Window, canvas, div, point, prelude::*, px, rgb, svg,
};

use crate::{
    motion,
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, Segment, TextStyle, TypeStyleExt, layout,
    },
    story,
};

/// The `pink-usage-meter` package (`components/usage-meter`): PINK's usage
/// meter, the split bar and the plan badge, compiled here unchanged. The
/// Compact Breakdown's split bar draws through it too.
#[allow(dead_code)]
pub mod package {
    include!("../../components/usage-meter/src/lib.rs");
}

pub const NAME: &str = "Usage Meter";
pub const DESCRIPTION: &str = "A plan's usage card: the share left over a capsule meter that turns amber, then red, the reset and even-pace reading, a strip that speaks up when the cycle runs ahead of its allowance, spend by category over a daily chart and the plan's other limits as compact meters";

/// The chart's plot height, value-axis gutter and gutter gap (Pink: usage.rs
/// `CHART_HEIGHT`, `CHART_AXIS_WIDTH`, `CHART_AXIS_GAP`).
const CHART_HEIGHT: f32 = 200.;
const CHART_AXIS_WIDTH: f32 = 56.;
const CHART_AXIS_GAP: f32 = 8.;
/// The card's width: Pink's settings column (720) inside its 32 px gutters.
const WIDTH: f32 = layout::PAGE_MAX_WIDTH - 2. * 32.;
/// One cycle segment (Pink: two options in 150, four in 300).
const SEGMENT_W: f32 = 75.;
/// A compact limit line's columns (Pink: usage.rs `compact_limit_row`: a 50
/// label, the meter, a 30 percentage). The amount column is this
/// component's own, sized for "41.5 / 50 GB".
const LINE_LABEL_W: f32 = 50.;
const LINE_PERCENT_W: f32 = 30.;
const LINE_AMOUNT_W: f32 = 72.;

const ICON_WARNING: &str = "icons/usage-meter/danger-triangle.svg";
const ICON_CHART: &str = "icons/usage-meter/chart.svg";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Where credits go, e.g. "Agents".
#[derive(Clone)]
pub struct UsageCategory {
    pub name: SharedString,
    /// The category's series color; Pink's usage series tones by default.
    pub color: Option<Hsla>,
}

impl UsageCategory {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            color: None,
        }
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// One billing cycle.
#[derive(Clone)]
pub struct UsageCycle {
    pub label: SharedString,
    /// A label per day of the cycle, e.g. "Oct 14".
    pub days: Vec<SharedString>,
    /// Credits spent on each elapsed day, one series per category (in
    /// category order). Shorter than `days` while the cycle runs.
    pub series: Vec<Vec<f32>>,
}

impl UsageCycle {
    pub fn new(label: impl Into<SharedString>, days: Vec<SharedString>) -> Self {
        Self {
            label: label.into(),
            days,
            series: Vec::new(),
        }
    }

    /// The next category's credits per elapsed day.
    pub fn series(mut self, daily: Vec<f32>) -> Self {
        self.series.push(daily);
        self
    }

    fn elapsed(&self) -> usize {
        self.series
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .min(self.days.len())
    }

    fn closed(&self) -> bool {
        !self.days.is_empty() && self.elapsed() >= self.days.len()
    }

    /// Every category's credits on `day`.
    fn day(&self, day: usize) -> Vec<f32> {
        self.series
            .iter()
            .map(|s| s.get(day).copied().unwrap_or(0.))
            .collect()
    }

    fn category_total(&self, category: usize) -> f32 {
        self.series
            .get(category)
            .map_or(0., |s| s.iter().take(self.days.len()).sum())
    }
}

/// Another plan limit: seats, storage, keys.
#[derive(Clone)]
pub struct UsageLimit {
    pub label: SharedString,
    pub used: f32,
    pub limit: f32,
    pub unit: Option<SharedString>,
}

impl UsageLimit {
    pub fn new(label: impl Into<SharedString>, used: f32, limit: f32) -> Self {
        Self {
            label: label.into(),
            used,
            limit,
            unit: None,
        }
    }

    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = Some(unit.into());
        self
    }
}

#[derive(IntoElement)]
pub struct UsageMeter {
    id: ElementId,
    workspace: SharedString,
    plan: SharedString,
    allowance: f32,
    unit: SharedString,
    categories: Vec<UsageCategory>,
    cycles: Vec<UsageCycle>,
    limits: Vec<UsageLimit>,
    warn_at: f32,
    on_raise_limit: Option<Handler>,
    on_manage: Option<Handler>,
}

impl UsageMeter {
    pub fn new(
        id: impl Into<ElementId>,
        workspace: impl Into<SharedString>,
        plan: impl Into<SharedString>,
        allowance: f32,
    ) -> Self {
        Self {
            id: id.into(),
            workspace: workspace.into(),
            plan: plan.into(),
            allowance,
            unit: "credits".into(),
            categories: Vec::new(),
            cycles: Vec::new(),
            limits: Vec::new(),
            warn_at: 0.85,
            on_raise_limit: None,
            on_manage: None,
        }
    }

    /// What is metered (default "credits").
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn category(mut self, category: UsageCategory) -> Self {
        self.categories.push(category);
        self
    }

    /// Cycles to switch between; the first is shown first.
    pub fn cycle(mut self, cycle: UsageCycle) -> Self {
        self.cycles.push(cycle);
        self
    }

    pub fn limit(mut self, limit: UsageLimit) -> Self {
        self.limits.push(limit);
        self
    }

    /// Share of the allowance at which the card speaks up even if the
    /// projection stays under it (default 0.85).
    pub fn warn_at(mut self, share: f32) -> Self {
        self.warn_at = share;
        self
    }

    /// The header's action while the cycle is headed over its allowance.
    pub fn on_raise_limit(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_raise_limit = Some(Rc::new(handler));
        self
    }

    /// The header's action while the cycle is on track or closed.
    pub fn on_manage(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_manage = Some(Rc::new(handler));
        self
    }
}

struct State {
    cycle: usize,
    /// The chart's hovered day.
    day: Option<usize>,
}

/// The numbers a cycle's quota row, strip and action share.
struct Outlook {
    used: f32,
    /// Share of the cycle's days elapsed, 0–1.
    elapsed: f32,
    /// Credits by the cycle's last day at the last week's pace.
    projected: f32,
    /// The day the projection crosses the allowance.
    runs_out: Option<usize>,
}

fn outlook(cycle: &UsageCycle, allowance: f32) -> Outlook {
    let elapsed = cycle.elapsed();
    let n_days = cycle.days.len();
    let totals: Vec<f32> = (0..elapsed).map(|d| cycle.day(d).iter().sum()).collect();
    let used: f32 = totals.iter().sum();
    // Credits per day over the last week.
    let recent = elapsed.min(7);
    let pace = if recent > 0 {
        totals[elapsed - recent..].iter().sum::<f32>() / recent as f32
    } else {
        0.
    };
    let projected = used + pace * n_days.saturating_sub(elapsed) as f32;
    let runs_out = if used >= allowance {
        let mut acc = 0.;
        totals.iter().position(|d| {
            acc += d;
            acc >= allowance
        })
    } else if projected > allowance && pace > 0. {
        let days = ((allowance - used) / pace).ceil().max(1.) as usize;
        Some((elapsed + days - 1).min(n_days.saturating_sub(1)))
    } else {
        None
    };
    Outlook {
        used,
        elapsed: if n_days > 0 {
            elapsed as f32 / n_days as f32
        } else {
            0.
        },
        projected,
        runs_out,
    }
}

impl RenderOnce for UsageMeter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State {
                cycle: 0,
                day: None,
            },
        );
        if self.cycles.is_empty() {
            return empty_card(self.id.clone(), &self.workspace, &t).into_any_element();
        }
        let (cycle_ix, day) = {
            let s = state.read(cx);
            (s.cycle.min(self.cycles.len() - 1), s.day)
        };
        let cycle = &self.cycles[cycle_ix];
        let n_days = cycle.days.len();
        let elapsed = cycle.elapsed();
        let closed = cycle.closed();
        let o = outlook(cycle, self.allowance);
        let allowance = self.allowance.max(1.);
        let share = o.used / allowance;
        let over = !closed && o.projected > allowance;
        let speak_up = !closed && (over || share >= self.warn_at);
        let n_series = self.categories.len().max(cycle.series.len());
        let tones: Vec<Hsla> = (0..n_series)
            .map(|i| {
                self.categories
                    .get(i)
                    .and_then(|c| c.color)
                    .unwrap_or_else(|| series_tone(i, &t))
            })
            .collect();
        let names: Vec<SharedString> = (0..n_series)
            .map(|i| {
                self.categories
                    .get(i)
                    .map_or_else(|| format!("Category {}", i + 1).into(), |c| c.name.clone())
            })
            .collect();
        let remaining_days = n_days.saturating_sub(elapsed);
        let last_day = cycle.days.last().cloned().unwrap_or_default();

        // --- Header: workspace, plan badge, cycle switch, plan action ----
        let period = match (cycle.days.first(), cycle.days.last()) {
            (Some(a), Some(b)) => format!("{a} to {b}, "),
            _ => String::new(),
        };
        let progress = if closed {
            "Closed".to_string()
        } else {
            format!("Day {elapsed} of {n_days}")
        };
        let switch = (self.cycles.len() > 1).then(|| {
            let state = state.clone();
            pink::segmented(
                format!("{key}-cycle"),
                self.cycles
                    .iter()
                    .enumerate()
                    .map(|(ix, c)| Segment::new(ix.to_string(), c.label.clone()))
                    .collect(),
                cycle_ix,
                SEGMENT_W * self.cycles.len() as f32,
                ControlSize::Md,
                &t,
                move |ix: &usize, _, cx| {
                    let ix = *ix;
                    state.update(cx, |s, cx| {
                        if s.cycle != ix {
                            s.cycle = ix;
                            s.day = None;
                            cx.notify();
                        }
                    })
                },
            )
        });
        let action = if speak_up {
            let handler = self.on_raise_limit.clone();
            pink::button(
                SharedString::from(format!("{key}-raise")),
                ButtonKind::Primary,
                ControlSize::Md,
                &t,
            )
            .child("Raise limit")
            .when_some(handler, |b, f| {
                b.on_click(move |_, window, cx| f(window, cx))
            })
        } else {
            let handler = self.on_manage.clone();
            pink::button(
                SharedString::from(format!("{key}-manage")),
                ButtonKind::Secondary,
                ControlSize::Md,
                &t,
            )
            .child("Manage plan")
            .when_some(handler, |b, f| {
                b.on_click(move |_, window, cx| f(window, cx))
            })
        };
        // Pink: settings/widgets.rs `section_header`, with the plan badge on
        // the title's line.
        let header = div()
            .w_full()
            .px(px(layout::ROW_PAD_X))
            .pt(px(14.))
            .pb(px(12.))
            .flex()
            .flex_row()
            .flex_wrap()
            .items_start()
            .gap_x(px(12.))
            .gap_y(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(layout::ROW_TEXT_MIN))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .type_style(TextStyle::Title3)
                                    .text_color(t.text)
                                    .child(self.workspace.clone()),
                            )
                            .child(plan_badge(self.plan.clone(), &t)),
                    )
                    .child(pink::row_description(format!("{period}{progress}"), &t)),
            )
            .child(pink::row_controls().children(switch).child(action));

        // --- Quota row: the credits, "NN% left" and the meter -------------
        let used_percent = share * 100.;
        let left = (100. - used_percent.clamp(0., 100.)).round();
        let status: SharedString = if closed {
            format!(
                "Closed at {:.0}%, {} {} unused",
                used_percent,
                count((allowance - o.used).max(0.)),
                self.unit
            )
            .into()
        } else if o.used > allowance {
            // Pink's `quota_pace` has no reading past the limit.
            format!(
                "Resets in {remaining_days}d, {} {} over",
                count(o.used - allowance),
                self.unit
            )
            .into()
        } else {
            // Pink: usage.rs `quota_pace`, against the cycle's days.
            let elapsed_percent = o.elapsed * 100.;
            let delta = used_percent - elapsed_percent;
            format!(
                "Resets in {remaining_days}d. Even pace: {:.0}% of cycle elapsed, used {:.0} points {}",
                elapsed_percent,
                delta.abs(),
                if delta > 0. { "above" } else { "below" }
            )
            .into()
        };
        let quota = pink::card_row(false, &t)
            .child(pink::row_text(
                format!(
                    "{} of {} {}",
                    count(o.used),
                    count(self.allowance),
                    self.unit
                ),
                Some(status),
                &t,
            ))
            .child(pink::row_controls().child(remaining_figure(
                format!("{left:.0}%"),
                if closed { "unused" } else { "left" },
                &t,
            )))
            .child(
                pink::row_block().child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(usage_meter(used_percent, &t)),
                ),
            );

        // --- Speaking up: a strip under the quota row ---------------------
        let message: Option<(bool, SharedString)> = if !speak_up {
            None
        } else if o.used >= allowance {
            let on = o
                .runs_out
                .map(|i| format!(" on {}", cycle.days[i]))
                .unwrap_or_default();
            Some((
                true,
                format!("Allowance used up{on}. Overage billing applies until the cycle resets")
                    .into(),
            ))
        } else if let Some(i) = o.runs_out {
            Some((
                false,
                format!(
                    "On pace to run out on {}, projected {} {} by {last_day}",
                    cycle.days[i],
                    count(o.projected),
                    self.unit
                )
                .into(),
            ))
        } else {
            Some((
                false,
                format!(
                    "{:.0}% used with {remaining_days}d to go, projected {} {} by {last_day}",
                    used_percent,
                    count(o.projected),
                    self.unit
                )
                .into(),
            ))
        };
        // Pink: usage.rs `render_usage_page` `notice`, a strip inside the
        // card on the rows' text edge.
        let notice = message.map(|(danger, copy)| {
            div()
                .w_full()
                .px(px(layout::ROW_PAD_X))
                .pb(px(layout::ROW_PAD_Y))
                .child(strip(danger, copy, &t))
        });

        // --- One row per category (Pink: the Usage page's provider rows) ---
        let days_counted = elapsed.max(1) as f32;
        let category_rows: Vec<_> = (0..self.categories.len())
            .filter_map(|i| {
                let amount = cycle.category_total(i);
                (amount > 0.).then(|| {
                    let of_used = if o.used > 0. { amount / o.used } else { 0. };
                    pink::card_row(false, &t)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(layout::ROW_TEXT_MIN))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    // The dot is the category's series color
                                    // in the chart below.
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div()
                                                .flex_none()
                                                .size(px(8.))
                                                .rounded_full()
                                                .bg(tones[i]),
                                        )
                                        .child(pink::row_title(names[i].clone(), &t)),
                                )
                                .child(pink::row_description(
                                    format!(
                                        "{} of {}, {} a day",
                                        usage_percent(of_used),
                                        self.unit,
                                        count(amount / days_counted)
                                    ),
                                    &t,
                                )),
                        )
                        .child(
                            pink::row_controls().child(
                                div()
                                    .text_size(px(13.))
                                    .line_height(px(18.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text)
                                    .child(count(amount)),
                            ),
                        )
                })
            })
            .collect();

        // --- The daily chart ------------------------------------------------
        let day = day.filter(|d| *d < elapsed);
        let periods: Vec<Vec<f32>> = (0..elapsed).map(|d| cycle.day(d)).collect();
        let hover = day.map(|d| {
            let values = &periods[d];
            let total: f32 = values.iter().sum();
            let parts: Vec<String> = values
                .iter()
                .enumerate()
                .filter(|(_, v)| **v > 0.)
                .map(|(i, v)| format!("{} {}", names[i], count(*v)))
                .collect();
            let detail = if parts.is_empty() {
                format!("{}: No activity", cycle.days[d])
            } else {
                format!(
                    "{}: {}, Total {}",
                    cycle.days[d],
                    parts.join(", "),
                    count(total)
                )
            };
            (format!("{}: Total {}", cycle.days[d], count(total)), detail)
        });
        let (readout, detail) = match hover {
            Some((readout, detail)) => (Some(readout), detail),
            None => (
                None,
                "Each point covers one day. Hover for category values.".to_string(),
            ),
        };
        let chart_row = pink::card_row(false, &t)
            .child(pink::row_text(
                format!("Daily {}", self.unit),
                Some(detail.into()),
                &t,
            ))
            .when_some(readout, |row, readout| {
                row.child(
                    pink::row_controls().child(
                        div()
                            .text_size(px(11.))
                            .line_height(px(14.))
                            .text_color(t.readable_muted(0.85))
                            .child(readout),
                    ),
                )
            })
            .child(pink::row_block().child(daily_chart(
                &key,
                &cycle.days,
                periods,
                &names,
                tones,
                day,
                &state,
                &t,
            )));

        // --- The plan's other limits as compact meter lines ---------------
        let limits_row = (!self.limits.is_empty()).then(|| {
            pink::card_row(false, &t)
                .child(pink::row_text(
                    "Plan limits",
                    Some(format!("Included with the {} plan", self.plan).into()),
                    &t,
                ))
                .child(
                    pink::row_block().child(
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .children(self.limits.iter().map(|l| limit_line(l, &t))),
                    ),
                )
        });

        motion::keep_alive(window);
        pink::section_card(&t)
            .mt_0()
            .id(self.id.clone())
            .w(px(WIDTH))
            .font_family(pink::SANS)
            .child(header)
            .child(quota)
            .children(notice)
            .children(category_rows)
            .child(chart_row)
            .children(limits_row)
            .into_any_element()
    }
}

/// Pink: usage.rs `usage_meter` / `usage_meter_with`, drawn by the
/// `pink-usage-meter` package: `text_muted` until 75%, warning from 75%,
/// danger from 90%, on a `fill(Secondary)` track; a 6 px capsule with
/// nothing drawn below 0.5%.
fn usage_meter(used_percent: f32, t: &Pink) -> gpui::Div {
    package::meter(used_percent, &meter_style(t))
}

/// The kit's meter tones (Pink: usage.rs `usage_meter`): `text_muted`, the
/// readable warning and danger tones, the `fill(Secondary)` track.
pub fn meter_style(t: &Pink) -> package::MeterStyle {
    package::MeterStyle {
        neutral: t.text_muted,
        warning: t.readable_tone(t.warning, 1.0),
        danger: t.readable_tone(t.danger, 1.0),
        track: t.fill(FillTier::Secondary),
    }
}

/// A category's series color: Pink's usage series tones in order (usage.rs
/// `provider_tone`: `text`, #5b9bbd, #d97757), then Pink's project hues.
fn series_tone(index: usize, t: &Pink) -> Hsla {
    match index {
        0 => t.text,
        1 => rgb(0x5b9bbd).into(),
        2 => rgb(0xd97757).into(),
        _ => pink::project_tint(index - 3),
    }
}

/// The plan badge (Pink: usage.rs `compact_limit_row`), drawn by the
/// package: `px 5 / py 1`, radius 5, `surface_raised`, 11/12 in
/// `text_muted`.
fn plan_badge(plan: SharedString, t: &Pink) -> gpui::Div {
    package::plan_badge(
        &package::BadgeStyle {
            background: t.surface_raised,
            text: t.text_muted,
        },
        plan,
    )
}

/// The remaining share (Pink: usage.rs `pooled_limit_sections`): the figure
/// at Title2 and its word at 12/16 in `readable_muted(0.85)`, on one
/// baseline, gap 4.
fn remaining_figure(figure: String, word: &'static str, t: &Pink) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .gap(px(4.))
        .child(
            div()
                .type_style(TextStyle::Title2)
                .text_color(t.text)
                .child(figure),
        )
        .child(
            div()
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(t.readable_muted(0.85))
                .child(word),
        )
}

/// Pink: settings/widgets.rs `warning_strip` (`danger` false: amber, py 10,
/// 12 text, 14 glyph) and `error_strip` (red, py 12, 12.5 text, 16 glyph):
/// radius 12, `tone @ 0.2` edge on `tone @ 0.06`, the copy and the danger
/// triangle in `readable_tone(muted, 0.9)`, gap 8.
fn strip(danger: bool, message: SharedString, t: &Pink) -> gpui::Div {
    let (tone, muted) = if danger {
        (t.danger, t.danger_muted)
    } else {
        (t.warning, t.warning_muted)
    };
    let (plate, edge, copy) = pink::strip_colors(tone, muted, t);
    div()
        .px(px(16.))
        .py(px(if danger { 12. } else { 10. }))
        .rounded(px(pink::radius::STRIP))
        .border_1()
        .border_color(edge)
        .bg(plate)
        .text_size(px(if danger { 12.5 } else { 12. }))
        .line_height(px(if danger { 18. } else { 16. }))
        .text_color(copy)
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.))
        .child(
            div().flex_none().mt(px(2.)).child(
                svg()
                    .path(ICON_WARNING)
                    .size(px(if danger { 16. } else { 14. }))
                    .text_color(copy),
            ),
        )
        .child(div().min_w_0().child(message))
}

/// One limit as a compact meter line (Pink: usage.rs `compact_limit_row`'s
/// window lines): 11/14 in `text_muted`, gap 6 – the label, the meter, the
/// used share and the amount, in fixed columns so stacked meters line up.
fn limit_line(l: &UsageLimit, t: &Pink) -> gpui::Div {
    let used = if l.limit > 0. {
        (l.used / l.limit * 100.).clamp(0., 100.)
    } else {
        0.
    };
    let unit = l.unit.clone().map(|u| format!(" {u}")).unwrap_or_default();
    div()
        .mt(px(3.))
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(t.text_muted)
        .child(
            div()
                .w(px(LINE_LABEL_W))
                .flex_none()
                .truncate()
                .child(l.label.clone()),
        )
        .child(usage_meter(used, t))
        .child(
            div()
                .w(px(LINE_PERCENT_W))
                .flex_none()
                .whitespace_nowrap()
                .text_align(TextAlign::Right)
                .child(format!("{used:.0}%")),
        )
        .child(
            div()
                .w(px(LINE_AMOUNT_W))
                .flex_none()
                .whitespace_nowrap()
                .text_align(TextAlign::Right)
                .child(format!(
                    "{} of {}{unit}",
                    trim_num(l.used),
                    trim_num(l.limit)
                )),
        )
}

/// No cycle to show (Pink: usage.rs "No limits reported"): a section card
/// with one row, the chart tile and the copy.
fn empty_card(id: ElementId, workspace: &SharedString, t: &Pink) -> impl IntoElement {
    pink::section_card(t)
        .mt_0()
        .id(id)
        .w(px(WIDTH))
        .font_family(pink::SANS)
        .child(
            pink::card_row(true, t)
                .child(pink::row_tile(ICON_CHART, t))
                .child(pink::row_text(
                    "No usage reported",
                    Some(format!("{workspace} has no billing cycle to show yet.").into()),
                    t,
                )),
        )
}

/// The daily chart (Pink: usage.rs `usage_chart`): a 56 px value gutter of
/// three 11/14 labels, a 200 px plot with `border` gridlines at the top,
/// middle and bottom, one monotone curve per category (a 2 px stroke over a
/// `tone @ 0.12` area, the largest series painted first), a 1 px
/// `text_muted` guide at the hovered day, and the first, middle and last day
/// under it in capitals. Each day is a hover cell with its values in a
/// tooltip.
#[allow(clippy::too_many_arguments)]
fn daily_chart(
    key: &str,
    days: &[SharedString],
    periods: Vec<Vec<f32>>,
    names: &[SharedString],
    tones: Vec<Hsla>,
    hovered: Option<usize>,
    state: &Entity<State>,
    t: &Pink,
) -> AnyElement {
    let peak = periods.iter().flatten().copied().fold(0f32, f32::max);
    let scale = if peak <= 0. {
        1.
    } else {
        let rough = peak / 2.;
        let magnitude = 10f32.powf(rough.log10().floor());
        let unit = [1., 2., 5., 10.]
            .into_iter()
            .find(|step| step * magnitude >= rough)
            .unwrap_or(10.)
            * magnitude;
        (peak / unit).ceil() * unit
    };
    let labels: [String; 3] = if peak <= 0. {
        [String::new(), String::new(), "0".into()]
    } else {
        [scale, scale / 2., 0.].map(|v| if v == 0. { "0".into() } else { compact(v) })
    };
    let grid = t.border;
    let hover_color = t.text_muted;
    let drawing = periods.clone();
    let plot = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            if width < 2. || height < 2. {
                return;
            }
            let x0 = bounds.origin.x;
            let y0 = bounds.origin.y;
            for ratio in [0f32, 0.5, 1.] {
                let mut path = PathBuilder::stroke(px(1.));
                let y = y0 + px(ratio * height);
                path.move_to(point(x0, y));
                path.line_to(point(x0 + px(width), y));
                if let Ok(path) = path.build() {
                    window.paint_path(path, grid);
                }
            }
            if drawing.is_empty() {
                return;
            }
            let value = |values: &Vec<f32>, index: usize| values.get(index).copied().unwrap_or(0.);
            // Each series is layered from zero; the largest is painted first
            // so smaller series and their lines stay visible.
            let mut order: Vec<usize> = (0..tones.len()).collect();
            order.sort_by(|a, b| {
                let sum = |index| drawing.iter().map(|v| value(v, index)).sum::<f32>();
                sum(*b).total_cmp(&sum(*a))
            });
            let length = drawing.len();
            for fill in [true, false] {
                for &index in &order {
                    if drawing.iter().all(|v| value(v, index) <= 0.) {
                        continue;
                    }
                    let mut path = if fill {
                        PathBuilder::fill()
                    } else {
                        PathBuilder::stroke(px(2.))
                    };
                    let coordinates: Vec<(f32, f32)> = drawing
                        .iter()
                        .enumerate()
                        .map(|(position, values)| {
                            let x = if length == 1 {
                                width / 2.
                            } else {
                                width * position as f32 / (length - 1) as f32
                            };
                            let y = height
                                - (value(values, index) / scale).clamp(0., 1.) * (height - 8.);
                            (x, y)
                        })
                        .collect();
                    path.move_to(point(x0 + px(coordinates[0].0), y0 + px(coordinates[0].1)));
                    // A monotone cubic never draws a false peak between two
                    // measured days.
                    let tangents = monotone_tangents(&coordinates);
                    for (i, pair) in coordinates.windows(2).enumerate() {
                        let (xa, ya) = pair[0];
                        let (xb, yb) = pair[1];
                        let dx = (xb - xa) / 3.;
                        path.cubic_bezier_to(
                            point(x0 + px(xb), y0 + px(yb)),
                            point(x0 + px(xa + dx), y0 + px(ya + tangents[i] * dx)),
                            point(x0 + px(xb - dx), y0 + px(yb - tangents[i + 1] * dx)),
                        );
                    }
                    if fill {
                        let last = coordinates[coordinates.len() - 1].0;
                        path.line_to(point(x0 + px(last), y0 + px(height)));
                        path.line_to(point(x0 + px(coordinates[0].0), y0 + px(height)));
                        path.close();
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(
                            path,
                            if fill {
                                tones[index].opacity(0.12)
                            } else {
                                tones[index]
                            },
                        );
                    }
                }
            }
            if let Some(index) = hovered.filter(|i| *i < length) {
                let x = if length == 1 {
                    width / 2.
                } else {
                    width * index as f32 / (length - 1) as f32
                };
                let mut guide = PathBuilder::stroke(px(1.));
                guide.move_to(point(x0 + px(x), y0));
                guide.line_to(point(x0 + px(x), y0 + px(height)));
                if let Ok(guide) = guide.build() {
                    window.paint_path(guide, hover_color);
                }
            }
        },
    )
    .w_full()
    .h(px(CHART_HEIGHT));
    let x_labels: [String; 3] = if periods.is_empty() {
        [String::new(), String::new(), String::new()]
    } else {
        [0, periods.len() / 2, periods.len() - 1].map(|i| days[i].to_uppercase())
    };
    let cells: Vec<_> = periods
        .iter()
        .enumerate()
        .map(|(ix, values)| {
            let mut detail = days[ix].to_string();
            for (i, v) in values.iter().enumerate() {
                detail.push_str(&format!("\n{} {}", names[i], count(*v)));
            }
            detail.push_str(&format!("\nTotal {}", count(values.iter().sum())));
            let state = state.clone();
            div()
                .id(SharedString::from(format!("{key}-day-{ix}")))
                .flex_1()
                .h_full()
                .on_hover(move |hovered: &bool, _, cx| {
                    let hovered = *hovered;
                    state.update(cx, |s, cx| {
                        if hovered {
                            s.day = Some(ix);
                        } else if s.day == Some(ix) {
                            s.day = None;
                        }
                        cx.notify();
                    })
                })
                .tooltip(pink::text_tooltip(detail))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
        })
        .collect();
    // Axis labels are captions in the secondary tone.
    let label_tone = t.readable_muted(0.85);
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .w_full()
                .flex()
                .gap(px(CHART_AXIS_GAP))
                .child(
                    div()
                        .w(px(CHART_AXIS_WIDTH))
                        .h(px(CHART_HEIGHT))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .text_align(TextAlign::Right)
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(label_tone)
                        .children(labels),
                )
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h(px(CHART_HEIGHT))
                        .child(plot)
                        .child(div().absolute().inset_0().flex().children(cells)),
                ),
        )
        .child(
            div()
                .w_full()
                .pl(px(CHART_AXIS_WIDTH + CHART_AXIS_GAP))
                .flex()
                .justify_between()
                .text_size(px(11.))
                .line_height(px(14.))
                .text_color(label_tone)
                .children(x_labels),
        )
        .into_any_element()
}

/// Pink: usage.rs `monotone_usage_tangents` (Fritsch–Carlson).
fn monotone_tangents(points: &[(f32, f32)]) -> Vec<f32> {
    if points.len() < 2 {
        return vec![0.; points.len()];
    }
    let slopes: Vec<f32> = points
        .windows(2)
        .map(|pair| {
            let dx = pair[1].0 - pair[0].0;
            if dx == 0. {
                0.
            } else {
                (pair[1].1 - pair[0].1) / dx
            }
        })
        .collect();
    let mut tangents = vec![0.; points.len()];
    tangents[0] = slopes[0];
    tangents[points.len() - 1] = slopes[slopes.len() - 1];
    for i in 1..points.len() - 1 {
        let previous = slopes[i - 1];
        let next = slopes[i];
        tangents[i] = if previous * next <= 0. {
            0.
        } else {
            (previous + next) / 2.
        };
    }
    for (i, slope) in slopes.iter().copied().enumerate() {
        if slope == 0. {
            tangents[i] = 0.;
            tangents[i + 1] = 0.;
            continue;
        }
        let a = tangents[i] / slope;
        let b = tangents[i + 1] / slope;
        let magnitude = a * a + b * b;
        if magnitude > 9. {
            let scale = 3. / magnitude.sqrt();
            tangents[i] = scale * a * slope;
            tangents[i + 1] = scale * b * slope;
        }
    }
    tangents
}

/// 7180 → "7,180" (Pink: usage.rs `count`).
fn count(v: f32) -> String {
    let n = v.max(0.).round() as u64;
    let text = n.to_string();
    let mut grouped = String::new();
    for (index, ch) in text.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

/// 1_250 → "1.25K" (Pink: usage.rs `compact_tokens`).
fn compact(v: f32) -> String {
    let value = v.max(0.).round() as u64;
    for (threshold, suffix) in [(1_000_000_u64, "M"), (1_000, "K")] {
        if value >= threshold {
            let scaled = value as f64 / threshold as f64;
            let digits = if scaled >= 100. {
                0
            } else if scaled >= 10. {
                1
            } else {
                2
            };
            let formatted = format!("{scaled:.digits$}");
            let trimmed = if formatted.contains('.') {
                formatted.trim_end_matches('0').trim_end_matches('.')
            } else {
                &formatted
            };
            return format!("{trimmed}{suffix}");
        }
    }
    count(v)
}

/// A share as "45.1%", "<0.1%" for a sliver (Pink: usage.rs `usage_percent`).
fn usage_percent(share: f32) -> String {
    let percent = share * 100.;
    if percent > 0. && percent < 0.1 {
        "<0.1%".into()
    } else {
        format!("{percent:.1}%")
    }
}

/// One decimal unless it is zero.
fn trim_num(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round())
    } else {
        format!("{v:.1}")
    }
}

// ---------------------------------------------------------------------------
// Demo data
// ---------------------------------------------------------------------------

fn jitter(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297A_2D39);
    x ^= x >> 15;
    x as f32 / u32::MAX as f32
}

/// Daily credits for `days` days summing to `total`, ramping by `growth`,
/// with lighter weekends (`first_weekday`: 0 = Monday) and, for batch work,
/// a heavy run every `spike` weekday.
fn daily_spend(
    days: usize,
    total: f32,
    growth: f32,
    first_weekday: usize,
    seed: u32,
    spike: Option<usize>,
) -> Vec<f32> {
    let raw: Vec<f32> = (0..days)
        .map(|d| {
            let weekday = (first_weekday + d) % 7;
            let ramp = 1. + growth * d as f32 / days.max(1) as f32;
            let weekend = if weekday >= 5 { 0.55 } else { 1. };
            let run = match spike {
                Some(w) if w == weekday => 3.2,
                Some(_) => 0.6,
                None => 1.,
            };
            ramp * weekend * run * (0.8 + 0.4 * jitter(seed + d as u32))
        })
        .collect();
    let sum: f32 = raw.iter().sum();
    let mut days: Vec<f32> = raw.iter().map(|r| (r / sum * total).round()).collect();
    // Put the rounding remainder on the last day so the cycle adds up exactly.
    let drift = total - days.iter().sum::<f32>();
    if let Some(last) = days.last_mut() {
        *last += drift;
    }
    days
}

fn demo() -> UsageMeter {
    let october: Vec<SharedString> = (1..=31).map(|d| format!("Oct {d}").into()).collect();
    let september: Vec<SharedString> = (1..=30).map(|d| format!("Sep {d}").into()).collect();
    // Oct 1, 2026 is a Thursday; Sep 1 a Tuesday. Batch jobs run on Mondays.
    UsageMeter::new("usage-meter", "Northwind Labs", "Team", 10_000.)
        .category(UsageCategory::new("Agents"))
        .category(UsageCategory::new("Chat"))
        .category(UsageCategory::new("Batch jobs"))
        .cycle(
            UsageCycle::new("This cycle", october)
                .series(daily_spend(20, 3_980., 1.4, 3, 11, None))
                .series(daily_spend(20, 2_420., 0.3, 3, 23, None))
                .series(daily_spend(20, 1_240., 0.6, 3, 37, Some(0))),
        )
        .cycle(
            UsageCycle::new("Last cycle", september)
                .series(daily_spend(30, 3_050., 0.4, 1, 57, None))
                .series(daily_spend(30, 2_770., 0.1, 1, 71, None))
                .series(daily_spend(30, 1_600., 0.2, 1, 89, Some(0))),
        )
        .limit(UsageLimit::new("Seats", 18., 20.))
        .limit(UsageLimit::new("Storage", 41.5, 50.).unit("GB"))
        .limit(UsageLimit::new("API keys", 6., 10.))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| demo())
}
