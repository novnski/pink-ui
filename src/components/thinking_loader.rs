//! Thinking Loader – the live line under a reply while the model works.
//!
//! Pink: `transcript.rs` `Transcript::render_working_trailer`, the working
//! trailer under the last transcript row. No chip and no card: a bare row
//! (`pt 16`, `gap 8`, top-aligned) of the breathing accent orb in a 16px line
//! box, the status – a calm verb in `content_muted` under a slow shimmer, then
//! its object in `text` – a tabular "12s" timer in `content_faint`, and a
//! "Stop" that only shows under the pointer. Each new status fades in with a
//! 2px settle. While a message is still on its way the line reads "Sending"
//! (no timer), "Setting up worktree <command>" or, offline, "Queued, will
//! send automatically"; a provider notice ("Reconnecting… 2/5") takes the
//! status's place while it lasts. Pink has no done state: when the turn ends
//! the trailer simply leaves, so stop rendering it.
//!
//! ```ignore
//! ThinkingLoader::new("reply")
//!     .phases([
//!         LoaderPhase::new("Thinking"),
//!         LoaderPhase::new("Reading").object("ci.yml"),
//!         LoaderPhase::new("Running").object("cargo test --workspace"),
//!     ])
//!     .phase(step)
//!     .started_at(turn_started)
//!     .on_stop(|_, _, cx| stop_turn(cx))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyView, App, BoxShadow, ClickEvent, Context, ElementId, Hsla, IntoElement, Role, SharedString,
    Task, Window, div, point, prelude::*, px,
};

use crate::{
    HoverFill, motion,
    pink::{
        self, FillTier, Pink, TextStyle, TypeStyleExt,
        loaders::{
            LIVE_STATUS_OBJECT_CHARS, WORKING_STATUS_LINE_HEIGHT, WORKING_TIMER_SIZE,
            ellipsize_chars, orb_slot, working_status, working_timer,
        },
        radius, space,
    },
    story,
};

pub const NAME: &str = "Thinking Loader";
pub const DESCRIPTION: &str = "The live line under an AI reply while it works: a breathing orb, a shimmering status that names what it is doing, a ticking timer and a quiet Stop on hover";

/// The phases shown when none are given: Pink's calm fallback word – with
/// nothing specific in flight it says "Thinking" rather than inventing
/// progress (Pink `live_status`).
pub const DEFAULT_PHASES: [&str; 1] = ["Thinking"];

/// Pink: the queued line (`render_working_trailer`).
const QUEUED_LABEL: &str = "Queued, will send automatically";
/// Pink: the line while the host runs the worktree's setup command.
const SETUP_LABEL: &str = "Setting up worktree";
/// Pink: the send → turn bridge.
const SENDING_LABEL: &str = "Sending";
/// The group the hover-only Stop listens to (Pink `"working-trailer"`).
const TRAILER_GROUP: &str = "working-trailer";

type StopHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// One step of the work (Pink `LiveStatus`): a present-tense verb and, when
/// the step names one, what it acts on – "Running" + "cargo test". The verb
/// shimmers; the object reads in primary text and is cut to one line of at
/// most 44 characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoaderPhase {
    pub verb: SharedString,
    pub object: Option<SharedString>,
}

impl LoaderPhase {
    pub fn new(verb: impl Into<SharedString>) -> Self {
        Self {
            verb: verb.into(),
            object: None,
        }
    }

    /// What the verb acts on: a file, a command, a pattern, a host.
    pub fn object(mut self, object: impl Into<SharedString>) -> Self {
        self.object = Some(object.into());
        self
    }

    /// The whole line, "Running cargo test" (Pink `LiveStatus::label`).
    pub fn label(&self) -> SharedString {
        match &self.object {
            Some(object) => format!("{} {object}", self.verb).into(),
            None => self.verb.clone(),
        }
    }
}

impl From<&'static str> for LoaderPhase {
    fn from(verb: &'static str) -> Self {
        Self::new(verb)
    }
}

impl From<String> for LoaderPhase {
    fn from(verb: String) -> Self {
        Self::new(verb)
    }
}

