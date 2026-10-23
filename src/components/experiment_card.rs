//! Experiment Card – an A/B test that reads in two lines and unfolds on hover.
//!
//! Folded, the card is a headline: the test's name, its status and the
//! variant in the lead. Hover it (or click to pin it open) and it unfolds
//! into the evidence: a metric switch, a small chart of each variant's lift
//! over the days, and one row per variant with its value and a confidence
//! interval on a shared lift axis. Hovering a row picks out its line; the
//! numbers roll and the intervals glide when the metric changes. Clicking a
//! row nominates it as the winner, and the footer asks to ship it.
//!
//! Keys, with the card focused: ←/→ switch the metric, ↑/↓ nominate a
//! variant, Enter ships it (or pins the card), Escape steps back.
//!
//! ```ignore
//! ExperimentCard::new("annual", "Annual billing as the default")
//!     .variant(Variant::new("A", "Monthly default"))
//!     .variant(Variant::new("B", "Annual default"))
//!     .metric(ExperimentMetric::new("Upgrade rate", MetricFormat::Percent).result(..))
//!     .on_ship(|ix, _, _| println!("ship variant {ix}"))
//! ```

use std::sync::Arc;
use std::{rc::Rc, time::Duration};

use gpui::{
    Animation, AnyView, App, Bounds, ElementId, FocusHandle, FontWeight, Hsla, KeyDownEvent,
    MouseButton, PathBuilder, Pixels, SharedString, Transformation, Window, canvas, div, fill,
    point, prelude::*, px, radians, size, svg,
};

use crate::{
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, AnimationExt, MotionSpec},
    story,
};

pub const NAME: &str = "Experiment Card";
pub const DESCRIPTION: &str = "An A/B test folded into two lines that unfolds on hover into lift trends and confidence intervals per variant, with a metric switch and a two-step ship";

const ICON_FLASK: &str = "icons/experiment-card/flask.svg";
const ICON_UP: &str = "icons/experiment-card/trend-up.svg";
const ICON_FLAT: &str = "icons/experiment-card/trend-flat.svg";
const ICON_CHECK: &str = "icons/experiment-card/check.svg";
const ICON_CHEVRON: &str = "icons/experiment-card/chevron.svg";
const ICON_ROCKET: &str = "icons/experiment-card/rocket.svg";

const CHART_H: f32 = 64.;
const ROW_H: f32 = 36.;
const TABS_H: f32 = 24.;
const FOOT_H: f32 = 48.;
const CI_W: f32 = 150.;
/// Unfolding and folding.
const UNFOLD: MotionSpec = MotionSpec::new(460, motion::EASE_OUT_QUINT);
/// Intervals and lines settling on a new metric.
const SETTLE: MotionSpec = MotionSpec::new(520, motion::EASE_OUT_QUINT);
const DRAW_IN: MotionSpec = MotionSpec::new(800, motion::EASE_OUT_QUINT).with_delay(80);
const DIM: MotionSpec = MotionSpec::new(180, motion::EASE);

type ShipHandler = Rc<dyn Fn(Option<usize>, &mut Window, &mut App)>;

/// One arm of the test; the first one added is the control.
#[derive(Clone)]
pub struct Variant {
    pub key: SharedString,
    pub name: SharedString,
}

impl Variant {
    pub fn new(key: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
        }
    }
}

/// How a metric's values print.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MetricFormat {
    /// A rate: 0.0412 → "4.12%".
    Percent,
    /// Money per unit: 2.84 → "$2.84".
    Currency,
}

/// A variant's result on one metric. Lift is relative to the control
/// (0.063 = +6.3%), with a confidence interval around it.
#[derive(Clone, Copy, Debug)]
pub struct VariantResult {
    pub value: f32,
    pub lift: f32,
    pub low: f32,
    pub high: f32,
    /// Probability that this variant beats the control.
    pub win_chance: f32,
}

impl VariantResult {
    /// The control's row: its value, no lift.
    pub fn control(value: f32) -> Self {
        Self {
            value,
            lift: 0.,
            low: 0.,
            high: 0.,
            win_chance: 0.5,
        }
    }

    pub fn new(value: f32, lift: f32, low: f32, high: f32, win_chance: f32) -> Self {
        Self {
            value,
            lift,
            low,
            high,
            win_chance,
        }
    }

    fn significant(&self) -> bool {
        self.low > 0. || self.high < 0.
    }
}

