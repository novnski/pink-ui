//! Email Draft – an agent-written email waiting for review, on a Pink card.
//!
//! The record the email is about heads the card: its reference, a status
//! pill and the amount, over the account. Below sits the email: a pop-up
//! button that switches the recipient from a popover menu (arrows and the
//! pointer share one highlight), the attachment as a message badge you can
//! remove and add back, the text (click it or the pen to edit in place;
//! ⌘/Ctrl+Enter sends, Escape stops editing) and a segmented tone switch.
//! The footer names the agent and holds Discard and Send. While the email
//! is in flight the status line shimmers; once sent or discarded the email
//! folds into a one-line receipt with Undo, the card gliding to its new
//! height.
//!
//! ```ignore
//! EmailDraft::new("renewal")
//!     .reference("SUB-1182")
//!     .on_send(|email, _, _| println!("to {}", email.to.email))
//! ```

use std::{cell::Cell, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Bounds, ClickEvent, Context, CursorStyle, Div, ElementId, Entity,
    FocusHandle, FontWeight, HighlightStyle, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, Pixels, SharedString, Stateful, StyledText, Subscription, Task, UnderlineStyle,
    Window, canvas, deferred, div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt},
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, Segment, TextStyle, TypeStyleExt, chrome,
        icon_size, layout, loaders, radius,
    },
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Email Draft";
pub const DESCRIPTION: &str = "An agent-written email waiting for review – switch the recipient from a menu, pick a tone, edit the text in place or drop the attachment, then send or discard it into a one-line receipt with undo";

/// The card's width.
const CARD_W: f32 = 480.;
/// The three-way tone switch: 68 per option.
const TONE_W: f32 = 204.;
/// How far the email dims while it is in flight.
const SENDING_OPACITY: f32 = 0.6;
/// The fake network round trip before the email counts as sent.
const SEND_DELAY: Duration = Duration::from_millis(900);

const ICON_PAPERCLIP: &str = "icons/email-draft/paperclip.svg";
const ICON_PEN: &str = "icons/email-draft/pen.svg";
const ICON_CLOSE: &str = "icons/email-draft/close.svg";
const ICON_PLUS: &str = "icons/email-draft/plus.svg";
const ICON_CHECK: &str = "icons/email-draft/check.svg";
const ICON_TRASH: &str = "icons/email-draft/trash-bin-minimalistic.svg";
const ICON_UNDO: &str = "icons/email-draft/undo-left.svg";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type SendHandler = Rc<dyn Fn(&SentEmail, &mut Window, &mut App)>;

/// Someone the email can go to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftContact {
    pub name: SharedString,
    pub email: SharedString,
}

impl DraftContact {
    pub fn new(name: impl Into<SharedString>, email: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
        }
    }

    /// The first word of the name ("Jonas" for "Jonas Weber").
    pub fn first_name(&self) -> &str {
        self.name.split(' ').next().unwrap_or_default()
    }
}

/// A text variant the reviewer can switch to, written for the current recipient.
#[derive(Clone)]
pub struct DraftTone {
    pub id: SharedString,
    pub label: SharedString,
    body: Rc<dyn Fn(&DraftContact) -> String>,
}

impl DraftTone {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        body: impl Fn(&DraftContact) -> String + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            body: Rc::new(body),
        }
    }

    /// The text for `to`.
    pub fn body(&self, to: &DraftContact) -> String {
        (self.body)(to)
    }
}

/// The link appended to the email.
#[derive(Clone, Debug)]
pub struct DraftLink {
    pub label: SharedString,
    pub href: SharedString,
}

impl DraftLink {
    pub fn new(label: impl Into<SharedString>, href: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
        }
    }
}

/// What `on_send` receives.
#[derive(Clone, Debug)]
pub struct SentEmail {
    pub to: DraftContact,
    /// The text with the link appended (`… Review quote: #`).
    pub body: String,
    pub attachment: Option<SharedString>,
}

/// Jonas Weber, Lena Hart and the Finance team at Northwind.
pub fn default_contacts() -> Vec<DraftContact> {
    vec![
        DraftContact::new("Jonas Weber", "jonas@northwind.io"),
        DraftContact::new("Lena Hart", "lena@northwind.io"),
        DraftContact::new("Finance team", "billing@northwind.io"),
    ]
}

/// Friendly, Short and Formal renewal reminders.
pub fn default_tones() -> Vec<DraftTone> {
    vec![
        DraftTone::new("friendly", "Friendly", |c| {
            format!(
                "Hi {}, a heads-up that your Growth plan renews on October 9 for $8,900 a year. If you'd like to add seats or switch to monthly billing, you can review the quote here:",
                c.first_name()
            )
        }),
        DraftTone::new("short", "Short", |c| {
            format!(
                "Hi {}, your Growth plan renews on October 9 ($8,900/year). Quote for changes:",
                c.first_name()
            )
        }),
        DraftTone::new("formal", "Formal", |c| {
            format!(
                "Dear {}, this is a reminder that your Growth subscription is scheduled to renew on October 9 at $8,900 per year. Should you wish to adjust seats or billing, please review the attached quote:",
                c.name
            )
        }),
    ]
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Draft,
    Sending,
    Sent,
    Discarded,
}

