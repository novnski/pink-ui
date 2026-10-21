//! Progress Tracker – an onboarding checklist with a progress bar.
//!
//! Clean: one white card with hairline shadows, quiet gray type, a thin blue
//! bar and blue checks. Classic: a bordered card with a thick ink bar and ink
//! checks.
//!
//! Checking a step fills its ring and pops it, then the tick draws itself
//! while a line strikes through the label; the bar grows with a light band
//! sweeping across it and the counter's digit rolls in from below. With
//! `auto_play` the open steps tick one after another and start over, until
//! someone clicks or presses a key inside the card.
//!
//! ```ignore
//! ProgressTracker::new("onboarding", vec![
//!     ProgressStep::new("tools", "Connect your tools"),
//!     ProgressStep::new("project", "Create a project").done(true),
//! ])
//! .auto_play(true)
//! .on_change(|steps, _, _| println!("{} done", steps.iter().filter(|s| s.done).count()))
//! ```

use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyView, App, BoxShadow, Context, ElementId, FocusHandle, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseDownEvent, SharedString, Task, Window, div, linear_color_stop,
    linear_gradient, prelude::*, px, relative, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, AnimationExt, EASE, EASE_IN, EASE_OUT, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, ring, shadow, white_a},
    story,
};

pub const NAME: &str = "Progress Tracker";
pub const DESCRIPTION: &str =
    "Onboarding checklist with an animated progress bar, ticks that draw themselves and auto play";

type ChangeHandler = Rc<dyn Fn(&[ProgressStep], &mut Window, &mut App)>;

/// One step of the checklist.
#[derive(Clone)]
pub struct ProgressStep {
    pub id: SharedString,
    pub label: SharedString,
    pub done: bool,
    /// Small icon on the right (an SVG path under `assets/`), e.g. the app
    /// the step belongs to.
    pub icon: Option<SharedString>,
}

impl ProgressStep {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            done: false,
            icon: None,
        }
    }

    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }
}

/// The `--ui-pgc-*` / `--ui-pg-*` tokens.
struct Palette {
    card: Hsla,
    border: Option<Hsla>,
    shadow: Vec<BoxShadow>,
    title: Hsla,
    ink: Hsla,
    /// The counter's changing digit.
    digit: Hsla,
    muted: Hsla,
    /// Clean strikes in a faint gray; Classic in the label's own color.
    strike: Option<Hsla>,
    icon: Hsla,
    ring: Hsla,
    track: Hsla,
    hover: Hsla,
    accent: Hsla,
    on_accent: Hsla,
    shine: Hsla,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => Palette {
                card: hex(0xffffff),
                border: None,
                shadow: vec![
                    ring(0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                title: hex(0x000000),
                ink: black_a(0.88),
                digit: black_a(0.88),
                muted: black_a(0.45),
                strike: Some(black_a(0.27)),
                icon: black_a(0.447),
                ring: black_a(0.2),
                track: black_a(0.06),
                hover: black_a(0.03),
                accent: hexa(0x0060e2, 0.765),
                on_accent: hex(0xffffff),
                shine: white_a(0.82),
                focus: hexa(0x0060e2, 0.35),
            },
            (Look::Clean, true) => Palette {
                card: hex(0x1c1c1e),
                border: None,
                shadow: vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                title: hex(0xffffff),
                ink: white_a(0.9),
                digit: white_a(0.9),
                muted: white_a(0.46),
                strike: Some(white_a(0.3)),
                icon: white_a(0.5),
                ring: white_a(0.24),
                track: white_a(0.08),
                hover: white_a(0.04),
                accent: hex(0x4d8dff),
                on_accent: hex(0xffffff),
                shine: hex(0xffffff),
                focus: hexa(0x4d8dff, 0.5),
            },
            (Look::Classic, false) => Palette {
                card: hex(0xffffff),
                border: Some(hex(0xe5e5e5)),
                shadow: vec![
                    shadow(0., 1., 2., 0., black_a(0.03)),
                    shadow(0., 8., 24., 0., black_a(0.035)),
                ],
                title: hex(0x171717),
                ink: hex(0x171717),
                digit: hex(0xa3a3a3),
                muted: hex(0xa3a3a3),
                strike: None,
                icon: hex(0x171717),
                ring: hex(0x8f8f8f),
                track: hex(0xf0f0f0),
                hover: hex(0xf7f7f7),
                accent: hex(0x171717),
                on_accent: hex(0xffffff),
                shine: white_a(0.28),
                focus: hexa(0x171717, 0.22),
            },
            (Look::Classic, true) => Palette {
                card: hex(0x232326),
                border: Some(white_a(0.08)),
                shadow: vec![
                    shadow(0., 1., 2., 0., black_a(0.4)),
                    shadow(0., 8., 24., 0., black_a(0.35)),
                ],
                title: hex(0xededed),
                ink: hex(0xededed),
                digit: hex(0x8f8f8f),
                muted: hex(0x8f8f8f),
                strike: None,
                icon: hex(0xededed),
                ring: hex(0x6b6b70),
                track: white_a(0.1),
                hover: hex(0x2a2a2d),
                accent: hex(0xededed),
                on_accent: hex(0x121213),
                shine: white_a(0.7),
                focus: hexa(0xededed, 0.3),
            },
        }
    }
}

