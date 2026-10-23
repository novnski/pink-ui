//! Experiment Card – an A/B test as a settings card whose header folds open
//! onto the evidence.
//!
//! Folded, the card is one settings row (Pink `settings/widgets.rs`
//! `card_row` around a `row_tile`): the test's name over its status line, the
//! leading variant's lift as a settings badge, and a disclosure chevron.
//! Clicking the header (Enter / Space when focused) folds the body open on
//! Pink's tool fold (`transcript.rs`, `motion::fold_progress`): the height
//! glides over `DISCLOSURE` while the body fades in and settles.
//!
//! The body is three rows of the same card:
//! - the cumulative lift chart (Pink `shell/usage.rs` `usage_chart`: a
//!   three-line grid, monotone cubic lines over 12% areas, a value gutter),
//!   with a segmented metric switch; pointing at a day draws the guide and
//!   reads the day out in the row's description;
//! - one menu row per variant (Pink `popover::menu_row`): its monogram, its
//!   value, a confidence interval on a shared lift axis and the lift. Pointing
//!   at a row picks its line out of the chart; clicking a challenger picks it;
//! - a footer in Pink's question-wizard footer shape: a quiet note on the
//!   left, Ghost actions and one Primary on the right.
//!
//! Keys, inside the card: ←/→ switch the metric, ↑/↓ pick a challenger,
//! Escape drops the pick, then folds the card.
//!
//! ```ignore
//! ExperimentCard::new("annual", "Annual billing as the default")
//!     .variant(Variant::new("A", "Monthly default"))
//!     .variant(Variant::new("B", "Annual default"))
//!     .metric(ExperimentMetric::new("Upgrade rate", MetricFormat::Percent).result(..))
//!     .on_ship(|ix, _, _| println!("ship variant {ix:?}"))
//! ```

use std::{f32::consts::FRAC_PI_2, rc::Rc, sync::Arc};

use gpui::{
    AnyElement, AnyView, App, Bounds, ElementId, FontWeight, Hsla, KeyDownEvent, PathBuilder, Role,
    SharedString, Toggled, Transformation, Window, canvas, div, fill, point, prelude::*, px,
    radians, size, svg,
};
use web_time::Instant;

use crate::{
    HoverFill,
    motion::{self, EMPHASIZED},
    pink::{
        self, ButtonKind, ControlSize, Pink, Segment, Tone, glyphs, layout, loaders, radius, space,
    },
    story,
};

pub const NAME: &str = "Experiment Card";
pub const DESCRIPTION: &str = "An A/B test whose header folds open onto each variant's cumulative lift, value and confidence interval – switch the metric, point at a day for a readout, pick a winner and ship it";

/// Solar `test-tube-minimalistic` (Pink's icon family; Pink has no test glyph).
const ICON_TEST: &str = "icons/experiment-card/test-tube.svg";
const ICON_PAUSE: &str = "icons/experiment-card/pause.svg";
const ICON_CHEVRON: &str = "icons/experiment-card/alt-arrow-right.svg";

const CARD_W: f32 = 600.;
/// A settings row's title stack: 13/18 title, 2 gap, 12/16 description.
const TEXT_STACK_H: f32 = 36.;
/// The header row: the text stack inside `py 12`.
const HEADER_H: f32 = TEXT_STACK_H + 2. * layout::ROW_PAD_Y;
/// The plot (Pink `usage.rs` keeps 200 for a full page; a card keeps its
/// value gutter and gap at a card's height).
const CHART_H: f32 = 120.;
/// Value-axis gutter and gap (Pink `usage.rs` `CHART_AXIS_WIDTH` / `_GAP`).
const CHART_AXIS_W: f32 = 56.;
const CHART_AXIS_GAP: f32 = 8.;
/// Axis captions: 11/14, `gap 6` under the plot.
const AXIS_LABEL_H: f32 = 14.;
const AXIS_LABEL_GAP: f32 = 6.;
/// The card row's `gap_y` between its text and its block.
const ROW_BLOCK_GAP: f32 = 10.;
/// The chart row: hairline, text stack, then the chart with its day labels.
const TREND_H: f32 = 1.
    + layout::ROW_PAD_Y
    + TEXT_STACK_H
    + ROW_BLOCK_GAP
    + CHART_H
    + AXIS_LABEL_GAP
    + AXIS_LABEL_H
    + layout::ROW_PAD_Y;
