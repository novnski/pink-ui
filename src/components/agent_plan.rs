//! Agent Plan – the steps an agent proposes, shaped by you before it runs.
//!
//! In **draft** the plan is a list you edit: double-click a title to rename
//! it, drag the grip to reorder, remove a step or add your own. **Run** turns
//! the same list into a live run: a segmented bar fills step by step, the
//! active step spins and counts up, and a failed step unfolds with its error,
//! a "Retry from here" and a "Skip".
//!
//! ```ignore
//! AgentPlan::new("rotate", "Rotate staging database credentials")
//!     .step(PlanStep::new("Snapshot the staging database").tool("db.snapshot").estimate(3.0))
//!     .step(PlanStep::new("Restart services one by one").tool("deploy.rollout").estimate(4.0))
//!     .on_run(|steps, _, _| println!("running {} steps", steps.len()))
//! ```
//!
//! The run is simulated from each step's estimate (use [`AgentPlan::fail_once`]
//! to rehearse a failure), so the component works on its own in a demo.

use std::{
    f32::consts::{PI, TAU},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, App, ClickEvent, ElementId, Entity, FontWeight,
    Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, PathBuilder, Pixels,
    SharedString, Subscription, Task, Window, canvas, div, point, prelude::*, px, relative, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, EASE_OUT_BACK, GLIDE, LINEAR, MotionSpec, ROW_IN},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Agent Plan";
pub const DESCRIPTION: &str = "Shape an agent's plan before it starts – rename, reorder, drop or add steps – then follow the run step by step and pick it up again from a failed step";

const ICON_PLAN: &str = "icons/agent-plan/plan.svg";
const ICON_GRIP: &str = "icons/agent-plan/grip.svg";
const ICON_PLUS: &str = "icons/agent-plan/plus.svg";
const ICON_CLOSE: &str = "icons/agent-plan/close.svg";
const ICON_CHECK: &str = "icons/agent-plan/check.svg";
const ICON_PLAY: &str = "icons/agent-plan/play.svg";
const ICON_RETRY: &str = "icons/agent-plan/retry.svg";
const ICON_ALERT: &str = "icons/agent-plan/alert.svg";
const ICON_TOOL: &str = "icons/agent-plan/tool.svg";
const ICON_SKIP: &str = "icons/agent-plan/skip.svg";
const ICON_STOP: &str = "icons/agent-plan/stop.svg";

/// Height of a step row; a failed row unfolds by [`FAIL_EXTRA`].
const ROW_H: f32 = 56.0;
const FAIL_EXTRA: f32 = 36.0;
/// Width of the plan card.
const WIDTH: f32 = 560.0;
/// How long the demo waits before it acts on its own.
const AUTO_RUN: Duration = Duration::from_millis(2600);
const AUTO_RETRY: Duration = Duration::from_millis(3400);
const AUTO_RESET: Duration = Duration::from_millis(5200);

type StepsHandler = Rc<dyn Fn(&[PlanStep], &mut Window, &mut App)>;

/// One step of the plan.
#[derive(Clone, Debug)]
pub struct PlanStep {
    pub title: SharedString,
    /// The tool the agent will call, shown in monospace under the title.
    pub tool: SharedString,
    /// Expected run time in seconds.
    pub estimate: f32,
}

impl PlanStep {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            tool: "manual".into(),
            estimate: 3.0,
        }
    }

    pub fn tool(mut self, tool: impl Into<SharedString>) -> Self {
        self.tool = tool.into();
        self
    }

    pub fn estimate(mut self, seconds: f32) -> Self {
        self.estimate = seconds.max(0.5);
        self
    }
}

/// Where a step is in the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
}

#[derive(IntoElement)]
pub struct AgentPlan {
    id: ElementId,
    title: SharedString,
    steps: Vec<PlanStep>,
    autoplay: bool,
    failure: Option<(usize, SharedString)>,
    on_run: Option<StepsHandler>,
    on_change: Option<StepsHandler>,
}

