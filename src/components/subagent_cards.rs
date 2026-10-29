//! Subagent Cards – the cards a chat's helper agents leave in its transcript.
//!
//! Pink: `transcript/agent_cards.rs` `AgentCard`, the card a child chat's
//! creation leaves in its parent's transcript, and
//! `transcript/child_notices.rs` `display_entry`, the result notice the child
//! posts back. A card follows the Settings row anatomy: the provider mark in
//! a 28px tile, centred against two lines – the title with a quiet
//! "Fresh side chat" type badge, then a status dot and status, the live or
//! settled elapsed time, model and reasoning and a one-line preview of the
//! prompt or of the helper's latest message – and its actions in one trailing
//! column: Show in split beside Open chat. At narrow widths the actions wrap
//! under the text; only the preview truncates. Statuses read like Pink's
//! sidebar (`shell/child_chats.rs` `ChildChatStatus`): a coloured dot and
//! quiet words, red only on failure. A running card ticks its timer once a
//! second; a settled one freezes it. Once the helper's result is posted in
//! the chat (its own notice below), the card goes back to saying what was
//! asked.
//!
//! The card is the `pink-row-card` package's `agent_card`, which PINK pins:
//! this story adds the demo, the statuses and the result notices.
//!
//! ```ignore
//! SubagentCards::new("helpers")
//!     .card(
//!         SubagentCard::new("audit", "Audit webhook retries")
//!             .provider("Helix", "icons/subagent-cards/helix.svg")
//!             .model("Helix 4 Pro")
//!             .reasoning("High")
//!             .prompt("List every webhook handler that is not idempotent.")
//!             .status(AgentStatus::Running)
//!             .started_at(started),
//!     )
//!     .result(SubagentResult::new("audit", ResultOutcome::Completed, "Two handlers…"))
//!     .on_open(|agent, mode, _, _| println!("open {agent} in {mode:?}"))
//! ```

use std::{ops::Range, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, HighlightStyle, Hsla, InteractiveText,
    IntoElement, SharedString, StyledText, Task, UnderlineStyle, Window, div, prelude::*, px,
};

use crate::{
    components::{row_card as rc, settings_parts as parts},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, layout, space},
    story,
};

pub const NAME: &str = "Subagent Cards";
pub const DESCRIPTION: &str = "The cards a chat's helper agents leave in its transcript: each names its model, wears a status dot with a ticking or settled timer and a one-line preview of the ask or the latest reply, and opens the helper's chat on its own or in a split, with its result posted below as a linked notice";

/// Pink: `icons::SPLIT_COLUMNS` (Show in split).
const ICON_SPLIT: &str = "icons/subagent-cards/columns-2.svg";
/// Pink: `icons::WIDGET`, the mark of a card with no known provider.
const ICON_WIDGET: &str = "icons/subagent-cards/widget.svg";
/// The demo's providers: pink-ui's fictional model brands.
const ICON_HELIX: &str = "icons/subagent-cards/helix.svg";
const ICON_ORBIT: &str = "icons/subagent-cards/orbit.svg";
const ICON_RELAY: &str = "icons/subagent-cards/relay.svg";

/// Pink: `agent_cards.rs` `NEW_THREAD` – a child-created record is always a
/// fresh side chat with no inherited context.
pub const NEW_THREAD: &str = "Fresh side chat";
/// Pink: the title a child chat without one shows.
const UNTITLED: &str = "Agent task";
/// Pink: `child_notices.rs`, the heading of a result whose chat is unknown.
const UNKNOWN_RESULT: &str = "Helper result";
/// Pink: the preview is cut to 200 characters before the line truncates it.
const PREVIEW_CHARS: usize = 200;
/// Pink: `markdown/render.rs` `MD_TEXT_SIZE` / `MD_LINE_HEIGHT` / `MD_BLOCK_GAP`.
const MD_TEXT_SIZE: f32 = 14.0;
const MD_LINE_HEIGHT: f32 = 22.0;
const MD_BLOCK_GAP: f32 = 12.0;

type OpenHandler = Rc<dyn Fn(&SharedString, OpenMode, &mut Window, &mut App)>;

/// Where a helper stands (Pink `shell/child_chats.rs` `ChildChatStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    NeedsApproval,
    NeedsInput,
    Running,
    Failed,
    /// Interrupted before its turn ended.
    Stopped,
    /// Its own turn ended (Pink never implies its background work is done).
    Completed,
    Idle,
    Archived,
}