/// Sizes from the two stylesheets.
struct Metrics {
    max_w: f32,
    pad_t: f32,
    pad_x: f32,
    pad_b: f32,
    radius: f32,
    head_pad: f32,
    title_size: f32,
    title_line: f32,
    count_size: f32,
    count_line: f32,
    bar_h: f32,
    bar_t: f32,
    bar_x: f32,
    bar_b: f32,
    list_gap: f32,
    step_py: f32,
    step_px: f32,
    step_radius: f32,
    step_gap: f32,
    step_size: f32,
    step_line: f32,
    check: f32,
    check_border: f32,
    tick: f32,
    strike: f32,
    strike_at: f32,
    icon: f32,
    pop_peak: f32,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                max_w: 340.,
                pad_t: 14.,
                pad_x: 6.,
                pad_b: 6.,
                radius: 10.,
                head_pad: 10.,
                title_size: 15.,
                title_line: 24.,
                count_size: 12.,
                count_line: 16.,
                bar_h: 4.,
                bar_t: 10.,
                bar_x: 10.,
                bar_b: 8.,
                list_gap: 0.,
                step_py: 6.,
                step_px: 10.,
                step_radius: 6.,
                step_gap: 10.,
                step_size: 13.,
                step_line: 20.,
                check: 16.,
                check_border: 1.,
                tick: 12.,
                strike: 1.,
                strike_at: 0.6,
                icon: 16.,
                pop_peak: 1.08,
            },
            Look::Classic => Metrics {
                max_w: 360.,
                pad_t: 22.,
                pad_x: 16.,
                pad_b: 14.,
                radius: 20.,
                head_pad: 6.,
                title_size: 16.,
                // `line-height: normal` for Inter
                title_line: 16. * NORMAL_LINE,
                count_size: 14.,
                count_line: 14. * NORMAL_LINE,
                bar_h: 8.,
                bar_t: 16.,
                bar_x: 6.,
                bar_b: 12.,
                list_gap: 2.,
                step_py: 9.,
                step_px: 6.,
                step_radius: 12.,
                step_gap: 12.,
                step_size: 15.,
                step_line: 15. * 1.45,
                check: 20.,
                check_border: 1.5,
                tick: 14.,
                strike: 1.5,
                strike_at: 0.58,
                icon: 18.,
                pop_peak: 1.06,
            },
        }
    }
}

/// Inter's ascender and descender (hhea), in em.
const ASCENT: f32 = 0.96875;
const DESCENT: f32 = 0.2421875;
const NORMAL_LINE: f32 = ASCENT + DESCENT;

/// Distance from the top of a line box to the text baseline.
fn baseline(size: f32, line: f32) -> f32 {
    (line - size * NORMAL_LINE) / 2. + size * ASCENT
}

/// Where a `background-position-y: <at>` line of `thick` px sits in an inline
/// span: inline backgrounds cover the font's content area, not the line box.
fn strike_top(size: f32, line: f32, thick: f32, at: f32) -> f32 {
    let content = size * NORMAL_LINE;
    (line - content) / 2. + (content - thick) * at
}

// ---------------------------------------------------------------------------
// Motion
// ---------------------------------------------------------------------------

