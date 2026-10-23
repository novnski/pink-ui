//! Message Actions – a Pink component.
//!
//! The hover lane under a transcript entry (Pink: transcript.rs
//! `Transcript::render_row`, the hover-revealed metadata strip). A 28px band
//! stays reserved under the message, so revealing it never moves the layout.
//! While the message is hovered the lane fades in over 150ms: the message's
//! timestamp, then 24px icon actions – Side chat (Send to main inside a side
//! chat), Fork from here, Edit message and Rewind on your own messages, and
//! Copy, which swaps to a check labelled "Copied" for 1.2s. A user message's
//! lane lines up with the right edge of its bubble, a reply's with the left
//! edge of its text.
//!
//! ```ignore
//! MessageActions::new("reply-7")
//!     .role(MessageRole::Assistant)
//!     .timestamp("Oct 23, 3:42 PM")
//!     .text(reply.clone())
//!     .message_hovered(hovered == Some(7))
//!     .on_action(|action, window, cx| { /* open a side chat, fork, … */ })
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyView, App, BoxShadow, ClipboardItem, Context, Div, ElementId, Entity, IntoElement, Role,
    SharedString, Stateful, Task, Window, div, point, prelude::*, px, relative, svg,
};

use crate::{
    motion,
    pink::{self, Pink, TextStyle, TypeStyleExt, icon_size, radius, space, text},
    story,
};

pub const NAME: &str = "Message Actions";
pub const DESCRIPTION: &str = "The hover lane under a chat message – its timestamp and quiet icon actions to branch into a side chat, fork from here, edit, rewind or copy, fading in while the message is hovered, with Copy turning into a check";

const ICON_COPY: &str = "icons/message-actions/copy.svg";
const ICON_COPY_CHECK: &str = "icons/message-actions/copy-check.svg";
const ICON_SIDE_CHAT: &str = "icons/message-actions/chat-line.svg";
const ICON_SEND_TO_MAIN: &str = "icons/message-actions/undo-left.svg";
const ICON_FORK: &str = "icons/message-actions/arrow-fork.svg";
const ICON_EDIT: &str = "icons/message-actions/pen.svg";
const ICON_REWIND: &str = "icons/message-actions/restart.svg";

/// The reserved lane (Pink: transcript.rs `META_LANE_HEIGHT`): the 24px
/// actions plus a 4px lead above them.
const LANE_HEIGHT: f32 = 28.0;
/// One action's square (Pink: transcript.rs `meta_action`).
const ACTION_SIZE: f32 = 24.0;
/// How long Copy shows its check (Pink: `Transcript::copy_message`).
const COPIED_FOR: Duration = Duration::from_millis(1200);

type CopyHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(MessageAction, &mut Window, &mut App)>;

/// Who wrote the message. User lanes align to the end (under the bubble),
/// assistant lanes to the start (under the text).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MessageRole {
    User,
    #[default]
    Assistant,
}

impl MessageRole {
    /// The actions Pink offers on this kind of message: history edits (Edit,
    /// Rewind) only on your own messages.
    pub fn default_actions(self) -> &'static [MessageAction] {
        match self {
            MessageRole::User => &[
                MessageAction::SideChat,
                MessageAction::Fork,
                MessageAction::Edit,
                MessageAction::Rewind,
                MessageAction::Copy,
            ],
            MessageRole::Assistant => &[
                MessageAction::SideChat,
                MessageAction::Fork,
                MessageAction::Copy,
            ],
        }
    }
}

/// One action in the lane. The lane always lays them out in Pink's order,
/// whatever order they are given in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageAction {
    /// Reply to this message in a side chat.
    SideChat,
    /// Inside a side chat: hand this result back to the main chat's draft.
    /// Takes Side chat's place when both are given.
    SendToMain,
    /// Start a new chat from this point of the conversation.
    Fork,
    /// Edit an earlier message of yours and continue from there.
    Edit,
    /// Rewind the conversation to before this message.
    Rewind,
    /// Copy the message text.
    Copy,
}

