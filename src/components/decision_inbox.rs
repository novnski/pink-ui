//! Decision Inbox – the decisions waiting on you, answered one at a time.
//!
//! PINK has no inbox card of its own; this puts three of its pieces together.
//! The list sits under Activity's section headings (Pink: activity_center.rs
//! `activity_section_heading`): "Needs you" with its count, then "Answered".
//! The decision you are on opens in the question panel (Pink: composer.rs
//! `render_wizard`): the r26 composer pill, a tracked uppercase header with a
//! "1/3" counter, the question, two numbered option rows and a footer with
//! "Answer later" and Send. Every other decision is a one-line card (Pink:
//! transcript.rs `async_question_card`): an accent edge and "Open · click to
//! answer" while it waits, a hairline and "Answered: …, click to change"
//! once it has settled.
//!
//! Picking an option shows it picked for 220 ms, then sends it and opens the
//! next decision (Pink's single-select auto-advance). 1 and 2 pick from the
//! keyboard and Enter sends a pick. "Answer later" puts the decision back in
//! the list; clicking a card opens it in the panel, so an answer can change.
//!
//! ```ignore
//! DecisionInbox::new("inbox", vec![
//!     Decision::new("renewal", "Renew Figma for the design team?", "Renew", "Change seats")
//!         .header("Renewal")
//!         .detail("12 seats for $1,800 per year, renews in 5 days"),
//! ])
//! .on_decide(|decision, answer, _, _| println!("{}: {answer:?}", decision.id))
//! ```

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};
use web_time::Duration;

use gpui::{
    AnyView, App, Context, Div, ElementId, Entity, FocusHandle, FontWeight, KeyDownEvent, Role,
    SharedString, Stateful, Task, Toggled, Window, div, phi, prelude::*, px, svg,
};

use crate::{
    components::empty_state::{NoteStyle, note},
    motion::{self, TOOL_ROW_REVEAL, TRANSCRIPT_IN, TRANSCRIPT_IN_RISE},
    pink::{self, ButtonKind, ControlSize, Pink, layout, radius, space},
    story,
};

pub const NAME: &str = "Decision Inbox";
pub const DESCRIPTION: &str = "Decisions waiting on you under a Needs you heading: the current one opens in the question panel with two numbered answers, the rest wait as one-line cards, and answered ones settle into a record you can click to change";

const ICON_QUESTION: &str = "icons/decision-inbox/chat-round-line.svg";

/// A pick shows for this long before it is sent (Pink composer.rs
/// `AUTO_ADVANCE_MS`).
const AUTO_ADVANCE: Duration = Duration::from_millis(220);
/// An Activity heading row (Pink spaces.rs `SIDEBAR_DISCLOSURE_HEADER_HEIGHT`).
const HEADING_H: f32 = 28.;
/// Above every Activity section but the first (Pink spaces.rs
/// `SIDEBAR_SECTION_GAP`).
const SECTION_GAP: f32 = 12.;
/// The transcript card's row (Pink transcript.rs `async_question_card`).
const CARD_H: f32 = 34.;

/// Which of the two options answered a decision: option 1 or option 2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Primary,
    Secondary,
}

impl Answer {
    const ALL: [Answer; 2] = [Answer::Primary, Answer::Secondary];

    fn index(self) -> usize {
        match self {
            Answer::Primary => 0,
            Answer::Secondary => 1,
        }
    }
}

/// One open question with its two answers.
#[derive(Clone, Debug)]
pub struct Decision {
    pub id: SharedString,
    /// The panel's tracked uppercase header ("Renewal"); "Decision" unless set.
    pub header: SharedString,
    pub title: SharedString,
    /// Short context under the question.
    pub detail: Option<SharedString>,
    /// Option 1.
    pub primary: SharedString,
    /// Option 2.
    pub secondary: SharedString,
    /// One line under each option, in option order.
    pub descriptions: [Option<SharedString>; 2],
    /// Starts put off: a waiting card until it is opened.
    pub collapsed: bool,
    /// Starts settled with this answer.
    pub answered: Option<Answer>,
}

