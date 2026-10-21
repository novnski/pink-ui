//! Thinking State – an original component in luna style.
//!
//! A live panel for an assistant at work. The header carries a spinner, the
//! running step count and a ticking timer, with a hairline along its bottom
//! edge that fills as steps finish. Steps rise in one by one on a rail: the
//! running one breathes next to a spinner, finished ones settle with a check
//! and their duration, and a step can list what it read as chips that pop in
//! one after another. Once everything is done the header turns into a
//! "Thought for 7.4s" summary and the body folds away; click the header (or
//! press Enter / Space on it) to open it again.
//!
//! ```ignore
//! ThinkingState::new("plan", vec![
//!     ThinkingStep::new("Reading the request").done(Duration::from_millis(900)),
//!     ThinkingStep::new("Searched the codebase")
//!         .item(StepItem::new("schema.prisma").icon(ICON_FILE))
//!         .running(),
//!     ThinkingStep::new("Drafting a plan"),
//! ])
//! ```

use std::{
    cell::Cell,
    f32::consts::TAU,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnyView, App, Context, ElementId, FocusHandle, FontWeight, Hsla, IntoElement,
    KeyDownEvent, SharedString, Task, Transformation, Window, canvas, div, percentage, prelude::*,
    px, radians, relative, svg,
};

use crate::{
    HoverFill,
    luna::{self, FillTier, Luna, radius, space, text},
    motion::{self, AnimationExt, EASE_IN_OUT, GLIDE, MotionSpec, ROW_IN},
    story,
};

pub const NAME: &str = "Thinking State";
pub const DESCRIPTION: &str = "A live working panel: a ticking timer and a filling hairline over steps that rise in one by one, the files each step read popping in as chips, folding to a one-line summary when the work is done";

pub const ICON_FILE: &str = "icons/thinking-state/file.svg";
pub const ICON_PULL: &str = "icons/thinking-state/pull.svg";
pub const ICON_TABLE: &str = "icons/thinking-state/table.svg";
const ICON_CHECK: &str = "icons/thinking-state/check-bold.svg";
const ICON_CHEVRON: &str = "icons/thinking-state/chevron.svg";
const ICON_RING: &str = "icons/thinking-state/ring.svg";
const ICON_ARC: &str = "icons/thinking-state/arc.svg";
const ICON_SPARK: &str = "icons/thinking-state/spark.svg";

/// How long the finished panel stays open before folding.
const FOLD_AFTER: Duration = Duration::from_millis(1100);

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Where a step stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    /// Not started – hidden until it runs.
    Pending,
    Running,
    Done,
}

/// Something a step read or touched, shown as a chip under it.
#[derive(Clone)]
pub struct StepItem {
    pub label: SharedString,
    /// An SVG path under `assets/`; a file glyph when unset.
    pub icon: Option<SharedString>,
}

impl StepItem {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            icon: None,
        }
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }
}

/// One line of the trace.
#[derive(Clone)]
pub struct ThinkingStep {
    pub title: SharedString,
    pub detail: Option<SharedString>,
    pub items: Vec<StepItem>,
    /// Extra items folded into a "+N more" chip.
    pub more: usize,
    pub status: StepStatus,
    /// Shown next to a finished step.
    pub took: Option<Duration>,
}

impl ThinkingStep {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            detail: None,
            items: Vec::new(),
            more: 0,
            status: StepStatus::Pending,
            took: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn item(mut self, item: StepItem) -> Self {
        self.items.push(item);
        self
    }

    pub fn more(mut self, count: usize) -> Self {
        self.more = count;
        self
    }

    pub fn status(mut self, status: StepStatus) -> Self {
        self.status = status;
        self
    }

    pub fn running(self) -> Self {
        self.status(StepStatus::Running)
    }

    pub fn done(mut self, took: Duration) -> Self {
        self.took = Some(took);
        self.status(StepStatus::Done)
    }
}

