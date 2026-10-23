//! Agent Inbox – every background task your agents run, in one quiet list.
//!
//! Tasks sort themselves by urgency: whatever needs you floats to the top,
//! failures next, then the running work and finally what's done. When a task
//! changes state its row glides to its new place, and the filter tabs roll
//! their counts. Click a row to unfold its last steps and the actions that
//! fit its state: approve or reject, stop, retry, open.
//!
//! ```ignore
//! AgentInbox::new("inbox")
//!     .task(InboxTask::new("coupons", "Migrate legacy coupon codes", "Ledger")
//!         .status(TaskStatus::NeedsYou)
//!         .ask("Wants to retire 1,180 expired codes in production"))
//!     .on_action(|task, action, _, _| println!("{task}: {action:?}"))
//! ```

use std::{
    cell::RefCell,
    f32::consts::{PI, TAU},
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, App, Bounds, Div, ElementId, FontWeight, Hsla,
    IntoElement, PathBuilder, Pixels, SharedString, Task, Transformation, Window, canvas, div,
    point, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    motion::{self, EASE_IN_OUT, GLIDE, MotionSpec, ROW_IN},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, Tone, radius, space, text},
    story,
};

pub const NAME: &str = "Agent Inbox";
pub const DESCRIPTION: &str = "One calm list for every background agent task – urgent work floats up, counts roll as states change, and each row unfolds into its last steps and the right next action";

const ICON_CHEVRON: &str = "icons/agent-inbox/chevron.svg";
const ICON_CHECK: &str = "icons/agent-inbox/check.svg";
const ICON_CLOSE: &str = "icons/agent-inbox/close.svg";
const ICON_RETRY: &str = "icons/agent-inbox/retry.svg";
const ICON_OPEN: &str = "icons/agent-inbox/open.svg";
const ICON_STOP: &str = "icons/agent-inbox/stop.svg";
const ICON_INBOX: &str = "icons/agent-inbox/inbox.svg";
const ICON_ATTENTION: &str = "icons/agent-inbox/attention.svg";
const ICON_CLEAR: &str = "icons/agent-inbox/clear.svg";

const WIDTH: f32 = 640.0;
const HEAD_H: f32 = 56.0;
const STEP_H: f32 = 22.0;
const ASK_H: f32 = 34.0;
const ACTIONS_H: f32 = 28.0;
/// How long the demo plays before it starts over.
const DEMO_LOOP: f32 = 28.0;

/// Where a task stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// Paused until you approve or reject something.
    NeedsYou,
    Running,
    Failed,
    Done,
    Cancelled,
}

impl TaskStatus {
    /// Sort order: what needs a person first.
    fn rank(self) -> u8 {
        match self {
            TaskStatus::NeedsYou => 0,
            TaskStatus::Failed => 1,
            TaskStatus::Running => 2,
            TaskStatus::Done | TaskStatus::Cancelled => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            TaskStatus::NeedsYou => "Needs you",
            TaskStatus::Running => "Running",
            TaskStatus::Failed => "Failed",
            TaskStatus::Done => "Done",
            TaskStatus::Cancelled => "Cancelled",
        }
    }

    fn tone(self) -> Tone {
        match self {
            TaskStatus::NeedsYou => Tone::Warning,
            TaskStatus::Running => Tone::Accent,
            TaskStatus::Failed => Tone::Danger,
            TaskStatus::Done => Tone::Success,
            TaskStatus::Cancelled => Tone::Neutral,
        }
    }
}

/// What a person did with a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxAction {
    Approve,
    Reject,
    Stop,
    Retry,
    Open,
}

/// One background task.
#[derive(Clone, Debug)]
pub struct InboxTask {
    pub id: SharedString,
    pub title: SharedString,
    pub agent: SharedString,
    pub summary: SharedString,
    pub status: TaskStatus,
    /// What the agent is waiting on you for.
    pub ask: Option<SharedString>,
    /// Recent steps, oldest first, with how many seconds ago they happened.
    pub steps: Vec<(SharedString, f32)>,
    /// Seconds in the current state (running time, waiting time, or age).
    pub age: f32,
}