impl Decision {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        primary: impl Into<SharedString>,
        secondary: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            header: "Decision".into(),
            title: title.into(),
            detail: None,
            primary: primary.into(),
            secondary: secondary.into(),
            descriptions: [None, None],
            collapsed: false,
            answered: None,
        }
    }

    pub fn header(mut self, header: impl Into<SharedString>) -> Self {
        self.header = header.into();
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// A line under one option, saying what it does.
    pub fn describe(mut self, answer: Answer, description: impl Into<SharedString>) -> Self {
        self.descriptions[answer.index()] = Some(description.into());
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    pub fn answered(mut self, answer: Answer) -> Self {
        self.answered = Some(answer);
        self
    }

    fn label(&self, answer: Answer) -> SharedString {
        match answer {
            Answer::Primary => self.primary.clone(),
            Answer::Secondary => self.secondary.clone(),
        }
    }
}

type DecideHandler = Rc<dyn Fn(&Decision, Answer, &mut Window, &mut App)>;
/// An answer to report once the state entity is released.
type Decided = (DecideHandler, Decision, Answer);

fn eid(id: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(id.into()))
}

fn report(decided: Option<Decided>, window: &mut Window, cx: &mut App) {
    if let Some((f, decision, answer)) = decided {
        f(&decision, answer, window, cx);
    }
}

struct InboxState {
    decisions: Vec<Decision>,
    /// Decisions whose `collapsed` / `answered` seeds were applied.
    seen: HashSet<SharedString>,
    /// Every answer given, oldest first (Pink keeps them on the card).
    answers: HashMap<SharedString, Vec<Answer>>,
    /// Put off with "Answer later".
    later: HashSet<SharedString>,
    /// Opened from its card; otherwise the panel holds the first open one.
    opened: Option<SharedString>,
    /// The pick showing before it is sent.
    picked: Option<(SharedString, Answer)>,
    focus: FocusHandle,
    on_decide: Option<DecideHandler>,
    _advance: Option<Task<()>>,
}

impl InboxState {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            decisions: Vec::new(),
            seen: HashSet::new(),
            answers: HashMap::new(),
            later: HashSet::new(),
            opened: None,
            picked: None,
            focus: cx.focus_handle(),
            on_decide: None,
            _advance: None,
        }
    }

    /// Takes this frame's decisions, seeding the ones not seen before.
    fn sync(&mut self, decisions: Vec<Decision>, on_decide: Option<DecideHandler>) {
        for d in &decisions {
            if !self.seen.insert(d.id.clone()) {
                continue;
            }
            if let Some(answer) = d.answered {
                self.answers.entry(d.id.clone()).or_default().push(answer);
            }
            if d.collapsed {
                self.later.insert(d.id.clone());
            }
        }
        self.decisions = decisions;
        self.on_decide = on_decide;
    }

    fn answers_of(&self, id: &SharedString) -> &[Answer] {
        self.answers.get(id).map_or(&[], Vec::as_slice)
    }

    /// The decision in the panel: the one opened from its card, else the first
    /// open one that was not put off.
    fn current(&self) -> Option<&Decision> {
        self.opened
            .as_ref()
            .and_then(|id| self.decisions.iter().find(|d| &d.id == id))
            .or_else(|| {
                self.decisions
                    .iter()
                    .find(|d| self.answers_of(&d.id).is_empty() && !self.later.contains(&d.id))
            })
    }

    fn select(&mut self, answer: Answer, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.current().map(|d| d.id.clone()) else {
            return;
        };
        self.picked = Some((id, answer));
        // Pink `schedule_auto_advance`: a single-select pick sends itself.
        self._advance = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTO_ADVANCE).await;
            if let Ok(decided) = this.update(cx, |s, cx| s.advance(cx)) {
                cx.update(|window, cx| report(decided, window, cx)).ok();
            }
        }));
        cx.notify();
    }

    /// Sends the pick: the decision settles and the panel moves on.
    fn advance(&mut self, cx: &mut Context<Self>) -> Option<Decided> {
        let decision = self.current()?.clone();
        let (id, answer) = self.picked.take().filter(|(id, _)| *id == decision.id)?;
        self.answers.entry(id.clone()).or_default().push(answer);
        self.later.remove(&id);
        self.opened = None;
        cx.notify();
        self.on_decide.clone().map(|f| (f, decision, answer))
    }

    fn answer_later(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.current().map(|d| d.id.clone()) else {
            return;
        };
        if self.answers_of(&id).is_empty() {
            self.later.insert(id);
        }
        self.opened = None;
        self.picked = None;
        self._advance = None;
        cx.notify();
    }

    fn open(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.opened = Some(id);
        self.picked = None;
        self._advance = None;
        // So 1, 2 and Enter answer right away.
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Pink `on_wizard_key`: a bare digit picks, Enter sends, Escape stays
    /// inside the panel.
    fn on_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Decided> {
        let ks = &event.keystroke;
        if ks.modifiers.modified() {
            return None;
        }
        match ks.key.as_str() {
            "1" | "2" => {
                let answer = if ks.key == "1" {
                    Answer::Primary
                } else {
                    Answer::Secondary
                };
                self.select(answer, window, cx);
                cx.stop_propagation();
                None
            }
            "enter" => {
                cx.stop_propagation();
                self.advance(cx)
            }
            "escape" => {
                cx.stop_propagation();
                None
            }
            _ => None,
        }
    }
}

