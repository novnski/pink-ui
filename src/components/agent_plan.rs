//! Agent Plan – the steps an agent proposes, shaped by you, then followed
//! as they run. The plan rides on the composer, the way Pink stacks its own
//! pieces there (`composer.rs`: checklist, notices and the queue tray sit in
//! the column above the pill).
//!
//! In **draft** the plan is a tray emerging from behind the composer, drawn
//! like Pink's message queue (`queue.rs`) under the goal card's header bar
//! (`composer/goals.rs`): drag a row to reorder it, rename a
//! step with the pen (or a double-click), remove one, add your own, then
//! **Run plan**. The run is Pink's provider checklist
//! (`composer/checklist.rs`): one quiet "2/5 · In progress: …" line that
//! unfolds into a "Status · step" line per step. A failed step raises Pink's
//! composer notice (`notice.rs`) with "Retry from here" and "Skip step"; the
//! checklist's close button ends the run and hands the plan back to you.
//!
//! ```ignore
//! AgentPlan::new("rotate", "Rotate staging database credentials")
//!     .step(PlanStep::new("Snapshot the staging database").tool("db.snapshot").estimate(3.0))
//!     .step(PlanStep::new("Restart services one by one").tool("deploy.rollout").estimate(4.0))
//!     .composer(my_composer)
//!     .on_run(|steps, _, _| println!("running {} steps", steps.len()))
//! ```
//!
//! The run is simulated from each step's estimate (use [`AgentPlan::fail_once`]
//! to rehearse a failure), so the component works on its own in a demo.

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyElement, AnyView, App, ClickEvent, ClipboardItem, Div, DragMoveEvent,
    ElementId, EmptyView, Entity, FontWeight, IntoElement, MouseButton, MouseDownEvent, Role,
    SharedString, Stateful, Subscription, Task, Window, div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, TAB_SLIDE},
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, layout, loaders::format_elapsed, radius,
        tabular_figures,
    },
    primitives::{InputEvent, TextInput},
    story,
};

#[allow(dead_code)]
mod package {
    include!("../../components/agent-plan/src/lib.rs");
}
use package as plan;
pub use package::{
    CHECKLIST_ROW_H, CLOSE, ChecklistStyle, HEADER_H, HeaderStyle, ROW_H, STACK_GAP, StepStatus,
    StepStyle, Summary, TRAY_INSET, TRAY_OVERLAP, add_label, checklist, checklist_row, close,
    controls, disclosure, drag_offsets, drop_index, field, header, header_summary, line, lines,
    move_step, note, slide_offset, slot, status_text, step_text, step_title, summary, ui_rems,
};

pub const NAME: &str = "Agent Plan";
pub const DESCRIPTION: &str = "An agent's plan stacked on the composer: reorder, rename, drop or add steps in a tray tucked behind the pill, then follow the run as a quiet 2 of 5 line that unfolds into one status per step, with a notice to retry or skip a step that fails";

const ICON_ARROW_DOWN: &str = "icons/agent-plan/alt-arrow-down.svg";
const ICON_ARROW_RIGHT: &str = "icons/agent-plan/alt-arrow-right.svg";
const ICON_ARROW_UP: &str = "icons/agent-plan/arrow-up.svg";
const ICON_CHECKLIST: &str = "icons/agent-plan/checklist.svg";
const ICON_CLOSE: &str = "icons/agent-plan/close.svg";
const ICON_COPY: &str = "icons/agent-plan/copy.svg";
const ICON_DANGER: &str = "icons/agent-plan/danger-triangle.svg";
const ICON_DRAG: &str = "icons/agent-plan/queue-drag-handle.svg";
const ICON_EDIT: &str = "icons/agent-plan/pen.svg";
const ICON_PLAY: &str = "icons/agent-plan/play.svg";
const ICON_PLUS: &str = "icons/agent-plan/plus.svg";
const ICON_REMOVE: &str = "icons/agent-plan/trash-bin-minimalistic.svg";
const ICON_SAVE: &str = "icons/agent-plan/queue-check.svg";
const ICON_CANCEL: &str = "icons/agent-plan/queue-close.svg";

