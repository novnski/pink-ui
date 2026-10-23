//! Agent Steering – talk to an agent while it works, at the bottom of a pink
//! chat pane.
//!
//! The pane reproduces how pink shows a running chat and the notes typed
//! into it (`transcript.rs`, `composer.rs`, `queue.rs`, `composer/goals.rs`,
//! `composer/checklist.rs`):
//!
//! - **The transcript tail**: the task as the first user bubble, one tool
//!   group per step with its activity on the activity rail (a finished step
//!   folds to its header), every note that reached the agent as a user
//!   bubble, and the working line (orb · status · timer, hover "Stop").
//! - **Above the composer**: the checklist line ("2/6 · In progress: …"),
//!   which unfolds into a line per step, and the goal bar: the agent's
//!   status, the task, the run clock and Pause / Resume.
//! - **The queue tray** tucked behind the composer: notes typed while the
//!   agent works. A queued note waits for the next step; drag a row to
//!   reorder it, edit it in the composer, remove it, or deliver it now.
//!   **Steer now** lands it in the running step without interrupting;
//!   **Send now** interrupts the step and restarts it with the note.
//! - **The composer pill**: Enter queues the note (or steers it, with the
//!   Steer chip on), ⌘/Ctrl+Shift+Enter does the other once, ⌘/Ctrl+Enter on
//!   an empty composer delivers the newest note, and the disc stops the run.
//!
//! ```ignore
//! AgentSteering::new("reindex", "Indexer", "Rebuild the product search index")
//!     .step(SteerStep::new("Map old analyzers to the new schema").activity("Reading 41 analyzer configs"))
//!     .step(SteerStep::new("Reindex 2.4M products"))
//!     .on_steer(|note, _, _| println!("{} ({:?})", note.text, note.mode))
//! ```

use std::{collections::HashMap, f32::consts::FRAC_PI_2, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyElement, AnyView, App, BoxShadow, ClickEvent, Context, CursorStyle, Div,
    DragMoveEvent, ElementId, EmptyView, Entity, FillOptions, FillRule, FontWeight, IntoElement,
    KeyDownEvent, MouseButton, PathBuilder, PathStyle, Pixels, Point, Role, ScrollHandle,
    SharedString, Stateful, Subscription, Task, Transformation, Window, canvas, div,
    linear_color_stop, linear_gradient, point, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    motion::{self, DISCLOSURE, GLYPH_SWAP, TAB_SLIDE, TOOL_CONNECTOR_REVEAL, TOOL_ROW_REVEAL},
    pink::{
        self, FillTier, Pink, layout,
        loaders::{
            LIVE_STATUS_OBJECT_CHARS, TOOL_LABEL_LINE_HEIGHT, TOOL_LABEL_SIZE,
            WORKING_STATUS_LINE_HEIGHT, WORKING_TIMER_SIZE, ellipsize_chars, format_elapsed,
            mini_glyph_spinner, orb_slot, shimmer_text, tabular_figures, working_status,
            working_timer,
        },
        radius,
    },
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Agent Steering";
pub const DESCRIPTION: &str = "A running agent's chat pane – notes typed while it works wait in a queue tray behind the composer, where you reorder or edit them, steer one into the running step or send it now to interrupt, under the goal bar, checklist and live activity of the run";

const ICON_ARROW_UP: &str = "icons/agent-steering/arrow-up.svg";
const ICON_CHEVRON_DOWN: &str = "icons/agent-steering/alt-arrow-down.svg";
const ICON_CHEVRON_RIGHT: &str = "icons/agent-steering/alt-arrow-right.svg";
const ICON_STEER: &str = "icons/agent-steering/bolt-fast.svg";
const ICON_DRAG: &str = "icons/agent-steering/queue-drag-handle.svg";
const ICON_EDIT: &str = "icons/agent-steering/pen.svg";
const ICON_REMOVE: &str = "icons/agent-steering/trash-bin-minimalistic.svg";
const ICON_SAVE: &str = "icons/agent-steering/queue-check.svg";
const ICON_CANCEL: &str = "icons/agent-steering/queue-close.svg";
const ICON_QUEUE_SEND: &str = "icons/agent-steering/queue-send.svg";
const ICON_TARGET: &str = "icons/agent-steering/target.svg";
const ICON_PAUSE: &str = "icons/agent-steering/pause.svg";
const ICON_PLAY: &str = "icons/agent-steering/play.svg";
const ICON_FILE_TEXT: &str = "icons/agent-steering/file-text.svg";
const ICON_FILE_SEARCH: &str = "icons/agent-steering/file-search.svg";
const ICON_FILE_PEN: &str = "icons/agent-steering/file-pen.svg";
const ICON_TERMINAL: &str = "icons/agent-steering/terminal.svg";
const ICON_TOOLBOX: &str = "icons/agent-steering/toolbox.svg";
const ICON_LAYERS: &str = "icons/agent-steering/layers-minimalistic.svg";

/// The chat pane: pink's content panel (`radius::PANEL`, `bg`, `border`).
const PANEL_W: f32 = 632.;
const PANEL_H: f32 = 600.;
/// pink `transcript_row_gutter` (48 from 480 wide up).
const TRANSCRIPT_GUTTER: f32 = 48.;
/// pink `composer_gutter` (`Theme::SPACE_LG`).
const COMPOSER_GUTTER: f32 = 16.;
/// The composer column's gap (pink `Theme::SPACE_SM`).
const STACK_GAP: f32 = 8.;
/// Trays are inset from the column and tucked behind the next layer (pink
/// `QUEUE_SIDE_INSET`, `QUEUE_COMPOSER_OVERLAP`).
const TRAY_INSET: f32 = 16.;
const TRAY_OVERLAP: f32 = 18.;

/// pink `queue.rs`: compact, borderless rows inside the tray.
const ROW_HEIGHT: f32 = 36.;
const QUEUE_TEXT_SIZE: f32 = 12.5;
const ROW_PAD_X: f32 = 8.;
const ROW_RADIUS: f32 = 8.;
const QUEUE_ICON_SIZE: f32 = 13.;
const QUEUE_ACTION_SIZE: f32 = 28.;
const QUEUE_PRIMARY_W: f32 = 72.;
/// Rows shown before the tray scrolls.
const QUEUE_VISIBLE_ROWS: f32 = 4.;

/// pink `composer/goals.rs`: the goal bar.
const GOAL_HEADER_HEIGHT: f32 = 38.;
const GOAL_GLYPH: f32 = 14.;
const GOAL_CONTROL: f32 = 24.;
/// pink `GOAL_CHIP_ICON`, the composer's mode chip glyph.
const MODE_CHIP_ICON: f32 = 15.;

/// pink `composer/checklist.rs`.
const CHECKLIST_ROW: f32 = 28.;
const CHECKLIST_TEXT: f32 = 11.5;
const CHECKLIST_LIST_MAX: f32 = 144.;

/// pink `composer.rs`: the pill's textarea and controls row.
const INPUT_TEXT_SIZE: f32 = 14.;
const INPUT_LINE_HEIGHT: f32 = 22.75;
const TEXTAREA_PAD_TOP: f32 = 16.;
const TEXTAREA_PAD_BOTTOM: f32 = 4.;
const SESSION_TEXTAREA_MIN: f32 = 60.;
const ACTIONS_ROW_HEIGHT: f32 = 46.;

/// pink `transcript.rs`: tool groups, the activity rail and user bubbles.
const TOOL_GROUP_HEADER_HEIGHT: f32 = 26.;
const TOOL_TREE_ROW_HEIGHT: f32 = 32.;
const CHIP_CARD_HEIGHT: f32 = 30.;
const CHIPS_TOP_PAD: f32 = 2.;
const ACTIVITY_GUTTER_WIDTH: f32 = 48.;
const ACTIVITY_TEXT_GAP: f32 = 8.;
const ACTIVITY_TRUNK_X: f32 = 12.5;
const ACTIVITY_BEND_RADIUS: f32 = 6.;
const ACTIVITY_BRANCH_END_X: f32 = 28.;
const ACTIVITY_ICON_LEFT: f32 = 32.;
const ACTIVITY_ICON_SIZE: f32 = 14.;
const DISCLOSURE_CHEVRON_SIZE: f32 = 12.;
const USER_LINE_HEIGHT: f32 = 22.;
const USER_BUBBLE_SHARE: f32 = 0.8;
/// Transcript rhythm: a new turn, a tool group, the working line.
const TURN_GAP: f32 = 16.;
const GROUP_GAP: f32 = 12.;

/// Seconds a step takes, and when its activity lines appear.
const STEP_SECS: f32 = 8.0;
const LINE_EVERY: f32 = 2.4;
/// Where the demo picks the first step up: its first line just in.
const START_STEP_T: f32 = 0.6;
const TYPE_EVERY: f32 = 0.035;
/// A delivery in flight ("Requesting steer…", "Sending…"): steer accepted
/// at the first mark, landed at the second.
const STEER_ACCEPTED_AT: f32 = 0.45;
const CLAIM_SECS: f32 = 0.9;
const TICK: Duration = Duration::from_millis(50);

/// How a note reached the agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SteerMode {
    /// Queued: it waits for the agent's next step.
    Queue,
    /// Steered into the running step without interrupting it (pink "Steer now").
    Steer,
    /// Sent at once (pink "Send now"): interrupts the running step, or
    /// resumes a paused run.
    Send,
}

/// What Enter does with a note while the agent works (pink
/// `FollowUpBehavior`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FollowUp {
    /// Queue it for the next step.
    #[default]
    Queue,
    /// Steer it into the running step.
    Steer,
}

impl FollowUp {
    fn opposite(self) -> Self {
        match self {
            Self::Queue => Self::Steer,
            Self::Steer => Self::Queue,
        }
    }
}

/// A note sent to the agent.
#[derive(Clone, Debug)]
pub struct SteerNote {
    pub text: SharedString,
    pub mode: SteerMode,
}

/// One step of the agent's run, with the activity it reports. Write activity
/// as "Reading 41 configs": the first word is the working line's verb and
/// picks the row's glyph.
#[derive(Clone, Debug)]
pub struct SteerStep {
    pub title: SharedString,
    pub activity: Vec<SharedString>,
}

impl SteerStep {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            activity: Vec::new(),
        }
    }

    pub fn activity(mut self, line: impl Into<SharedString>) -> Self {
        self.activity.push(line.into());
        self
    }
}