impl AgentPlan {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            steps: Vec::new(),
            autoplay: false,
            failure: None,
            on_run: None,
            on_change: None,
        }
    }

    /// Appends a step to the initial plan.
    pub fn step(mut self, step: PlanStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn steps(mut self, steps: impl IntoIterator<Item = PlanStep>) -> Self {
        self.steps.extend(steps);
        self
    }

    /// Plays the demo on its own – run, retry, reset – until someone touches it.
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    /// The step at `index` fails with `message` the first time it runs.
    pub fn fail_once(mut self, index: usize, message: impl Into<SharedString>) -> Self {
        self.failure = Some((index, message.into()));
        self
    }

    /// Called with the plan as it stands when the run starts.
    pub fn on_run(
        mut self,
        handler: impl Fn(&[PlanStep], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }

    /// Called after every edit: rename, reorder, remove or add.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[PlanStep], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Draft,
    Running,
    Failed,
    Done,
}

struct Step {
    /// Stable key for element ids and followers.
    key: usize,
    plan: PlanStep,
    status: StepStatus,
    started: Option<Instant>,
    elapsed: f32,
    error: Option<SharedString>,
}

impl Step {
    /// Simulated duration: the estimate with a little per-step variance.
    fn duration(&self) -> f32 {
        self.plan.estimate * (0.82 + ((self.key * 37) % 29) as f32 / 100.0)
    }
}

struct Drag {
    key: usize,
    start_mouse: Pixels,
    start_top: f32,
    top: f32,
}

struct PlanState {
    steps: Vec<Step>,
    phase: Phase,
    phase_since: Instant,
    run_started: Option<Instant>,
    run_elapsed: f32,
    editing: Option<usize>,
    rename: Entity<TextInput>,
    adding: bool,
    add: Entity<TextInput>,
    drag: Option<Drag>,
    next_key: usize,
    /// (step key, message): fails once, then clears.
    failure: Option<(usize, SharedString)>,
    autoplay: bool,
    on_run: Option<StepsHandler>,
    on_change: Option<StepsHandler>,
    _subscriptions: Vec<Subscription>,
    _ticker: Task<()>,
}

impl PlanState {
    fn new(
        steps: Vec<PlanStep>,
        failure: Option<(usize, SharedString)>,
        autoplay: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rename = cx.new(|cx| TextInput::new(window, cx));
        let add = cx.new(|cx| TextInput::new(window, cx).placeholder("Describe the step…"));
        let subscriptions = vec![
            cx.subscribe_in(
                &rename,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Submit(_) | InputEvent::Blur => this.commit_rename(window, cx),
                    InputEvent::Escape => {
                        this.editing = None;
                        cx.notify();
                    }
                    _ => {}
                },
            ),
            cx.subscribe_in(
                &add,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Submit(text) => {
                        this.add_step(text.clone(), window, cx);
                    }
                    InputEvent::Escape => {
                        this.adding = false;
                        this.add.update(cx, |input, cx| input.clear(cx));
                        cx.notify();
                    }
                    InputEvent::Blur if this.add.read(cx).is_empty() => {
                        this.adding = false;
                        cx.notify();
                    }
                    _ => {}
                },
            ),
        ];
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if this.update(cx, |state, cx| state.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let count = steps.len();
        Self {
            steps: steps
                .into_iter()
                .enumerate()
                .map(|(key, plan)| Step {
                    key,
                    plan,
                    status: StepStatus::Pending,
                    started: None,
                    elapsed: 0.0,
                    error: None,
                })
                .collect(),
            phase: Phase::Draft,
            phase_since: Instant::now(),
            run_started: None,
            run_elapsed: 0.0,
            editing: None,
            rename,
            adding: false,
            add,
            drag: None,
            next_key: count,
            failure,
            autoplay,
            on_run: None,
            on_change: None,
            _subscriptions: subscriptions,
            _ticker: ticker,
        }
    }

    fn plan(&self) -> Vec<PlanStep> {
        self.steps.iter().map(|s| s.plan.clone()).collect()
    }

    fn changed(&self, window: &mut Window, cx: &mut App) {
        if let Some(handler) = self.on_change.clone() {
            handler(&self.plan(), window, cx);
        }
    }

    fn set_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.phase_since = Instant::now();
    }

    /// Any manual input hands the demo over to the person.
    fn take_over(&mut self) {
        self.autoplay = false;
    }

    fn start_step(&mut self, ix: usize) {
        let step = &mut self.steps[ix];
        step.status = StepStatus::Running;
        step.started = Some(Instant::now());
        step.elapsed = 0.0;
        step.error = None;
    }

    fn start_run(&mut self) {
        if self.steps.is_empty() {
            return;
        }
        self.editing = None;
        self.adding = false;
        self.drag = None;
        for step in &mut self.steps {
            step.status = StepStatus::Pending;
            step.elapsed = 0.0;
            step.error = None;
        }
        self.run_started = Some(Instant::now());
        self.run_elapsed = 0.0;
        self.set_phase(Phase::Running);
        self.start_step(0);
    }

    /// Resets every step from `ix` on and runs again from there.
    fn retry_from(&mut self, ix: usize) {
        for step in &mut self.steps[ix..] {
            step.status = StepStatus::Pending;
            step.elapsed = 0.0;
            step.error = None;
        }
        self.set_phase(Phase::Running);
        self.start_step(ix);
    }

    fn skip(&mut self, ix: usize) {
        self.steps[ix].status = StepStatus::Skipped;
        self.steps[ix].error = None;
        self.advance_from(ix + 1);
    }

    fn advance_from(&mut self, ix: usize) {
        if ix < self.steps.len() {
            self.set_phase(Phase::Running);
            self.start_step(ix);
        } else {
            self.run_elapsed = self
                .run_started
                .map(|s| s.elapsed().as_secs_f32())
                .unwrap_or(0.0);
            self.set_phase(Phase::Done);
        }
    }

    fn back_to_draft(&mut self) {
        for step in &mut self.steps {
            step.status = StepStatus::Pending;
            step.elapsed = 0.0;
            step.started = None;
            step.error = None;
        }
        self.run_started = None;
        self.set_phase(Phase::Draft);
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let since = now.saturating_duration_since(self.phase_since);
        match self.phase {
            Phase::Draft => {
                if self.autoplay && since >= AUTO_RUN && self.editing.is_none() {
                    self.start_run();
                    cx.notify();
                }
            }
            Phase::Running => {
                if let Some(ix) = self
                    .steps
                    .iter()
                    .position(|s| s.status == StepStatus::Running)
                {
                    let duration = self.steps[ix].duration();
                    let key = self.steps[ix].key;
                    let step = &mut self.steps[ix];
                    step.elapsed = step
                        .started
                        .map(|s| now.saturating_duration_since(s).as_secs_f32())
                        .unwrap_or(0.0);
                    let fails = self.failure.as_ref().is_some_and(|(k, _)| *k == key);
                    if fails && step.elapsed >= duration * 0.7 {
                        let (_, message) = self.failure.take().unwrap_or_default();
                        let step = &mut self.steps[ix];
                        step.status = StepStatus::Failed;
                        step.error = Some(message);
                        self.set_phase(Phase::Failed);
                    } else if step.elapsed >= duration {
                        step.elapsed = duration;
                        step.status = StepStatus::Done;
                        self.advance_from(ix + 1);
                    }
                }
                cx.notify();
            }
            Phase::Failed => {
                if self.autoplay && since >= AUTO_RETRY {
                    if let Some(ix) = self
                        .steps
                        .iter()
                        .position(|s| s.status == StepStatus::Failed)
                    {
                        self.retry_from(ix);
                    }
                    cx.notify();
                }
            }
            Phase::Done => {
                if self.autoplay && since >= AUTO_RESET {
                    self.back_to_draft();
                    cx.notify();
                }
            }
        }
    }

    fn begin_rename(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(step) = self.steps.iter().find(|s| s.key == key) else {
            return;
        };
        let title = step.plan.title.clone();
        self.editing = Some(key);
        self.rename
            .update(cx, |input, cx| input.set_text(title, cx));
        self.rename.read(cx).focus(window);
        cx.notify();
    }

    fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.editing.take() else {
            return;
        };
        let text = self.rename.read(cx).text();
        let text = text.trim();
        if !text.is_empty()
            && let Some(step) = self.steps.iter_mut().find(|s| s.key == key)
            && step.plan.title != text
        {
            step.plan.title = SharedString::from(text.to_string());
            self.changed(window, cx);
        }
        cx.notify();
    }

    fn add_step(&mut self, text: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let key = self.next_key;
        self.next_key += 1;
        self.steps.push(Step {
            key,
            plan: PlanStep::new(text.to_string())
                .tool("you.note")
                .estimate(2.4),
            status: StepStatus::Pending,
            started: None,
            elapsed: 0.0,
            error: None,
        });
        self.add.update(cx, |input, cx| input.clear(cx));
        self.changed(window, cx);
        cx.notify();
    }

    fn remove(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.steps.retain(|s| s.key != key);
        self.changed(window, cx);
        cx.notify();
    }

    /// Row tops and heights in the current order.
    fn layout(&self) -> Vec<(usize, f32, f32)> {
        let mut top = 0.0;
        self.steps
            .iter()
            .map(|step| {
                let h = row_height(step);
                let entry = (step.key, top, h);
                top += h;
                entry
            })
            .collect()
    }

    /// Moves the dragged row to follow the pointer and reorders around it.
    fn drag_to(&mut self, mouse_y: Pixels, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let total: f32 = self.steps.iter().map(row_height).sum();
        let top = (drag.start_top + f32::from(mouse_y - drag.start_mouse))
            .clamp(-ROW_H * 0.4, total - ROW_H * 0.6);
        drag.top = top;
        let key = drag.key;
        let Some(from) = self.steps.iter().position(|s| s.key == key) else {
            return;
        };
        let center = top + ROW_H / 2.0;
        let mut acc = 0.0;
        let mut to = 0;
        for step in self.steps.iter().filter(|s| s.key != key) {
            let h = row_height(step);
            if acc + h / 2.0 < center {
                to += 1;
            }
            acc += h;
        }
        if to != from {
            let step = self.steps.remove(from);
            self.steps.insert(to, step);
        }
        cx.notify();
    }

    fn drop_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take() {
            let moved = drag.top != drag.start_top;
            if moved {
                self.changed(window, cx);
            }
            cx.notify();
        }
    }
}

