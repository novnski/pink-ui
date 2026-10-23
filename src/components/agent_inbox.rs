//! Agent Inbox – pink's sidebar Activity view: every agent task that needs
//! you, has a fresh result, or is still running, in one triage list.
//!
//! Rows sort into three sections – Needs you, Ready, Running – newest first
//! within each, and glide to their new place when a task changes state. A
//! row is pink's two-line task row: the title over a quiet project · branch
//! line, the time-ago in the corner (pin and archive under the pointer) and
//! the status at the right edge – a spinner while it works, a dot when it is
//! done, and a pill naming the next step when it waits on you. Opening a row
//! reads it: a result stays listed while you read it and leaves once you open
//! another. The bell turns the view off, back to the plain task list.
//!
//! ```ignore
//! AgentInbox::new("inbox")
//!     .task(InboxTask::new("coupons", "Retire expired coupon codes", "billing-api")
//!         .status(TaskStatus::NeedsYou)
//!         .branch("coupons/retire"))
//!     .on_action(|task, action, _, _| println!("{task}: {action:?}"))
//! ```

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyElement, AnyView, App, Context, Div, ElementId, Entity, FontWeight, Hsla,
    IntoElement, Render, Role, SharedString, Stateful, Task, Toggled, Transformation, Window, div,
    linear_color_stop, linear_gradient, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    motion::{self, COLLAPSE, RESORT},
    pink::{self, FillTier, Pink, chrome, layout, loaders, radius, space},
    story,
};

pub const NAME: &str = "Agent Inbox";
pub const DESCRIPTION: &str = "Every agent task in one triage list – what needs you, fresh results and live work in three quiet sections, each row gliding to its new place as its task changes";

const ICON_SHIELD: &str = "icons/agent-inbox/shield-warning.svg";
const ICON_QUESTION: &str = "icons/agent-inbox/chat-round-question-mark.svg";
const ICON_DANGER: &str = "icons/agent-inbox/danger-circle.svg";
const ICON_BELL: &str = "icons/agent-inbox/bell.svg";
const ICON_BRANCH: &str = "icons/agent-inbox/git-branch.svg";
const ICON_PIN: &str = "icons/agent-inbox/pin.svg";
const ICON_ARCHIVE: &str = "icons/agent-inbox/archive-minimalistic.svg";
const ICON_FOLDER: &str = "icons/agent-inbox/folder.svg";
const ICON_CHECK: &str = "icons/agent-inbox/check.svg";
const ICON_CHEVRON: &str = "icons/agent-inbox/alt-arrow-right.svg";

/// pink: shell.rs `chat_row_height` – a title line and one meta line.
const ROW_H: f32 = 52.0;
/// pink: spaces.rs `SIDEBAR_DISCLOSURE_HEADER_HEIGHT`.
const HEADING_H: f32 = 28.0;
/// pink: spaces.rs `SIDEBAR_SECTION_GAP`, above every section after the first.
const SECTION_GAP: f32 = 12.0;
/// pink: spaces.rs `SIDEBAR_DISCLOSURE_BODY_INSET`.
const BODY_INSET: f32 = 4.0;
/// pink: the Pinned group's rule – `pt 8`, `pb 8` and one pixel.
const DIVIDER_H: f32 = 17.0;
/// pink: shell.rs `SIDEBAR_GUTTER` (`SPACE_SM`).
const GUTTER: f32 = space::SM;
/// pink: shell.rs `sidebar_faded_label` – the fade that stands in for an ellipsis.
const LABEL_FADE: f32 = 20.0;
/// pink: activity_center.rs `UNREAD_DOT`.
const UNREAD_DOT: f32 = 6.0;
/// How long the demo plays before it starts over.
const DEMO_LOOP: f32 = 28.0;

/// Where a task stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    /// Paused until you approve something (pink: awaiting approval).
    NeedsYou,
    /// Asked you a question (pink: awaiting input).
    NeedsReply,
    Running,
    Failed,
    /// Finished; a fresh result until someone reads it.
    Done,
    /// Stopped; nothing is going on (pink: idle).
    Cancelled,
}

/// What a person did with a task's row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxAction {
    /// Clicked the row: the task opens and its result counts as read.
    Open,
    Pin,
    Unpin,
    Archive,
}

/// One background task.
#[derive(Clone, Debug)]
pub struct InboxTask {
    pub id: SharedString,
    pub title: SharedString,
    pub project: SharedString,
    pub branch: Option<SharedString>,
    pub status: TaskStatus,
    /// Its result (or failure) was already read: a read task leaves the
    /// Activity view and waits in the full list only.
    pub seen: bool,
    pub pinned: bool,
    /// Seconds since its last message, counted in the row's corner.
    pub age: f32,
}

