//! Agent Steering – talk to an agent while it works.
//!
//! The run reads as a track of steps that scrolls upward: **Now** at the top
//! with the agent's live activity, **Next** below it. A note you type is
//! queued under Next; when the agent finishes the current step the whole track
//! glides up, and the note travels with it into Now, where it turns into a
//! "picked up" line in the activity log. **Interrupt** (or ⌘/Ctrl+Enter)
//! skips the wait and lands the note in the current step right away.
//!
//! ```ignore
//! AgentSteering::new("reindex", "Indexer", "Rebuild the product search index")
//!     .step(SteerStep::new("Map old analyzers to the new schema").activity("Read 41 analyzer configs"))
//!     .step(SteerStep::new("Reindex 2.4M products"))
//!     .on_steer(|note, _, _| println!("{} (interrupt: {})", note.text, note.interrupt))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, App, ElementId, Entity, FontWeight, Hsla,
    IntoElement, KeyDownEvent, SharedString, Subscription, Task, Window, div, linear_color_stop,
    linear_gradient, prelude::*, px, svg,
};

use crate::{
    motion::{self, EASE_IN_OUT, EASE_OUT_QUINT, MotionSpec, ROW_IN},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, radius, space, text},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Agent Steering";
pub const DESCRIPTION: &str = "Steer a running agent: queue a note for its next step and watch the plan carry it into the work, or interrupt to apply it at once";

const ICON_PAUSE: &str = "icons/agent-steering/pause.svg";
const ICON_PLAY: &str = "icons/agent-steering/play.svg";
const ICON_BOLT: &str = "icons/agent-steering/bolt.svg";
const ICON_QUEUE: &str = "icons/agent-steering/queue.svg";
const ICON_CLOSE: &str = "icons/agent-steering/close.svg";
const ICON_CHECK: &str = "icons/agent-steering/check.svg";
const ICON_NOTE: &str = "icons/agent-steering/note.svg";

const WIDTH: f32 = 600.0;
/// Height of the scrolling step track.
const VIEW_H: f32 = 296.0;
/// Section anatomy: label + title, then entries, then bottom padding.
const HEAD_H: f32 = 58.0;
const PAD_B: f32 = 12.0;
const LINE_H: f32 = 24.0;
const APPLIED_H: f32 = 34.0;
const NOTE_H: f32 = 40.0;
/// Seconds a step takes, and when its activity lines appear.
const STEP_SECS: f32 = 6.4;
const LINE_EVERY: f32 = 1.7;
const TYPE_EVERY: f32 = 0.035;
/// The track glides a little slower than a row, so the hand-off reads.
const TRACK: MotionSpec = MotionSpec::new(640, EASE_OUT_QUINT);

/// A note sent to the agent.
#[derive(Clone, Debug)]
pub struct SteerNote {
    pub text: SharedString,
    /// Applied to the current step at once instead of waiting for the next.
    pub interrupt: bool,
}

/// One step of the agent's run, with the activity lines it reports.
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