impl AgentStatus {
    /// Pink: `ChildChatStatus::label`.
    pub fn label(self) -> &'static str {
        match self {
            Self::NeedsApproval => "Needs approval",
            Self::NeedsInput => "Needs input",
            Self::Running => "Running",
            Self::Failed => "Failed",
            Self::Stopped => "Stopped",
            Self::Completed => "Turn completed",
            Self::Idle => "Idle",
            Self::Archived => "Archived",
        }
    }

    /// Pink: `ChildChatStatus::color` – the sidebar's status palette
    /// (`spaces.rs` `status_dot_color`), kept at least 85% strong because a
    /// card is roomier than a sidebar row.
    pub fn color(self, t: &Pink) -> Hsla {
        let color = match self {
            Self::NeedsApproval => t.warning.opacity(0.72),
            Self::NeedsInput => t.accent.opacity(0.6),
            Self::Running => t.busy.opacity(0.55),
            Self::Failed => t.danger.opacity(0.65),
            Self::Completed => t.success.opacity(0.9),
            Self::Stopped | Self::Idle | Self::Archived => return t.readable_muted(0.55),
        };
        Hsla {
            a: color.a.max(0.85),
            ..color
        }
    }

    /// A terminal status whose duration is frozen.
    pub fn settled(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Stopped)
    }
}

/// How a helper's chat opens (Pink `TranscriptEvent::OpenAgentChat { split }`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenMode {
    /// Open chat: the helper's conversation takes the view.
    Chat,
    /// Show in split: beside this one.
    Split,
}

/// One helper agent's card.
#[derive(Clone, Debug)]
pub struct SubagentCard {
    pub id: SharedString,
    pub title: SharedString,
    /// The provider's name, read out with the mark ("Helix provider").
    pub provider: Option<SharedString>,
    /// The provider's mark, an SVG path under `assets/`.
    pub icon: Option<SharedString>,
    /// A brand tint for the mark; without one it paints in `content_muted`.
    pub tint: Option<Hsla>,
    pub model: Option<SharedString>,
    pub reasoning: Option<SharedString>,
    /// What the helper was asked.
    pub prompt: SharedString,
    /// The helper's latest message, previewed once it settles.
    pub last_message: Option<SharedString>,
    /// Its result is already posted in this chat, so the card shows the ask.
    pub reported: bool,
    pub status: AgentStatus,
    /// When its live turn started; the timer counts from here while running.
    pub started_at: Option<Instant>,
    /// The settled duration of its last turn. Without one a settled card
    /// shows no time rather than an invented one.
    pub duration: Option<Duration>,
}

impl SubagentCard {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            provider: None,
            icon: None,
            tint: None,
            model: None,
            reasoning: None,
            prompt: SharedString::default(),
            last_message: None,
            reported: false,
            status: AgentStatus::Running,
            started_at: None,
            duration: None,
        }
    }

    pub fn provider(
        mut self,
        name: impl Into<SharedString>,
        icon: impl Into<SharedString>,
    ) -> Self {
        self.provider = Some(name.into());
        self.icon = Some(icon.into());
        self
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn reasoning(mut self, reasoning: impl Into<SharedString>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }

    pub fn prompt(mut self, prompt: impl Into<SharedString>) -> Self {
        self.prompt = prompt.into();
        self
    }

    pub fn last_message(mut self, message: impl Into<SharedString>) -> Self {
        self.last_message = Some(message.into());
        self
    }

    pub fn reported(mut self, reported: bool) -> Self {
        self.reported = reported;
        self
    }

    pub fn status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }

    pub fn started_at(mut self, started_at: Instant) -> Self {
        self.started_at = Some(started_at);
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Pink: `snapshot`'s preview – the ask while the helper is live or once
    /// its result is posted here, else its latest message (falling back to
    /// the ask), on one line of at most 200 characters.
    pub fn preview(&self) -> String {
        let latest = self
            .last_message
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty());
        let source = match latest {
            Some(latest) if self.status.settled() && !self.reported => latest,
            _ => self.prompt.as_ref(),
        };
        single_line(source).chars().take(PREVIEW_CHARS).collect()
    }

    /// Pink: `duration_text` – the live time while running, the frozen one
    /// once settled, and nothing when nothing reliable is known.
    pub fn duration_text(&self, now: Instant) -> Option<String> {
        if self.status == AgentStatus::Running {
            return self
                .started_at
                .map(|started| duration_label(now.saturating_duration_since(started)));
        }
        self.duration
            .filter(|_| self.status.settled())
            .map(duration_label)
    }
}