fn row_height(step: &Step) -> f32 {
    if step.status == StepStatus::Failed {
        ROW_H + FAIL_EXTRA
    } else {
        ROW_H
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn seconds(value: f32) -> String {
    if value < 60.0 {
        format!("{value:.1}s")
    } else {
        format!("{}m {:02}s", (value / 60.0) as u32, (value % 60.0) as u32)
    }
}

/// A ring with a sweeping arc: the running step's node.
fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla, track: Hsla) -> impl IntoElement {
    div().size(px(size)).flex_none().with_animation(
        id,
        Animation::new(Duration::from_millis(900)).repeat(),
        move |el, t| {
            el.child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let c = bounds.center();
                        let r = f32::from(bounds.size.width) / 2.0 - 1.25;
                        let at = |a: f32| point(c.x + px(r * a.cos()), c.y + px(r * a.sin()));
                        let mut ring = PathBuilder::stroke(px(1.5));
                        ring.move_to(at(0.0));
                        ring.arc_to(point(px(r), px(r)), px(0.), false, true, at(PI));
                        ring.arc_to(point(px(r), px(r)), px(0.), false, true, at(TAU));
                        if let Ok(path) = ring.build() {
                            window.paint_path(path, track);
                        }
                        let start = t * TAU - PI / 2.0;
                        let mut arc = PathBuilder::stroke(px(1.75));
                        arc.move_to(at(start));
                        arc.arc_to(
                            point(px(r), px(r)),
                            px(0.),
                            false,
                            true,
                            at(start + PI * 0.6),
                        );
                        if let Ok(path) = arc.build() {
                            window.paint_path(path, color);
                        }
                    },
                )
                .size_full(),
            )
        },
    )
}

