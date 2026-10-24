//! Approval Card – an agent stops and asks before it acts.
//!
//! PINK has one approval surface, and this is it. While the request is open,
//! the question panel takes the composer's place (Pink: composer.rs
//! `render_wizard`): the same r26 pill, a tracked uppercase header, the
//! question, the approval context as label-over-value rows ending in the
//! expiry row, numbered option rows (1–9 pick one), a free answer field and a
//! footer with the countdown, "Keep waiting" and Send. Above it sits the
//! passive transcript chip (Pink: transcript.rs `input_chip`), reading
//! "Awaiting your answer…" until the request is answered, then naming it.
//!
//! Picking an option shows it picked for 220 ms and sends it. Enter sends the
//! picked option; a typed answer wins over a pick and goes back as a Deny. When
//! the request carries an auto answer, the footer says what PINK will pick and
//! when; touching the panel, typing, or "Keep waiting" holds it.
//!
//! ```ignore
//! ApprovalCard::new("migrate", ApprovalRequest::new("Approve command", "Claude wants to run …")
//!     .fact("Command / input", "npm run db:migrate")
//!     .option(ApprovalOption::deny())
//!     .option(ApprovalOption::allow_once())
//!     .auto_answer(Decision::Deny, Duration::from_secs(45)))
//!     .on_resolve(|decision, _, _| println!("{decision:?}"))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyView, App, Div, ElementId, Entity, FocusHandle, FontWeight, KeyDownEvent, Role,
    SharedString, Subscription, Task, Toggled, Window, div, phi, prelude::*, px,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink, layout, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Approval Card";
pub const DESCRIPTION: &str = "An agent's request to act, asked in the composer's place – the action and its context, numbered choices you can pick with 1–9, a countdown PINK answers for you unless you keep waiting, and a quiet one-line record in the transcript once it is answered";

const ICON_QUESTION: &str = "icons/approval-card/chat-round-line.svg";

/// A single-select pick shows for this long before it is sent (Pink
/// composer.rs `AUTO_ADVANCE_MS`).
const AUTO_ADVANCE: Duration = Duration::from_millis(220);
/// The countdown's tick (Pink `ensure_countdown_ticking`).
const TICK: Duration = Duration::from_secs(1);
/// Space between the transcript chip and the panel in the composer slot.
const SLOT_GAP: f32 = 16.;

const PLACEHOLDER: &str = "Type your own answer, or pick an option above";
const PLACEHOLDER_PICKED: &str =
    "Type your own answer, or leave this blank to use the selected option";

type DecisionHandler = Rc<dyn Fn(Decision, &mut Window, &mut App)>;
type ReplyHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// What the agent is told (Pink `ApprovalDecision`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    AllowOnce,
    AllowForSession,
    Deny,
    /// Deny and stop the agent's turn.
    CancelTurn,
}

impl Decision {
    fn label(self) -> &'static str {
        match self {
            Decision::AllowOnce => "Allow once",
            Decision::AllowForSession => "Allow for session",
            Decision::Deny => "Deny",
            Decision::CancelTurn => "Cancel turn",
        }
    }
}

/// One answer the request offers (Pink `ApprovalChoice`: label, scope and
/// description, rendered as "scope · description" under the label).
#[derive(Clone, Debug)]
pub struct ApprovalOption {
    pub decision: Decision,
    pub label: SharedString,
    /// The grant's lifetime: "request", "session", "none"…
    pub scope: Option<SharedString>,
    pub description: Option<SharedString>,
}

impl ApprovalOption {
    pub fn new(decision: Decision, label: impl Into<SharedString>) -> Self {
        Self {
            decision,
            label: label.into(),
            scope: None,
            description: None,
        }
    }

    pub fn allow_once() -> Self {
        Self::new(Decision::AllowOnce, "Allow once")
            .scope("request")
            .description("Allow only this tool call.")
    }

    pub fn allow_for_session() -> Self {
        Self::new(Decision::AllowForSession, "Allow for session")
            .scope("session")
            .description("Allow this call and similar ones for the rest of this session.")
    }