struct DraftState {
    to: DraftContact,
    file: Option<SharedString>,
    tone: SharedString,
    /// Edited text wins over the generated variant until the tone changes.
    edited: Option<String>,
    editing: bool,
    menu: bool,
    /// The menu row under the arrows and the pointer: Pink's one shared
    /// highlight (`menu_row_nav`).
    cursor: usize,
    /// When the menu began closing, for its fade out (Pink `menu_out`).
    menu_closed_at: Option<Instant>,
    /// Bumps on every close, so each exit plays from the start.
    menu_closes: u64,
    phase: Phase,
    /// Bumps on every tone pick, replaying the text's fade.
    tone_key: u64,
    /// Bumps whenever the attachment comes and goes, replaying its fade.
    file_key: u64,
    /// Bumps whenever the email and the receipt swap, replaying their entrance.
    view_key: u64,
    input: Entity<TextInput>,
    menu_focus: FocusHandle,
    /// The natural height of the email / receipt, measured after layout.
    content_height: Rc<Cell<Option<Pixels>>>,
    /// The recipient switch's bounds, so pressing it while the menu is open
    /// toggles the menu instead of counting as a press outside.
    trigger_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    over_link: Rc<Cell<bool>>,
    /// Whether editing was on when the pen went down: the press blurs the
    /// field first, so the click alone can't tell "stop" from "start".
    pen_was_editing: Rc<Cell<bool>>,
    _send: Option<Task<()>>,
    _input_events: Subscription,
}

