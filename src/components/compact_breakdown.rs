//! Compact Breakdown – a total and its parts in one small card.
//!
//! A ring shows the split, ranked rows list each part with its value and
//! share, and the footer names the fastest-growing part. Switching the
//! period makes everything move together: the segments glide around the
//! ring, the numbers roll to their new values and the rows slide past each
//! other into the new ranking. Hovering a row or a segment isolates that
//! part and the ring's center reads it out. With the card focused, ←/→
//! switch periods and ↑/↓ step through the parts.
//!
//! ```ignore
//! CompactBreakdown::new("tokens", "Tokens by model")
//!     .part(BreakdownPart::new("chat", "Chat"))
//!     .period(BreakdownPeriod::new("7d", vec![352e6]).previous(vec![331e6]))
//! ```

use std::sync::Arc;
use std::{cell::Cell, f32::consts::TAU, rc::Rc};

use gpui::{
    AnyView, App, Bounds, ElementId, FocusHandle, FontWeight, Hsla, KeyDownEvent, MouseButton,
    MouseMoveEvent, PathBuilder, Pixels, Point, SharedString, Window, canvas, div, point,
    prelude::*, px,
};

use crate::{
    luna::{self, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, MotionSpec},
    story,
};

pub const NAME: &str = "Compact Breakdown";
pub const DESCRIPTION: &str = "A total split into ranked parts around a ring; switching the period glides the segments, rolls the numbers and reorders the rows";

const RING: f32 = 120.;
const STROKE: f32 = 14.;
const ROW_H: f32 = 30.;
/// Gap between ring segments, in radians.
const GAP: f32 = 0.045;
/// Values, segments and rows moving to a new period.
const SETTLE: MotionSpec = MotionSpec::new(640, motion::EASE_OUT_QUINT);
/// The ring sweeping in on first render.
const SWEEP_IN: MotionSpec = MotionSpec::new(1000, motion::EASE_OUT_QUINT).with_delay(100);
const DIM: MotionSpec = MotionSpec::new(180, motion::EASE);

type PeriodHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type Formatter = Rc<dyn Fn(f64) -> String>;

/// One slice of the total.
#[derive(Clone)]
pub struct BreakdownPart {
    pub id: SharedString,
    pub label: SharedString,
    /// Overrides the palette color.
    pub color: Option<Hsla>,
}

impl BreakdownPart {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            color: None,
        }
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }
}

/// The parts' values over one period, in the order the parts were added.
#[derive(Clone)]
pub struct BreakdownPeriod {
    pub label: SharedString,
    pub values: Vec<f64>,
    /// The same parts one period earlier, for the growth readouts.
    pub previous: Option<Vec<f64>>,
}

impl BreakdownPeriod {
    pub fn new(label: impl Into<SharedString>, values: Vec<f64>) -> Self {
        Self {
            label: label.into(),
            values,
            previous: None,
        }
    }

    pub fn previous(mut self, values: Vec<f64>) -> Self {
        self.previous = Some(values);
        self
    }
}

#[derive(IntoElement)]
pub struct CompactBreakdown {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    unit: SharedString,
    parts: Vec<BreakdownPart>,
    periods: Vec<BreakdownPeriod>,
    default_period: usize,
    format: Formatter,
    on_period_change: Option<PeriodHandler>,
}