/// Open decisions with two answers each: the current one in the question
/// panel, the others as one-line cards under Activity headings.
#[derive(IntoElement)]
pub struct DecisionInbox {
    id: ElementId,
    decisions: Vec<Decision>,
    on_decide: Option<DecideHandler>,
}

impl DecisionInbox {
    pub fn new(id: impl Into<ElementId>, decisions: Vec<Decision>) -> Self {
        Self {
            id: id.into(),
            decisions,
            on_decide: None,
        }
    }

    /// Called with the decision and the chosen answer, again whenever an
    /// answer is changed.
    pub fn on_decide(
        mut self,
        handler: impl Fn(&Decision, Answer, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for DecisionInbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| InboxState::new(cx),
        );
        let (decisions, on_decide) = (self.decisions, self.on_decide);
        state.update(cx, |s, _| s.sync(decisions, on_decide));

        let s = state.read(cx);
        let current = s.current().cloned();
        let current_id = current.as_ref().map(|d| d.id.clone());
        let is_current = |d: &Decision| current_id.as_ref() == Some(&d.id);
        // "Needs you" lists what is open plus the decision in the panel.
        let needs_you: Vec<&Decision> = s
            .decisions
            .iter()
            .filter(|d| is_current(d) || s.answers_of(&d.id).is_empty())
            .collect();
        let waiting: Vec<Decision> = needs_you
            .iter()
            .filter(|d| !is_current(d))
            .map(|d| (*d).clone())
            .collect();
        let settled: Vec<(Decision, Vec<Answer>)> = s
            .decisions
            .iter()
            .filter(|d| !is_current(d) && !s.answers_of(&d.id).is_empty())
            .map(|d| (d.clone(), s.answers_of(&d.id).to_vec()))
            .collect();
        let panel = current.map(|d| {
            let position = needs_you.iter().position(|x| x.id == d.id).unwrap_or(0);
            let picked = s
                .picked
                .as_ref()
                .filter(|(id, _)| *id == d.id)
                .map(|(_, answer)| *answer);
            let answers = s.answers_of(&d.id).to_vec();
            let paging = (position + 1, needs_you.len());
            (d, paging, answers, picked)
        });
        let needs_you = needs_you.len();
        let focus = s.focus.clone();

        let mut root = div()
            .w_full()
            .max_w(px(layout::COMPOSER_MAX_WIDTH))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            // Pink's shell root: 14 px Geist on GPUI's default line height.
            .text_size(px(14.))
            .line_height(phi())
            .text_color(t.text);

        if needs_you == 0 {
            root = root.child(motion::fade_quick(
                eid(format!("{key}-empty")),
                empty_state(&t),
            ));
        } else {
            root = root.child(section_heading(
                format!("{key}-needs-you"),
                "Needs you",
                needs_you,
                true,
                &t,
            ));
        }
        if let Some((d, paging, answers, picked)) = panel {
            let panel = question_panel(&key, &d, paging, &answers, picked, &state, &focus, &t);
            // Pink mounts the panel with `fade_quick("composer-wizard")`.
            root = root.child(motion::fade_quick(
                eid(format!("{key}-panel-in")),
                div().pt(px(4.)).pb(px(8.)).child(panel),
            ));
        }
        root = root.children(
            waiting
                .iter()
                .map(|d| decision_card(&key, d, &[], &state, &t)),
        );
        if !settled.is_empty() {
            root = root
                .child(section_heading(
                    format!("{key}-answered"),
                    "Answered",
                    settled.len(),
                    false,
                    &t,
                ))
                .children(
                    settled
                        .iter()
                        .map(|(d, answers)| decision_card(&key, d, answers, &state, &t)),
                );
        }
        root
    }
}