impl DraftState {
    fn new(
        to: DraftContact,
        file: Option<SharedString>,
        tone: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).multiline());
        let input_events = cx.subscribe(&input, |this, _, event: &InputEvent, cx| match event {
            InputEvent::Change(text) => {
                this.edited = Some(text.to_string());
                cx.notify();
            }
            InputEvent::Escape | InputEvent::Blur if this.editing => {
                this.editing = false;
                cx.notify();
            }
            _ => {}
        });
        Self {
            to,
            file,
            tone,
            edited: None,
            editing: false,
            menu: false,
            cursor: 0,
            menu_closed_at: None,
            menu_closes: 0,
            phase: Phase::Draft,
            tone_key: 0,
            file_key: 0,
            view_key: 0,
            input,
            menu_focus: cx.focus_handle(),
            content_height: Rc::new(Cell::new(None)),
            trigger_bounds: Rc::new(Cell::new(None)),
            over_link: Rc::new(Cell::new(false)),
            pen_was_editing: Rc::new(Cell::new(false)),
            _send: None,
            _input_events: input_events,
        }
    }

    /// The generated text of the current tone.
    fn generated(&self, tones: &[DraftTone]) -> String {
        tones
            .iter()
            .find(|t| t.id == self.tone)
            .or(tones.first())
            .map(|t| t.body(&self.to))
            .unwrap_or_default()
    }

    fn body(&self, tones: &[DraftTone]) -> String {
        self.edited.clone().unwrap_or_else(|| self.generated(tones))
    }

    /// Hand edits that differ from the generated text.
    fn is_edited(&self, tones: &[DraftTone]) -> bool {
        self.edited
            .as_ref()
            .is_some_and(|text| *text != self.generated(tones))
    }

    fn start_editing(&mut self, tones: &[DraftTone], window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != Phase::Draft {
            return;
        }
        let body = self.body(tones);
        self.edited = Some(body.clone());
        self.editing = true;
        self.close_menu();
        self.input.update(cx, |input, cx| input.set_text(body, cx));
        TextInput::focus(&self.input, window, cx);
        cx.notify();
    }

    fn stop_editing(&mut self, cx: &mut Context<Self>) {
        if self.editing {
            self.editing = false;
            cx.notify();
        }
    }

    fn pick_tone(&mut self, id: SharedString, cx: &mut Context<Self>) {
        if id == self.tone && self.edited.is_none() {
            return;
        }
        self.tone = id;
        self.edited = None;
        self.editing = false;
        self.tone_key += 1;
        cx.notify();
    }

    fn open_menu(&mut self, contacts: &[DraftContact]) {
        if self.phase != Phase::Draft || self.menu {
            return;
        }
        self.menu = true;
        self.cursor = contacts
            .iter()
            .position(|c| c.email == self.to.email)
            .unwrap_or(0);
    }

    fn close_menu(&mut self) {
        if self.menu {
            self.menu = false;
            self.menu_closed_at = Some(Instant::now());
            self.menu_closes += 1;
        }
    }

    fn pick_contact(&mut self, contact: DraftContact, cx: &mut Context<Self>) {
        // Keep manual edits, but follow the new name in them
        if let Some(edited) = &self.edited {
            self.edited = Some(follow_name(edited, &self.to, &contact));
        }
        self.to = contact;
        self.close_menu();
        cx.notify();
    }

    fn set_file(&mut self, file: Option<SharedString>, cx: &mut Context<Self>) {
        self.file = file;
        self.file_key += 1;
        cx.notify();
    }

    fn send(
        &mut self,
        email: SentEmail,
        on_send: Option<SendHandler>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Draft {
            return;
        }
        self.editing = false;
        self.close_menu();
        self.phase = Phase::Sending;
        self._send = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SEND_DELAY).await;
            this.update_in(cx, |this, window, cx| {
                this.phase = Phase::Sent;
                this.view_key += 1;
                cx.notify();
                if let Some(on_send) = on_send {
                    on_send(&email, window, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn discard(&mut self, cx: &mut Context<Self>) -> bool {
        if self.phase != Phase::Draft {
            return false;
        }
        self.editing = false;
        self.close_menu();
        self.phase = Phase::Discarded;
        self.view_key += 1;
        cx.notify();
        true
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Draft;
        self.view_key += 1;
        cx.notify();
    }
}

/// Swaps the old recipient's name for the new one, like
/// `text.replace(/\bFirst\b|Full Name/g, …)` – a full name match becomes the
/// new full name, a first name the new first name.
fn follow_name(text: &str, from: &DraftContact, to: &DraftContact) -> String {
    let first = from.first_name();
    let full: &str = &from.name;
    let is_word = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    let boundary = |before: Option<char>, after: Option<char>| is_word(before) != is_word(after);
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let before = text[..i].chars().next_back();
        let first_hit = !first.is_empty()
            && rest.starts_with(first)
            && boundary(before, first.chars().next())
            && boundary(
                first.chars().next_back(),
                rest[first.len()..].chars().next(),
            );
        let hit = if first_hit {
            Some(first)
        } else if !full.is_empty() && rest.starts_with(full) {
            Some(full)
        } else {
            None
        };
        match hit {
            Some(hit) => {
                out.push_str(if hit == full {
                    &to.name
                } else {
                    to.first_name()
                });
                i += hit.len();
            }
            None => {
                let ch = rest.chars().next().unwrap_or_default();
                out.push(ch);
                i += ch.len_utf8().max(1);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// An agent-written email waiting for review.
#[derive(IntoElement)]
pub struct EmailDraft {
    id: ElementId,
    reference: SharedString,
    account: SharedString,
    amount: SharedString,
    status: SharedString,
    agent: SharedString,
    contacts: Vec<DraftContact>,
    attachment: Option<SharedString>,
    tones: Vec<DraftTone>,
    link: DraftLink,
    on_send: Option<SendHandler>,
    on_discard: Option<Handler>,
}

impl EmailDraft {
    /// A draft with the demo content; override any of it below.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            reference: "SUB-1182".into(),
            account: "Northwind Logistics".into(),
            amount: "$8,900".into(),
            status: "Renews in 5 days".into(),
            agent: "Renewal agent".into(),
            contacts: default_contacts(),
            attachment: Some("Renewal-quote.pdf".into()),
            tones: default_tones(),
            link: DraftLink::new("Review quote", "#"),
            on_send: None,
            on_discard: None,
        }
    }

    /// Reference of the record, e.g. a subscription or invoice number.
    pub fn reference(mut self, reference: impl Into<SharedString>) -> Self {
        self.reference = reference.into();
        self
    }

    /// Account or company under the reference.
    pub fn account(mut self, account: impl Into<SharedString>) -> Self {
        self.account = account.into();
        self
    }

    /// Amount shown on the right.
    pub fn amount(mut self, amount: impl Into<SharedString>) -> Self {
        self.amount = amount.into();
        self
    }

    /// Status pill next to the reference.
    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = status.into();
        self
    }

    /// Who wrote the draft.
    pub fn agent(mut self, agent: impl Into<SharedString>) -> Self {
        self.agent = agent.into();
        self
    }

    /// The recipients to pick from; the first one is selected.
    pub fn contacts(mut self, contacts: Vec<DraftContact>) -> Self {
        self.contacts = contacts;
        self
    }

    pub fn attachment(mut self, file: impl Into<SharedString>) -> Self {
        self.attachment = Some(file.into());
        self
    }

    pub fn no_attachment(mut self) -> Self {
        self.attachment = None;
        self
    }

    /// Text variants to switch between – the first one is shown first.
    pub fn tones(mut self, tones: Vec<DraftTone>) -> Self {
        self.tones = tones;
        self
    }

    /// Label and target of the link appended to the email.
    pub fn link(mut self, label: impl Into<SharedString>, href: impl Into<SharedString>) -> Self {
        self.link = DraftLink::new(label, href);
        self
    }

    /// Called once the email went out (after the Sending beat).
    pub fn on_send(
        mut self,
        handler: impl Fn(&SentEmail, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_send = Some(Rc::new(handler));
        self
    }

    pub fn on_discard(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_discard = Some(Rc::new(handler));
        self
    }
}

/// What every part of the draft needs to render.
struct Ctx {
    key: String,
    state: Entity<DraftState>,
    contacts: Rc<[DraftContact]>,
    tones: Rc<[DraftTone]>,
    send: Handler,
    start_editing: Handler,
    on_discard: Option<Handler>,
}

impl Ctx {
    /// A hover / follower / control key unique to this draft.
    fn key(&self, part: &str) -> String {
        format!("{}-{part}", self.key)
    }
}

impl RenderOnce for EmailDraft {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();

        let contacts: Rc<[DraftContact]> = if self.contacts.is_empty() {
            default_contacts().into()
        } else {
            std::mem::take(&mut self.contacts).into()
        };
        let tones: Rc<[DraftTone]> = if self.tones.is_empty() {
            default_tones().into()
        } else {
            std::mem::take(&mut self.tones).into()
        };

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "email-draft-state".into()),
            cx,
            {
                let to = contacts[0].clone();
                let file = self.attachment.clone();
                let tone = tones[0].id.clone();
                move |window, cx| DraftState::new(to, file, tone, window, cx)
            },
        );
        let (caret, selection) = (t.caret, t.selection);
        state.update(cx, |s, cx| {
            s.input.update(cx, |input, _| {
                input.set_colors(None, Some(caret), Some(selection))
            });
        });

        let send: Handler = Rc::new({
            let state = state.clone();
            let tones = tones.clone();
            let link = self.link.clone();
            let on_send = self.on_send.clone();
            move |window, cx| {
                state.update(cx, |s, cx| {
                    let email = SentEmail {
                        to: s.to.clone(),
                        body: format!("{} {}: {}", s.body(&tones), link.label, link.href),
                        attachment: s.file.clone(),
                    };
                    s.send(email, on_send.clone(), window, cx);
                })
            }
        });
        let start_editing: Handler = Rc::new({
            let state = state.clone();
            let tones = tones.clone();
            move |window, cx| state.update(cx, |s, cx| s.start_editing(&tones, window, cx))
        });
        let ctx = Ctx {
            key,
            state: state.clone(),
            contacts,
            tones,
            send,
            start_editing,
            on_discard: self.on_discard.clone(),
        };

        let (phase, view_key, content_height) = {
            let s = state.read(cx);
            (s.phase, s.view_key, s.content_height.clone())
        };

        // Email ↔ receipt: only this part glides to its new height
        let content: AnyElement = if matches!(phase, Phase::Draft | Phase::Sending) {
            let email = self.email(&ctx, &t, window, cx);
            entrance(("ed-email", view_key), email)
        } else {
            entrance(("ed-receipt", view_key), receipt(&ctx, &t, phase, cx))
        };
        let height = content_height
            .get()
            .map(|h| motion::follow(&ctx.key("height"), f32::from(h), motion::DISCLOSURE));
        let fold = div()
            .w_full()
            .when_some(height, |d, h| d.h(px(h)).overflow_hidden())
            .child(
                div()
                    .relative()
                    .w_full()
                    .flex_none()
                    .child(content)
                    .child(measure(content_height)),
            );
        motion::keep_alive(window);

        pink::card(&t)
            .id(self.id.clone())
            .w_full()
            .max_w(px(CARD_W))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(self.record(&t))
            .child(fold)
    }
}

impl EmailDraft {
    /// The record the email is about, in the settings card header's recipe
    /// (Pink `widgets::section_header`: `px 16 / pt 14 / pb 12`, gap 3): the
    /// reference and the amount in Title3, a status pill (Pink's sidebar
    /// waiting pill) and the account as a row description.
    fn record(&self, t: &Pink) -> Div {
        let title = |text: SharedString| {
            div()
                .flex_none()
                .type_style(TextStyle::Title3)
                .text_color(t.text)
                .child(text)
        };
        div()
            .px(px(layout::ROW_PAD_X))
            .pt(px(14.))
            .pb(px(12.))
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .min_w_0()
                    .child(title(self.reference.clone()))
                    .child(pink::status_pill(self.status.clone(), t.warning, None, t))
                    .child(div().flex_1())
                    .child(title(self.amount.clone()).font_features(pink::tabular_figures())),
            )
            .child(pink::row_description(self.account.clone(), t))
    }

    /// The email: recipient and attachment, the text, the tone switch and
    /// the footer with the decision.
    fn email(&self, ctx: &Ctx, t: &Pink, window: &mut Window, cx: &mut App) -> Div {
        let sending = ctx.state.read(cx).phase == Phase::Sending;
        let dim = motion::follow(
            &ctx.key("dim"),
            if sending { SENDING_OPACITY } else { 1. },
            motion::FADE_QUICK,
        );
        let email = div()
            .relative()
            .flex()
            .flex_col()
            .opacity(dim)
            .child(meta_row(ctx, t, self.attachment.clone(), window, cx))
            .child(body(ctx, t, &self.link, cx))
            .child(tone_row(ctx, t, cx))
            // In flight: nothing under the dimmed email takes the pointer
            .when(sending, |d| {
                d.child(div().id("ed-in-flight").absolute().inset_0().occlude())
            });
        div()
            .flex()
            .flex_col()
            .child(email)
            .child(self.footer(ctx, t, sending))
    }

    /// The agent and its status on the left, Discard and Send on the right,
    /// laid out like Pink's question wizard footer (notes, then one Primary).
    fn footer(&self, ctx: &Ctx, t: &Pink, sending: bool) -> Div {
        let status: SharedString = if sending {
            "Sending".into()
        } else {
            "Ready for review".into()
        };
        // The 12 px slot holds Pink's attention dot, or its running mark
        // (`mini_glyph_spinner`) while the email is in flight
        let mark = div()
            .flex_none()
            .size(px(loaders::ORB_BOX))
            .flex()
            .items_center()
            .justify_center()
            .when(sending, |d| {
                d.child(loaders::mini_glyph_spinner(ctx.key("spin"), 2., t.glyph))
            })
            .when(!sending, |d| {
                d.child(div().size(px(6.)).rounded_full().bg(t.accent))
            });
        let status_line =
            loaders::shimmer_text(status.clone(), 12., 16., t.readable_muted(0.7), t.text)
                .live(sending)
                .flex_1()
                .min_w_0()
                .overflow_hidden();
        let status_line =
            motion::status_swap(loaders::label_id(&ctx.key("status"), &status), status_line);
        let who = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_size(px(12.))
            .line_height(px(16.))
            .child(mark)
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text.opacity(0.9))
                    .child(self.agent.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .text_color(t.decorative_muted(0.3))
                    .child("·"),
            )
            .child(status_line);

        let discard = {
            let state = ctx.state.clone();
            let on_discard = ctx.on_discard.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                if state.update(cx, |s, cx| s.discard(cx))
                    && let Some(on_discard) = &on_discard
                {
                    on_discard(window, cx);
                }
            }
        };
        let send = ctx.send.clone();
        let buttons = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                pink::button_enabled(
                    ctx.key("discard"),
                    ButtonKind::Ghost,
                    ControlSize::Md,
                    !sending,
                    t,
                )
                .when(!sending, |d| d.on_click(discard))
                .child("Discard"),
            )
            .child(
                pink::button_enabled(
                    ctx.key("send"),
                    ButtonKind::Primary,
                    ControlSize::Md,
                    !sending,
                    t,
                )
                .when(!sending, |d| {
                    d.on_click(move |_, window, cx| send(window, cx))
                })
                .child(if sending { "Sending…" } else { "Send email" }),
            );
        row(t)
            .justify_between()
            .gap(px(12.))
            .child(who)
            .child(buttons)
    }
}