const CHECK_FILL: MotionSpec = MotionSpec::new(220, EASE_OUT_QUINT);
const TICK_IN: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT).with_delay(160);
const TICK_OUT: MotionSpec = MotionSpec::new(120, EASE_IN);
const LABEL_FADE: MotionSpec = MotionSpec::new(300, EASE);
const STRIKE_IN: MotionSpec = MotionSpec::new(620, EASE_OUT_QUINT).with_delay(220);
const STRIKE_OUT: MotionSpec = MotionSpec::new(380, EASE_OUT_QUINT);
const BAR: MotionSpec = MotionSpec::new(1100, EASE_OUT_QUINT);
const POP: Duration = Duration::from_millis(560);
const SHINE: Duration = Duration::from_millis(1400);
const ROLL: MotionSpec = MotionSpec::new(560, EASE_OUT_QUINT);
const CARD_IN: MotionSpec = MotionSpec::new(520, EASE_OUT_QUINT);

/// A CSS transition with a delay: eases from wherever it was when the target
/// changed. (`motion::follow` has no delay, which the tick and strike need.)
#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    spec: MotionSpec,
}

impl Tween {
    fn settled(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: Instant::now(),
            spec: MotionSpec::new(0, EASE),
        }
    }

    fn total(&self) -> Duration {
        self.spec.total().mul_f32(motion::speed_scale())
    }

    fn value(&self, now: Instant) -> f32 {
        let total = self.total();
        let elapsed = now.saturating_duration_since(self.start);
        if total.is_zero() || elapsed >= total {
            return self.to;
        }
        let raw = elapsed.as_secs_f32() / total.as_secs_f32();
        motion::lerp(self.from, self.to, self.spec.progress(raw))
    }

    fn moving(&self, now: Instant) -> bool {
        self.from != self.to && now.saturating_duration_since(self.start) < self.total()
    }

    fn set(&mut self, to: f32, spec: MotionSpec, reduced: bool, now: Instant) {
        if self.to == to {
            return;
        }
        *self = if reduced {
            Self::settled(to)
        } else {
            Self {
                from: self.value(now),
                to,
                start: now,
                spec,
            }
        };
    }
}

/// Raw 0..1 progress of a one-shot keyframe animation started at `at`.
fn keyframe(at: Option<Instant>, length: Duration, now: Instant) -> Option<f32> {
    let length = length.mul_f32(motion::speed_scale());
    let elapsed = now.saturating_duration_since(at?);
    (elapsed < length).then(|| elapsed.as_secs_f32() / length.as_secs_f32())
}

/// `ui-progress-pop`: 0.9 → peak at 60% → 1, eased per segment.
fn pop_scale(t: f32, peak: f32) -> f32 {
    if t < 0.6 {
        motion::lerp(0.9, peak, EASE_OUT_QUINT.eval(t / 0.6))
    } else {
        motion::lerp(peak, 1., EASE_OUT_QUINT.eval((t - 0.6) / 0.4))
    }
}

/// How far (in the 24-unit viewBox) the tick `M6.5 12.5l3.5 3.5 7.5-8` has
/// been drawn at `progress` of its length. The path only ever moves right,
/// so revealing it left to right stands in for `stroke-dashoffset`.
fn tick_reach(progress: f32, stroke: f32) -> f32 {
    if progress <= 0. {
        return 0.;
    }
    if progress >= 1. {
        return 24.;
    }
    let first = (3.5f32 * 3.5 * 2.).sqrt();
    let second = (7.5f32 * 7.5 + 8. * 8.).sqrt();
    let along = progress * (first + second);
    let x = if along <= first {
        6.5 + 3.5 * along / first
    } else {
        10. + 7.5 * (along - first) / second
    };
    x + stroke / 2.
}

struct StepMotion {
    fill: Tween,
    tick: Tween,
    label: Tween,
    strike: Tween,
    popped: Option<Instant>,
}

impl StepMotion {
    fn new(done: bool, now: Instant) -> Self {
        let v = if done { 1. } else { 0. };
        Self {
            fill: Tween::settled(v),
            tick: Tween::settled(v),
            label: Tween::settled(v),
            strike: Tween::settled(v),
            // The pop is a keyframe animation, so it also plays on mount
            popped: done.then_some(now),
        }
    }