/// Width of the composer column the plan stacks in.
const WIDTH: f32 = 600.0;
/// Tray rows' padding and corners (Pink `queue.rs`: `ROW_PAD_X`,
/// `ROW_RADIUS`, `QUEUE_ICON_SIZE`).
const ROW_PAD_X: f32 = 8.0;
const ROW_RADIUS: f32 = radius::MD;
const TRAY_ICON: f32 = 13.0;
/// How long the demo waits before it acts on its own.
const AUTO_RUN: Duration = Duration::from_millis(2600);
const AUTO_RETRY: Duration = Duration::from_millis(3400);
const AUTO_RESET: Duration = Duration::from_millis(5200);

type StepsHandler = Rc<dyn Fn(&[PlanStep], &mut Window, &mut App)>;

/// One step of the plan.
#[derive(Clone, Debug)]
pub struct PlanStep {
    pub title: SharedString,
    /// The tool the agent will call, shown under the title in the draft.
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

#[derive(IntoElement)]
pub struct AgentPlan {
    id: ElementId,
    title: SharedString,
    steps: Vec<PlanStep>,
    autoplay: bool,
    expanded: bool,
    failure: Option<(usize, SharedString)>,
    composer: Option<AnyElement>,
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
            expanded: false,
            failure: None,
            composer: None,
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

    /// The composer the plan stacks on: the draft tray tucks behind it, the
    /// checklist and notices sit above it.
    pub fn composer(mut self, composer: impl IntoElement) -> Self {
        self.composer = Some(composer.into_any_element());
        self
    }

    /// Opens the run's checklist unfolded (Pink starts it folded).
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
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
    /// Stable key for element ids.
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