/// A menu row: `py 6` around an 18 line.
const VARIANT_ROW_H: f32 = 30.;
/// The variant list's own padding, so row text sits 12 from the hairlines
/// like a card row's.
const LIST_PAD_Y: f32 = 6.;
/// The footer row: hairline, `py 12` around a Md control.
const FOOTER_H: f32 = 1. + 2. * layout::ROW_PAD_Y + ControlSize::Md.height();
/// One metric segment.
const SEGMENT_W: f32 = 96.;
/// Variant row columns.
const VALUE_W: f32 = 64.;
const CI_W: f32 = 140.;
const LIFT_W: f32 = 52.;
const MONOGRAM: f32 = 20.;
/// Other lines while a row is pointed at.
const DIMMED: f32 = 0.3;

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

    fn result_of(&self, ix: usize) -> VariantResult {
        self.results
            .get(ix)
            .copied()
            .unwrap_or(VariantResult::control(0.))
    }

    fn lift_at(&self, ix: usize, day: usize) -> Option<f32> {
        self.trend.get(ix).and_then(|s| s.get(day)).copied()
    }

    /// Days in the longest challenger series.
    fn days(&self) -> usize {
        self.trend.iter().skip(1).map(Vec::len).max().unwrap_or(0)
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

    /// Starts unfolded.
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
    open: bool,
    /// When the fold last flipped, for Pink's fold tween.
    toggled_at: Option<Instant>,
    metric: usize,
    /// The day under the pointer in the chart.
    day: Option<usize>,
    /// The picked challenger, waiting for Ship.
    candidate: Option<usize>,
    shipped: Option<usize>,
}

impl State {
    fn toggle(&mut self) {
        self.open = !self.open;
        self.toggled_at = Some(Instant::now());
    }
}

/// Variant colors: the control stays neutral; challengers take Pink's data
/// tones (`usage.rs` `provider_tone`: #5b9bbd, #d97757), then the project
/// tints.
fn variant_tone(ix: usize, t: &Pink) -> Hsla {
    match ix {
        0 => t.text_muted,
        1 => gpui::rgb(0x5b9bbd).into(),
        2 => gpui::rgb(0xd97757).into(),
        _ => pink::project_tint(ix - 3),
    }
}

/// The lift's copy color: the result's tone only when the interval clears
/// zero (Pink's diff counts: `readable_tone(diff_add / diff_del)`).
fn lift_tone(r: &VariantResult, t: &Pink) -> Hsla {
    if !r.significant() {
        t.readable_muted(0.85)
    } else if r.lift >= 0. {
        t.readable_tone(t.success, 1.0)
    } else {
        t.readable_tone(t.danger, 1.0)
    }
}

fn toggled(on: bool) -> Toggled {
    if on { Toggled::True } else { Toggled::False }
}

fn variants_height(n: usize) -> f32 {
    1. + 2. * LIST_PAD_Y + n as f32 * VARIANT_ROW_H + n.saturating_sub(1) as f32 * layout::MENU_GAP
}