/// An Activity section heading (Pink: activity_center.rs
/// `activity_section_heading`): a heading, not a disclosure. The name in 11
/// semibold, the count beside it; sections after the first keep the sidebar's
/// section gap above.
fn section_heading(id: String, label: &'static str, count: usize, first: bool, t: &Pink) -> Div {
    let items = if count == 1 { "decision" } else { "decisions" };
    div()
        .w_full()
        .flex_none()
        .flex()
        .flex_col()
        .pt(px(if first { 0. } else { SECTION_GAP }))
        .child(
            div()
                .id(eid(id))
                .role(Role::Heading)
                .aria_label(format!("{label}, {count} {items}"))
                .h(px(HEADING_H))
                .px(px(space::SM))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.readable_faint(0.9))
                        .child(label),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .text_color(t.readable_faint(0.7))
                        .child(count.to_string()),
                ),
        )
}

/// Nothing open (Pink: shell.rs Activity's empty list): a centred title and
/// the next step under it, the `pink-empty-state` note.
fn empty_state(t: &Pink) -> Div {
    note(
        &NoteStyle {
            title: t.readable_muted(0.9),
            hint: t.readable_faint(0.9),
        },
        "Nothing needs attention",
        "New decisions appear here.",
    )
}

/// A decision outside the panel (Pink: transcript.rs `async_question_card`):
/// a 34 px r10 row that is always answerable, so it reads as a control. An
/// open one has an accent edge and accent status; a settled one a hairline
/// and the answer in `content_muted`. It arrives like a transcript row.
fn decision_card(
    key: &str,
    d: &Decision,
    answers: &[Answer],
    state: &Entity<InboxState>,
    t: &Pink,
) -> impl IntoElement {
    let answered = !answers.is_empty();
    let status: SharedString = match answers {
        [] => "Open, click to answer".into(),
        [only] => format!("Answered: {}, click to change", d.label(*only)).into(),
        [.., last] => format!(
            "Answered {} times, latest: {}, click to change",
            answers.len(),
            d.label(*last)
        )
        .into(),
    };
    let ring = t.focus_ring();
    let hover = t.ink(0.07);
    let (state, id) = (state.clone(), d.id.clone());
    let card = div().py(px(4.)).w_full().child(
        div()
            .id(eid(format!("{key}-card-{}", d.id)))
            .role(Role::Button)
            .aria_label(format!("{}, {status}", d.title))
            .tab_index(0)
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .on_key_down(pink::swallow_activation_key)
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                state.update(cx, |s, cx| s.open(id.clone(), window, cx));
            })
            .h(px(CARD_H))
            .w_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .overflow_hidden()
            .rounded(px(radius::LG))
            .border_1()
            .border_color(if answered {
                t.hairline(0.08)
            } else {
                t.accent.opacity(0.35)
            })
            .bg(t.ink(0.045))
            .hover(move |style| style.bg(hover))
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
                        svg()
                            .path(ICON_QUESTION)
                            .flex_none()
                            .size(px(12.))
                            .text_color(t.content_muted),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .max_w(px(320.))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text.opacity(0.85))
                    .child(d.title.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_color(if answered { t.content_muted } else { t.accent })
                    .child(status),
            ),
    );
    // Pink's transcript rows arrive over `TOOL_ROW_REVEAL`, rising 4 px.
    motion::rise_in(
        eid(format!("{key}-card-in-{}-{}", d.id, answers.len())),
        TOOL_ROW_REVEAL,
        4.,
        card,
    )
}