    fn reset(&mut self) {
        self.status = StepStatus::Pending;
        self.started = None;
        self.elapsed = 0.0;
        self.error = None;
    }
}

/// A tray row being dragged (Pink `QueueDragPayload`), scoped to its plan.
struct StepDrag {
    plan: SharedString,
    from: usize,
}

/// Where the dragged row would land (Pink `QueueDragState`).
struct DragState {
    from: usize,
    over: usize,
    prev_over: usize,
    epoch: usize,
}

struct PlanState {
    steps: Vec<Step>,
    phase: Phase,
    phase_since: Instant,
    /// Run time banked while paused, plus the running stretch.
    run_clock: f32,
    run_resumed: Option<Instant>,
    /// Bumped when the plan swaps between the tray and the checklist, so
    /// the arriving piece fades in.
    epoch: usize,
    /// Bumped on every failure, so each notice fades in.
    failures: usize,
    expanded: bool,
    editing: Option<usize>,
    rename: Entity<TextInput>,
    adding: bool,
    add: Entity<TextInput>,
    drag: Option<DragState>,
    /// A row's height in px at the window's Interface size, for the drag.
    row_h: f32,
    next_key: usize,
    /// (step key, message): fails once, then clears; autoplay re-arms it.
    failure: Option<(usize, SharedString)>,
    rehearsal: Option<(usize, SharedString)>,
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
        expanded: bool,
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
                    InputEvent::Escape => this.cancel_rename(cx),
                    _ => {}
                },
            ),
            cx.subscribe_in(
                &add,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Submit(text) => this.add_step(text.clone(), window, cx),
                    InputEvent::Escape => this.cancel_add(cx),
                    InputEvent::Blur if this.adding => {
                        let text = this.add.read(cx).text();
                        this.add_step(text, window, cx);
                        this.cancel_add(cx);
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
            run_clock: 0.0,
            run_resumed: None,
            epoch: 0,
            failures: 0,
            expanded,
            editing: None,
            rename,
            adding: false,
            add,
            drag: None,
            row_h: ROW_H,
            next_key: count,
            rehearsal: failure.clone(),
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
        if (self.phase == Phase::Draft) != (phase == Phase::Draft) {
            self.epoch += 1;
        }
        self.phase = phase;
        self.phase_since = Instant::now();
    }

    /// Any manual input hands the demo over to the person.
    fn take_over(&mut self) {
        self.autoplay = false;
    }

    fn run_elapsed(&self) -> f32 {
        self.run_clock + self.run_resumed.map_or(0.0, |r| r.elapsed().as_secs_f32())
    }

    fn pause_clock(&mut self) {
        self.run_clock = self.run_elapsed();
        self.run_resumed = None;
    }

    fn resume_clock(&mut self) {
        if self.run_resumed.is_none() {
            self.run_resumed = Some(Instant::now());
        }
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
        self.steps.iter_mut().for_each(Step::reset);
        self.run_clock = 0.0;
        self.run_resumed = Some(Instant::now());
        self.set_phase(Phase::Running);
        self.start_step(0);
    }

    /// Resets every step from `ix` on and runs again from there.
    fn retry_from(&mut self, ix: usize) {
        self.steps[ix..].iter_mut().for_each(Step::reset);
        self.resume_clock();
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
            self.resume_clock();
            self.set_phase(Phase::Running);
            self.start_step(ix);
        } else {
            self.pause_clock();
            self.set_phase(Phase::Done);
        }
    }

    fn back_to_draft(&mut self) {
        self.steps.iter_mut().for_each(Step::reset);
        self.run_clock = 0.0;
        self.run_resumed = None;
        self.set_phase(Phase::Draft);
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let since = now.saturating_duration_since(self.phase_since);
        match self.phase {
            Phase::Draft => {
                let idle = self.editing.is_none() && !self.adding && self.drag.is_none();
                if self.autoplay && idle && since >= AUTO_RUN {
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
                        self.failures += 1;
                        self.pause_clock();
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
                    if let Some(ix) = self.failed_ix() {
                        self.retry_from(ix);
                    }
                    cx.notify();
                }
            }
            Phase::Done => {
                if self.autoplay && since >= AUTO_RESET {
                    // The loop rehearses its failure again on the next run.
                    self.failure = self.rehearsal.clone();
                    self.back_to_draft();
                    cx.notify();
                }
            }
        }
    }

    fn failed_ix(&self) -> Option<usize> {
        self.steps
            .iter()
            .position(|s| s.status == StepStatus::Failed)
    }

    fn begin_rename(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(step) = self.steps.iter().find(|s| s.key == key) else {
            return;
        };
        let title = step.plan.title.clone();
        self.adding = false;
        self.editing = Some(key);
        self.rename
            .update(cx, |input, cx| input.set_text(title, cx));
        TextInput::focus(&self.rename, window, cx);
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

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if self.editing.take().is_some() {
            cx.notify();
        }
    }

    fn begin_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing = None;
        self.adding = true;
        TextInput::focus(&self.add, window, cx);
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
            plan: PlanStep::new(text.to_string()).estimate(2.4),
            status: StepStatus::Pending,
            started: None,
            elapsed: 0.0,
            error: None,
        });
        self.add.update(cx, |input, cx| input.clear(cx));
        self.changed(window, cx);
        cx.notify();
    }

    fn cancel_add(&mut self, cx: &mut Context<Self>) {
        self.adding = false;
        self.add.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    fn remove(&mut self, key: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.steps.retain(|s| s.key != key);
        if self.editing == Some(key) {
            self.editing = None;
        }
        self.changed(window, cx);
        cx.notify();
    }

    /// Tracks the drop slot while a row is dragged (Pink
    /// `update_queue_drag_over`).
    fn drag_over(&mut self, from: usize, over: usize, cx: &mut Context<Self>) {
        match &mut self.drag {
            Some(drag) if drag.from == from => {
                if drag.over != over {
                    drag.prev_over = drag.over;
                    drag.over = over;
                    drag.epoch = drag.epoch.wrapping_add(1);
                    cx.notify();
                }
            }
            _ => {
                self.drag = Some(DragState {
                    from,
                    over,
                    prev_over: from,
                    epoch: 0,
                });
                cx.notify();
            }
        }
    }

    fn cancel_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.take().is_some() {
            cx.notify();
        }
    }

    /// Moves the row at `from` to `to` (Pink `move_queued`).
    fn move_step(&mut self, from: usize, to: usize, window: &mut Window, cx: &mut Context<Self>) {
        if plan::move_step(&mut self.steps, from, to) {
            self.changed(window, cx);
        }
        cx.notify();
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// The run as Pink's provider checklist (Pink: composer/checklist.rs
/// `Composer::render_checklist`), drawn by the package: a 28 disclosure row
/// reading "2 of 5, In progress: step" beside a 24 close tile, unfolding
/// into one "Status: step" line per step and a closing note, all 11.5
/// `text_muted`. This adds the buttons, roles and the scroller.
fn run_checklist(key: &str, s: &PlanState, state: &Entity<PlanState>, t: &Pink) -> Stateful<Div> {
    let ring = t.focus_ring();
    let hover = t.element_hover;
    let total = s.steps.len();
    let style = ChecklistStyle {
        text: t.text_muted,
        danger: t.readable_tone(t.danger, 0.9),
    };
    let sum = summary(s.steps.iter().map(|st| (st.status, st.plan.title.as_ref())));
    let expanded = s.expanded;
    let close_label = if s.phase == Phase::Running {
        "Stop the run and edit the plan"
    } else {
        "Close the run and edit the plan"
    };

    let row = checklist_row()
        .child(
            disclosure(
                &style,
                if expanded {
                    ICON_ARROW_DOWN
                } else {
                    ICON_ARROW_RIGHT
                },
                &sum,
            )
            .id(format!("{key}-expand"))
            .role(Role::Button)
            .tab_index(0)
            .tab_stop(true)
            .aria_label(format!(
                "{} plan checklist: {}",
                if expanded { "Collapse" } else { "Expand" },
                sum.label
            ))
            .aria_expanded(expanded)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .on_key_down(pink::swallow_activation_key)
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.take_over();
                        s.expanded = !s.expanded;
                        cx.notify();
                    })
                }
            }),
        )
        .child(
            close(&style, ICON_CLOSE)
                .id(format!("{key}-close"))
                .role(Role::Button)
                .tab_index(0)
                .tab_stop(true)
                .aria_label(close_label)
                .cursor_pointer()
                .hover(move |style| style.bg(hover))
                .focus_visible(move |style| pink::focus_ring(style, ring))
                .on_key_down(pink::swallow_activation_key)
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
        );

    let panel = checklist(&style).id(format!("{key}-checklist")).child(row);
    if !expanded {
        return panel;
    }
    let steps = s.steps.iter().enumerate().map(|(index, step)| {
        line(&style, step.status, &step.plan.title)
            .id(format!("{key}-checklist-step-{index}"))
            .role(Role::ListItem)
            .aria_label(format!("{}: {}", step.status.word(), step.plan.title))
    });
    let closing = match s.phase {
        Phase::Failed => format!(
            "Agent plan paused at step {} of {total}",
            s.failed_ix().map_or(0, |ix| ix + 1)
        ),
        Phase::Done => format!(
            "Agent plan finished in {}",
            format_elapsed(s.run_elapsed().round() as i64)
        ),
        _ => format!(
            "Agent plan running for {}",
            format_elapsed(s.run_elapsed().floor() as i64)
        ),
    };
    panel
        .child(
            lines()
                .id(format!("{key}-checklist-rows"))
                .role(Role::List)
                .aria_label("Plan steps")
                .overflow_y_scroll()
                .children(steps),
        )
        .child(note(closing).font_features(tabular_figures()))
}