    fn set(&mut self, done: bool, reduced: bool, now: Instant) {
        let v = if done { 1. } else { 0. };
        self.fill.set(v, CHECK_FILL, reduced, now);
        self.tick
            .set(v, if done { TICK_IN } else { TICK_OUT }, reduced, now);
        self.label.set(v, LABEL_FADE, reduced, now);
        self.strike
            .set(v, if done { STRIKE_IN } else { STRIKE_OUT }, reduced, now);
        if done && !reduced {
            self.popped = Some(now);
        }
    }

    fn moving(&self, now: Instant) -> bool {
        self.fill.moving(now)
            || self.tick.moving(now)
            || self.label.moving(now)
            || self.strike.moving(now)
            || keyframe(self.popped, POP, now).is_some()
    }
}

struct State {
    steps: Vec<ProgressStep>,
    done: Vec<bool>,
    motion: Vec<StepMotion>,
    playing: bool,
    shine: Option<Instant>,
    reduced: bool,
    on_change: Option<ChangeHandler>,
    focus: Vec<FocusHandle>,
    /// Focus came from the keyboard, so the focus ring shows.
    keyboard: bool,
    _play: Option<Task<()>>,
}

impl State {
    fn new(
        steps: Vec<ProgressStep>,
        auto_play: bool,
        interval: Duration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let now = Instant::now();
        let done: Vec<bool> = steps.iter().map(|s| s.done).collect();
        let play = auto_play.then(|| Self::play(interval, window, cx));
        Self {
            motion: done.iter().map(|&d| StepMotion::new(d, now)).collect(),
            focus: steps.iter().map(|_| cx.focus_handle()).collect(),
            steps,
            done,
            playing: auto_play,
            shine: Some(now),
            reduced: motion::reduced_motion(cx),
            on_change: None,
            keyboard: false,
            _play: play,
        }
    }

    /// Ticks the next open step every `interval`; when all are done, waits
    /// twice as long and starts over.
    fn play(interval: Duration, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            loop {
                let wait = this.update(cx, |s, _| {
                    s.playing.then(|| {
                        if s.done.iter().all(|d| *d) {
                            interval * 2
                        } else {
                            interval
                        }
                    })
                });
                let Ok(Some(wait)) = wait else { break };
                cx.background_executor().timer(wait).await;
                let changed = this.update(cx, |s, cx| {
                    if !s.playing {
                        return None;
                    }
                    match s.done.iter().position(|d| !d) {
                        Some(ix) => s.toggle(ix),
                        None => s.reset(),
                    }
                    cx.notify();
                    Some(s.report())
                });
                let Ok(Some((handler, steps))) = changed else {
                    break;
                };
                if let Some(handler) = handler {
                    let _ = cx.update(|window, cx| handler(&steps, window, cx));
                }
            }
        })
    }

    fn toggle(&mut self, ix: usize) {
        let now = Instant::now();
        self.done[ix] = !self.done[ix];
        self.motion[ix].set(self.done[ix], self.reduced, now);
        self.shine = Some(now);
    }

    /// Back to the steps as they were passed in.
    fn reset(&mut self) {
        let now = Instant::now();
        for (ix, step) in self.steps.iter().enumerate() {
            if self.done[ix] != step.done {
                self.done[ix] = step.done;
                self.motion[ix].set(step.done, self.reduced, now);
            }
        }
        self.shine = Some(now);
    }

    /// The change handler and the steps with their current state.
    fn report(&self) -> (Option<ChangeHandler>, Vec<ProgressStep>) {
        let steps = self
            .steps
            .iter()
            .zip(&self.done)
            .map(|(s, &done)| ProgressStep { done, ..s.clone() })
            .collect();
        (self.on_change.clone(), steps)
    }

    /// Steps passed on this render: keep the state of the ones we know.
    fn sync(&mut self, steps: &[ProgressStep], cx: &mut Context<Self>) {
        let same = steps.len() == self.steps.len()
            && steps.iter().zip(&self.steps).all(|(a, b)| a.id == b.id);
        if !same {
            let now = Instant::now();
            let old: Vec<(SharedString, bool)> = self
                .steps
                .iter()
                .map(|s| s.id.clone())
                .zip(self.done.iter().copied())
                .collect();
            self.done = steps
                .iter()
                .map(|s| {
                    old.iter()
                        .find(|(id, _)| *id == s.id)
                        .map_or(s.done, |(_, d)| *d)
                })
                .collect();
            self.motion = self.done.iter().map(|&d| StepMotion::new(d, now)).collect();
            self.focus = steps.iter().map(|_| cx.focus_handle()).collect();
        }
        self.steps = steps.to_vec();
    }
}