impl RenderOnce for ExperimentCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let (expanded, shipped) = (self.expanded, self.shipped);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State {
                open: expanded,
                toggled_at: None,
                metric: 0,
                day: None,
                candidate: None,
                shipped,
            },
        );
        let card = pink::section_card(&t)
            .mt_0()
            .id(self.id.clone())
            .w(px(CARD_W))
            .flex_none()
            .font_family(pink::SANS);
        let n = self.variants.len();
        if self.metrics.is_empty() || n < 2 {
            return card;
        }
        let (open, toggled_at, metric_ix, day, candidate, shipped) = {
            let s = state.read(cx);
            (
                s.open,
                s.toggled_at,
                s.metric.min(self.metrics.len() - 1),
                s.day,
                s.candidate.filter(|c| *c < n),
                s.shipped.filter(|c| *c < n),
            )
        };
        let reduced = motion::reduced_motion(cx);
        let elapsed = toggled_at
            .map(|at| Instant::now().saturating_duration_since(at))
            .filter(|e| !reduced && *e < motion::FOLD_TWEEN_WINDOW.mul_f32(motion::speed_scale()));
        let (progress, body_alpha) = motion::fold_progress(open, elapsed);
        if elapsed.is_some() {
            window.request_animation_frame();
        }
        let primary = &self.metrics[0];
        // The challenger most likely to beat the control on the primary metric.
        let leader = (1..n).max_by(|a, b| {
            primary
                .result_of(*a)
                .win_chance
                .total_cmp(&primary.result_of(*b).win_chance)
        });
        let inner_radius = radius::inner(radius::SECTION_CARD, 1.);
        let ring = t.focus_ring();

        // ---- Header: the folded headline (Pink settings row + row_tile) ----
        let muted = t.readable_muted(0.85);
        let (glyph, meta): (Option<AnyElement>, String) = match (shipped, self.status) {
            (Some(i), _) => (
                None,
                format!(
                    "{} · {} serves all traffic",
                    self.variants[i].key, self.variants[i].name
                ),
            ),
            (None, ExperimentStatus::Running { day, of }) => {
                let mut s = format!("Running · day {day} of {of}");
                if let Some(v) = self.visitors {
                    s.push_str(&format!(" · {} visitors", grouped(v)));
                }
                // A live test breathes (Pink's working orb).
                (Some(loaders::orb_slot(t.accent).into_any_element()), s)
            }
            (None, ExperimentStatus::Paused) => {
                let mut s = String::from("Paused");
                if let Some(v) = self.visitors {
                    s.push_str(&format!(" · {} visitors", grouped(v)));
                }
                // A waiting state wears its glyph (Pink: shape before color).
                let pause = svg()
                    .path(ICON_PAUSE)
                    .flex_none()
                    .size(px(pink::icon_size::XS))
                    .text_color(t.readable_icon(t.text_muted));
                (Some(pause.into_any_element()), s)
            }
            (None, ExperimentStatus::Concluded) => (None, "Concluded · no variant shipped".into()),
        };
        let lead_badge = match (shipped, leader) {
            (Some(_), _) => Some(pink::badge("Shipped", Tone::Success, &t)),
            (None, Some(i)) => {
                let r = primary.result_of(i);
                let tone = match (r.significant(), r.lift >= 0.) {
                    (false, _) => Tone::Neutral,
                    (true, true) => Tone::Success,
                    (true, false) => Tone::Danger,
                };
                Some(pink::badge(
                    format!("{} {}", self.variants[i].key, signed_pct(r.lift)),
                    tone,
                    &t,
                ))
            }
            (None, None) => None,
        };
        let header = {
            let toggle_state = state.clone();
            div()
                .id(SharedString::from(format!("{key}-head")))
                .role(Role::Button)
                .aria_label(self.name.clone())
                .aria_expanded(open)
                .tab_index(0)
                .flex_none()
                .h(px(HEADER_H))
                .px(px(layout::ROW_PAD_X))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S12))
                .rounded_t(px(inner_radius))
                .when(progress <= 0., |el| el.rounded_b(px(inner_radius)))
                .cursor_pointer()
                .focus_visible(move |s| pink::focus_ring(s, ring))
                .on_key_down(pink::swallow_activation_key)
                // Pink `changes.rs` file header: the fold's header brightens a
                // touch under the pointer.
                .hover_fill(format!("{key}-head"), t.ink(0.), t.ink(0.05))
                .on_click(move |_, _, cx| {
                    toggle_state.update(cx, |s, cx| {
                        s.toggle();
                        cx.notify();
                    })
                })
                .child(pink::row_tile(ICON_TEST, &t))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(single_line(pink::row_title(self.name.clone(), &t)))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(space::S6))
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(muted)
                                .children(glyph)
                                .child(div().flex_1().min_w_0().truncate().child(meta)),
                        ),
                )
                .children(lead_badge.map(|b| {
                    b.line_height(px(14.))
                        .font_features(pink::tabular_figures())
                }))
                .child(disclosure_chevron(progress, &t))
        };

        // ---- Body: chart, variants, footer (folds on Pink's tool fold) ----
        let body = (progress > 0.).then(|| {
            let metric = &self.metrics[metric_ix];
            let trend = trend_row(&key, &self, metric_ix, day, &state, &t);
            let rows =
                (0..n).map(|i| variant_row(&key, &self, metric, i, candidate, shipped, &state, &t));
            let list = div()
                .flex_none()
                .h(px(variants_height(n)))
                .border_t_1()
                .border_color(t.border)
                .px(px(layout::ROW_PAD_X - 8.))
                .py(px(LIST_PAD_Y))
                .flex()
                .flex_col()
                .gap(px(layout::MENU_GAP))
                .children(rows);
            let footer = footer_row(&key, &self, leader, candidate, shipped, &state, &t);
            let body_h = TREND_H + variants_height(n) + FOOTER_H;
            div()
                .flex_none()
                .h(px(body_h * progress))
                .overflow_hidden()
                .child(
                    div()
                        .relative()
                        .top(px(motion::TOOL_BODY_RISE * (1. - body_alpha)))
                        .opacity(body_alpha)
                        .h(px(body_h))
                        .flex()
                        .flex_col()
                        .child(trend)
                        .child(list)
                        .child(footer),
                )
        });

        let key_state = state.clone();
        let metric_count = self.metrics.len();
        motion::keep_alive(window);
        card.on_key_down(move |ev: &KeyDownEvent, _, cx| {
            let k = ev.keystroke.key.as_str();
            let handled = key_state.update(cx, |s, cx| {
                match k {
                    "left" if s.open && s.metric > 0 => {
                        s.metric -= 1;
                        s.day = None;
                    }
                    "right" if s.open && s.metric + 1 < metric_count => {
                        s.metric += 1;
                        s.day = None;
                    }
                    "up" | "down" if s.shipped.is_none() => {
                        let cur = s.candidate.unwrap_or(if k == "down" { 0 } else { n });
                        s.candidate = Some(if k == "down" {
                            (cur + 1).min(n - 1)
                        } else {
                            cur.saturating_sub(1).max(1)
                        });
                        if !s.open {
                            s.toggle();
                        }
                    }
                    "escape" if s.candidate.is_some() => s.candidate = None,
                    "escape" if s.open => s.toggle(),
                    _ => return false,
                }
                cx.notify();
                true
            });
            if handled {
                cx.stop_propagation();
            }
        })
        .child(header)
        .children(body)
    }
}