    pub fn deny() -> Self {
        Self::new(Decision::Deny, "Deny")
            .scope("none")
            .description("Reject this tool call; the agent may continue.")
    }

    pub fn cancel_turn() -> Self {
        Self::new(Decision::CancelTurn, "Cancel turn")
            .scope("none")
            .description("Reject this tool call and stop the agent's turn.")
    }

    pub fn scope(mut self, scope: impl Into<SharedString>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// The line under the label (Pink: `"{scope} · {description}"`).
    fn detail(&self) -> Option<SharedString> {
        match (&self.scope, &self.description) {
            (Some(scope), Some(description)) => Some(format!("{scope} · {description}").into()),
            (None, Some(description)) => Some(description.clone()),
            (Some(scope), None) => Some(scope.clone()),
            (None, None) => None,
        }
    }
}

/// What the agent wants to do (Pink `UserInputQuestion` with its
/// `ApprovalRequest`).
#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    /// The kind of request: "Approve command", "Approve file change"…
    pub header: SharedString,
    /// The sentence naming the action.
    pub question: SharedString,
    /// Approval context, label over value (Pink `display_rows`).
    pub facts: Vec<(SharedString, SharedString)>,
    /// Clock time the request expires; `None` waits until answered.
    pub expires_at: Option<SharedString>,
    /// Empty means Deny, Allow once and Cancel turn.
    pub options: Vec<ApprovalOption>,
    /// PINK answers with this decision when nobody replies in time.
    pub auto_answer: Option<(Decision, Duration)>,
    /// Whether the free answer field shows.
    pub reply: bool,
}

impl ApprovalRequest {
    pub fn new(header: impl Into<SharedString>, question: impl Into<SharedString>) -> Self {
        Self {
            header: header.into(),
            question: question.into(),
            facts: Vec::new(),
            expires_at: None,
            options: Vec::new(),
            auto_answer: None,
            reply: true,
        }
    }

    pub fn fact(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.facts.push((label.into(), value.into()));
        self
    }

    /// The clock time the request expires, e.g. "14:32:05".
    pub fn expires_at(mut self, time: impl Into<SharedString>) -> Self {
        self.expires_at = Some(time.into());
        self
    }

    pub fn option(mut self, option: ApprovalOption) -> Self {
        self.options.push(option);
        self
    }

    /// PINK picks `decision` after `after` unless someone answers first.
    pub fn auto_answer(mut self, decision: Decision, after: Duration) -> Self {
        self.auto_answer = Some((decision, after));
        self
    }

    /// Shows or hides the free answer field (shown by default, as in Pink).
    pub fn reply(mut self, reply: bool) -> Self {
        self.reply = reply;
        self
    }

    fn resolved_options(&self) -> Vec<ApprovalOption> {
        if self.options.is_empty() {
            vec![
                ApprovalOption::deny(),
                ApprovalOption::allow_once(),
                ApprovalOption::cancel_turn(),
            ]
        } else {
            self.options.clone()
        }
    }

    /// The context rows plus the expiry row (Pink `approval_expiry_row`).
    fn context_rows(&self) -> Vec<(SharedString, SharedString)> {
        let mut rows = self.facts.clone();
        rows.push(match &self.expires_at {
            Some(time) => ("Approval expires".into(), time.clone()),
            None => (
                "Approval waits".into(),
                "Until you answer, the agent stops or its turn ends".into(),
            ),
        });
        rows
    }
}

/// How an answered request reads in the transcript chip (Pink: transcript.rs
/// `rows_for_entry`, `MessagePart::Input`): its header, plus " · no reply in
/// time, PINK picked …" when the auto answer fired.
fn record_value(header: &str, label: &str, by_pink: bool) -> String {
    if by_pink {
        format!("{header} · no reply in time, PINK picked {label}")
    } else {
        header.to_string()
    }
}

/// A bare digit picks option 1–9, unless a modifier is held or the answer
/// field has focus (Pink `wizard_number_shortcut`).
fn number_shortcut(key: &str, modified: bool, reply_focused: bool) -> Option<usize> {
    if modified || reply_focused {
        return None;
    }
    let digit = key.parse::<usize>().ok()?;
    (1..=9).contains(&digit).then(|| digit - 1)
}