#[derive(Clone)]
pub struct ExperimentMetric {
    pub name: SharedString,
    pub format: MetricFormat,
    /// One result per variant, control first.
    pub results: Vec<VariantResult>,
    /// Cumulative lift per day, one series per variant (the control's is ignored).
    pub trend: Vec<Vec<f32>>,
}

impl ExperimentMetric {
    pub fn new(name: impl Into<SharedString>, format: MetricFormat) -> Self {
        Self {
            name: name.into(),
            format,
            results: Vec::new(),
            trend: Vec::new(),
        }
    }

    pub fn result(mut self, result: VariantResult) -> Self {
        self.results.push(result);
        self
    }

    pub fn trend(mut self, trend: Vec<Vec<f32>>) -> Self {
        self.trend = trend;
        self
    }

    fn format(&self, v: f32) -> String {
        match self.format {
            MetricFormat::Percent => format!("{:.2}%", v * 100.),
            MetricFormat::Currency => format!("${v:.2}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExperimentStatus {
    Running { day: u32, of: u32 },
    Paused,
    Concluded,
}

#[derive(IntoElement)]
pub struct ExperimentCard {
    id: ElementId,
    name: SharedString,
    status: ExperimentStatus,
    visitors: Option<u32>,
    variants: Vec<Variant>,
    metrics: Vec<ExperimentMetric>,
    shipped: Option<usize>,
    expanded: bool,
    on_ship: Option<ShipHandler>,
}

impl ExperimentCard {
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            status: ExperimentStatus::Concluded,
            visitors: None,
            variants: Vec::new(),
            metrics: Vec::new(),
            shipped: None,
            expanded: false,
            on_ship: None,
        }
    }

    pub fn status(mut self, status: ExperimentStatus) -> Self {
        self.status = status;
        self
    }

    pub fn visitors(mut self, visitors: u32) -> Self {
        self.visitors = Some(visitors);
        self
    }

    pub fn variant(mut self, variant: Variant) -> Self {
        self.variants.push(variant);
        self
    }

    /// Metrics to switch between; the first is the primary one the headline reports.
    pub fn metric(mut self, metric: ExperimentMetric) -> Self {
        self.metrics.push(metric);
        self
    }

    /// The variant already shipped, if any.
    pub fn shipped(mut self, variant: usize) -> Self {
        self.shipped = Some(variant);
        self
    }

    /// Starts unfolded and pinned open.
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    /// Called with the shipped variant, or `None` when a ship is undone.
    pub fn on_ship(
        mut self,
        handler: impl Fn(Option<usize>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_ship = Some(Rc::new(handler));
        self
    }
}

struct State {
    hovered: bool,
    pinned: bool,
    metric: usize,
    /// Row under the pointer: its line is picked out in the chart.
    hot: Option<usize>,
    /// Nominated winner awaiting confirmation.
    candidate: Option<usize>,
    shipped: Option<usize>,
    focus: FocusHandle,
}

/// Variant colors: the control stays neutral, challengers take the accent family.
fn variant_color(ix: usize, t: &Luna) -> Hsla {
    let amber = if t.dark {
        t.warning
    } else {
        luna::oklch(0.70, 0.165, 62.)
    };
    match ix {
        0 => luna::neutral(0.62),
        1 => t.accent,
        2 => t.success,
        3 => amber,
        _ => t.danger,
    }
}

impl RenderOnce for ExperimentCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let (expanded, shipped) = (self.expanded, self.shipped);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| State {
                hovered: false,
                pinned: expanded,
                metric: 0,
                hot: None,
                candidate: None,
                shipped,
                focus: cx.focus_handle(),
            },
        );
        if self.metrics.is_empty() || self.variants.len() < 2 {
            return luna::card(&t).id(self.id.clone());
        }
        let (hovered, pinned, metric_ix, hot, candidate, shipped, focus) = {
            let s = state.read(cx);
            (
                s.hovered,
                s.pinned,
                s.metric.min(self.metrics.len() - 1),
                s.hot,
                s.candidate,
                s.shipped,
                s.focus.clone(),
            )
        };
        let open = hovered || pinned;
        let open_t = motion::follow(&format!("{key}-open"), if open { 1. } else { 0. }, UNFOLD);
        let n = self.variants.len();
        let metric = &self.metrics[metric_ix];
        let primary = &self.metrics[0];
        let result = |m: &ExperimentMetric, i: usize| {
            m.results
                .get(i)
                .copied()
                .unwrap_or(VariantResult::control(0.))
        };