/// The header's disclosure (Pink `changes.rs` file header): alt-arrow-right
/// at 12 in a 14 slot, `readable_muted(0.7)`, turning a quarter with the fold.
fn disclosure_chevron(progress: f32, t: &Pink) -> gpui::Div {
    div()
        .flex_none()
        .size(px(pink::icon_size::SM))
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(ICON_CHEVRON)
                .size(px(pink::icon_size::XS))
                .text_color(t.readable_muted(0.7))
                .with_transformation(Transformation::rotate(radians(FRAC_PI_2 * progress))),
        )
}

/// One line of a settings row's text that truncates instead of wrapping.
fn single_line(text: gpui::Div) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .child(text.flex_1().min_w_0().truncate())
}

/// The chart row (Pink `usage.rs`: the trend row of the summary card): its
/// title and a description that reads out the pointed day, the metric switch
/// as the row's control, then the chart.
fn trend_row(
    key: &str,
    card: &ExperimentCard,
    metric_ix: usize,
    day: Option<usize>,
    state: &gpui::Entity<State>,
    t: &Pink,
) -> gpui::Div {
    let metric = &card.metrics[metric_ix];
    let n = card.variants.len();
    let days = metric.days();
    let day = day.filter(|d| *d < days);
    let control = &card.variants[0];
    let description: SharedString = match day {
        Some(d) => {
            let mut s = format!("Day {}", d + 1);
            for i in 1..n {
                if let Some(v) = metric.lift_at(i, d) {
                    s.push_str(&format!(" · {} {}", card.variants[i].key, signed_pct(v)));
                }
            }
            s.into()
        }
        None => format!("Against {} · {}, by day", control.key, control.name).into(),
    };
    let switch = (card.metrics.len() > 1).then(|| {
        let segments = card
            .metrics
            .iter()
            .enumerate()
            .map(|(ix, m)| Segment::new(ix.to_string(), m.name.clone()))
            .collect();
        let state = state.clone();
        pink::segmented(
            format!("{key}-metric"),
            segments,
            metric_ix,
            SEGMENT_W * card.metrics.len() as f32,
            ControlSize::Md,
            t,
            move |ix: &usize, _, cx| {
                let ix = *ix;
                state.update(cx, |s, cx| {
                    if s.metric != ix {
                        s.metric = ix;
                        s.day = None;
                        cx.notify();
                    }
                })
            },
        )
    });
    div()
        .flex_none()
        .h(px(TREND_H))
        .border_t_1()
        .border_color(t.border)
        .px(px(layout::ROW_PAD_X))
        .py(px(layout::ROW_PAD_Y))
        .flex()
        .flex_col()
        .gap(px(ROW_BLOCK_GAP))
        .child(
            div()
                .h(px(TEXT_STACK_H))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S12))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(single_line(pink::row_title(
                            format!("{} lift", metric.name),
                            t,
                        )))
                        .child(single_line(
                            pink::row_description(description, t)
                                .font_features(pink::tabular_figures()),
                        )),
                )
                .children(switch),
        )
        .child(trend_chart(key, card, metric, day, state, t))
}