impl MessageAction {
    /// Pink's order in the lane (after the timestamp).
    const ORDER: [MessageAction; 6] = [
        MessageAction::SideChat,
        MessageAction::SendToMain,
        MessageAction::Fork,
        MessageAction::Edit,
        MessageAction::Rewind,
        MessageAction::Copy,
    ];

    /// The tooltip and accessible name.
    pub fn label(self) -> &'static str {
        match self {
            MessageAction::SideChat => "Side chat",
            MessageAction::SendToMain => "Send to main",
            MessageAction::Fork => "Fork from here",
            MessageAction::Edit => "Edit message",
            MessageAction::Rewind => "Rewind",
            MessageAction::Copy => "Copy",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            MessageAction::SideChat => ICON_SIDE_CHAT,
            MessageAction::SendToMain => ICON_SEND_TO_MAIN,
            MessageAction::Fork => ICON_FORK,
            MessageAction::Edit => ICON_EDIT,
            MessageAction::Rewind => ICON_REWIND,
            MessageAction::Copy => ICON_COPY,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            MessageAction::SideChat => "side-chat",
            MessageAction::SendToMain => "send-to-main",
            MessageAction::Fork => "fork",
            MessageAction::Edit => "edit",
            MessageAction::Rewind => "rewind",
            MessageAction::Copy => "copy",
        }
    }

    /// A longer accessible name where Pink gives one.
    fn description(self) -> Option<&'static str> {
        match self {
            MessageAction::SideChat => Some("Reply to this message or selection in a side chat"),
            MessageAction::SendToMain => Some("Insert this result into the parent draft"),
            _ => None,
        }
    }
}

/// The hover fade of one action button (Pink: `meta_action`'s `tl-{id}`).
fn action_fade_key(lane: &str, action: MessageAction) -> String {
    format!("tl-{lane}-{}", action.slug())
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// What the lane was last rendered with – Copy reads it.
#[derive(Clone, Default)]
struct Props {
    text: Option<SharedString>,
    on_copy: Option<CopyHandler>,
    on_action: Option<ActionHandler>,
}

/// What the lane keeps between frames: the Copied feedback and its own
/// hover. Normally keyed by the lane's id; create one yourself and pass it
/// with [`MessageActions::state`] to drive the lane from code.
pub struct MessageActionsState {
    props: Props,
    copied: bool,
    hovered: bool,
    copy_timer: Option<Task<()>>,
}

impl MessageActionsState {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        MessageActionsState {
            props: Props::default(),
            copied: false,
            hovered: false,
            copy_timer: None,
        }
    }

    /// Whether Copy is showing its check.
    pub fn is_copied(&self) -> bool {
        self.copied
    }

    /// Copy (Pink: `Transcript::copy_message`): writes the text to the
    /// clipboard and shows the check for 1.2s.
    pub fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.props.text.clone();
        if let Some(text) = &text {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
        self.copied = true;
        self.copy_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            this.update(cx, |s, cx| {
                s.copied = false;
                s.copy_timer = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
        if let Some(f) = self.props.on_copy.clone() {
            f(text, window, cx);
        }
        if let Some(f) = self.props.on_action.clone() {
            f(MessageAction::Copy, window, cx);
        }
    }

    fn activate(&mut self, action: MessageAction, window: &mut Window, cx: &mut Context<Self>) {
        if action == MessageAction::Copy {
            self.copy(window, cx);
        } else if let Some(f) = self.props.on_action.clone() {
            f(action, window, cx);
        }
    }

    fn set_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.hovered != hovered {
            self.hovered = hovered;
            cx.notify();
        }
    }
}

// ---------------------------------------------------------------------------
// The lane
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct MessageActions {
    id: ElementId,
    role: MessageRole,
    timestamp: Option<SharedString>,
    actions: Option<Vec<MessageAction>>,
    rewind_pending: bool,
    quiet: bool,
    message_hovered: bool,
    props: Props,
    state: Option<Entity<MessageActionsState>>,
}

impl MessageActions {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            role: MessageRole::default(),
            timestamp: None,
            actions: None,
            rewind_pending: false,
            quiet: true,
            message_hovered: false,
            props: Props::default(),
            state: None,
        }
    }

    /// Who wrote the message: aligns the lane and picks the default actions.
    pub fn role(mut self, role: MessageRole) -> Self {
        self.role = role;
        self
    }

    /// When the message was sent, already formatted – Pink writes
    /// "Oct 23, 3:42 PM".
    pub fn timestamp(mut self, timestamp: impl Into<SharedString>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// The message text – what Copy copies.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.props.text = Some(text.into());
        self
    }

    /// Which actions to offer; [`MessageRole::default_actions`] by default.
    pub fn actions(mut self, actions: impl IntoIterator<Item = MessageAction>) -> Self {
        self.actions = Some(actions.into_iter().collect());
        self
    }

    /// A rewind is waiting to be reconciled: Rewind reads "Reconcile rewind".
    pub fn rewind_pending(mut self, pending: bool) -> Self {
        self.rewind_pending = pending;
        self
    }

    /// Hidden until the message or the lane is hovered (default true, as in
    /// Pink); false keeps the lane shown.
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Whether the message this lane belongs to is hovered – reveals it.
    pub fn message_hovered(mut self, hovered: bool) -> Self {
        self.message_hovered = hovered;
        self
    }

    /// Called after Copy, with the text it copied.
    pub fn on_copy(
        mut self,
        f: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.props.on_copy = Some(Rc::new(f));
        self
    }

    /// Called for every action pressed, Copy included.
    pub fn on_action(mut self, f: impl Fn(MessageAction, &mut Window, &mut App) + 'static) -> Self {
        self.props.on_action = Some(Rc::new(f));
        self
    }

    /// Drives the lane from a state you own instead of one keyed by its id.
    pub fn state(mut self, state: Entity<MessageActionsState>) -> Self {
        self.state = Some(state);
        self
    }
}