struct CardState {
    focus: FocusHandle,
    reply: Entity<TextInput>,
    /// The option each row stands for, refreshed every render.
    decisions: Vec<Decision>,
    labels: Vec<SharedString>,
    picked: Option<usize>,
    decision: Option<Decision>,
    /// The auto answer fired.
    by_pink: bool,
    /// When PINK answers; cleared once held or answered.
    deadline: Option<Instant>,
    auto: Option<Decision>,
    on_resolve: Option<DecisionHandler>,
    on_reply: Option<ReplyHandler>,
    _advance: Option<Task<()>>,
    _tick: Option<Task<()>>,
    _reply_events: Subscription,
}

impl CardState {
    fn new(
        auto_answer: Option<(Decision, Duration)>,
        resolved: Option<(Decision, bool)>,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let reply = cx.new(|cx| TextInput::new(window, cx).placeholder(PLACEHOLDER));
        let reply_events = cx.subscribe_in(&reply, window, |this, _, event, window, cx| {
            match event {
                // The field's Enter is its own submit (Pink: Enter in the
                // answer input runs `wizard_advance`).
                InputEvent::Submit(_) => this.advance(window, cx),
                InputEvent::Change(_) => {
                    this.hold();
                    cx.notify();
                }
                _ => {}
            }
        });
        let (decision, by_pink) = match resolved {
            Some((d, by_pink)) => (Some(d), by_pink),
            None => (None, false),
        };
        let mut state = CardState {
            focus: cx.focus_handle(),
            reply,
            decisions: Vec::new(),
            labels: Vec::new(),
            picked: None,
            decision,
            by_pink,
            deadline: None,
            auto: None,
            on_resolve: None,
            on_reply: None,
            _advance: None,
            _tick: None,
            _reply_events: reply_events,
        };
        if let (None, Some((auto, after))) = (decision, auto_answer) {
            state.auto = Some(auto);
            state.deadline = Some(Instant::now() + after);
            state.start_ticking(window, cx);
        }
        state
    }

    fn left(&self) -> Option<Duration> {
        let deadline = self.deadline?;
        Some(deadline.saturating_duration_since(Instant::now()))
    }

    /// Ticks on the countdown's second boundaries, so the copy changes when
    /// the number does and the auto answer lands on time.
    fn start_ticking(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        self._tick = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                let Ok(Some(left)) = this.update(cx, |s, _| s.left()) else {
                    break;
                };
                let rem = Duration::from_millis((left.as_millis() % TICK.as_millis()) as u64);
                let wait = if rem.is_zero() { TICK } else { rem };
                cx.background_executor()
                    .timer(wait.min(left.max(Duration::from_millis(1))))
                    .await;
                let running = this
                    .update_in(cx, |s, window, cx| s.on_tick(window, cx))
                    .unwrap_or(false);
                if !running {
                    break;
                }
            }
        }));
    }

    fn on_tick(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> bool {
        if self.decision.is_some() || self.deadline.is_none() {
            self.deadline = None;
            return false;
        }
        if self.left().is_some_and(|left| left.is_zero()) {
            self.deadline = None;
            if let Some(auto) = self.auto {
                self.by_pink = true;
                self.resolve(auto, window, cx);
            }
            return false;
        }
        cx.notify();
        true
    }

    /// The user is answering: PINK must not answer for them (Pink
    /// `note_wizard_interaction` / `hold_auto_answer`).
    fn hold(&mut self) {
        self.deadline = None;
    }

    fn select(&mut self, ix: usize, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.decision.is_some() || ix >= self.decisions.len() {
            return;
        }
        self.hold();
        self.picked = Some(ix);
        self.reply.update(cx, |input, cx| {
            input.set_placeholder(PLACEHOLDER_PICKED, cx)
        });
        self._advance = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTO_ADVANCE).await;
            this.update_in(cx, |s, window, cx| s.advance(window, cx))
                .ok();
        }));
        cx.notify();
    }

    /// Send: a typed answer wins over a pick and goes back as a Deny (Pink
    /// `approval_decision` finds no option label in it).
    fn advance(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.decision.is_some() {
            return;
        }
        let typed = self.reply.read(cx).text();
        if !typed.trim().is_empty() {
            self.resolve(Decision::Deny, window, cx);
            if let Some(f) = self.on_reply.clone() {
                f(typed, window, cx);
            }
        } else if let Some(decision) = self.picked.and_then(|ix| self.decisions.get(ix).copied()) {
            self.resolve(decision, window, cx);
        }
    }

    fn resolve(&mut self, decision: Decision, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.decision.is_some() {
            return;
        }
        // The tick loop and a pending advance see this and stop on their own.
        self.decision = Some(decision);
        self.deadline = None;
        if let Some(f) = self.on_resolve.clone() {
            f(decision, window, cx);
        }
        cx.notify();
    }

    fn on_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.decision.is_some() {
            return;
        }
        // Any key in the panel means the user is answering.
        self.hold();
        let reply_focused = self.reply.read(cx).is_focused(window);
        let ks = &event.keystroke;
        let key = ks.key.as_str();
        if let Some(ix) = number_shortcut(key, ks.modifiers.modified(), reply_focused) {
            self.select(ix, window, cx);
            cx.stop_propagation();
        } else if key == "enter" {
            if !reply_focused {
                self.advance(window, cx);
                cx.stop_propagation();
            }
        } else if key == "escape" {
            cx.stop_propagation();
        }
        cx.notify();
    }

    /// The label of `decision` among this request's options.
    fn label_of(&self, decision: Decision) -> SharedString {
        self.decisions
            .iter()
            .position(|d| *d == decision)
            .and_then(|ix| self.labels.get(ix).cloned())
            .unwrap_or_else(|| decision.label().into())
    }
}