/// The status node in front of each step.
fn node(step: &Step, index: usize, phase: Phase, key: &str, t: &Luna) -> impl IntoElement {
    let size = 20.0;
    let frame = || {
        div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
    };
    match step.status {
        StepStatus::Running => spinner(
            ElementId::Name(format!("{key}-spin-{}", step.key).into()),
            size,
            t.accent,
            t.fill(FillTier::Primary),
        )
        .into_any_element(),
        StepStatus::Done => frame()
            .bg(t.accent)
            .child(svg().path(ICON_CHECK).size(px(14.)).text_color(t.on_accent))
            .with_animation(
                ElementId::Name(format!("{key}-done-{}", step.key).into()),
                MotionSpec::new(320, EASE_OUT_BACK).animation(),
                |el, p| el.opacity(p.min(1.0)),
            )
            .into_any_element(),
        StepStatus::Failed => frame()
            .bg(t.danger_strong)
            .child(svg().path(ICON_ALERT).size(px(16.)).text_color(t.on_accent))
            .into_any_element(),
        StepStatus::Skipped => frame()
            .border_1()
            .border_color(t.border_strong)
            .child(svg().path(ICON_SKIP).size(px(12.)).text_color(t.text_faint))
            .into_any_element(),
        StepStatus::Pending => frame()
            .border_1()
            .border_color(if phase == Phase::Draft {
                t.border_strong
            } else {
                t.border
            })
            .text_size(px(text::CAPTION))
            .font_weight(FontWeight::MEDIUM)
            .text_color(if phase == Phase::Draft {
                t.text_muted
            } else {
                t.text_faint
            })
            .child(format!("{}", index + 1))
            .into_any_element(),
    }
}