/// The lift chart (Pink `usage.rs` `usage_chart`): three gridlines in
/// `border`, here at +scale, 0 and −scale on a nice-number scale; per
/// challenger a 12% area to the zero line and a 2 px monotone cubic line
/// (largest painted first); a 1 px `text_muted` guide on the pointed day; one
/// hover cell per day; 11/14 axis captions in `readable_muted(0.85)`. Values
/// glide to a new metric over `EMPHASIZED`; pointing at a variant row dims
/// the other lines with the row's hover fade.
fn trend_chart(
    key: &str,
    card: &ExperimentCard,
    metric: &ExperimentMetric,
    day: Option<usize>,
    state: &gpui::Entity<State>,
    t: &Pink,
) -> gpui::Div {
    let n = card.variants.len();
    let days = metric.days();
    let peak = (1..n)
        .flat_map(|i| metric.trend.get(i).into_iter().flatten())
        .fold(0f32, |m, v| m.max(v.abs()));
    let scale_target = nice_scale(peak);
    let scale = motion::follow(&format!("{key}-scale"), scale_target, EMPHASIZED);
    let hot: Vec<f32> = (0..n)
        .map(|i| {
            if i == 0 {
                0.
            } else {
                motion::hover_t(&hot_key(key, i))
            }
        })
        .collect();
    // Largest first, so smaller series and their lines stay visible.
    let mut order: Vec<usize> = (1..n).filter(|i| metric.trend.get(*i).is_some()).collect();
    let weight = |i: usize| -> f32 { metric.trend[i].iter().map(|v| v.abs()).sum() };
    order.sort_by(|a, b| weight(*b).total_cmp(&weight(*a)));
    let lines: Vec<(Hsla, Vec<f32>)> = order
        .into_iter()
        .map(|i| {
            let others = (1..n)
                .filter(|j| *j != i)
                .map(|j| hot[j])
                .fold(0f32, f32::max);
            let emphasis = 1. - (1. - DIMMED) * others;
            let values = (0..days)
                .map(|d| {
                    let v = metric.lift_at(i, d).unwrap_or(0.);
                    motion::follow(&format!("{key}-pt-{i}-{d}"), v, EMPHASIZED)
                })
                .collect();
            (variant_tone(i, t).opacity(emphasis), values)
        })
        .collect();
    let grid = t.border;
    let guide = t.text_muted;
    let plot = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            if width < 2. || height < 2. {
                return;
            }
            let (x0, y0) = (bounds.origin.x, bounds.origin.y);
            for ratio in [0.0_f32, 0.5, 1.0] {
                let mut path = PathBuilder::stroke(px(1.));
                let y = y0 + px(ratio * height);
                path.move_to(point(x0, y));
                path.line_to(point(x0 + px(width), y));
                if let Ok(path) = path.build() {
                    window.paint_path(path, grid);
                }
            }
            if days == 0 {
                return;
            }
            let mid = height / 2.;
            let x_of = |d: usize| {
                if days == 1 {
                    width / 2.
                } else {
                    width * d as f32 / (days - 1) as f32
                }
            };
            let y_of = |v: f32| mid - (v / scale.max(1e-4)).clamp(-1., 1.) * (mid - 4.);
            for fill in [true, false] {
                for (color, values) in &lines {
                    let coords: Vec<(f32, f32)> = values
                        .iter()
                        .enumerate()
                        .map(|(d, v)| (x_of(d), y_of(*v)))
                        .collect();
                    if coords.is_empty() {
                        continue;
                    }
                    let mut path = if fill {
                        PathBuilder::fill()
                    } else {
                        PathBuilder::stroke(px(2.))
                    };
                    path.move_to(point(x0 + px(coords[0].0), y0 + px(coords[0].1)));
                    // A monotone cubic never draws a false peak between two
                    // measured days.
                    let tangents = monotone_tangents(&coords);
                    for (i, pair) in coords.windows(2).enumerate() {
                        let ((xa, ya), (xb, yb)) = (pair[0], pair[1]);
                        let dx = (xb - xa) / 3.;
                        path.cubic_bezier_to(
                            point(x0 + px(xb), y0 + px(yb)),
                            point(x0 + px(xa + dx), y0 + px(ya + tangents[i] * dx)),
                            point(x0 + px(xb - dx), y0 + px(yb - tangents[i + 1] * dx)),
                        );
                    }
                    if fill {
                        let last = coords[coords.len() - 1].0;
                        path.line_to(point(x0 + px(last), y0 + px(mid)));
                        path.line_to(point(x0 + px(coords[0].0), y0 + px(mid)));
                        path.close();
                    }
                    if let Ok(path) = path.build() {
                        let alpha = color.a;
                        window.paint_path(
                            path,
                            if fill {
                                color.opacity(0.12 * alpha)
                            } else {
                                *color
                            },
                        );
                    }
                }
            }
            if let Some(d) = day {
                let x = x_of(d);
                let mut line = PathBuilder::stroke(px(1.));
                line.move_to(point(x0 + px(x), y0));
                line.line_to(point(x0 + px(x), y0 + px(height)));
                if let Ok(line) = line.build() {
                    window.paint_path(line, guide);
                }
            }
        },
    )
    .w_full()
    .h(px(CHART_H));

    let cells = (0..days).map(|d| {
        let state = state.clone();
        div()
            .id(SharedString::from(format!("{key}-day-{d}")))
            .flex_1()
            .h_full()
            .on_hover(move |hovered, _, cx| {
                state.update(cx, |s, cx| {
                    if *hovered {
                        s.day = Some(d);
                    } else if s.day == Some(d) {
                        s.day = None;
                    }
                    cx.notify();
                })
            })
    });
    let y_labels = [
        signed_axis(scale_target),
        "0%".to_string(),
        signed_axis(-scale_target),
    ];
    let x_labels: [String; 3] = if days == 0 {
        Default::default()
    } else {
        [0, days / 2, days - 1].map(|d| format!("Day {}", d + 1).to_uppercase())
    };
    let label_tone = t.readable_muted(0.85);
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(AXIS_LABEL_GAP))
        .text_size(px(11.))
        .line_height(px(AXIS_LABEL_H))
        .text_color(label_tone)
        .font_features(pink::tabular_figures())
        .child(
            div()
                .w_full()
                .flex()
                .gap(px(CHART_AXIS_GAP))
                .child(
                    div()
                        .w(px(CHART_AXIS_W))
                        .h(px(CHART_H))
                        .flex_none()
                        .flex()
                        .flex_col()
                        .justify_between()
                        .text_align(gpui::TextAlign::Right)
                        .children(y_labels),
                )
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .h(px(CHART_H))
                        .child(plot)
                        .child(div().absolute().inset_0().flex().children(cells)),
                ),
        )
        .child(
            div()
                .w_full()
                .pl(px(CHART_AXIS_W + CHART_AXIS_GAP))
                .flex()
                .justify_between()
                .children(x_labels),
        )
}