/// An agent's request for sign-off: the question panel in the composer's
/// place while it is open, and its one-line transcript chip.
#[derive(IntoElement)]
pub struct ApprovalCard {
    id: ElementId,
    request: ApprovalRequest,
    resolved: Option<(Decision, bool)>,
    autofocus: bool,
    on_resolve: Option<DecisionHandler>,
    on_reply: Option<ReplyHandler>,
}

impl ApprovalCard {
    pub fn new(id: impl Into<ElementId>, request: ApprovalRequest) -> Self {
        Self {
            id: id.into(),
            request,
            resolved: None,
            autofocus: false,
            on_resolve: None,
            on_reply: None,
        }
    }

    /// Starts as the record of an earlier answer.
    pub fn resolved(mut self, decision: Decision) -> Self {
        self.resolved = Some((decision, false));
        self
    }

    /// Starts as the record of a request PINK answered after nobody replied.
    pub fn auto_resolved(mut self, decision: Decision) -> Self {
        self.resolved = Some((decision, true));
        self
    }

    /// Takes keyboard focus on first render, so 1–9 and Enter work right away.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    pub fn on_resolve(mut self, f: impl Fn(Decision, &mut Window, &mut App) + 'static) -> Self {
        self.on_resolve = Some(Rc::new(f));
        self
    }

    /// A typed answer, sent instead of a choice (it resolves as a Deny).
    pub fn on_reply(mut self, f: impl Fn(SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_reply = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ApprovalCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let req = self.request;
        let options = req.resolved_options();
        let (auto_answer, resolved, autofocus) = (req.auto_answer, self.resolved, self.autofocus);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| {
                let state = CardState::new(auto_answer, resolved, window, cx);
                if autofocus && state.decision.is_none() {
                    window.focus(&state.focus, cx);
                }
                state
            },
        );
        let (on_resolve, on_reply) = (self.on_resolve.clone(), self.on_reply.clone());
        let (decisions, labels): (Vec<_>, Vec<_>) = options
            .iter()
            .map(|o| (o.decision, o.label.clone()))
            .unzip();
        state.update(cx, |s, cx| {
            s.on_resolve = on_resolve;
            s.on_reply = on_reply;
            s.decisions = decisions;
            s.labels = labels;
            // Pink composer input: `placeholder` text, accent caret.
            s.reply.update(cx, |input, _| {
                input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
            });
        });

        let s = state.read(cx);
        let chip_value: SharedString = match s.decision {
            Some(decision) => record_value(&req.header, &s.label_of(decision), s.by_pink).into(),
            None => "Awaiting your answer…".into(),
        };
        let open = s.decision.is_none();

        div()
            .w_full()
            .max_w(px(layout::COMPOSER_MAX_WIDTH))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            // Pink's shell root: 14 px Geist on GPUI's default line height.
            .text_size(px(14.))
            .line_height(phi())
            .text_color(t.text)
            .child(input_chip(chip_value, &t))
            .when(open, |d| {
                d.child(motion::fade_quick(
                    eid(format!("{key}-panel-in")),
                    div()
                        .pt(px(SLOT_GAP))
                        .child(question_panel(&req, &options, &key, &state, &t, cx)),
                ))
            })
    }
}