impl From<SharedString> for LoaderPhase {
    fn from(verb: SharedString) -> Self {
        Self::new(verb)
    }
}

impl From<(&'static str, &'static str)> for LoaderPhase {
    fn from((verb, object): (&'static str, &'static str)) -> Self {
        Self::new(verb).object(object)
    }
}

/// The trailer's clock (Pink: the turn's `started_at`). It wakes the host
/// view on every whole second for the timer and on every phase boundary, so
/// both keep moving under reduced motion too, when the pulse clock rests.
struct Clock {
    mounted: Instant,
    start: Instant,
    /// Set while phases advance on their own.
    interval_ms: Option<u64>,
    _ticker: Task<()>,
}

impl Clock {
    fn new(now: Instant, cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |clock, _| clock.next_tick(Instant::now())) else {
                    break;
                };
                cx.background_executor().timer(wait).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Self {
            mounted: now,
            start: now,
            interval_ms: None,
            _ticker: ticker,
        }
    }

    /// Until just past the next second or phase boundary.
    fn next_tick(&self, now: Instant) -> Duration {
        let ms = now.saturating_duration_since(self.start).as_millis() as u64;
        let mut wait = 1000 - ms % 1000;
        if let Some(interval) = self.interval_ms {
            wait = wait.min(interval - ms % interval);
        }
        Duration::from_millis(wait + 1)
    }
}

/// Pink `single_line`: whitespace runs (newlines included) become one space.
fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A live status object as Pink shows it: one line, at most 44 characters.
fn status_object(object: &str) -> SharedString {
    ellipsize_chars(&single_line(object), LIVE_STATUS_OBJECT_CHARS).into()
}

/// Pink's working trailer: "the model is working" under the reply.
#[derive(IntoElement)]
pub struct ThinkingLoader {
    id: ElementId,
    phases: Vec<LoaderPhase>,
    interval: Duration,
    phase: Option<usize>,
    looping: bool,
    started_at: Option<Instant>,
    sending: bool,
    queued: bool,
    setup: Option<SharedString>,
    notice: Option<SharedString>,
    on_stop: Option<StopHandler>,
}