fn hot_key(key: &str, ix: usize) -> String {
    format!("{key}-hot-{ix}")
}

/// One variant (Pink `popover::menu_row`: gap 10, `px 8 / py 6`, radius 8,
/// 13, `text @ 0.9` → `text`, the hover wash `menu_hover_bg()`, the pick
/// `card_selected_bg()`): its monogram tile (Pink `project_tile`) in the
/// variant's tone, its name, its value, the interval and the lift, and a
/// trailing 14 px check on the pick or the shipped variant (Pink
/// `menu_row_nav`). The control and a shipped test's rows are records, not
/// controls: same anatomy, no hover, no tab stop.
#[allow(clippy::too_many_arguments)]
fn variant_row(
    key: &str,
    card: &ExperimentCard,
    metric: &ExperimentMetric,
    i: usize,
    candidate: Option<usize>,
    shipped: Option<usize>,
    state: &gpui::Entity<State>,
    t: &Pink,
) -> AnyElement {
    let v = &card.variants[i];
    let r = metric.result_of(i);
    let tone = variant_tone(i, t);
    let picked = candidate == Some(i) && shipped.is_none();
    let checked = picked || shipped == Some(i);
    let interactive = i > 0 && shipped.is_none();

    // The interval, on an axis shared by every row.
    let extent_target = (1..card.variants.len())
        .map(|j| metric.result_of(j))
        .fold(0.02f32, |m, r| m.max(r.low.abs()).max(r.high.abs()))
        * 1.15;
    let extent = motion::follow(&format!("{key}-extent"), extent_target, EMPHASIZED);
    let low = motion::follow(&format!("{key}-ci-lo-{i}"), r.low, EMPHASIZED);
    let mid = motion::follow(&format!("{key}-ci-mid-{i}"), r.lift, EMPHASIZED);
    let high = motion::follow(&format!("{key}-ci-hi-{i}"), r.high, EMPHASIZED);
    let zero = t.border;
    let interval = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let w = f32::from(bounds.size.width);
            let h = f32::from(bounds.size.height);
            let at = |x: f32, y: f32| bounds.origin + point(px(x), px(y));
            let x_of = |v: f32| w / 2. + (v / extent.max(1e-4)).clamp(-1., 1.) * (w / 2. - 4.);
            let cy = h / 2.;
            window.paint_quad(fill(
                Bounds::new(at(w / 2. - 0.5, 0.), size(px(1.), px(h))),
                zero,
            ));
            if i == 0 {
                // The control sits on zero: a hollow mark.
                window.paint_quad(gpui::quad(
                    Bounds::new(at(w / 2. - 4., cy - 4.), size(px(8.), px(8.))),
                    px(4.),
                    gpui::transparent_black(),
                    px(1.5),
                    tone,
                    Default::default(),
                ));
                return;
            }
            let (x0, x1) = (x_of(low), x_of(high));
            window.paint_quad(
                fill(
                    Bounds::new(at(x0, cy - 3.), size(px((x1 - x0).max(6.)), px(6.))),
                    tone.opacity(0.35),
                )
                .corner_radii(px(3.)),
            );
            let x = x_of(mid);
            window.paint_quad(
                fill(Bounds::new(at(x - 4., cy - 4.), size(px(8.), px(8.))), tone)
                    .corner_radii(px(4.)),
            );
        },
    )
    .size_full();

    let value = metric.format(r.value);
    let lift_copy = if i == 0 {
        "Control".to_string()
    } else {
        signed_pct(r.lift)
    };
    let cells: Vec<AnyElement> = vec![
        pink::monogram_tile(tone, &v.key, MONOGRAM, t).into_any_element(),
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .child(v.name.clone())
            .into_any_element(),
        div()
            .flex_none()
            .w(px(VALUE_W))
            .flex()
            .justify_end()
            .text_size(px(12.))
            .text_color(t.readable_muted(0.85))
            .child(motion::status_swap(
                loaders::label_id(&format!("{key}-value-{i}"), &value),
                div().child(value),
            ))
            .into_any_element(),
        div()
            .flex_none()
            .w(px(CI_W))
            .h(px(18.))
            .child(interval)
            .into_any_element(),
        div()
            .flex_none()
            .w(px(LIFT_W))
            .flex()
            .justify_end()
            .text_size(px(12.))
            .font_weight(if i == 0 {
                FontWeight::NORMAL
            } else {
                FontWeight::MEDIUM
            })
            .text_color(if i == 0 {
                t.readable_faint(0.9)
            } else {
                lift_tone(&r, t)
            })
            .child(motion::status_swap(
                loaders::label_id(&format!("{key}-lift-{i}"), &lift_copy),
                div().child(lift_copy),
            ))
            .into_any_element(),
        div()
            .flex_none()
            .size(px(pink::icon_size::SM))
            .when(checked, |d| {
                d.child(pink::line_glyph(glyphs::CHECK, pink::icon_size::SM, t.text).size_full())
            })
            .into_any_element(),
    ];

    let row = if interactive {
        let ring = t.focus_ring();
        let pick_state = state.clone();
        pink::menu_row(format!("{key}-variant-{i}"), picked, t)
            .role(Role::RadioButton)
            .aria_label(format!("{} · {}", v.key, v.name))
            .aria_toggled(toggled(picked))
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .on_key_down(pink::swallow_activation_key)
            .on_click(move |_, _, cx| {
                pick_state.update(cx, |s, cx| {
                    if s.shipped.is_none() {
                        s.candidate = if s.candidate == Some(i) {
                            None
                        } else {
                            Some(i)
                        };
                        cx.notify();
                    }
                })
            })
            .h(px(VARIANT_ROW_H))
            .line_height(px(18.))
            .children(cells)
            .into_any_element()
    } else {
        div()
            .h(px(VARIANT_ROW_H))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .px(px(8.))
            .py(px(6.))
            .rounded(px(radius::MENU_ITEM))
            .text_size(px(13.))
            .line_height(px(18.))
            .text_color(t.text.opacity(0.9))
            .children(cells)
            .into_any_element()
    };
    // The row's own hover fade, read by the chart to pick its line out.
    let hot = hot_key(key, i);
    div()
        .id(SharedString::from(hot.clone()))
        .font_features(pink::tabular_figures())
        .when(i > 0, |d| d.on_hover(motion::hover_listener(hot)))
        .child(row)
        .into_any_element()
}