type NoteHandler = Rc<dyn Fn(&SteerNote, &mut Window, &mut App)>;
type PauseHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[derive(Clone)]
struct Rehearsal {
    at: f32,
    text: SharedString,
    follow_up: FollowUp,
}

#[derive(IntoElement)]
pub struct AgentSteering {
    id: ElementId,
    agent: SharedString,
    task: SharedString,
    steps: Vec<SteerStep>,
    start_at: usize,
    elapsed: f32,
    follow_up: FollowUp,
    steerable: bool,
    autoplay: bool,
    queued: Vec<SharedString>,
    rehearsals: Vec<Rehearsal>,
    on_steer: Option<NoteHandler>,
    on_pause: Option<PauseHandler>,
}

impl AgentSteering {
    pub fn new(
        id: impl Into<ElementId>,
        agent: impl Into<SharedString>,
        task: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            agent: agent.into(),
            task: task.into(),
            steps: Vec::new(),
            start_at: 0,
            elapsed: 0.0,
            follow_up: FollowUp::Queue,
            steerable: true,
            autoplay: false,
            queued: Vec::new(),
            rehearsals: Vec::new(),
            on_steer: None,
            on_pause: None,
        }
    }

    pub fn step(mut self, step: SteerStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Steps before `index` are already done when the component appears.
    pub fn start_at(mut self, index: usize) -> Self {
        self.start_at = index;
        self
    }

    /// Seconds the run has been going when the component appears.
    pub fn elapsed(mut self, seconds: f32) -> Self {
        self.elapsed = seconds;
        self
    }

    /// What Enter does while the agent works (default: queue for the next
    /// step). The composer's Steer chip flips it.
    pub fn follow_up(mut self, follow_up: FollowUp) -> Self {
        self.follow_up = follow_up;
        self
    }

    /// Whether the agent takes notes in the middle of a step (default
    /// `true`). Without it a steered note is held for the next step, and its
    /// row offers only Send now.
    pub fn steerable(mut self, steerable: bool) -> Self {
        self.steerable = steerable;
        self
    }

    /// A note already waiting in the queue when the component appears.
    pub fn queued(mut self, text: impl Into<SharedString>) -> Self {
        self.queued.push(text.into());
        self
    }

    /// Loops the run on its own (and plays the rehearsed notes) until someone
    /// steps in.
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    /// During autoplay, types `text` into the composer `at` seconds in and
    /// queues it for the next step.
    pub fn rehearse(mut self, at: f32, text: impl Into<SharedString>) -> Self {
        self.rehearsals.push(Rehearsal {
            at,
            text: text.into(),
            follow_up: FollowUp::Queue,
        });
        self
    }

    /// Like [`rehearse`](Self::rehearse), with the Steer chip on: the note
    /// steers the running step.
    pub fn rehearse_steer(mut self, at: f32, text: impl Into<SharedString>) -> Self {
        self.rehearsals.push(Rehearsal {
            at,
            text: text.into(),
            follow_up: FollowUp::Steer,
        });
        self
    }

    /// Called when a note is queued, steered into the running step, or sent
    /// at once.
    pub fn on_steer(
        mut self,
        handler: impl Fn(&SteerNote, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_steer = Some(Rc::new(handler));
        self
    }

    /// Called with `true` when the run is paused and `false` when it resumes.
    pub fn on_pause(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_pause = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// One line of a step's transcript: an activity row or a note that landed.
/// `at` is when it reveals (`None`: it was there when the pane appeared).
#[derive(Clone)]
enum Entry {
    Activity {
        text: SharedString,
        at: Option<Instant>,
    },
    Note {
        id: usize,
        text: SharedString,
        at: Option<Instant>,
    },
}

/// The one delivery action a queue row advertises (pink
/// `QueuePrimaryAction`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Primary {
    SendNow,
    SteerNow,
}

impl Primary {
    fn label(self) -> &'static str {
        match self {
            Self::SendNow => "Send now",
            Self::SteerNow => "Steer now",
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Self::SendNow => "Send now (interrupts an active response)",
            Self::SteerNow => "Steer now (without interrupting)",
        }
    }
}

/// A delivery in flight, timed on the demo clock.
#[derive(Clone, Copy)]
struct Claim {
    action: Primary,
    since: f32,
}

/// A note in the queue tray (pink `QueuedMessage`).
#[derive(Clone)]
struct Queued {
    id: usize,
    text: SharedString,
    follow_up: FollowUp,
    claim: Option<Claim>,
}

/// A queue row being dragged (pink `QueueDragPayload`), scoped to its pane.
struct NoteDrag {
    owner: SharedString,
    from: usize,
}

/// Where the dragged row would land (pink `QueueDragState`).
struct DragState {
    from: usize,
    over: usize,
    prev_over: usize,
    epoch: usize,
}

struct Typing {
    text: SharedString,
    follow_up: FollowUp,
    shown: usize,
    started: f32,
}

struct SteerState {
    steps: Vec<SteerStep>,
    start_at: usize,
    start_clock: f32,
    current: usize,
    /// Seconds into the current step.
    step_t: f32,
    /// Activity lines revealed so far in the current step.
    lines: usize,
    clock: f32,
    /// Seconds since this state started (drives the rehearsals and claims).
    demo_t: f32,
    step_started: Vec<f32>,
    log: Vec<Vec<Entry>>,
    queue: Vec<Queued>,
    next_id: usize,
    paused: bool,
    finished_at: Option<f32>,
    configured_follow_up: FollowUp,
    follow_up: FollowUp,
    steerable: bool,
    autoplay: bool,
    initial_queue: Vec<SharedString>,
    rehearsals: Vec<Rehearsal>,
    played: usize,
    typing: Option<Typing>,
    input: Entity<TextInput>,
    placeholder: SharedString,
    has_text: bool,
    /// The queued note being edited in the composer, and the draft it
    /// displaced.
    editing: Option<usize>,
    displaced: Option<SharedString>,
    drag: Option<DragState>,
    queue_scroll: ScrollHandle,
    checklist_open: bool,
    /// Tool groups the user folded or unfolded, by (step, segment).
    folds: HashMap<(usize, usize), bool>,
    last_tick: Instant,
    on_steer: Option<NoteHandler>,
    on_pause: Option<PauseHandler>,
    _subscriptions: Vec<Subscription>,
    _ticker: Task<()>,
}

impl SteerState {
    fn new(config: &AgentSteering, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).multiline().submit_on_enter());
        let subscriptions =
            vec![
                cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                    match event {
                        InputEvent::Submit(_) => this.submit(this.follow_up, window, cx),
                        InputEvent::Change(text) => {
                            this.take_over();
                            this.has_text = !text.trim().is_empty();
                            cx.notify();
                        }
                        InputEvent::Focus => {
                            this.take_over();
                            cx.notify();
                        }
                        InputEvent::Escape => this.finish_edit(false, cx),
                        _ => {}
                    }
                }),
            ];
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this.update(cx, |state, cx| state.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let count = config.steps.len();
        let start_at = config.start_at.min(count.saturating_sub(1));
        let mut state = Self {
            steps: config.steps.clone(),
            start_at,
            start_clock: config.elapsed,
            current: start_at,
            step_t: 0.0,
            lines: 0,
            clock: config.elapsed,
            demo_t: 0.0,
            step_started: vec![0.0; count],
            log: vec![Vec::new(); count],
            queue: Vec::new(),
            next_id: 0,
            paused: false,
            finished_at: None,
            configured_follow_up: config.follow_up,
            follow_up: config.follow_up,
            steerable: config.steerable,
            autoplay: config.autoplay,
            initial_queue: config.queued.clone(),
            rehearsals: config.rehearsals.clone(),
            played: 0,
            typing: None,
            input,
            placeholder: SharedString::default(),
            has_text: false,
            editing: None,
            displaced: None,
            drag: None,
            queue_scroll: ScrollHandle::new(),
            checklist_open: false,
            folds: HashMap::new(),
            last_tick: Instant::now(),
            on_steer: None,
            on_pause: None,
            _subscriptions: subscriptions,
            _ticker: ticker,
        };
        state.reset_run();
        state
    }

    fn working(&self) -> bool {
        !self.paused && self.finished_at.is_none() && !self.steps.is_empty()
    }

    /// Back to the configured starting point: earlier steps done with all
    /// their activity, the first line of the current one in, the seeded
    /// notes waiting.
    fn reset_run(&mut self) {
        let count = self.steps.len();
        self.clock = self.start_clock;
        self.demo_t = 0.0;
        self.played = 0;
        self.typing = None;
        self.queue.clear();
        self.editing = None;
        self.displaced = None;
        self.drag = None;
        self.folds.clear();
        self.paused = false;
        self.finished_at = None;
        self.follow_up = self.configured_follow_up;
        self.log = self
            .steps
            .iter()
            .enumerate()
            .map(|(ix, step)| {
                if ix < self.start_at {
                    step.activity
                        .iter()
                        .map(|text| Entry::Activity {
                            text: text.clone(),
                            at: None,
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            })
            .collect();
        for text in self.initial_queue.clone() {
            let id = self.next_id;
            self.next_id += 1;
            self.queue.push(Queued {
                id,
                text,
                follow_up: FollowUp::Queue,
                claim: None,
            });
        }
        if count == 0 {
            return;
        }
        self.begin_step(self.start_at);
        self.step_t = START_STEP_T;
        self.reveal_lines(None);
    }

    fn begin_step(&mut self, ix: usize) {
        self.current = ix;
        self.step_t = 0.0;
        self.lines = 0;
        if let Some(started) = self.step_started.get_mut(ix) {
            *started = self.clock;
        }
    }

    /// Reveals the activity lines that are due. A line opening a new tool
    /// group waits for its header (pink `TOOL_FIRST_ROW_DELAY_MS`).
    fn reveal_lines(&mut self, now: Option<Instant>) {
        let Some(step) = self.steps.get(self.current) else {
            return;
        };
        while self.lines < step.activity.len()
            && self.step_t >= 0.5 + self.lines as f32 * LINE_EVERY
        {
            let log = &mut self.log[self.current];
            let new_group = !matches!(log.last(), Some(Entry::Activity { .. }));
            let at = now.map(|now| now + motion::tool_row_arrival_delay(0, new_group));
            log.push(Entry::Activity {
                text: step.activity[self.lines].clone(),
                at,
            });
            self.lines += 1;
        }
    }

    /// A note reaches the agent: it lands in the current step as a user
    /// bubble. Send restarts the step from its last line (the agent drops
    /// what it was doing) and, with `resume`, wakes a paused run. Returns
    /// whether the run resumed.
    fn land(&mut self, id: usize, text: SharedString, mode: SteerMode, resume: bool) -> bool {
        if let Some(log) = self.log.get_mut(self.current) {
            log.push(Entry::Note {
                id,
                text,
                at: Some(Instant::now()),
            });
        }
        if mode != SteerMode::Send || self.finished_at.is_some() {
            return false;
        }
        let last_line = 0.5 + self.lines.saturating_sub(1) as f32 * LINE_EVERY;
        self.step_t = self.step_t.min(last_line);
        let resumed = resume && self.paused;
        if resumed {
            self.paused = false;
        }
        resumed
    }

    /// The step ends: held notes are delivered as the next one starts (pink
    /// drains the queue when the turn ends); notes being edited stay.
    fn end_step(&mut self) {
        let editing = self.editing;
        let (drained, kept): (Vec<Queued>, Vec<Queued>) = std::mem::take(&mut self.queue)
            .into_iter()
            .partition(|note| note.claim.is_none() && Some(note.id) != editing);
        self.queue = kept;
        if self.current + 1 < self.steps.len() {
            self.begin_step(self.current + 1);
        } else {
            self.finished_at = Some(self.demo_t);
        }
        for note in drained {
            self.land(note.id, note.text, SteerMode::Queue, false);
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .clamp(TICK.as_secs_f32(), 0.25);
        self.last_tick = now;
        self.demo_t += dt;

        // Rehearsed notes: type them into the composer, then press Enter.
        if self.autoplay {
            if self.typing.is_none()
                && let Some(rehearsal) = self.rehearsals.get(self.played).cloned()
                && self.demo_t >= rehearsal.at
            {
                self.played += 1;
                self.follow_up = rehearsal.follow_up;
                self.typing = Some(Typing {
                    text: rehearsal.text,
                    follow_up: rehearsal.follow_up,
                    shown: 0,
                    started: self.demo_t,
                });
            }
            if let Some(typing) = self.typing.as_mut() {
                // Three characters every `TYPE_EVERY`, on the demo clock.
                let total = typing.text.chars().count();
                let typed = self.demo_t - typing.started;
                let shown = ((typed / TYPE_EVERY) as usize * 3).min(total);
                let done_at = (total as f32 / 3.).ceil() * TYPE_EVERY;
                if shown > typing.shown {
                    typing.shown = shown;
                    let partial: String = typing.text.chars().take(shown).collect();
                    self.has_text = true;
                    self.input
                        .update(cx, |input, cx| input.set_text(partial, cx));
                } else if shown == total && typed >= done_at + 0.45 {
                    let (text, follow_up) = (typing.text.clone(), typing.follow_up);
                    self.typing = None;
                    self.clear_input(cx);
                    self.accept(text, follow_up);
                    self.follow_up = self.configured_follow_up;
                }
            }
        }

        // Deliveries in flight land once claimed long enough.
        let due: Vec<Queued> = self
            .queue
            .iter()
            .filter(|note| {
                note.claim
                    .is_some_and(|claim| self.demo_t - claim.since >= CLAIM_SECS)
            })
            .cloned()
            .collect();
        for note in due {
            self.queue.retain(|q| q.id != note.id);
            let mode = match note.claim.map(|claim| claim.action) {
                Some(Primary::SteerNow) => SteerMode::Steer,
                _ => SteerMode::Send,
            };
            self.land(note.id, note.text, mode, false);
        }

        if self.working() {
            self.clock += dt;
            self.step_t += dt;
            self.reveal_lines(Some(now));
            if self.step_t >= STEP_SECS {
                self.end_step();
                self.reveal_lines(Some(now));
            }
        }

        if self.autoplay
            && let Some(done) = self.finished_at
            && self.demo_t - done > 5.0
        {
            self.reset_run();
            self.clear_input(cx);
        }
        cx.notify();
    }

    /// Someone stepped in: the demo stops driving.
    fn take_over(&mut self) {
        if self.autoplay || self.typing.is_some() {
            self.autoplay = false;
            self.typing = None;
            self.follow_up = self.configured_follow_up;
        }
    }

    fn clear_input(&mut self, cx: &mut Context<Self>) {
        self.has_text = false;
        self.input.update(cx, |input, cx| input.clear(cx));
    }

    /// Takes a typed note and says how it reached the agent. While the agent
    /// works it joins the queue: held for the next step, or claimed at once
    /// as a steer. A paused or finished run gets it right away.
    fn accept(&mut self, text: SharedString, follow_up: FollowUp) -> (SteerMode, bool) {
        let id = self.next_id;
        self.next_id += 1;
        if !self.working() {
            let resumed = self.land(id, text, SteerMode::Send, true);
            return (SteerMode::Send, resumed);
        }
        let steer = follow_up == FollowUp::Steer && self.steerable;
        self.queue.push(Queued {
            id,
            text,
            follow_up,
            claim: steer.then_some(Claim {
                action: Primary::SteerNow,
                since: self.demo_t,
            }),
        });
        let mode = if steer {
            SteerMode::Steer
        } else {
            SteerMode::Queue
        };
        (mode, false)
    }

    /// Enter in the composer (or the disc): saves an edit, or sends the note
    /// with `follow_up`.
    fn submit(&mut self, follow_up: FollowUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            self.finish_edit(true, cx);
            return;
        }
        let text = self.input.read(cx).text();
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.take_over();
        let text = SharedString::from(text.to_string());
        self.clear_input(cx);
        let (mode, resumed) = self.accept(text.clone(), follow_up);
        cx.notify();
        if let Some(handler) = self.on_steer.clone() {
            handler(&SteerNote { text, mode }, window, cx);
        }
        if resumed && let Some(handler) = self.on_pause.clone() {
            handler(false, window, cx);
        }
    }

    /// ⌘/Ctrl+Enter: on an empty composer, the newest queued note's primary
    /// action (pink `activate_latest_queued_primary`); otherwise a submit.
    fn modifier_submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let empty = self.input.read(cx).text().trim().is_empty();
        if empty && self.editing.is_none() {
            if let Some(id) = self.queue.iter().map(|note| note.id).max() {
                self.activate(id, window, cx);
            }
        } else {
            self.submit(self.follow_up, window, cx);
        }
    }

    /// The row's primary action: Steer now when the note is a steer the agent
    /// can take mid-step, else Send now (pink `resolve_queue_primary_action`).
    fn primary_for(&self, note: &Queued) -> Primary {
        if let Some(claim) = note.claim {
            return claim.action;
        }
        if note.follow_up == FollowUp::Steer && self.working() && self.steerable {
            Primary::SteerNow
        } else {
            Primary::SendNow
        }
    }

    /// Why a row can't be delivered right now (pink `QueuePrimaryBlock`).
    fn blocked(&self, note: &Queued) -> Option<&'static str> {
        if self.editing == Some(note.id) {
            return Some("Finish editing before sending");
        }
        note.claim.map(|claim| {
            if claim.action == Primary::SteerNow && self.demo_t - claim.since >= STEER_ACCEPTED_AT {
                "Accepted and pending; the agent hasn't confirmed it yet"
            } else {
                "This message is already being delivered"
            }
        })
    }

    /// The row's text with its delivery status in front (pink `queue_row`).
    fn row_text(&self, note: &Queued) -> SharedString {
        let text = one_line(&note.text);
        match note.claim {
            Some(claim)
                if claim.action == Primary::SteerNow
                    && self.demo_t - claim.since >= STEER_ACCEPTED_AT =>
            {
                format!("Steer accepted, pending · {text}").into()
            }
            Some(claim) if claim.action == Primary::SteerNow => {
                format!("Requesting steer… · {text}").into()
            }
            Some(_) => format!("Sending… · {text}").into(),
            None if note.follow_up == FollowUp::Steer && !self.steerable => {
                format!("Steer unavailable · {text}").into()
            }
            None => text,
        }
    }

    /// Delivers a queued note now: claimed while the agent works, landed at
    /// once when it is paused (which resumes it) or finished.
    fn activate(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.queue.iter().position(|note| note.id == id) else {
            return;
        };
        if self.blocked(&self.queue[ix]).is_some() {
            return;
        }
        self.take_over();
        let action = self.primary_for(&self.queue[ix]);
        let mode = match action {
            Primary::SteerNow => SteerMode::Steer,
            Primary::SendNow => SteerMode::Send,
        };
        let text = self.queue[ix].text.clone();
        let mut resumed = false;
        if self.working() {
            self.queue[ix].claim = Some(Claim {
                action,
                since: self.demo_t,
            });
        } else {
            self.queue.remove(ix);
            resumed = self.land(id, text.clone(), SteerMode::Send, true);
        }
        cx.notify();
        if let Some(handler) = self.on_steer.clone() {
            handler(&SteerNote { text, mode }, window, cx);
        }
        if resumed && let Some(handler) = self.on_pause.clone() {
            handler(false, window, cx);
        }
    }

    fn remove(&mut self, id: usize, cx: &mut Context<Self>) {
        self.take_over();
        self.queue
            .retain(|note| note.id != id || note.claim.is_some());
        cx.notify();
    }

    /// Moves a queued note into the composer; its row keeps its place (pink
    /// `begin_queue_edit`).
    fn begin_edit(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            self.finish_edit(false, cx);
        }
        let Some(text) = self
            .queue
            .iter()
            .find(|note| note.id == id && note.claim.is_none())
            .map(|note| note.text.clone())
        else {
            return;
        };
        self.take_over();
        self.displaced = Some(self.input.read(cx).text());
        self.input.update(cx, |input, cx| input.set_text(text, cx));
        self.has_text = true;
        self.editing = Some(id);
        TextInput::focus(&self.input, window, cx);
        cx.notify();
    }

    /// Ends an edit: saving writes the composer's text back to the row, and
    /// the displaced draft returns either way.
    fn finish_edit(&mut self, save: bool, cx: &mut Context<Self>) {
        let Some(id) = self.editing.take() else {
            return;
        };
        if save {
            let text = self.input.read(cx).text().trim().to_string();
            if !text.is_empty()
                && let Some(note) = self.queue.iter_mut().find(|note| note.id == id)
            {
                note.text = text.into();
            }
        }
        let draft = self.displaced.take().unwrap_or_default();
        self.has_text = !draft.trim().is_empty();
        self.input.update(cx, |input, cx| input.set_text(draft, cx));
        cx.notify();
    }

    fn set_paused(&mut self, paused: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.finished_at.is_some() || self.paused == paused {
            return;
        }
        self.take_over();
        self.paused = paused;
        cx.notify();
        if let Some(handler) = self.on_pause.clone() {
            handler(paused, window, cx);
        }
    }

    /// Tracks the drop slot while a row is dragged (pink
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

    fn move_note(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        self.take_over();
        if from != to && from < self.queue.len() {
            let note = self.queue.remove(from);
            self.queue.insert(to.min(self.queue.len()), note);
        }
        cx.notify();
    }

    /// The composer's placeholder for the run's state.
    fn placeholder_for(&self, agent: &str) -> SharedString {
        if self.editing.is_some() {
            "Edit the queued note".into()
        } else if self.finished_at.is_some() {
            format!("Follow up with {agent}").into()
        } else if self.paused {
            format!("Send {agent} a note to resume").into()
        } else if self.follow_up == FollowUp::Steer && self.steerable {
            format!("Steer {agent} mid-step").into()
        } else {
            format!("Queue a note for {agent}'s next step").into()
        }
    }
}

