//! Thinking Loader – A small working pill for AI replies – an eight-ray spinner and a shimmering label that rolls through phases and settles on a check
//!
//! Clean: a white chip with hairline shadows, thin accent-blue rays and a
//! quiet gray label. Classic: a bordered capsule with dark rays. Either way
//! the label walks through phases (Thinking → Searching the web → …): the
//! old one rolls up and out, the new one rolls in, a light band sweeps over
//! it and the chip glides to its width. When done, the spinner turns into a
//! check and the label settles on "Thought for 6s".
//!
//! ```ignore
//! ThinkingLoader::new("reply")
//!     .phases(["Thinking", "Searching the web", "Writing response"])
//!     .interval(2000)
//!     .done(finished)
//! ```

use std::{f32::consts::PI, mem, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, BoxShadow, Context, ElementId, FontWeight, HighlightStyle, Hsla,
    IntoElement, PathBuilder, Pixels, Point, Rgba, SharedString, StyledText, Task, TextRun, Window,
    canvas, div, font, point, prelude::*, px,
};

use crate::{
    ActiveTheme, Look,
    motion::{self, AnimationExt, EASE, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Thinking Loader";
pub const DESCRIPTION: &str = "A small working pill for AI replies – an eight-ray spinner and a shimmering label that rolls through phases and settles on a check";

/// The phases shown when none are given.
pub const DEFAULT_PHASES: [&str; 4] = [
    "Thinking",
    "Searching the web",
    "Reading sources",
    "Writing response",
];

/// The incoming label: `ui-tl-in`, 460ms.
const LABEL_IN: MotionSpec = MotionSpec::new(460, EASE_OUT_QUINT);
/// The outgoing label: `ui-tl-out`, 320ms.
const LABEL_OUT: MotionSpec = MotionSpec::new(320, EASE_OUT_QUINT);
/// The text box following the new label's width.
const WIDTH_GLIDE: MotionSpec = MotionSpec::new(460, EASE_OUT_QUINT);
/// Spinner ↔ check: `opacity 240ms ease`, `transform 420ms`.
const ICON_FADE: MotionSpec = MotionSpec::new(240, EASE);
const ICON_TURN: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT);
/// The check drawing itself in (`ui-tl-draw 360ms … 120ms`).
const CHECK_DRAW: MotionSpec = MotionSpec::new(360, EASE_OUT_QUINT).with_delay(120);
/// One turn of the rays (`ui-tl-ray 800ms linear infinite`).
const RAY_PERIOD_MS: f32 = 800.;

type DoneLabel = Rc<dyn Fn(u64) -> SharedString>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoaderSize {
    Sm,
    #[default]
    Md,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LoaderVariant {
    /// The bordered chip.
    #[default]
    Pill,
    /// Just the spinner and the label, e.g. inside a chat bubble.
    Plain,
}

/// The `--ui-tl-*` / `--ui-tlc-*` tokens.
struct Palette {
    bg: Hsla,
    shadow: Vec<BoxShadow>,
    /// Classic draws a 1px edge (an inset ring on the web).
    border: Option<Hsla>,
    text: Hsla,
    ink: Hsla,
    ray: Hsla,
    check: Hsla,
    shine: Hsla,
}

impl Palette {
    fn get(look: Look, dark: bool) -> Self {
        match (look, dark) {
            (Look::Clean, false) => Palette {
                bg: hex(0xffffff),
                shadow: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                border: None,
                text: black_a(0.61),
                ink: black_a(0.88),
                ray: hexa(0x0060e2, 0.765),
                check: hexa(0x0060e2, 0.765),
                shine: white_a(0.82),
            },
            (Look::Clean, true) => Palette {
                bg: hex(0x1c1c1e),
                shadow: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                border: None,
                text: white_a(0.62),
                ink: white_a(0.9),
                ray: hex(0x4d8dff),
                check: hex(0x4d8dff),
                shine: hex(0xffffff),
            },
            (Look::Classic, false) => Palette {
                bg: hex(0xffffff),
                shadow: vec![shadow(0., 1., 2., 0., black_a(0.04))],
                border: Some(hex(0xe7e7e7)),
                text: hex(0x6b6b6b),
                ink: hex(0x171717),
                ray: hex(0x171717),
                check: hex(0x171717),
                shine: white_a(0.82),
            },
            (Look::Classic, true) => Palette {
                bg: hex(0x161618),
                shadow: vec![shadow(0., 1., 2., 0., black_a(0.04))],
                border: Some(white_a(0.1)),
                text: hex(0x8f8f8f),
                ink: hex(0xededed),
                ray: hex(0xededed),
                check: hex(0xededed),
                shine: white_a(0.95),
            },
        }
    }
}

/// Sizes per look and size.
struct Metrics {
    /// `None` lets a plain Clean loader take the label's height.
    height: Option<Pixels>,
    pad_left: f32,
    pad_right: f32,
    radius: Pixels,
    gap: Pixels,
    font: Pixels,
    line: Pixels,
    weight: FontWeight,
    icon: f32,
    /// Stroke widths in the 16-unit viewBox.
    ray_width: f32,
    check_width: f32,
    /// Opacity a ray fades down to.
    ray_rest: f32,
    /// One sweep of the shine.
    shine_ms: f32,
}

impl Metrics {
    fn get(look: Look, size: LoaderSize, variant: LoaderVariant) -> Self {
        let plain = variant == LoaderVariant::Plain;
        match (look, size) {
            (Look::Clean, LoaderSize::Md) => Metrics {
                height: (!plain).then_some(px(32.)),
                pad_left: 8.,
                pad_right: 10.,
                radius: px(8.),
                gap: px(6.),
                font: px(13.),
                line: px(20.),
                weight: FontWeight::MEDIUM,
                icon: 16.,
                ray_width: 1.1,
                check_width: 1.2,
                ray_rest: 0.2,
                shine_ms: 1600.,
            },
            (Look::Clean, LoaderSize::Sm) => Metrics {
                height: (!plain).then_some(px(26.)),
                pad_left: 6.,
                pad_right: 8.,
                radius: px(6.),
                gap: px(5.),
                font: px(12.),
                line: px(16.),
                weight: FontWeight::MEDIUM,
                icon: 14.,
                ray_width: 1.1,
                check_width: 1.2,
                ray_rest: 0.2,
                shine_ms: 1600.,
            },
            // `line-height: 1.3em` on the label box
            (Look::Classic, LoaderSize::Md) => Metrics {
                height: Some(px(34.)),
                pad_left: 12.,
                pad_right: 15.,
                radius: px(999.),
                gap: px(8.),
                font: px(14.),
                line: px(18.2),
                weight: FontWeight::NORMAL,
                icon: 15.,
                ray_width: 1.6,
                check_width: 1.8,
                ray_rest: 0.18,
                shine_ms: 1800.,
            },
            (Look::Classic, LoaderSize::Sm) => Metrics {
                height: Some(px(28.)),
                pad_left: 10.,
                pad_right: 12.,
                radius: px(999.),
                gap: px(7.),
                font: px(13.),
                line: px(16.9),
                weight: FontWeight::NORMAL,
                icon: 13.,
                ray_width: 1.6,
                check_width: 1.8,
                ray_rest: 0.18,
                shine_ms: 1800.,
            },
        }
    }
}

/// A label on screen, keyed so each one gets its own roll animation.
#[derive(Clone)]
struct Label {
    key: u64,
    text: SharedString,
    /// When it came in, or – for the leaving one – when it started to go.
    at: Instant,
}

struct LoaderState {
    /// Mount time: the clock for the spinner and the shine.
    epoch: Instant,
    /// When the current run of work started (phase timer and "Thought for").
    started: Instant,
    /// Seconds read when `done` turned on.
    seconds: Option<u64>,
    done_at: Option<Instant>,
    shown: Option<Label>,
    leaving: Option<Label>,
}

impl LoaderState {
    fn new(now: Instant) -> Self {
        Self {
            epoch: now,
            started: now,
            seconds: None,
            done_at: None,
            shown: None,
            leaving: None,
        }
    }
}

/// A "the model is working" pill.
#[derive(IntoElement)]
pub struct ThinkingLoader {
    id: ElementId,
    phases: Vec<SharedString>,
    interval: Duration,
    phase: Option<usize>,
    looping: bool,
    done: bool,
    done_label: DoneLabel,
    size: LoaderSize,
    variant: LoaderVariant,
    look: Option<Look>,
}

impl ThinkingLoader {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            phases: DEFAULT_PHASES
                .iter()
                .map(|p| SharedString::from(*p))
                .collect(),
            interval: Duration::from_millis(2200),
            phase: None,
            looping: false,
            done: false,
            done_label: Rc::new(|s| format!("Thought for {s}s").into()),
            size: LoaderSize::Md,
            variant: LoaderVariant::Pill,
            look: None,
        }
    }

    /// Labels to walk through, one after another.
    pub fn phases<S: Into<SharedString>>(mut self, phases: impl IntoIterator<Item = S>) -> Self {
        self.phases = phases.into_iter().map(Into::into).collect();
        self
    }

    /// Time per phase, in milliseconds (default 2200).
    pub fn interval(mut self, ms: u64) -> Self {
        self.interval = Duration::from_millis(ms);
        self
    }

    /// Shows a specific phase instead of advancing on a timer – e.g. from
    /// your stream events.
    pub fn phase(mut self, index: usize) -> Self {
        self.phase = Some(index);
        self
    }

    /// Starts over after the last phase instead of holding it.
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Finished: the spinner turns into a check and the label into the done label.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    /// A fixed label once done.
    pub fn done_label(mut self, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        self.done_label = Rc::new(move |_| label.clone());
        self
    }

    /// The label once done, from the seconds since the loader started.
    pub fn done_label_with(mut self, label: impl Fn(u64) -> SharedString + 'static) -> Self {
        self.done_label = Rc::new(label);
        self
    }

    pub fn size(mut self, size: LoaderSize) -> Self {
        self.size = size;
        self
    }

    pub fn variant(mut self, variant: LoaderVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Overrides the global Clean/Classic look for this loader.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for ThinkingLoader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let p = Palette::get(look, cx.is_dark());
        let m = Metrics::get(look, self.size, self.variant);
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);
        let scale = motion::speed_scale();
        let now = Instant::now();
        let done = self.done;

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| LoaderState::new(now),
        );

        let phases = self.phases;
        let interval = self.interval.as_millis().max(1);
        let (phase, looping, done_label) = (self.phase, self.looping, self.done_label);
        let (shown, leaving, done_at, epoch) = state.update(cx, |s, _| {
            // Read the clock on the render that turns done on, restart it when work resumes
            let elapsed = s.seconds.unwrap_or_else(|| {
                (now.saturating_duration_since(s.started)
                    .as_secs_f32()
                    .round() as u64)
                    .max(1)
            });
            if done && s.seconds.is_none() {
                s.seconds = Some(elapsed);
                s.done_at = Some(now);
            }
            if !done && s.seconds.is_some() {
                s.seconds = None;
                s.done_at = None;
                s.started = now;
            }

            let label = if done {
                done_label(elapsed)
            } else {
                let count = phases.len();
                let ticks =
                    (now.saturating_duration_since(s.started).as_millis() / interval) as usize;
                let step = match (count < 2, looping) {
                    (true, _) => 0,
                    (false, true) => ticks % count,
                    (false, false) => ticks.min(count - 1),
                };
                let index = phase.unwrap_or(step).min(count.saturating_sub(1));
                phases.get(index).cloned().unwrap_or_default()
            };

            // Swap during render so the old label rolls out as the new one rolls in
            match &mut s.shown {
                None => {
                    s.shown = Some(Label {
                        key: 0,
                        text: label,
                        at: now,
                    })
                }
                Some(shown) if shown.text != label => {
                    let next = Label {
                        key: shown.key + 1,
                        text: label,
                        at: now,
                    };
                    let mut old = mem::replace(shown, next);
                    old.at = now;
                    s.leaving = Some(old);
                }
                Some(_) => {}
            }
            let gone = LABEL_OUT.total().mul_f32(scale);
            if s.leaving
                .as_ref()
                .is_some_and(|l| now.saturating_duration_since(l.at) > gone)
            {
                s.leaving = None;
            }
            (
                s.shown.clone().expect("label set above"),
                s.leaving.clone(),
                s.done_at,
                s.epoch,
            )
        });

        // The text box takes the width of the incoming label (+1 covers rounding)
        let mut label_font = font(SANS);
        label_font.weight = m.weight;
        let shaped = window.text_system().shape_line(
            shown.text.clone(),
            m.font,
            &[TextRun {
                len: shown.text.len(),
                font: label_font,
                color: p.text,
                background_color: None,
                underline: None,
                strikethrough: None,
            }],
            None,
        );
        let box_width = motion::follow(
            &format!("{key}-width"),
            f32::from(shaped.width) + 1.,
            WIDTH_GLIDE,
        );

        let clock_ms = now.saturating_duration_since(epoch).as_secs_f32() * 1000. / scale;
        let label_color = if done { p.ink } else { p.text };

        // A light band sweeping through the label while it works
        let text = if done || reduced {
            StyledText::new(shown.text.clone())
        } else {
            let width = f32::from(shaped.width).max(1.);
            let sweep = (clock_ms % m.shine_ms) / m.shine_ms;
            let center = -0.5 * width + 2. * width * sweep;
            let half = 0.36 * width;
            let highlights: Vec<_> = shown
                .text
                .char_indices()
                .filter_map(|(ix, ch)| {
                    let end = ix + ch.len_utf8();
                    let x = f32::from(shaped.x_for_index(ix) + shaped.x_for_index(end)) / 2.;
                    let strength = 1. - (x - center).abs() / half;
                    (strength > 0.).then(|| {
                        let color = over(p.shine.opacity(strength), label_color);
                        (
                            ix..end,
                            HighlightStyle {
                                color: Some(color),
                                ..Default::default()
                            },
                        )
                    })
                })
                .collect();
            StyledText::new(shown.text.clone()).with_highlights(highlights)
        };

        let rise = f32::from(m.line) * 0.7;
        let label = |text: StyledText| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .whitespace_nowrap()
                .line_height(m.line)
                .text_color(label_color)
                .child(text)
        };
        let incoming: AnyElement = if shown.key > 0 && !reduced {
            label(text)
                .with_animation(
                    ElementId::NamedInteger("in".into(), shown.key),
                    LABEL_IN.animation(),
                    move |el, t| el.opacity(t).top(px(rise * (1. - t))),
                )
                .into_any_element()
        } else {
            label(text).into_any_element()
        };
        let outgoing = leaving.filter(|_| !reduced).map(|old| {
            label(StyledText::new(old.text)).with_animation(
                ElementId::NamedInteger("out".into(), old.key),
                LABEL_OUT.animation(),
                move |el, t| el.opacity(1. - t).top(px(-rise * t)),
            )
        });
        let text_box = div()
            .relative()
            .flex_none()
            .w(px(box_width))
            .h(m.line)
            .overflow_hidden()
            .children(outgoing)
            .child(incoming);

        // Spinner and check share one square and swap with a little scale
        let target = if done { 1. } else { 0. };
        let fade = motion::follow(&format!("{key}-icon-fade"), target, ICON_FADE);
        let turn = motion::follow(&format!("{key}-icon-turn"), target, ICON_TURN);
        let draw = match done_at {
            Some(_) if reduced => 1.,
            Some(at) => {
                let total = CHECK_DRAW.total().mul_f32(scale).as_secs_f32();
                CHECK_DRAW.progress(now.saturating_duration_since(at).as_secs_f32() / total)
            }
            None => 0.,
        };
        let rays = std::array::from_fn(|i| {
            if reduced {
                return 0.6;
            }
            let lead = (8 - i) as f32 * 100.;
            let phase = ((clock_ms + lead) % RAY_PERIOD_MS) / RAY_PERIOD_MS;
            1. - (1. - m.ray_rest) * phase
        });
        let icon = Icon {
            size: m.icon,
            rays,
            ray_color: p.ray,
            ray_width: m.ray_width,
            spinner_opacity: 1. - fade,
            spinner_scale: motion::lerp(1., 0.6, turn),
            spinner_angle: motion::lerp(0., -PI / 4., turn),
            check_color: p.check,
            check_width: m.check_width,
            check_opacity: fade,
            check_scale: motion::lerp(0.6, 1., turn),
            draw,
        }
        .render();

        // Keep the spinner turning, and the check drawing
        let drawing = done_at.is_some_and(|at| {
            now.saturating_duration_since(at) < CHECK_DRAW.total().mul_f32(scale)
        });
        if (!done && !reduced) || drawing {
            window.request_animation_frame();
        }
        motion::keep_alive(window);

        let border = if self.variant == LoaderVariant::Pill {
            p.border
        } else {
            None
        };
        let inset = if border.is_some() { 1. } else { 0. };
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .gap(m.gap)
            .font_family(SANS)
            .font_weight(m.weight)
            .text_size(m.font)
            .line_height(m.line)
            .text_color(p.text)
            .when_some(m.height, |d, h| d.h(h))
            .when(self.variant == LoaderVariant::Pill, |d| {
                d.pl(px(m.pad_left - inset))
                    .pr(px(m.pad_right - inset))
                    .rounded(m.radius)
                    .bg(p.bg)
                    .shadow(p.shadow)
            })
            .when_some(border, |d, color| d.border_1().border_color(color))
            .child(icon)
            .child(text_box)
    }
}