/// A failed step's notice above the composer (Pink: notice.rs `notice_chip`,
/// the danger variant, docked `mx 4` like the composer's failure notice):
/// an opaque tint, the triangle, a medium title, capsule actions and a copy
/// button on the title row, the error under it.
fn failure_notice(key: &str, s: &PlanState, state: &Entity<PlanState>, t: &Pink) -> Option<Div> {
    let ix = s.failed_ix()?;
    let message = s.steps[ix].error.clone().unwrap_or_default();
    let tone = t.danger;
    let copy = t.readable_tone(t.danger_muted, 0.9);
    let plate = pink::flatten(tone.opacity(0.08), t.canvas());
    let copy_key = format!("{key}-notice-copy");
    let copied = message.clone();
    Some(
        div()
            .mx(px(4.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .rounded(px(radius::STACK_NOTICE))
            .border_1()
            .border_color(tone.opacity(0.22))
            .bg(plate)
            .px(px(10.))
            .py(px(8.))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(copy)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        svg()
                            .path(ICON_DANGER)
                            .flex_none()
                            .size(px(14.))
                            .text_color(copy),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("Step {} failed", ix + 1)),
                    )
                    .child(div().flex_1())
                    .child(
                        pink::notice_action(format!("{key}-retry"), t)
                            .aria_label("Retry the plan from this step")
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
                        pink::notice_action(format!("{key}-skip"), t)
                            .aria_label("Skip this step")
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
                    .child(
                        div()
                            .id(copy_key.clone())
                            .role(Role::Button)
                            .aria_label("Copy the error")
                            .flex_none()
                            .size(px(pink::chrome::MIN_HIT))
                            .rounded(px(radius::SM))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .hover_fill(copy_key, tone.opacity(0.0), tone.opacity(0.12))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    copied.to_string(),
                                ));
                            })
                            .child(
                                svg()
                                    .path(ICON_COPY)
                                    .flex_none()
                                    .size(px(12.))
                                    .text_color(t.readable_tone(t.danger_muted, 0.8)),
                            ),
                    ),
            )
            .child(div().min_w_0().w_full().child(message)),
    )
}