impl InboxTask {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        project: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            project: project.into(),
            branch: None,
            status: TaskStatus::Running,
            seen: false,
            pinned: false,
            age: 0.0,
        }
    }

    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    pub fn status(mut self, status: TaskStatus) -> Self {
        self.status = status;
        self
    }

    pub fn seen(mut self, seen: bool) -> Self {
        self.seen = seen;
        self
    }

    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    pub fn age(mut self, seconds: f32) -> Self {
        self.age = seconds;
        self
    }

    /// What the row shows (pink `ChatIndicator`): a result reads as done only
    /// until it is read.
    fn indicator(&self) -> Indicator {
        match self.status {
            TaskStatus::Running => Indicator::Working,
            TaskStatus::NeedsYou => Indicator::Approval,
            TaskStatus::NeedsReply => Indicator::Input,
            TaskStatus::Failed => Indicator::Errored,
            TaskStatus::Done if !self.seen => Indicator::Completed,
            TaskStatus::Done | TaskStatus::Cancelled => Indicator::Idle,
        }
    }

    /// Where the row lists by its own state (pink: spaces.rs
    /// `activity_section`); a read failure no longer needs you.
    fn live_section(&self) -> Option<Section> {
        match self.indicator() {
            Indicator::Errored if self.seen => None,
            Indicator::Approval | Indicator::Input | Indicator::Errored => Some(Section::NeedsYou),
            Indicator::Completed => Some(Section::Ready),
            Indicator::Working => Some(Section::Running),
            Indicator::Idle => None,
        }
    }

    /// Lights the bell's dot (pink: activity_center.rs `has_attention`).
    fn wants_you(&self) -> bool {
        match self.indicator() {
            Indicator::Approval | Indicator::Input | Indicator::Completed => true,
            Indicator::Errored => !self.seen,
            Indicator::Working | Indicator::Idle => false,
        }
    }
}

/// A scripted change for the demo: at `at` seconds, `task` replaces the task
/// with the same id (or arrives as a new one).
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
    selected: Option<SharedString>,
    activity: bool,
    autoplay: bool,
    width: f32,
    height: f32,
    on_action: Option<ActionHandler>,
}

impl AgentInbox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: "Activity".into(),
            tasks: Vec::new(),
            events: Vec::new(),
            selected: None,
            activity: true,
            autoplay: false,
            width: layout::SIDEBAR_DEFAULT,
            height: 600.0,
            on_action: None,
        }
    }

    /// The header's label while the Activity view is on.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn task(mut self, task: InboxTask) -> Self {
        self.tasks.push(task);
        self
    }

    /// Opens this task's row when the inbox first appears.
    pub fn selected(mut self, task_id: impl Into<SharedString>) -> Self {
        self.selected = Some(task_id.into());
        self
    }

    /// Starts in the Activity view (the default) or, with `false`, in the
    /// plain task list; the bell switches between them.
    pub fn activity_view(mut self, on: bool) -> Self {
        self.activity = on;
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

    /// The column's width (pink's sidebar: 224 to 400, 256 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The column's height; the list scrolls inside it.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
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
// Model
// ---------------------------------------------------------------------------

/// pink: proto `ChatIndicator`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Indicator {
    Working,
    Approval,
    Input,
    Errored,
    Completed,
    Idle,
}

/// Activity's sections, most urgent first (pink: spaces.rs `ActivitySection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    NeedsYou,
    Ready,
    Running,
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Section::NeedsYou => "needs-you",
            Section::Ready => "ready",
            Section::Running => "running",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::NeedsYou => "Needs you",
            Section::Ready => "Ready",
            Section::Running => "Running",
        }
    }
}

#[derive(Clone)]
struct Row {
    task: InboxTask,
    /// Clock time of its last message: newer rows list first.
    updated: f32,
}

/// Labels that overflowed at their last paint: they wear the edge fade.
type Clips = Rc<RefCell<HashSet<String>>>;