/// The pill chrome the composer and the question panel share (Pink:
/// composer.rs `composer_surface`). Here the panel sits in the list's flow
/// rather than floating over a transcript, so it goes without the opaque
/// treatment's `shadow_lg`, as Pink's frosted pill does.
fn composer_surface(t: &Pink) -> Div {
    div()
        .rounded(px(radius::COMPOSER_PILL))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
}

/// One numbered option (Pink: composer.rs `render_wizard` option rows).
fn option_row(
    key: &str,
    d: &Decision,
    answer: Answer,
    picked: bool,
    state: &Entity<InboxState>,
    t: &Pink,
) -> impl IntoElement {
    let ix = answer.index();
    let hover_key = format!("{key}-option-{}-{ix}", d.id);
    let accent = t.accent;
    let state = state.clone();
    let label = d.label(answer);
    div()
        .id(eid(hover_key.clone()))
        .role(Role::RadioButton)
        .aria_label(label.clone())
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
        // The pick rides GPUI's key-up keyboard click.
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, window, cx| {
            state.update(cx, |s, cx| s.select(answer, window, cx));
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
                .child(label)
                .when_some(d.descriptions[ix].clone(), |el, description| {
                    el.child(
                        div()
                            .text_size(px(11.5))
                            .font_weight(FontWeight::NORMAL)
                            .text_color(t.readable_muted(0.65))
                            .child(description),
                    )
                }),
        )
        .child(
            // Number kbd chip: `size-[22px] rounded-md text-[11px]`.
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
}

/// The current decision in the question panel (Pink: composer.rs
/// `render_wizard`): header and counter, the question, its detail, the two
/// options, then "Answer later" and Send. `paging` is the decision's place
/// among the ones that need you.
#[allow(clippy::too_many_arguments)]
fn question_panel(
    key: &str,
    d: &Decision,
    paging: (usize, usize),
    answers: &[Answer],
    picked: Option<Answer>,
    state: &Entity<InboxState>,
    focus: &FocusHandle,
    t: &Pink,
) -> Stateful<Div> {
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.readable_muted(0.6))
                .child(pink::tracked_upper(&d.header)),
        )
        .when(paging.1 > 1, |el| {
            // Pink `Wizard::counter`, shown while paged.
            el.child(
                div()
                    .h(px(20.))
                    .px(px(6.))
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .bg(t.ink(0.06))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.readable_muted(0.6))
                    .child(format!("{}/{}", paging.0, paging.1)),
            )
        });

    let question = div()
        .id(eid(format!("{key}-question")))
        .max_h(px(160.))
        .overflow_y_scroll()
        .mt(px(6.))
        .text_size(px(15.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text)
        .child(d.title.clone());

    // The note under a question (Pink's multi-select hint line).
    let detail = d.detail.clone().map(|detail| {
        div()
            .mt(px(4.))
            .text_size(px(12.))
            .text_color(t.readable_muted(0.65))
            .child(detail)
    });

    let options = div()
        .id(eid(format!("{key}-options")))
        .max_h(px(220.))
        .overflow_y_scroll()
        .mt(px(12.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .children(
            Answer::ALL.map(|answer| option_row(key, d, answer, picked == Some(answer), state, t)),
        );

    // A settled decision reopened from its card (Pink's async note).
    let note = answers
        .last()
        .map(|last| format!("You answered {}. Send again to change it.", d.label(*last)));
    let later_state = state.clone();
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
                .when_some(note, |el, note| {
                    el.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(12.))
                            .text_color(t.readable_muted(0.7))
                            .child(note),
                    )
                })
                .child(
                    pink::button(
                        format!("{key}-later"),
                        ButtonKind::Ghost,
                        ControlSize::Lg,
                        t,
                    )
                    .aria_label("Answer later")
                    .child("Answer later")
                    .on_click(move |_, _, cx| {
                        later_state.update(cx, |s, cx| s.answer_later(cx));
                    }),
                ),
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
            .when(picked.is_none(), |el| el.opacity(0.4))
            .child("Send")
            .on_click(move |_, window, cx| {
                let decided = send_state.update(cx, |s, cx| s.advance(cx));
                report(decided, window, cx);
            }),
        );

    // Each new decision swaps in like transcript content.
    let body = motion::rise_in(
        eid(format!("{key}-body-{}-{}", d.id, answers.len())),
        TRANSCRIPT_IN,
        TRANSCRIPT_IN_RISE,
        div()
            .px(px(16.))
            .pt(px(16.))
            .flex()
            .flex_col()
            .child(header)
            .child(question)
            .children(detail)
            .child(options),
    );

    let key_state = state.clone();
    composer_surface(t)
        .id(eid(format!("{key}-panel")))
        .track_focus(focus)
        .on_key_down(
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let decided = key_state.update(cx, |s, cx| s.on_key(event, window, cx));
                report(decided, window, cx);
            },
        )
        .flex()
        .flex_col()
        .child(body)
        .child(footer)
}