/// One row of the card: Pink's settings card row in its compact form
/// (`px 16`, a 28 px control line at 48 tall), with the `border` hairline on
/// top.
fn row(t: &Pink) -> Div {
    div()
        .flex()
        .items_center()
        .min_w_0()
        .min_h(px(layout::ROW_MIN_HEIGHT))
        .px(px(layout::ROW_PAD_X))
        .py(px(10.))
        .border_t_1()
        .border_color(t.border)
}

/// The email (or the receipt) rising in where the other stood: Pink's
/// in-place content swap (`TRANSCRIPT_IN`, 160 ms, 4 px).
fn entrance(id: impl Into<ElementId>, element: impl IntoElement + Styled + 'static) -> AnyElement {
    motion::rise_in(
        id,
        motion::TRANSCRIPT_IN,
        motion::TRANSCRIPT_IN_RISE,
        element,
    )
    .into_any_element()
}

/// An invisible layer filling its parent that records the parent's height
/// after layout, and asks for another frame when it changed.
fn measure(cell: Rc<Cell<Option<Pixels>>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let height = bounds.size.height;
            let changed = cell
                .get()
                .is_none_or(|old| (f32::from(old) - f32::from(height)).abs() > 0.1);
            if changed {
                cell.set(Some(height));
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// An invisible layer filling its parent that records the parent's bounds.
fn measure_bounds(cell: Rc<Cell<Option<Bounds<Pixels>>>>) -> impl IntoElement {
    canvas(move |bounds, _, _| cell.set(Some(bounds)), |_, _, _, _| {})
        .absolute()
        .top_0()
        .left_0()
        .size_full()
}

/// "To" with the recipient switch, and the attachment.
fn meta_row(
    ctx: &Ctx,
    t: &Pink,
    original: Option<SharedString>,
    window: &mut Window,
    cx: &mut App,
) -> Div {
    row(t)
        .justify_between()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .min_w_0()
                .child(
                    div()
                        .flex_none()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(t.readable_muted(0.7))
                        .child("To"),
                )
                .child(recipient(ctx, t, window, cx)),
        )
        .children(attachment(ctx, t, original, cx))
}

/// The recipient as Pink's pop-up button (`button::select_frame`: a
/// Secondary control, the choice and one chevron), with its menu.
fn recipient(ctx: &Ctx, t: &Pink, window: &mut Window, cx: &mut App) -> Div {
    let s = ctx.state.read(cx);
    let (to, open, cursor) = (s.to.clone(), s.menu, s.cursor);
    let (menu_focus, trigger_bounds) = (s.menu_focus.clone(), s.trigger_bounds.clone());
    let closing = (!open && !motion::reduced_motion(cx))
        .then_some(s.menu_closed_at)
        .flatten()
        .map(motion::menu_exit_progress)
        .filter(|progress| *progress < 1.);
    let closes = s.menu_closes;

    let toggle = {
        let state = ctx.state.clone();
        let contacts = ctx.contacts.clone();
        let menu_focus = menu_focus.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            let opened = state.update(cx, |s, cx| {
                if s.menu {
                    s.close_menu();
                } else {
                    s.open_menu(&contacts);
                }
                cx.notify();
                s.menu
            });
            if opened {
                window.focus(&menu_focus, cx);
            }
        }
    };
    let open_on_arrow = {
        let state = ctx.state.clone();
        let contacts = ctx.contacts.clone();
        let menu_focus = menu_focus.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if !matches!(event.keystroke.key.as_str(), "down" | "up") {
                return;
            }
            let opened = state.update(cx, |s, cx| {
                s.open_menu(&contacts);
                cx.notify();
                s.menu
            });
            if opened {
                window.focus(&menu_focus, cx);
            }
            cx.stop_propagation();
        }
    };
    let trigger = pink::select_frame(ctx.key("to"), ControlSize::Md, open, true, t)
        .flex_shrink_1()
        .min_w_0()
        .on_click(toggle)
        .on_key_down(open_on_arrow)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .overflow_hidden()
                .child(div().flex_none().child(to.name.clone()))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(t.readable_muted(0.85))
                        .child(to.email.clone()),
                ),
        )
        .child(pink::select_chevron(t));

    // Below the trigger with Pink's 6 px gap, its leading edge pulled out by
    // the card inset so the row text lines up with the trigger's
    let top = ControlSize::Md.height() + layout::POPOVER_GAP;
    let menu = open.then(|| {
        let panel = menu_panel(ctx, t, &to, cursor, top, &menu_focus, true);
        deferred(
            panel.with_animation("ed-menu-in", motion::MENU_IN.animation(), move |el, p| {
                el.opacity(p)
                    .top(px(top + motion::MENU_RISE_DOWN * (1. - p)))
            }),
        )
        .with_priority(1)
        .into_any_element()
    });
    // Closing: the same card fades out over MENU_OUT, taking no clicks
    let dying = closing.map(|progress| {
        window.request_animation_frame();
        let panel = menu_panel(ctx, t, &to, cursor, top, &menu_focus, false);
        deferred(motion::menu_out(
            ("ed-menu-out", closes as usize),
            progress,
            panel,
        ))
        .with_priority(1)
        .into_any_element()
    });

    div()
        .relative()
        .flex()
        .min_w_0()
        .child(trigger)
        .child(measure_bounds(trigger_bounds))
        .children(menu)
        .children(dying)
}