        // The challenger most likely to beat the control on the primary metric.
        let leader = (1..n).max_by(|a, b| {
            result(primary, *a)
                .win_chance
                .total_cmp(&result(primary, *b).win_chance)
        });

        // --- Folded headline ----------------------------------------------
        let running = matches!(self.status, ExperimentStatus::Running { .. });
        let status_dot = {
            let color = match (shipped, self.status) {
                (Some(_), _) => t.success,
                (None, ExperimentStatus::Running { .. }) => t.accent,
                (None, ExperimentStatus::Paused) => t.warning,
                (None, ExperimentStatus::Concluded) => t.text_faint,
            };
            let dot = div().size(px(6.)).rounded(px(3.)).bg(color);
            if running && shipped.is_none() {
                dot.with_animation(
                    SharedString::from(format!("{key}-live")),
                    Animation::new(Duration::from_millis(1600)).repeat(),
                    |d, t| d.opacity(0.45 + 0.55 * (t * std::f32::consts::TAU).cos().abs()),
                )
                .into_any_element()
            } else {
                dot.into_any_element()
            }
        };
        let meta: SharedString = match (shipped, self.status) {
            (Some(i), _) => format!(
                "Shipped {} · {}",
                self.variants[i].key, self.variants[i].name
            )
            .into(),
            (None, ExperimentStatus::Running { day, of }) => {
                let mut s = format!("Running · day {day} of {of}");
                if let Some(v) = self.visitors {
                    s.push_str(&format!(" · {} visitors", grouped(v)));
                }
                s.into()
            }
            (None, ExperimentStatus::Paused) => "Paused".into(),
            (None, ExperimentStatus::Concluded) => "Concluded · no winner shipped".into(),
        };
        let lead_badge = match shipped {
            Some(i) => badge_with_icon(
                ICON_CHECK,
                format!("{} shipped", self.variants[i].key),
                Tone::Success,
                &t,
            ),
            None => match leader {
                Some(i) => {
                    let r = result(primary, i);
                    let tone = if !r.significant() {
                        Tone::Neutral
                    } else if r.lift >= 0. {
                        Tone::Success
                    } else {
                        Tone::Danger
                    };
                    badge_with_icon(
                        if r.significant() { ICON_UP } else { ICON_FLAT },
                        format!("{} {}", self.variants[i].key, signed_pct(r.lift)),
                        tone,
                        &t,
                    )
                }
                None => luna::badge("–", Tone::Neutral, &t).into_any_element(),
            },
        };
        let head = {
            let toggle_state = state.clone();
            let chevron_turn = open_t * std::f32::consts::PI;
            div()
                .id(SharedString::from(format!("{key}-head")))
                .flex()
                .items_center()
                .gap(px(space::S12))
                .px(px(space::S16))
                .py(px(space::S12))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    toggle_state.update(cx, |s, cx| {
                        s.pinned = !s.pinned;
                        cx.notify();
                    })
                })
                .child(
                    div()
                        .flex_none()
                        .size(px(32.))
                        .rounded(px(radius::MD))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(if shipped.is_some() {
                            t.success.opacity(if t.dark { 0.16 } else { 0.10 })
                        } else {
                            t.accent_wash
                        })
                        .child(
                            svg()
                                .path(if shipped.is_some() {
                                    ICON_ROCKET
                                } else {
                                    ICON_FLASK
                                })
                                .size(px(16.))
                                .text_color(if shipped.is_some() {
                                    t.success
                                } else if t.dark {
                                    t.text
                                } else {
                                    t.accent
                                }),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::BODY))
                                .line_height(px(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(self.name.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S6))
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(status_dot)
                                .child(div().truncate().child(meta)),
                        ),
                )
                .child(lead_badge)
                .child(
                    svg()
                        .path(ICON_CHEVRON)
                        .size(px(14.))
                        .flex_none()
                        .text_color(if pinned { t.text } else { t.text_faint })
                        .with_transformation(Transformation::rotate(radians(chevron_turn))),
                )
        };

        // --- Unfolded evidence --------------------------------------------
        let body_h = TABS_H + 12. + CHART_H + 8. + ROW_H * n as f32 + 12. + 1. + FOOT_H;
        let body = (open_t > 0.001).then(|| {
            // Shared, symmetric lift axis for the chart and the intervals.
            let extent = (1..n)
                .flat_map(|i| {
                    let r = result(metric, i);
                    [r.low.abs(), r.high.abs()]
                })
                .fold(0.02f32, f32::max)
                * 1.15;
            let extent = motion::follow(&format!("{key}-extent"), extent, SETTLE);
            // The trend chart fits its own range, always including zero.
            let (lo, hi) = metric
                .trend
                .iter()
                .skip(1)
                .flatten()
                .fold((0f32, 0f32), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
            let pad = ((hi - lo) * 0.18).max(0.005);
            let lo = motion::follow(&format!("{key}-lo"), lo - pad, SETTLE);
            let hi = motion::follow(&format!("{key}-hi"), hi + pad, SETTLE);
            let reveal = motion::follow_from(&format!("{key}-reveal-{metric_ix}"), 0., 1., DRAW_IN);

            let tabs = self.metrics.iter().enumerate().map(|(ix, m)| {
                let state = state.clone();
                luna::chip(
                    SharedString::from(format!("{key}-metric-{ix}")),
                    ix == metric_ix,
                    ControlSize::Sm,
                    &t,
                )
                .child(m.name.clone())
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.metric = ix;
                        cx.notify();
                    })
                })
            });
            let tabs = div()
                .h(px(TABS_H))
                .mb(px(12.))
                .flex()
                .items_center()
                .justify_between()
                .child(div().flex().gap(px(space::S4)).children(tabs))
                .child(
                    div()
                        .text_size(px(text::CAPTION))
                        .text_color(t.text_faint)
                        .child(format!("Lift vs {}", self.variants[0].key)),
                );

            // Lift over time, one line per challenger.
            let lines: Vec<(Vec<f32>, Hsla, f32)> = (1..n)
                .map(|i| {
                    let series = metric.trend.get(i).cloned().unwrap_or_default();
                    let emphasis = match hot {
                        Some(h) if h != i => 0.25,
                        _ => 1.,
                    };
                    (
                        series,
                        variant_color(i, &t),
                        motion::follow(&format!("{key}-line-op-{i}"), emphasis, DIM),
                    )
                })
                .collect();
            let zero = t.border_strong;
            let card_bg = t.surface_card;
            let chart = canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let w = f32::from(bounds.size.width);
                    let h = f32::from(bounds.size.height);
                    let at = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
                    let y_of = |v: f32| h - 2. - ((v - lo) / (hi - lo).max(1e-4)) * (h - 4.);
                    dashed(window, at(0., y_of(0.)), at(w, y_of(0.)), zero);
                    let mask = gpui::ContentMask {
                        bounds: Bounds::new(bounds.origin, size(px(w * reveal + 6.), px(h))),
                    };
                    window.with_content_mask(Some(mask), |window| {
                        for (series, color, op) in lines.iter() {
                            if series.len() < 2 {
                                continue;
                            }
                            let step = (w - 6.) / (series.len() - 1) as f32;
                            let mut path = PathBuilder::stroke(px(1.75));
                            for (d, v) in series.iter().enumerate() {
                                let p = at(d as f32 * step, y_of(*v));
                                if d == 0 {
                                    path.move_to(p);
                                } else {
                                    path.line_to(p);
                                }
                            }
                            if let Ok(path) = path.build() {
                                window.paint_path(path, color.opacity(*op));
                            }
                            let last = at(
                                (series.len() - 1) as f32 * step,
                                y_of(*series.last().unwrap_or(&0.)),
                            );
                            let r = 3.5;
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        last - point(px(r + 1.5), px(r + 1.5)),
                                        size(px(r * 2. + 3.), px(r * 2. + 3.)),
                                    ),
                                    card_bg,
                                )
                                .corner_radii(px(r + 1.5)),
                            );
                            window.paint_quad(
                                fill(
                                    Bounds::new(
                                        last - point(px(r), px(r)),
                                        size(px(r * 2.), px(r * 2.)),
                                    ),
                                    color.opacity(*op),
                                )
                                .corner_radii(px(r)),
                            );
                        }
                    });
                },
            )
            .size_full();

            // Rows: value, interval on the lift axis, lift.
            let rows = (0..n).map(|i| {
                let v = &self.variants[i];
                let r = result(metric, i);
                let color = variant_color(i, &t);
                let low = motion::follow(&format!("{key}-ci-lo-{i}"), r.low, SETTLE);
                let mid = motion::follow(&format!("{key}-ci-mid-{i}"), r.lift, SETTLE);
                let high = motion::follow(&format!("{key}-ci-hi-{i}"), r.high, SETTLE);
                let chosen = candidate == Some(i) || shipped == Some(i);
                let row_key = format!("{key}-row-{i}");
                let wash = motion::hover_blend(
                    &row_key,
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                );
                let listener = motion::hover_listener(row_key.clone());
                let hover_state = state.clone();
                let click_state = state.clone();
                let lift_color = if i == 0 {
                    t.text_faint
                } else if !r.significant() {
                    t.text_muted
                } else if r.lift >= 0. {
                    t.success
                } else {
                    t.danger
                };
                let whisker_track = t.border;
                let ci = canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let w = f32::from(bounds.size.width);
                        let h = f32::from(bounds.size.height);
                        let at = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
                        let x_of = |v: f32| w / 2. + (v / extent).clamp(-1., 1.) * (w / 2. - 5.);
                        let cy = h / 2.;
                        dashed(window, at(w / 2., 0.), at(w / 2., h), whisker_track);
                        if i == 0 {
                            let r = 3.5;
                            window.paint_quad(gpui::quad(
                                Bounds::new(at(w / 2. - r, cy - r), size(px(r * 2.), px(r * 2.))),
                                px(r),
                                card_bg,
                                px(1.5),
                                color,
                                Default::default(),
                            ));
                            return;
                        }
                        let (x0, x1) = (x_of(low), x_of(high));
                        window.paint_quad(
                            fill(
                                Bounds::new(at(x0, cy - 3.), size(px((x1 - x0).max(2.)), px(6.))),
                                color.opacity(0.3),
                            )
                            .corner_radii(px(3.)),
                        );
                        let x = x_of(mid);
                        let r = 4.;
                        window.paint_quad(
                            fill(
                                Bounds::new(
                                    at(x - r - 1.5, cy - r - 1.5),
                                    size(px(r * 2. + 3.), px(r * 2. + 3.)),
                                ),
                                card_bg,
                            )
                            .corner_radii(px(r + 1.5)),
                        );
                        window.paint_quad(
                            fill(
                                Bounds::new(at(x - r, cy - r), size(px(r * 2.), px(r * 2.))),
                                color,
                            )
                            .corner_radii(px(r)),
                        );
                    },
                )
                .size_full();
                div()
                    .id(SharedString::from(row_key))
                    .h(px(ROW_H))
                    .flex()
                    .items_center()
                    .gap(px(space::S10))
                    .px(px(space::S8))
                    .mx(px(-space::S8))
                    .rounded(px(radius::MD))
                    .bg(if chosen {
                        t.accent.opacity(if t.dark { 0.14 } else { 0.07 })
                    } else {
                        wash
                    })
                    .when(shipped.is_none() && i > 0, |d| d.cursor_pointer())
                    .on_hover(move |h, window, cx| {
                        listener(h, window, cx);
                        hover_state.update(cx, |s, cx| {
                            let next = if *h {
                                Some(i)
                            } else if s.hot == Some(i) {
                                None
                            } else {
                                s.hot
                            };
                            if next != s.hot {
                                s.hot = next;
                                cx.notify();
                            }
                        })
                    })
                    .on_click(move |_, _, cx| {
                        click_state.update(cx, |s, cx| {
                            if s.shipped.is_none() && i > 0 {
                                s.candidate = if s.candidate == Some(i) {
                                    None
                                } else {
                                    Some(i)
                                };
                                cx.notify();
                            }
                        })
                    })
                    .child(
                        div()
                            .flex_none()
                            .size(px(20.))
                            .rounded(px(radius::SM))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(color.opacity(if t.dark { 0.18 } else { 0.12 }))
                            .text_size(px(text::CAPTION))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(if i == 0 { t.text_muted } else { color })
                            .child(v.key.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::BODY))
                            .text_color(t.text)
                            .child(v.name.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(64.))
                            .flex()
                            .justify_end()
                            .overflow_hidden()
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text)
                            .child(motion::rise_in(
                                SharedString::from(format!("{key}-val-{metric_ix}-{i}")),
                                MotionSpec::new(360, motion::EASE_OUT_QUINT)
                                    .with_delay(i as u64 * 40),
                                8.,
                                div().child(metric.format(r.value)),
                            )),
                    )
                    .child(div().flex_none().w(px(CI_W)).h(px(ROW_H)).child(ci))
                    .child(
                        div()
                            .flex_none()
                            .w(px(52.))
                            .flex()
                            .justify_end()
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(lift_color)
                            .child(if i == 0 {
                                "base".to_string()
                            } else {
                                signed_pct(mid)
                            }),
                    )
                    .child(if i == 0 {
                        div().flex_none().size(px(16.))
                    } else {
                        radio(chosen, shipped.is_none(), &t)
                    })
            });

            // Footer: a hint, the confirmation, or the shipped state.
            let footer: gpui::AnyElement = match (shipped, candidate) {
                (Some(i), _) => {
                    let undo_state = state.clone();
                    let handler = self.on_ship.clone();
                    footer_row(&t)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S8))
                                .text_color(t.text_muted)
                                .child(svg().path(ICON_CHECK).size(px(14.)).text_color(t.success))
                                .child(format!(
                                    "{} · {} serves all traffic",
                                    self.variants[i].key, self.variants[i].name
                                )),
                        )
                        .child(
                            luna::button(
                                SharedString::from(format!("{key}-undo")),
                                ButtonKind::Ghost,
                                ControlSize::Sm,
                                &t,
                            )
                            .child("Undo")
                            .on_click(move |_, window, cx| {
                                undo_state.update(cx, |s, cx| {
                                    s.shipped = None;
                                    s.candidate = None;
                                    cx.notify();
                                });
                                if let Some(f) = &handler {
                                    f(None, window, cx);
                                }
                            }),
                        )
                        .into_any_element()
                }
                (None, Some(i)) => {
                    let cancel_state = state.clone();
                    let ship_state = state.clone();
                    let handler = self.on_ship.clone();
                    let r = result(metric, i);
                    footer_row(&t)
                        .child(
                            div()
                                .flex()
                                .gap(px(space::S4))
                                .text_color(t.text_muted)
                                .child("Ship")
                                .child(
                                    div()
                                        .text_color(t.text)
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(self.variants[i].name.clone()),
                                )
                                .child(format!("· {:.0}% to win", r.win_chance * 100.)),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(space::S6))
                                .child(
                                    luna::button(
                                        SharedString::from(format!("{key}-cancel")),
                                        ButtonKind::Ghost,
                                        ControlSize::Sm,
                                        &t,
                                    )
                                    .child("Cancel")
                                    .on_click(
                                        move |_, _, cx| {
                                            cancel_state.update(cx, |s, cx| {
                                                s.candidate = None;
                                                cx.notify();
                                            })
                                        },
                                    ),
                                )
                                .child(
                                    luna::button(
                                        SharedString::from(format!("{key}-ship")),
                                        ButtonKind::Accent,
                                        ControlSize::Sm,
                                        &t,
                                    )
                                    .child(
                                        luna::glyph(ICON_ROCKET, ControlSize::Sm)
                                            .text_color(t.on_accent),
                                    )
                                    .child(format!("Ship {}", self.variants[i].key))
                                    .on_click(
                                        move |_, window, cx| {
                                            ship(&ship_state, i, cx);
                                            if let Some(f) = &handler {
                                                f(Some(i), window, cx);
                                            }
                                        },
                                    ),
                                ),
                        )
                        .into_any_element()
                }
                (None, None) => {
                    let summary = leader.map(|i| {
                        let r = result(metric, i);
                        let color = if r.win_chance >= 0.95 {
                            t.success
                        } else {
                            t.text_muted
                        };
                        div()
                            .flex()
                            .gap(px(space::S4))
                            .child(
                                div()
                                    .font_family(luna::MONO)
                                    .text_color(color)
                                    .child(format!("{:.0}%", r.win_chance * 100.)),
                            )
                            .child(format!(
                                "chance {} beats {}",
                                self.variants[i].key, self.variants[0].key
                            ))
                    });
                    footer_row(&t)
                        .child(
                            div()
                                .text_color(t.text_faint)
                                .child("Pick a winner to ship it"),
                        )
                        .children(summary.map(|s| s.text_color(t.text_muted)))
                        .into_any_element()
                }
            };

            div().h(px(body_h * open_t)).overflow_hidden().child(
                div()
                    .h(px(body_h))
                    .flex()
                    .flex_col()
                    .opacity(((open_t - 0.15) / 0.85).clamp(0., 1.))
                    .child(
                        div()
                            .px(px(space::S16))
                            .pb(px(12.))
                            .child(tabs)
                            .child(div().h(px(CHART_H)).mb(px(8.)).child(chart))
                            .children(rows),
                    )
                    .child(luna::divider(&t))
                    .child(motion::rise_in(
                        SharedString::from(format!(
                            "{key}-foot-{}-{}",
                            shipped.map_or(9, |s| s),
                            candidate.map_or(9, |c| c)
                        )),
                        motion::VIEW_IN,
                        3.,
                        div().h(px(FOOT_H)).child(footer),
                    )),
            )
        });

        let hover_state = state.clone();
        let key_state = state.clone();
        let focus_click = focus.clone();
        let handler = self.on_ship.clone();
        let metric_count = self.metrics.len();
        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .w(px(560.))
            .flex_none()
            .overflow_hidden()
            .font_family(luna::SANS)
            .track_focus(&focus)
            .when(open, |d| d.border_color(t.border_strong))
            .on_hover(move |h, _, cx| {
                hover_state.update(cx, |s, cx| {
                    s.hovered = *h;
                    if !*h {
                        s.hot = None;
                    }
                    cx.notify();
                })
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus_click, cx)
            })
            .on_key_down(move |ev: &KeyDownEvent, window, cx| {
                let k = ev.keystroke.key.as_str();
                let mut shipped_now = None;
                let handled = key_state.update(cx, |s, cx| {
                    match k {
                        "left" => s.metric = s.metric.saturating_sub(1),
                        "right" => s.metric = (s.metric + 1).min(metric_count - 1),
                        "up" | "down" if s.shipped.is_none() => {
                            let cur = s.candidate.unwrap_or(0);
                            s.candidate = Some(if k == "down" {
                                (cur + 1).min(n - 1)
                            } else {
                                cur.saturating_sub(1).max(1)
                            });
                            s.pinned = true;
                        }
                        "enter" => match s.candidate {
                            Some(c) if s.shipped.is_none() => {
                                s.shipped = Some(c);
                                s.candidate = None;
                                shipped_now = Some(c);
                            }
                            _ => s.pinned = !s.pinned,
                        },
                        "escape" => {
                            if s.candidate.is_some() {
                                s.candidate = None;
                            } else {
                                s.pinned = false;
                            }
                        }
                        _ => return false,
                    }
                    cx.notify();
                    true
                });
                if let (Some(c), Some(f)) = (shipped_now, &handler) {
                    f(Some(c), window, cx);
                }
                if handled {
                    cx.stop_propagation();
                }
            })
            .child(head)
            .children(body)
    }
}