impl CompactBreakdown {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            unit: "total".into(),
            parts: Vec::new(),
            periods: Vec::new(),
            default_period: 0,
            format: Rc::new(short_number),
            on_period_change: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// The word under the total in the ring, e.g. "tokens".
    pub fn unit(mut self, unit: impl Into<SharedString>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn part(mut self, part: BreakdownPart) -> Self {
        self.parts.push(part);
        self
    }

    pub fn period(mut self, period: BreakdownPeriod) -> Self {
        self.periods.push(period);
        self
    }

    pub fn default_period(mut self, ix: usize) -> Self {
        self.default_period = ix;
        self
    }

    /// Formats values (default: "61.2M", "1.46B").
    pub fn format(mut self, format: impl Fn(f64) -> String + 'static) -> Self {
        self.format = Rc::new(format);
        self
    }

    pub fn on_period_change(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_period_change = Some(Rc::new(handler));
        self
    }
}

struct State {
    period: usize,
    /// Part index under the pointer or the keyboard highlight.
    active: Option<usize>,
    ring: Rc<Cell<Bounds<Pixels>>>,
    focus: FocusHandle,
}

fn palette(t: &Luna) -> [Hsla; 5] {
    let amber = if t.dark {
        t.warning
    } else {
        luna::oklch(0.70, 0.165, 62.)
    };
    // A rose a little softer than luna's danger, so no part reads as an error.
    let rose = if t.dark {
        luna::oklch(0.70, 0.17, 5.)
    } else {
        luna::oklch(0.63, 0.20, 8.)
    };
    [t.accent, t.success, amber, rose, luna::neutral(0.62)]
}

impl RenderOnce for CompactBreakdown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let default_period = self.default_period;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| State {
                period: default_period,
                active: None,
                ring: Rc::new(Cell::new(Bounds::default())),
                focus: cx.focus_handle(),
            },
        );
        if self.periods.is_empty() || self.parts.is_empty() {
            return luna::card(&t).id(self.id.clone());
        }
        let (period_ix, active, ring_cell, focus) = {
            let s = state.read(cx);
            (
                s.period.min(self.periods.len() - 1),
                s.active,
                s.ring.clone(),
                s.focus.clone(),
            )
        };
        let period = &self.periods[period_ix];
        let count = self.parts.len();
        let colors = palette(&t);
        let color_of = |i: usize| self.parts[i].color.unwrap_or(colors[i % colors.len()]);
        let value_of = |i: usize| period.values.get(i).copied().unwrap_or(0.).max(0.);
        let total: f64 = (0..count).map(value_of).sum();
        let share_of = |i: usize| {
            if total > 0. { value_of(i) / total } else { 0. }
        };
        let growth_of = |i: usize| {
            period
                .previous
                .as_ref()
                .and_then(|p| p.get(i).copied())
                .filter(|p| *p > 0.)
                .map(|p| value_of(i) / p - 1.)
        };

        // Rank by value, largest first.
        let mut order: Vec<usize> = (0..count).collect();
        order.sort_by(|a, b| value_of(*b).total_cmp(&value_of(*a)));
        let rank_of = |i: usize| order.iter().position(|o| *o == i).unwrap_or(0);

        // Segment geometry follows the ranking, so parts slide around the ring.
        let sweep_in = motion::follow_from(&format!("{key}-sweep"), 0., 1., SWEEP_IN);
        let mut start = 0.;
        let mut segments = vec![(0f32, 0f32); count];
        for &i in &order {
            let share = share_of(i) as f32;
            segments[i] = (start, share);
            start += share;
        }
        let segments: Vec<Segment> = (0..count)
            .map(|i| {
                let (s, len) = segments[i];
                let s = motion::follow(&format!("{key}-seg-start-{}", self.parts[i].id), s, SETTLE);
                let len =
                    motion::follow(&format!("{key}-seg-len-{}", self.parts[i].id), len, SETTLE);
                let lit = active.is_none_or(|a| a == i);
                Segment {
                    start: s * sweep_in,
                    len: len * sweep_in,
                    color: color_of(i),
                    opacity: motion::follow(
                        &format!("{key}-seg-op-{i}"),
                        if lit { 1. } else { 0.28 },
                        DIM,
                    ),
                    lifted: active == Some(i),
                }
            })
            .collect();

        // --- Header ---------------------------------------------------
        let tabs = self.periods.iter().enumerate().map(|(ix, p)| {
            let state = state.clone();
            let handler = self.on_period_change.clone();
            luna::chip(
                SharedString::from(format!("{key}-period-{ix}")),
                ix == period_ix,
                ControlSize::Sm,
                &t,
            )
            .font_family(luna::MONO)
            .child(p.label.clone())
            .on_click(move |_, window, cx| {
                set_period(&state, ix, cx);
                if let Some(f) = &handler {
                    f(ix, window, cx);
                }
            })
        });
        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(space::S12))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(text::CALLOUT))
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(self.title.clone()),
                    )
                    .when_some(self.subtitle.clone(), |d, sub| {
                        d.child(
                            div()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(sub),
                        )
                    }),
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

        // --- Ring -----------------------------------------------------
        let track = t.fill(FillTier::Secondary);
        let ring_canvas = canvas(
            {
                let ring_cell = ring_cell.clone();
                move |bounds, _, _| ring_cell.set(bounds)
            },
            move |bounds, _, window, _| {
                let c = bounds.center();
                let r_out = RING / 2.;
                // The faint track ahead of the segments while they sweep in.
                if sweep_in < 0.999 {
                    let a0 = sweep_in * TAU + GAP / 2.;
                    paint_arc(window, c, r_out, r_out - STROKE, a0, TAU - GAP / 2., track);
                }
                for seg in segments.iter().filter(|s| s.len > 0.002) {
                    let a0 = seg.start * TAU + GAP / 2.;
                    let a1 = (seg.start + seg.len) * TAU - GAP / 2.;
                    if a1 <= a0 {
                        continue;
                    }
                    let (outer, inner) = if seg.lifted {
                        (r_out + 1.5, r_out - STROKE - 1.5)
                    } else {
                        (r_out, r_out - STROKE)
                    };
                    paint_arc(
                        window,
                        c,
                        outer,
                        inner,
                        a0,
                        a1,
                        seg.color.opacity(seg.opacity),
                    );
                }
            },
        )
        .size_full();

        let center_value = match active {
            Some(i) => value_of(i),
            None => total,
        };
        let center_value =
            motion::follow(&format!("{key}-center"), center_value as f32, SETTLE) as f64;
        let center_caption: SharedString = match active {
            Some(i) => self.parts[i].label.clone(),
            None => self.unit.clone(),
        };
        let ring = {
            let move_state = state.clone();
            let leave_state = state.clone();
            let ring_cell = ring_cell.clone();
            let order = order.clone();
            let shares: Vec<f64> = (0..count).map(share_of).collect();
            div()
                .id(SharedString::from(format!("{key}-ring")))
                .relative()
                .flex_none()
                .size(px(RING + 6.))
                .child(div().absolute().inset_0().child(ring_canvas))
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_size(px(text::LARGE))
                                .line_height(px(24.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.text)
                                .child((self.format)(center_value)),
                        )
                        .child(
                            div()
                                .max_w(px(RING - STROKE * 2. - 8.))
                                .truncate()
                                .text_size(px(text::CAPTION))
                                .line_height(px(14.))
                                .text_color(t.text_faint)
                                .child(center_caption),
                        ),
                )
                .on_mouse_move(move |ev: &MouseMoveEvent, _, cx| {
                    let b = ring_cell.get();
                    let d = ev.position - b.center();
                    let (dx, dy) = (f32::from(d.x), f32::from(d.y));
                    let dist = (dx * dx + dy * dy).sqrt();
                    let hit = if dist > RING / 2. - STROKE - 4. && dist < RING / 2. + 4. {
                        let a = dx.atan2(-dy).rem_euclid(TAU) / TAU;
                        let mut acc = 0.;
                        order.iter().copied().find(|&i| {
                            acc += shares[i] as f32;
                            a <= acc
                        })
                    } else {
                        None
                    };
                    move_state.update(cx, |s, cx| {
                        if s.active != hit {
                            s.active = hit;
                            cx.notify();
                        }
                    });
                })
                .on_hover(move |hovered, _, cx| {
                    if !hovered {
                        clear_active(&leave_state, cx);
                    }
                })
        };

        // --- Ranked rows ---------------------------------------------
        let rows = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(ROW_H * count as f32))
            .children((0..count).map(|i| {
                let part = &self.parts[i];
                let rank = rank_of(i);
                let top = motion::follow(
                    &format!("{key}-row-top-{}", part.id),
                    rank as f32 * ROW_H,
                    SETTLE,
                );
                let shown = motion::follow(
                    &format!("{key}-row-val-{}", part.id),
                    value_of(i) as f32,
                    SETTLE,
                ) as f64;
                let share = motion::follow(
                    &format!("{key}-row-share-{}", part.id),
                    share_of(i) as f32,
                    SETTLE,
                );
                let lit = active.is_none_or(|a| a == i);
                let hover_key = format!("{key}-row-{}", part.id);
                let wash = if active == Some(i) {
                    t.fill(FillTier::Tertiary)
                } else {
                    t.fill(FillTier::Tertiary).opacity(0.)
                };
                let enter_state = state.clone();
                let row = div()
                    .id(SharedString::from(hover_key))
                    .h(px(ROW_H))
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .px(px(space::S8))
                    .rounded(px(radius::SM))
                    .bg(wash)
                    .on_hover(move |hovered, _, cx| {
                        if *hovered {
                            enter_state.update(cx, |s, cx| {
                                s.active = Some(i);
                                cx.notify();
                            });
                        }
                    })
                    .child(
                        div()
                            .flex_none()
                            .size(px(8.))
                            .rounded(px(2.))
                            .bg(color_of(i)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::BODY))
                            .text_color(t.text)
                            .child(part.label.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(56.))
                            .flex()
                            .justify_end()
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text)
                            .child((self.format)(shown)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(px(34.))
                            .flex()
                            .justify_end()
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(format!("{:.0}%", share * 100.)),
                    );
                // Dimming lives on the wrapper: the entrance animation owns the row's opacity.
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(top))
                    .opacity(motion::follow(
                        &format!("{key}-row-op-{i}"),
                        if lit { 1. } else { 0.45 },
                        DIM,
                    ))
                    .child(motion::rise_in(
                        SharedString::from(format!("{key}-row-in-{i}")),
                        motion::ROW_IN.with_delay(160 + rank as u64 * 55),
                        6.,
                        row,
                    ))
            }));
        let rows = {
            let leave_state = state.clone();
            div()
                .id(SharedString::from(format!("{key}-rows")))
                .flex_1()
                .min_w_0()
                .child(rows)
                .on_hover(move |hovered, _, cx| {
                    if !hovered {
                        clear_active(&leave_state, cx);
                    }
                })
        };

        // --- Footer: total growth and the fastest grower --------------
        let prev_total: Option<f64> = period.previous.as_ref().map(|p| p.iter().sum());
        let total_growth = prev_total.filter(|p| *p > 0.).map(|p| total / p - 1.);
        let leader = (0..count)
            .filter_map(|i| growth_of(i).map(|g| (i, g)))
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let footer = motion::rise_in(
            SharedString::from(format!("{key}-foot-{period_ix}")),
            motion::VIEW_IN,
            4.,
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(space::S8))
                .text_size(px(text::FOOTNOTE))
                .line_height(px(16.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .text_color(t.text_muted)
                        .when_some(leader, |d, (i, g)| {
                            d.child("Fastest growing")
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(space::S6))
                                        .text_color(t.text)
                                        .child(div().size(px(6.)).rounded(px(3.)).bg(color_of(i)))
                                        .child(self.parts[i].label.clone()),
                                )
                                .child(growth_badge(g, &t))
                        }),
                )
                .when_some(total_growth, |d, g| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S6))
                            .text_color(t.text_faint)
                            .child(growth_badge(g, &t))
                            .child("overall"),
                    )
                }),
        );

        // Followers started above need frames even when nothing else animates.
        motion::keep_alive(window);
        let key_state = state.clone();
        let periods = self.periods.len();
        let handler = self.on_period_change.clone();
        let focus_click = focus.clone();
        luna::card(&t)
            .id(self.id.clone())
            .w(px(420.))
            .font_family(luna::SANS)
            .track_focus(&focus)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus_click, cx)
            })
            .on_key_down(move |ev: &KeyDownEvent, window, cx| {
                let (period, active) = {
                    let s = key_state.read(cx);
                    (s.period, s.active)
                };
                match ev.keystroke.key.as_str() {
                    "left" | "right" => {
                        let next = if ev.keystroke.key == "left" {
                            period.saturating_sub(1)
                        } else {
                            (period + 1).min(periods - 1)
                        };
                        if next != period {
                            set_period(&key_state, next, cx);
                            if let Some(f) = &handler {
                                f(next, window, cx);
                            }
                        }
                    }
                    "up" | "down" => {
                        // Steps through the parts in their ranked order.
                        let pos = active.and_then(|a| order.iter().position(|o| *o == a));
                        let down = ev.keystroke.key == "down";
                        let next = match pos {
                            None if down => 0,
                            None => count - 1,
                            Some(p) if down => (p + 1).min(count - 1),
                            Some(p) => p.saturating_sub(1),
                        };
                        let part = order[next];
                        key_state.update(cx, |s, cx| {
                            s.active = Some(part);
                            cx.notify();
                        });
                    }
                    "escape" => clear_active(&key_state, cx),
                    _ => return,
                }
                cx.stop_propagation();
            })
            .child(div().px(px(space::S16)).pt(px(space::S16)).child(header))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S16))
                    .px(px(space::S16))
                    .py(px(space::S16))
                    .child(ring)
                    .child(rows),
            )
            .child(luna::divider(&t))
            .child(div().px(px(space::S16)).py(px(space::S12)).child(footer))
    }
}