impl InboxTask {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        agent: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            agent: agent.into(),
            summary: SharedString::default(),
            status: TaskStatus::Running,
            ask: None,
            steps: Vec::new(),
            age: 0.0,
        }
    }

    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn status(mut self, status: TaskStatus) -> Self {
        self.status = status;
        self
    }

    pub fn ask(mut self, ask: impl Into<SharedString>) -> Self {
        self.ask = Some(ask.into());
        self
    }

    pub fn step(mut self, text: impl Into<SharedString>, seconds_ago: f32) -> Self {
        self.steps.push((text.into(), seconds_ago));
        self
    }

    pub fn age(mut self, seconds: f32) -> Self {
        self.age = seconds;
        self
    }
}

/// A scripted change for the demo: at `at` seconds, `task` moves to `status`.
#[derive(Clone, Debug)]
pub struct InboxEvent {
    pub at: f32,
    pub task: InboxTask,
}

type ActionHandler = Rc<dyn Fn(&SharedString, InboxAction, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct AgentInbox {
    id: ElementId,
    title: SharedString,
    tasks: Vec<InboxTask>,
    events: Vec<InboxEvent>,
    expanded: Option<SharedString>,
    autoplay: bool,
    on_action: Option<ActionHandler>,
}

impl AgentInbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: "Agent inbox".into(),
            tasks: Vec::new(),
            events: Vec::new(),
            expanded: None,
            autoplay: false,
            on_action: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn task(mut self, task: InboxTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Unfolds this task's row when the inbox first appears.
    pub fn expanded(mut self, task_id: impl Into<SharedString>) -> Self {
        self.expanded = Some(task_id.into());
        self
    }

    /// During autoplay, replaces (or adds) the task with the same id `at` seconds in.
    pub fn event(mut self, at: f32, task: InboxTask) -> Self {
        self.events.push(InboxEvent { at, task });
        self
    }

    /// Plays the scripted events in a loop until someone clicks.
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, InboxAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    All,
    NeedsYou,
    Running,
    Failed,
    Done,
}

const FILTERS: [Filter; 5] = [
    Filter::All,
    Filter::NeedsYou,
    Filter::Running,
    Filter::Failed,
    Filter::Done,
];

impl Filter {
    fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::NeedsYou => "Needs you",
            Filter::Running => "Running",
            Filter::Failed => "Failed",
            Filter::Done => "Done",
        }
    }

    fn admits(self, status: TaskStatus) -> bool {
        match self {
            Filter::All => true,
            Filter::NeedsYou => status == TaskStatus::NeedsYou,
            Filter::Running => status == TaskStatus::Running,
            Filter::Failed => status == TaskStatus::Failed,
            Filter::Done => matches!(status, TaskStatus::Done | TaskStatus::Cancelled),
        }
    }
}

struct Row {
    key: usize,
    task: InboxTask,
    /// Clock time the current state began (age counts from here).
    since: f32,
    /// Clock time of the last change: newer rows sort first in their group.
    updated: f32,
    /// Steps as (text, clock time).
    steps: Vec<(SharedString, f32)>,
}

impl Row {
    fn new(key: usize, task: InboxTask, clock: f32, updated: f32) -> Self {
        let since = clock - task.age;
        let steps = task
            .steps
            .iter()
            .map(|(text, ago)| (text.clone(), clock - ago))
            .collect();
        Row {
            key,
            task,
            since,
            updated,
            steps,
        }
    }

    fn detail_height(&self) -> f32 {
        let ask = if self.task.ask.is_some() && self.task.status == TaskStatus::NeedsYou {
            ASK_H + 8.0
        } else {
            0.0
        };
        let steps = self.steps.len().min(4) as f32 * STEP_H;
        ask + steps + 10.0 + ACTIONS_H + 14.0
    }
}

/// Tab bounds from the last paint, for the sliding underline.
#[derive(Default)]
struct TabMarks {
    bar: Option<Bounds<Pixels>>,
    tabs: [Option<Bounds<Pixels>>; 5],
}

struct InboxState {
    initial: Vec<InboxTask>,
    initial_expanded: Option<SharedString>,
    events: Vec<InboxEvent>,
    played: usize,
    rows: Vec<Row>,
    next_key: usize,
    filter: Filter,
    expanded: Option<usize>,
    clock: f32,
    loop_start: f32,
    /// Bumps on every demo loop so rows fade back in.
    generation: usize,
    autoplay: bool,
    counts: [usize; 5],
    prev_counts: [usize; 5],
    count_serial: [usize; 5],
    marks: Rc<RefCell<TabMarks>>,
    last_tick: Instant,
    on_action: Option<ActionHandler>,
    _ticker: Task<()>,
}