/// One action in the lane (Pink: transcript.rs `meta_action`): a 24px r6
/// square, transparent easing to `ink(0.08)` on hover, a 14px glyph in
/// `content_muted`, the inset focus ring and a text tooltip on GPUI's
/// default delay.
fn meta_action(
    id: SharedString,
    fade_key: String,
    icon: &'static str,
    label: &'static str,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .size(px(ACTION_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .cursor_pointer()
        .bg(motion::hover_blend(
            &fade_key,
            gpui::transparent_black(),
            t.ink(0.08),
        ))
        .on_hover(motion::hover_listener(fade_key))
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .tooltip(pink::text_tooltip(label))
        .child(
            svg()
                .path(icon)
                .flex_none()
                .size(px(icon_size::SM))
                .text_color(t.content_muted),
        )
}

impl RenderOnce for MessageActions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = match self.state.clone() {
            Some(state) => state,
            None => window.use_keyed_state(
                ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
                cx,
                MessageActionsState::new,
            ),
        };
        let props = self.props.clone();
        state.update(cx, |s, _| s.props = props);
        let (copied, lane_hovered) = {
            let s = state.read(cx);
            (s.copied, s.hovered)
        };
        let shown = !self.quiet || self.message_hovered || lane_hovered;
        let actions = self
            .actions
            .clone()
            .unwrap_or_else(|| self.role.default_actions().to_vec());
        let handoff = actions.contains(&MessageAction::SendToMain);

        // The strip: a 6px lead, the timestamp, then the actions, 2px apart
        // (Pink: `render_row`'s `metadata`). It mounts only while shown and
        // fades in each time (Pink: `motion::fade_quick`, 150ms ease).
        let strip = shown.then(|| {
            let mut strip = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S2))
                .child(div().mr(px(space::S6)))
                .children(self.timestamp.clone().map(|stamp| {
                    div()
                        .type_style(TextStyle::Footnote)
                        .text_color(t.content_faint)
                        .child(stamp)
                }));
            for action in MessageAction::ORDER {
                if !actions.contains(&action) || (action == MessageAction::SideChat && handoff) {
                    continue;
                }
                let (icon, label) = match action {
                    MessageAction::Copy if copied => (ICON_COPY_CHECK, "Copied"),
                    MessageAction::Rewind if self.rewind_pending => {
                        (action.icon(), "Reconcile rewind")
                    }
                    _ => (action.icon(), action.label()),
                };
                let button = meta_action(
                    format!("{key}-{}", action.slug()).into(),
                    action_fade_key(&key, action),
                    icon,
                    label,
                    &t,
                )
                .when_some(action.description(), |el, aria| el.aria_label(aria))
                // Pink swallows Enter/Space on the actions that move focus
                // elsewhere, so the keyboard click is their only activation.
                .when(
                    matches!(
                        action,
                        MessageAction::SideChat | MessageAction::SendToMain | MessageAction::Rewind
                    ),
                    |el| el.on_key_down(pink::swallow_activation_key),
                );
                let state = state.clone();
                strip = strip.child(button.on_click(move |_, window, cx| {
                    if action == MessageAction::Copy {
                        cx.stop_propagation();
                    }
                    state.update(cx, |s, cx| s.activate(action, window, cx));
                }));
            }
            motion::fade_quick(ElementId::Name(format!("meta-{key}").into()), strip)
        });

        // Followers and hover fades started in this render need frames.
        motion::keep_alive(window);

        let hover = state.clone();
        div()
            .id(self.id.clone())
            .w_full()
            .h(px(LANE_HEIGHT))
            .pt(px(LANE_HEIGHT - ACTION_SIZE))
            .flex()
            .items_center()
            .font_family(pink::SANS)
            // No horizontal inset: the lane sits on the content column's
            // edges, start for replies and end for the user's bubble.
            .when(self.role == MessageRole::User, |el| el.justify_end())
            .on_hover(move |hovered, _, cx| hover.update(cx, |s, cx| s.set_hovered(*hovered, cx)))
            .children(strip)
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// The demo's content column.
const COLUMN: f32 = 600.;
/// Transcript prose line height (Pink: transcript.rs `USER_LINE_HEIGHT` and
/// markdown `MD_LINE_HEIGHT`, both 22 on 14px text).
const PROSE_LINE_HEIGHT: f32 = 22.;