#[derive(IntoElement)]
pub struct AgentSteering {
    id: ElementId,
    agent: SharedString,
    monogram: Option<SharedString>,
    task: SharedString,
    steps: Vec<SteerStep>,
    start_at: usize,
    elapsed: f32,
    autoplay: bool,
    rehearsals: Vec<(f32, SharedString)>,
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
            monogram: None,
            task: task.into(),
            steps: Vec::new(),
            start_at: 0,
            elapsed: 0.0,
            autoplay: false,
            rehearsals: Vec::new(),
            on_steer: None,
            on_pause: None,
        }
    }

    /// One or two letters for the agent tile (defaults to the first letter).
    pub fn monogram(mut self, monogram: impl Into<SharedString>) -> Self {
        self.monogram = Some(monogram.into());
        self
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

    /// Loops the run on its own (and plays the rehearsed notes) until someone
    /// steps in.
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    /// During autoplay, types `text` into the composer `at` seconds in and queues it.
    pub fn rehearse(mut self, at: f32, text: impl Into<SharedString>) -> Self {
        self.rehearsals.push((at, text.into()));
        self
    }

    /// Called when a note is queued or sent as an interrupt.
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

#[derive(Clone)]
enum Entry {
    Activity {
        text: SharedString,
        at: f32,
    },
    Note {
        id: usize,
        text: SharedString,
        urgent: bool,
    },
}

impl Entry {
    fn height(&self) -> f32 {
        match self {
            Entry::Activity { .. } => LINE_H,
            Entry::Note { .. } => APPLIED_H,
        }
    }
}

struct Queued {
    id: usize,
    text: SharedString,
    step: usize,
}

struct Typing {
    text: SharedString,
    shown: usize,
    next_at: f32,
}

struct SteerState {
    steps: Vec<SteerStep>,
    start_at: usize,
    start_clock: f32,
    current: usize,
    /// Seconds into the current step.
    step_t: f32,
    /// Lines revealed so far in the current step.
    lines: usize,
    clock: f32,
    /// Seconds since this state started (drives the rehearsals).
    demo_t: f32,
    step_started: Vec<f32>,
    step_took: Vec<Option<f32>>,
    log: Vec<Vec<Entry>>,
    queued: Vec<Queued>,
    next_id: usize,
    paused: bool,
    finished_at: Option<f32>,
    autoplay: bool,
    rehearsals: Vec<(f32, SharedString)>,
    played: usize,
    typing: Option<Typing>,
    input: Entity<TextInput>,
    focused: bool,
    has_text: bool,
    last_tick: Instant,
    on_steer: Option<NoteHandler>,
    on_pause: Option<PauseHandler>,
    _subscriptions: Vec<Subscription>,
    _ticker: Task<()>,
}

impl SteerState {
    fn new(config: &AgentSteering, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Steer the agent…"));
        let subscriptions =
            vec![
                cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                    match event {
                        InputEvent::Submit(_) => this.send(false, window, cx),
                        InputEvent::Change(text) => {
                            this.autoplay = false;
                            this.typing = None;
                            this.has_text = !text.trim().is_empty();
                            cx.notify();
                        }
                        InputEvent::Focus => {
                            this.focused = true;
                            this.autoplay = false;
                            this.typing = None;
                            cx.notify();
                        }
                        InputEvent::Blur => {
                            this.focused = false;
                            cx.notify();
                        }
                        _ => {}
                    }
                }),
            ];
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
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
            step_took: vec![None; count],
            log: vec![Vec::new(); count],
            queued: Vec::new(),
            next_id: 0,
            paused: false,
            finished_at: None,
            autoplay: config.autoplay,
            rehearsals: config.rehearsals.clone(),
            played: 0,
            typing: None,
            input,
            focused: false,
            has_text: false,
            last_tick: Instant::now(),
            on_steer: None,
            on_pause: None,
            _subscriptions: subscriptions,
            _ticker: ticker,
        };
        state.reset();
        state
    }

    /// Back to the configured starting point, part-way into the first step.
    fn reset(&mut self) {
        let count = self.steps.len();
        self.current = self.start_at;
        self.clock = self.start_clock;
        self.demo_t = 0.0;
        self.played = 0;
        self.typing = None;
        self.queued.clear();
        self.finished_at = None;
        self.log = vec![Vec::new(); count];
        self.step_took = vec![None; count];
        for ix in 0..self.start_at {
            // Earlier steps finished before we arrived.
            self.step_took[ix] = Some(28.0 + (ix as f32 * 17.0) % 31.0);
        }
        self.begin_step(self.start_at);
        self.step_t = LINE_EVERY * 0.9;
        self.reveal_lines();
    }

    fn begin_step(&mut self, ix: usize) {
        self.current = ix;
        self.step_t = 0.0;
        self.lines = 0;
        if let Some(started) = self.step_started.get_mut(ix) {
            *started = self.clock;
        }
        // Notes queued for this step are picked up first.
        let picked: Vec<Entry> = self
            .queued
            .iter()
            .filter(|q| q.step == ix)
            .map(|q| Entry::Note {
                id: q.id,
                text: q.text.clone(),
                urgent: false,
            })
            .collect();
        self.queued.retain(|q| q.step != ix);
        if let Some(log) = self.log.get_mut(ix) {
            log.extend(picked);
        }
    }

    fn reveal_lines(&mut self) {
        let Some(step) = self.steps.get(self.current) else {
            return;
        };
        while self.lines < step.activity.len()
            && self.step_t >= 0.5 + self.lines as f32 * LINE_EVERY
        {
            let text = step.activity[self.lines].clone();
            self.log[self.current].push(Entry::Activity {
                text,
                at: self.clock,
            });
            self.lines += 1;
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .min(0.25);
        self.last_tick = now;
        self.demo_t += dt;

        // Rehearsed notes: type them into the composer, then queue them.
        if self.autoplay {
            if self.typing.is_none()
                && let Some((at, text)) = self.rehearsals.get(self.played).cloned()
                && self.demo_t >= at
            {
                self.played += 1;
                self.typing = Some(Typing {
                    text,
                    shown: 0,
                    next_at: self.demo_t,
                });
            }
            if let Some(typing) = self.typing.as_mut()
                && self.demo_t >= typing.next_at
            {
                let total = typing.text.chars().count();
                if typing.shown < total {
                    typing.shown = (typing.shown + 3).min(total);
                    typing.next_at = self.demo_t + TYPE_EVERY;
                    let partial: String = typing.text.chars().take(typing.shown).collect();
                    self.has_text = true;
                    self.input
                        .update(cx, |input, cx| input.set_text(partial, cx));
                } else if self.demo_t >= typing.next_at + 0.45 {
                    let text = typing.text.clone();
                    self.typing = None;
                    self.queue(text, false, cx);
                }
            }
        }

        if !self.paused && self.finished_at.is_none() {
            self.clock += dt;
            self.step_t += dt;
            self.reveal_lines();
            if self.step_t >= STEP_SECS {
                let took = self.clock - self.step_started[self.current];
                self.step_took[self.current] = Some(took);
                if self.current + 1 < self.steps.len() {
                    self.begin_step(self.current + 1);
                } else {
                    self.finished_at = Some(self.demo_t);
                }
            }
        }

        if self.autoplay
            && let Some(done) = self.finished_at
            && self.demo_t - done > 5.0
        {
            self.reset();
        }
        cx.notify();
    }

    /// The step a queued note lands in: the next one, or the last.
    fn next_step(&self) -> usize {
        (self.current + 1).min(self.steps.len().saturating_sub(1))
    }

    fn queue(&mut self, text: SharedString, interrupt: bool, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let finished = self.finished_at.is_some();
        if interrupt || self.next_step() == self.current || finished {
            if let Some(log) = self.log.get_mut(self.current) {
                log.push(Entry::Note {
                    id,
                    text,
                    urgent: interrupt,
                });
            }
            if interrupt && !finished {
                // The agent drops what it was doing and picks the step up again
                // with the note: its next line (and the step's end) move back.
                let last_line = 0.5 + self.lines.saturating_sub(1) as f32 * LINE_EVERY;
                self.step_t = self.step_t.min(last_line);
                self.paused = false;
            }
        } else {
            let step = self.next_step();
            self.queued.push(Queued { id, text, step });
        }
        self.has_text = false;
        self.input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    fn send(&mut self, interrupt: bool, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text();
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.autoplay = false;
        let text = SharedString::from(text.to_string());
        self.queue(text.clone(), interrupt, cx);
        if let Some(handler) = self.on_steer.clone() {
            handler(&SteerNote { text, interrupt }, window, cx);
        }
    }

    fn send_now(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.queued.iter().position(|q| q.id == id) else {
            return;
        };
        self.autoplay = false;
        let note = self.queued.remove(ix);
        self.queue(note.text.clone(), true, cx);
        if let Some(handler) = self.on_steer.clone() {
            handler(
                &SteerNote {
                    text: note.text,
                    interrupt: true,
                },
                window,
                cx,
            );
        }
    }

    fn section_height(&self, ix: usize) -> f32 {
        let mut h = HEAD_H + PAD_B;
        h += self.log[ix].iter().map(Entry::height).sum::<f32>();
        h += self.queued.iter().filter(|q| q.step == ix).count() as f32 * NOTE_H;
        h
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn clock_label(seconds: f32) -> String {
    let s = seconds.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Done,
    Now,
    Next,
    Later,
}

/// A softly pulsing dot: the agent is working.
fn pulse(id: impl Into<ElementId>, color: Hsla, live: bool) -> AnyElement {
    let dot = div().size(px(6.)).flex_none().rounded_full().bg(color);
    if live {
        dot.with_animation(
            id,
            Animation::new(Duration::from_millis(1400))
                .repeat()
                .with_easing(EASE_IN_OUT.easing()),
            |el, t| el.opacity(0.35 + 0.65 * (1.0 - (t * 2.0 - 1.0).abs())),
        )
        .into_any_element()
    } else {
        dot.into_any_element()
    }
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
        let (on_steer, on_pause) = (self.on_steer.clone(), self.on_pause.clone());
        state.update(cx, |s, _| {
            s.on_steer = on_steer;
            s.on_pause = on_pause;
        });
        let s = state.read(cx);
        let total = s.steps.len();
        let finished = s.finished_at.is_some();
        let working = !s.paused && !finished;

        // -- Header ---------------------------------------------------------
        let monogram = self.monogram.clone().unwrap_or_else(|| {
            self.agent
                .chars()
                .next()
                .map(|c| c.to_uppercase().collect::<String>())
                .unwrap_or_default()
                .into()
        });
        let (status, status_color) = if finished {
            ("Finished", t.success)
        } else if s.paused {
            ("Paused", t.warning)
        } else {
            ("Working", t.accent)
        };
        let pause_label = if s.paused { ICON_PLAY } else { ICON_PAUSE };
        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S12))
            .px(px(space::S16))
            .py(px(space::S12))
            .child(
                div()
                    .size(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::MD))
                    .bg(t.accent_wash)
                    .text_size(px(text::FOOTNOTE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if t.dark { t.text } else { t.accent })
                    .child(monogram),
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
                            .child(self.task.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(t.text_muted)
                            .child(format!(
                                "{} · step {} of {total}",
                                self.agent,
                                (s.current + 1).min(total)
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .h(px(24.))
                    .px(px(space::S8))
                    .rounded(px(radius::SM))
                    .bg(t.fill(FillTier::Quaternary))
                    .border_1()
                    .border_color(t.border)
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .child(pulse(
                        ElementId::Name(format!("{key}-status-dot").into()),
                        status_color,
                        working,
                    ))
                    .child(div().text_color(t.text).child(status))
                    .child(div().font_family(pink::MONO).child(clock_label(s.clock))),
            )
            .child(
                pink::icon_button(format!("{key}-pause"), pause_label, ControlSize::Md, &t)
                    .when(finished, |el| el.opacity(0.4))
                    .on_click({
                        let state = state.clone();
                        move |_, window, cx| {
                            let handler = state.update(cx, |s, cx| {
                                if s.finished_at.is_some() {
                                    return None;
                                }
                                s.autoplay = false;
                                s.paused = !s.paused;
                                cx.notify();
                                s.on_pause.clone().map(|h| (h, s.paused))
                            });
                            if let Some((handler, paused)) = handler {
                                handler(paused, window, cx);
                            }
                        }
                    }),
            );

        // -- Step track -------------------------------------------------------
        let offset: f32 = (0..s.current).map(|ix| s.section_height(ix)).sum();
        let offset = motion::follow(&format!("{key}-offset"), offset, TRACK);
        let mut sections = Vec::new();
        for (ix, step) in s.steps.iter().enumerate() {
            let role = if ix < s.current || (finished && ix == s.current) {
                Role::Done
            } else if ix == s.current {
                Role::Now
            } else if ix == s.current + 1 {
                Role::Next
            } else {
                Role::Later
            };
            let h = motion::follow(
                &format!("{key}-sec-{ix}"),
                s.section_height(ix),
                motion::GLIDE,
            );
            let now_t = motion::follow(
                &format!("{key}-now-{ix}"),
                if role == Role::Now { 1.0 } else { 0.0 },
                TRACK,
            );
            let queued_here: Vec<&Queued> = s.queued.iter().filter(|q| q.step == ix).collect();

            let label: SharedString = match role {
                Role::Done => match s.step_took[ix] {
                    Some(took) => format!("Done · {}", clock_label(took)).into(),
                    None => "Done".into(),
                },
                Role::Now => format!("Now · step {} of {total}", ix + 1).into(),
                Role::Next if !queued_here.is_empty() => format!(
                    "Next · {} {} waiting",
                    queued_here.len(),
                    if queued_here.len() == 1 {
                        "note"
                    } else {
                        "notes"
                    }
                )
                .into(),
                Role::Next => format!("Next · step {}", ix + 1).into(),
                Role::Later => format!("Step {}", ix + 1).into(),
            };
            let label_color = match role {
                Role::Done => t.success,
                Role::Now if s.paused => t.warning,
                Role::Now => t.accent,
                Role::Next => t.text_muted,
                Role::Later => t.text_faint,
            };
            let title_color = match role {
                Role::Done => t.text_muted,
                Role::Now | Role::Next => t.text,
                Role::Later => t.text_faint,
            };
            let elapsed_here = if role == Role::Now {
                Some(clock_label(s.clock - s.step_started[ix]))
            } else {
                None
            };

            let head = div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(space::S2))
                .child(
                    div()
                        .h(px(16.))
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .text_size(px(text::CAPTION))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(label_color)
                        .when(role == Role::Now, |el| {
                            el.child(pulse(
                                ElementId::Name(format!("{key}-now-dot-{ix}").into()),
                                label_color,
                                working,
                            ))
                        })
                        .when(role == Role::Done, |el| {
                            el.child(svg().path(ICON_CHECK).size(px(12.)).text_color(t.success))
                        })
                        .child(div().flex_1().child(label))
                        .when_some(elapsed_here, |el, elapsed| {
                            el.child(
                                div()
                                    .font_family(pink::MONO)
                                    .font_weight(FontWeight::NORMAL)
                                    .text_color(t.text_faint)
                                    .child(elapsed),
                            )
                        }),
                )
                .child(
                    div().flex().child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::CALLOUT))
                            .line_height(px(22.))
                            .font_weight(if role == Role::Now {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(title_color)
                            .child(step.title.clone()),
                    ),
                );

            // Activity and picked-up notes.
            let entries = s.log[ix].len();
            let entry_rows = s.log[ix].iter().enumerate().map(|(n, entry)| {
                let newest = n + 1 == entries && role == Role::Now;
                match entry {
                    Entry::Activity { text: line, at } => motion::rise_in(
                        ElementId::Name(format!("{key}-line-{ix}-{n}").into()),
                        ROW_IN,
                        6.0,
                        div()
                            .h(px(LINE_H))
                            .flex()
                            .items_center()
                            .gap(px(space::S10))
                            .text_size(px(text::BODY))
                            .child(
                                div()
                                    .w(px(34.))
                                    .flex_none()
                                    .font_family(pink::MONO)
                                    .text_size(px(text::CAPTION))
                                    .text_color(t.text_faint)
                                    .child(clock_label(*at)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(if newest { t.text } else { t.text_muted })
                                    .child(line.clone()),
                            )
                            .when(newest && working, |el| {
                                el.child(pulse(
                                    ElementId::Name(format!("{key}-busy-{ix}").into()),
                                    t.text_faint,
                                    true,
                                ))
                            }),
                    )
                    .into_any_element(),
                    Entry::Note {
                        id,
                        text: note,
                        urgent,
                    } => {
                        let tint = if *urgent { t.warning } else { t.accent };
                        div()
                            .h(px(APPLIED_H))
                            .py(px(3.))
                            .child(
                                div()
                                    .h_full()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::S8))
                                    .pl(px(space::S10))
                                    .pr(px(space::S8))
                                    .rounded(px(radius::SM))
                                    .bg(tint.opacity(if t.dark { 0.12 } else { 0.07 }))
                                    .border_l_2()
                                    .border_color(tint)
                                    .text_size(px(text::BODY))
                                    .child(svg().path(ICON_NOTE).size(px(14.)).text_color(tint))
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_color(t.text)
                                            .child(note.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(space::S4))
                                            .text_size(px(text::CAPTION))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(tint)
                                            .child(
                                                svg()
                                                    .path(if *urgent {
                                                        ICON_BOLT
                                                    } else {
                                                        ICON_CHECK
                                                    })
                                                    .size(px(12.))
                                                    .text_color(tint),
                                            )
                                            .child(if *urgent {
                                                "Applied now"
                                            } else {
                                                "Picked up"
                                            }),
                                    ),
                            )
                            .with_animation(
                                ElementId::Name(format!("{key}-applied-{id}").into()),
                                MotionSpec::new(360, motion::EASE).animation(),
                                |el, p| el.opacity(p),
                            )
                            .into_any_element()
                    }
                }
            });

            // Notes still waiting for this step.
            let waiting = queued_here.iter().map(|q| {
                let id = q.id;
                let note_key = format!("{key}-queued-{id}");
                motion::rise_in(
                    ElementId::Name(format!("{note_key}-in").into()),
                    ROW_IN,
                    8.0,
                    div().h(px(NOTE_H)).pb(px(6.)).child(
                        div()
                            .id(ElementId::Name(note_key.clone().into()))
                            .h_full()
                            .flex()
                            .items_center()
                            .gap(px(space::S8))
                            .pl(px(space::S10))
                            .pr(px(space::S4))
                            .rounded(px(radius::MD))
                            .border_1()
                            .border_color(t.border_strong)
                            .bg(t.surface_card)
                            .shadow(t.card_shadow())
                            .text_size(px(text::BODY))
                            .child(svg().path(ICON_NOTE).size(px(14.)).text_color(t.text_muted))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_color(t.text)
                                    .child(q.text.clone()),
                            )
                            .child(
                                pink::button(
                                    format!("{note_key}-now"),
                                    ButtonKind::Ghost,
                                    ControlSize::Sm,
                                    &t,
                                )
                                .child(
                                    pink::glyph(ICON_BOLT, ControlSize::Sm).text_color(t.warning),
                                )
                                .child("Send now")
                                .on_click({
                                    let state = state.clone();
                                    move |_, window, cx| {
                                        state.update(cx, |s, cx| s.send_now(id, window, cx))
                                    }
                                }),
                            )
                            .child(
                                pink::icon_button(
                                    format!("{note_key}-drop"),
                                    ICON_CLOSE,
                                    ControlSize::Sm,
                                    &t,
                                )
                                .on_click({
                                    let state = state.clone();
                                    move |_, _, cx| {
                                        state.update(cx, |s, cx| {
                                            s.autoplay = false;
                                            s.queued.retain(|q| q.id != id);
                                            cx.notify();
                                        })
                                    }
                                }),
                            ),
                    ),
                )
            });

            sections.push(
                div()
                    .relative()
                    .h(px(h))
                    .flex_none()
                    .overflow_hidden()
                    .px(px(space::S16))
                    .pt(px(space::S12))
                    .bg(t.fill(FillTier::Quaternary).opacity(now_t))
                    .child(head)
                    .child(
                        div()
                            .mt(px(space::S6))
                            .flex()
                            .flex_col()
                            .children(entry_rows)
                            .children(waiting),
                    )
                    // Hairline under every section.
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left(px(space::S16))
                            .right(px(space::S16))
                            .h(px(1.))
                            .bg(t.border.opacity(1.0 - now_t)),
                    ),
            );
        }
        let fade_top = linear_gradient(
            180.,
            linear_color_stop(t.surface_card, 0.),
            linear_color_stop(t.surface_card.opacity(0.), 1.),
        );
        let fade_bottom = linear_gradient(
            0.,
            linear_color_stop(t.surface_card, 0.),
            linear_color_stop(t.surface_card.opacity(0.), 1.),
        );
        let track = div()
            .relative()
            .h(px(VIEW_H))
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(-offset))
                    .flex()
                    .flex_col()
                    .children(sections),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(10.))
                    .bg(fade_top),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(44.))
                    .bg(fade_bottom),
            );

        // -- Composer -----------------------------------------------------------
        let next_step = s.next_step();
        let hint: SharedString = if finished {
            "The run is over – notes go to the final step".into()
        } else if next_step == s.current {
            "Last step – notes apply right away".into()
        } else {
            format!("Notes land at the start of step {}", next_step + 1).into()
        };
        let ready = s.has_text;
        let interrupt_keys = if cfg!(target_os = "macos") {
            "⌘↵"
        } else {
            "Ctrl ↵"
        };
        let composer = div()
            .p(px(space::S12))
            .border_t_1()
            .border_color(t.border)
            .child(
                div()
                    .id(ElementId::Name(format!("{key}-composer").into()))
                    .flex()
                    .flex_col()
                    .gap(px(space::S8))
                    .p(px(space::S10))
                    .pl(px(space::S12))
                    .rounded(px(radius::LG))
                    .bg(t.input_bg)
                    .border_1()
                    .border_color(if s.focused { t.focus_ring() } else { t.border })
                    .on_key_down({
                        let state = state.clone();
                        move |event: &KeyDownEvent, window, cx| {
                            if event.keystroke.key == "enter"
                                && event.keystroke.modifiers.secondary()
                            {
                                cx.stop_propagation();
                                state.update(cx, |s, cx| s.send(true, window, cx));
                            }
                        }
                    })
                    .child(
                        div()
                            .text_size(px(text::BODY))
                            .line_height(px(20.))
                            .text_color(t.text)
                            .child(s.input.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S6))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_faint)
                                    .child(hint),
                            )
                            .child(
                                pink::button(
                                    format!("{key}-interrupt"),
                                    ButtonKind::Ghost,
                                    ControlSize::Sm,
                                    &t,
                                )
                                .when(!ready, |el| el.opacity(0.5))
                                .child(
                                    pink::glyph(ICON_BOLT, ControlSize::Sm).text_color(t.warning),
                                )
                                .child("Interrupt")
                                .child(
                                    div()
                                        .text_color(t.text_faint)
                                        .font_weight(FontWeight::NORMAL)
                                        .child(interrupt_keys),
                                )
                                .on_click({
                                    let state = state.clone();
                                    move |_, window, cx| {
                                        state.update(cx, |s, cx| s.send(true, window, cx))
                                    }
                                }),
                            )
                            .child(
                                pink::button(
                                    format!("{key}-queue"),
                                    if ready {
                                        ButtonKind::Accent
                                    } else {
                                        ButtonKind::Secondary
                                    },
                                    ControlSize::Sm,
                                    &t,
                                )
                                .child(
                                    pink::glyph(ICON_QUEUE, ControlSize::Sm).text_color(if ready {
                                        t.on_accent
                                    } else {
                                        t.text_muted
                                    }),
                                )
                                .child("Queue")
                                .when(!ready, |el| el.text_color(t.text_muted))
                                .on_click({
                                    let state = state.clone();
                                    move |_, window, cx| {
                                        state.update(cx, |s, cx| s.send(false, window, cx))
                                    }
                                }),
                            ),
                    ),
            );

        motion::keep_alive(window);
        pink::card(&t)
            .w(px(WIDTH))
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(header)
            .child(pink::divider(&t))
            .child(track)
            .child(composer)
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentSteering::new(
            "agent-steering",
            "Indexer",
            "Rebuild the product search index",
        )
        .monogram("Ix")
        .step(SteerStep::new("Snapshot the live index"))
        .step(SteerStep::new("Read the analyzer configs"))
        .step(
            SteerStep::new("Map old analyzers to the new schema")
                .activity("Read 41 analyzer configs")
                .activity("3 still use the legacy stemmer")
                .activity("Drafting mappings for catalog_en"),
        )
        .step(
            SteerStep::new("Reindex 2.4M products into the new cluster")
                .activity("Streaming batch 1 of 48")
                .activity("Batch 12 of 48 · 610k documents")
                .activity("Batch 31 of 48 · no rejects so far"),
        )
        .step(
            SteerStep::new("Compare results on saved queries")
                .activity("Running both indexes side by side")
                .activity("Top-3 overlap is 94%")
                .activity("12 queries drifted – collecting them"),
        )
        .step(
            SteerStep::new("Swap the alias, keep the old index a day")
                .activity("Alias products now points at v7")
                .activity("Old index scheduled for removal tomorrow"),
        )
        .start_at(2)
        .elapsed(134.0)
        .rehearse(
            0.8,
            "Skip the archived catalogs – they get deleted on Friday",
        )
        .rehearse(7.6, "Use the 200 most searched queries for the comparison")
        .autoplay(true)
    })
}