/// One line of a queued note: the newlines that make it a paragraph in the
/// composer would make it several rows here (pink `one_line`).
fn one_line(text: &str) -> SharedString {
    text.split_whitespace().collect::<Vec<_>>().join(" ").into()
}

/// The glyph for an activity line, from its verb (pink picks it from the
/// tool's kind, `tool_icon_path`).
fn activity_glyph(line: &str) -> &'static str {
    let verb = line
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_lowercase();
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| verb.starts_with(p));
    if starts(&["read", "load", "open", "scan", "inspect"]) {
        ICON_FILE_TEXT
    } else if starts(&["run", "stream", "build", "index", "reindex", "exec", "test"]) {
        ICON_TERMINAL
    } else if starts(&[
        "search", "find", "compar", "check", "collect", "look", "grep",
    ]) {
        ICON_FILE_SEARCH
    } else if starts(&[
        "edit", "writ", "draft", "updat", "map", "patch", "point", "schedul",
    ]) {
        ICON_FILE_PEN
    } else {
        ICON_TOOLBOX
    }
}

/// A group's glyph: the rows' shared kind, else the mixed mark (pink
/// `tool_group_glyph`).
fn group_glyph<'a>(mut lines: impl Iterator<Item = &'a SharedString>) -> &'static str {
    let Some(first) = lines.next().map(|line| activity_glyph(line)) else {
        return ICON_TOOLBOX;
    };
    if lines.all(|line| activity_glyph(line) == first) {
        first
    } else {
        ICON_LAYERS
    }
}