struct InboxState {
    initial: Vec<InboxTask>,
    initial_selected: Option<SharedString>,
    events: Vec<InboxEvent>,
    played: usize,
    rows: Vec<Row>,
    /// The Activity view (bell on), or the plain task list.
    activity: bool,
    /// The task being read.
    selected: Option<SharedString>,
    /// pink `ActivityHold`: the most urgent section the task being read has
    /// listed under since it was opened. It never lists below it.
    hold: Option<Section>,
    /// The row under the pointer: its corner trades the time for actions.
    hovered: Option<SharedString>,
    pinned_collapsed: bool,
    clock: f32,
    loop_start: f32,
    autoplay: bool,
    clips: Clips,
    // The resort glide (pink: shell.rs `render_chat_sidebar`).
    prev_order: Vec<(String, f32)>,
    resort: HashMap<String, f32>,
    new_keys: HashSet<String>,
    epoch: usize,
    /// A disclosure animates its own height, so its toggle is no resort.
    skip_resort: bool,
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
            initial_selected: config.selected.clone(),
            events: config.events.clone(),
            played: 0,
            rows: Vec::new(),
            activity: config.activity,
            selected: None,
            hold: None,
            hovered: None,
            pinned_collapsed: false,
            clock: 0.0,
            loop_start: 0.0,
            autoplay: config.autoplay,
            clips: Rc::default(),
            prev_order: Vec::new(),
            resort: HashMap::new(),
            new_keys: HashSet::new(),
            epoch: 0,
            skip_resort: false,
            last_tick: Instant::now(),
            on_action: None,
            _ticker: ticker,
        };
        state.load();
        state
    }

    /// (Re)loads the configured tasks and opens the configured row.
    fn load(&mut self) {
        self.played = 0;
        self.loop_start = self.clock;
        let clock = self.clock;
        self.rows = self
            .initial
            .iter()
            .map(|task| Row {
                task: task.clone(),
                updated: clock - task.age,
            })
            .collect();
        self.selected = None;
        self.hold = None;
        if let Some(id) = self.initial_selected.clone() {
            self.open(&id);
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
                self.load();
            }
        }
        cx.notify();
    }

    /// Replaces the task with the same id, or adds it.
    fn apply(&mut self, task: InboxTask) {
        let updated = self.clock - task.age;
        let reading = self.selected.as_ref() == Some(&task.id);
        if let Some(row) = self.rows.iter_mut().find(|r| r.task.id == task.id) {
            // What arrives in the task being read is read as it arrives.
            let seen = task.seen || reading;
            row.task = InboxTask {
                seen,
                pinned: row.task.pinned,
                ..task
            };
            row.updated = updated;
        } else {
            self.rows.push(Row { task, updated });
        }
    }

    /// Opens a row (pink: activity_center.rs `hold_activity_section`, then
    /// the open that reads it).
    fn open(&mut self, id: &SharedString) {
        let Some(row) = self.rows.iter().find(|r| &r.task.id == id) else {
            return;
        };
        self.hold = if self.activity {
            self.section_of(&row.task)
        } else {
            None
        };
        self.selected = Some(id.clone());
        if let Some(row) = self.rows.iter_mut().find(|r| &r.task.id == id) {
            row.task.seen = true;
        }
    }

    fn toggle_pin(&mut self, id: &SharedString) -> bool {
        let Some(row) = self.rows.iter_mut().find(|r| &r.task.id == id) else {
            return false;
        };
        row.task.pinned = !row.task.pinned;
        row.task.pinned
    }

    fn archive(&mut self, id: &SharedString) {
        self.rows.retain(|r| &r.task.id != id);
        if self.selected.as_ref() == Some(id) {
            self.selected = None;
            self.hold = None;
        }
        if self.hovered.as_ref() == Some(id) {
            self.hovered = None;
        }
    }

    /// Where a row lists in Activity (pink: spaces.rs
    /// `activity_sidebar_chats`): the task being read never lists below its
    /// held section, and stays listed while it is read.
    fn section_of(&self, task: &InboxTask) -> Option<Section> {
        let live = task.live_section();
        let reading = self.selected.as_ref() == Some(&task.id);
        let held = if reading { self.hold } else { None };
        let section = match (held, live) {
            (Some(held), Some(live)) => Some(held.min(live)),
            (held, live) => held.or(live),
        };
        section.or(reading.then_some(Section::Ready))
    }

    /// Activity's rows: by section, then newest first.
    fn activity_rows(&self) -> Vec<(Section, Row)> {
        let mut rows: Vec<(Section, Row)> = self
            .rows
            .iter()
            .filter_map(|row| self.section_of(&row.task).map(|s| (s, row.clone())))
            .collect();
        rows.sort_by(|(ls, l), (rs, r)| {
            ls.cmp(rs)
                .then(r.updated.total_cmp(&l.updated))
                .then(l.task.id.cmp(&r.task.id))
        });
        rows
    }

    /// The plain list: every task, newest first.
    fn thread_rows(&self) -> Vec<Row> {
        let mut rows = self.rows.clone();
        rows.sort_by(|l, r| {
            r.updated
                .total_cmp(&l.updated)
                .then(l.task.id.cmp(&r.task.id))
        });
        rows
    }

    /// Records the rendered order; a reorder glides the rows that moved and
    /// fades in the new ones (pink: shell.rs `resort_offsets`).
    fn note_order(&mut self, order: Vec<(String, f32)>) {
        if self.prev_order == order {
            return;
        }
        let skip = std::mem::take(&mut self.skip_resort);
        let reordered = self.prev_order.len() != order.len()
            || self
                .prev_order
                .iter()
                .zip(&order)
                .any(|((old, _), (new, _))| old != new);
        if !self.prev_order.is_empty() && reordered && !skip {
            let mut old_y = HashMap::new();
            let mut y = 0.0;
            for (key, height) in &self.prev_order {
                old_y.insert(key.as_str(), y);
                y += height;
            }
            let mut offsets = HashMap::new();
            let mut new_keys = HashSet::new();
            let mut y = 0.0;
            for (key, height) in &order {
                match old_y.get(key.as_str()) {
                    Some(prev) if (prev - y).abs() > 0.5 => {
                        offsets.insert(key.clone(), prev - y);
                    }
                    Some(_) => {}
                    None => {
                        new_keys.insert(key.clone());
                    }
                }
                y += height;
            }
            if !offsets.is_empty() || !new_keys.is_empty() {
                self.epoch += 1;
                self.resort = offsets;
                self.new_keys = new_keys;
            }
        }
        self.prev_order = order;
    }
}

/// One keyed entry of the rendered list.
enum Item {
    /// pink: activity_center.rs `activity_section_heading`.
    Heading {
        section: Section,
        count: usize,
        first: bool,
    },
    Task(Row),
    /// pink: the full list's Pinned disclosure group.
    Pinned {
        rows: Vec<Row>,
        collapsed: bool,
    },
    /// pink: the rule under the Pinned group.
    Divider,
}

impl Item {
    fn key(&self) -> String {
        match self {
            Item::Heading { section, .. } => format!("h:{}", section.key()),
            Item::Task(row) => format!("c:{}", row.task.id),
            Item::Pinned { .. } => "g:pinned".into(),
            Item::Divider => "divider:pinned".into(),
        }
    }