/// `top` composited over `bottom` (premultiplied), like the shine layer
/// painted over the label.
fn over(top: Hsla, bottom: Hsla) -> Hsla {
    let (t, b) = (Rgba::from(top), Rgba::from(bottom));
    let a = t.a + b.a * (1. - t.a);
    if a <= f32::EPSILON {
        return top;
    }
    let channel = |x: f32, y: f32| (x * t.a + y * b.a * (1. - t.a)) / a;
    Rgba {
        r: channel(t.r, b.r),
        g: channel(t.g, b.g),
        b: channel(t.b, b.b),
        a,
    }
    .into()
}

/// The eight rays and the check, painted in a 16-unit viewBox.
struct Icon {
    size: f32,
    rays: [f32; 8],
    ray_color: Hsla,
    ray_width: f32,
    spinner_opacity: f32,
    spinner_scale: f32,
    spinner_angle: f32,
    check_color: Hsla,
    check_width: f32,
    check_opacity: f32,
    check_scale: f32,
    /// How much of the check is drawn (stroke-dashoffset 1 → 0).
    draw: f32,
}

impl Icon {
    fn render(self) -> impl IntoElement {
        let size = self.size;
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let unit = size / 16.;
                let center = bounds.center();
                // viewBox point → window, scaled and turned around the middle
                let map = |x: f32, y: f32, scale: f32, angle: f32| -> Point<Pixels> {
                    let (dx, dy) = ((x - 8.) * scale * unit, (y - 8.) * scale * unit);
                    let (sin, cos) = angle.sin_cos();
                    point(
                        center.x + px(dx * cos - dy * sin),
                        center.y + px(dx * sin + dy * cos),
                    )
                };

                if self.spinner_opacity > 0.001 {
                    let half = self.ray_width * unit * self.spinner_scale / 2.;
                    for (i, opacity) in self.rays.iter().enumerate() {
                        let angle = i as f32 * PI / 4. + self.spinner_angle;
                        let from = map(8., 1.75, self.spinner_scale, angle);
                        let to = map(8., 4.75, self.spinner_scale, angle);
                        let color = self
                            .ray_color
                            .opacity(opacity * self.spinner_opacity.clamp(0., 1.));
                        paint_capsule(window, from, to, half, color);
                    }
                }

                if self.check_opacity > 0.001 && self.draw > 0.001 {
                    let points = [(3.5, 8.4), (6.6, 11.3), (12.5, 4.9)];
                    let width = self.check_width * unit * self.check_scale;
                    let partial = partial_polyline(&points, self.draw);
                    let mut mapped: Vec<Point<Pixels>> = partial
                        .iter()
                        .map(|(x, y)| map(*x, *y, self.check_scale, 0.))
                        .collect();
                    // Square ends stand in for round caps
                    extend_ends(&mut mapped, width / 2.);
                    let mut path = PathBuilder::stroke(px(width));
                    path.move_to(mapped[0]);
                    for p in &mapped[1..] {
                        path.line_to(*p);
                    }
                    if let Ok(path) = path.build() {
                        let color = self.check_color.opacity(self.check_opacity.clamp(0., 1.));
                        window.paint_path(path, color);
                    }
                }
            },
        )
        .flex_none()
        .size(px(size))
    }
}