impl ThinkingLoader {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            phases: DEFAULT_PHASES
                .iter()
                .map(|p| LoaderPhase::new(*p))
                .collect(),
            interval: Duration::from_millis(2200),
            phase: None,
            looping: false,
            started_at: None,
            sending: false,
            queued: false,
            setup: None,
            notice: None,
            on_stop: None,
        }
    }

    /// Steps to walk through, one after another: plain verbs (`"Thinking"`),
    /// `(verb, object)` pairs or [`LoaderPhase`]s.
    pub fn phases<P: Into<LoaderPhase>>(mut self, phases: impl IntoIterator<Item = P>) -> Self {
        self.phases = phases.into_iter().map(Into::into).collect();
        self
    }

    /// Time per phase, in milliseconds (default 2200), counted from
    /// [`started_at`](Self::started_at).
    pub fn interval(mut self, ms: u64) -> Self {
        self.interval = Duration::from_millis(ms);
        self
    }

    /// Shows a specific phase instead of advancing on a timer – the newest
    /// unresolved step of your stream, as Pink derives it.
    pub fn phase(mut self, index: usize) -> Self {
        self.phase = Some(index);
        self
    }

    /// Starts over after the last phase instead of holding it.
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// When the turn began: the timer and the phase clock count from here
    /// (default: when the loader first rendered). For [`setup`](Self::setup),
    /// when the setup command started.
    pub fn started_at(mut self, at: Instant) -> Self {
        self.started_at = Some(at);
        self
    }

    /// The message is still on its way to the agent: "Sending", no timer and
    /// no Stop (Pink's send → turn bridge).
    pub fn sending(mut self, sending: bool) -> Self {
        self.sending = sending;
        self
    }

    /// The send is held offline and goes out on reconnect: a warning-toned
    /// "Queued — will send automatically", no timer and no Stop.
    pub fn queued(mut self, queued: bool) -> Self {
        self.queued = queued;
        self
    }

    /// The host holds the message behind the worktree's setup command:
    /// "Setting up worktree <command>", timed from
    /// [`started_at`](Self::started_at), no Stop.
    pub fn setup(mut self, command: impl Into<SharedString>) -> Self {
        self.setup = Some(command.into());
        self
    }

    /// A transient provider notice ("Reconnecting… 2/5") that takes the
    /// status's place in a warning tone and may wrap. Ignored while sending.
    pub fn notice(mut self, notice: impl Into<SharedString>) -> Self {
        self.notice = Some(notice.into());
        self
    }

    /// Shows the hover-only "Stop" (Pink: interrupts the turn; the composer's
    /// Stop button stays the primary control).
    pub fn on_stop(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ThinkingLoader {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let now = Instant::now();
        let key = self.id.to_string();

        let count = self.phases.len();
        let interval_ms = (self.interval.as_millis() as u64).max(1);
        let auto = self.phase.is_none() && count > 1;
        let clock = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "clock".into()),
            cx,
            |_, cx| Clock::new(now, cx),
        );
        let start = clock.update(cx, |clock, _| {
            clock.start = self.started_at.unwrap_or(clock.mounted);
            clock.interval_ms = auto.then_some(interval_ms);
            clock.start
        });
        let elapsed = now.saturating_duration_since(start);
        let elapsed_secs = elapsed.as_secs() as i64;

        // The current step: timed from the start, or the one you named
        let ticks = (elapsed.as_millis() as u64 / interval_ms) as usize;
        let step = match (count < 2, self.looping) {
            (true, _) => 0,
            (false, true) => ticks % count,
            (false, false) => ticks.min(count - 1),
        };
        let index = self.phase.unwrap_or(step).min(count.saturating_sub(1));
        let status = self
            .phases
            .get(index)
            .cloned()
            .unwrap_or_else(|| LoaderPhase::new(DEFAULT_PHASES[0]));

        // Pink: queued, then setup, then sending, then a provider notice,
        // then the live status – one line, three tones.
        let queued = self.queued;
        let setup = (!queued).then_some(self.setup).flatten();
        let sending = self.sending || queued || setup.is_some();
        let notice = (!sending).then_some(self.notice).flatten();
        let (verb, object, tone): (SharedString, Option<SharedString>, Hsla) = if queued {
            (QUEUED_LABEL.into(), None, t.warning)
        } else if let Some(command) = &setup {
            (
                SETUP_LABEL.into(),
                Some(status_object(command).into()),
                t.content_muted,
            )
        } else if sending {
            (SENDING_LABEL.into(), None, t.content_muted)
        } else if let Some(notice) = &notice {
            (notice.clone(), None, t.warning)
        } else {
            (
                status.verb.clone(),
                status.object.as_deref().map(status_object),
                t.content_muted,
            )
        };
        // A sentence wraps in the width the timer leaves and does not shimmer
        let sentence = queued || notice.is_some();
        let status_line = {
            let line = working_status(verb, tone, t.text).sentence(sentence);
            match object {
                Some(object) => line.object(object),
                None => line,
            }
        };

        // A held send has no timer of its own; a setup holding it does
        let show_timer = !sending || setup.is_some();
        let stop = self.on_stop.filter(|_| !sending);

        div()
            .id(self.id)
            .group(TRAILER_GROUP)
            .font_family(pink::SANS)
            .flex()
            .flex_row()
            // Each item is one line box tall, so the orb, timer and Stop
            // stay on the first line of a wrapped sentence
            .items_start()
            .gap(px(space::SM))
            .pt(px(space::LG))
            .min_w_0()
            .child(orb_slot(t.accent))
            .child(status_line)
            .when(show_timer, |el| {
                el.child(working_timer(elapsed_secs, t.content_faint))
            })
            .when_some(stop, |el, on_stop| el.child(stop_button(&key, on_stop, &t)))
    }
}