fn ship(state: &gpui::Entity<State>, ix: usize, cx: &mut App) {
    state.update(cx, |s, cx| {
        s.shipped = Some(ix);
        s.candidate = None;
        cx.notify();
    })
}

fn footer_row(t: &Luna) -> gpui::Div {
    div()
        .h(px(FOOT_H))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(space::S12))
        .px(px(space::S16))
        .text_size(px(text::FOOTNOTE))
        .bg(t.fill(FillTier::Quaternary))
}

/// A radio dot: hollow at rest, accent-filled when chosen.
fn radio(chosen: bool, enabled: bool, t: &Luna) -> gpui::Div {
    div()
        .flex_none()
        .size(px(16.))
        .rounded(px(8.))
        .border_1()
        .border_color(if chosen {
            t.accent
        } else if enabled {
            t.border_strong
        } else {
            t.border
        })
        .flex()
        .items_center()
        .justify_center()
        .when(chosen, |d| {
            d.child(div().size(px(8.)).rounded(px(4.)).bg(t.accent))
        })
}

fn badge_with_icon(
    icon: &'static str,
    label: impl Into<SharedString>,
    tone: Tone,
    t: &Luna,
) -> gpui::AnyElement {
    luna::badge(label, tone, t)
        .gap(px(space::S4))
        .font_family(luna::MONO)
        .child(svg().path(icon).size(px(12.)).text_color(tone.color(t)))
        .flex_row_reverse()
        .into_any_element()
}