impl InboxState {
    fn new(config: &AgentInbox, cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if this.update(cx, |state, cx| state.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let mut state = Self {
            initial: config.tasks.clone(),
            initial_expanded: config.expanded.clone(),
            events: config.events.clone(),
            played: 0,
            rows: Vec::new(),
            next_key: 0,
            filter: Filter::All,
            expanded: None,
            clock: 0.0,
            loop_start: 0.0,
            generation: 0,
            autoplay: config.autoplay,
            counts: [0; 5],
            prev_counts: [0; 5],
            count_serial: [0; 5],
            marks: Rc::default(),
            last_tick: Instant::now(),
            on_action: None,
            _ticker: ticker,
        };
        state.load();
        state.counts = state.tally();
        state.prev_counts = state.counts;
        state
    }

    /// (Re)loads the configured tasks.
    fn load(&mut self) {
        self.rows.clear();
        self.played = 0;
        self.loop_start = self.clock;
        let clock = self.clock;
        for (ix, task) in self.initial.clone().into_iter().enumerate() {
            let key = self.next_key;
            self.next_key += 1;
            self.rows
                .push(Row::new(key, task, clock, clock - ix as f32));
        }
        self.expanded = self
            .initial_expanded
            .as_ref()
            .and_then(|id| self.rows.iter().find(|r| &r.task.id == id).map(|r| r.key));
    }

    fn tally(&self) -> [usize; 5] {
        let mut counts = [0; 5];
        for (ix, filter) in FILTERS.iter().enumerate() {
            counts[ix] = self
                .rows
                .iter()
                .filter(|r| filter.admits(r.task.status))
                .count();
        }
        counts
    }

    /// Remembers the previous counts so the tabs can roll from them.
    fn recount(&mut self) {
        let counts = self.tally();
        for (ix, count) in counts.into_iter().enumerate() {
            if count != self.counts[ix] {
                self.prev_counts[ix] = self.counts[ix];
                self.counts[ix] = count;
                self.count_serial[ix] += 1;
            }
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.clock += now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .min(1.0);
        self.last_tick = now;
        if self.autoplay {
            let t = self.clock - self.loop_start;
            while let Some(event) = self.events.get(self.played).cloned() {
                if t < event.at {
                    break;
                }
                self.played += 1;
                self.apply(event.task);
            }
            if t >= DEMO_LOOP {
                self.generation += 1;
                self.load();
            }
            self.recount();
        }
        cx.notify();
    }

    /// Replaces the task with the same id, or adds it at the top of its group.
    fn apply(&mut self, task: InboxTask) {
        let clock = self.clock;
        if let Some(row) = self.rows.iter_mut().find(|r| r.task.id == task.id) {
            let changed = row.task.status != task.status;
            for (text, ago) in &task.steps {
                if !row.steps.iter().any(|(t, _)| t == text) {
                    row.steps.push((text.clone(), clock - ago));
                }
            }
            if changed {
                row.since = clock - task.age;
                row.updated = clock;
            }
            row.task = InboxTask {
                steps: Vec::new(),
                ..task
            };
        } else {
            let key = self.next_key;
            self.next_key += 1;
            self.rows.push(Row::new(key, task, clock, clock));
        }
    }

    fn act(&mut self, key: usize, action: InboxAction) {
        let clock = self.clock;
        let Some(row) = self.rows.iter_mut().find(|r| r.key == key) else {
            return;
        };
        let (status, step) = match action {
            InboxAction::Approve => (TaskStatus::Running, "Approved by you"),
            InboxAction::Reject => (TaskStatus::Cancelled, "Rejected by you"),
            InboxAction::Stop => (TaskStatus::Cancelled, "Stopped by you"),
            InboxAction::Retry => (TaskStatus::Running, "Retried by you"),
            InboxAction::Open => return,
        };
        row.task.status = status;
        row.since = clock;
        row.updated = clock;
        row.steps.push((step.into(), clock));
        if action == InboxAction::Approve
            && let Some(ask) = row.task.ask.take()
        {
            row.task.summary = ask;
        }
        self.recount();
    }

    /// Visible rows in display order: urgency first, newest first within a group.
    fn ordered(&self) -> Vec<&Row> {
        let mut rows: Vec<&Row> = self
            .rows
            .iter()
            .filter(|r| self.filter.admits(r.task.status))
            .collect();
        rows.sort_by(|a, b| {
            a.task
                .status
                .rank()
                .cmp(&b.task.status.rank())
                .then(b.updated.total_cmp(&a.updated))
        });
        rows
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn duration_label(seconds: f32) -> String {
    let s = seconds.max(0.0) as u32;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    }
}

fn ago_label(seconds: f32) -> String {
    let s = seconds.max(0.0) as u32;
    if s < 45 {
        "just now".into()
    } else if s < 3600 {
        format!("{}m ago", (s + 30) / 60)
    } else {
        format!("{}h ago", s / 3600)
    }
}

/// A stable hue per agent name, from Pink's palette family.
fn agent_hue(name: &str, t: &Pink) -> Hsla {
    const HUES: [f32; 5] = [277.0, 200.0, 155.0, 60.0, 330.0];
    let ix = name
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize))
        % 5;
    let (l, c) = if t.dark { (0.74, 0.13) } else { (0.56, 0.17) };
    pink::oklch(l, c, HUES[ix])
}

/// A tiny square monogram for an agent.
fn monogram(name: &str, size: f32, t: &Pink) -> Div {
    let hue = agent_hue(name, t);
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::XS))
        .bg(hue.opacity(if t.dark { 0.2 } else { 0.13 }))
        .text_size(px(size * 0.6))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(hue)
        .child(name.chars().next().unwrap_or('?').to_string())
}