/// The working line's verb and object from an activity line.
fn live_status(line: &str) -> (SharedString, Option<SharedString>) {
    let line = line.trim();
    match line.split_once(' ') {
        Some((verb, object)) => (
            verb.to_string().into(),
            Some(ellipsize_chars(object.trim(), LIVE_STATUS_OBJECT_CHARS).into()),
        ),
        None => (line.to_string().into(), None),
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// What one render needs from its caller.
struct Ctx<'a> {
    key: &'a str,
    s: &'a SteerState,
    state: &'a Entity<SteerState>,
    t: &'a Pink,
    now: Instant,
    reduced: bool,
}

impl RenderOnce for AgentSteering {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "steer".into()),
            cx,
            |window, cx| SteerState::new(&self, window, cx),
        );
        // A drop outside the tray ends GPUI's drag without our `on_drop`:
        // never leave a row parked in a stale slot.
        let drag_live = cx.has_active_drag();
        let (on_steer, on_pause) = (self.on_steer.clone(), self.on_pause.clone());
        let agent = self.agent.clone();
        state.update(cx, |s, cx| {
            s.on_steer = on_steer;
            s.on_pause = on_pause;
            if !drag_live {
                s.drag = None;
            }
            let placeholder = s.placeholder_for(&agent);
            let changed = placeholder != s.placeholder;
            s.placeholder = placeholder.clone();
            s.input.update(cx, |input, cx| {
                input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection));
                if changed {
                    input.set_placeholder(placeholder, cx);
                }
            });
        });
        let reduced = cx.reduce_motion();
        let focused = state.read(cx).input.read(cx).is_focused(window);
        let s = state.read(cx);
        let c = Ctx {
            key: &key,
            s,
            state: &state,
            t: &t,
            now: Instant::now(),
            reduced,
        };

        let (tail, in_flight) = transcript_tail(&c, &self.task);
        let column = div()
            .flex_none()
            .px(px(COMPOSER_GUTTER))
            .pb(px(COMPOSER_GUTTER))
            .flex()
            .flex_col()
            .gap(px(STACK_GAP))
            .when(!s.steps.is_empty(), |el| el.child(checklist(&c)))
            .child(motion::fade_quick(
                SharedString::from(format!("{key}-goal-in")),
                div()
                    .mx(px(TRAY_INSET))
                    .mb(px(-(STACK_GAP + TRAY_OVERLAP)))
                    .child(goal_bar(&c, &self.agent, &self.task)),
            ))
            .when(!s.queue.is_empty(), |el| {
                el.child(motion::fade_quick(
                    SharedString::from(format!("{key}-queue-in")),
                    div()
                        .mx(px(TRAY_INSET))
                        .mb(px(-(STACK_GAP + TRAY_OVERLAP)))
                        .child(queue_tray(&c)),
                ))
            })
            .child(composer_pill(&c, focused));

        if in_flight {
            window.request_animation_frame();
        }
        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .w(px(PANEL_W))
            .h(px(PANEL_H))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .border_1()
            .border_color(t.border)
            .bg(t.bg)
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(tail)
            .child(column)
    }
}

// -- Transcript tail ------------------------------------------------------------

/// A run of the transcript tail: a user bubble or one tool group.
enum Block<'a> {
    Bubble {
        id: SharedString,
        text: SharedString,
        at: Option<Instant>,
    },
    Group {
        step: usize,
        seg: usize,
        rows: Vec<(usize, &'a SharedString, Option<Instant>)>,
    },
}

/// The bottom of the transcript (pink `transcript.rs`): bottom-anchored, the
/// older rows passing under a 24px fade at the top. Returns whether a reveal
/// is still in flight.
fn transcript_tail(c: &Ctx, task: &SharedString) -> (Div, bool) {
    let (s, t, key) = (c.s, c.t, c.key);
    let mut blocks = vec![Block::Bubble {
        id: format!("{key}-task").into(),
        text: task.clone(),
        at: None,
    }];
    let last_step = s.current.min(s.steps.len().saturating_sub(1));
    for ix in 0..=last_step {
        let Some(log) = s.log.get(ix) else {
            continue;
        };
        let mut seg = 0;
        let mut run = Vec::new();
        for (n, entry) in log.iter().enumerate() {
            match entry {
                Entry::Activity { text, at } => run.push((n, text, *at)),
                Entry::Note { id, text, at } => {
                    if !run.is_empty() {
                        blocks.push(Block::Group {
                            step: ix,
                            seg,
                            rows: std::mem::take(&mut run),
                        });
                        seg += 1;
                    }
                    blocks.push(Block::Bubble {
                        id: format!("{key}-note-{id}").into(),
                        text: text.clone(),
                        at: *at,
                    });
                }
            }
        }
        if !run.is_empty() {
            blocks.push(Block::Group {
                step: ix,
                seg,
                rows: run,
            });
        }
    }

    let working = s.working();
    let block_count = blocks.len();
    let mut in_flight = false;
    let mut column = div().flex_none().w_full().flex().flex_col();
    for (n, block) in blocks.into_iter().enumerate() {
        let first = n == 0;
        match block {
            Block::Bubble { id, text, at } => {
                let p = motion::reveal_progress(TOOL_ROW_REVEAL, at, c.now, c.reduced);
                in_flight |= p < 1.;
                column = column.child(
                    div()
                        .when(!first, |el| el.pt(px(TURN_GAP)))
                        .child(user_bubble(id, text, p, t)),
                );
            }
            Block::Group { step, seg, rows } => {
                let live = working && step == s.current && n + 1 == block_count;
                let (group, moving) = tool_group(c, step, seg, &rows, live);
                in_flight |= moving;
                column = column.child(div().when(!first, |el| el.pt(px(GROUP_GAP))).child(group));
            }
        }
    }
    if working {
        column = column.child(working_line(c));
    }

    let fade = linear_gradient(
        180.,
        linear_color_stop(t.bg, 0.),
        linear_color_stop(t.bg.opacity(0.), 1.),
    );
    let tail = div()
        .relative()
        .flex_1()
        .min_h_0()
        .overflow_hidden()
        .flex()
        .flex_col()
        .justify_end()
        .px(px(TRANSCRIPT_GUTTER))
        .pb(px(TURN_GAP))
        .child(column)
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(layout::FADE_BAND))
                .bg(fade),
        );
    (tail, in_flight)
}

/// A user bubble (pink `transcript.rs` user row): right-aligned, at most 80%
/// of the column, `user_bubble_bg` with the light-mode inset edge, radius 16,
/// `px 16 py 10`, 14/22 `text`. An arriving bubble grows in and fades up
/// over `TOOL_ROW_REVEAL`.
fn user_bubble(id: SharedString, text: SharedString, p: f32, t: &Pink) -> AnyElement {
    let max_w = (PANEL_W - 2. * TRANSCRIPT_GUTTER - 2.) * USER_BUBBLE_SHARE;
    // An upper bound of its height while it grows (about 7.5px a glyph).
    let lines = (text.chars().count() as f32 * 7.5 / (max_w - 32.))
        .ceil()
        .max(1.);
    let bound = 20. + USER_LINE_HEIGHT * (lines + 1.);
    let bubble = div()
        .id(id)
        .min_w_0()
        .max_w(px(max_w))
        .bg(t.user_bubble_bg())
        .when_some(t.user_bubble_edge(), |el, edge| {
            el.shadow(vec![BoxShadow {
                color: edge,
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: px(1.),
                inset: true,
            }])
        })
        .rounded(px(radius::BUBBLE))
        .px(px(16.))
        .py(px(10.))
        .text_size(px(14.))
        .line_height(px(USER_LINE_HEIGHT))
        .text_color(t.text)
        .child(text);
    div()
        .w_full()
        .flex()
        .justify_end()
        .when(p < 1., |el| {
            el.max_h(px(bound * p))
                .overflow_hidden()
                .relative()
                .top(px(4. * (1. - p)))
                .opacity(p)
        })
        .child(bubble)
        .into_any_element()
}