/// Pink: the trailer's "Stop" – a quiet way out that appears only under the
/// pointer (or on keyboard focus), 11/16 `content_muted` with the standard
/// hover wash.
fn stop_button(key: &str, on_stop: StopHandler, t: &Pink) -> impl IntoElement {
    div()
        .id("working-stop")
        .role(Role::Button)
        .aria_label("Stop the current turn")
        .tab_index(0)
        .flex_none()
        .ml(px(space::XS))
        .px(px(space::S6))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_size(px(WORKING_TIMER_SIZE))
        .line_height(px(WORKING_STATUS_LINE_HEIGHT))
        .text_color(t.content_muted)
        .opacity(0.)
        .group_hover(TRAILER_GROUP, |style| style.opacity(1.))
        .focus_visible(|style| style.opacity(1.))
        .hover_wash(format!("{key}-stop"), t.fill(FillTier::Tertiary))
        .child("Stop")
        .on_click(move |event, window, cx| {
            cx.stop_propagation();
            on_stop(event, window, cx);
        })
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// Pink: the user bubble's line height (`transcript.rs` `USER_LINE_HEIGHT`)
/// and assistant markdown's (`markdown/render.rs` `MD_LINE_HEIGHT`), both at
/// 14px (`MD_TEXT_SIZE`).
const TRANSCRIPT_LINE_HEIGHT: f32 = 22.;

const DEMO_QUESTION: &str = "CI hangs on cargo test since this morning. Can you find out why?";
const DEMO_REPLY: [&str; 2] = [
    "The hang is in the pool tests: one of them calls block_on inside the Tokio runtime and ends up waiting on itself.",
    " Switching it to #[tokio::test] fixes it, and the whole suite now passes locally in 41s.",
];

/// The steps of the demo turn, as Pink would name them.
fn demo_phases() -> [LoaderPhase; 5] {
    [
        LoaderPhase::new("Thinking"),
        LoaderPhase::new("Reading").object("ci.yml"),
        // joiners: allow a shell command's argument separator
        LoaderPhase::new("Running").object("cargo test --workspace -- --test-threads=1"),
        LoaderPhase::new("Searching for").object("block_on"),
        LoaderPhase::new("Writing"),
    ]
}

/// One beat of the demo turn.
#[derive(Clone, Copy, PartialEq)]
enum Beat {
    /// The message is on its way.
    Send,
    /// Working on a step.
    Step(usize),
    /// A provider retry over a step.
    Notice(usize),
    /// The turn is over: the trailer leaves, the reply stays.
    Done,
}

const DEMO_SCRIPT: [(Beat, u64); 8] = [
    (Beat::Send, 1200),
    (Beat::Step(0), 2400),
    (Beat::Step(1), 2200),
    (Beat::Step(2), 2400),
    (Beat::Notice(2), 1800),
    (Beat::Step(3), 2200),
    (Beat::Step(4), 2600),
    (Beat::Done, 3200),
];

/// The demo's clock: sends, works through the steps, answers, starts over.
struct Demo {
    mounted: Instant,
    run: u64,
    beat: usize,
    turn_started: Option<Instant>,
    /// Where Stop ended the turn, if it did.
    stopped: Option<Beat>,
    _timer: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            mounted: Instant::now(),
            run: 0,
            beat: 0,
            turn_started: None,
            stopped: None,
            _timer: Self::play(cx),
        }
    }

    /// Waits out the current beat, then moves on, forever.
    fn play(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |demo, _| DEMO_SCRIPT[demo.beat].1) else {
                    break;
                };
                cx.background_executor()
                    .timer(Duration::from_millis(wait))
                    .await;
                let next = this.update(cx, |demo, cx| {
                    demo.advance();
                    cx.notify();
                });
                if next.is_err() {
                    break;
                }
            }
        })
    }

    fn advance(&mut self) {
        self.beat += 1;
        if self.beat >= DEMO_SCRIPT.len() {
            self.run += 1;
            self.beat = 0;
            self.turn_started = None;
            self.stopped = None;
        } else if self.turn_started.is_none() {
            // The turn (and its timer) starts once the send lands
            self.turn_started = Some(Instant::now());
        }
    }

    /// Stop from the trailer: the turn ends where it is, the trailer leaves.
    fn stop(&mut self, cx: &mut Context<Self>) {
        self.stopped = Some(self.current());
        self.beat = DEMO_SCRIPT.len() - 1;
        self._timer = Self::play(cx);
        cx.notify();
    }

    fn current(&self) -> Beat {
        DEMO_SCRIPT[self.beat].0
    }

    /// A start `secs` before the story mounted, for the static states.
    fn ago(&self, secs: u64) -> Instant {
        self.mounted
            .checked_sub(Duration::from_secs(secs))
            .unwrap_or(self.mounted)
    }
}