/// How a helper's turn ended, as its result notice says it (Pink
/// `child_notices.rs` `display_entry`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultOutcome {
    Completed,
    Failed,
    Interrupted,
    NeedsInput,
    NeedsApproval,
}

impl ResultOutcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Interrupted => "Interrupted",
            Self::NeedsInput => "Needs input",
            Self::NeedsApproval => "Needs approval",
        }
    }
}

/// A helper's result posted back into the chat.
#[derive(Clone, Debug)]
pub struct SubagentResult {
    /// The card it belongs to; its title links to the helper's chat.
    pub agent: SharedString,
    pub outcome: ResultOutcome,
    /// The result; blank lines separate paragraphs.
    pub text: SharedString,
}

impl SubagentResult {
    pub fn new(
        agent: impl Into<SharedString>,
        outcome: ResultOutcome,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            agent: agent.into(),
            outcome,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug)]
enum Entry {
    Card(SubagentCard),
    Result(SubagentResult),
}

/// The helpers' cards and results in transcript order, stacked 8px apart
/// like Pink's consecutive child notices (`transcript.rs` `top_gap_for`).
#[derive(IntoElement)]
pub struct SubagentCards {
    id: ElementId,
    entries: Vec<Entry>,
    width: Option<f32>,
    on_open: Option<OpenHandler>,
}

impl SubagentCards {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            entries: Vec::new(),
            width: None,
            on_open: None,
        }
    }

    pub fn card(mut self, card: SubagentCard) -> Self {
        self.entries.push(Entry::Card(card));
        self
    }

    pub fn cards(mut self, cards: impl IntoIterator<Item = SubagentCard>) -> Self {
        self.entries.extend(cards.into_iter().map(Entry::Card));
        self
    }

    pub fn result(mut self, result: SubagentResult) -> Self {
        self.entries.push(Entry::Result(result));
        self
    }

    /// A fixed width; by default the stack fills its parent.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Open chat, Show in split and a result's linked title.
    pub fn on_open(
        mut self,
        handler: impl Fn(&SharedString, OpenMode, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

/// Pink: `AgentCard::sync_tick` – a one-second self-notify kept alive exactly
/// while a card shows a live timer.
struct Ticker {
    task: Option<Task<()>>,
}

impl Ticker {
    fn sync(&mut self, live: bool, cx: &mut Context<Self>) {
        if !live {
            self.task = None;
            return;
        }
        if self.task.is_some() {
            return;
        }
        self.task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
    }
}

impl RenderOnce for SubagentCards {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let now = Instant::now();
        let key = self.id.to_string();

        let live = self.entries.iter().any(|entry| {
            matches!(entry, Entry::Card(card)
                if card.status == AgentStatus::Running && card.started_at.is_some())
        });
        let ticker = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "tick".into()),
            cx,
            |_, _| Ticker { task: None },
        );
        ticker.update(cx, |ticker, cx| ticker.sync(live, cx));

        let titles: Vec<(SharedString, SharedString)> = self
            .entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Card(card) => Some((card.id.clone(), card_title(card))),
                Entry::Result(_) => None,
            })
            .collect();

        div()
            .id(self.id.clone())
            .font_family(pink::SANS)
            .flex()
            .flex_col()
            .gap(px(space::SM))
            .map(|el| match self.width {
                Some(width) => el.w(px(width)),
                None => el.w_full(),
            })
            .min_w_0()
            .children(
                self.entries
                    .into_iter()
                    .enumerate()
                    .map(|(ix, entry)| match entry {
                        Entry::Card(card) => {
                            render_card(&key, &card, now, self.on_open.clone(), &t)
                        }
                        Entry::Result(result) => {
                            let title = titles
                                .iter()
                                .find(|(id, _)| *id == result.agent)
                                .map(|(_, title)| title.clone());
                            render_result(&key, ix, &result, title, self.on_open.clone(), &t)
                        }
                    }),
            )
    }
}

fn card_title(card: &SubagentCard) -> SharedString {
    if card.title.trim().is_empty() {
        UNTITLED.into()
    } else {
        card.title.clone()
    }
}