/// The recipient menu (Pink `popover_card` with `menu_row_nav` rows): a
/// heading, then one row per contact with its avatar, name and address, the
/// current one checked. `live` wires the keys and the pointer; the closing
/// copy is inert.
fn menu_panel(
    ctx: &Ctx,
    t: &Pink,
    to: &DraftContact,
    cursor: usize,
    top: f32,
    menu_focus: &FocusHandle,
    live: bool,
) -> Stateful<Div> {
    let rows = ctx.contacts.iter().enumerate().map(|(ix, contact)| {
        let selected = contact.email == to.email;
        pink::menu_row_nav(selected, ix == cursor, t)
            .id(("ed-contact", ix))
            .when(live, |row| {
                let state = ctx.state.clone();
                let hover_state = ctx.state.clone();
                let contact = contact.clone();
                row.on_mouse_move(move |_, _, cx| {
                    hover_state.update(cx, |s, cx| {
                        if s.cursor != ix {
                            s.cursor = ix;
                            cx.notify();
                        }
                    })
                })
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| s.pick_contact(contact.clone(), cx))
                })
            })
            .child(pink::avatar(pink::monogram(&contact.name), t))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(pink::space::TEXT_STACK_GAP))
                    .child(
                        div()
                            .line_height(px(18.))
                            .truncate()
                            .child(contact.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .line_height(px(14.))
                            .text_color(t.text_faint)
                            .truncate()
                            .child(contact.email.clone()),
                    ),
            )
    });

    let on_key = {
        let state = ctx.state.clone();
        let contacts = ctx.contacts.clone();
        move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let len = contacts.len();
            if len == 0 {
                return;
            }
            let key = event.keystroke.key.as_str();
            state.update(cx, |s, cx| {
                match key {
                    "escape" | "tab" => s.close_menu(),
                    "enter" | "space" => {
                        let contact = contacts[s.cursor.min(len - 1)].clone();
                        s.pick_contact(contact, cx);
                    }
                    "down" => s.cursor = (s.cursor + 1) % len,
                    "up" => s.cursor = (s.cursor + len - 1) % len,
                    "home" => s.cursor = 0,
                    "end" => s.cursor = len - 1,
                    _ => return,
                }
                cx.notify();
                cx.stop_propagation();
            });
        }
    };
    let close_outside = {
        let state = ctx.state.clone();
        let trigger = ctx.state.clone();
        move |event: &MouseDownEvent, _: &mut Window, cx: &mut App| {
            // A press on the switch itself toggles it instead
            let on_trigger = trigger
                .read(cx)
                .trigger_bounds
                .get()
                .is_some_and(|bounds| bounds.contains(&event.position));
            if !on_trigger {
                state.update(cx, |s, cx| {
                    s.close_menu();
                    cx.notify();
                });
            }
        }
    };
    pink::popover_card(t)
        .id(if live { "ed-menu" } else { "ed-menu-closing" })
        .absolute()
        .top(px(top))
        .left(px(-layout::CARD_INSET))
        .w(px(layout::MENU_W_S))
        .flex()
        .flex_col()
        .occlude()
        .when(live, |panel| {
            panel
                .track_focus(menu_focus)
                .on_key_down(on_key)
                .on_mouse_down_out(close_outside)
        })
        .child(pink::menu_heading("Send to", t))
        .children(rows)
}