struct State {
    focus: FocusHandle,
    started: Instant,
    done_at: Option<Instant>,
    /// The reader's own choice, until the next run starts.
    user_open: Option<bool>,
    body_height: Rc<Cell<f32>>,
}

#[derive(IntoElement)]
pub struct ThinkingState {
    id: ElementId,
    steps: Vec<ThinkingStep>,
    started_at: Option<Instant>,
    auto_fold: bool,
    width: f32,
    on_toggle: Option<ToggleHandler>,
}

impl ThinkingState {
    pub fn new(id: impl Into<ElementId>, steps: Vec<ThinkingStep>) -> Self {
        Self {
            id: id.into(),
            steps,
            started_at: None,
            auto_fold: true,
            width: 580.,
            on_toggle: None,
        }
    }

    /// When the work began; defaults to the first render (and restarts when
    /// a finished trace starts running again).
    pub fn started_at(mut self, at: Instant) -> Self {
        self.started_at = Some(at);
        self
    }

    /// Keep the steps open after the work finishes.
    pub fn keep_open(mut self) -> Self {
        self.auto_fold = false;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Called with the new open state when the reader folds or unfolds it.
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }
}

/// Records the laid-out height of its parent into `cell`, and asks for one
/// more frame when it changed so a follower can glide to it.
fn measure(cell: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            if (cell.get() - h).abs() > 0.5 {
                cell.set(h);
                window.on_next_frame(|window, _| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A rotating arc on a faint ring.
fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla, reduced: bool) -> impl IntoElement {
    let arc = svg().path(ICON_ARC).size(px(size)).text_color(color);
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(
            svg()
                .path(ICON_RING)
                .absolute()
                .size(px(size))
                .text_color(color.opacity(0.22)),
        )
        .child(if reduced {
            arc.into_any_element()
        } else {
            arc.with_animation(
                id,
                Animation::new(Duration::from_millis(850)).repeat(),
                |s, d| s.with_transformation(Transformation::rotate(percentage(d))),
            )
            .into_any_element()
        })
}

fn seconds(d: Duration) -> String {
    format!("{:.1}s", d.as_secs_f32())
}

impl RenderOnce for ThinkingState {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let reduced = motion::reduced_motion(cx);
        let key = self.id.to_string();
        let now = Instant::now();

        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            |_, cx: &mut Context<State>| State {
                focus: cx.focus_handle(),
                started: Instant::now(),
                done_at: None,
                user_open: None,
                body_height: Rc::new(Cell::new(0.)),
            },
        );

        let total = self.steps.len();
        let done_count = self
            .steps
            .iter()
            .filter(|s| s.status == StepStatus::Done)
            .count();
        let done = total > 0 && done_count == total;
        let running_ix = self
            .steps
            .iter()
            .position(|s| s.status == StepStatus::Running);

        // A finished trace that starts again resets the clock and the fold.
        state.update(cx, |s, _| {
            if done && s.done_at.is_none() {
                s.done_at = Some(now);
            } else if !done && s.done_at.is_some() {
                s.done_at = None;
                s.user_open = None;
                s.started = now;
            }
        });
        let s = state.read(cx);
        let started = self.started_at.unwrap_or(s.started);
        let elapsed = s.done_at.unwrap_or(now).saturating_duration_since(started);
        let folded_by_itself = self.auto_fold
            && s.done_at
                .is_some_and(|d| now.saturating_duration_since(d) >= FOLD_AFTER);
        if done && self.auto_fold && !folded_by_itself {
            // Keep frames coming until the fold kicks in.
            window.request_animation_frame();
        }
        let open = s.user_open.unwrap_or(!folded_by_itself);
        let focus = s.focus.clone();
        let body_height = s.body_height.clone();

        // ---- header ---------------------------------------------------------
        let read_count: usize = self.steps.iter().map(|s| s.items.len() + s.more).sum();
        let status_glyph = if done {
            div()
                .flex_none()
                .size(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(t.success.opacity(if t.dark { 0.18 } else { 0.14 }))
                .child(svg().path(ICON_CHECK).size(px(10.)).text_color(t.success))
                .into_any_element()
        } else if running_ix.is_some() {
            spinner(("ts-head-spin", 0usize), 16., t.accent, reduced).into_any_element()
        } else {
            svg()
                .path(ICON_SPARK)
                .size(px(16.))
                .flex_none()
                .text_color(t.text_faint)
                .into_any_element()
        };
        let title: SharedString = if done {
            format!("Thought for {}", seconds(elapsed)).into()
        } else {
            "Thinking".into()
        };
        let subtitle: SharedString = if done {
            if read_count > 0 {
                format!("{total} steps · read {read_count} sources").into()
            } else {
                format!("{total} steps").into()
            }
        } else if let Some(ix) = running_ix {
            format!("Step {} of {}", ix + 1, total).into()
        } else {
            "Starting".into()
        };

        let toggle = {
            let state = state.clone();
            let on_toggle = self.on_toggle.clone();
            move |window: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| {
                    s.user_open = Some(!open);
                    cx.notify();
                });
                if let Some(f) = &on_toggle {
                    f(!open, window, cx);
                }
            }
        };
        let toggle = Rc::new(toggle);

        let head_key = format!("{key}-head");
        let chevron_turn = motion::follow(
            &format!("{key}-chevron"),
            if open { 0. } else { -0.25 },
            GLIDE,
        );
        let progress = if total == 0 {
            0.
        } else {
            done_count as f32 / total as f32
        };
        let fill = motion::follow(&format!("{key}-progress"), progress, GLIDE);
        let fill_alpha = motion::follow(
            &format!("{key}-progress-alpha"),
            if done { 0. } else { 1. },
            MotionSpec::new(700, EASE_IN_OUT).with_delay(300),
        );
        let body_open = motion::follow(&format!("{key}-open"), if open { 1. } else { 0. }, GLIDE);

        let header = div()
            .id("ts-head")
            .track_focus(&focus)
            .relative()
            .h(px(44.))
            .px(px(space::S16 - 2.))
            .flex()
            .items_center()
            .gap(px(space::S8))
            .cursor_pointer()
            .rounded_t(px(radius::LG - 1.))
            .when(body_open < 0.02, |d| d.rounded_b(px(radius::LG - 1.)))
            .hover_fill(
                head_key.clone(),
                t.fill(FillTier::Quaternary).opacity(0.),
                t.fill(FillTier::Quaternary),
            )
            .on_click({
                let toggle = toggle.clone();
                move |_, window, cx| toggle(window, cx)
            })
            .on_key_down({
                let toggle = toggle.clone();
                move |ev: &KeyDownEvent, window, cx| match ev.keystroke.key.as_str() {
                    "enter" | "space" => {
                        toggle(window, cx);
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            })
            .child(status_glyph)
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(space::S8))
                    .min_w_0()
                    .flex_1()
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(text::BODY))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(subtitle),
                    ),
            )
            .when(!done, |d| {
                d.child(
                    div()
                        .flex_none()
                        .text_size(px(text::FOOTNOTE))
                        .font_family(luna::MONO)
                        .text_color(t.text_muted)
                        .child(seconds(elapsed)),
                )
            })
            .child(
                svg()
                    .path(ICON_CHEVRON)
                    .size(px(16.))
                    .flex_none()
                    .text_color(motion::hover_blend(&head_key, t.text_faint, t.text))
                    .with_transformation(Transformation::rotate(radians(chevron_turn * TAU))),
            )
            // The hairline under the header doubles as a progress bar.
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(1.))
                    .bg(t.border.opacity(body_open))
                    .child(
                        div()
                            .h_full()
                            .w(relative(fill))
                            .bg(t.accent.opacity(fill_alpha)),
                    ),
            );

        // ---- steps ----------------------------------------------------------
        let visible: Vec<(usize, &ThinkingStep)> = self
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.status != StepStatus::Pending)
            .collect();
        let last_visible = visible.last().map(|(ix, _)| *ix);
        let rows = visible.iter().map(|&(ix, step)| {
            let is_last = Some(ix) == last_visible;
            let running = step.status == StepStatus::Running;

            let node = if running {
                spinner(("ts-spin", ix), 14., t.accent, reduced).into_any_element()
            } else {
                div()
                    .flex_none()
                    .size(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(t.success.opacity(if t.dark { 0.18 } else { 0.14 }))
                    .child(
                        svg()
                            .path(ICON_CHECK)
                            .size(px(9.))
                            .text_color(t.success)
                            .with_animation(
                                ("ts-check", ix),
                                MotionSpec::new(360, motion::EASE_OUT_BACK).animation(),
                                |s, d| s.size(px(4. + 5. * d)).opacity(d),
                            ),
                    )
                    .into_any_element()
            };
            let rail = div()
                .flex_none()
                .w(px(14.))
                .flex()
                .flex_col()
                .items_center()
                .pt(px(3.))
                .child(node)
                .when(!is_last, |d| {
                    d.child(div().flex_1().w(px(1.)).mt(px(4.)).mb(px(1.)).child(
                        div().w_full().bg(t.border_strong).with_animation(
                            ("ts-line", ix),
                            ROW_IN.animation(),
                            |l, d| l.h(relative(d)),
                        ),
                    ))
                });

            let muted = t.text_muted;
            let bright = t.text;
            let title = div()
                .flex_1()
                .min_w_0()
                .text_size(px(text::BODY))
                .line_height(px(20.))
                .text_color(if running { bright } else { muted })
                .child(step.title.clone());
            let title = if running && !reduced {
                title
                    .with_animation(
                        ("ts-breathe", ix),
                        Animation::new(Duration::from_millis(1800)).repeat(),
                        move |el, d| {
                            let w = 0.5 + 0.5 * (d * TAU).cos();
                            el.text_color(motion::mix(muted, bright, 0.35 + 0.65 * w))
                        },
                    )
                    .into_any_element()
            } else {
                title.into_any_element()
            };

            let chips = (!step.items.is_empty() || step.more > 0).then(|| {
                let mut chips: Vec<_> = step
                    .items
                    .iter()
                    .map(|item| {
                        (
                            item.icon.clone().unwrap_or_else(|| ICON_FILE.into()),
                            item.label.clone(),
                        )
                    })
                    .map(Some)
                    .collect();
                if step.more > 0 {
                    chips.push(None);
                }
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(space::S6))
                    .mt(px(space::S8))
                    .children(chips.into_iter().enumerate().map(|(k, chip)| {
                        let base = div()
                            .h(px(22.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .rounded(px(radius::SM))
                            .border_1()
                            .border_color(t.border)
                            .bg(t.fill(FillTier::Quaternary))
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_muted);
                        let chip = match chip {
                            Some((icon, label)) => base
                                .font_family(luna::MONO)
                                .child(
                                    svg()
                                        .path(icon)
                                        .size(px(12.))
                                        .flex_none()
                                        .text_color(t.text_faint),
                                )
                                .child(label),
                            None => base.child(format!("+{} more", step.more)),
                        };
                        motion::rise_in(
                            ElementId::NamedInteger("ts-chip".into(), (ix * 32 + k) as u64),
                            ROW_IN.with_delay(120 + 90 * k as u64),
                            4.,
                            chip,
                        )
                    }))
            });

            let content = div()
                .flex_1()
                .min_w_0()
                .pb(px(if is_last { 2. } else { space::S12 }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S8))
                        .child(title)
                        .when_some(step.took.filter(|_| !running), |d, took| {
                            d.child(
                                div()
                                    .flex_none()
                                    .text_size(px(text::CAPTION))
                                    .font_family(luna::MONO)
                                    .text_color(t.text_faint)
                                    .child(seconds(took)),
                            )
                        }),
                )
                .when_some(step.detail.clone(), |d, detail| {
                    d.child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(18.))
                            .text_color(t.text_faint)
                            .child(detail),
                    )
                })
                .children(chips);

            motion::rise_in(
                ElementId::NamedInteger("ts-row".into(), ix as u64),
                ROW_IN,
                6.,
                div().flex().gap(px(space::S12)).child(rail).child(content),
            )
        });

        let natural = body_height.get();
        let body = div()
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .px(px(space::S16))
            .pt(px(space::S12 + 2.))
            .pb(px(space::S12))
            .children(rows)
            .child(measure(body_height));
        let target = if open { natural } else { 0. };
        let height = motion::follow(&format!("{key}-height"), target, GLIDE);

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .font_family(luna::SANS)
            .child(
                luna::card(&t).flex().flex_col().child(header).child(
                    div()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        // Natural height until the first measurement lands.
                        .when(natural > 0., |d| d.h(px(height)))
                        .opacity(body_open.clamp(0., 1.))
                        .child(body),
                ),
            )
    }
}