/// One step's tool group (pink `render_tool_group`): the 24px header plate –
/// kind glyph, the step's title shimmering while live, the disclosure
/// chevron – over its rows on the activity rail. Open while it is the live
/// tail or a row is still arriving, unless the user chose; the fold rides
/// `DISCLOSURE`. Returns whether anything is still moving.
fn tool_group(
    c: &Ctx,
    step: usize,
    seg: usize,
    rows: &[(usize, &SharedString, Option<Instant>)],
    live: bool,
) -> (AnyElement, bool) {
    let (s, t, key) = (c.s, c.t, c.key);
    let title = s.steps[step].title.clone();
    let glyph = group_glyph(rows.iter().map(|(_, text, _)| *text));
    let first_at = rows.first().and_then(|(_, _, at)| *at);
    let header_start = first_at.map(|at| {
        at.checked_sub(motion::tool_row_arrival_delay(0, true))
            .unwrap_or(at)
    });
    let header_reveal = motion::reveal_progress(TOOL_ROW_REVEAL, header_start, c.now, c.reduced);
    let reveals: Vec<f32> = rows
        .iter()
        .map(|(_, _, at)| motion::reveal_progress(TOOL_ROW_REVEAL, *at, c.now, c.reduced))
        .collect();
    let connectors: Vec<f32> = rows
        .iter()
        .map(|(_, _, at)| motion::reveal_progress(TOOL_CONNECTOR_REVEAL, *at, c.now, c.reduced))
        .collect();
    let arriving = connectors.iter().any(|p| *p < 1.);
    let auto_open = live || arriving;
    let open = s.folds.get(&(step, seg)).copied().unwrap_or(auto_open);
    let fold = motion::follow(
        &format!("{key}-fold-{step}-{seg}"),
        if open { 1. } else { 0. },
        DISCLOSURE,
    );
    let body_alpha = if open {
        fold
    } else {
        ((fold - 0.4) / 0.6).clamp(0., 1.)
    };
    let moving = arriving || header_reveal < 1.;

    let ring = t.focus_ring();
    let plate_key = format!("tl-{key}-grp-{step}-{seg}");
    let label = match first_at.filter(|_| live) {
        Some(start) => shimmer_text(
            title.clone(),
            TOOL_LABEL_SIZE,
            TOOL_LABEL_LINE_HEIGHT,
            t.content_muted,
            t.text,
        )
        .started_at(start)
        .flex_none()
        .into_any_element(),
        None => div()
            .min_w_0()
            .truncate()
            .child(title.clone())
            .into_any_element(),
    };
    let plate = div()
        .id(SharedString::from(format!("{key}-grp-{step}-{seg}")))
        .role(Role::Button)
        .aria_label(SharedString::from(format!("Tool activity: {title}")))
        .aria_expanded(open)
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .flex_none()
        .max_w_full()
        .min_w_0()
        .h(px(TOOL_GROUP_HEADER_HEIGHT - 2.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .pl(px(ACTIVITY_TRUNK_X - ACTIVITY_ICON_SIZE / 2.))
        .pr(px(8.))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_size(px(TOOL_LABEL_SIZE))
        .line_height(px(TOOL_LABEL_LINE_HEIGHT))
        .hover_fill_fg(
            plate_key.clone(),
            t.fill(FillTier::Quaternary),
            t.fill(FillTier::Tertiary),
            t.content_muted,
            t.text,
        )
        .on_click({
            let state = c.state.clone();
            move |_: &ClickEvent, _, cx| {
                cx.stop_propagation();
                state.update(cx, |s, cx| {
                    s.folds.insert((step, seg), !open);
                    cx.notify();
                })
            }
        })
        .child(
            svg()
                .path(glyph)
                .flex_none()
                .size(px(ACTIVITY_ICON_SIZE))
                .text_color(motion::hover_blend(&plate_key, t.content_muted, t.text)),
        )
        .child(
            div()
                .min_w_0()
                .h(px(TOOL_LABEL_LINE_HEIGHT))
                .flex()
                .items_center()
                .child(label),
        )
        .child(
            svg()
                .path(ICON_CHEVRON_RIGHT)
                .flex_none()
                .size(px(DISCLOSURE_CHEVRON_SIZE))
                .with_transformation(Transformation::rotate(radians(FRAC_PI_2 * fold)))
                .text_color(t.content_faint),
        );
    let header = div()
        .h(px(TOOL_GROUP_HEADER_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .child(plate);
    let header = if header_reveal < 1. {
        div()
            .h(px(TOOL_GROUP_HEADER_HEIGHT * header_reveal))
            .overflow_hidden()
            .opacity(header_reveal)
            .child(header)
    } else {
        header
    };

    let count = rows.len();
    let natural = CHIPS_TOP_PAD + reveals.iter().sum::<f32>() * TOOL_TREE_ROW_HEIGHT;
    let body = (fold > 0.).then(|| {
        let rows_el = rows.iter().enumerate().map(|(ix, (n, text, _))| {
            let connector = connectors[ix];
            let content = motion::tool_connector_parts(connector, ix > 0).1;
            let continuation = motion::tool_connector_continuation(connectors.get(ix + 1).copied());
            let running = (live && ix + 1 == count).then(|| {
                mini_glyph_spinner(format!("{key}-run-{step}-{n}"), 2., t.glyph).into_any_element()
            });
            let rail = activity_rail(
                activity_glyph(text),
                ix > 0,
                ix + 1 < count,
                connector,
                continuation,
                running,
                t,
            );
            let card = div()
                .ml(px(ACTIVITY_TEXT_GAP))
                .my(px((TOOL_TREE_ROW_HEIGHT - CHIP_CARD_HEIGHT) / 2.))
                .h(px(CHIP_CARD_HEIGHT))
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .overflow_hidden()
                .text_size(px(TOOL_LABEL_SIZE))
                .line_height(px(TOOL_LABEL_LINE_HEIGHT))
                .text_color(t.content_muted)
                .when(content < 1., |card| {
                    card.relative()
                        .top(px(4. * (1. - content)))
                        .opacity(content)
                })
                .child(div().min_w_0().truncate().child((*text).clone()));
            let row = div()
                .h(px(TOOL_TREE_ROW_HEIGHT))
                .w_full()
                .flex()
                .flex_row()
                .child(rail)
                .child(card);
            let reveal = reveals[ix];
            if reveal < 1. {
                div()
                    .w_full()
                    .h(px(TOOL_TREE_ROW_HEIGHT * reveal))
                    .flex_none()
                    .overflow_hidden()
                    .child(row)
                    .into_any_element()
            } else {
                row.into_any_element()
            }
        });
        div().h(px(natural * fold)).overflow_hidden().child(
            div()
                .relative()
                .top(px(motion::TOOL_BODY_RISE * (1. - body_alpha)))
                .opacity(body_alpha)
                .pt(px(CHIPS_TOP_PAD))
                .flex()
                .flex_col()
                .children(rows_el),
        )
    });

    (
        div()
            .flex()
            .flex_col()
            .child(header)
            .children(body)
            .into_any_element(),
        moving,
    )
}

/// pink `activity_branch_points`: the elbow from the trunk to the branch
/// tip, cut to `progress` of its length.
fn branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<Point<f32>> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.;
            point(
                ACTIVITY_BEND_RADIUS * t * t,
                ACTIVITY_BEND_RADIUS * (2. * t - t * t),
            )
        })
        .collect();
    path.push(point(
        ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X,
        ACTIVITY_BEND_RADIUS,
    ));
    if progress >= 1. {
        return path;
    }
    let lengths: Vec<f32> = path
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .collect();
    let mut remaining = lengths.iter().sum::<f32>() * progress.clamp(0., 1.);
    let mut visible = vec![path[0]];
    for (pair, length) in path.windows(2).zip(lengths) {
        if remaining <= 0. {
            break;
        }
        let t = (remaining / length).min(1.);
        visible.push(point(
            motion::lerp(pair[0].x, pair[1].x, t),
            motion::lerp(pair[0].y, pair[1].y, t),
        ));
        remaining -= length;
    }
    visible
}

/// pink `activity_ribbon`: a 1px ribbon contour; nonzero fill unions the
/// trunk and the elbow so the fork paints one alpha.
fn ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
    if points.len() < 2 {
        return;
    }
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for (ix, p) in points.iter().enumerate() {
        let a = points[ix.saturating_sub(1)];
        let b = points[(ix + 1).min(points.len() - 1)];
        let dx = f32::from(b.x - a.x);
        let dy = f32::from(b.y - a.y);
        let length = dx.hypot(dy).max(0.0001);
        let normal = point(px(-dy / length * 0.5), px(dx / length * 0.5));
        left.push(*p + normal);
        right.push(*p - normal);
    }
    path.move_to(left[0]);
    for p in left.iter().skip(1).chain(right.iter().rev()) {
        path.line_to(*p);
    }
    path.close();
}