/// The footer (Pink composer question-wizard footer: a 12 note in
/// `readable_muted(0.7)` on the left; Ghost actions and one Primary, held at
/// 45% until there's something to ship, on the right). A new note arrives on
/// `STATUS_SWAP`.
#[allow(clippy::too_many_arguments)]
fn footer_row(
    key: &str,
    card: &ExperimentCard,
    leader: Option<usize>,
    candidate: Option<usize>,
    shipped: Option<usize>,
    state: &gpui::Entity<State>,
    t: &Pink,
) -> gpui::Div {
    let primary = &card.metrics[0];
    let control = &card.variants[0].key;
    let note_tone = t.readable_muted(0.7);
    let (note, lead_glyph, actions): (String, Option<AnyElement>, Vec<AnyElement>) =
        match (shipped, candidate) {
            (Some(i), _) => {
                let undo_state = state.clone();
                let handler = card.on_ship.clone();
                (
                    format!("{} serves all traffic", card.variants[i].name),
                    Some(
                        pink::line_glyph(
                            glyphs::CHECK,
                            pink::icon_size::SM,
                            t.readable_tone(t.success, 1.0),
                        )
                        .flex_none()
                        .size(px(pink::icon_size::SM))
                        .into_any_element(),
                    ),
                    vec![
                        pink::button(format!("{key}-undo"), ButtonKind::Ghost, ControlSize::Md, t)
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
                            })
                            .into_any_element(),
                    ],
                )
            }
            (None, Some(i)) => {
                let cancel_state = state.clone();
                let ship_state = state.clone();
                let handler = card.on_ship.clone();
                let r = primary.result_of(i);
                (
                    format!(
                        "Ships {} to all traffic · {:.0}% chance to beat {control}",
                        card.variants[i].name,
                        r.win_chance * 100.
                    ),
                    None,
                    vec![
                        pink::button(
                            format!("{key}-cancel"),
                            ButtonKind::Ghost,
                            ControlSize::Md,
                            t,
                        )
                        .child("Cancel")
                        .on_click(move |_, _, cx| {
                            cancel_state.update(cx, |s, cx| {
                                s.candidate = None;
                                cx.notify();
                            })
                        })
                        .into_any_element(),
                        pink::button(
                            format!("{key}-ship"),
                            ButtonKind::Primary,
                            ControlSize::Md,
                            t,
                        )
                        .child(format!("Ship {}", card.variants[i].key))
                        .on_click(move |_, window, cx| {
                            ship_state.update(cx, |s, cx| {
                                s.shipped = Some(i);
                                s.candidate = None;
                                cx.notify();
                            });
                            if let Some(f) = &handler {
                                f(Some(i), window, cx);
                            }
                        })
                        .into_any_element(),
                    ],
                )
            }
            (None, None) => (
                match leader {
                    Some(i) => format!(
                        "Pick a variant to ship · {} has a {:.0}% chance to beat {control}",
                        card.variants[i].key,
                        primary.result_of(i).win_chance * 100.
                    ),
                    None => "Pick a variant to ship".into(),
                },
                None,
                vec![
                    pink::button_enabled(
                        format!("{key}-ship"),
                        ButtonKind::Primary,
                        ControlSize::Md,
                        false,
                        t,
                    )
                    .child("Ship")
                    .into_any_element(),
                ],
            ),
        };
    div()
        .flex_none()
        .h(px(FOOTER_H))
        .border_t_1()
        .border_color(t.border)
        .px(px(layout::ROW_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S12))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S6))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(note_tone)
                .font_features(pink::tabular_figures())
                .children(lead_glyph)
                .child(div().flex_1().min_w_0().flex().child(motion::status_swap(
                    loaders::label_id(&format!("{key}-note"), &note),
                    div().flex_1().min_w_0().truncate().child(note.clone()),
                ))),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S8))
                .children(actions),
        )
}