impl RenderOnce for AgentPlan {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let initial = self.steps.clone();
        let failure = self.failure.clone();
        let autoplay = self.autoplay;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "plan".into()),
            cx,
            move |window, cx| PlanState::new(initial, failure, autoplay, window, cx),
        );
        let (on_run, on_change) = (self.on_run.clone(), self.on_change.clone());
        state.update(cx, |s, _| {
            s.on_run = on_run;
            s.on_change = on_change;
        });

        let s = state.read(cx);
        let phase = s.phase;
        let draft = phase == Phase::Draft;
        let total = s.steps.len();
        let done_count = s
            .steps
            .iter()
            .filter(|st| matches!(st.status, StepStatus::Done | StepStatus::Skipped))
            .count();
        let estimate: f32 = s.steps.iter().map(|st| st.plan.estimate).sum();
        let run_elapsed = match phase {
            Phase::Done => s.run_elapsed,
            _ => s
                .run_started
                .map(|r| r.elapsed().as_secs_f32())
                .unwrap_or(0.0),
        };
        let running_ix = s
            .steps
            .iter()
            .position(|st| st.status == StepStatus::Running);
        let failed_ix = s
            .steps
            .iter()
            .position(|st| st.status == StepStatus::Failed);

        // -- Header ---------------------------------------------------------
        let (badge_label, badge_tone) = match phase {
            Phase::Draft => ("Draft", Tone::Neutral),
            Phase::Running => ("Running", Tone::Accent),
            Phase::Failed => ("Needs attention", Tone::Danger),
            Phase::Done => ("Completed", Tone::Success),
        };
        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S12))
            .px(px(space::S16))
            .pt(px(space::S16))
            .pb(px(space::S12))
            .child(
                div()
                    .size(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::MD))
                    .bg(t.accent_wash)
                    .child(svg().path(ICON_PLAN).size(px(16.)).text_color(if t.dark {
                        t.text
                    } else {
                        t.accent
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::CALLOUT))
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text)
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(t.text_muted)
                            .child(format!(
                                "{total} {} · est. {}",
                                if total == 1 { "step" } else { "steps" },
                                seconds(estimate)
                            )),
                    ),
            )
            .child(luna::badge(badge_label, badge_tone, &t));

        // -- Segmented progress --------------------------------------------
        let complete = motion::follow(
            &format!("{key}-complete"),
            if phase == Phase::Done { 1.0 } else { 0.0 },
            GLIDE,
        );
        let segment_color = motion::mix(t.accent, t.success, complete);
        let segments = s.steps.iter().map(|st| {
            let target = match st.status {
                StepStatus::Done | StepStatus::Failed | StepStatus::Skipped => 1.0,
                StepStatus::Running => (st.elapsed / st.duration()).clamp(0.0, 1.0),
                StepStatus::Pending => 0.0,
            };
            let spec = if st.status == StepStatus::Running {
                MotionSpec::new(120, LINEAR)
            } else {
                MotionSpec::new(260, motion::EASE_OUT)
            };
            let frac = motion::follow(&format!("{key}-seg-{}", st.key), target, spec);
            let color = match st.status {
                StepStatus::Failed => t.danger,
                StepStatus::Skipped => t.text_faint,
                _ => segment_color,
            };
            div()
                .flex_1()
                .h(px(4.))
                .rounded(px(2.))
                .overflow_hidden()
                .bg(t.fill(FillTier::Secondary))
                .child(div().h_full().w(relative(frac)).rounded(px(2.)).bg(color))
        });
        let progress = div()
            .flex()
            .gap(px(3.))
            .px(px(space::S16))
            .pb(px(space::S12))
            .children(segments);

        // -- Steps ------------------------------------------------------------
        let layout = s.layout();
        let list_h: f32 = layout.iter().map(|(_, _, h)| h).sum();
        let list_h = motion::follow(&format!("{key}-list-h"), list_h, GLIDE);
        let drag_key = s.drag.as_ref().map(|d| d.key);
        let drag_top = s.drag.as_ref().map(|d| d.top).unwrap_or(0.0);
        let editing = s.editing;
        let rename_input = s.rename.clone();

        let mut rows = Vec::new();
        let mut dragged_row = None;
        for (index, (step, (_, top, height))) in s.steps.iter().zip(layout).enumerate() {
            let row_key = format!("{key}-row-{}", step.key);
            let dragging = drag_key == Some(step.key);
            let y = if dragging {
                motion::follow(
                    &format!("{row_key}-y"),
                    drag_top,
                    MotionSpec::new(0, LINEAR),
                )
            } else {
                motion::follow(&format!("{row_key}-y"), top, GLIDE)
            };
            let h = motion::follow(&format!("{row_key}-h"), height, GLIDE);
            let hover = motion::hover_t(&row_key);
            let is_last = index + 1 == total;
            let step_key = step.key;

            // Grip: only while drafting, brightening with the row.
            let grip = div()
                .id(ElementId::Name(format!("{row_key}-grip").into()))
                .w(px(14.))
                .h(px(20.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .when(draft, |el| {
                    el.cursor_grab()
                        .opacity(if dragging { 1.0 } else { hover })
                        .on_mouse_down(MouseButton::Left, {
                            let state = state.clone();
                            move |event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                state.update(cx, |s, cx| {
                                    s.take_over();
                                    s.editing = None;
                                    s.drag = Some(Drag {
                                        key: step_key,
                                        start_mouse: event.position.y,
                                        start_top: top,
                                        top,
                                    });
                                    cx.notify();
                                });
                            }
                        })
                })
                .when(!draft, |el| el.opacity(0.))
                .child(svg().path(ICON_GRIP).size(px(14.)).text_color(t.text_faint));

            let title_color = match step.status {
                StepStatus::Pending if !draft => t.text_muted,
                StepStatus::Skipped => t.text_faint,
                _ => t.text,
            };
            let title: AnyElement = if editing == Some(step.key) {
                div()
                    .flex_1()
                    .h(px(24.))
                    .mt(px(-2.))
                    .mb(px(-2.))
                    .ml(px(-6.))
                    .px(px(6.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::SM))
                    .bg(t.input_bg)
                    .border_1()
                    .border_color(t.focus_ring())
                    .text_size(px(text::BODY))
                    .text_color(t.text)
                    .child(rename_input.clone())
                    .into_any_element()
            } else {
                div()
                    .id(ElementId::Name(format!("{row_key}-title").into()))
                    .w_full()
                    .truncate()
                    .text_size(px(text::BODY))
                    .line_height(px(20.))
                    .font_weight(if step.status == StepStatus::Running {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(title_color)
                    .when(step.status == StepStatus::Skipped, |el| el.line_through())
                    .child(step.plan.title.clone())
                    .when(draft, |el| {
                        el.cursor_text().on_click({
                            let state = state.clone();
                            move |event: &ClickEvent, window, cx| {
                                if event.click_count() >= 2 {
                                    state.update(cx, |s, cx| {
                                        s.take_over();
                                        s.begin_rename(step_key, window, cx)
                                    });
                                }
                            }
                        })
                    })
                    .into_any_element()
            };

            let subline = match (&step.error, step.status) {
                (Some(error), StepStatus::Failed) => div()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .text_size(px(text::FOOTNOTE))
                    .line_height(px(16.))
                    .text_color(t.danger)
                    .child(div().truncate().child(error.clone())),
                _ => div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .text_size(px(text::CAPTION))
                    .line_height(px(16.))
                    .text_color(t.text_faint)
                    .child(svg().path(ICON_TOOL).size(px(12.)).text_color(t.text_faint))
                    .child(div().font_family(luna::MONO).child(step.plan.tool.clone())),
            };

            let actions = (step.status == StepStatus::Failed).then(|| {
                let ix = index;
                div()
                    .flex()
                    .gap(px(space::S6))
                    .mt(px(space::S10))
                    .child(
                        luna::button(
                            format!("{row_key}-retry"),
                            ButtonKind::Secondary,
                            ControlSize::Sm,
                            &t,
                        )
                        .child(luna::glyph(ICON_RETRY, ControlSize::Sm).text_color(t.text))
                        .child("Retry from here")
                        .on_click({
                            let state = state.clone();
                            move |_, _, cx| {
                                state.update(cx, |s, cx| {
                                    s.take_over();
                                    s.retry_from(ix);
                                    cx.notify();
                                })
                            }
                        }),
                    )
                    .child(
                        luna::button(
                            format!("{row_key}-skip"),
                            ButtonKind::Ghost,
                            ControlSize::Sm,
                            &t,
                        )
                        .child("Skip step")
                        .on_click({
                            let state = state.clone();
                            move |_, _, cx| {
                                state.update(cx, |s, cx| {
                                    s.take_over();
                                    s.skip(ix);
                                    cx.notify();
                                })
                            }
                        }),
                    )
            });

            let time_label = match step.status {
                StepStatus::Running | StepStatus::Done | StepStatus::Failed => {
                    Some((seconds(step.elapsed), step.status == StepStatus::Running))
                }
                StepStatus::Pending => Some((format!("~{:.0}s", step.plan.estimate), false)),
                StepStatus::Skipped => None,
            };
            let right = div()
                .flex()
                .items_center()
                .gap(px(space::S2))
                .h(px(20.))
                .when_some(time_label, |el, (label, live)| {
                    el.child(
                        div()
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(if live { t.text } else { t.text_faint })
                            .child(label),
                    )
                })
                .when(draft, |el| {
                    el.child(
                        div().opacity(hover).child(
                            luna::icon_button(
                                format!("{row_key}-remove"),
                                ICON_CLOSE,
                                ControlSize::Sm,
                                &t,
                            )
                            .on_click({
                                let state = state.clone();
                                move |_, window, cx| {
                                    state.update(cx, |s, cx| {
                                        s.take_over();
                                        s.remove(step_key, window, cx)
                                    })
                                }
                            }),
                        ),
                    )
                });

            let content = div()
                .flex()
                .items_start()
                .gap(px(space::S10))
                .px(px(space::S8))
                .pt(px(space::S10))
                .child(grip)
                .child(node(step, index, phase, &key, &t))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(space::S2))
                        .child(title)
                        .child(subline)
                        .children(actions),
                )
                .child(right);

            let row = div()
                .id(ElementId::Name(row_key.clone().into()))
                .absolute()
                .left_0()
                .right_0()
                .top(px(y))
                .h(px(h))
                .overflow_hidden()
                .rounded(px(radius::MD))
                .when(dragging, |el| {
                    el.bg(t.surface_overlay)
                        .border_1()
                        .border_color(t.border)
                        .shadow(t.menu_shadow())
                })
                .when(!dragging && draft, |el| {
                    el.hover_fill(
                        row_key.clone(),
                        t.fill(FillTier::Quaternary).opacity(0.),
                        t.fill(FillTier::Quaternary),
                    )
                })
                .when(step.status == StepStatus::Failed, |el| {
                    el.bg(t.danger.opacity(if t.dark { 0.08 } else { 0.05 }))
                })
                .child(motion::rise_in(
                    ElementId::Name(format!("{row_key}-in").into()),
                    ROW_IN.with_delay(40 * index.min(8) as u64),
                    6.0,
                    content,
                ))
                // Hairline between rows, from the title column to the edge.
                .when(!is_last && !dragging, |el| {
                    el.child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left(px(62.))
                            .right(px(8.))
                            .h(px(1.))
                            .bg(t.border),
                    )
                });
            if dragging {
                dragged_row = Some(row);
            } else {
                rows.push(row);
            }
        }

        let list = div()
            .relative()
            .mx(px(space::S8))
            .h(px(list_h))
            .children(rows)
            .children(dragged_row);

        // -- Add a step -------------------------------------------------------
        let add_row = draft.then(|| {
            let add_key = format!("{key}-add");
            if s.adding {
                div()
                    .id(ElementId::Name(add_key.into()))
                    .mx(px(space::S8))
                    .mt(px(space::S4))
                    .h(px(36.))
                    .px(px(space::S8))
                    .flex()
                    .items_center()
                    .gap(px(space::S10))
                    .rounded(px(radius::MD))
                    .bg(t.input_bg)
                    .border_1()
                    .border_color(t.focus_ring())
                    .child(div().w(px(14.)).flex_none())
                    .child(
                        div()
                            .size(px(20.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(svg().path(ICON_PLUS).size(px(14.)).text_color(t.accent)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(text::BODY))
                            .text_color(t.text)
                            .child(s.add.clone()),
                    )
                    .child(luna::kbd("↵", &t))
            } else {
                let tone = motion::hover_blend(&add_key, t.text_muted, t.text);
                div()
                    .id(ElementId::Name(add_key.clone().into()))
                    .mx(px(space::S8))
                    .mt(px(space::S4))
                    .h(px(36.))
                    .px(px(space::S8))
                    .flex()
                    .items_center()
                    .gap(px(space::S10))
                    .rounded(px(radius::MD))
                    .cursor_pointer()
                    .text_size(px(text::BODY))
                    .text_color(tone)
                    .hover_fill(
                        add_key,
                        t.fill(FillTier::Quaternary).opacity(0.),
                        t.fill(FillTier::Quaternary),
                    )
                    .on_click({
                        let state = state.clone();
                        move |_, window, cx| {
                            state.update(cx, |s, cx| {
                                s.take_over();
                                s.adding = true;
                                s.add.read(cx).focus(window);
                                cx.notify();
                            })
                        }
                    })
                    .child(div().w(px(14.)).flex_none())
                    .child(
                        div()
                            .size(px(20.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .border_1()
                            .border_color(t.border)
                            .child(svg().path(ICON_PLUS).size(px(12.)).text_color(tone)),
                    )
                    .child("Add a step")
            }
        });

        // -- Footer -----------------------------------------------------------
        let status_line: SharedString = match phase {
            Phase::Draft => "Double-click a step to rename it, drag the grip to reorder".into(),
            Phase::Running => format!(
                "Step {} of {total} · {}",
                running_ix.map_or(done_count, |i| i + 1),
                seconds(run_elapsed)
            )
            .into(),
            Phase::Failed => format!(
                "Stopped at step {} of {total}",
                failed_ix.map_or(0, |i| i + 1)
            )
            .into(),
            Phase::Done => format!("All {total} steps finished in {}", seconds(run_elapsed)).into(),
        };
        let footer_actions = match phase {
            Phase::Draft => vec![
                luna::button(
                    format!("{key}-run"),
                    ButtonKind::Accent,
                    ControlSize::Md,
                    &t,
                )
                .child(luna::glyph(ICON_PLAY, ControlSize::Md).text_color(t.on_accent))
                .child("Run plan")
                .on_click({
                    let state = state.clone();
                    move |_, window, cx| {
                        let handler = state.update(cx, |s, cx| {
                            s.take_over();
                            s.start_run();
                            cx.notify();
                            s.on_run.clone().map(|h| (h, s.plan()))
                        });
                        if let Some((handler, plan)) = handler {
                            handler(&plan, window, cx);
                        }
                    }
                }),
            ],
            Phase::Running => vec![
                luna::button(
                    format!("{key}-stop"),
                    ButtonKind::Secondary,
                    ControlSize::Md,
                    &t,
                )
                .child(luna::glyph(ICON_STOP, ControlSize::Md).text_color(t.text))
                .child("Stop")
                .on_click({
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.take_over();
                            s.back_to_draft();
                            cx.notify();
                        })
                    }
                }),
            ],
            Phase::Failed | Phase::Done => vec![
                luna::button(
                    format!("{key}-edit"),
                    ButtonKind::Secondary,
                    ControlSize::Md,
                    &t,
                )
                .child("Edit plan")
                .on_click({
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.take_over();
                            s.back_to_draft();
                            cx.notify();
                        })
                    }
                }),
            ],
        };
        let footer = div()
            .flex()
            .items_center()
            .gap(px(space::S12))
            .mt(px(space::S8))
            .px(px(space::S16))
            .py(px(space::S12))
            .border_t_1()
            .border_color(t.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .child(status_line),
            )
            .children(footer_actions);

        let dragging = s.drag.is_some();
        // Followers started above need frames of their own: this render runs
        // after the story's `keep_alive`.
        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .w(px(WIDTH))
            .font_family(luna::SANS)
            .text_color(t.text)
            .when(dragging, |el| el.cursor_grabbing())
            .on_mouse_move({
                let state = state.clone();
                move |event: &MouseMoveEvent, window, cx| {
                    state.update(cx, |s, cx| {
                        if s.drag.is_some() {
                            if event.pressed_button == Some(MouseButton::Left) {
                                s.drag_to(event.position.y, cx);
                            } else {
                                s.drop_drag(window, cx);
                            }
                        }
                    })
                }
            })
            .on_mouse_up(MouseButton::Left, {
                let state = state.clone();
                move |_, window, cx| state.update(cx, |s, cx| s.drop_drag(window, cx))
            })
            .on_mouse_up_out(MouseButton::Left, {
                let state = state.clone();
                move |_, window, cx| state.update(cx, |s, cx| s.drop_drag(window, cx))
            })
            .child(header)
            .child(progress)
            .child(luna::divider(&t))
            .child(div().pt(px(space::S6)).child(list))
            .children(add_row)
            .child(footer)
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentPlan::new("agent-plan", "Rotate staging database credentials")
            .step(
                PlanStep::new("Snapshot the staging database")
                    .tool("db.snapshot")
                    .estimate(3.0),
            )
            .step(
                PlanStep::new("Mint new credentials in the vault")
                    .tool("vault.write")
                    .estimate(2.0),
            )
            .step(
                PlanStep::new("Point six service configs at the new secret")
                    .tool("repo.edit")
                    .estimate(3.4),
            )
            .step(
                PlanStep::new("Restart services one at a time")
                    .tool("deploy.rollout")
                    .estimate(3.8),
            )
            .step(
                PlanStep::new("Verify connections and revoke the old keys")
                    .tool("db.query")
                    .estimate(2.6),
            )
            .fail_once(3, "billing-api missed its health check after the restart")
            .autoplay(true)
    })
}