    fn height(&self) -> f32 {
        match self {
            Item::Heading { first, .. } => HEADING_H + if *first { 0.0 } else { SECTION_GAP },
            Item::Task(_) => ROW_H,
            Item::Pinned { rows, collapsed } => {
                SECTION_GAP
                    + HEADING_H
                    + if *collapsed {
                        0.0
                    } else {
                        BODY_INSET + ROW_H * rows.len() as f32
                    }
            }
            Item::Divider => DIVIDER_H,
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Compact relative time, no "ago" (pink: proto `format_time_ago`).
fn time_ago(seconds: f32) -> String {
    let s = seconds.max(0.0) as u64;
    if s < 60 {
        return "now".into();
    }
    let m = s / 60;
    if m < 60 {
        return format!("{m}m");
    }
    let h = m / 60;
    if h < 24 {
        return format!("{h}h");
    }
    let d = h / 24;
    if d < 7 {
        return format!("{d}d");
    }
    let w = d / 7;
    if w < 5 {
        return format!("{w}w");
    }
    let mo = d / 30;
    if mo < 12 {
        return format!("{mo}mo");
    }
    format!("{}y", d / 365)
}

/// pink: spaces.rs `status_dot_color` – every state but Done sits well below
/// full strength.
fn status_dot_color(indicator: Indicator, t: &Pink) -> Hsla {
    match indicator {
        Indicator::Working => t.busy.opacity(0.55),
        Indicator::Approval => t.warning.opacity(0.72),
        Indicator::Input => t.accent.opacity(0.6),
        Indicator::Errored => t.danger.opacity(0.65),
        Indicator::Completed => t.success.opacity(0.9),
        Indicator::Idle => t.ink(0.14),
    }
}

/// pink: shell.rs `sidebar_faded_label` – no ellipsis: a clipped label's last
/// glyphs fade out over 20 px into `fade_to`, the row's painted color.
fn faded_label(
    id: String,
    fill: bool,
    label: impl IntoElement,
    fade_to: Hsla,
    clips: &Clips,
) -> Div {
    let clipped = clips.borrow().contains(&id);
    let clips = clips.clone();
    div()
        .when(fill, |el| el.flex_1())
        .relative()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .child(div().flex_none().whitespace_nowrap().child(label))
        // A probe the size of the visible box, read against the label.
        .child(div().absolute().inset_0())
        .when(clipped, |el| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(LABEL_FADE))
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(fade_to.opacity(0.0), 0.0),
                        linear_color_stop(fade_to, 1.0),
                    )),
            )
        })
        .on_children_prepainted(move |bounds, window, _| {
            let (Some(label), Some(probe)) = (bounds.first(), bounds.get(1)) else {
                return;
            };
            let clipped = label.size.width > probe.size.width + px(0.5);
            let mut clips = clips.borrow_mut();
            if clipped != clips.contains(&id) {
                if clipped {
                    clips.insert(id.clone());
                } else {
                    clips.remove(&id);
                }
                // Read a frame late: draw once more with the fade settled.
                window.request_animation_frame();
            }
        })
}

/// A 20 px corner action (pink: shell.rs `render_chat_row` pin / archive):
/// radius 4, a quiet wash under the pointer, a 12 px glyph.
fn corner_action(
    id: String,
    label: &'static str,
    glyph: &'static str,
    tone: Hsla,
    t: &Pink,
) -> Stateful<Div> {
    div()
        .id(ElementId::Name(id.clone().into()))
        .role(Role::Button)
        .aria_label(label)
        .size(px(chrome::CONTROL_S))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_S))
        .cursor_pointer()
        .hover_wash(id, t.fill(FillTier::Tertiary))
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            svg()
                .path(glyph)
                .size(px(chrome::ICON_S))
                .flex_none()
                .text_color(tone),
        )
}

/// The row's details card after the hover delay (pink: shell.rs
/// `SidebarThreadTooltip`).
#[derive(Clone)]
struct TaskDetails {
    title: SharedString,
    project: SharedString,
    branch: Option<SharedString>,
    done: bool,
}

impl Render for TaskDetails {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let detail = |glyph: &'static str, label: SharedString| {
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .min_w_0()
                .child(
                    svg()
                        .path(glyph)
                        .size(px(13.0))
                        .flex_none()
                        .text_color(t.text_muted),
                )
                .child(div().min_w_0().truncate().child(label))
        };
        div()
            .w(px(244.0))
            .px(px(12.0))
            .py(px(11.0))
            .flex()
            .flex_col()
            .gap(px(7.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(t.border_strong)
            .bg(t.tooltip_surface())
            .shadow(t.tooltip_shadows())
            .font_family(pink::SANS)
            .text_size(px(11.0))
            .text_color(t.text_muted)
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(px(12.0))
                    .text_color(t.text)
                    .child(self.title.clone()),
            )
            .child(detail(ICON_FOLDER, self.project.clone()))
            .when_some(self.branch.clone(), |el, branch| {
                el.child(detail(ICON_BRANCH, branch))
            })
            .when(self.done, |el| el.child(detail(ICON_CHECK, "Done".into())))
    }
}