/// Pink: `AgentCard::render`, drawn by the `pink-row-card` package's agent
/// card: tile, text and actions in one row that wraps.
fn render_card(
    key: &str,
    card: &SubagentCard,
    now: Instant,
    on_open: Option<OpenHandler>,
    t: &Pink,
) -> AnyElement {
    let id = card.id.clone();
    let provider_name = card.provider.clone().unwrap_or_else(|| "Agent".into());
    let status_label = card.status.label();
    let status_color = card.status.color(t);
    let duration = card.duration_text(now);
    let preview = card.preview();

    // The provider mark in the shared 28px row tile (`pink-settings-parts`);
    // its label carries provider and status so neither is shape- or
    // colour-only.
    let tile = parts::row_tile(
        &parts::TileStyle {
            border: t.hairline(0.07),
            background: t.fill(FillTier::Quaternary),
            icon: card.tint.unwrap_or(t.content_muted),
        },
        card.icon.clone().unwrap_or_else(|| ICON_WIDGET.into()),
    )
    .id(SharedString::from(format!("{key}-tile-{id}")))
    .aria_label(SharedString::from(format!(
        "{provider_name} provider, {NEW_THREAD}, {status_label}"
    )));

    // The title with its quiet type badge, then the status (a coloured dot
    // and quiet words; only a failure tints them), the live or frozen time,
    // model and reasoning and the preview, which alone gives way when the
    // line runs out. Drawn by `pink-row-card`'s agent card.
    let detail = [card.model.clone(), card.reasoning.clone()]
        .into_iter()
        .flatten()
        .map(|part| part.to_string())
        .collect::<Vec<_>>();
    let detail = (!detail.is_empty()).then(|| detail.join(" "));
    let failed = card.status == AgentStatus::Failed;
    let style = rc::AgentCardStyle {
        card: rc::CardStyle {
            border: t.hairline(0.07),
            background: t.ink(0.03),
        },
        badge: rc::BadgeStyle {
            background: t.ink(0.06),
            text: t.content_muted,
        },
        title: t.text,
        meta: t.content_muted,
        status_dot: status_color,
        status_tone: failed.then(|| t.readable_tone(t.danger, 1.0)),
        preview: t.readable_muted(0.8),
    };
    let text = rc::AgentCardText {
        title: card_title(card),
        badge: NEW_THREAD.into(),
        status: status_label.into(),
        duration: duration.map(SharedString::from),
        detail: detail.map(SharedString::from),
        preview: preview.into(),
    };

    // The actions in one trailing column: Show in split as a kit icon
    // button beside the Open chat button. Ids carry the card id, so two
    // cards never share a fade.
    let split = pink::icon_button(format!("{key}-split-{id}"), ICON_SPLIT, ControlSize::Sm, t)
        .aria_label("Show agent chat in split")
        .tooltip(pink::text_tooltip("Show in split"))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .when_some(on_open.clone(), |el, handler| {
            let id = id.clone();
            el.on_click(move |_, window, cx| handler(&id, OpenMode::Split, window, cx))
        });
    let open = pink::button(
        format!("{key}-open-{id}"),
        ButtonKind::Secondary,
        ControlSize::Sm,
        t,
    )
    .aria_label("Open agent chat")
    .child("Open chat")
    .when_some(on_open, |el, handler| {
        let id = id.clone();
        el.on_click(move |_, window, cx| handler(&id, OpenMode::Chat, window, cx))
    });
    let actions = rc::agent_actions().child(split).child(open);

    rc::agent_card(&style, tile, text, actions)
        .id(SharedString::from(format!("{key}-card-{id}")))
        .into_any_element()
}

/// Pink: `child_notices.rs` `display_entry` – the helper's result as the
/// chat's own markdown: "[Title](its chat), Completed", then the result.
/// The title links only when the helper's card is known.
fn render_result(
    key: &str,
    ix: usize,
    result: &SubagentResult,
    title: Option<SharedString>,
    on_open: Option<OpenHandler>,
    t: &Pink,
) -> AnyElement {
    let linked = title.is_some();
    let title = title.unwrap_or_else(|| UNKNOWN_RESULT.into());
    let (heading, link) = heading_text(&title, result.outcome);
    // Pink's markdown links stay monochrome: the text colour with a 1px
    // underline in `content_muted`.
    let styled = StyledText::new(heading).with_highlights(linked.then(|| {
        (
            link.clone(),
            HighlightStyle {
                underline: Some(UnderlineStyle {
                    color: Some(t.content_muted),
                    thickness: px(1.0),
                    wavy: false,
                }),
                ..Default::default()
            },
        )
    }));
    let agent = result.agent.clone();
    let heading =
        InteractiveText::new(ElementId::Name(format!("{key}-result-{ix}").into()), styled)
            .when_some(on_open.filter(|_| linked), move |text, handler| {
                text.on_click(vec![link], move |_, window, cx| {
                    handler(&agent, OpenMode::Chat, window, cx)
                })
            });

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(MD_BLOCK_GAP))
        .text_size(px(MD_TEXT_SIZE))
        .line_height(px(MD_LINE_HEIGHT))
        .text_color(t.text)
        .child(div().w_full().child(heading))
        .children(
            paragraphs(&result.text)
                .into_iter()
                .map(|paragraph| div().w_full().child(paragraph)),
        )
        .into_any_element()
}