/// pink `activity_rail`: the 48px gutter of one row – the trunk at x 12.5
/// drawing down from the row above, a 6px elbow to x 28 and the row's glyph
/// (14, `content_muted`) at x 32, or the running row's mini spinner. The
/// ribbons are `hairline(0.12)`.
fn activity_rail(
    glyph: &'static str,
    has_predecessor: bool,
    continues: bool,
    reveal: f32,
    continuation: f32,
    running: Option<AnyElement>,
    t: &Pink,
) -> Div {
    let color = t.hairline(0.12);
    let row_height = TOOL_TREE_ROW_HEIGHT;
    let (incoming, branch) = motion::tool_connector_parts(reveal, has_predecessor);
    let icon_top = row_height / 2. - ACTIVITY_ICON_SIZE / 2.;
    div()
        .relative()
        .w(px(ACTIVITY_GUTTER_WIDTH))
        .flex_none()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let x = bounds.origin.x + px(ACTIVITY_TRUNK_X);
                    let bend_y = bounds.origin.y + px(row_height / 2. - ACTIVITY_BEND_RADIUS);
                    let mut tree = PathBuilder::fill().with_style(PathStyle::Fill(
                        FillOptions::default().with_fill_rule(FillRule::NonZero),
                    ));
                    if incoming > 0. {
                        let mut bottom = point(
                            x,
                            bounds.origin.y
                                + px((row_height / 2. - ACTIVITY_BEND_RADIUS) * incoming),
                        );
                        if incoming >= 1. && continues && continuation > 0. {
                            let below = (f32::from(bounds.size.height)
                                - (row_height / 2. - ACTIVITY_BEND_RADIUS))
                                .max(0.);
                            bottom = point(x, bend_y + px(below * continuation));
                        }
                        ribbon(&mut tree, &[point(x, bounds.origin.y), bottom]);
                    }
                    if branch > 0. {
                        let points: Vec<Point<Pixels>> = branch_points(branch)
                            .into_iter()
                            .map(|p| point(x + px(p.x), bend_y + px(p.y)))
                            .collect();
                        ribbon(&mut tree, &points);
                    }
                    if let Ok(path) = tree.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(match running {
            Some(spinner) => div()
                .absolute()
                .left(px(ACTIVITY_ICON_LEFT))
                .top(px(icon_top))
                .size(px(ACTIVITY_ICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .opacity(branch)
                .child(spinner)
                .into_any_element(),
            None => svg()
                .path(glyph)
                .absolute()
                .left(px(ACTIVITY_ICON_LEFT))
                .top(px(icon_top))
                .size(px(ACTIVITY_ICON_SIZE))
                .opacity(branch)
                .text_color(t.content_muted)
                .into_any_element(),
        })
}

/// The working line under the last row (pink `render_working_trailer`): the
/// breathing orb, the live status (verb in `content_muted` shimmering toward
/// `text`, object in `text`), the step's timer and a "Stop" that shows only
/// under the pointer.
fn working_line(c: &Ctx) -> Stateful<Div> {
    let (s, t, key) = (c.s, c.t, c.key);
    let newest = s.log.get(s.current).and_then(|log| {
        log.iter().rev().find_map(|entry| match entry {
            Entry::Activity { text, .. } => Some(text.clone()),
            Entry::Note { .. } => None,
        })
    });
    let (verb, object) = newest
        .as_deref()
        .map(live_status)
        .unwrap_or_else(|| ("Thinking".into(), None));
    let status = working_status(verb, t.content_muted, t.text);
    let status = match object {
        Some(object) => status.object(object),
        None => status,
    };
    let elapsed = (s.clock - s.step_started.get(s.current).copied().unwrap_or(0.)) as i64;
    let group = SharedString::from(format!("{key}-working"));
    let stop_key = format!("tl-{key}-working-stop");
    div()
        .id(group.clone())
        .group(group.clone())
        .flex()
        .flex_row()
        .items_start()
        .gap(px(STACK_GAP))
        .pt(px(TURN_GAP))
        .min_w_0()
        .child(orb_slot(t.accent))
        .child(div().min_w_0().flex().child(status))
        .child(working_timer(elapsed, t.content_faint))
        .child(
            div()
                .id(SharedString::from(format!("{key}-working-stop")))
                .role(Role::Button)
                .aria_label("Stop the current step")
                .tab_index(0)
                .flex_none()
                .ml(px(4.))
                .px(px(6.))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .text_size(px(WORKING_TIMER_SIZE))
                .line_height(px(WORKING_STATUS_LINE_HEIGHT))
                .text_color(t.content_muted)
                .opacity(0.)
                .group_hover(group, |style| style.opacity(1.))
                .focus_visible(|style| style.opacity(1.))
                .hover_wash(stop_key, t.fill(FillTier::Tertiary))
                .on_key_down(pink::swallow_activation_key)
                .child("Stop")
                .on_click({
                    let state = c.state.clone();
                    move |_: &ClickEvent, window, cx| {
                        cx.stop_propagation();
                        state.update(cx, |s, cx| s.set_paused(true, window, cx))
                    }
                }),
        )
}

// -- Checklist line -------------------------------------------------------------

/// The run's steps as pink's provider checklist (`composer/checklist.rs`):
/// one quiet "2/6 · In progress: …" line in 11.5 `text_muted` that unfolds
/// into a "Status · step" line per step.
fn checklist(c: &Ctx) -> Div {
    let (s, t, key) = (c.s, c.t, c.key);
    let total = s.steps.len();
    let finished = s.finished_at.is_some();
    let done = if finished { total } else { s.current };
    let title = &s.steps[s.current.min(total - 1)].title;
    let detail = if finished {
        "Checklist complete".to_string()
    } else if s.paused {
        format!("Last active: {title}")
    } else {
        format!("In progress: {title}")
    };
    let label = format!("{done}/{total} · {detail}");
    let open = s.checklist_open;
    let ring = t.focus_ring();
    let hover = t.element_hover;
    let row = div()
        .id(SharedString::from(format!("{key}-checklist")))
        .role(Role::Button)
        .tab_index(0)
        .aria_label(SharedString::from(format!(
            "{} checklist: {label}",
            if open { "Collapse" } else { "Expand" }
        )))
        .aria_expanded(open)
        .flex_1()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(6.))
        .px(px(8.))
        .h(px(CHECKLIST_ROW))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_key_down(pink::swallow_activation_key)
        .child(
            svg()
                .path(if open {
                    ICON_CHEVRON_DOWN
                } else {
                    ICON_CHEVRON_RIGHT
                })
                .flex_none()
                .size(px(12.))
                .text_color(t.text_muted),
        )
        .child(div().min_w_0().truncate().child(label))
        .on_click({
            let state = c.state.clone();
            move |_: &ClickEvent, _, cx| {
                state.update(cx, |s, cx| {
                    s.checklist_open = !s.checklist_open;
                    cx.notify();
                })
            }
        });
    let list = open.then(|| {
        div()
            .id(SharedString::from(format!("{key}-checklist-rows")))
            .role(Role::List)
            .max_h(px(CHECKLIST_LIST_MAX))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .px(px(12.))
            .gap(px(4.))
            .children(s.steps.iter().enumerate().map(|(ix, step)| {
                let status = if finished || ix < s.current {
                    "Completed"
                } else if ix == s.current && s.paused {
                    "Last active"
                } else if ix == s.current {
                    "In progress"
                } else {
                    "Pending"
                };
                div()
                    .id(SharedString::from(format!("{key}-checklist-step-{ix}")))
                    .role(Role::ListItem)
                    .truncate()
                    .child(format!("{status} · {}", step.title))
            }))
    });
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .text_size(px(CHECKLIST_TEXT))
        .line_height(px(16.))
        .text_color(t.text_muted)
        .child(div().flex().items_center().gap(px(4.)).min_w_0().child(row))
        .children(list)
}

// -- Goal bar ---------------------------------------------------------------------

/// The run as pink's goal bar (`composer/goals.rs` `render_goal_card`): a
/// tray tucked behind the next layer, its 38px header holding the target in
/// the status tone, the status, the task, the run clock and Pause / Resume.
fn goal_bar(c: &Ctx, agent: &SharedString, task: &SharedString) -> Stateful<Div> {
    let (s, t, key) = (c.s, c.t, c.key);
    let finished = s.finished_at.is_some();
    let live = s.working();
    let (title, tone) = if finished {
        (format!("{agent} finished"), t.success)
    } else if s.paused {
        (format!("{agent} paused"), t.text_muted)
    } else {
        (format!("{agent} is working"), t.accent)
    };
    let timer = format_elapsed(s.clock as i64);
    let summary = div()
        .id(SharedString::from(format!("{key}-goal-summary")))
        .role(Role::Status)
        .aria_label(SharedString::from(format!(
            "{title}, {task}, {timer} {}",
            if live { "so far" } else { "worked" }
        )))
        .flex_1()
        .min_w_0()
        .h_full()
        .pl(px(14.))
        .pr(px(6.))
        .flex()
        .items_center()
        .gap(px(7.))
        .child(
            svg()
                .path(ICON_TARGET)
                .flex_none()
                .size(px(GOAL_GLYPH))
                .text_color(tone),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(title),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(13.))
                .text_color(t.readable_faint(0.8))
                .child("·"),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .text_color(t.readable_muted(0.8))
                .child(task.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(12.))
                .font_features(tabular_figures())
                .text_color(if live {
                    t.text.opacity(0.85)
                } else {
                    t.readable_muted(0.75)
                })
                .child(timer),
        );
    let toggle = (!finished).then(|| {
        let (label, glyph, pause) = if s.paused {
            ("Resume", ICON_PLAY, false)
        } else {
            ("Pause", ICON_PAUSE, true)
        };
        goal_control(format!("{key}-goal-toggle"), label, glyph, t).on_click({
            let state = c.state.clone();
            move |_: &ClickEvent, window, cx| {
                state.update(cx, |s, cx| s.set_paused(pause, window, cx))
            }
        })
    });
    div()
        .id(SharedString::from(format!("{key}-goal")))
        .role(Role::Group)
        .aria_label("Goal")
        .occlude()
        .rounded_t(px(radius::STACK_TRAY))
        .bg(t.surface_bg())
        .border_1()
        .border_color(t.border)
        .shadow(t.menu_shadows())
        .pb(px(TRAY_OVERLAP))
        .flex()
        .flex_col()
        .text_color(t.text)
        .child(
            div()
                .h(px(GOAL_HEADER_HEIGHT))
                .flex_none()
                .flex()
                .items_center()
                .child(summary)
                .child(
                    div()
                        .flex_none()
                        .h_full()
                        .pr(px(8.))
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .children(toggle),
                ),
        )
}

/// A goal bar control (pink `goal_control`, icon only): 24px, radius 6, the
/// glyph in `readable_muted(0.85)` brightening to `text` over an `ink(0.08)`
/// wash; focus washes it in the accent.
fn goal_control(id: String, label: &'static str, glyph: &'static str, t: &Pink) -> Stateful<Div> {
    let group = SharedString::from(format!("{id}-group"));
    let (rest, hover) = (t.readable_muted(0.85), t.text);
    let wash = t.ink(0.08);
    let accent = t.accent;
    div()
        .id(SharedString::from(id.clone()))
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .group(group.clone())
        .size(px(GOAL_CONTROL))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::CONTROL))
        .text_color(rest)
        .cursor_pointer()
        .hover_fill_fg(id, wash.opacity(0.), wash, rest, hover)
        .focus_visible(move |style| style.bg(accent.opacity(0.18)).text_color(accent))
        .on_key_down(pink::swallow_activation_key)
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            svg()
                .path(glyph)
                .size(px(GOAL_GLYPH))
                .text_color(rest)
                .group_hover(group, move |style| style.text_color(hover)),
        )
}

// -- Queue tray -------------------------------------------------------------------

/// pink `drop_index`: the row slot under a pointer `rel_y` into the list.
fn drop_index(rel_y: f32, slot: f32, count: usize) -> usize {
    if count == 0 || slot <= 0. {
        return 0;
    }
    ((rel_y / slot).floor().max(0.) as usize).min(count - 1)
}