/// A line with round caps, filled so it never overlaps itself.
fn paint_capsule(
    window: &mut Window,
    from: Point<Pixels>,
    to: Point<Pixels>,
    half: f32,
    color: Hsla,
) {
    let (dx, dy) = (f32::from(to.x - from.x), f32::from(to.y - from.y));
    let base = dy.atan2(dx);
    let mut points = Vec::with_capacity(20);
    const STEPS: usize = 8;
    for (end, start) in [(to, base - PI / 2.), (from, base + PI / 2.)] {
        for k in 0..=STEPS {
            let a = start + PI * k as f32 / STEPS as f32;
            points.push(point(
                end.x + px(half * a.cos()),
                end.y + px(half * a.sin()),
            ));
        }
    }
    let mut path = PathBuilder::fill();
    path.add_polygon(&points, true);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// The first `t` (0..1) of a polyline, by length.
fn partial_polyline(points: &[(f32, f32)], t: f32) -> Vec<(f32, f32)> {
    let lengths: Vec<f32> = points
        .windows(2)
        .map(|w| ((w[1].0 - w[0].0).powi(2) + (w[1].1 - w[0].1).powi(2)).sqrt())
        .collect();
    let mut left = lengths.iter().sum::<f32>() * t.clamp(0., 1.);
    let mut out = vec![points[0]];
    for (w, len) in points.windows(2).zip(&lengths) {
        if left >= *len {
            out.push(w[1]);
            left -= len;
        } else {
            let f = left / len.max(0.0001);
            out.push((
                w[0].0 + (w[1].0 - w[0].0) * f,
                w[0].1 + (w[1].1 - w[0].1) * f,
            ));
            break;
        }
    }
    out
}

/// Pushes both ends of a polyline outward by `by` px.
fn extend_ends(points: &mut [Point<Pixels>], by: f32) {
    let n = points.len();
    if n < 2 {
        return;
    }
    let push = |end: Point<Pixels>, toward: Point<Pixels>| {
        let (dx, dy) = (f32::from(end.x - toward.x), f32::from(end.y - toward.y));
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.0001 {
            return end;
        }
        point(end.x + px(dx / len * by), end.y + px(dy / len * by))
    };
    points[0] = push(points[0], points[1]);
    points[n - 1] = push(points[n - 1], points[n - 2]);
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

const DEMO_PHASES: [&str; 5] = [
    "Thinking",
    "Searching the web",
    "Reading 12 sources",
    "Comparing pricing",
    "Writing response",
];
const DEMO_INTERVAL: u64 = 2000;
/// How long the finished loader stays before the demo starts over.
const DEMO_HOLD: u64 = 2600;

/// The demo's clock: works through the phases, finishes, holds, starts over.
struct ChatDemo {
    run: u64,
    done: bool,
    _timer: Task<()>,
}

impl ChatDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        let work = Duration::from_millis(DEMO_PHASES.len() as u64 * DEMO_INTERVAL);
        let hold = Duration::from_millis(DEMO_HOLD);
        let timer = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(work).await;
                let finished = this.update(cx, |demo, cx| {
                    demo.done = true;
                    cx.notify();
                });
                if finished.is_err() {
                    break;
                }
                cx.background_executor().timer(hold).await;
                let restarted = this.update(cx, |demo, cx| {
                    demo.done = false;
                    demo.run += 1;
                    cx.notify();
                });
                if restarted.is_err() {
                    break;
                }
            }
        });
        Self {
            run: 0,
            done: false,
            _timer: timer,
        }
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("thinking-loader-demo", cx, |_, cx| ChatDemo::new(cx));
        let (run, done) = {
            let demo = demo.read(cx);
            (demo.run, demo.done)
        };
        let clean = cx.look() == Look::Clean;
        let dark = cx.is_dark();
        let (fg, bg) = match (clean, dark) {
            (true, false) => (hexa(0x000000, 0.878), hexa(0x000000, 0.039)),
            (true, true) => (hexa(0xffffff, 0.9), hexa(0xffffff, 0.078)),
            (false, false) => (hex(0x171717), hex(0xf0f0f0)),
            (false, true) => (hex(0xededed), hex(0x262628)),
        };
        let bubble = div()
            .max_w(px(420. * 0.88))
            .text_color(fg)
            .bg(bg)
            .child("Which CRM fits a 10-person sales team best?");
        let bubble = if clean {
            bubble
                .px(px(12.))
                .py(px(8.))
                .text_size(px(13.))
                .line_height(px(20.))
                .rounded(px(10.))
                .rounded_br(px(4.))
        } else {
            bubble
                .px(px(14.))
                .py(px(10.))
                .text_size(px(14.))
                .line_height(px(20.3))
                .rounded(px(18.))
                .rounded_br(px(6.))
        };

        div()
            .w(px(420.))
            .flex()
            .flex_col()
            .items_start()
            .gap(px(18.))
            .child(div().w_full().flex().justify_end().child(bubble))
            .child(
                ThinkingLoader::new(ElementId::NamedInteger("thinking-loader".into(), run))
                    .phases(DEMO_PHASES)
                    .interval(DEMO_INTERVAL)
                    .done(done),
            )
    })
}