/// The attachment as Pink's message badge (`badges::render`: 24 tall, `px 8`,
/// r 8, `ink(0.06)`, 12 medium `text_muted`, a 12 px glyph) with a remove
/// button in Pink's row-action size (`CONTROL_S` 20, `RADIUS_S`, a 12 px
/// glyph on the quiet hover wash); once removed, a Ghost button adds it back.
fn attachment(
    ctx: &Ctx,
    t: &Pink,
    original: Option<SharedString>,
    cx: &mut App,
) -> Option<AnyElement> {
    let s = ctx.state.read(cx);
    let (file, file_key) = (s.file.clone(), s.file_key);
    if let Some(file) = file {
        let remove_key = ctx.key("remove");
        let glyph = motion::hover_blend(&remove_key, t.readable_muted(0.7), t.text);
        let remove = div()
            .id("ed-remove")
            .flex_none()
            .size(px(chrome::CONTROL_S))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(chrome::RADIUS_S))
            .cursor_pointer()
            .hover_wash(remove_key, t.fill(FillTier::Tertiary))
            .tooltip(pink::text_tooltip("Remove attachment"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click({
                let state = ctx.state.clone();
                move |_, _, cx| state.update(cx, |s, cx| s.set_file(None, cx))
            })
            .child(
                svg()
                    .path(ICON_CLOSE)
                    .size(px(icon_size::XS))
                    .flex_none()
                    .text_color(glyph),
            );
        let badge = pink::message_badge(t)
            .min_w_0()
            .pr(px(2.))
            .child(
                svg()
                    .path(ICON_PAPERCLIP)
                    .size(px(icon_size::XS))
                    .flex_none()
                    .text_color(t.readable_muted(0.7)),
            )
            .child(div().min_w_0().truncate().child(file))
            .child(remove);
        return Some(reveal(("ed-file", file_key as usize), file_key > 0, badge));
    }

    let original = original?;
    let id = ctx.key("attach");
    let glyph = motion::hover_blend(&format!("pink-btn-{id}"), t.readable_muted(0.95), t.text);
    let button = pink::button(id, ButtonKind::Ghost, ControlSize::Sm, t)
        .on_click({
            let state = ctx.state.clone();
            let original = original.clone();
            move |_, _, cx| state.update(cx, |s, cx| s.set_file(Some(original.clone()), cx))
        })
        .child(
            svg()
                .path(ICON_PLUS)
                .size(px(ControlSize::Sm.glyph()))
                .flex_none()
                .text_color(glyph),
        )
        .child(format!("Attach {original}"));
    Some(reveal(
        ("ed-attach", file_key as usize),
        file_key > 0,
        button,
    ))
}

/// A swapped-in attachment fading in (Pink `ATTACHMENT_IN`, 240 ms
/// expo-out), once the reviewer changed it.
fn reveal(
    id: impl Into<ElementId>,
    play: bool,
    element: impl IntoElement + Styled + 'static,
) -> AnyElement {
    if !play {
        return element.into_any_element();
    }
    element
        .with_animation(id, motion::ATTACHMENT_IN.animation(), |el, p| el.opacity(p))
        .into_any_element()
}

/// The email text at Pink's chat prose size (14/22), its link drawn the way
/// Pink's markdown draws one: the text color with a 1 px `content_muted`
/// underline. Click the text to edit it in place: the field keeps every
/// glyph where it was, on Pink's field plate (`fill(Quaternary)`, a
/// `separator(true)` edge, r 8), with the accent caret.
fn body(ctx: &Ctx, t: &Pink, link: &DraftLink, cx: &mut App) -> Div {
    let s = ctx.state.read(cx);
    let (editing, tone_key) = (s.editing, s.tone_key);
    let text = s.body(&ctx.tones);
    let (input, over_link) = (s.input.clone(), s.over_link.clone());
    let plate = t.fill(FillTier::Quaternary);
    let frame = div()
        .mx(px(-8.))
        .my(px(-6.))
        .rounded(px(radius::MD))
        .text_size(px(14.))
        .line_height(px(22.))
        .text_color(t.text);
    let wrap = div().px(px(layout::ROW_PAD_X)).pt(px(14.));

    if editing {
        let send = ctx.send.clone();
        let state = ctx.state.clone();
        return wrap.child(
            frame
                .px(px(7.))
                .py(px(5.))
                .bg(plate)
                .border_1()
                .border_color(t.separator(true))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    let keys = &event.keystroke;
                    if keys.key == "enter" && (keys.modifiers.platform || keys.modifiers.control) {
                        send(window, cx);
                        cx.stop_propagation();
                    }
                })
                // Like a textarea's blur: a press anywhere else ends editing
                .on_mouse_down_out(move |_, _, cx| state.update(cx, |s, cx| s.stop_editing(cx)))
                .child(input),
        );
    }

    let link_start = text.len() + 1;
    let full = format!("{text} {}", link.label);
    let link_end = full.len();
    let styled = StyledText::new(full).with_highlights([(
        link_start..link_end,
        HighlightStyle {
            underline: Some(UnderlineStyle {
                color: Some(t.content_muted),
                thickness: px(1.),
                wavy: false,
            }),
            ..Default::default()
        },
    )]);
    let layout = styled.layout().clone();
    let text_key = ctx.key("text");
    let hover = motion::hover_listener(text_key.clone());
    let start_editing = ctx.start_editing.clone();
    let href = link.href.clone();
    let paragraph = frame
        .id("ed-text")
        .px(px(8.))
        .py(px(6.))
        .cursor(if over_link.get() {
            CursorStyle::PointingHand
        } else {
            CursorStyle::IBeam
        })
        .bg(motion::hover_blend(&text_key, plate.opacity(0.), plate))
        .on_hover({
            let over_link = over_link.clone();
            move |hovered, window, cx| {
                hover(hovered, window, cx);
                if !hovered {
                    over_link.set(false);
                }
            }
        })
        .on_mouse_move({
            let over_link = over_link.clone();
            move |event, window, _| {
                let over = layout
                    .index_for_position(event.position)
                    .is_ok_and(|ix| ix >= link_start);
                if over != over_link.replace(over) {
                    window.refresh();
                }
            }
        })
        .on_click(move |_, window, cx| {
            // The link goes to its target, not into editing
            if !over_link.get() {
                start_editing(window, cx);
            } else if !href.is_empty() && href != "#" {
                cx.open_url(&href);
            }
        })
        .child(styled);
    let paragraph = if tone_key > 0 {
        motion::fade_quick(("ed-text-in", tone_key as usize), paragraph).into_any_element()
    } else {
        paragraph.into_any_element()
    };
    wrap.child(paragraph)
}