/// The running spinner: a thin ring and a sweeping arc.
fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla, track: Hsla) -> AnyElement {
    div()
        .size(px(size))
        .flex_none()
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1000)).repeat(),
            move |el, t| {
                el.child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let c = bounds.center();
                            let r = f32::from(bounds.size.width) / 2.0 - 1.0;
                            let at = |a: f32| point(c.x + px(r * a.cos()), c.y + px(r * a.sin()));
                            let mut ring = PathBuilder::stroke(px(1.5));
                            ring.move_to(at(0.0));
                            ring.arc_to(point(px(r), px(r)), px(0.), false, true, at(PI));
                            ring.arc_to(point(px(r), px(r)), px(0.), false, true, at(TAU));
                            if let Ok(path) = ring.build() {
                                window.paint_path(path, track);
                            }
                            let start = t * TAU;
                            let mut arc = PathBuilder::stroke(px(1.5));
                            arc.move_to(at(start));
                            arc.arc_to(
                                point(px(r), px(r)),
                                px(0.),
                                false,
                                true,
                                at(start + PI * 0.55),
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
        .into_any_element()
}

fn status_glyph(row: &Row, key: &str, t: &Pink) -> AnyElement {
    let size = 18.0;
    let frame = || {
        div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
    };
    match row.task.status {
        TaskStatus::Running => frame()
            .child(spinner(
                ElementId::Name(format!("{key}-spin-{}", row.key).into()),
                size - 2.0,
                t.accent,
                t.fill(FillTier::Primary),
            ))
            .into_any_element(),
        TaskStatus::NeedsYou => frame()
            .border_1()
            .border_color(t.warning.opacity(0.6))
            .child(
                div()
                    .size(px(8.))
                    .rounded_full()
                    .bg(t.warning)
                    .with_animation(
                        ElementId::Name(format!("{key}-wait-{}", row.key).into()),
                        Animation::new(Duration::from_millis(1600))
                            .repeat()
                            .with_easing(EASE_IN_OUT.easing()),
                        |el, p| el.opacity(0.45 + 0.55 * (1.0 - (p * 2.0 - 1.0).abs())),
                    ),
            )
            .into_any_element(),
        TaskStatus::Done => frame()
            .bg(t.success)
            .child(svg().path(ICON_CHECK).size(px(13.)).text_color(t.bg))
            .into_any_element(),
        TaskStatus::Failed => frame()
            .bg(t.danger_strong)
            .child(svg().path(ICON_CLOSE).size(px(12.)).text_color(t.on_accent))
            .into_any_element(),
        TaskStatus::Cancelled => frame()
            .border_1()
            .border_color(t.border_strong)
            .child(div().w(px(7.)).h(px(1.5)).rounded_full().bg(t.text_faint))
            .into_any_element(),
    }
}

/// A count that rolls: the old number slides up and out as the new one rises in.
fn rolling_count(id: String, prev: usize, now: usize, color: Hsla) -> AnyElement {
    let digits = prev.to_string().len().max(now.to_string().len()) as f32;
    let cell = |n: usize| {
        div()
            .absolute()
            .left_0()
            .right_0()
            .h(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .child(n.to_string())
    };
    div()
        .relative()
        .w(px(digits * 7.0 + 1.0))
        .h(px(16.))
        .overflow_hidden()
        .font_family(pink::MONO)
        .text_size(px(text::CAPTION))
        .text_color(color)
        .with_animation(
            ElementId::Name(id.into()),
            MotionSpec::new(420, motion::EASE_OUT_QUINT).animation(),
            move |el, p| {
                let up = now >= prev;
                let dir = if up { 1.0 } else { -1.0 };
                el.child(cell(prev).top(px(-12.0 * p * dir)).opacity(1.0 - p))
                    .child(cell(now).top(px(12.0 * (1.0 - p) * dir)).opacity(p))
            },
        )
        .into_any_element()
}

/// An invisible layer that records its bounds when painted.
fn mark(marks: Rc<RefCell<TabMarks>>, slot: Option<usize>) -> impl IntoElement {
    canvas(
        move |bounds, _, _| {
            let mut marks = marks.borrow_mut();
            match slot {
                Some(ix) => marks.tabs[ix] = Some(bounds),
                None => marks.bar = Some(bounds),
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

impl RenderOnce for AgentInbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "inbox".into()),
            cx,
            |_, cx| InboxState::new(&self, cx),
        );
        let on_action = self.on_action.clone();
        state.update(cx, |s, _| s.on_action = on_action);
        let s = state.read(cx);
        let clock = s.clock;
        let generation = s.generation;

        // -- Header -------------------------------------------------------------
        let needs = s.counts[1];
        let running = s.counts[2];
        let has_done = s.counts[4] > 0;
        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S12))
            .px(px(space::S16))
            .pt(px(space::S16))
            .pb(px(space::S8))
            .child(
                div()
                    .size(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::MD))
                    .bg(t.fill(FillTier::Tertiary))
                    .child(svg().path(ICON_INBOX).size(px(16.)).text_color(t.text)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
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
                            .child(format!("{running} running · {needs} waiting on you")),
                    ),
            )
            .child(
                pink::button(
                    format!("{key}-clear"),
                    ButtonKind::Ghost,
                    ControlSize::Md,
                    &t,
                )
                .when(!has_done, |el| el.opacity(0.45))
                .child(pink::glyph(ICON_CLEAR, ControlSize::Md).text_color(t.text_muted))
                .child("Clear done")
                .on_click({
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.autoplay = false;
                            s.rows.retain(|r| {
                                !matches!(r.task.status, TaskStatus::Done | TaskStatus::Cancelled)
                            });
                            s.recount();
                            cx.notify();
                        })
                    }
                }),
            );

        // -- Filter tabs ----------------------------------------------------------
        let marks = s.marks.clone();
        let tabs = FILTERS.iter().enumerate().map(|(ix, filter)| {
            let active = s.filter == *filter;
            let tab_key = format!("{key}-tab-{ix}");
            let fg = if active {
                t.text
            } else {
                motion::hover_blend(&tab_key, t.text_muted, t.text)
            };
            let count = s.counts[ix];
            let urgent = *filter == Filter::NeedsYou && count > 0;
            let pill_fg = if urgent { t.warning } else { t.text_muted };
            let pill_bg = if urgent {
                t.warning.opacity(if t.dark { 0.16 } else { 0.12 })
            } else {
                t.fill(FillTier::Tertiary)
            };
            let count_el = if s.prev_counts[ix] != count {
                rolling_count(
                    format!("{tab_key}-roll-{}", s.count_serial[ix]),
                    s.prev_counts[ix],
                    count,
                    pill_fg,
                )
            } else {
                div()
                    .font_family(pink::MONO)
                    .text_size(px(text::CAPTION))
                    .text_color(pill_fg)
                    .child(count.to_string())
                    .into_any_element()
            };
            let filter = *filter;
            div()
                .id(ElementId::Name(tab_key.clone().into()))
                .relative()
                .h(px(36.))
                .px(px(space::S8))
                .flex()
                .items_center()
                .gap(px(space::S6))
                .cursor_pointer()
                .text_size(px(text::BODY))
                .font_weight(if active {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(fg)
                .on_hover(motion::hover_listener(tab_key.clone()))
                .on_click({
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.autoplay = false;
                            s.filter = filter;
                            cx.notify();
                        })
                    }
                })
                .child(mark(marks.clone(), Some(ix)))
                .child(filter.label())
                .child(
                    div()
                        .h(px(18.))
                        .px(px(5.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::SM))
                        .bg(pill_bg)
                        .child(count_el),
                )
        });
        let underline = {
            let m = marks.borrow();
            let active_ix = FILTERS.iter().position(|f| *f == s.filter).unwrap_or(0);
            m.bar.zip(m.tabs[active_ix]).map(|(bar, tab)| {
                let x = f32::from(tab.origin.x - bar.origin.x) + space::S8;
                let w = f32::from(tab.size.width) - space::S8 * 2.0;
                let x = motion::follow(&format!("{key}-ul-x"), x, GLIDE);
                let w = motion::follow(&format!("{key}-ul-w"), w, GLIDE);
                div()
                    .absolute()
                    .bottom(px(-1.))
                    .left(px(x))
                    .w(px(w))
                    .h(px(2.))
                    .rounded_full()
                    .bg(t.accent)
            })
        };
        let tab_bar = div()
            .relative()
            .flex()
            .items_center()
            .gap(px(space::S2))
            .mx(px(space::S8))
            .border_b_1()
            .border_color(t.border)
            .child(mark(marks.clone(), None))
            .children(tabs)
            .children(underline);

        // -- Rows -------------------------------------------------------------------
        let ordered = s.ordered();
        let mut top = 0.0;
        let mut rows = Vec::new();
        for (ix, row) in ordered.iter().enumerate() {
            let expanded = s.expanded == Some(row.key);
            let height = HEAD_H + if expanded { row.detail_height() } else { 0.0 };
            let row_key = format!("{key}-row-{}", row.key);
            let y = motion::follow(&format!("{row_key}-y"), top, GLIDE);
            let h = motion::follow(&format!("{row_key}-h"), height, GLIDE);
            let open = motion::follow(
                &format!("{row_key}-open"),
                if expanded { 1.0 } else { 0.0 },
                GLIDE,
            );
            top += height;
            let status = row.task.status;
            let row_id = row.key;

            let (time, time_color) = match status {
                TaskStatus::Running => (duration_label(clock - row.since), t.text_muted),
                TaskStatus::NeedsYou => (
                    format!("waiting {}", duration_label(clock - row.since)),
                    t.warning,
                ),
                TaskStatus::Failed => (ago_label(clock - row.since), t.danger),
                TaskStatus::Done | TaskStatus::Cancelled => {
                    (ago_label(clock - row.since), t.text_faint)
                }
            };

            let head_key = format!("{row_key}-head");
            let head = div()
                .id(ElementId::Name(head_key.clone().into()))
                .h(px(HEAD_H))
                .flex()
                .items_center()
                .gap(px(space::S12))
                .px(px(space::S12))
                .cursor_pointer()
                .rounded(px(radius::MD))
                .hover_fill(
                    head_key,
                    t.fill(FillTier::Quaternary).opacity(0.),
                    t.fill(FillTier::Quaternary),
                )
                .on_click({
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.autoplay = false;
                            s.expanded = if s.expanded == Some(row_id) {
                                None
                            } else {
                                Some(row_id)
                            };
                            cx.notify();
                        })
                    }
                })
                .child(status_glyph(row, &key, &t))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap(px(space::S2))
                        .child(
                            div().flex().child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(text::BODY))
                                    .line_height(px(18.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text)
                                    .child(row.task.title.clone()),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S6))
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .child(monogram(&row.task.agent, 14.0, &t))
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(t.text_muted)
                                        .child(row.task.agent.clone()),
                                )
                                .child(div().text_color(t.text_faint).child("·"))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(t.text_faint)
                                        .child(row.task.summary.clone()),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(pink::MONO)
                        .text_size(px(text::FOOTNOTE))
                        .text_color(time_color)
                        .child(time),
                )
                .child(
                    div()
                        .w(px(74.))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .child(pink::badge(status.label(), status.tone(), &t)),
                )
                .child(
                    svg()
                        .path(ICON_CHEVRON)
                        .size(px(14.))
                        .flex_none()
                        .text_color(t.text_faint)
                        .with_transformation(Transformation::rotate(radians(PI * open))),
                );

            // Unfolded: what it asks, its last steps and the fitting actions.
            let detail = expanded.then(|| {
                let ask = (status == TaskStatus::NeedsYou)
                    .then(|| row.task.ask.clone())
                    .flatten()
                    .map(|ask| {
                        div()
                            .h(px(ASK_H))
                            .mb(px(8.))
                            .px(px(space::S10))
                            .flex()
                            .items_center()
                            .gap(px(space::S8))
                            .rounded(px(radius::MD))
                            .bg(t.warning.opacity(if t.dark { 0.10 } else { 0.08 }))
                            .text_size(px(text::BODY))
                            .text_color(t.text)
                            .child(
                                svg()
                                    .path(ICON_ATTENTION)
                                    .size(px(14.))
                                    .flex_none()
                                    .text_color(t.warning),
                            )
                            .child(div().flex_1().min_w_0().truncate().child(ask))
                    });
                let shown = row.steps.len().min(4);
                let steps = row.steps[row.steps.len() - shown..].iter().enumerate().map(
                    |(n, (text, at))| {
                        let last = n + 1 == shown;
                        div()
                            .h(px(STEP_H))
                            .flex()
                            .items_center()
                            .gap(px(space::S10))
                            .text_size(px(text::FOOTNOTE))
                            .child(div().size(px(5.)).flex_none().rounded_full().bg(if last {
                                t.text_muted
                            } else {
                                t.border_strong
                            }))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(if last { t.text } else { t.text_muted })
                                    .child(text.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .font_family(pink::MONO)
                                    .text_size(px(text::CAPTION))
                                    .text_color(t.text_faint)
                                    .child(ago_label(clock - at)),
                            )
                    },
                );
                let button = |name: &str,
                              kind: ButtonKind,
                              icon: Option<&'static str>,
                              action: InboxAction| {
                    let fg = match kind {
                        ButtonKind::Accent | ButtonKind::Danger => t.on_accent,
                        ButtonKind::Primary => t.on_solid,
                        _ => t.text,
                    };
                    pink::button(format!("{row_key}-{name}"), kind, ControlSize::Md, &t)
                        .when_some(icon, |el, icon| {
                            el.child(pink::glyph(icon, ControlSize::Md).text_color(fg))
                        })
                        .child(name.to_string())
                        .on_click({
                            let state = state.clone();
                            move |_, window, cx| {
                                let (handler, id) = state.update(cx, |s, cx| {
                                    s.autoplay = false;
                                    s.act(row_id, action);
                                    cx.notify();
                                    let id = s
                                        .rows
                                        .iter()
                                        .find(|r| r.key == row_id)
                                        .map(|r| r.task.id.clone());
                                    (s.on_action.clone(), id)
                                });
                                if let (Some(handler), Some(id)) = (handler, id) {
                                    handler(&id, action, window, cx);
                                }
                            }
                        })
                };
                let actions = match status {
                    TaskStatus::NeedsYou => vec![
                        button(
                            "Approve",
                            ButtonKind::Accent,
                            Some(ICON_CHECK),
                            InboxAction::Approve,
                        ),
                        button("Reject", ButtonKind::Secondary, None, InboxAction::Reject),
                        button(
                            "Open",
                            ButtonKind::Ghost,
                            Some(ICON_OPEN),
                            InboxAction::Open,
                        ),
                    ],
                    TaskStatus::Running => vec![
                        button(
                            "Stop",
                            ButtonKind::Secondary,
                            Some(ICON_STOP),
                            InboxAction::Stop,
                        ),
                        button(
                            "Open",
                            ButtonKind::Ghost,
                            Some(ICON_OPEN),
                            InboxAction::Open,
                        ),
                    ],
                    TaskStatus::Failed => vec![
                        button(
                            "Retry",
                            ButtonKind::Accent,
                            Some(ICON_RETRY),
                            InboxAction::Retry,
                        ),
                        button(
                            "Open logs",
                            ButtonKind::Ghost,
                            Some(ICON_OPEN),
                            InboxAction::Open,
                        ),
                    ],
                    TaskStatus::Done | TaskStatus::Cancelled => vec![button(
                        "Open",
                        ButtonKind::Secondary,
                        Some(ICON_OPEN),
                        InboxAction::Open,
                    )],
                };
                div()
                    .pl(px(42.))
                    .pr(px(space::S12))
                    .opacity(open)
                    .children(ask)
                    .children(steps)
                    .child(
                        div()
                            .mt(px(10.))
                            .flex()
                            .gap(px(space::S6))
                            .children(actions),
                    )
            });

            let row_el = div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(y))
                .h(px(h))
                .overflow_hidden()
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Quaternary).opacity(open))
                .when(ix > 0, |el| {
                    el.child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(space::S12))
                            .right(px(space::S12))
                            .h(px(1.))
                            .bg(t.border.opacity(1.0 - open)),
                    )
                })
                .child(head)
                .children(detail);
            // Absolute rows can't take a `top` offset: they fade in instead.
            rows.push(row_el.with_animation(
                ElementId::Name(format!("{row_key}-in-{generation}").into()),
                ROW_IN.with_delay(30 * ix.min(8) as u64).animation(),
                |el, p| el.opacity(p),
            ));
        }
        let list_h = motion::follow(&format!("{key}-list-h"), top.max(HEAD_H), GLIDE);
        let empty = ordered.is_empty().then(|| {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(text::BODY))
                .text_color(t.text_faint)
                .child("Nothing here right now")
        });
        let list = div()
            .relative()
            .mx(px(space::S8))
            .my(px(space::S8))
            .h(px(list_h))
            .children(rows)
            .children(empty);

        // Tab marks are read a frame late: ask for one more until they land.
        if marks.borrow().bar.is_none() {
            window.request_animation_frame();
        }
        motion::keep_alive(window);
        pink::card(&t)
            .w(px(WIDTH))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(header)
            .child(tab_bar)
            .child(list)
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentInbox::new("agent-inbox")
            .task(
                InboxTask::new("coupons", "Migrate legacy coupon codes", "Ledger")
                    .status(TaskStatus::NeedsYou)
                    .summary("Paused before writing to production")
                    .ask("Wants to retire 1,180 expired codes in production")
                    .step("Matched 4,902 codes to the new schema", 410.0)
                    .step("Dry run on staging passed", 190.0)
                    .step("Paused for your approval", 72.0)
                    .age(72.0),
            )
            .task(
                InboxTask::new("crm", "Sync CRM contacts", "Atlas")
                    .status(TaskStatus::Failed)
                    .summary("Rate limited after 3,400 of 9,100 contacts")
                    .step("Pulled 9,100 contacts", 1020.0)
                    .step("Rate limited by the CRM API", 760.0)
                    .age(760.0),
            )
            .task(
                InboxTask::new("invoices", "Backfill invoice PDFs", "Relay")
                    .summary("Rendering March and April invoices")
                    .step("Found 1,904 invoices without a PDF", 180.0)
                    .step("Rendering March and April invoices", 95.0)
                    .age(221.0),
            )
            .task(
                InboxTask::new("churn", "Summarize churn interviews", "Quill")
                    .summary("Reading 14 transcripts")
                    .step("Collected 14 interview transcripts", 300.0)
                    .step("Reading 14 transcripts", 120.0)
                    .age(362.0),
            )
            .task(
                InboxTask::new("flags", "Audit unused feature flags", "Scout")
                    .summary("Scanning three repositories")
                    .step("Listed 212 flags from the dashboard", 70.0)
                    .age(80.0),
            )
            .task(
                InboxTask::new("digest", "Weekly revenue digest", "Quill")
                    .status(TaskStatus::Done)
                    .summary("Posted to the finance channel with 4 charts")
                    .step("Pulled numbers from billing", 1700.0)
                    .step("Posted the digest", 1440.0)
                    .age(1440.0),
            )
            .expanded("coupons")
            .event(
                3.5,
                InboxTask::new("invoices", "Backfill invoice PDFs", "Relay")
                    .status(TaskStatus::NeedsYou)
                    .summary("Paused before overwriting files")
                    .ask("Wants to overwrite 212 PDFs in the archive bucket")
                    .step("Rendered 1,904 invoices", 2.0)
                    .step("Paused for your approval", 0.0),
            )
            .event(
                7.5,
                InboxTask::new("churn", "Summarize churn interviews", "Quill")
                    .status(TaskStatus::Done)
                    .summary("Wrote a two-page brief with 9 themes")
                    .step("Wrote a two-page brief", 0.0),
            )
            .event(
                11.0,
                InboxTask::new("tickets", "Triage new support tickets", "Scout")
                    .summary("Labeling 37 tickets since this morning")
                    .step("Fetched 37 new tickets", 1.0),
            )
            .event(
                15.5,
                InboxTask::new("flags", "Audit unused feature flags", "Scout")
                    .status(TaskStatus::Done)
                    .summary("Found 23 flags that are safe to delete")
                    .step("Found 23 flags that are safe to delete", 0.0),
            )
            .autoplay(true)
    })
}