struct Entry {
    role: MessageRole,
    timestamp: &'static str,
    paragraphs: &'static [&'static str],
}

const ENTRIES: [Entry; 4] = [
    Entry {
        role: MessageRole::User,
        timestamp: "Oct 23, 3:41 PM",
        paragraphs: &["The checkout test fails on CI about one run in five. Can you find out why?"],
    },
    Entry {
        role: MessageRole::Assistant,
        timestamp: "Oct 23, 3:43 PM",
        paragraphs: &[
            "The payment helper polls with a fixed two-second timeout, but the CI runners take up to three seconds to boot the Stripe mock, so the first assertion sometimes runs before the server is listening.",
            "I made the helper wait for the mock's health check before it starts the clock and raised the timeout to ten seconds. Twenty runs in a row now pass locally.",
        ],
    },
    Entry {
        role: MessageRole::User,
        timestamp: "Oct 23, 3:46 PM",
        paragraphs: &["Great. Add a retry around the webhook assertion too, then open a PR."],
    },
    Entry {
        role: MessageRole::Assistant,
        timestamp: "Oct 23, 3:48 PM",
        paragraphs: &[
            "Done. The webhook check now retries three times with a short backoff, and PR #482 carries both fixes with a note on the root cause.",
        ],
    },
];

fn lane_id(ix: usize) -> ElementId {
    ElementId::NamedInteger("ma-lane".into(), ix as u64)
}