/// The transcript's record of the request (Pink: transcript.rs `input_chip`):
/// a 34 px neutral row that never recolors on resolution.
fn input_chip(value: SharedString, t: &Pink) -> Div {
    div().py(px(4.)).w_full().child(
        div()
            .h(px(34.))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .overflow_hidden()
            .rounded(px(radius::LG))
            .border_1()
            .border_color(t.hairline(0.08))
            .bg(t.ink(0.045))
            .px(px(8.))
            .text_size(px(12.))
            .child(
                div()
                    .flex_none()
                    .size(px(20.))
                    .rounded(px(radius::SM))
                    .bg(t.ink(0.09))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        gpui::svg()
                            .path(ICON_QUESTION)
                            .flex_none()
                            .size(px(12.))
                            .text_color(t.content_muted),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.content_muted)
                    .child("Question"),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_color(t.text.opacity(0.9))
                    .child(value),
            ),
    )
}

/// The pill chrome the composer and the question panel share (Pink:
/// composer.rs `composer_surface`, on the frosted composer plate).
fn composer_surface(t: &Pink) -> Div {
    div()
        .rounded(px(radius::COMPOSER))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
        .shadow(pink::shadow_lg())
}

/// One numbered option (Pink: composer.rs `render_wizard` option rows).
fn option_row(
    ix: usize,
    option: &ApprovalOption,
    picked: bool,
    key: &str,
    state: &Entity<CardState>,
    t: &Pink,
) -> impl IntoElement {
    let hover_key = format!("{key}-option-{ix}");
    let accent = t.accent;
    let click_state = state.clone();
    div()
        .id(eid(hover_key.clone()))
        .role(Role::RadioButton)
        .aria_label(option.label.clone())
        .aria_toggled(if picked {
            Toggled::True
        } else {
            Toggled::False
        })
        .tab_index(0)
        .focus_visible(move |style| style.border_color(accent))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(12.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(12.))
        .border_1()
        .border_color(if picked {
            t.ink(0.16)
        } else {
            gpui::transparent_black()
        })
        .bg(if picked {
            t.ink(0.09)
        } else {
            motion::hover_blend(&hover_key, t.ink(0.025), t.ink(0.06))
        })
        .on_hover(motion::hover_listener(hover_key))
        .cursor_pointer()
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, window, cx| {
            click_state.update(cx, |s, cx| s.select(ix, window, cx));
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if picked { t.text } else { t.text.opacity(0.9) })
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(option.label.clone())
                .when_some(option.detail(), |el, detail| {
                    el.child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(t.readable_muted(0.65))
                            .child(detail),
                    )
                }),
        )
        .when(ix < 9, |el| {
            // Number kbd chip: `size-[22px] rounded-md text-[11px]`.
            el.child(
                div()
                    .flex_none()
                    .size(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .bg(if picked { t.ink(0.16) } else { t.ink(0.05) })
                    .text_size(px(11.))
                    .text_color(if picked {
                        t.text
                    } else {
                        t.readable_muted(0.6)
                    })
                    .child(format!("{}", ix + 1)),
            )
        })
}