/// A tray row's glyph action (Pink: queue.rs `queue_action`): a 28 circle,
/// transparent → `fill(Tertiary)`, the 13 glyph brightening to `text` with
/// it, a tooltip after the usual delay. Callers add the handlers.
fn tray_action(
    row_key: &str,
    slot: &str,
    label: &'static str,
    glyph: &'static str,
    t: &Pink,
) -> Stateful<Div> {
    let key = format!("{row_key}-{slot}");
    let group = format!("{key}-grp");
    let ring = t.focus_ring();
    let wash = t.fill(FillTier::Tertiary);
    let text = t.text;
    div()
        .id(key.clone())
        .group(group.clone())
        .role(Role::Button)
        .aria_label(label)
        .size(px(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .hover_fill_fg(key, wash.opacity(0.0), wash, t.content_muted, t.text)
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_key_down(pink::swallow_activation_key)
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            svg()
                .path(glyph)
                .flex_none()
                .size(px(TRAY_ICON))
                .text_color(t.readable_muted(0.8))
                .group_hover(group, move |style| style.text_color(text)),
        )
}

/// A row frame shared by the tray's step and add rows (Pink `queue_row`):
/// 36 tall, `px 8`, gap 8, radius 8.
fn row_frame(id: String) -> Stateful<Div> {
    div()
        .id(id)
        .h(ui_rems(ROW_H))
        .flex_none()
        .px(px(ROW_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .rounded(px(ROW_RADIUS))
}

/// Save and cancel for a row being typed into (Pink's queue edit actions).
/// They act on mouse-down, before the field's blur commits the text.
fn edit_actions(
    row_key: &str,
    save_label: &'static str,
    state: &Entity<PlanState>,
    adding: bool,
    t: &Pink,
) -> Div {
    let save = move |s: &mut PlanState, window: &mut Window, cx: &mut Context<PlanState>| {
        if adding {
            let text = s.add.read(cx).text();
            s.add_step(text, window, cx);
            s.cancel_add(cx);
        } else {
            s.commit_rename(window, cx);
        }
    };
    let cancel = move |s: &mut PlanState, cx: &mut Context<PlanState>| {
        if adding {
            s.cancel_add(cx);
        } else {
            s.cancel_rename(cx);
        }
    };
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.))
        .child(
            tray_action(row_key, "save", save_label, ICON_SAVE, t)
                .on_mouse_down(MouseButton::Left, {
                    let state = state.clone();
                    move |_: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        state.update(cx, |s, cx| save(s, window, cx));
                    }
                })
                .on_click({
                    let state = state.clone();
                    move |event: &ClickEvent, window, cx| {
                        if event.is_keyboard() {
                            state.update(cx, |s, cx| save(s, window, cx));
                        }
                    }
                }),
        )
        .child(
            tray_action(row_key, "cancel", "Cancel", ICON_CANCEL, t)
                .on_mouse_down(MouseButton::Left, {
                    let state = state.clone();
                    move |_: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        state.update(cx, |s, cx| cancel(s, cx));
                    }
                })
                .on_click({
                    let state = state.clone();
                    move |event: &ClickEvent, _, cx| {
                        if event.is_keyboard() {
                            state.update(cx, |s, cx| cancel(s, cx));
                        }
                    }
                }),
        )
}