/// What every row of one render shares.
struct RowCx<'a> {
    key: &'a str,
    t: &'a Pink,
    state: &'a Entity<InboxState>,
    clips: &'a Clips,
    /// The sidebar surface the rows sit on.
    surface: Hsla,
    clock: f32,
    selected: Option<&'a SharedString>,
    hovered: Option<&'a SharedString>,
}

/// One task row (pink: shell.rs `Shell::render_chat_row`, two-line sidebar
/// row): 52 tall, `px 8 py 6`, radius 8; the title over the meta line, the
/// time (or the hover actions) in the corner, the status at the right edge.
fn task_row(c: &RowCx, row: &Row) -> AnyElement {
    let t = c.t;
    let task = &row.task;
    let row_id = format!("{}-row-{}", c.key, task.id);
    let fade_key = format!("{row_id}-hover");
    let selected = c.selected == Some(&task.id);
    let corner_hovered = c.hovered == Some(&task.id);
    let indicator = task.indicator();
    let status_color = status_dot_color(indicator, t);

    // Working keeps the square loader and Done its dot; a task waiting on
    // you names the next step in a pill wearing the glyph of what it waits for.
    let pill = |word: &'static str, glyph: &'static str| {
        pink::status_pill(word, status_color, Some(glyph.into()), t).into_any_element()
    };
    let status_badge = match indicator {
        Indicator::Working => Some(
            loaders::mini_glyph_spinner(format!("{row_id}-working"), 2.0, t.glyph)
                .into_any_element(),
        ),
        Indicator::Completed => Some(
            div()
                .size(px(6.0))
                .flex_none()
                .rounded_full()
                .bg(status_color)
                .into_any_element(),
        ),
        Indicator::Approval => Some(pill("Approve", ICON_SHIELD)),
        Indicator::Input => Some(pill("Reply", ICON_QUESTION)),
        Indicator::Errored => Some(pill("Failed", ICON_DANGER)),
        Indicator::Idle => None,
    }
    .map(|badge| div().flex_none().flex().items_center().child(badge));

    // The wash and the title brighten ride one 150 ms blend; a selected row
    // never drifts toward the hover wash.
    let text = t.text;
    let selected_wash = t.glass_selected_bg();
    let rest_bg = if selected { selected_wash } else { t.wash(0.0) };
    let hover_bg = if selected {
        selected_wash
    } else {
        t.sidebar_hover_bg()
    };
    let rest_text = if selected {
        text
    } else if indicator == Indicator::Completed {
        text.opacity(0.95)
    } else {
        text.opacity(0.8)
    };
    let hover = motion::hover_t_or(&fade_key, if corner_hovered { 1.0 } else { 0.0 });
    let row_bg = motion::mix(rest_bg, hover_bg, hover);
    let fade_to = pink::flatten(row_bg, c.surface);
    // Project and branch are information: quiet, held to the text floor.
    let subline = t.readable_muted(0.5);

    // At rest the corner is the time alone; under the pointer, the row's
    // pin and archive.
    let corner_body = if corner_hovered {
        let pin_id = task.id.clone();
        let archive_id = task.id.clone();
        let pinned = task.pinned;
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(px(2.0))
            .h(px(18.0))
            .child(
                corner_action(
                    format!("{row_id}-pin"),
                    if pinned { "Unpin task" } else { "Pin task" },
                    ICON_PIN,
                    if pinned {
                        t.readable_icon(t.accent)
                    } else {
                        t.text_muted
                    },
                    t,
                )
                .on_click({
                    let state = c.state.clone();
                    move |_, window, cx| {
                        cx.stop_propagation();
                        let (handler, pinned) = state.update(cx, |s, cx| {
                            s.autoplay = false;
                            let pinned = s.toggle_pin(&pin_id);
                            cx.notify();
                            (s.on_action.clone(), pinned)
                        });
                        let action = if pinned {
                            InboxAction::Pin
                        } else {
                            InboxAction::Unpin
                        };
                        if let Some(handler) = handler {
                            handler(&pin_id, action, window, cx);
                        }
                    }
                }),
            )
            .child(
                corner_action(
                    format!("{row_id}-archive"),
                    "Archive task",
                    ICON_ARCHIVE,
                    t.text_muted,
                    t,
                )
                .on_click({
                    let state = c.state.clone();
                    move |_, window, cx| {
                        cx.stop_propagation();
                        let handler = state.update(cx, |s, cx| {
                            s.autoplay = false;
                            s.archive(&archive_id);
                            cx.notify();
                            s.on_action.clone()
                        });
                        if let Some(handler) = handler {
                            handler(&archive_id, InboxAction::Archive, window, cx);
                        }
                    }
                }),
            )
            .into_any_element()
    } else {
        div()
            .h(px(18.0))
            .flex()
            .items_center()
            .text_size(px(11.0))
            .font_features(pink::tabular_figures())
            .child(time_ago(c.clock - row.updated))
            .into_any_element()
    };
    let corner = div()
        .id(ElementId::Name(format!("{row_id}-corner").into()))
        .flex_none()
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_end()
        .child(corner_body);

    // Line 2: project · branch on the title's left edge, the status at the right.
    let meta = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .text_size(px(11.0))
        .line_height(px(14.0))
        .text_color(subline)
        .child(faded_label(
            format!("{row_id}-project"),
            false,
            task.project.clone(),
            fade_to,
            c.clips,
        ))
        .when_some(task.branch.clone(), |el, branch| {
            el.child(div().flex_none().child("·"))
                .child(
                    svg()
                        .path(ICON_BRANCH)
                        .size(px(11.0))
                        .flex_none()
                        .text_color(subline),
                )
                .child(faded_label(
                    format!("{row_id}-branch"),
                    false,
                    branch,
                    fade_to,
                    c.clips,
                ))
        });

    let details = TaskDetails {
        title: task.title.clone(),
        project: task.project.clone(),
        branch: task.branch.clone(),
        done: indicator == Indicator::Completed,
    };
    let open_id = task.id.clone();
    let hover_id = task.id.clone();
    div()
        .id(ElementId::Name(row_id.clone().into()))
        .h(px(ROW_H))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.0))
        .rounded(px(radius::MD))
        .px(px(space::SM))
        .py(px(6.0))
        .text_color(motion::mix(rest_text, text, hover))
        .bg(row_bg)
        // One listener drives both the wash blend and the corner's swap.
        .on_hover({
            let fade = motion::hover_listener(fade_key);
            let state = c.state.clone();
            move |hovered: &bool, window, cx| {
                fade(hovered, window, cx);
                state.update(cx, |s, cx| {
                    if *hovered {
                        if s.hovered.as_ref() != Some(&hover_id) {
                            s.hovered = Some(hover_id.clone());
                            cx.notify();
                        }
                    } else if s.hovered.as_ref() == Some(&hover_id) {
                        s.hovered = None;
                        cx.notify();
                    }
                });
            }
        })
        .cursor_pointer()
        .role(Role::Button)
        .aria_selected(selected)
        .aria_label(task.title.clone())
        .on_click({
            let state = c.state.clone();
            move |_, window, cx| {
                let handler = state.update(cx, |s, cx| {
                    s.autoplay = false;
                    s.open(&open_id);
                    cx.notify();
                    s.on_action.clone()
                });
                if let Some(handler) = handler {
                    handler(&open_id, InboxAction::Open, window, cx);
                }
            }
        })
        .tooltip(move |_, cx| cx.new(|_| details.clone()).into())
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        // Line 1: the title, then the time (or the hover actions).
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::SM))
                .child(faded_label(
                    format!("{row_id}-title"),
                    true,
                    div()
                        .text_size(px(13.0))
                        .line_height(px(17.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(task.title.clone()),
                    fade_to,
                    c.clips,
                ))
                .child(div().text_color(subline).child(corner)),
        )
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(meta)
                .children(status_badge),
        )
        .into_any_element()
}