/// pink `slide_offset`: rows between the dragged row and its slot shift one
/// slot toward the gap it leaves.
fn slide_offset(ix: usize, from: usize, over: usize) -> f32 {
    if from < over && ix > from && ix <= over {
        -1.
    } else if over < from && ix >= over && ix < from {
        1.
    } else {
        0.
    }
}

/// pink `queue_drag_offsets`: the dragged row travels to its slot while
/// every row in its path slides into the space it leaves.
fn drag_offsets(ix: usize, from: usize, prev_over: usize, over: usize) -> (f32, f32) {
    if ix == from {
        (
            (prev_over as f32 - from as f32) * ROW_HEIGHT,
            (over as f32 - from as f32) * ROW_HEIGHT,
        )
    } else {
        (
            slide_offset(ix, from, prev_over) * ROW_HEIGHT,
            slide_offset(ix, from, over) * ROW_HEIGHT,
        )
    }
}

/// The queue tray (pink `queue_panel_surface` + `queue_rows`): one surface
/// emerging from behind the composer – radius 18 on top, the popover plate,
/// a hairline edge and the menu shadow – whose rows sit flush, with no gap.
/// The whole plate is the drop target for a dragged row.
fn queue_tray(c: &Ctx) -> Stateful<Div> {
    let (s, t, key) = (c.s, c.t, c.key);
    let count = s.queue.len();
    let owner: SharedString = key.to_string().into();
    let scroll = s.queue_scroll.clone();
    let rows = s
        .queue
        .iter()
        .enumerate()
        .map(|(ix, note)| queue_row(c, ix, note));
    div()
        .id(SharedString::from(format!("{key}-queue")))
        .role(Role::List)
        .aria_label("Queued notes")
        .occlude()
        .rounded_t(px(radius::STACK_TRAY))
        .bg(t.surface_bg())
        .border_1()
        .border_color(t.border)
        .shadow(t.menu_shadows())
        .pb(px(TRAY_OVERLAP))
        .flex()
        .flex_col()
        .on_drag_move::<NoteDrag>({
            let state = c.state.clone();
            let owner = owner.clone();
            move |event: &DragMoveEvent<NoteDrag>, _, cx| {
                let (from, mine) = {
                    let drag = event.drag(cx);
                    (drag.from, drag.owner == owner)
                };
                if !mine {
                    return;
                }
                let rel_y = f32::from(event.event.position.y - event.bounds.top())
                    - f32::from(scroll.offset().y);
                let over = drop_index(rel_y, ROW_HEIGHT, count);
                state.update(cx, |s, cx| s.drag_over(from, over, cx));
            }
        })
        .on_drop::<NoteDrag>({
            let state = c.state.clone();
            let owner = owner.clone();
            move |payload: &NoteDrag, _, cx| {
                state.update(cx, |s, cx| {
                    let to = s.drag.take().map_or(payload.from, |drag| drag.over);
                    if payload.owner == owner {
                        s.move_note(payload.from, to, cx);
                    } else {
                        cx.notify();
                    }
                })
            }
        })
        .on_mouse_up_out(MouseButton::Left, {
            let state = c.state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if s.drag.take().is_some() {
                        cx.notify();
                    }
                })
            }
        })
        .child(
            div()
                .id(SharedString::from(format!("{key}-queue-rows")))
                .max_h(px(ROW_HEIGHT * QUEUE_VISIBLE_ROWS))
                .overflow_y_scroll()
                .track_scroll(&s.queue_scroll)
                .flex()
                .flex_col()
                .children(rows),
        )
}

/// One queued note (pink `queue_row`): the drag marker, the text with its
/// delivery status in front, then Remove, Edit and the one primary action.
/// While it is edited in the composer the row washes and offers Save and
/// Cancel instead.
fn queue_row(c: &Ctx, ix: usize, note: &Queued) -> AnyElement {
    let (s, t, key) = (c.s, c.t, c.key);
    let id = note.id;
    let row_key = format!("{key}-queue-{id}");
    let editing = s.editing == Some(id);
    let claimed = note.claim.is_some();
    let primary = s.primary_for(note);
    let blocked = s.blocked(note);
    // A Steer that can't steer now says why before offering Send now (pink
    // `steer_row_note`).
    let tooltip: SharedString = match blocked {
        Some(reason) => reason.into(),
        None if note.follow_up == FollowUp::Steer && primary == Primary::SendNow && s.working() => {
            "Steer unavailable. Send now interrupts the active response.".into()
        }
        None => primary.tooltip().into(),
    };

    let marker = div()
        .id(SharedString::from(format!("{row_key}-drag")))
        .w(px(20.))
        .h(px(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.))
        .cursor_pointer()
        .when(claimed, |el| el.cursor(CursorStyle::Arrow).opacity(0.35))
        .child(
            svg()
                .path(ICON_DRAG)
                .size(px(QUEUE_ICON_SIZE))
                .text_color(t.readable_muted(0.5)),
        );

    let actions = if editing {
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(3.))
            .child(queue_action(
                &row_key,
                "save",
                "Save to queue",
                ICON_SAVE,
                true,
                t,
                {
                    let state = c.state.clone();
                    move |_, cx| state.update(cx, |s, cx| s.finish_edit(true, cx))
                },
            ))
            .child(queue_action(
                &row_key,
                "cancel",
                "Cancel",
                ICON_CANCEL,
                true,
                t,
                {
                    let state = c.state.clone();
                    move |_, cx| state.update(cx, |s, cx| s.finish_edit(false, cx))
                },
            ))
    } else {
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(3.))
            .child(queue_action(
                &row_key,
                "drop",
                "Remove",
                ICON_REMOVE,
                !claimed,
                t,
                {
                    let state = c.state.clone();
                    move |_, cx| state.update(cx, |s, cx| s.remove(id, cx))
                },
            ))
            .child(queue_action(
                &row_key,
                "edit",
                "Edit",
                ICON_EDIT,
                !claimed,
                t,
                {
                    let state = c.state.clone();
                    move |window, cx| state.update(cx, |s, cx| s.begin_edit(id, window, cx))
                },
            ))
            .child(queue_primary(
                &row_key,
                primary,
                blocked.is_none(),
                tooltip,
                t,
                {
                    let state = c.state.clone();
                    move |window, cx| state.update(cx, |s, cx| s.activate(id, window, cx))
                },
            ))
    };

    let row = div()
        .id(SharedString::from(row_key.clone()))
        .role(Role::ListItem)
        .h(px(ROW_HEIGHT))
        .flex_none()
        .px(px(ROW_PAD_X))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .rounded(px(ROW_RADIUS))
        .cursor(CursorStyle::Arrow)
        .when(editing, |el| el.bg(t.fill(FillTier::Tertiary)))
        .when(!editing, |el| {
            el.hover_wash(format!("{row_key}-row"), t.fill(FillTier::Tertiary))
        })
        // The marker hints that the row belongs to the queue; the whole row
        // is the drag hitbox. Editing turns it off so selecting text never
        // reorders.
        .when(!editing && !claimed, |el| {
            el.on_drag(
                NoteDrag {
                    owner: key.to_string().into(),
                    from: ix,
                },
                |_, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| EmptyView)
                },
            )
        })
        .when(!editing, |el| el.child(marker))
        // Keep the text on its column while the marker steps aside.
        .when(editing, |el| el.child(div().w(px(14.)).flex_none()))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(QUEUE_TEXT_SIZE))
                .line_height(px(16.))
                .text_color(if editing {
                    t.text_muted
                } else {
                    t.text.opacity(0.9)
                })
                .child(if editing {
                    SharedString::from("Editing in composer")
                } else {
                    s.row_text(note)
                }),
        )
        .child(actions);

    let Some(drag) = s.drag.as_ref() else {
        return row.into_any_element();
    };
    let (start, target) = drag_offsets(ix, drag.from, drag.prev_over, drag.over);
    if c.reduced {
        return div()
            .relative()
            .top(px(target))
            .child(row)
            .into_any_element();
    }
    div()
        .flex_none()
        .child(row)
        .with_animation(
            SharedString::from(format!("{row_key}-slide-{}", drag.epoch)),
            TAB_SLIDE.animation(),
            move |el, p| el.relative().top(px(motion::lerp(start, target, p))),
        )
        .into_any_element()
}