/// The tone switch as Pink's segmented control (two to five fixed options)
/// with the pen beside it (a small icon button, on the selected fill while
/// editing), and while editing, Pink's key legend.
fn tone_row(ctx: &Ctx, t: &Pink, cx: &mut App) -> Div {
    let s = ctx.state.read(cx);
    let (tone, editing, pen_was_editing) = (s.tone.clone(), s.editing, s.pen_was_editing.clone());
    let edited = s.is_edited(&ctx.tones);
    let selected = ctx.tones.iter().position(|x| x.id == tone).unwrap_or(0);
    let segments = ctx
        .tones
        .iter()
        .map(|x| Segment::new(x.id.clone(), x.label.clone()))
        .collect();
    let switch = {
        let tones = ctx.tones.clone();
        let state = ctx.state.clone();
        pink::segmented(
            ctx.key("tone"),
            segments,
            selected,
            TONE_W,
            ControlSize::Sm,
            t,
            move |ix, _, cx| {
                if let Some(tone) = tones.get(*ix) {
                    let id = tone.id.clone();
                    state.update(cx, |s, cx| s.pick_tone(id, cx));
                }
            },
        )
    };
    let pen = {
        let state = ctx.state.clone();
        let start_editing = ctx.start_editing.clone();
        let was_editing = pen_was_editing.clone();
        pink::icon_button(ctx.key("pen"), ICON_PEN, ControlSize::Sm, t)
            .when(editing, |d| d.bg(t.fill(FillTier::Primary)))
            .tooltip(pink::text_tooltip(if editing {
                "Done editing"
            } else {
                "Edit"
            }))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |_, _, cx| pen_was_editing.set(state.read(cx).editing)
            })
            .on_click(move |_, window, cx| {
                if was_editing.replace(false) {
                    state.update(cx, |s, cx| s.stop_editing(cx));
                } else {
                    start_editing(window, cx);
                }
            })
    };
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(layout::ROW_PAD_X))
        .pt(px(12.))
        .pb(px(14.))
        .child(switch)
        .child(pen)
        .when(edited && !editing, |d| {
            d.child(
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .text_color(t.readable_muted(0.7))
                    .child("Edited"),
            )
        })
        .child(div().flex_1())
        .when(editing, |d| {
            d.child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(pink::key_hint_text("⌘↵", "Send", t))
                    .child(pink::key_hint_text("esc", "Done", t)),
            )
        })
}