/// One Activity section's heading (pink: activity_center.rs
/// `activity_section_heading`): a heading, not a disclosure – what needs you
/// never folds away. 11 semibold label, the count beside it.
fn section_heading(key: &str, section: Section, count: usize, first: bool, t: &Pink) -> AnyElement {
    let tasks = if count == 1 { "task" } else { "tasks" };
    div()
        .w_full()
        .flex_none()
        .flex()
        .flex_col()
        .pt(px(if first { 0.0 } else { SECTION_GAP }))
        .child(
            div()
                .id(ElementId::Name(
                    format!("{key}-section-{}", section.key()).into(),
                ))
                .role(Role::Heading)
                .aria_label(format!("{}, {count} {tasks}", section.label()))
                .h(px(HEADING_H))
                .px(px(space::SM))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.readable_faint(0.9))
                        .child(section.label()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.0))
                        .text_color(t.readable_faint(0.7))
                        .child(count.to_string()),
                ),
        )
        .into_any_element()
}

/// The full list's Pinned group (pink: spaces.rs `sidebar_disclosure_header`
/// and `render_sidebar_disclosure_body`): a 28 px header with a quiet wash and
/// the chevron only under the pointer; the body folds over `COLLAPSE`.
fn pinned_group(c: &RowCx, rows: &[Row], collapsed: bool) -> AnyElement {
    let t = c.t;
    let header_id = format!("{}-pinned", c.key);
    let group: SharedString = format!("{header_id}-header").into();
    let reveal = motion::follow(
        &format!("{header_id}-open"),
        if collapsed { 0.0 } else { 1.0 },
        COLLAPSE,
    );
    let body_h = BODY_INSET + ROW_H * rows.len() as f32;
    let label: SharedString = if collapsed {
        format!("Pinned ({})", rows.len()).into()
    } else {
        "Pinned".into()
    };
    let header = div()
        .id(ElementId::Name(header_id.clone().into()))
        .group(group.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .h(px(HEADING_H))
        .px(px(space::SM))
        .rounded(px(radius::SM))
        .cursor_pointer()
        .hover_wash(header_id.clone(), t.fill(FillTier::Tertiary))
        .role(Role::Button)
        .aria_expanded(!collapsed)
        .on_click({
            let state = c.state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.autoplay = false;
                    s.pinned_collapsed = !s.pinned_collapsed;
                    s.skip_resort = true;
                    cx.notify();
                })
            }
        })
        .child(faded_label(
            format!("{header_id}-label"),
            false,
            div()
                .text_size(px(11.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.readable_faint(0.9))
                .child(label),
            c.surface,
            c.clips,
        ))
        .child(div().flex_1())
        .child(
            div()
                .flex_none()
                .opacity(0.0)
                .group_hover(group, |s| s.opacity(1.0))
                .child(
                    svg()
                        .path(ICON_CHEVRON)
                        .size(px(12.0))
                        .text_color(t.readable_icon(t.readable_muted(0.5)))
                        .with_transformation(Transformation::rotate(radians(reveal * FRAC_PI_2))),
                ),
        );
    let body = div()
        .w_full()
        .flex_none()
        .overflow_hidden()
        .h(px(body_h * reveal))
        .opacity(0.35 + 0.65 * reveal)
        .relative()
        .top(px(-3.0 * (1.0 - reveal)))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .pt(px(BODY_INSET))
                .children(rows.iter().map(|row| task_row(c, row))),
        );
    div()
        .w_full()
        .flex()
        .flex_col()
        .pt(px(SECTION_GAP))
        .child(header)
        .child(body)
        .into_any_element()
}