/// Pink's monotone cubic tangents (`usage.rs` `monotone_usage_tangents`).
fn monotone_tangents(points: &[(f32, f32)]) -> Vec<f32> {
    if points.len() < 2 {
        return vec![0.0; points.len()];
    }
    let slopes: Vec<f32> = points
        .windows(2)
        .map(|pair| {
            let dx = pair[1].0 - pair[0].0;
            if dx == 0.0 {
                0.0
            } else {
                (pair[1].1 - pair[0].1) / dx
            }
        })
        .collect();
    let mut tangents = vec![0.0; points.len()];
    tangents[0] = slopes[0];
    tangents[points.len() - 1] = slopes[slopes.len() - 1];
    for ix in 1..points.len() - 1 {
        let (previous, next) = (slopes[ix - 1], slopes[ix]);
        tangents[ix] = if previous * next <= 0.0 {
            0.0
        } else {
            (previous + next) / 2.0
        };
    }
    for (ix, slope) in slopes.iter().copied().enumerate() {
        if slope == 0.0 {
            tangents[ix] = 0.0;
            tangents[ix + 1] = 0.0;
            continue;
        }
        let a = tangents[ix] / slope;
        let b = tangents[ix + 1] / slope;
        let magnitude = a * a + b * b;
        if magnitude > 9.0 {
            let scale = 3.0 / magnitude.sqrt();
            tangents[ix] = scale * a * slope;
            tangents[ix + 1] = scale * b * slope;
        }
    }
    tangents
}

/// Pink's nice-number chart scale (`usage.rs` `usage_chart`): the peak
/// rounded up to a whole number of 1/2/5 steps near half of it. A flat chart
/// keeps a 1% scale.
fn nice_scale(peak: f32) -> f32 {
    if peak <= 0. {
        return 0.01;
    }
    let rough = peak / 2.;
    let magnitude = 10f32.powf(rough.log10().floor());
    let unit = [1., 2., 5., 10.]
        .into_iter()
        .find(|step| step * magnitude >= rough)
        .unwrap_or(10.)
        * magnitude;
    (peak / unit).ceil() * unit
}

/// An axis caption: 0.2 → "+20%", 0.015 → "+1.5%".
fn signed_axis(v: f32) -> String {
    let pct = (v * 1000.).round() / 10.;
    let sign = if pct > 0. {
        "+"
    } else if pct < 0. {
        "−"
    } else {
        ""
    };
    if pct.fract() == 0. {
        format!("{sign}{:.0}%", pct.abs())
    } else {
        format!("{sign}{:.1}%", pct.abs())
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

/// 48210 → "48,210" (Pink `usage.rs` `count`).
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
            "Checkouts",
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
            "Revenue",
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
        .status(ExperimentStatus::Paused)
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
            "Retention",
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
        .gap(px(space::S12))
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
        assert_eq!(signed_axis(0.2), "+20%");
        assert_eq!(signed_axis(-0.015), "−1.5%");
    }

    #[test]
    fn significance_needs_an_interval_clear_of_zero() {
        assert!(VariantResult::new(1., 0.06, 0.01, 0.11, 0.97).significant());
        assert!(!VariantResult::new(1., 0.03, -0.01, 0.08, 0.8).significant());
        assert!(VariantResult::new(1., -0.06, -0.1, -0.02, 0.03).significant());
    }

    #[test]
    fn scale_rounds_up_to_a_nice_step() {
        assert!((nice_scale(0.12) - 0.2).abs() < 1e-6);
        assert!((nice_scale(0.08) - 0.1).abs() < 1e-6);
        assert!((nice_scale(0.) - 0.01).abs() < 1e-6);
    }

    #[test]
    fn visitors_group_by_thousands() {
        assert_eq!(grouped(48_210), "48,210");
        assert_eq!(grouped(980), "980");
    }
}