/// The question panel in the composer's slot (Pink: composer.rs
/// `render_wizard` for an approval question).
fn question_panel(
    req: &ApprovalRequest,
    options: &[ApprovalOption],
    key: &str,
    state: &Entity<CardState>,
    t: &Pink,
    cx: &App,
) -> impl IntoElement {
    let s = state.read(cx);
    let reply = s.reply.clone();
    let typed_empty = reply.read(cx).text().trim().is_empty();
    // Selection reads on the row only while no typed answer overrides it.
    let picked = s.picked.filter(|_| typed_empty);
    let can_advance = s.picked.is_some() || !typed_empty;
    let countdown = s.left().zip(s.auto).map(|(left, auto)| {
        let seconds = left.as_millis().div_ceil(1000);
        format!(
            "PINK picks {} in {seconds}s unless you answer",
            s.label_of(auto)
        )
    });

    let header = div().flex().flex_row().items_center().gap(px(10.)).child(
        div()
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.readable_muted(0.6))
            .child(pink::tracked_upper(&req.header)),
    );

    let question = div()
        .id(eid(format!("{key}-question")))
        .max_h(px(160.))
        .overflow_y_scroll()
        .mt(px(6.))
        .text_size(px(15.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text)
        .child(req.question.clone());

    let context = div()
        .id(eid(format!("{key}-context")))
        .mt(px(8.))
        .max_h(px(220.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(6.))
        .children(req.context_rows().into_iter().map(|(label, value)| {
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.readable_muted(0.65))
                        .child(label),
                )
                .child(div().text_size(px(12.)).text_color(t.text).child(value))
        }));

    let option_rows = div()
        .id(eid(format!("{key}-options")))
        .max_h(px(220.))
        .overflow_y_scroll()
        .mt(px(12.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .children(
            options
                .iter()
                .enumerate()
                .map(|(ix, option)| option_row(ix, option, picked == Some(ix), key, state, t)),
        );

    // The free answer: a composer surface nested in the panel.
    let answer = req.reply.then(|| {
        div().mt(px(12.)).pb(px(4.)).px(px(4.)).child(
            composer_surface(t)
                .id(eid(format!("{key}-answer")))
                .w_full()
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .min_w_0()
                        .px(px(8.))
                        .py(px(12.))
                        // Pink composer input: `text-[14px] leading-relaxed`.
                        .text_size(px(14.))
                        .line_height(px(22.75))
                        .text_color(t.text)
                        .child(div().flex_1().min_w_0().child(reply.clone())),
                ),
        )
    });

    let hold_state = state.clone();
    let keep_state = state.clone();
    let send_state = state.clone();
    let footer = div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .pb(px(16.))
        .pt(px(4.))
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(8.))
                .when_some(countdown, |el, copy| {
                    el.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(t.readable_muted(0.7))
                            .child(copy),
                    )
                    .child(
                        pink::button(
                            format!("{key}-keep-waiting"),
                            ButtonKind::Ghost,
                            ControlSize::Lg,
                            t,
                        )
                        .aria_label("Keep waiting")
                        .child("Keep waiting")
                        .on_click(move |_, _, cx| {
                            keep_state.update(cx, |s, cx| {
                                s.hold();
                                cx.notify();
                            })
                        }),
                    )
                }),
        )
        .child(
            pink::button(
                format!("{key}-send"),
                ButtonKind::Primary,
                ControlSize::Lg,
                t,
            )
            .aria_label("Send")
            .px(px(16.))
            .when(!can_advance, |el| el.opacity(0.4))
            .child("Send")
            .on_click(move |_, window, cx| send_state.update(cx, |s, cx| s.advance(window, cx))),
        );

    let key_state = state.clone();
    composer_surface(t)
        .id(eid(format!("{key}-panel")))
        .track_focus(&s.focus)
        .capture_any_mouse_down(move |_, _, cx| {
            hold_state.update(cx, |s, cx| {
                if s.deadline.is_some() {
                    s.hold();
                    cx.notify();
                }
            })
        })
        .on_key_down(
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                key_state.update(cx, |s, cx| s.on_key(event, window, cx));
            },
        )
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(16.))
                .pt(px(16.))
                .flex()
                .flex_col()
                .child(header)
                .child(question)
                .child(context)
                .child(option_rows)
                .children(answer),
        )
        .child(footer)
}

// ---------------------------------------------------------------------------
// Story: a release agent asking to touch production
// ---------------------------------------------------------------------------

