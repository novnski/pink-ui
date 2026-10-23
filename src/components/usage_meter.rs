//! Usage Meter – a billing cycle's credits, where they went and where they're headed.
//!
//! One stacked meter splits the credits used by category and extends a
//! hatched projection to the end of the cycle, with a tick at the plan's
//! allowance. Under it, a burn-up chart pairs the daily spend (bars) with the
//! running total (line) and the projected path to the limit; hovering a day
//! reads it out. Small rings show the plan's other limits. The footer stays
//! quiet while the cycle is on track and speaks up when the projection
//! crosses the allowance: a warning with the date it runs out and an accent
//! button with a soft pulse.
//!
//! ```ignore
//! UsageMeter::new("usage", "Northwind Labs", "Team", 10_000.)
//!     .category(UsageCategory::new("Agents"))
//!     .cycle(UsageCycle::new("This cycle", days).daily(daily).split(vec![3240., 2010.]))
//!     .on_raise_limit(|_, _| {})
//! ```

use std::{cell::Cell, f32::consts::TAU, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    Animation, AnyView, App, Bounds, ElementId, FontWeight, Hsla, MouseMoveEvent, PathBuilder,
    Pixels, Point, SharedString, Window, canvas, div, fill, linear_color_stop, linear_gradient,
    pattern_slash, point, prelude::*, px, relative, size, svg,
};

use crate::{
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, AnimationExt, MotionSpec},
    primitives::shadow,
    story,
};

pub const NAME: &str = "Usage Meter";
pub const DESCRIPTION: &str = "A billing cycle at a glance: credits by category with a projection to cycle end, a burn-up chart, plan limits and a call to action that speaks up before the allowance runs out";

const CHART_H: f32 = 104.;
const METER_H: f32 = 10.;
/// Meter segments and projections settling.
const SETTLE: MotionSpec = MotionSpec::new(700, motion::EASE_OUT_QUINT);
const DRAW_IN: MotionSpec = MotionSpec::new(1000, motion::EASE_OUT_QUINT).with_delay(150);
const DIM: MotionSpec = MotionSpec::new(180, motion::EASE);

const ICON_GAUGE: &str = "icons/usage-meter/gauge.svg";
const ICON_ALERT: &str = "icons/usage-meter/alert.svg";
const ICON_CHECK: &str = "icons/usage-meter/check.svg";
const ICON_ARROW: &str = "icons/usage-meter/arrow-up.svg";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Where credits go, e.g. "Agents".
#[derive(Clone)]
pub struct UsageCategory {
    pub name: SharedString,
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
    /// Credits spent on each elapsed day (shorter than `days` while the cycle runs).
    pub daily: Vec<f32>,
    /// The credits used, split by category (in category order).
    pub split: Vec<f32>,
}

impl UsageCycle {
    pub fn new(label: impl Into<SharedString>, days: Vec<SharedString>) -> Self {
        Self {
            label: label.into(),
            days,
            daily: Vec::new(),
            split: Vec::new(),
        }
    }

    pub fn daily(mut self, daily: Vec<f32>) -> Self {
        self.daily = daily;
        self
    }

    pub fn split(mut self, split: Vec<f32>) -> Self {
        self.split = split;
        self
    }

    fn closed(&self) -> bool {
        self.daily.len() >= self.days.len()
    }
}

/// Another plan limit, drawn as a small ring: seats, storage, keys.
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

    /// Share of the allowance at which the footer speaks up even if the
    /// projection stays under it (default 0.85).
    pub fn warn_at(mut self, share: f32) -> Self {
        self.warn_at = share;
        self
    }

    /// The footer's action while the cycle is headed over its allowance.
    pub fn on_raise_limit(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_raise_limit = Some(Rc::new(handler));
        self
    }

    /// The footer's action while the cycle is on track.
    pub fn on_manage(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_manage = Some(Rc::new(handler));
        self
    }
}

struct State {
    cycle: usize,
    day: Option<usize>,
    category: Option<usize>,
    chart: Rc<Cell<Bounds<Pixels>>>,
}

fn palette(t: &Luna) -> [Hsla; 5] {
    let amber = if t.dark {
        t.warning
    } else {
        luna::oklch(0.70, 0.165, 62.)
    };
    // A lighter accent for the second slot keeps the meter in one family.
    let lilac = if t.dark {
        luna::oklch(0.80, 0.10, 285.)
    } else {
        luna::oklch(0.70, 0.15, 290.)
    };
    [t.accent, lilac, t.success, amber, luna::neutral(0.62)]
}