/// One beat of the auto-play: what the "pointer" does after how long.
#[derive(Clone, Copy)]
enum Step {
    /// Rests on an entry, or leaves the transcript.
    Hover(Option<usize>),
    /// Rests on one of an entry's actions (its hover wash), or leaves it.
    Point(Option<(usize, MessageAction)>),
    /// Presses Copy on an entry.
    Copy(usize),
}

const SCRIPT: [(u64, Step); 9] = [
    (600, Step::Hover(Some(1))),
    (900, Step::Point(Some((1, MessageAction::Copy)))),
    (400, Step::Copy(1)),
    (1500, Step::Point(None)),
    (200, Step::Hover(Some(2))),
    (1000, Step::Point(Some((2, MessageAction::Edit)))),
    (3200, Step::Point(None)),
    (200, Step::Hover(Some(3))),
    (2000, Step::Hover(None)),
];

struct Demo {
    lanes: Vec<Entity<MessageActionsState>>,
    /// The entry the auto-play rests on.
    auto_row: Option<usize>,
    /// The action the auto-play rests on.
    pointed: Option<(usize, MessageAction)>,
    /// The entry under the real pointer (Pink: `hovered_entry`).
    pointer_row: Option<usize>,
    /// The viewer took over: auto-play stopped and Copy copies the message.
    live: bool,
    autoplay: Option<Task<()>>,
}

impl Demo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let lanes = ENTRIES
            .iter()
            .map(|_| cx.new(|cx| MessageActionsState::new(window, cx)))
            .collect::<Vec<_>>();
        let autoplay = Some(Self::autoplay(window, cx));
        Demo {
            lanes,
            auto_row: None,
            pointed: None,
            pointer_row: None,
            live: false,
            autoplay,
        }
    }

    fn autoplay(window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn_in(window, async move |demo, cx| {
            loop {
                for (wait, step) in SCRIPT {
                    cx.background_executor()
                        .timer(Duration::from_millis(wait))
                        .await;
                    let played =
                        cx.update(|window, cx| demo.update(cx, |d, cx| d.play(step, window, cx)));
                    if !matches!(played, Ok(Ok(()))) {
                        return;
                    }
                }
            }
        })
    }

    fn play(&mut self, step: Step, window: &mut Window, cx: &mut Context<Self>) {
        match step {
            Step::Hover(row) => self.auto_row = row,
            Step::Point(target) => self.point(target, window, cx),
            Step::Copy(ix) => {
                if let Some(lane) = self.lanes.get(ix) {
                    lane.update(cx, |s, cx| s.copy(window, cx));
                }
            }
        }
        cx.notify();
    }

    /// Lights an action's hover wash as if the pointer rested on it.
    fn point(
        &mut self,
        target: Option<(usize, MessageAction)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reduced = motion::reduced_motion(cx);
        if let Some((ix, action)) = self.pointed.take() {
            motion::set_hover(
                &action_fade_key(&lane_id(ix).to_string(), action),
                false,
                reduced,
            );
        }
        if let Some((ix, action)) = target {
            motion::set_hover(
                &action_fade_key(&lane_id(ix).to_string(), action),
                true,
                reduced,
            );
        }
        self.pointed = target;
        window.refresh();
    }

    fn take_over(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.live {
            self.live = true;
            self.autoplay = None;
            self.point(None, window, cx);
            cx.notify();
        }
    }

    /// Only the entry that owns the reveal may clear it (Pink: `render_row`).
    fn hover_row(&mut self, ix: usize, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            if self.pointer_row != Some(ix) {
                self.pointer_row = Some(ix);
                cx.notify();
            }
        } else if self.pointer_row == Some(ix) {
            self.pointer_row = None;
            cx.notify();
        }
    }

    fn hovered(&self) -> Option<usize> {
        if self.live {
            self.pointer_row
        } else {
            self.auto_row
        }
    }
}