/// One step of the draft (Pink: queue.rs `queue_row`): the drag marker, the
/// title over its tool and estimate, remove and edit always present. The
/// whole row drags; the row being renamed takes the Tertiary fill and the
/// field in place of its title.
fn step_row(
    key: &str,
    ix: usize,
    step: &Step,
    s: &PlanState,
    state: &Entity<PlanState>,
    t: &Pink,
) -> AnyElement {
    let row_key = format!("{key}-row-{}", step.key);
    let editing = s.editing == Some(step.key);
    let step_key = step.key;
    let stop = |_: &MouseDownEvent, _: &mut Window, cx: &mut App| cx.stop_propagation();

    let content = if editing {
        field(t.text).child(s.rename.clone()).into_any_element()
    } else {
        let style = StepStyle {
            title: t.text.opacity(0.9),
            detail: t.text_muted,
        };
        let detail = format!(
            "{}, about {}",
            step.plan.tool,
            format_elapsed(step.plan.estimate.round() as i64)
        );
        let title = step_title(style.title, step.plan.title.clone())
            .id(format!("{row_key}-title"))
            .on_click({
                let state = state.clone();
                move |event: &ClickEvent, window, cx| {
                    if event.click_count() >= 2 {
                        state.update(cx, |s, cx| {
                            s.take_over();
                            s.begin_rename(step_key, window, cx)
                        });
                    }
                }
            });
        step_text(&style, title, Some(detail.into())).into_any_element()
    };

    let actions = if editing {
        edit_actions(&row_key, "Save step", state, false, t)
    } else {
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(3.))
            .child(
                tray_action(&row_key, "remove", "Remove step", ICON_REMOVE, t)
                    .on_mouse_down(MouseButton::Left, stop)
                    .on_click({
                        let state = state.clone();
                        move |_, window, cx| {
                            cx.stop_propagation();
                            state.update(cx, |s, cx| {
                                s.take_over();
                                s.remove(step_key, window, cx)
                            })
                        }
                    }),
            )
            .child(
                tray_action(&row_key, "edit", "Rename step", ICON_EDIT, t)
                    .on_mouse_down(MouseButton::Left, stop)
                    .on_click({
                        let state = state.clone();
                        move |_, window, cx| {
                            cx.stop_propagation();
                            state.update(cx, |s, cx| {
                                s.take_over();
                                s.begin_rename(step_key, window, cx)
                            })
                        }
                    }),
            )
    };

    let row = row_frame(row_key.clone())
        .when(editing, |el| el.bg(t.fill(FillTier::Tertiary)))
        .when(!editing, |el| {
            el.hover_wash(row_key.clone(), t.fill(FillTier::Tertiary))
                .on_drag(
                    StepDrag {
                        plan: key.to_string().into(),
                        from: ix,
                    },
                    |_, _, _, cx| {
                        cx.stop_propagation();
                        cx.new(|_| EmptyView)
                    },
                )
        })
        // Keep the title on its column while the field replaces it.
        .child(if editing {
            div().w(px(20.)).flex_none()
        } else {
            slot(ICON_DRAG, t.readable_muted(0.5)).cursor_pointer()
        })
        .child(content)
        .child(actions);

    let Some(drag) = s.drag.as_ref() else {
        return row.into_any_element();
    };
    let (start, target) = drag_offsets(ix, drag.from, drag.prev_over, drag.over);
    let (start, target) = (start * s.row_h, target * s.row_h);
    div()
        .flex_none()
        .child(row)
        .with_animation(
            ElementId::Name(format!("{row_key}-slide-{}", drag.epoch).into()),
            TAB_SLIDE.animation(),
            move |el, p| el.relative().top(px(motion::lerp(start, target, p))),
        )
        .into_any_element()
}

/// The row that adds a step: a quiet "Add a step" until clicked, then the
/// field with save and cancel, in the tray's row anatomy.
fn add_row(key: &str, s: &PlanState, state: &Entity<PlanState>, t: &Pink) -> Stateful<Div> {
    let add_key = format!("{key}-add");
    if s.adding {
        return row_frame(add_key.clone())
            .bg(t.fill(FillTier::Tertiary))
            .child(slot(ICON_PLUS, t.readable_muted(0.8)))
            .child(field(t.text).child(s.add.clone()))
            .child(edit_actions(&add_key, "Add step", state, true, t));
    }
    let ring = t.focus_ring();
    let tone = motion::hover_blend(&add_key, t.text_muted, t.text);
    row_frame(add_key.clone())
        .role(Role::Button)
        .aria_label("Add a step")
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_key_down(pink::swallow_activation_key)
        .cursor_pointer()
        .hover_wash(add_key, t.fill(FillTier::Tertiary))
        .child(slot(ICON_PLUS, t.readable_muted(0.8)))
        .child(add_label(tone, "Add a step"))
        .on_click({
            let state = state.clone();
            move |_, window, cx| {
                state.update(cx, |s, cx| {
                    s.take_over();
                    s.begin_add(window, cx);
                })
            }
        })
}