/// A permanently visible row action (pink `queue_action`): a 28px circle in
/// `content_muted`, a Tertiary wash and `text` under the pointer, the 13px
/// glyph in `readable_muted(0.8)`; disabled at 45%.
fn queue_action_frame(
    row_key: &str,
    slot: &str,
    label: &'static str,
    enabled: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id = format!("{row_key}-{slot}");
    let ring = t.focus_ring();
    div()
        .id(SharedString::from(id.clone()))
        .role(Role::Button)
        .aria_label(label)
        .size(px(QUEUE_ACTION_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_color(t.content_muted)
        .when(enabled, |el| {
            el.cursor_pointer()
                .hover_fill_fg(
                    id,
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                    t.content_muted,
                    t.text,
                )
                .tab_index(0)
                .focus_visible(move |style| pink::focus_ring(style, ring))
                .on_key_down(pink::swallow_activation_key)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        })
        .when(!enabled, |el| el.cursor(CursorStyle::Arrow).opacity(0.45))
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
}

fn queue_action_glyph(
    row_key: &str,
    slot: &str,
    glyph: &'static str,
    t: &Pink,
) -> impl IntoElement {
    let text = t.text;
    svg()
        .path(glyph)
        .size(px(QUEUE_ICON_SIZE))
        .text_color(t.readable_muted(0.8))
        .group_hover(
            SharedString::from(format!("{row_key}-{slot}-grp")),
            move |s| s.text_color(text),
        )
}

fn queue_action(
    row_key: &str,
    slot: &str,
    label: &'static str,
    glyph: &'static str,
    enabled: bool,
    t: &Pink,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    queue_action_frame(row_key, slot, label, enabled, t)
        .group(SharedString::from(format!("{row_key}-{slot}-grp")))
        .when(enabled, |el| {
            el.on_click(move |_: &ClickEvent, window, cx| {
                cx.stop_propagation();
                on_click(window, cx);
            })
        })
        .child(queue_action_glyph(row_key, slot, glyph, t))
}

/// The row's one delivery action (pink `queue_primary_action_button`): a
/// 72×28 capsule in 12 medium `text_muted`, `text` over a Tertiary wash
/// under the pointer; 45% while blocked. Its tooltip explains it.
fn queue_primary(
    row_key: &str,
    action: Primary,
    enabled: bool,
    tooltip: SharedString,
    t: &Pink,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let id = format!("{row_key}-primary");
    let ring = t.focus_ring();
    div()
        .id(SharedString::from(id.clone()))
        .role(Role::Button)
        .aria_label(tooltip.clone())
        .w(px(QUEUE_PRIMARY_W))
        .h(px(QUEUE_ACTION_SIZE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text_muted)
        .when(enabled, |el| {
            el.cursor_pointer()
                .tab_index(0)
                .hover_fill_fg(
                    id,
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                    t.text_muted,
                    t.text,
                )
                .focus_visible(move |style| pink::focus_ring(style, ring))
                .on_key_down(pink::swallow_activation_key)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_: &ClickEvent, window, cx| {
                    cx.stop_propagation();
                    on_click(window, cx);
                })
        })
        .when(!enabled, |el| el.opacity(0.45))
        .tooltip(pink::text_tooltip(tooltip))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(action.label())
}

// -- Composer ---------------------------------------------------------------------

/// The Send / Queue / Stop disc's mode (pink `send_button_mode`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum DiscMode {
    Send,
    Queue,
    Stop,
}

/// The composer pill (pink `composer_surface` in the opaque treatment): radius
/// 26, a hairline edge, the input plate and `shadow_lg`. The textarea (`pt 16
/// pb 4 px 16`, 14/22.75, at least 60 tall) sits over the 46px controls row:
/// the Steer chip, then the disc.
fn composer_pill(c: &Ctx, focused: bool) -> Stateful<Div> {
    let (s, t, key) = (c.s, c.t, c.key);
    let working = s.working();
    let mode = match (working, s.has_text) {
        (false, _) => DiscMode::Send,
        (true, true) => DiscMode::Queue,
        (true, false) => DiscMode::Stop,
    };
    let recessed = mode != DiscMode::Stop && !s.has_text;
    let steer_on = s.follow_up == FollowUp::Steer;
    let input = s.input.clone();
    div()
        .id(SharedString::from(format!("{key}-pill")))
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(radius::COMPOSER))
        .border_1()
        .border_color(t.border)
        .bg(t.input_glass_bg())
        .shadow(pink::shadow_lg())
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            TextInput::focus(&input, window, cx)
        })
        .on_key_down({
            let state = c.state.clone();
            move |event: &KeyDownEvent, window, cx| {
                let keystroke = &event.keystroke;
                if keystroke.key == "enter" && keystroke.modifiers.secondary() {
                    cx.stop_propagation();
                    let shift = keystroke.modifiers.shift;
                    state.update(cx, |s, cx| {
                        if shift {
                            s.submit(s.follow_up.opposite(), window, cx)
                        } else {
                            s.modifier_submit(window, cx)
                        }
                    });
                }
            }
        })
        .child(
            div()
                .min_h(px(SESSION_TEXTAREA_MIN))
                .pt(px(TEXTAREA_PAD_TOP))
                .pb(px(TEXTAREA_PAD_BOTTOM))
                .px(px(16.))
                .text_size(px(INPUT_TEXT_SIZE))
                .line_height(px(INPUT_LINE_HEIGHT))
                .text_color(t.text)
                .child(s.input.clone()),
        )
        .child(
            div()
                .h(px(ACTIONS_ROW_HEIGHT))
                .flex_none()
                .pl(px(10.))
                .pr(px(10.))
                .pt(px(2.))
                .pb(px(12.))
                .flex()
                .items_center()
                .gap(px(STACK_GAP))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(steer_chip(c, steer_on)),
                )
                .child(send_disc(c, mode, recessed, focused)),
        )
}

/// The composer's Steer switch, drawn like pink's Goal mode chip
/// (`render_goal_mode_chip`): a 28px capsule, 12 medium. On, it is the
/// accent badge (accent 14% → 20%, label `readable_tone(accent)`); off, the
/// ghost trigger pill (`element_hover` under the pointer, `text_muted` →
/// `text`).
fn steer_chip(c: &Ctx, on: bool) -> Stateful<Div> {
    let (s, t, key) = (c.s, c.t, c.key);
    let id = format!("{key}-steer-mode");
    let ring = t.focus_ring();
    let accent = t.accent;
    let rest = if on {
        t.readable_tone(accent, 0.95)
    } else {
        t.text_muted
    };
    let tooltip = match (on, s.steerable) {
        (_, false) => "This agent takes notes only between steps",
        (true, true) => "Steer is on: notes join the running step without interrupting it",
        (false, true) => {
            "Notes wait for the next step. Turn on Steer to add them to the running one"
        }
    };
    let chip = div()
        .id(SharedString::from(id.clone()))
        .role(Role::Switch)
        .aria_label("Steer mode")
        .aria_toggled(if on {
            gpui::Toggled::True
        } else {
            gpui::Toggled::False
        })
        .tab_index(0)
        .flex_none()
        .h(px(layout::COMPOSER_CONTROL))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_key_down(pink::swallow_activation_key)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .tooltip(pink::text_tooltip(tooltip))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_click({
            let state = c.state.clone();
            move |_: &ClickEvent, _, cx| {
                state.update(cx, |s, cx| {
                    s.take_over();
                    s.follow_up = s.follow_up.opposite();
                    s.configured_follow_up = s.follow_up;
                    cx.notify();
                })
            }
        });
    let chip = if on {
        chip.text_color(rest)
            .hover_fill(id.clone(), accent.opacity(0.14), accent.opacity(0.2))
    } else {
        chip.hover_fill_fg(
            id.clone(),
            t.element_hover.opacity(0.),
            t.element_hover,
            rest,
            t.text,
        )
    };
    let glyph_tone = if on {
        rest
    } else {
        motion::hover_blend(&id, rest, t.text)
    };
    chip.child(
        svg()
            .path(ICON_STEER)
            .flex_none()
            .size(px(MODE_CHIP_ICON))
            .text_color(glyph_tone),
    )
    .child("Steer")
}

/// The Send / Queue / Stop disc (pink `render_send_button`): one 28px circle,
/// solid `text` with a `bg` glyph when it can act (hover 0.88, press 0.76),
/// recessed `ink(0.08)` with the glyph at 35% when there is nothing to send –
/// the arrow taking the accent while the composer is focused and empty. Only
/// the glyph changes between modes, fading in over `GLYPH_SWAP`.
fn send_disc(c: &Ctx, mode: DiscMode, recessed: bool, focused: bool) -> Stateful<Div> {
    let (s, t, key) = (c.s, c.t, c.key);
    let ready = recessed && focused;
    let glyph_color = if ready {
        t.readable_tone(t.accent, 1.0)
    } else if recessed {
        t.text.opacity(0.35)
    } else {
        t.bg
    };
    let steer = s.follow_up == FollowUp::Steer && s.steerable;
    let (state_key, label) = match (mode, recessed) {
        (DiscMode::Stop, _) => ("stop", "Stop"),
        (DiscMode::Queue, _) if steer => ("queue", "Steer (⏎)"),
        (DiscMode::Queue, _) => ("queue", "Queue (⏎)"),
        (_, false) => ("send", "Send (⏎)"),
        (_, true) if ready => ("ready", "Send (⏎)"),
        (_, true) => ("rest", "Send (⏎)"),
    };
    let glyph: AnyElement = match mode {
        DiscMode::Stop => div()
            .size(px(11.))
            .rounded(px(3.))
            .bg(glyph_color)
            .into_any_element(),
        DiscMode::Queue => svg()
            .path(ICON_QUEUE_SEND)
            .size(px(14.))
            .text_color(glyph_color)
            .into_any_element(),
        DiscMode::Send => svg()
            .path(ICON_ARROW_UP)
            .size(px(14.))
            .text_color(glyph_color)
            .into_any_element(),
    };
    let glyph = div()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph)
        .with_animation(
            SharedString::from(format!("{key}-glyph-{state_key}")),
            GLYPH_SWAP.animation(),
            |el, p| el.opacity(p),
        );
    let fade = format!("{key}-send");
    let ring = t.focus_ring();
    let disc = div()
        .id(SharedString::from(fade.clone()))
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_key_down(pink::swallow_activation_key)
        .size(px(layout::COMPOSER_CONTROL))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(glyph);
    if recessed {
        return disc.bg(t.ink(0.08));
    }
    let state = c.state.clone();
    disc.cursor_pointer()
        .bg(motion::hover_blend(&fade, t.text, t.text.opacity(0.88)))
        .on_hover(motion::hover_listener(fade))
        .active(|style| style.opacity(0.76))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            state.update(cx, |s, cx| match mode {
                DiscMode::Stop => s.set_paused(true, window, cx),
                _ => s.submit(s.follow_up, window, cx),
            })
        })
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentSteering::new(
            "agent-steering",
            "Indexer",
            "Rebuild the product search index on the new cluster",
        )
        .step(
            SteerStep::new("Snapshot the live index")
                .activity("Running a snapshot of products_v6")
                .activity("Checking the snapshot restores cleanly"),
        )
        .step(
            SteerStep::new("Read the analyzer configs")
                .activity("Reading 41 analyzer configs")
                .activity("Finding 3 configs on the legacy stemmer"),
        )
        .step(
            SteerStep::new("Map old analyzers to the new schema")
                .activity("Mapping 38 analyzers one to one")
                .activity("Checking synonyms for catalog_fr")
                .activity("Drafting mappings for catalog_en"),
        )
        .step(
            SteerStep::new("Reindex 2.4M products into the new cluster")
                .activity("Streaming batch 1 of 48")
                .activity("Streaming batch 12 of 48 · 610k documents")
                .activity("Checking rejects after batch 31"),
        )
        .step(
            SteerStep::new("Compare results on saved queries")
                .activity("Running both indexes side by side")
                .activity("Comparing top-3 results · 94% overlap")
                .activity("Collecting 12 queries that drifted"),
        )
        .step(
            SteerStep::new("Swap the alias, keep the old index a day")
                .activity("Pointing the products alias at v7")
                .activity("Scheduling the old index for removal"),
        )
        .start_at(2)
        .elapsed(134.0)
        .queued("Skip the archived catalogs – they get deleted on Friday")
        .rehearse_steer(0.8, "Keep the legacy stemmer for catalog_de")
        .rehearse(3.6, "Throttle the reindex to 2 batches at a time")
        .autoplay(true)
    })
}