/// The header's Activity toggle (pink: activity_center.rs
/// `render_activity_action`): a 28 px bell, accent while the view is on, with
/// a 6 px accent dot on its shoulder while something needs you.
fn bell(c: &RowCx, active: bool, has_attention: bool) -> Stateful<Div> {
    let t = c.t;
    let id = format!("{}-activity", c.key);
    let label = if active {
        "Turn off activity view"
    } else {
        "View activity"
    };
    let ring = t.focus_ring();
    div()
        .id(ElementId::Name(id.clone().into()))
        .relative()
        .size(px(chrome::CONTROL_M))
        .flex_none()
        .rounded(px(chrome::RADIUS_M))
        .border_1()
        .border_color(t.border.opacity(0.0))
        .flex()
        .items_center()
        .justify_center()
        .role(Role::Button)
        .aria_label(label)
        .aria_toggled(if active {
            Toggled::True
        } else {
            Toggled::False
        })
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .cursor_pointer()
        .hover_fill(
            id,
            if active {
                t.glass_hover()
            } else {
                t.glass_hover().opacity(0.0)
            },
            t.glass_hover(),
        )
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_click({
            let state = c.state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.autoplay = false;
                    s.activity = !s.activity;
                    cx.notify();
                })
            }
        })
        .on_key_down(pink::swallow_activation_key)
        .child(
            svg()
                .path(ICON_BELL)
                .size(px(16.0))
                .flex_none()
                .text_color(if active { t.accent } else { t.text_muted }),
        )
        .when(has_attention, |el| {
            el.child(
                div()
                    .absolute()
                    .top(px(4.5))
                    .right(px(5.5))
                    .size(px(UNREAD_DOT))
                    .rounded_full()
                    .bg(t.accent),
            )
        })
}