/// The tray's header bar (Pink: composer/goals.rs `render_goal_card`, the
/// goal bar on the same stack tray): a 38 strip reading "Plan · objective",
/// the step count and estimate trailing in 12 tabular figures, then the
/// controls; here the one Primary button.
fn tray_header(
    key: &str,
    title: &SharedString,
    s: &PlanState,
    state: &Entity<PlanState>,
    t: &Pink,
) -> Div {
    let count = s.steps.len();
    let estimate: f32 = s.steps.iter().map(|st| st.plan.estimate).sum();
    let steps = format!("{count} {}", if count == 1 { "step" } else { "steps" });
    let estimate = format!("about {}", format_elapsed(estimate.round() as i64));
    let style = HeaderStyle {
        glyph: t.readable_muted(0.85),
        label: t.text,
        objective: t.readable_muted(0.8),
        facts: t.readable_muted(0.75),
    };
    // Facts sit apart on the row's gap: no separator glyph.
    let summary = header_summary(
        &style,
        ICON_CHECKLIST,
        "Plan",
        title.clone(),
        [steps.clone().into(), estimate.clone().into()],
    )
    .id(format!("{key}-summary"))
    .role(Role::Status)
    .aria_label(format!("Plan, {title}, {steps}, {estimate}"))
    .font_features(tabular_figures());
    let run = pink::button_enabled(
        format!("{key}-run"),
        ButtonKind::Primary,
        ControlSize::Sm,
        count > 0,
        t,
    )
    .aria_label("Run the plan")
    .child(pink::glyph(ICON_PLAY, ControlSize::Sm).text_color(t.on_solid))
    .child("Run plan")
    .on_click({
        let state = state.clone();
        move |_, window, cx| {
            let handler = state.update(cx, |s, cx| {
                s.take_over();
                s.commit_rename(window, cx);
                s.start_run();
                cx.notify();
                s.on_run.clone().map(|h| (h, s.plan()))
            });
            if let Some((handler, plan)) = handler {
                handler(&plan, window, cx);
            }
        }
    });
    header().child(summary).child(controls().child(run))
}

/// The draft as a tray above the composer (Pink: queue.rs
/// `render_queue_panel` / `queue_panel_surface`): one opaque plate with a
/// hairline edge, the goal bar's header and borderless rows. Docked on a
/// composer it takes only top corners and tucks its last 18 px behind the
/// pill.
fn plan_tray(
    key: &str,
    title: &SharedString,
    s: &PlanState,
    state: &Entity<PlanState>,
    docked: bool,
    t: &Pink,
) -> Stateful<Div> {
    let count = s.steps.len();
    let row_h = s.row_h;
    let rows = s
        .steps
        .iter()
        .enumerate()
        .map(|(ix, step)| step_row(key, ix, step, s, state, t));
    let plan_key: SharedString = key.to_string().into();
    let list = div()
        .id(format!("{key}-steps"))
        .flex()
        .flex_col()
        .on_drag_move::<StepDrag>({
            let state = state.clone();
            let plan_key = plan_key.clone();
            move |event: &DragMoveEvent<StepDrag>, _, cx| {
                let (plan, from) = {
                    let drag = event.drag(cx);
                    (drag.plan.clone(), drag.from)
                };
                if plan != plan_key {
                    return;
                }
                let rel_y = f32::from(event.event.position.y - event.bounds.top());
                let over = drop_index(rel_y, row_h, count);
                state.update(cx, |s, cx| {
                    s.take_over();
                    s.drag_over(from, over, cx)
                });
            }
        })
        .children(rows);

    let edge = t.border;
    div()
        .id(format!("{key}-tray"))
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(t.surface_bg())
        .border_1()
        .border_color(edge)
        // Docked, the tray floats over the transcript with the composer and
        // takes Pink's opaque-treatment `menu_shadows`; on its own it lies flat.
        .when(docked, |el| {
            el.mx(ui_rems(TRAY_INSET))
                .mb(ui_rems(-(STACK_GAP + TRAY_OVERLAP)))
                .rounded_t(px(radius::STACK_TRAY))
                .pb(ui_rems(TRAY_OVERLAP))
                .shadow(t.menu_shadows())
        })
        .when(!docked, |el| el.rounded(px(radius::STACK_TRAY)))
        // The whole plate is the drop target (Pink: its padding included).
        .on_drop::<StepDrag>({
            let state = state.clone();
            move |payload: &StepDrag, window, cx| {
                state.update(cx, |s, cx| {
                    let to = s.drag.take().map_or(payload.from, |d| d.over);
                    if payload.plan == plan_key {
                        s.take_over();
                        s.move_step(payload.from, to, window, cx);
                    } else {
                        cx.notify();
                    }
                })
            }
        })
        .on_mouse_up_out(MouseButton::Left, {
            let state = state.clone();
            move |_, _, cx| state.update(cx, |s, cx| s.cancel_drag(cx))
        })
        .child(tray_header(key, title, s, state, t))
        .child(list)
        .child(add_row(key, s, state, t))
}