/// The numbers a cycle's footer, meter and chart share.
struct Outlook {
    used: f32,
    /// Credits per day over the last week.
    pace: f32,
    projected: f32,
    /// Day index on which the projection crosses the allowance.
    runs_out: Option<usize>,
}

fn outlook(cycle: &UsageCycle, allowance: f32) -> Outlook {
    let used: f32 = cycle.daily.iter().sum();
    let elapsed = cycle.daily.len();
    let recent = elapsed.min(7);
    let pace = if recent > 0 {
        cycle.daily[elapsed - recent..].iter().sum::<f32>() / recent as f32
    } else {
        0.
    };
    let remaining = cycle.days.len().saturating_sub(elapsed);
    let projected = used + pace * remaining as f32;
    let runs_out = if used >= allowance {
        let mut acc = 0.;
        cycle.daily.iter().position(|d| {
            acc += d;
            acc >= allowance
        })
    } else if projected > allowance && pace > 0. {
        let days = ((allowance - used) / pace).ceil() as usize;
        Some((elapsed + days - 1).min(cycle.days.len().saturating_sub(1)))
    } else {
        None
    };
    Outlook {
        used,
        pace,
        projected,
        runs_out,
    }
}

impl RenderOnce for UsageMeter {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State {
                cycle: 0,
                day: None,
                category: None,
                chart: Rc::new(Cell::new(Bounds::default())),
            },
        );
        if self.cycles.is_empty() {
            return luna::card(&t).id(self.id.clone());
        }
        let (cycle_ix, day, category, chart_cell) = {
            let s = state.read(cx);
            (
                s.cycle.min(self.cycles.len() - 1),
                s.day,
                s.category,
                s.chart.clone(),
            )
        };
        let cycle = &self.cycles[cycle_ix];
        let closed = cycle.closed();
        let o = outlook(cycle, self.allowance);
        let allowance = self.allowance.max(1.);
        let share = o.used / allowance;
        let over = !closed && o.projected > allowance;
        let speak_up = !closed && (over || share >= self.warn_at);
        let colors = palette(&t);
        let color_of = |i: usize| self.categories[i].color.unwrap_or(colors[i % colors.len()]);
        let n_days = cycle.days.len().max(1);
        let elapsed = cycle.daily.len();
        let day = day.filter(|d| *d < n_days);

        // --- Header ---------------------------------------------------
        let tabs = self.cycles.iter().enumerate().map(|(ix, c)| {
            let state = state.clone();
            luna::chip(
                SharedString::from(format!("{key}-cycle-{ix}")),
                ix == cycle_ix,
                ControlSize::Sm,
                &t,
            )
            .child(c.label.clone())
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if s.cycle != ix {
                        s.cycle = ix;
                        s.day = None;
                        cx.notify();
                    }
                })
            })
        });
        let period: SharedString = match (cycle.days.first(), cycle.days.last()) {
            (Some(a), Some(b)) => format!("{a} – {b}").into(),
            _ => "".into(),
        };
        let remaining = n_days.saturating_sub(elapsed);
        let status: SharedString = if closed {
            "closed".into()
        } else if remaining == 1 {
            "resets tomorrow".into()
        } else {
            format!("resets in {remaining} days").into()
        };
        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(space::S12))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S10))
                    .child(
                        div()
                            .flex_none()
                            .size(px(32.))
                            .rounded(px(radius::MD))
                            .bg(t.accent_wash)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path(ICON_GAUGE).size(px(16.)).text_color(if t.dark {
                                t.text
                            } else {
                                t.accent
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::S6))
                                    .child(
                                        div()
                                            .text_size(px(text::CALLOUT))
                                            .line_height(px(18.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(t.text)
                                            .child(self.workspace.clone()),
                                    )
                                    .child(luna::badge(self.plan.clone(), Tone::Accent, &t)),
                            )
                            .child(
                                div()
                                    .text_size(px(text::FOOTNOTE))
                                    .line_height(px(16.))
                                    .text_color(t.text_faint)
                                    .child(format!("{period} · {status}")),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(space::S2))
                    .p(px(space::S2))
                    .rounded(px(radius::MD))
                    .bg(t.fill(FillTier::Quaternary))
                    .children(tabs),
            );

        // --- Headline and meter ---------------------------------------
        let used_shown = motion::follow(&format!("{key}-used"), o.used, SETTLE);
        let share_tone = if share >= 1. {
            Tone::Danger
        } else if speak_up {
            Tone::Warning
        } else {
            Tone::Neutral
        };
        let headline = div()
            .flex()
            .items_end()
            .justify_between()
            .gap(px(space::S12))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(space::S6))
                    .child(
                        div()
                            .text_size(px(28.))
                            .line_height(px(32.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text)
                            .child(grouped(used_shown)),
                    )
                    .child(
                        div()
                            .text_size(px(text::BODY))
                            .text_color(t.text_muted)
                            .child(format!("of {} {}", grouped(self.allowance), self.unit)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .when(!closed, |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S6))
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_muted)
                                .child(div().size(px(10.)).rounded(px(2.)).bg(pattern_slash(
                                    if over { t.warning } else { t.text_faint },
                                    1.5,
                                    3.,
                                )))
                                .child("Projected")
                                .child(
                                    div()
                                        .font_family(luna::MONO)
                                        .text_color(if over { t.warning } else { t.text })
                                        .child(compact(o.projected)),
                                ),
                        )
                    })
                    .child(
                        luna::badge(format!("{:.0}%", share * 100.), share_tone, &t)
                            .font_family(luna::MONO),
                    ),
            );

        // Bar scale: the allowance, stretched when the projection overshoots it.
        let scale = if closed {
            allowance.max(o.used)
        } else {
            allowance.max(o.projected)
        };
        let scale = motion::follow(&format!("{key}-scale"), scale, SETTLE);
        let total_split: f32 = cycle.split.iter().sum::<f32>().max(1.);
        let mut left = 0.;
        let segments: Vec<_> = (0..self.categories.len())
            .map(|i| {
                let amount = cycle.split.get(i).copied().unwrap_or(0.) / total_split * o.used;
                let target = amount / scale;
                let w = motion::follow_from(
                    &format!("{key}-seg-{i}"),
                    0.,
                    target,
                    SETTLE.with_delay(120 + i as u64 * 70),
                );
                let x = left;
                left += w;
                let lit = category.is_none_or(|c| c == i);
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(x))
                    .w(relative(w))
                    .border_r_2()
                    .border_color(t.surface_card)
                    .bg(color_of(i).opacity(motion::follow(
                        &format!("{key}-seg-op-{i}"),
                        if lit { 1. } else { 0.3 },
                        DIM,
                    )))
            })
            .collect();
        let used_frac = left;
        let projection = (!closed).then(|| {
            let end = motion::follow_from(
                &format!("{key}-proj"),
                0.,
                o.projected / scale,
                SETTLE.with_delay(420),
            );
            let w = (end - used_frac).max(0.);
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .left(relative(used_frac))
                .w(relative(w))
                .bg(pattern_slash(
                    if over {
                        t.warning.opacity(0.75)
                    } else {
                        t.text_faint.opacity(0.6)
                    },
                    1.5,
                    3.,
                ))
        });
        let limit_at = (allowance / scale).min(1.);
        let meter = div()
            .relative()
            .pt(px(18.))
            .child(
                // The allowance tick with its label above the bar.
                div()
                    .absolute()
                    .top_0()
                    .left(relative(limit_at))
                    .ml(px(-1.))
                    .h(px(18. + METER_H + 4.))
                    .w(px(2.))
                    .rounded(px(1.))
                    .bg(if over { t.warning } else { t.border_strong }),
            )
            .child(
                div()
                    .absolute()
                    .top(px(-1.))
                    .right(relative(1. - limit_at))
                    .mr(px(6.))
                    .text_size(px(text::CAPTION))
                    .line_height(px(14.))
                    .whitespace_nowrap()
                    .text_color(t.text_faint)
                    .child(format!("Allowance {}", compact(self.allowance))),
            )
            .child(
                div()
                    .relative()
                    .h(px(METER_H))
                    .rounded(px(radius::XS))
                    .overflow_hidden()
                    .bg(t.fill(FillTier::Tertiary))
                    .children(segments)
                    .children(projection),
            );

        let legend = div()
            .flex()
            .flex_wrap()
            .gap_x(px(space::S16))
            .gap_y(px(space::S4))
            .children(self.categories.iter().enumerate().map(|(i, c)| {
                let amount = cycle.split.get(i).copied().unwrap_or(0.) / total_split * o.used;
                let shown = motion::follow(&format!("{key}-cat-{i}"), amount, SETTLE);
                let state_hover = state.clone();
                let lit = category.is_none_or(|x| x == i);
                div()
                    .id(SharedString::from(format!("{key}-legend-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .text_size(px(text::FOOTNOTE))
                    .line_height(px(18.))
                    .cursor_default()
                    .opacity(motion::follow(
                        &format!("{key}-legend-op-{i}"),
                        if lit { 1. } else { 0.5 },
                        DIM,
                    ))
                    .on_hover(move |hovered, _, cx| {
                        state_hover.update(cx, |s, cx| {
                            let next = if *hovered {
                                Some(i)
                            } else if s.category == Some(i) {
                                None
                            } else {
                                s.category
                            };
                            if next != s.category {
                                s.category = next;
                                cx.notify();
                            }
                        })
                    })
                    .child(div().size(px(8.)).rounded(px(2.)).bg(color_of(i)))
                    .child(div().text_color(t.text_muted).child(c.name.clone()))
                    .child(
                        div()
                            .font_family(luna::MONO)
                            .text_color(t.text)
                            .child(grouped(shown)),
                    )
            }));

        // --- Burn-up chart ----------------------------------------------
        let reveal = motion::follow_from(&format!("{key}-reveal-{cycle_ix}"), 0., 1., DRAW_IN);
        let daily = cycle.daily.clone();
        let max_daily = daily.iter().copied().fold(1f32, f32::max);
        let chart_scale = allowance.max(o.projected).max(o.used) * 1.12;
        let accent = t.accent;
        let bar_rest = t.fill(FillTier::Primary);
        let bar_hot = t.accent.opacity(0.55);
        let future = t.fill(FillTier::Secondary);
        let limit_color = if over || share >= 1. {
            t.warning
        } else {
            t.text_faint.opacity(0.7)
        };
        let grid = t.border;
        let card_bg = t.surface_card;
        let projected = o.projected;
        let runs_out = o.runs_out;
        let chart_canvas = canvas(
            {
                let chart_cell = chart_cell.clone();
                move |bounds, _, _| chart_cell.set(bounds)
            },
            move |bounds, _, window, _| {
                let w = f32::from(bounds.size.width);
                let h = f32::from(bounds.size.height);
                let at = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
                let col = w / n_days as f32;
                let y_of = |v: f32| h - 1. - (v / chart_scale).min(1.) * (h - 2.);
                window.paint_quad(fill(Bounds::new(at(0., h - 1.), size(px(w), px(1.))), grid));

                // Daily bars along the bottom third, future days as stubs.
                let bar_w = (col - 3.).max(1.5);
                for i in 0..n_days {
                    let x = i as f32 * col + (col - bar_w) / 2.;
                    match daily.get(i) {
                        Some(v) => {
                            let bh = (v / max_daily * h * 0.34 * reveal).max(2.);
                            let color = if day == Some(i) { bar_hot } else { bar_rest };
                            window.paint_quad(
                                fill(
                                    Bounds::new(at(x, h - 1. - bh), size(px(bar_w), px(bh))),
                                    color,
                                )
                                .corner_radii(px(1.5)),
                            );
                        }
                        None => window.paint_quad(
                            fill(Bounds::new(at(x, h - 3.), size(px(bar_w), px(2.))), future)
                                .corner_radii(px(1.)),
                        ),
                    }
                }

                // The allowance as a dashed rule.
                let ly = y_of(allowance);
                let mut rule = PathBuilder::stroke(px(1.)).dash_array(&[px(3.), px(3.)]);
                rule.move_to(at(0., ly));
                rule.line_to(at(w, ly));
                if let Ok(path) = rule.build() {
                    window.paint_path(path, limit_color);
                }

                // Running total, revealed from the left.
                let mut points = vec![at(0., h - 1.)];
                let mut acc = 0.;
                for (i, v) in daily.iter().enumerate() {
                    acc += v;
                    points.push(at((i + 1) as f32 * col, y_of(acc)));
                }
                let reveal_w = w * reveal;
                window.with_content_mask(
                    Some(gpui::ContentMask {
                        bounds: Bounds::new(bounds.origin, size(px(reveal_w), px(h))),
                    }),
                    |window| {
                        if points.len() > 1 {
                            let mut area = PathBuilder::fill();
                            area.move_to(points[0]);
                            for p in &points[1..] {
                                area.line_to(*p);
                            }
                            area.line_to(at(
                                points
                                    .last()
                                    .map_or(0., |p| f32::from(p.x - bounds.origin.x)),
                                h - 1.,
                            ));
                            area.close();
                            if let Ok(path) = area.build() {
                                window.paint_path(
                                    path,
                                    linear_gradient(
                                        180.,
                                        linear_color_stop(accent.opacity(0.18), 0.),
                                        linear_color_stop(accent.opacity(0.02), 1.),
                                    ),
                                );
                            }
                            let mut line = PathBuilder::stroke(px(2.));
                            line.move_to(points[0]);
                            for p in &points[1..] {
                                line.line_to(*p);
                            }
                            if let Ok(path) = line.build() {
                                window.paint_path(path, accent);
                            }
                        }
                        // The projection to cycle end, dashed.
                        if let Some(last) = points.last().copied()
                            && daily.len() < n_days
                        {
                            let mut proj =
                                PathBuilder::stroke(px(1.5)).dash_array(&[px(4.), px(3.)]);
                            proj.move_to(last);
                            proj.line_to(at(w, y_of(projected)));
                            if let Ok(path) = proj.build() {
                                window.paint_path(path, accent.opacity(0.6));
                            }
                            dot(window, last, 4., accent, card_bg);
                        }
                    },
                );
                if reveal > 0.98
                    && let Some(i) = runs_out
                {
                    let p = at((i + 1) as f32 * col, ly);
                    dot(window, p, 3.5, limit_color, card_bg);
                }
                if let Some(i) = day {
                    let x = (i as f32 + 0.5) * col;
                    window.paint_quad(fill(
                        Bounds::new(at(x.round(), 0.), size(px(1.), px(h))),
                        grid.opacity(1.),
                    ));
                }
            },
        )
        .size_full();

        let readout: SharedString = match day {
            Some(i) => match cycle.daily.get(i) {
                Some(v) => {
                    let acc: f32 = cycle.daily[..=i].iter().sum();
                    format!(
                        "{} · {} {} · {} total",
                        cycle.days[i],
                        grouped(*v),
                        self.unit,
                        grouped(acc)
                    )
                    .into()
                }
                None => format!("{} · not yet", cycle.days[i]).into(),
            },
            None => {
                if closed {
                    format!(
                        "{} {} a day on average",
                        grouped(o.used / n_days as f32),
                        self.unit
                    )
                    .into()
                } else {
                    format!("{} {} a day this week", grouped(o.pace), self.unit).into()
                }
            }
        };
        let chart_w = f32::from(chart_cell.get().size.width);
        let chart = {
            let move_state = state.clone();
            let leave_state = state.clone();
            let chart_cell = chart_cell.clone();
            div()
                .flex()
                .flex_col()
                .gap(px(space::S8))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .child(
                            div()
                                .text_color(t.text_muted)
                                .font_weight(FontWeight::MEDIUM)
                                .child("Burn-up"),
                        )
                        .child(
                            div()
                                .text_color(if day.is_some() { t.text } else { t.text_faint })
                                .child(readout),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("{key}-chart")))
                        .h(px(CHART_H))
                        .child(chart_canvas)
                        .on_mouse_move(move |ev: &MouseMoveEvent, _, cx| {
                            let b = chart_cell.get();
                            if b.size.width <= px(0.) {
                                return;
                            }
                            let fx = ((ev.position.x - b.origin.x) / b.size.width).clamp(0., 0.999);
                            let i = (fx * n_days as f32) as usize;
                            move_state.update(cx, |s, cx| {
                                if s.day != Some(i) {
                                    s.day = Some(i);
                                    cx.notify();
                                }
                            });
                        })
                        .on_hover(move |hovered, _, cx| {
                            if !hovered {
                                leave_state.update(cx, |s, cx| {
                                    s.day = None;
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child({
                    // "Today" sits under the end of the running total.
                    let today_x = elapsed as f32 / n_days as f32 * chart_w;
                    div()
                        .relative()
                        .h(px(14.))
                        .flex()
                        .justify_between()
                        .text_size(px(text::CAPTION))
                        .line_height(px(14.))
                        .text_color(t.text_faint)
                        .child(cycle.days.first().cloned().unwrap_or_default())
                        .child(cycle.days.last().cloned().unwrap_or_default())
                        .when(!closed && elapsed > 0 && chart_w > 0., |d| {
                            d.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left(px(today_x - 60.))
                                    .w(px(120.))
                                    .flex()
                                    .justify_center()
                                    .text_color(t.text_muted)
                                    .child(format!("Today, {}", cycle.days[elapsed - 1])),
                            )
                        })
                })
        };

        // --- Other limits ------------------------------------------------
        let limits = div()
            .flex()
            .gap(px(space::S8))
            .children(self.limits.iter().enumerate().map(|(i, l)| {
                let ratio = if l.limit > 0. {
                    (l.used / l.limit).clamp(0., 1.)
                } else {
                    0.
                };
                let tone = if ratio >= 0.95 {
                    t.danger
                } else if ratio >= 0.8 {
                    t.warning
                } else {
                    t.accent
                };
                let shown = motion::follow_from(
                    &format!("{key}-limit-{i}"),
                    0.,
                    ratio,
                    motion::FILL.with_delay(300 + i as u64 * 90),
                );
                let track = t.fill(FillTier::Secondary);
                let unit = l.unit.clone().map(|u| format!(" {u}")).unwrap_or_default();
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(space::S10))
                    .p(px(space::S8))
                    .rounded(px(radius::MD))
                    .bg(t.fill(FillTier::Quaternary))
                    .child(
                        div().flex_none().size(px(28.)).child(
                            canvas(
                                |_, _, _| {},
                                move |bounds, _, window, _| {
                                    let c = bounds.center();
                                    ring(window, c, 13., 3., 0., TAU - 0.001, track);
                                    if shown > 0.005 {
                                        ring(window, c, 13., 3., 0., shown * TAU, tone);
                                    }
                                },
                            )
                            .size_full(),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(text::CAPTION))
                                    .line_height(px(14.))
                                    .text_color(t.text_faint)
                                    .child(l.label.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap(px(space::S2))
                                    .text_size(px(text::FOOTNOTE))
                                    .line_height(px(16.))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(t.text)
                                            .child(trim_num(l.used)),
                                    )
                                    .child(
                                        div()
                                            .text_color(t.text_faint)
                                            .child(format!("/ {}{unit}", trim_num(l.limit))),
                                    ),
                            ),
                    )
            }));

        // --- Footer: quiet on track, speaks up when headed over ----------
        let message: (SharedString, &str, Hsla) = if closed {
            let left = (allowance - o.used).max(0.);
            (
                format!(
                    "Closed at {:.0}% · {} {} unused",
                    share * 100.,
                    grouped(left),
                    self.unit
                )
                .into(),
                ICON_CHECK,
                t.success,
            )
        } else if o.used >= allowance {
            (
                "Allowance used up · overage billing applies".into(),
                ICON_ALERT,
                t.danger,
            )
        } else if let Some(i) = runs_out {
            (
                format!("On pace to run out on {}", cycle.days[i]).into(),
                ICON_ALERT,
                t.warning,
            )
        } else if speak_up {
            (
                format!("{:.0}% used with {} days to go", share * 100., remaining).into(),
                ICON_ALERT,
                t.warning,
            )
        } else {
            (
                format!(
                    "On track · {} {} to spare",
                    grouped(allowance - o.projected),
                    self.unit
                )
                .into(),
                ICON_CHECK,
                t.success,
            )
        };
        let action = if speak_up {
            let handler = self.on_raise_limit.clone();
            let glow = t.accent;
            luna::button(
                SharedString::from(format!("{key}-raise")),
                ButtonKind::Accent,
                ControlSize::Md,
                &t,
            )
            .child(luna::glyph(ICON_ARROW, ControlSize::Md).text_color(t.on_accent))
            .child("Raise limit")
            .when_some(handler, |b, f| {
                b.on_click(move |_, window, cx| f(window, cx))
            })
            .with_animation(
                SharedString::from(format!("{key}-pulse")),
                Animation::new(Duration::from_millis(1800)).repeat(),
                move |b, t| {
                    // A ring that swells and fades once per beat.
                    let wave = (t / 0.7).min(1.);
                    let alpha = (1. - wave) * 0.45;
                    b.shadow(vec![shadow(0., 0., 0., wave * 6., glow.opacity(alpha))])
                },
            )
            .into_any_element()
        } else {
            let handler = self.on_manage.clone();
            luna::button(
                SharedString::from(format!("{key}-manage")),
                ButtonKind::Secondary,
                ControlSize::Md,
                &t,
            )
            .child("Manage plan")
            .when_some(handler, |b, f| {
                b.on_click(move |_, window, cx| f(window, cx))
            })
            .into_any_element()
        };
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(space::S12))
            .px(px(space::S20))
            .py(px(space::S12))
            .bg(if speak_up {
                t.warning.opacity(if t.dark { 0.06 } else { 0.05 })
            } else {
                t.fill(FillTier::Quaternary)
            })
            .child(motion::rise_in(
                SharedString::from(format!("{key}-msg-{cycle_ix}")),
                motion::VIEW_IN,
                4.,
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .child(svg().path(message.1).size(px(16.)).text_color(message.2))
                    .child(message.0),
            ))
            .child(action);

        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .w(px(600.))
            .overflow_hidden()
            .font_family(luna::SANS)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S20))
                    .p(px(space::S20))
                    .child(header)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(space::S10))
                            .child(headline)
                            .child(meter)
                            .child(legend),
                    )
                    .child(chart)
                    .when(!self.limits.is_empty(), |d| d.child(limits)),
            )
            .child(luna::divider(&t))
            .child(footer)
    }
}