fn dashed(window: &mut Window, from: gpui::Point<Pixels>, to: gpui::Point<Pixels>, color: Hsla) {
    let mut path = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(3.)]);
    path.move_to(from);
    path.line_to(to);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// 0.063 → "+6.3%".
fn signed_pct(v: f32) -> String {
    let sign = if v > 0.0005 {
        "+"
    } else if v < -0.0005 {
        "−"
    } else {
        ""
    };
    format!("{sign}{:.1}%", v.abs() * 100.)
}

fn grouped(v: u32) -> String {
    let digits = v.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Demo
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

/// A metric whose challengers' lift wanders early and settles on the result.
fn metric(
    name: &str,
    format: MetricFormat,
    days: usize,
    seed: u32,
    results: Vec<VariantResult>,
) -> ExperimentMetric {
    let trend = results
        .iter()
        .enumerate()
        .map(|(v, r)| {
            (0..days)
                .map(|d| {
                    if v == 0 || d + 1 == days {
                        return r.lift;
                    }
                    let settle = 1. - (-(d as f32 + 1.) / 3.).exp();
                    let noise = (jitter(seed + v as u32 * 97 + d as u32) - 0.5)
                        * (r.high - r.low).max(0.04)
                        * 1.6
                        / (d as f32 + 1.).sqrt();
                    r.lift * settle + noise
                })
                .collect()
        })
        .collect();
    results
        .into_iter()
        .fold(ExperimentMetric::new(name.to_string(), format), |m, r| {
            m.result(r)
        })
        .trend(trend)
}

fn demo() -> impl IntoElement {
    let annual = ExperimentCard::new("experiment-annual", "Annual billing as the default")
        .status(ExperimentStatus::Running { day: 12, of: 21 })
        .visitors(48_210)
        .variant(Variant::new("A", "Monthly default"))
        .variant(Variant::new("B", "Annual default"))
        .variant(Variant::new("C", "Annual, 2 months free"))
        .metric(metric(
            "Upgrade rate",
            MetricFormat::Percent,
            12,
            3,
            vec![
                VariantResult::control(0.0412),
                VariantResult::new(0.0438, 0.063, 0.012, 0.114, 0.97),
                VariantResult::new(0.0427, 0.036, -0.014, 0.087, 0.84),
            ],
        ))
        .metric(metric(
            "Checkout starts",
            MetricFormat::Percent,
            12,
            17,
            vec![
                VariantResult::control(0.118),
                VariantResult::new(0.111, -0.059, -0.098, -0.02, 0.04),
                VariantResult::new(0.124, 0.051, 0.01, 0.092, 0.98),
            ],
        ))
        .metric(metric(
            "Revenue / visitor",
            MetricFormat::Currency,
            12,
            29,
            vec![
                VariantResult::control(2.84),
                VariantResult::new(3.21, 0.13, 0.061, 0.199, 0.99),
                VariantResult::new(3.02, 0.063, -0.006, 0.132, 0.92),
            ],
        ))
        .expanded(true);
    let checklist = ExperimentCard::new("experiment-checklist", "Guided onboarding checklist")
        .status(ExperimentStatus::Running { day: 5, of: 14 })
        .visitors(9_840)
        .variant(Variant::new("A", "Current checklist"))
        .variant(Variant::new("B", "Guided checklist"))
        .metric(metric(
            "Activation",
            MetricFormat::Percent,
            5,
            41,
            vec![
                VariantResult::control(0.384),
                VariantResult::new(0.389, 0.013, -0.031, 0.057, 0.68),
            ],
        ))
        .metric(metric(
            "Day-7 retention",
            MetricFormat::Percent,
            5,
            53,
            vec![
                VariantResult::control(0.221),
                VariantResult::new(0.23, 0.041, -0.019, 0.101, 0.81),
            ],
        ));
    let trial = ExperimentCard::new("experiment-trial", "Trial length: 14 or 21 days")
        .status(ExperimentStatus::Concluded)
        .variant(Variant::new("A", "14-day trial"))
        .variant(Variant::new("B", "21-day trial"))
        .metric(metric(
            "Trial to paid",
            MetricFormat::Percent,
            21,
            67,
            vec![
                VariantResult::control(0.182),
                VariantResult::new(0.196, 0.077, 0.029, 0.125, 0.99),
            ],
        ))
        .shipped(1);
    // Anchored to the top, so a card unfolding never moves the one under the pointer.
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(space::S10))
        .child(annual)
        .child(checklist)
        .child(trial)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| demo())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lift_formatting() {
        assert_eq!(signed_pct(0.063), "+6.3%");
        assert_eq!(signed_pct(-0.059), "−5.9%");
        assert_eq!(signed_pct(0.), "0.0%");
    }

    #[test]
    fn significance_needs_an_interval_clear_of_zero() {
        assert!(VariantResult::new(1., 0.06, 0.01, 0.11, 0.97).significant());
        assert!(!VariantResult::new(1., 0.03, -0.01, 0.08, 0.8).significant());
        assert!(VariantResult::new(1., -0.06, -0.1, -0.02, 0.03).significant());
    }
}