/// The one-line receipt after send / discard: Pink's answered-question chip
/// (`transcript::input_chip`) laid bare in a card row – a 20 px tile
/// (`ink(0.09)`, r 6, a 12 px glyph in `content_muted`), the verdict in
/// medium `content_muted`, the detail in `text @ 0.9` – then Undo.
fn receipt(ctx: &Ctx, t: &Pink, phase: Phase, cx: &mut App) -> Div {
    let s = ctx.state.read(cx);
    let (to, file) = (s.to.clone(), s.file.clone());
    let sent = phase == Phase::Sent;
    let detail = match (sent, file) {
        (true, Some(file)) => format!("to {} with {file}", to.name),
        (true, None) => format!("to {}", to.name),
        (false, _) => format!("Draft to {}", to.name),
    };
    let undo_id = ctx.key("undo");
    let glyph = motion::hover_blend(
        &format!("pink-btn-{undo_id}"),
        t.readable_muted(0.95),
        t.text,
    );
    let state = ctx.state.clone();
    row(t)
        .gap(px(8.))
        .text_size(px(12.))
        .line_height(px(16.))
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
                        .path(if sent { ICON_CHECK } else { ICON_TRASH })
                        .size(px(icon_size::XS))
                        .flex_none()
                        .text_color(t.content_muted),
                ),
        )
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.content_muted)
                .child(if sent { "Sent" } else { "Discarded" }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(t.text.opacity(0.9))
                .child(detail),
        )
        .child(
            pink::button(undo_id, ButtonKind::Ghost, ControlSize::Sm, t)
                .on_click(move |_, _, cx| state.update(cx, |s, cx| s.undo(cx)))
                .child(
                    svg()
                        .path(ICON_UNDO)
                        .size(px(ControlSize::Sm.glyph()))
                        .flex_none()
                        .text_color(glyph),
                )
                .child("Undo"),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| EmailDraft::new("email-draft"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_new_name() {
        let jonas = DraftContact::new("Jonas Weber", "jonas@northwind.io");
        let lena = DraftContact::new("Lena Hart", "lena@northwind.io");
        // The first-name branch wins at "Jonas Weber", like the regex alternation
        assert_eq!(
            follow_name("Hi Jonas, Jonas Weber – Jonasson", &jonas, &lena),
            "Hi Lena, Lena Weber – Jonasson"
        );
        let finance = DraftContact::new("Finance", "billing@northwind.io");
        assert_eq!(
            follow_name("Dear Finance,", &finance, &lena),
            "Dear Lena Hart,"
        );
    }
}