/// Pink: the user message bubble (`transcript.rs` user row) – right-aligned,
/// 80% wide at most, r16, 14/22 on the bubble wash with a light-mode edge.
fn user_bubble(text: &'static str, width: f32, t: &Pink) -> impl IntoElement {
    let edge = t.user_bubble_edge();
    div().w_full().flex().justify_end().child(
        div()
            .min_w_0()
            .max_w(px(width * 0.8))
            .bg(t.user_bubble_bg())
            .when_some(edge, |el, edge| {
                el.shadow(vec![BoxShadow {
                    color: edge,
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(1.),
                    inset: true,
                }])
            })
            .rounded(px(radius::BUBBLE))
            .px(px(space::S16))
            .py(px(space::S10))
            .text_size(px(14.))
            .line_height(px(TRANSCRIPT_LINE_HEIGHT))
            .text_color(t.text)
            .child(text),
    )
}

/// Pink: assistant text is bare markdown on the canvas, 14/22.
fn reply(text: String) -> gpui::Div {
    div()
        .pt(px(space::LG))
        .text_size(px(14.))
        .line_height(px(TRANSCRIPT_LINE_HEIGHT))
        .child(text)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("thinking-loader-demo", cx, |_, cx| Demo::new(cx));
        let (run, beat, stopped, turn_started, setup_at, warned_at) = {
            let d = demo.read(cx);
            (
                d.run,
                d.current(),
                d.stopped,
                d.turn_started,
                d.ago(14),
                d.ago(192),
            )
        };
        const WIDTH: f32 = 560.;

        // The reply streams in while "Writing"; a stopped turn keeps what it had
        let streamed = match stopped.unwrap_or(beat) {
            Beat::Step(4) => Some(DEMO_REPLY[0].to_string()),
            Beat::Done => Some(DEMO_REPLY.concat()),
            _ => None,
        };
        let trailer = (beat != Beat::Done).then(|| {
            let loader = ThinkingLoader::new(ElementId::NamedInteger("reply".into(), run))
                .phases(demo_phases())
                .on_stop({
                    let demo = demo.clone();
                    move |_, _, cx| demo.update(cx, |d, cx| d.stop(cx))
                });
            let loader = match turn_started {
                Some(at) => loader.started_at(at),
                None => loader,
            };
            match beat {
                Beat::Send => loader.sending(true),
                Beat::Step(i) => loader.phase(i),
                Beat::Notice(i) => loader.phase(i).notice("Reconnecting, attempt 2 of 5"),
                Beat::Done => loader,
            }
        });

        let conversation = div()
            .flex()
            .flex_col()
            .child(user_bubble(DEMO_QUESTION, WIDTH, &t))
            .when_some(streamed, |el, text| {
                el.child(motion::fade_in(
                    ElementId::NamedInteger("reply-in".into(), run),
                    reply(text),
                ))
            })
            .children(trailer);

        // The lines Pink shows while a send is held, and a provider warning
        let states = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .type_style(TextStyle::Footnote)
                    .text_color(t.readable_muted(0.85))
                    .child("Other states"),
            )
            .child(ThinkingLoader::new("state-sending").sending(true))
            .child(
                ThinkingLoader::new("state-setup")
                    .setup("pnpm install --frozen-lockfile")
                    .started_at(setup_at),
            )
            .child(ThinkingLoader::new("state-queued").queued(true))
            .child(
                ThinkingLoader::new("state-warning")
                    .notice("Context window almost full. Older turns will be summarized")
                    .started_at(warned_at)
                    .on_stop(|_, _, _| {}),
            );

        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .gap(px(space::S32))
            .font_family(pink::SANS)
            .text_color(t.text)
            // Room for the reply, so the states below hold still
            .child(conversation.min_h(px(148.)))
            .child(pink::divider(&t))
            .child(states)
    })
}