#[derive(IntoElement)]
pub struct ProgressTracker {
    id: ElementId,
    title: SharedString,
    steps: Vec<ProgressStep>,
    auto_play: bool,
    interval: Duration,
    on_change: Option<ChangeHandler>,
    look: Option<Look>,
}

impl ProgressTracker {
    pub fn new(id: impl Into<ElementId>, steps: Vec<ProgressStep>) -> Self {
        Self {
            id: id.into(),
            title: "Getting started".into(),
            steps,
            auto_play: false,
            interval: Duration::from_millis(2200),
            on_change: None,
            look: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Ticks the open steps one after another, then starts over – stops as
    /// soon as someone interacts.
    pub fn auto_play(mut self, auto_play: bool) -> Self {
        self.auto_play = auto_play;
        self
    }

    /// Pause between two automatic steps (default 2.2s).
    pub fn interval(mut self, interval: Duration) -> Self {
        self.interval = interval;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&[ProgressStep], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Overrides the global Clean/Classic look for this tracker.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for ProgressTracker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx);
        let m = Metrics::get(look);
        let key = format!("pt-{}", self.id);
        let (steps, auto_play, interval) = (self.steps.clone(), self.auto_play, self.interval);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| State::new(steps, auto_play, interval, window, cx),
        );
        let reduced = motion::reduced_motion(cx);
        state.update(cx, |s, cx| {
            s.sync(&self.steps, cx);
            s.on_change = self.on_change.clone();
            s.reduced = reduced;
        });

        let now = Instant::now();
        let s = state.read(cx);
        let total = s.done.len();
        let count = s.done.iter().filter(|d| **d).count();
        let progress = if total > 0 {
            count as f32 / total as f32
        } else {
            0.
        };
        let mut moving = s.motion.iter().any(|m| m.moving(now));
        let shine = keyframe(s.shine, SHINE, now);
        moving |= shine.is_some();
        let keyboard = s.keyboard;
        let handles = s.focus.clone();
        let rows: Vec<_> = s
            .steps
            .iter()
            .enumerate()
            .map(|(ix, step)| {
                let motion = &s.motion[ix];
                let values = StepValues {
                    fill: motion.fill.value(now),
                    tick: motion.tick.value(now),
                    label: motion.label.value(now),
                    strike: motion.strike.value(now),
                    pop: keyframe(motion.popped, POP, now).map_or(1., |t| pop_scale(t, m.pop_peak)),
                };
                (ix, step.clone(), values)
            })
            .collect();

        let rows: Vec<_> = rows
            .into_iter()
            .map(|(ix, step, v)| {
                let focused = keyboard && handles[ix].is_focused(window);
                let toggle = {
                    let state = state.clone();
                    move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                        let (handler, steps) = state.update(cx, |s, cx| {
                            s.toggle(ix);
                            cx.notify();
                            s.report()
                        });
                        if let Some(handler) = handler {
                            handler(&steps, window, cx);
                        }
                    }
                };
                let row = step_row(&key, ix, &step, v, look, &m, &p, focused)
                    .track_focus(&handles[ix])
                    .on_click(toggle);
                // The rows fade up one after another
                motion::rise_in(
                    ("row-in", ix),
                    MotionSpec::new(640, EASE_OUT_QUINT).with_delay(180 + ix as u64 * 90),
                    8.,
                    div().w_full().child(row),
                )
            })
            .collect();