// ---------------------------------------------------------------------------
// Story: an agent planning a schema migration, looping.
// ---------------------------------------------------------------------------

struct DemoStep {
    title: &'static str,
    detail: &'static str,
    items: &'static [(&'static str, &'static str)],
    more: usize,
    from: f32,
    to: f32,
}

const DEMO: &[DemoStep] = &[
    DemoStep {
        title: "Reading the migration request",
        detail: "Split orders.total into subtotal, tax and shipping",
        items: &[],
        more: 0,
        from: 0.0,
        to: 1.3,
    },
    DemoStep {
        title: "Searched the codebase",
        detail: "7 files touch orders.total",
        items: &[
            (ICON_FILE, "schema.prisma"),
            (ICON_FILE, "checkout.ts"),
            (ICON_TABLE, "0041_orders.sql"),
            (ICON_PULL, "#1184"),
        ],
        more: 3,
        from: 1.3,
        to: 3.2,
    },
    DemoStep {
        title: "Checking 14 queries that write to orders",
        detail: "2 raw SQL writers need a backfill-safe path",
        items: &[],
        more: 0,
        from: 3.2,
        to: 4.6,
    },
    DemoStep {
        title: "Estimating lock time on 2.3M rows",
        detail: "Batches of 5,000 keep each lock under 40 ms",
        items: &[],
        more: 0,
        from: 4.6,
        to: 6.0,
    },
    DemoStep {
        title: "Drafting a zero-downtime plan",
        detail: "Expand, backfill, dual-write, then contract",
        items: &[],
        more: 0,
        from: 6.0,
        to: 7.4,
    },
];

/// Seconds from the end of a run to the next one.
const DEMO_REST: f32 = 5.0;

struct Demo {
    started: Instant,
    _ticker: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let alive = this.update(cx, |demo, cx| {
                    let end = DEMO.last().map_or(0., |s| s.to);
                    if demo.started.elapsed().as_secs_f32() > end + DEMO_REST {
                        demo.started = Instant::now();
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Demo {
            started: Instant::now(),
            _ticker: ticker,
        }
    }

    fn steps(&self) -> Vec<ThinkingStep> {
        let at = self.started.elapsed().as_secs_f32();
        DEMO.iter()
            .map(|d| {
                let mut step = ThinkingStep::new(d.title).detail(d.detail).more(d.more);
                for (icon, label) in d.items {
                    step = step.item(StepItem::new(*label).icon(*icon));
                }
                if at >= d.to {
                    step.done(Duration::from_secs_f32(d.to - d.from))
                } else if at >= d.from {
                    step.running()
                } else {
                    step
                }
            })
            .collect()
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("thinking-state-demo", cx, |_, cx| Demo::new(cx));
        let demo = demo.read(cx);
        ThinkingState::new("thinking-state", demo.steps()).started_at(demo.started)
    })
}