/// "Title, Completed" and the title's byte range.
fn heading_text(title: &str, outcome: ResultOutcome) -> (String, Range<usize>) {
    let title = single_line(title);
    let link = 0..title.len();
    (format!("{title}, {}", outcome.label()), link)
}

/// Blank-line separated paragraphs, each folded onto one wrapping line.
fn paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(single_line)
        .filter(|paragraph| !paragraph.is_empty())
        .collect()
}

/// Pink `transcript::single_line`: whitespace runs become one space.
fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Pink: `native_agents.rs` `duration_label` – "<1s", "42s", "3m 05s",
/// "1h 02m".
pub fn duration_label(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0 => "<1s".into(),
        1..=59 => format!("{secs}s"),
        _ if secs < 3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60),
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// The demo's beats, in seconds into the loop.
const BEAT_DONE: f32 = 2.5;
const BEAT_REPORTED: f32 = 5.0;
const BEAT_APPROVED: f32 = 9.0;
const DEMO_LOOP: f32 = 20.0;

struct Demo {
    start: Instant,
    _beats: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let beats = cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |demo, _| demo.until_next_beat(Instant::now()))
                else {
                    break;
                };
                cx.background_executor().timer(wait).await;
                let alive = this.update(cx, |demo, cx| {
                    if demo.elapsed(Instant::now()) >= DEMO_LOOP {
                        demo.start = Instant::now();
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Self {
            start: Instant::now(),
            _beats: beats,
        }
    }

    fn elapsed(&self, now: Instant) -> f32 {
        now.saturating_duration_since(self.start).as_secs_f32()
    }

    fn until_next_beat(&self, now: Instant) -> Duration {
        let elapsed = self.elapsed(now);
        let next = [BEAT_DONE, BEAT_REPORTED, BEAT_APPROVED, DEMO_LOOP]
            .into_iter()
            .find(|beat| *beat > elapsed)
            .unwrap_or(DEMO_LOOP);
        Duration::from_secs_f32((next - elapsed).max(0.0) + 0.01)
    }

    fn at(&self, secs: f32) -> Instant {
        if secs >= 0.0 {
            self.start + Duration::from_secs_f32(secs)
        } else {
            self.start
                .checked_sub(Duration::from_secs_f32(-secs))
                .unwrap_or(self.start)
        }
    }
}

fn demo_cards(demo: &Demo, now: Instant) -> SubagentCards {
    let elapsed = demo.elapsed(now);
    let helix = pink::project_tint(5);

    let audit = SubagentCard::new("audit", "Audit webhook retries")
        .provider("Helix", ICON_HELIX)
        .tint(helix)
        .model("Helix 4 Pro")
        .reasoning("High")
        .prompt(
            "Audit how billing-api retries payment webhooks and list every handler \
             that is not idempotent.",
        )
        .status(AgentStatus::Running)
        .started_at(demo.at(-83.0));

    let migration_done = elapsed >= BEAT_DONE;
    let migration_reported = elapsed >= BEAT_REPORTED;
    let migration = SubagentCard::new("migration", "Draft the invoice PDF migration")
        .provider("Orbit", ICON_ORBIT)
        .model("Orbit-6")
        .reasoning("Medium")
        .prompt(
            "Write the migration that adds pdf_url and pdf_generated_at to invoices, \
             with a backfill plan.",
        )
        .last_message(MIGRATION_RESULT_SHORT)
        .reported(migration_reported)
        .started_at(demo.at(-8.0))
        .duration(Duration::from_secs_f32(8.0 + BEAT_DONE))
        .status(if migration_done {
            AgentStatus::Completed
        } else {
            AgentStatus::Running
        });

    let approved = elapsed >= BEAT_APPROVED;
    let flaky = SubagentCard::new("flaky", "Fix the flaky checkout test")
        .provider("Relay", ICON_RELAY)
        .model("Kestrel 2 70B")
        .prompt("Find out why checkout_flow::applies_coupon fails one run in ten and fix it.")
        .started_at(demo.at(BEAT_APPROVED))
        .status(if approved {
            AgentStatus::Running
        } else {
            AgentStatus::NeedsApproval
        });

    let churn = SubagentCard::new("churn", "Summarize churn interviews")
        .provider("Helix", ICON_HELIX)
        .tint(helix)
        .model("Helix 4 Flash")
        .reasoning("Low")
        .prompt("Read the twelve churn interview notes and group what customers said.")
        .last_message(
            "Four themes: price jumps at renewal, slow CSV exports, no SSO on the \
             Team plan, and onboarding that skips invited teammates.",
        )
        .status(AgentStatus::Completed)
        .duration(Duration::from_secs(134));

    let driver = SubagentCard::new("driver", "Upgrade the Postgres driver")
        .provider("Orbit", ICON_ORBIT)
        .model("Orbit-6 Coder")
        .reasoning("High")
        .prompt("Bump the Postgres driver to 0.8 and fix whatever breaks.")
        .last_message("cargo test failed: 3 tests in db::pool time out after the upgrade.")
        .status(AgentStatus::Failed)
        .duration(Duration::from_secs(37));

    SubagentCards::new("subagent-cards")
        .cards([audit, migration, flaky, churn, driver])
        .when(migration_reported, |cards| {
            cards.result(SubagentResult::new(
                "migration",
                ResultOutcome::Completed,
                MIGRATION_RESULT,
            ))
        })
}

const MIGRATION_RESULT_SHORT: &str = "Added 0042_invoice_pdfs.sql: two nullable columns, an index on \
     pdf_generated_at and a batched backfill.";
const MIGRATION_RESULT: &str = "Added 0042_invoice_pdfs.sql with two nullable columns, \
     pdf_url and pdf_generated_at, and an index on pdf_generated_at.\n\n\
     The backfill runs in batches of 500 from the newest invoice back, so it can stop and \
     resume at any point. Nothing reads the new columns yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("subagent-cards-demo", cx, |_, cx| Demo::new(cx));
        let now = Instant::now();
        div().w(px(640.0)).child(demo_cards(demo.read(cx), now))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_like_pink() {
        assert_eq!(duration_label(Duration::from_millis(400)), "<1s");
        assert_eq!(duration_label(Duration::from_secs(42)), "42s");
        assert_eq!(duration_label(Duration::from_secs(185)), "3m 05s");
        assert_eq!(duration_label(Duration::from_secs(3720)), "1h 02m");
    }

    #[test]
    fn only_running_and_settled_cards_show_a_time() {
        let start = Instant::now();
        let now = start + Duration::from_secs(12);
        let card = SubagentCard::new("a", "A")
            .started_at(start)
            .duration(Duration::from_secs(5));
        assert_eq!(card.duration_text(now).as_deref(), Some("12s"));
        let done = card.clone().status(AgentStatus::Completed);
        assert_eq!(done.duration_text(now).as_deref(), Some("5s"));
        let waiting = card.clone().status(AgentStatus::NeedsApproval);
        assert_eq!(waiting.duration_text(now), None);
        let unknown = SubagentCard::new("b", "B").status(AgentStatus::Failed);
        assert_eq!(unknown.duration_text(now), None);
    }

    #[test]
    fn preview_shows_the_reply_until_it_is_reported() {
        let card = SubagentCard::new("a", "A")
            .prompt("Do the\n  thing")
            .last_message("Did it.");
        assert_eq!(card.preview(), "Do the thing");
        let done = card.clone().status(AgentStatus::Completed);
        assert_eq!(done.preview(), "Did it.");
        assert_eq!(done.clone().reported(true).preview(), "Do the thing");
        let blank = done.last_message("   ");
        assert_eq!(blank.preview(), "Do the thing");
    }

    #[test]
    fn status_dots_stay_legible() {
        let t = Pink::dark();
        assert_eq!(AgentStatus::Running.color(&t).a, 0.85);
        assert_eq!(AgentStatus::Completed.color(&t).a, 0.9);
        assert_eq!(AgentStatus::Stopped.color(&t), t.readable_muted(0.55));
    }

    #[test]
    fn result_heading_links_the_title() {
        let (text, link) = heading_text("Draft\nmigration", ResultOutcome::Completed);
        assert_eq!(text, "Draft migration, Completed");
        assert_eq!(&text[link], "Draft migration");
        assert_eq!(
            paragraphs("One\nline.\n\n\nTwo."),
            vec!["One line.", "Two."]
        );
    }
}