/// A centred empty list with the next step (pink: shell.rs
/// `render_chat_sidebar`, audit SH-24).
fn empty_state(activity: bool, t: &Pink) -> AnyElement {
    let (title, hint) = if activity {
        (
            "Nothing needs attention",
            "Running and waiting tasks appear here.",
        )
    } else {
        ("No tasks yet", "Start one with ⌘N.")
    };
    div()
        .px(px(space::SM))
        .py(px(space::LG))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(2.0))
        .text_center()
        .child(
            div()
                .text_size(px(13.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.readable_muted(0.9))
                .child(title),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(t.readable_faint(0.9))
                .child(hint),
        )
        .into_any_element()
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

        // -- What lists, in order ---------------------------------------------
        let s = state.read(cx);
        let activity = s.activity;
        let mut items = Vec::new();
        let mut drawn_selected = None;
        if activity {
            let rows = s.activity_rows();
            let mut current: Option<Section> = None;
            for (ix, (section, row)) in rows.iter().enumerate() {
                if current != Some(*section) {
                    let count = rows[ix..].iter().take_while(|(s, _)| s == section).count();
                    items.push(Item::Heading {
                        section: *section,
                        count,
                        first: current.is_none(),
                    });
                    current = Some(*section);
                }
                if s.selected.as_ref() == Some(&row.task.id) {
                    drawn_selected = Some(*section);
                }
                items.push(Item::Task(row.clone()));
            }
        } else {
            let (pinned, rest): (Vec<Row>, Vec<Row>) =
                s.thread_rows().into_iter().partition(|row| row.task.pinned);
            if !pinned.is_empty() {
                let has_rest = !rest.is_empty();
                items.push(Item::Pinned {
                    rows: pinned,
                    collapsed: s.pinned_collapsed,
                });
                if has_rest && !s.pinned_collapsed {
                    items.push(Item::Divider);
                }
            }
            items.extend(rest.into_iter().map(Item::Task));
        }
        let has_attention = s.rows.iter().any(|row| row.task.wants_you());
        let selected = s.selected.clone();
        let hovered = s.hovered.clone();
        let clock = s.clock;
        let clips = s.clips.clone();

        // A held row drawn in a more urgent section keeps it while it is
        // read; then the resort glide (pink: shell.rs `RESORT`).
        let order: Vec<(String, f32)> = items.iter().map(|i| (i.key(), i.height())).collect();
        let (epoch, resort, new_keys) = state.update(cx, |s, _| {
            if let (Some(drawn), Some(hold)) = (drawn_selected, s.hold) {
                s.hold = Some(hold.min(drawn));
            }
            s.note_order(order);
            (s.epoch, s.resort.clone(), s.new_keys.clone())
        });

        // -- Elements -----------------------------------------------------------
        let surface = t.chrome_surface();
        let c = RowCx {
            key: &key,
            t: &t,
            state: &state,
            clips: &clips,
            surface,
            clock,
            selected: selected.as_ref(),
            hovered: hovered.as_ref(),
        };
        let is_empty = items.is_empty();
        let list_items: Vec<AnyElement> = items
            .iter()
            .map(|item| {
                let item_key = item.key();
                let element = match item {
                    Item::Heading {
                        section,
                        count,
                        first,
                    } => section_heading(&key, *section, *count, *first, &t),
                    Item::Task(row) => task_row(&c, row),
                    Item::Pinned { rows, collapsed } => pinned_group(&c, rows, *collapsed),
                    Item::Divider => div()
                        .w_full()
                        .pt(px(space::SM))
                        .pb(px(space::SM))
                        .px(px(space::SM))
                        .child(div().h(px(1.0)).bg(t.border.opacity(0.6)))
                        .into_any_element(),
                };
                // Surviving rows glide from their old place; new rows fade
                // in; removals just go.
                if let Some(dy) = resort.get(&item_key).copied() {
                    div()
                        .child(element)
                        .with_animation(
                            ElementId::Name(format!("{key}-resort-{epoch}-{item_key}").into()),
                            RESORT.animation(),
                            move |el, p| el.relative().top(px(dy * (1.0 - p))),
                        )
                        .into_any_element()
                } else if new_keys.contains(&item_key) {
                    motion::fade_quick(
                        ElementId::Name(format!("{key}-row-in-{epoch}-{item_key}").into()),
                        div().child(element),
                    )
                    .into_any_element()
                } else {
                    element
                }
            })
            .collect();

        // pink: spaces.rs `render_threads_header` – the label turns into
        // "Activity" while the bell is on.
        let header = div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .pt(px(4.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(29.0))
                    .px(px(space::SM))
                    .flex()
                    .items_center()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if activity {
                        t.text
                    } else {
                        t.readable_faint(0.9)
                    })
                    .child(if activity {
                        self.title.clone()
                    } else {
                        "Threads".into()
                    }),
            )
            .child(bell(&c, activity, has_attention));

        let list = div()
            .id(ElementId::Name(format!("{key}-list").into()))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .pt(px(4.0))
            .when(is_empty, |el| el.child(empty_state(activity, &t)))
            .children(list_items);

        motion::keep_alive(window);
        // The sidebar column on the shell surface, with pink's left gutter
        // (and the canvas inset standing in on the right).
        div()
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .px(px(GUTTER))
            .pb(px(GUTTER))
            .rounded(px(radius::PANEL))
            .overflow_hidden()
            .bg(surface)
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(header)
            .child(list)
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentInbox::new("agent-inbox")
            .width(288.0)
            .height(640.0)
            .task(
                InboxTask::new("coupons", "Retire expired coupon codes", "billing-api")
                    .branch("coupons/retire")
                    .status(TaskStatus::NeedsYou)
                    .age(72.0),
            )
            .task(
                InboxTask::new("exports", "Pick a date format for exports", "reports-web")
                    .status(TaskStatus::NeedsReply)
                    .age(240.0),
            )
            .task(
                InboxTask::new("crm", "Sync CRM contacts", "crm-sync")
                    .branch("sync/backoff")
                    .status(TaskStatus::Failed)
                    .age(760.0),
            )
            .task(
                InboxTask::new("digest", "Weekly revenue digest", "finance-bots")
                    .status(TaskStatus::Done)
                    .age(1440.0),
            )
            .task(
                InboxTask::new("invoices", "Backfill invoice PDFs", "billing-api")
                    .branch("invoices/backfill")
                    .age(25.0),
            )
            .task(InboxTask::new("churn", "Summarize churn interviews", "research").age(130.0))
            .task(
                InboxTask::new(
                    "flags",
                    "Audit unused feature flags across the web app",
                    "web-app",
                )
                .branch("chore/flag-audit")
                .age(320.0),
            )
            .task(
                InboxTask::new("postgres", "Upgrade the Postgres driver", "billing-api")
                    .branch("deps/pg")
                    .status(TaskStatus::Done)
                    .seen(true)
                    .age(7200.0),
            )
            .task(
                InboxTask::new("notes", "Draft the 4.2 release notes", "docs")
                    .status(TaskStatus::Cancelled)
                    .age(172_800.0),
            )
            .selected("invoices")
            .event(
                3.5,
                InboxTask::new("invoices", "Backfill invoice PDFs", "billing-api")
                    .branch("invoices/backfill")
                    .status(TaskStatus::NeedsYou),
            )
            .event(
                7.5,
                InboxTask::new("churn", "Summarize churn interviews", "research")
                    .status(TaskStatus::Done),
            )
            .event(
                11.0,
                InboxTask::new("tickets", "Triage new support tickets", "support-desk"),
            )
            .event(
                15.5,
                InboxTask::new(
                    "flags",
                    "Audit unused feature flags across the web app",
                    "web-app",
                )
                .branch("chore/flag-audit")
                .status(TaskStatus::Done),
            )
            .event(
                20.0,
                InboxTask::new("exports", "Pick a date format for exports", "reports-web"),
            )
            .autoplay(true)
    })
}