        // Header: the title and a quiet counter whose digit rolls in
        let roll = m.count_line * 0.6;
        let digit = div()
            .text_color(p.digit)
            .child(count.to_string())
            .with_animation(("digit", count), ROLL.animation(), move |el, t| {
                el.relative().top(px(roll * (1. - t))).opacity(t)
            });
        let head = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(12.))
            .px(px(m.head_pad))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(px(m.title_size))
                    .line_height(px(m.title_line))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.title)
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .mt(px(
                        baseline(m.title_size, m.title_line) - baseline(m.count_size, m.count_line)
                    ))
                    .whitespace_nowrap()
                    .text_size(px(m.count_size))
                    .line_height(px(m.count_line))
                    .text_color(p.muted)
                    .child(digit)
                    .child(format!("/{total}")),
            );

        // The bar: the fill glides, and a light band sweeps across it
        let fill = motion::follow(&format!("{key}-fill"), progress, BAR);
        let bar_r = (m.bar_h / 2.).min(if clean { 2. } else { 999. });
        let band = shine.map(|t| {
            let e = EASE_OUT.eval(t);
            // `background-position: 120% → -20%` of a 220% wide gradient
            let center = -0.34 + 1.68 * e;
            let half = 0.44;
            let clear = p.shine.opacity(0.);
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .opacity(1. - e)
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .h_full()
                        .left(relative(center - half))
                        .w(relative(half))
                        .bg(linear_gradient(
                            90.,
                            linear_color_stop(clear, 0.),
                            linear_color_stop(p.shine, 1.),
                        )),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .h_full()
                        .left(relative(center))
                        .w(relative(half))
                        .bg(linear_gradient(
                            90.,
                            linear_color_stop(p.shine, 0.),
                            linear_color_stop(clear, 1.),
                        )),
                )
        });
        let bar = div()
            .relative()
            .h(px(m.bar_h))
            .mt(px(m.bar_t))
            .mb(px(m.bar_b))
            .mx(px(m.bar_x))
            .rounded(px(bar_r))
            .bg(p.track)
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(relative(fill))
                    .rounded_l(px(bar_r))
                    // `scaleX` squeezes the round end along with the bar
                    .rounded_r(px(bar_r * fill.min(1.)))
                    .bg(p.accent)
                    .overflow_hidden()
                    .children(band),
            );

        // Any press or key inside stops the auto play
        let stop_mouse = {
            let state = state.clone();
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, _| {
                    s.playing = false;
                    s._play = None;
                    s.keyboard = false;
                });
            }
        };
        let stop_key = {
            let state = state.clone();
            move |_: &KeyDownEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| {
                    s.playing = false;
                    s._play = None;
                    s.keyboard = true;
                    cx.notify();
                });
            }
        };
        let on_nav = {
            let handles = handles.clone();
            move |e: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let len = handles.len();
                if len == 0 || e.keystroke.modifiers.modified() {
                    return;
                }
                let current = handles.iter().position(|h| h.is_focused(window));
                let next = match e.keystroke.key.as_str() {
                    "down" => current.map_or(0, |i| (i + 1).min(len - 1)),
                    "up" => current.map_or(len - 1, |i| i.saturating_sub(1)),
                    "home" => 0,
                    "end" => len - 1,
                    _ => return,
                };
                window.focus(&handles[next]);
                cx.stop_propagation();
            }
        };

        let card = div()
            .id(self.id.clone())
            .w_full()
            .max_w(px(m.max_w))
            .pt(px(m.pad_t))
            .px(px(m.pad_x))
            .pb(px(m.pad_b))
            .bg(p.card)
            .rounded(px(m.radius))
            .shadow(p.shadow.clone())
            .when_some(p.border, |d, border| d.border_1().border_color(border))
            .font_family(SANS)
            .text_size(px(m.step_size))
            .line_height(px(m.step_line))
            .text_color(p.ink)
            .on_any_mouse_down(stop_mouse)
            .capture_key_down(stop_key)
            .on_key_down(on_nav)
            .child(head)
            .child(bar)
            .child(div().flex().flex_col().gap(px(m.list_gap)).children(rows));

        if moving {
            window.request_animation_frame();
        }
        motion::keep_alive(window);
        // On mount the card settles in
        motion::rise_in(
            ElementId::NamedChild(Box::new(self.id), "in".into()),
            CARD_IN,
            8.,
            card,
        )
    }
}

/// A step's animated values this frame.
#[derive(Clone, Copy)]
struct StepValues {
    /// Ring → filled check.
    fill: f32,
    /// How much of the tick is drawn.
    tick: f32,
    /// Label color, ink → muted (and the icon's fade).
    label: f32,
    /// How far the line has struck through the label.
    strike: f32,
    /// The check's pop scale.
    pop: f32,
}