impl RenderOnce for AgentPlan {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial = self.steps.clone();
        let failure = self.failure.clone();
        let (autoplay, expanded) = (self.autoplay, self.expanded);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "plan".into()),
            cx,
            move |window, cx| PlanState::new(initial, failure, autoplay, expanded, window, cx),
        );
        // A drop outside the tray ends GPUI's drag without our `on_drop`:
        // never leave a row parked in a stale slot.
        let drag_live = cx.has_active_drag();
        let (on_run, on_change) = (self.on_run.clone(), self.on_change.clone());
        state.update(cx, |s, cx| {
            s.on_run = on_run;
            s.on_change = on_change;
            s.row_h = ROW_H * f32::from(window.rem_size()) / 16.0;
            if !drag_live {
                s.drag = None;
            }
            for input in [s.rename.clone(), s.add.clone()] {
                input.update(cx, |input, _| {
                    input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
                });
            }
        });

        let s = state.read(cx);
        let docked = self.composer.is_some();
        let (epoch, failures) = (s.epoch, s.failures);
        let strip = (s.phase != Phase::Draft).then(|| {
            motion::fade_quick(
                ElementId::Name(format!("{key}-checklist-in-{epoch}").into()),
                run_checklist(&key, s, &state, &t),
            )
        });
        let notice = failure_notice(&key, s, &state, &t).map(|notice| {
            motion::fade_quick(
                ElementId::Name(format!("{key}-notice-in-{failures}").into()),
                notice,
            )
        });
        let tray = (s.phase == Phase::Draft).then(|| {
            motion::fade_quick(
                ElementId::Name(format!("{key}-tray-in-{epoch}").into()),
                plan_tray(&key, &self.title, s, &state, docked, &t),
            )
        });

        div()
            .id(self.id)
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .gap(ui_rems(STACK_GAP))
            .font_family(pink::SANS)
            .text_color(t.text)
            .children(notice)
            .children(strip)
            .children(tray)
            .children(self.composer)
    }
}

/// The composer the demo stacks the plan on (Pink: composer.rs
/// `composer_surface`, on the frosted plate, holding an empty session
/// draft and the recessed Send disc of `render_send_button`).
fn demo_composer(t: &Pink) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(radius::COMPOSER_PILL))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
        .shadow(pink::shadow_lg())
        .child(
            div()
                .min_h(px(60.))
                .pt(px(16.))
                .pb(px(4.))
                .px(px(16.))
                .text_size(px(14.))
                .line_height(px(22.75))
                .text_color(t.placeholder)
                .child("Ask for changes to the plan"),
        )
        .child(
            div()
                .h(px(46.))
                .px(px(10.))
                .pt(px(2.))
                .pb(px(12.))
                .flex()
                .items_center()
                .justify_end()
                .child(
                    div()
                        .size(px(28.))
                        .rounded_full()
                        .bg(t.ink(0.08))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            svg()
                                .path(ICON_ARROW_UP)
                                .size(px(14.))
                                .text_color(t.text.opacity(0.35)),
                        ),
                ),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        AgentPlan::new("agent-plan", "Rotate staging database credentials")
            .step(
                PlanStep::new("Snapshot the staging database")
                    .tool("db.snapshot")
                    .estimate(2.2),
            )
            .step(
                PlanStep::new("Mint new credentials in the vault")
                    .tool("vault.write")
                    .estimate(1.6),
            )
            .step(
                PlanStep::new("Point six service configs at the new secret")
                    .tool("repo.edit")
                    .estimate(2.4),
            )
            .step(
                PlanStep::new("Restart services one at a time")
                    .tool("deploy.rollout")
                    .estimate(3.0),
            )
            .step(
                PlanStep::new("Verify connections and revoke the old keys")
                    .tool("db.query")
                    .estimate(2.0),
            )
            .fail_once(3, "billing-api missed its health check after the restart")
            .expanded(true)
            .autoplay(true)
            .composer(demo_composer(&t))
    })
}