/// The demo's decisions: one in the panel, two waiting (one put off) and one
/// already answered.
pub fn demo_decisions() -> Vec<Decision> {
    vec![
        Decision::new(
            "rollout",
            "Roll out the new onboarding to everyone?",
            "Roll out",
            "Keep testing",
        )
        .header("Rollout")
        .detail("The test won with +8% activation over two weeks.")
        .describe(
            Answer::Primary,
            "Every new sign-up gets the new flow today.",
        )
        .describe(Answer::Secondary, "Run it at 50% for one more week."),
        Decision::new(
            "renewal",
            "Renew Figma for the design team?",
            "Renew",
            "Change seats",
        )
        .header("Renewal")
        .detail("12 seats for $1,800 per year, renews in 5 days"),
        Decision::new(
            "offsite",
            "Book the Lisbon venue for the March offsite?",
            "Book it",
            "Keep looking",
        )
        .header("Offsite")
        .detail("€4,200 for two nights, the hold expires Friday")
        .collapsed(true),
        Decision::new(
            "hiring",
            "Open a second product designer role?",
            "Open the role",
            "Not this quarter",
        )
        .header("Hiring")
        .answered(Answer::Primary),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    // Once nothing needs you, the inbox starts over 3.2 s later with fresh state.
    let round = Rc::new(Cell::new(0usize));
    let settled: Rc<RefCell<HashSet<SharedString>>> = Rc::default();
    story::view(cx, move |_, _| {
        let (round_, settled_) = (round.clone(), settled.clone());
        div().w(px(600.)).child(
            DecisionInbox::new(
                ElementId::named_usize("decision-inbox", round.get()),
                demo_decisions(),
            )
            .on_decide(move |decision, _, _, cx| {
                settled_.borrow_mut().insert(decision.id.clone());
                let done = demo_decisions()
                    .iter()
                    .all(|d| d.answered.is_some() || settled_.borrow().contains(&d.id));
                if !done {
                    return;
                }
                settled_.borrow_mut().clear();
                let round = round_.clone();
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(3200))
                        .await;
                    round.set(round.get() + 1);
                    cx.refresh();
                })
                .detach();
            }),
        )
    })
}