/// A filled circle with a card-colored halo.
fn dot(window: &mut Window, c: Point<Pixels>, r: f32, color: Hsla, halo: Hsla) {
    let o = r + 2.;
    window.paint_quad(
        fill(
            Bounds::new(c - point(px(o), px(o)), size(px(o * 2.), px(o * 2.))),
            halo,
        )
        .corner_radii(px(o)),
    );
    window.paint_quad(
        fill(
            Bounds::new(c - point(px(r), px(r)), size(px(r * 2.), px(r * 2.))),
            color,
        )
        .corner_radii(px(r)),
    );
}

/// A ring arc of `width` centered on radius `r`, clockwise from 12 o'clock.
fn ring(window: &mut Window, c: Point<Pixels>, r: f32, width: f32, a0: f32, a1: f32, color: Hsla) {
    let at = |a: f32| c + point(px(r * a.sin()), px(-r * a.cos()));
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(at(a0));
    path.arc_to(
        point(px(r), px(r)),
        px(0.),
        a1 - a0 > std::f32::consts::PI,
        true,
        at(a1),
    );
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// 7180 → "7,180".
fn grouped(v: f32) -> String {
    let n = v.round() as i64;
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// 11_129 → "11.1k", 10_000 → "10k".
fn compact(v: f32) -> String {
    if v.abs() >= 1000. {
        format!("{}k", trim_num(v / 1000.))
    } else {
        grouped(v)
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
/// with lighter weekends (`first_weekday`: 0 = Monday).
fn daily_spend(days: usize, total: f32, growth: f32, first_weekday: usize, seed: u32) -> Vec<f32> {
    let raw: Vec<f32> = (0..days)
        .map(|d| {
            let ramp = 1. + growth * d as f32 / days.max(1) as f32;
            let weekend = if (first_weekday + d) % 7 >= 5 {
                0.55
            } else {
                1.
            };
            ramp * weekend * (0.8 + 0.4 * jitter(seed + d as u32))
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
    // Oct 1, 2026 is a Thursday; Sep 1 a Tuesday.
    let this_cycle = daily_spend(20, 7_180., 0.9, 3, 11);
    let last_cycle = daily_spend(30, 7_420., 0.25, 1, 57);
    UsageMeter::new("usage-meter", "Northwind Labs", "Team", 10_000.)
        .category(UsageCategory::new("Agents"))
        .category(UsageCategory::new("Chat"))
        .category(UsageCategory::new("Embeddings"))
        .category(UsageCategory::new("Batch jobs"))
        .cycle(
            UsageCycle::new("This cycle", october)
                .daily(this_cycle)
                .split(vec![3_240., 2_010., 1_150., 780.]),
        )
        .cycle(
            UsageCycle::new("Last cycle", september)
                .daily(last_cycle)
                .split(vec![2_610., 2_370., 1_640., 800.]),
        )
        .limit(UsageLimit::new("Seats", 18., 20.))
        .limit(UsageLimit::new("Storage", 41.5, 50.).unit("GB"))
        .limit(UsageLimit::new("API keys", 6., 10.))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| demo())
}