/// A user bubble (Pink: transcript.rs `RowKind::User`): right-aligned, at
/// most 80% of the column, `user_bubble_bg` with light mode's inset edge,
/// r16, `px 16 / py 10`, 14/22 text.
fn user_bubble(text: &'static str, t: &Pink) -> Div {
    div().w_full().flex().justify_end().child(
        div()
            .min_w_0()
            .max_w(px(COLUMN * 0.8))
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
            .px(px(space::S16))
            .py(px(space::S10))
            .text_size(px(text::PROSE))
            .line_height(px(PROSE_LINE_HEIGHT))
            .text_color(t.text)
            .child(text),
    )
}

/// A reply: bare prose on the canvas (Pink: markdown `MD_TEXT_SIZE` 14 /
/// `MD_LINE_HEIGHT` 22, `MD_BLOCK_GAP` 12 between paragraphs).
fn reply(paragraphs: &'static [&'static str], t: &Pink) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(space::S12))
        .text_size(px(text::PROSE))
        .line_height(px(PROSE_LINE_HEIGHT))
        .text_color(t.text)
        .children(paragraphs.iter().map(|p| div().child(*p)))
}

fn demo_view(demo: &Entity<Demo>, cx: &mut App) -> Stateful<Div> {
    let t = Pink::get(cx);
    let d = demo.read(cx);
    let hovered = d.hovered();
    let live = d.live;
    let rows = ENTRIES.iter().enumerate().map(|(ix, entry)| {
        let user = entry.role == MessageRole::User;
        // Turn rhythm (Pink: `top_gap_for`): 16 before a new turn, 8
        // between a message and its reply.
        let gap = match (ix, user) {
            (0, _) => 0.,
            (_, true) => space::LG,
            (_, false) => space::SM,
        };
        let copy_text: SharedString = entry.paragraphs.join("\n\n").into();
        let lane = MessageActions::new(lane_id(ix))
            .role(entry.role)
            .timestamp(entry.timestamp)
            .message_hovered(hovered == Some(ix))
            .state(d.lanes[ix].clone())
            .when(live, |lane| lane.text(copy_text));
        let row_hover = demo.downgrade();
        div()
            .id(ElementId::NamedInteger("ma-row".into(), ix as u64))
            .w_full()
            .flex()
            .flex_col()
            .pt(px(gap))
            .on_hover(move |h, _, cx| {
                row_hover.update(cx, |d, cx| d.hover_row(ix, *h, cx)).ok();
            })
            .child(if user {
                user_bubble(entry.paragraphs[0], &t)
            } else {
                reply(entry.paragraphs, &t)
            })
            .child(lane)
    });

    // Any pointer or key on the demo stops the auto-play.
    let (enter, press, key) = (demo.downgrade(), demo.downgrade(), demo.downgrade());
    div()
        .id("ma-demo")
        .w(px(COLUMN))
        .max_w(relative(1.))
        .flex()
        .flex_col()
        .font_family(pink::SANS)
        .text_color(t.text)
        .on_hover(move |h, window, cx| {
            if *h {
                enter.update(cx, |d, cx| d.take_over(window, cx)).ok();
            }
        })
        .capture_any_mouse_down(move |_, window, cx| {
            press.update(cx, |d, cx| d.take_over(window, cx)).ok();
        })
        .capture_key_down(move |_, window, cx| {
            key.update(cx, |d, cx| d.take_over(window, cx)).ok();
        })
        .children(rows)
}

pub fn story(window: &mut Window, cx: &mut App) -> AnyView {
    let demo = cx.new(|cx| Demo::new(window, cx));
    story::view(cx, move |_, cx| demo_view(&demo, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_follow_pinks_order() {
        let user = MessageRole::User.default_actions();
        assert_eq!(user.first(), Some(&MessageAction::SideChat));
        assert_eq!(user.last(), Some(&MessageAction::Copy));
        assert!(
            !MessageRole::Assistant
                .default_actions()
                .contains(&MessageAction::Edit)
        );
        let order = MessageAction::ORDER;
        let at = |a| order.iter().position(|x| *x == a).unwrap();
        assert!(at(MessageAction::Fork) < at(MessageAction::Edit));
        assert!(at(MessageAction::Rewind) < at(MessageAction::Copy));
    }
}