#[derive(Clone)]
struct Segment {
    start: f32,
    len: f32,
    color: Hsla,
    opacity: f32,
    lifted: bool,
}

fn set_period(state: &gpui::Entity<State>, ix: usize, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.period != ix {
            s.period = ix;
            cx.notify();
        }
    });
}

fn clear_active(state: &gpui::Entity<State>, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.active.is_some() {
            s.active = None;
            cx.notify();
        }
    });
}

fn growth_badge(g: f64, t: &Luna) -> impl IntoElement {
    let tone = if g >= 0. { Tone::Success } else { Tone::Danger };
    luna::badge(
        format!("{}{:.0}%", if g >= 0. { "+" } else { "−" }, g.abs() * 100.),
        tone,
        t,
    )
    .font_family(luna::MONO)
}

/// A point on a circle, clockwise from 12 o'clock.
fn polar(c: Point<Pixels>, r: f32, a: f32) -> Point<Pixels> {
    c + point(px(r * a.sin()), px(-r * a.cos()))
}

/// An annular sector between `inner` and `outer` radii from angle `a0` to `a1`.
fn paint_arc(
    window: &mut Window,
    c: Point<Pixels>,
    outer: f32,
    inner: f32,
    a0: f32,
    a1: f32,
    color: Hsla,
) {
    let large = a1 - a0 > std::f32::consts::PI;
    let mut path = PathBuilder::fill();
    path.move_to(polar(c, outer, a0));
    path.arc_to(
        point(px(outer), px(outer)),
        px(0.),
        large,
        true,
        polar(c, outer, a1),
    );
    path.line_to(polar(c, inner, a1));
    path.arc_to(
        point(px(inner), px(inner)),
        px(0.),
        large,
        false,
        polar(c, inner, a0),
    );
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// 61_200_000 → "61.2M", 1_460_000_000 → "1.46B".
fn short_number(v: f64) -> String {
    let a = v.abs();
    if a >= 1e9 {
        format!("{:.2}B", v / 1e9)
    } else if a >= 1e8 {
        format!("{:.0}M", v / 1e6)
    } else if a >= 1e6 {
        trim(v / 1e6, "M")
    } else if a >= 1e3 {
        trim(v / 1e3, "k")
    } else {
        format!("{v:.0}")
    }
}

/// One decimal, dropped when it is zero: 61.0 → "61M", 9.8 → "9.8M".
fn trim(v: f64, unit: &str) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}{unit}", v.round())
    } else {
        format!("{v:.1}{unit}")
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        CompactBreakdown::new("compact-breakdown", "Tokens by model")
            .subtitle("Production · all workspaces")
            .unit("tokens")
            .part(BreakdownPart::new("reasoning", "Reasoning"))
            .part(BreakdownPart::new("chat", "Chat"))
            .part(BreakdownPart::new("embeddings", "Embeddings"))
            .part(BreakdownPart::new("vision", "Vision"))
            .part(BreakdownPart::new("speech", "Speech"))
            .period(
                BreakdownPeriod::new("24h", vec![61.2e6, 48.9e6, 27.3e6, 9.8e6, 4.1e6])
                    .previous(vec![52.0e6, 50.3e6, 25.1e6, 7.1e6, 4.4e6]),
            )
            .period(
                BreakdownPeriod::new("7d", vec![318e6, 352e6, 186e6, 61e6, 30e6])
                    .previous(vec![254e6, 331e6, 179e6, 44e6, 29e6]),
            )
            .period(
                BreakdownPeriod::new("30d", vec![0.88e9, 1.46e9, 0.92e9, 0.19e9, 0.21e9])
                    .previous(vec![0.61e9, 1.38e9, 0.85e9, 0.12e9, 0.20e9]),
            )
            .default_period(1)
    })
}