#[allow(clippy::too_many_arguments)]
fn step_row(
    key: &str,
    ix: usize,
    step: &ProgressStep,
    v: StepValues,
    look: Look,
    m: &Metrics,
    p: &Palette,
    focused: bool,
) -> gpui::Stateful<gpui::Div> {
    let clean = look == Look::Clean;

    // Checkbox: the ring fills and pops, then the tick draws itself
    let size = m.check * v.pop;
    let inset = (m.check - size) / 2.;
    let tick = m.tick * v.pop;
    let stroke = if clean { 2.2 } else { 2.4 };
    let reach = tick_reach(v.tick, stroke) / 24. * tick;
    let border = if clean {
        motion::mix(p.ring, p.ring.opacity(0.), v.fill)
    } else {
        motion::mix(p.ring, p.accent, v.fill)
    };
    let check = div().relative().flex_none().size(px(m.check)).child(
        div()
            .absolute()
            .left(px(inset))
            .top(px(inset))
            .size(px(size))
            .rounded_full()
            .border(px(m.check_border))
            .border_color(border)
            .bg(motion::mix(p.accent.opacity(0.), p.accent, v.fill))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div().relative().flex_none().size(px(tick)).child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .h_full()
                        .w(px(reach))
                        .overflow_hidden()
                        .child(
                            svg()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size(px(tick))
                                .path(if clean {
                                    "icons/progress-tracker/clean/check.svg"
                                } else {
                                    "icons/progress-tracker/check.svg"
                                })
                                .text_color(p.on_accent),
                        ),
                ),
            ),
    );

    // Label: grays out while a thin line strikes through it
    let color = motion::mix(p.ink, p.muted, v.label);
    let label = div()
        .flex()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .text_color(color)
        .child(
            div()
                .relative()
                .flex_none()
                .whitespace_nowrap()
                .child(step.label.clone())
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(strike_top(
                            m.step_size,
                            m.step_line,
                            m.strike,
                            m.strike_at,
                        )))
                        .h(px(m.strike))
                        .w(relative(v.strike))
                        .bg(p.strike.unwrap_or(color)),
                ),
        );

    let icon = step.icon.clone().map(|path| {
        svg()
            .path(path)
            .flex_none()
            .size(px(m.icon))
            .text_color(p.icon)
            .opacity(motion::lerp(1., 0.45, v.label))
    });

    div()
        .id(("step", ix))
        .relative()
        .flex()
        .items_center()
        .gap(px(m.step_gap))
        .w_full()
        .px(px(m.step_px))
        .py(px(m.step_py))
        .rounded(px(m.step_radius))
        .cursor_pointer()
        .hover_fill(
            format!("{key}-step-{}", step.id),
            p.hover.opacity(0.),
            p.hover,
        )
        .child(check)
        .child(label)
        .children(icon)
        .when(focused, |d| {
            d.child(
                div()
                    .absolute()
                    .top(px(-2.))
                    .left(px(-2.))
                    .right(px(-2.))
                    .bottom(px(-2.))
                    .border(px(2.))
                    .border_color(p.focus)
                    .rounded(px(m.step_radius + 2.)),
            )
        })
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        ProgressTracker::new(
            "progress-tracker",
            vec![
                ProgressStep::new("tools", "Connect your tools"),
                ProgressStep::new("notify", "Turn on notifications"),
                ProgressStep::new("updates", "Catch up on blockers"),
                ProgressStep::new("priorities", "Summarize priorities"),
                ProgressStep::new("project", "Create a project").done(true),
            ],
        )
        .auto_play(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_reveal_runs_left_to_right() {
        assert_eq!(tick_reach(0., 2.), 0.);
        assert_eq!(tick_reach(1., 2.), 24.);
        let mut last = 0.;
        for i in 1..100 {
            let x = tick_reach(i as f32 / 100., 2.);
            assert!(x >= last);
            last = x;
        }
        // The corner of the tick is about a third of the way along
        assert!((tick_reach(0.311, 0.) - 10.).abs() < 0.05);
    }

    #[test]
    fn pop_overshoots_then_settles() {
        assert!((pop_scale(0., 1.08) - 0.9).abs() < 1e-4);
        assert!((pop_scale(0.6, 1.08) - 1.08).abs() < 1e-4);
        assert!((pop_scale(1., 1.08) - 1.).abs() < 1e-4);
    }

    #[test]
    fn tween_holds_through_its_delay() {
        let now = Instant::now();
        let mut t = Tween::settled(0.);
        t.set(1., TICK_IN, false, now);
        assert_eq!(t.value(now + Duration::from_millis(100)), 0.);
        assert!(t.value(now + Duration::from_millis(300)) > 0.);
        assert_eq!(t.value(now + Duration::from_millis(2000)), 1.);
    }
}