struct Queue {
    next_visible: bool,
    _reveal: Option<Task<()>>,
}

fn migration() -> ApprovalRequest {
    ApprovalRequest::new(
        "Approve command",
        "Claude wants to run the invoice tax migration on prod-eu-1. Target: lena-mbp.",
    )
    .fact("Command / input", "npm run db:migrate -- --env production")
    .fact("Working directory", "~/code/billing")
    .fact(
        "Reason",
        "Adds tax_region to invoices and backfills about 1.2M rows",
    )
    .option(ApprovalOption::deny().description("Reject this tool call; Claude may continue."))
    .option(ApprovalOption::allow_once())
    .option(
        ApprovalOption::cancel_turn().description("Reject this tool call and stop Claude's turn."),
    )
    .auto_answer(Decision::Deny, Duration::from_secs(45))
}

fn key_rotation() -> ApprovalRequest {
    ApprovalRequest::new(
        "Approve file change",
        "Codex wants to modify files. scripts/rotate-keys.sh Target: lena-mbp.",
    )
    .fact("File changes", "scripts/rotate-keys.sh (+12 −3)")
    .fact("Working directory", "~/code/platform")
    .fact("Reason", "Rotate the staging API keys before the release")
    .expires_at("14:32:05")
    .option(ApprovalOption::allow_once().description("Approve only these file changes."))
    .option(
        ApprovalOption::allow_for_session()
            .description("Approve these changes and future changes to the same files."),
    )
    .option(ApprovalOption::deny().description("Reject this action; the agent may continue."))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let queue = window.use_keyed_state("approval-card-queue", cx, |_, _| Queue {
            next_visible: false,
            _reveal: None,
        });
        let next_visible = queue.read(cx).next_visible;
        let reveal = queue.clone();
        div()
            .w(px(680.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                ApprovalCard::new(
                    "approval-earlier",
                    ApprovalRequest::new(
                        "Approve network access",
                        "Claude wants to reach registry.npmjs.org.",
                    ),
                )
                .resolved(Decision::AllowOnce),
            )
            .child(
                ApprovalCard::new(
                    "approval-unattended",
                    ApprovalRequest::new(
                        "Approve command",
                        "Claude wants to run git push --force.",
                    ),
                )
                .auto_resolved(Decision::Deny),
            )
            .child(
                ApprovalCard::new("approval-migration", migration())
                    .autofocus(true)
                    .on_resolve(move |_, _, cx| {
                        reveal.update(cx, |q, cx| {
                            q._reveal = Some(cx.spawn(async move |this, cx| {
                                cx.background_executor()
                                    .timer(Duration::from_millis(600))
                                    .await;
                                this.update(cx, |q, cx| {
                                    q.next_visible = true;
                                    cx.notify();
                                })
                                .ok();
                            }));
                        })
                    }),
            )
            .when(next_visible, |d| {
                d.child(motion::fade_quick(
                    "approval-next-in",
                    div().child(ApprovalCard::new("approval-keys", key_rotation())),
                ))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_pick_options_only_from_the_panel() {
        assert_eq!(number_shortcut("1", false, false), Some(0));
        assert_eq!(number_shortcut("9", false, false), Some(8));
        assert_eq!(number_shortcut("0", false, false), None);
        assert_eq!(number_shortcut("3", true, false), None);
        assert_eq!(number_shortcut("3", false, true), None);
    }

    #[test]
    fn records_name_the_request_and_who_answered() {
        assert_eq!(
            record_value("Approve command", "Allow once", false),
            "Approve command"
        );
        assert_eq!(
            record_value("Approve command", "Deny", true),
            "Approve command · no reply in time, PINK picked Deny"
        );
    }

    #[test]
    fn context_ends_with_the_expiry_row() {
        let waits = ApprovalRequest::new("Approve command", "…").fact("Command / input", "ls");
        assert_eq!(waits.context_rows().last().unwrap().0, "Approval waits");
        let expires = waits.expires_at("14:32:05");
        assert_eq!(
            expires.context_rows().last().unwrap(),
            &("Approval expires".into(), "14:32:05".into())
        );
    }
}
