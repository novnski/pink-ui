//! Email Draft – Agent-written email for review – switch recipient and tone, edit inline, remove the attachment, send or discard with undo
//!
//! The record the email is about (reference, status, amount) heads it;
//! below sits the email: a recipient you can switch from a menu, an
//! attachment you can remove and add back, tone variants, inline editing
//! (click the text or the pencil; ⌘/Ctrl+Enter sends, Escape stops editing).
//! Sending or discarding folds the email into a one-line receipt with Undo,
//! and the email's part of the card glides to its new height.
//!
//! Clean: one white card with hairline shadows, the decision buttons in its
//! last row. Classic: the record and the footer sit on a gray tray around
//! the white email card, which holds the buttons.
//!
//! ```ignore
//! EmailDraft::new("renewal")
//!     .reference("SUB-1182")
//!     .on_send(|email, _, _| println!("to {}", email.to.email))
//! ```

use std::sync::Arc;
use std::{
    cell::Cell,
    f32::consts::PI,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AnyView, App, BoxShadow, Context, CursorStyle, Div, ElementId, Entity, FocusHandle,
    FontWeight, HighlightStyle, Hsla, IntoElement, KeyDownEvent, MouseDownEvent, PathBuilder,
    Pixels, Rgba, SharedString, StyledText, Subscription, Task, TextRun, Transformation,
    UnderlineStyle, Window, canvas, deferred, div, font, linear_color_stop, linear_gradient, point,
    prelude::*, px, radians, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, AnimationExt, EASE, EASE_OUT_QUINT, MotionSpec},
    primitives::{InputEvent, SANS, TextInput, black_a, color_mix, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Email Draft";
pub const DESCRIPTION: &str = "Agent-written email for review – switch recipient and tone, edit inline, remove the attachment, send or discard with undo";

/// The email ↔ receipt part gliding to its new height.
const HEIGHT_GLIDE: MotionSpec = MotionSpec::new(360, EASE_OUT_QUINT);
/// `ui-ed(c)-in`: the email or the receipt rising in.
const VIEW_IN: MotionSpec = MotionSpec::new(320, EASE_OUT_QUINT);
/// `ui-ed(c)-menu`: the recipient menu dropping in.
const MENU_IN: MotionSpec = MotionSpec::new(200, EASE_OUT_QUINT);
/// The chevron turning while the menu is open.
const CHEVRON_TURN: MotionSpec = MotionSpec::new(220, EASE_OUT_QUINT);
/// The email dimming while it sends.
const SENDING_DIM: MotionSpec = MotionSpec::new(220, EASE);
/// Fades: the text after a tone switch, the badge, the attach button.
const TEXT_FADE: MotionSpec = MotionSpec::new(280, EASE);
const BADGE_FADE: MotionSpec = MotionSpec::new(260, EASE);
const ATTACH_FADE: MotionSpec = MotionSpec::new(220, EASE);
/// The fake network round trip before the email counts as sent.
const SEND_DELAY: Duration = Duration::from_millis(900);
/// One turn of the Sending spinner.
const SPIN_MS: f32 = 700.;

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

    fn initials(&self) -> String {
        self.name
            .split(' ')
            .filter_map(|part| part.chars().next())
            .take(2)
            .collect()
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
// Colors
// ---------------------------------------------------------------------------

/// The `--ui-ed-*` / `--ui-edc-*` tokens, named by the role they play.
struct Palette {
    tray: Option<Hsla>,
    card: Hsla,
    card_shadow: Vec<BoxShadow>,
    /// Classic's 1px card edge.
    card_border: Option<Hsla>,
    line: Hsla,
    title: Hsla,
    account: Hsla,
    strong: Hsla,
    ink: Hsla,
    text: Hsla,
    muted: Hsla,
    hint: Hsla,
    /// Ghost-button hover fill (Clean `wash`, Classic `hover`).
    wash: Hsla,
    /// Pressed fill (Clean `fill`, Classic `chip`).
    fill: Hsla,
    chip_line: Hsla,
    recipient_bg: Hsla,
    recipient_hover: Hsla,
    chevron: Hsla,
    menu_shadow: Vec<BoxShadow>,
    option_hover: Hsla,
    avatar_bg: Hsla,
    avatar_fg: Hsla,
    tick: Hsla,
    file: Hsla,
    file_icon: Hsla,
    body: Hsla,
    body_hover: Hsla,
    link: Hsla,
    link_line: Hsla,
    link_line_hover: Hsla,
    edit_bg: Hsla,
    edit_border: Hsla,
    edit_fg: Hsla,
    caret: Option<Hsla>,
    tone_fg: Hsla,
    tone_hover_bg: Hsla,
    edit_btn_fg: Hsla,
    secondary_bg: Hsla,
    secondary_hover: Hsla,
    secondary_fg: Hsla,
    /// Send: a solid chip (Clean) or a top-to-bottom gradient (Classic).
    primary: (Hsla, Hsla),
    primary_hover: (Hsla, Hsla),
    primary_fg: Hsla,
    primary_ring: Option<Hsla>,
    ok: Hsla,
    ok_bg: Hsla,
    warn: Hsla,
    warn_bg: Hsla,
    quiet_bg: Hsla,
    quiet_fg: Hsla,
    receipt_icon: Hsla,
}

impl Palette {
    fn get(look: Look, dark: bool) -> Self {
        match (look, dark) {
            (Look::Clean, false) => Self::clean(CleanTokens {
                card: hex(0xffffff),
                strong: hex(0x000000),
                ink: black_a(0.88),
                text: black_a(0.61),
                muted: black_a(0.45),
                faint: black_a(0.27),
                icon: black_a(0.447),
                line: black_a(0.06),
                fill: black_a(0.06),
                wash: black_a(0.03),
                solid: black_a(0.88),
                on_solid: hex(0xfcfcfc),
                accent: hexa(0x0060e2, 0.765),
                ok: hex(0x1a9a52),
                ok_bg: hexa(0x1a9a52, 0.08),
                warn: hex(0xb47a06),
                warn_bg: hexa(0xe8a20c, 0.12),
                shadow: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                shadow_pop: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.08)),
                    shadow(0., 12., 28., -10., black_a(0.16)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                primary_hover: hex(0x000000),
            }),
            (Look::Clean, true) => Self::clean(CleanTokens {
                card: hex(0x1c1c1e),
                strong: hex(0xffffff),
                ink: white_a(0.9),
                text: white_a(0.62),
                muted: white_a(0.46),
                faint: white_a(0.3),
                icon: white_a(0.5),
                line: white_a(0.08),
                fill: white_a(0.08),
                wash: white_a(0.04),
                solid: white_a(0.92),
                on_solid: hex(0x161618),
                accent: hex(0x4d8dff),
                ok: hex(0x3ec27a),
                ok_bg: hexa(0x3ec27a, 0.14),
                warn: hex(0xf0b43c),
                warn_bg: hexa(0xe8a20c, 0.14),
                shadow: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                shadow_pop: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.12)),
                    shadow(0., 12., 28., -10., black_a(0.6)),
                ],
                primary_hover: hex(0xffffff),
            }),
            (Look::Classic, false) => Self::classic(ClassicTokens {
                tray: hex(0xf0f0f0),
                card: hex(0xffffff),
                card_line: hex(0xe5e5e5),
                line: hex(0xececec),
                ink: hex(0x171717),
                text: hex(0x404040),
                muted: hex(0x8f8f8f),
                chip: hex(0xf5f5f5),
                chip_line: hex(0xebebeb),
                hover: hex(0xf5f5f5),
                warn: hex(0xc2570c),
                warn_bg: hex(0xfdeedd),
                ok: hex(0x2f7d32),
                ok_bg: hex(0xe8f5e6),
                btn_top: hex(0x323137),
                btn_bottom: hex(0x201e25),
                btn_ring: hex(0x0d0d0d),
                btn_text: hex(0xffffff),
                shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
            }),
            (Look::Classic, true) => Self::classic(ClassicTokens {
                tray: hex(0x161618),
                card: hex(0x232326),
                card_line: white_a(0.08),
                line: white_a(0.07),
                ink: hex(0xededed),
                text: hex(0xc4c4c4),
                muted: hex(0x8f8f8f),
                chip: hex(0x2a2a2d),
                chip_line: white_a(0.08),
                hover: white_a(0.05),
                warn: hex(0xfb9a4b),
                warn_bg: hexa(0xfb9a4b, 0.14),
                ok: hex(0x6fd27a),
                ok_bg: hexa(0x6fd27a, 0.14),
                btn_top: hex(0xf4f4f5),
                btn_bottom: hex(0xe4e4e7),
                btn_ring: black_a(0.6),
                btn_text: hex(0x121213),
                shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
            }),
        }
    }

    fn clean(t: CleanTokens) -> Self {
        Palette {
            tray: None,
            card: t.card,
            card_shadow: t.shadow,
            card_border: None,
            line: t.line,
            title: t.strong,
            account: t.text,
            strong: t.strong,
            ink: t.ink,
            text: t.text,
            muted: t.muted,
            hint: t.faint,
            wash: t.wash,
            fill: t.fill,
            chip_line: t.line,
            recipient_bg: t.wash.opacity(0.),
            recipient_hover: t.wash,
            chevron: t.icon,
            menu_shadow: t.shadow_pop,
            option_hover: t.fill,
            avatar_bg: t.fill,
            avatar_fg: t.text,
            tick: t.accent,
            file: t.text,
            file_icon: t.icon,
            body: t.ink,
            body_hover: t.wash,
            link: t.strong,
            link_line: t.faint,
            link_line_hover: t.strong,
            edit_bg: t.wash,
            edit_border: t.line,
            edit_fg: t.strong,
            caret: Some(t.accent),
            tone_fg: t.text,
            tone_hover_bg: t.wash,
            edit_btn_fg: t.icon,
            secondary_bg: t.card,
            secondary_hover: over(t.wash, t.card),
            secondary_fg: t.strong,
            primary: (t.solid, t.solid),
            primary_hover: (t.primary_hover, t.primary_hover),
            primary_fg: t.on_solid,
            primary_ring: None,
            ok: t.ok,
            ok_bg: t.ok_bg,
            warn: t.warn,
            warn_bg: t.warn_bg,
            quiet_bg: t.fill,
            quiet_fg: t.text,
            receipt_icon: t.icon,
        }
    }

    fn classic(t: ClassicTokens) -> Self {
        // color-mix(in srgb, chip 80%, ink 6%): the shares are normalised and
        // the missing 14% becomes transparency
        let recipient_hover = color_mix(t.chip, t.ink, 0.8 / 0.86);
        let recipient_hover = recipient_hover.opacity(0.86);
        Palette {
            tray: Some(t.tray),
            card: t.card,
            card_shadow: t.shadow,
            card_border: Some(t.card_line),
            line: t.line,
            title: t.ink,
            account: t.muted,
            strong: t.ink,
            ink: t.ink,
            text: t.text,
            muted: t.muted,
            hint: t.muted,
            wash: t.hover,
            fill: t.chip,
            chip_line: t.chip_line,
            recipient_bg: t.chip,
            recipient_hover,
            chevron: t.muted,
            menu_shadow: vec![
                shadow(0., 0., 0., 1., t.card_line),
                shadow(0., 10., 30., 0., black_a(0.1)),
            ],
            option_hover: t.hover,
            avatar_bg: t.chip,
            avatar_fg: t.text,
            tick: t.ink,
            file: t.muted,
            file_icon: t.muted,
            body: t.text,
            body_hover: t.hover,
            link: t.text,
            link_line: t.text.opacity(0.35),
            link_line_hover: t.text,
            edit_bg: t.hover,
            edit_border: t.chip_line,
            edit_fg: t.ink,
            caret: None,
            tone_fg: t.muted,
            tone_hover_bg: t.hover.opacity(0.),
            edit_btn_fg: t.muted,
            secondary_bg: t.card,
            secondary_hover: t.hover,
            secondary_fg: t.text,
            primary: (t.btn_top, t.btn_bottom),
            primary_hover: (brighten(t.btn_top, 1.12), brighten(t.btn_bottom, 1.12)),
            primary_fg: t.btn_text,
            primary_ring: Some(t.btn_ring),
            ok: t.ok,
            ok_bg: t.ok_bg,
            warn: t.warn,
            warn_bg: t.warn_bg,
            quiet_bg: t.chip,
            quiet_fg: t.muted,
            receipt_icon: t.muted,
        }
    }
}

/// `--ui-edc-*`, verbatim.
struct CleanTokens {
    card: Hsla,
    strong: Hsla,
    ink: Hsla,
    text: Hsla,
    muted: Hsla,
    faint: Hsla,
    icon: Hsla,
    line: Hsla,
    fill: Hsla,
    wash: Hsla,
    solid: Hsla,
    on_solid: Hsla,
    accent: Hsla,
    ok: Hsla,
    ok_bg: Hsla,
    warn: Hsla,
    warn_bg: Hsla,
    shadow: Vec<BoxShadow>,
    shadow_pop: Vec<BoxShadow>,
    primary_hover: Hsla,
}

/// `--ui-ed-*`, verbatim.
struct ClassicTokens {
    tray: Hsla,
    card: Hsla,
    card_line: Hsla,
    line: Hsla,
    ink: Hsla,
    text: Hsla,
    muted: Hsla,
    chip: Hsla,
    chip_line: Hsla,
    hover: Hsla,
    warn: Hsla,
    warn_bg: Hsla,
    ok: Hsla,
    ok_bg: Hsla,
    btn_top: Hsla,
    btn_bottom: Hsla,
    btn_ring: Hsla,
    btn_text: Hsla,
    shadow: Vec<BoxShadow>,
}

/// `top` painted over `bottom` – `linear-gradient(wash, wash), card`.
fn over(top: Hsla, bottom: Hsla) -> Hsla {
    let (t, b) = (Rgba::from(top), Rgba::from(bottom));
    let a = t.a + b.a * (1. - t.a);
    if a <= f32::EPSILON {
        return top;
    }
    let channel = |x: f32, y: f32| (x * t.a + y * b.a * (1. - t.a)) / a;
    Rgba {
        r: channel(t.r, b.r),
        g: channel(t.g, b.g),
        b: channel(t.b, b.b),
        a,
    }
    .into()
}

/// `filter: brightness(k)`.
fn brighten(color: Hsla, k: f32) -> Hsla {
    let c = Rgba::from(color);
    Rgba {
        r: (c.r * k).min(1.),
        g: (c.g * k).min(1.),
        b: (c.b * k).min(1.),
        a: c.a,
    }
    .into()
}

/// A 0.5px CSS hairline in px: one device pixel at 1x, half a px on HiDPI.
fn hairline(window: &Window) -> f32 {
    0.5f32.max(1. / window.scale_factor())
}

/// Classic sets no line heights: its text takes the site's `line-height: 1.5`.
fn inherited(size: f32) -> Pixels {
    px(size * 1.5)
}

/// Keeps a run of text on one line and clips it. GPUI caches a text's first
/// measurement, so `text_ellipsis` inside a shrinking flex item can lock in
/// a truncated width; plain clipping measures true.
fn clip<E: Styled>(element: E) -> E {
    element.overflow_hidden().whitespace_nowrap()
}

fn icon(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/email-draft/clean/{name}.svg").into(),
        Look::Classic => format!("icons/email-draft/{name}.svg").into(),
    }
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
    phase: Phase,
    /// Bumps on every tone pick, replaying the text's fade.
    tone_key: u64,
    /// Bumps whenever the email and the receipt swap, replaying their entrance.
    view_key: u64,
    sending_since: Option<Instant>,
    input: Entity<TextInput>,
    menu_focus: FocusHandle,
    options: Vec<FocusHandle>,
    /// The natural height of the email / receipt, measured after layout.
    content_height: Rc<Cell<Option<Pixels>>>,
    /// The text's width, measured after layout, to size the edit field.
    text_width: Rc<Cell<Option<Pixels>>>,
    /// Whether the pointer is on the recipient switch, so pressing it while
    /// the menu is open toggles the menu instead of counting as outside.
    over_recipient: Rc<Cell<bool>>,
    over_link: Rc<Cell<bool>>,
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
            phase: Phase::Draft,
            tone_key: 0,
            view_key: 0,
            sending_since: None,
            input,
            menu_focus: cx.focus_handle(),
            options: Vec::new(),
            content_height: Rc::new(Cell::new(None)),
            text_width: Rc::new(Cell::new(None)),
            over_recipient: Rc::new(Cell::new(false)),
            over_link: Rc::new(Cell::new(false)),
            _send: None,
            _input_events: input_events,
        }
    }

    fn body(&self, tones: &[DraftTone]) -> String {
        self.edited.clone().unwrap_or_else(|| {
            tones
                .iter()
                .find(|t| t.id == self.tone)
                .or(tones.first())
                .map(|t| t.body(&self.to))
                .unwrap_or_default()
        })
    }

    fn start_editing(&mut self, tones: &[DraftTone], window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != Phase::Draft {
            return;
        }
        let body = self.body(tones);
        self.edited = Some(body.clone());
        self.editing = true;
        self.menu = false;
        self.input.update(cx, |input, cx| input.set_text(body, cx));
        TextInput::focus(&self.input, window, cx);
        cx.notify();
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

    fn pick_contact(&mut self, contact: DraftContact, cx: &mut Context<Self>) {
        // Keep manual edits, but follow the new name in them
        if let Some(edited) = &self.edited {
            self.edited = Some(follow_name(edited, &self.to, &contact));
        }
        self.to = contact;
        self.menu = false;
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
        self.menu = false;
        self.phase = Phase::Sending;
        self.sending_since = Some(Instant::now());
        self._send = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(SEND_DELAY).await;
            this.update_in(cx, |this, window, cx| {
                this.phase = Phase::Sent;
                this.sending_since = None;
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
        self.menu = false;
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
    look: Option<Look>,
}

impl EmailDraft {
    /// A draft with affectUI's demo content; override any of it below.
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
            look: None,
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

    /// Status chip next to the reference.
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

    /// Overrides the global Clean/Classic look for this draft.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

/// What every part of the draft needs to render.
struct Ctx {
    key: String,
    look: Look,
    clean: bool,
    reduced: bool,
    state: Entity<DraftState>,
    tones: Rc<[DraftTone]>,
    send: Handler,
    start_editing: Handler,
    on_discard: Option<Handler>,
    /// Clean's 0.5px hairline, at least one device pixel – like Chrome
    /// draws it (GPUI drops a 0.5px border side at 1x).
    hair: f32,
}

impl Ctx {
    fn hover_key(&self, part: &str) -> String {
        format!("{}-{part}", self.key)
    }

    /// `inset 0 0 0 0.5px color` as a `hair`-wide border: browsers paint the
    /// inset ring antialiased, so a 1px stand-in carries half the color.
    fn ring(&self, color: Hsla) -> Hsla {
        color.opacity(0.5 / self.hair)
    }
}

impl RenderOnce for EmailDraft {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx.is_dark());
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);

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
        let caret = p.caret;
        let option_count = contacts.len();
        state.update(cx, |s, cx| {
            while s.options.len() < option_count {
                s.options.push(cx.focus_handle());
            }
            s.input
                .update(cx, |input, _| input.set_colors(None, caret, None));
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
            key: key.clone(),
            look,
            clean,
            reduced,
            state: state.clone(),
            tones,
            send,
            start_editing,
            on_discard: self.on_discard.clone(),
            hair: hairline(window),
        };

        let (phase, view_key, content_height, sending_since) = {
            let s = state.read(cx);
            (
                s.phase,
                s.view_key,
                s.content_height.clone(),
                s.sending_since,
            )
        };
        let open = matches!(phase, Phase::Draft | Phase::Sending);

        // Email ↔ receipt: only this part glides to its new height
        let content: AnyElement = if open {
            let dim = motion::follow(
                &ctx.hover_key("sending"),
                if phase == Phase::Sending {
                    if clean { 0.6 } else { 0.7 }
                } else {
                    1.
                },
                SENDING_DIM,
            );
            let draft = self.draft(&ctx, &p, &contacts, window, cx).opacity(dim);
            entrance(("draft", view_key), reduced, div().child(draft))
        } else {
            entrance(("receipt", view_key), reduced, receipt(&ctx, &p, phase, cx))
        };
        let measured = content_height.get();
        let height =
            measured.map(|h| motion::follow(&ctx.hover_key("height"), f32::from(h), HEIGHT_GLIDE));
        let outer = div()
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

        if phase == Phase::Sending || sending_since.is_some() {
            window.request_animation_frame();
        }
        motion::keep_alive(window);

        let root = div()
            .id(self.id.clone())
            .w_full()
            .font_family(SANS)
            .text_color(p.ink);
        let record = self.record(&p, clean, ctx.hair);
        let foot = self.foot(&ctx, &p, phase, sending_since);
        match look {
            Look::Clean => root
                .flex()
                .flex_col()
                .max_w(px(480.))
                .rounded(px(10.))
                .bg(p.card)
                .shadow(p.card_shadow.clone())
                .text_size(px(13.))
                .line_height(px(20.))
                .child(record)
                .child(outer)
                .child(foot),
            Look::Classic => root
                .max_w(px(520.))
                .p(px(6.))
                .rounded(px(20.))
                .when_some(p.tray, |d, tray| d.bg(tray))
                .child(record)
                .child(outer)
                .child(foot),
        }
    }
}

impl EmailDraft {
    /// The record the email is about.
    fn record(&self, p: &Palette, clean: bool, hair: f32) -> Div {
        let title = |text: SharedString| {
            div()
                .flex_none()
                .text_size(px(15.))
                .font_weight(FontWeight::MEDIUM)
                .line_height(if clean { px(24.) } else { inherited(15.) })
                .text_color(p.title)
                .child(text)
        };
        let status = clip(div())
            .min_w_0()
            .py(px(2.))
            .bg(p.warn_bg)
            .text_color(p.warn)
            .font_weight(FontWeight::MEDIUM)
            .when(clean, |d| {
                d.px(px(6.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .line_height(px(16.))
            })
            .when(!clean, |d| {
                d.px(px(8.))
                    .rounded(px(8.))
                    .text_size(px(12.5))
                    .line_height(inherited(12.5))
            })
            .child(self.status.clone());
        let row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .min_w_0()
            .child(title(self.reference.clone()))
            .child(status)
            .child(div().flex_1())
            .child(title(self.amount.clone()));
        let account = div()
            .text_color(p.account)
            .when(clean, |d| d.text_size(px(12.)).line_height(px(16.)))
            .when(!clean, |d| d.text_size(px(13.)).line_height(inherited(13.)))
            .child(self.account.clone());
        let record = div().flex().flex_col().child(row).child(account);
        if clean {
            record
                .px(px(16.))
                .py(px(12.))
                .border_b(px(hair))
                .border_color(p.line)
        } else {
            record.gap(px(2.)).pt(px(8.)).px(px(10.)).pb(px(10.))
        }
    }

    /// The email: recipient and attachment, the text, tones (and, in Classic, the buttons).
    fn draft(
        &self,
        ctx: &Ctx,
        p: &Palette,
        contacts: &Rc<[DraftContact]>,
        window: &mut Window,
        cx: &mut App,
    ) -> Div {
        let clean = ctx.clean;
        let meta = div()
            .flex()
            .min_w_0()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(recipient(ctx, p, contacts, cx))
            .children(attachment(ctx, p, self.attachment.clone(), cx));
        let meta = if clean {
            meta.pt(px(10.))
                .pb(px(10.))
                .pl(px(16.))
                .pr(px(12.))
                .border_b(px(ctx.hair))
                .border_color(p.line)
        } else {
            meta.mx(px(14.))
                .pt(px(12.))
                .pb(px(11.))
                .border_b_1()
                .border_color(p.line)
        };

        let text_width = ctx.state.read(cx).text_width.clone();
        let body = div()
            .relative()
            .child(body(ctx, p, &self.link, window, cx))
            .child(measure_width(text_width));
        let body = if clean {
            div().pt(px(14.)).px(px(16.)).pb(px(2.)).child(body)
        } else {
            div().pt(px(12.)).px(px(14.)).pb(px(4.)).child(body)
        };

        let tones = tone_buttons(ctx, p, cx);
        let draft = div().child(meta).child(body);
        if clean {
            draft.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .pt(px(14.))
                    .pb(px(14.))
                    .pl(px(16.))
                    .pr(px(12.))
                    .child(tones)
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .whitespace_nowrap()
                            .text_color(p.hint)
                            .child("⌘ Enter to send"),
                    ),
            )
        } else {
            let (phase, since) = {
                let s = ctx.state.read(cx);
                (s.phase, s.sending_since)
            };
            div()
                .border_1()
                .border_color(p.card_border.unwrap_or(p.line))
                .rounded(px(16.))
                .bg(p.card)
                .shadow(p.card_shadow.clone())
                .child(draft)
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap(px(10.))
                        .pt(px(10.))
                        .pb(px(10.))
                        .pl(px(14.))
                        .pr(px(10.))
                        .child(tones)
                        .child(decision_buttons(ctx, p, phase, since)),
                )
        }
    }

    /// The last row: who wrote it, its state and (Clean) the decision.
    fn foot(&self, ctx: &Ctx, p: &Palette, phase: Phase, since: Option<Instant>) -> Div {
        let clean = ctx.clean;
        let open = matches!(phase, Phase::Draft | Phase::Sending);
        let (text, ok) = match phase {
            Phase::Sent => ("Sent", true),
            Phase::Discarded => ("Discarded", false),
            _ => ("Ready for review", true),
        };
        let badge = div()
            .flex_none()
            .py(px(2.))
            .font_weight(FontWeight::MEDIUM)
            .whitespace_nowrap()
            .bg(if ok { p.ok_bg } else { p.quiet_bg })
            .text_color(if ok { p.ok } else { p.quiet_fg })
            .when(clean, |d| {
                d.px(px(6.))
                    .rounded(px(5.))
                    .text_size(px(12.))
                    .line_height(px(16.))
            })
            .when(!clean, |d| {
                d.px(px(8.))
                    .rounded(px(8.))
                    .text_size(px(12.5))
                    .line_height(inherited(12.5))
            })
            .child(text);
        let badge = fade(
            ElementId::Name(format!("badge-{text}").into()),
            BADGE_FADE,
            ctx.reduced,
            badge,
        );
        let agent = clip(div())
            .min_w_0()
            .text_color(p.ink)
            .child(self.agent.clone());
        if clean {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .min_h(px(48.))
                .pt(px(10.))
                .pb(px(10.))
                .pl(px(16.))
                .pr(px(10.))
                .border_t(px(ctx.hair))
                .border_color(p.line)
                .child(agent.text_size(px(13.)))
                .child(badge)
                .child(div().flex_1())
                .when(open, |d| d.child(decision_buttons(ctx, p, phase, since)))
        } else {
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .pt(px(10.))
                .px(px(10.))
                .pb(px(6.))
                .child(agent.text_size(px(14.)).line_height(inherited(14.)))
                .child(badge)
                .child(div().flex_1())
                .when(open, |d| {
                    d.child(
                        div()
                            .flex_none()
                            .text_size(px(12.))
                            .line_height(inherited(12.))
                            .text_color(p.muted)
                            .child("⌘ Enter to send"),
                    )
                })
        }
    }
}

/// `opacity 0 → 1, translateY(4px) → 0` each time `id` changes.
fn entrance(
    id: impl Into<ElementId>,
    reduced: bool,
    element: impl IntoElement + Styled + 'static,
) -> AnyElement {
    if reduced {
        return element.into_any_element();
    }
    motion::rise_in(id, VIEW_IN, 4., element).into_any_element()
}

/// Opacity 0 → 1 each time `id` changes.
fn fade(
    id: impl Into<ElementId>,
    spec: MotionSpec,
    reduced: bool,
    element: impl IntoElement + Styled + 'static,
) -> AnyElement {
    if reduced {
        return element.into_any_element();
    }
    element
        .with_animation(id, spec.animation(), |el, t| el.opacity(t))
        .into_any_element()
}

/// An invisible layer filling its parent that records the parent's width.
fn measure_width(cell: Rc<Cell<Option<Pixels>>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let width = bounds.size.width;
            if cell.get() != Some(width) {
                cell.set(Some(width));
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

/// "To" with the recipient switch and its menu.
fn recipient(ctx: &Ctx, p: &Palette, contacts: &Rc<[DraftContact]>, cx: &mut App) -> Div {
    let clean = ctx.clean;
    let s = ctx.state.read(cx);
    let (to, menu, sending) = (s.to.clone(), s.menu, s.phase == Phase::Sending);
    let (menu_focus, options, over_button) = (
        s.menu_focus.clone(),
        s.options.clone(),
        s.over_recipient.clone(),
    );

    let button_key = ctx.hover_key("recipient");
    let turn = motion::follow(
        &ctx.hover_key("chevron"),
        if menu { PI } else { 0. },
        CHEVRON_TURN,
    );
    let chevron = svg()
        .path(icon(ctx.look, "chevron"))
        .size(px(if clean { 16. } else { 14. }))
        .flex_none()
        .text_color(p.chevron)
        .with_transformation(Transformation::rotate(radians(turn)));
    let email = clip(div())
        .min_w_0()
        .font_weight(FontWeight::NORMAL)
        .text_color(p.muted)
        .child(if clean {
            to.email.clone()
        } else {
            format!("({})", to.email).into()
        });
    let toggle = {
        let state = ctx.state.clone();
        let menu_focus = menu_focus.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
            let opened = state.update(cx, |s, cx| {
                if s.phase != Phase::Draft {
                    return false;
                }
                s.menu = !s.menu;
                cx.notify();
                s.menu
            });
            if opened {
                window.focus(&menu_focus, cx);
            }
        }
    };
    let (rest, hover) = match (clean, menu) {
        // Clean keeps the hover wash while the menu is open
        (true, true) => (p.recipient_hover, p.recipient_hover),
        _ => (p.recipient_bg, p.recipient_hover),
    };
    let reduced = ctx.reduced;
    let button = div()
        .id("recipient")
        .flex()
        .items_center()
        .min_w_0()
        .overflow_hidden()
        .whitespace_nowrap()
        .cursor_pointer()
        .text_size(px(13.))
        .text_color(p.ink)
        .bg(motion::hover_blend(&button_key, rest, hover))
        .on_hover({
            let over_button = over_button.clone();
            move |hovered, window, _| {
                motion::set_hover(&button_key, *hovered, reduced);
                over_button.set(*hovered);
                window.refresh();
            }
        })
        .when(!sending, |d| d.on_click(toggle));
    let button = if clean {
        // The button-switch look: a hairline, the address in gray
        button
            .gap(px(6.))
            .h(px(28.))
            .pl(px(10. - ctx.hair))
            .pr(px(6. - ctx.hair))
            .rounded(px(6.))
            .border(px(ctx.hair))
            .border_color(ctx.ring(p.line))
            .font_weight(FontWeight::MEDIUM)
            .line_height(px(20.))
            .child(div().flex_none().child(to.name.clone()))
    } else {
        button
            .gap(px(4.))
            .h(px(26.))
            .pl(px(7.))
            .pr(px(5.))
            .rounded(px(8.))
            .border_1()
            .border_color(p.chip_line)
            .line_height(inherited(13.))
            .child(div().flex_none().child(to.name.clone()))
    }
    .child(email)
    .child(chevron);

    let menu_el = menu.then(|| {
        let rows = contacts.iter().enumerate().map(|(ix, contact)| {
            let selected = contact.email == to.email;
            let row_key = ctx.hover_key(&format!("option-{ix}"));
            let pick = {
                let state = ctx.state.clone();
                let contact = contact.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| s.pick_contact(contact.clone(), cx))
                }
            };
            let avatar = div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .bg(p.avatar_bg)
                .text_color(p.avatar_fg)
                .child(contact.initials());
            let avatar = if clean {
                avatar
                    .size(px(28.))
                    .rounded(px(6.))
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
            } else {
                avatar
                    .size(px(26.))
                    .rounded_full()
                    .border_1()
                    .border_color(p.chip_line)
                    .text_size(px(10.5))
                    .font_weight(FontWeight::SEMIBOLD)
            };
            let text = div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .line_height(if clean { px(20.) } else { inherited(13.) })
                        .child(contact.name.clone()),
                )
                .child(
                    clip(div())
                        .text_size(px(12.))
                        .line_height(if clean { px(16.) } else { inherited(12.) })
                        .text_color(p.muted)
                        .child(contact.email.clone()),
                );
            let mut row = div()
                .id(("option", ix))
                .flex()
                .items_center()
                .gap(px(10.))
                .w_full()
                .px(px(8.))
                .py(px(if clean { 6. } else { 7. }))
                .rounded(px(if clean { 6. } else { 8. }))
                .text_size(px(13.))
                .text_color(p.ink)
                .cursor_pointer()
                .hover_fill(row_key, p.option_hover.opacity(0.), p.option_hover)
                .focus(|s| s.bg(p.option_hover))
                .on_click(pick)
                .child(avatar)
                .child(text);
            if let Some(handle) = options.get(ix) {
                row = row.track_focus(handle);
            }
            row.when(selected, |d| {
                d.child(
                    svg()
                        .path(icon(ctx.look, "tick"))
                        .size(px(if clean { 16. } else { 15. }))
                        .flex_none()
                        .text_color(p.tick),
                )
            })
        });

        let on_key = {
            let state = ctx.state.clone();
            let options = options.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let len = options.len();
                let current = options.iter().position(|f| f.is_focused(window));
                let next = match event.keystroke.key.as_str() {
                    "escape" => {
                        state.update(cx, |s, cx| {
                            s.menu = false;
                            cx.notify()
                        });
                        cx.stop_propagation();
                        return;
                    }
                    _ if len == 0 => return,
                    "down" => current.map_or(0, |i| (i + 1) % len),
                    "up" => current.map_or(len - 1, |i| (i + len - 1) % len),
                    "home" => 0,
                    "end" => len - 1,
                    _ => return,
                };
                window.focus(&options[next], cx);
                cx.stop_propagation();
            }
        };
        let close_outside = {
            let state = ctx.state.clone();
            move |_: &MouseDownEvent, _: &mut Window, cx: &mut App| {
                // A press on the switch itself toggles it instead
                if over_button.get() {
                    return;
                }
                state.update(cx, |s, cx| {
                    s.menu = false;
                    cx.notify()
                });
            }
        };
        let top = if clean { 34. } else { 32. };
        let panel = div()
            .id("menu")
            .track_focus(&menu_focus)
            .absolute()
            .top(px(top))
            .left(px(if clean { 26. } else { 18. }))
            .w(px(if clean { 260. } else { 250. }))
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(if clean { 4. } else { 5. }))
            .rounded(px(if clean { 10. } else { 12. }))
            .bg(p.card)
            .shadow(p.menu_shadow.clone())
            .occlude()
            .on_key_down(on_key)
            .on_mouse_down_out(close_outside)
            .children(rows);
        let reduced = ctx.reduced;
        deferred(
            panel.with_animation("menu-in", MENU_IN.animation(), move |el, t| {
                if reduced {
                    el
                } else {
                    el.opacity(t).top(px(top - 4. * (1. - t)))
                }
            }),
        )
        .with_priority(1)
    });

    div()
        .relative()
        .flex()
        .items_center()
        .gap(px(if clean { 8. } else { 6. }))
        .min_w_0()
        .child(
            div()
                .flex_none()
                .text_size(px(13.))
                .line_height(if clean { px(20.) } else { inherited(13.) })
                .text_color(p.muted)
                .child(if clean { "To" } else { "to" }),
        )
        .child(button)
        .children(menu_el)
}

/// The attachment with its remove button, or a button to add it back.
fn attachment(
    ctx: &Ctx,
    p: &Palette,
    original: Option<SharedString>,
    cx: &mut App,
) -> Option<AnyElement> {
    let clean = ctx.clean;
    let s = ctx.state.read(cx);
    let (file, sending) = (s.file.clone(), s.phase == Phase::Sending);
    if let Some(file) = file {
        let remove_key = ctx.hover_key("remove");
        let remove = {
            let state = ctx.state.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| {
                    s.file = None;
                    cx.notify()
                })
            }
        };
        let rest_bg = p.wash.opacity(0.);
        let x_color = motion::hover_blend(&remove_key, p.file_icon, p.ink);
        let button = div()
            .id("remove")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .cursor_pointer()
            .hover_fill(remove_key, rest_bg, p.wash)
            .when(!sending, |d| d.on_click(remove))
            .when(clean, |d| d.size(px(24.)))
            .when(!clean, |d| d.size(px(20.)))
            .child(
                svg()
                    .path(icon(ctx.look, "x"))
                    .size(px(if clean { 14. } else { 13. }))
                    .text_color(x_color),
            );
        return Some(
            div()
                .flex()
                .items_center()
                .gap(px(if clean { 6. } else { 5. }))
                .min_w_0()
                .whitespace_nowrap()
                .text_size(px(13.))
                .text_color(p.file)
                .child(
                    svg()
                        .path(icon(ctx.look, "paperclip"))
                        .size(px(if clean { 16. } else { 15. }))
                        .flex_none()
                        .text_color(p.file_icon),
                )
                // The remove button tucks 2px closer (`margin-left: -2px`);
                // a narrower gap does it, as taffy mis-sizes negative margins
                .child(
                    div()
                        .flex()
                        .items_center()
                        .min_w_0()
                        .gap(px(if clean { 4. } else { 5. }))
                        .child(clip(div()).min_w_0().child(file))
                        .child(button),
                )
                .into_any_element(),
        );
    }

    let original = original?;
    let attach_key = ctx.hover_key("attach");
    let attach = {
        let state = ctx.state.clone();
        let original = original.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                s.file = Some(original.clone());
                cx.notify()
            })
        }
    };
    let rest_bg = p.wash.opacity(0.);
    // Clean keeps the plus gray; Classic's follows the label
    let plus_color = if clean {
        p.file_icon
    } else {
        motion::hover_blend(&attach_key, p.file, p.ink)
    };
    let button = div()
        .id("attach")
        .flex()
        .items_center()
        .min_w_0()
        .whitespace_nowrap()
        .text_size(px(13.))
        .cursor_pointer()
        .hover_fill_fg(attach_key, rest_bg, p.wash, p.file, p.ink)
        .when(!sending, |d| d.on_click(attach))
        .when(clean, |d| {
            d.gap(px(6.))
                .h(px(28.))
                .pl(px(6.))
                .pr(px(8.))
                .rounded(px(6.))
        })
        .when(!clean, |d| {
            d.gap(px(4.))
                .px(px(6.))
                .py(px(3.))
                .rounded(px(8.))
                .line_height(inherited(13.))
        })
        .child(
            svg()
                .path(icon(ctx.look, "plus"))
                .size(px(if clean { 16. } else { 15. }))
                .flex_none()
                .text_color(plus_color),
        )
        .child(clip(div()).min_w_0().child(format!("Attach {original}")));
    Some(fade("attach-in", ATTACH_FADE, ctx.reduced, button))
}

/// The email text – click it to edit – or the edit field.
fn body(ctx: &Ctx, p: &Palette, link: &DraftLink, window: &mut Window, cx: &mut App) -> AnyElement {
    let clean = ctx.clean;
    let s = ctx.state.read(cx);
    let (editing, tone_key, sending) = (s.editing, s.tone_key, s.phase == Phase::Sending);
    let text = s.body(&ctx.tones);
    let (input, over_link, width) = (s.input.clone(), s.over_link.clone(), s.text_width.get());
    let (size, line, radius) = if clean {
        (px(13.), px(20.), px(6.))
    } else {
        (px(15.), px(24.), px(8.))
    };

    if editing {
        // The field grows with its text. TextInput measures its rows with the
        // window's root text style, so size it here from the real font.
        let rows = width.map(|width| {
            let run = TextRun {
                len: text.len(),
                font: font(SANS),
                color: p.edit_fg,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_text(text.clone().into(), size, &[run], Some(width), None)
                .map(|lines| {
                    lines
                        .iter()
                        .map(|line| line.wrap_boundaries().len() + 1)
                        .sum::<usize>()
                })
                .unwrap_or(1)
                .max(1)
        });
        let border = if clean { ctx.hair } else { 1. };
        let send = ctx.send.clone();
        let state = ctx.state.clone();
        return div()
            .mx(px(-6.))
            .my(px(-4.))
            .px(px(6. - border))
            .py(px(4. - border))
            .rounded(radius)
            .bg(p.edit_bg)
            .border(px(border))
            .border_color(if clean {
                ctx.ring(p.edit_border)
            } else {
                p.edit_border
            })
            .text_size(size)
            .line_height(line)
            .text_color(p.edit_fg)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let keys = &event.keystroke;
                if keys.key == "enter" && (keys.modifiers.platform || keys.modifiers.control) {
                    send(window, cx);
                    cx.stop_propagation();
                }
            })
            // Like the textarea's blur: a press anywhere else ends editing
            .on_mouse_down_out(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if s.editing {
                        s.editing = false;
                        cx.notify();
                    }
                })
            })
            .child(
                div()
                    .when_some(rows, |d, rows| d.h(line * rows as f32).overflow_hidden())
                    .child(input),
            )
            .into_any_element();
    }

    let link_start = text.len() + 1;
    let full = format!("{text} {}", link.label);
    let link_end = full.len();
    let link_key = ctx.hover_key("link");
    let link_t = motion::hover_t(&link_key);
    let styled = StyledText::new(full).with_highlights([(
        link_start..link_end,
        HighlightStyle {
            color: Some(p.link),
            font_weight: clean.then_some(FontWeight::MEDIUM),
            underline: Some(UnderlineStyle {
                color: Some(motion::mix(p.link_line, p.link_line_hover, link_t)),
                // 1.5px snaps to whole device pixels, as browsers draw it
                thickness: px(if clean {
                    1.
                } else {
                    (1.5 / ctx.hair).round() * ctx.hair
                }),
                wavy: false,
            }),
            ..Default::default()
        },
    )]);
    let layout = styled.layout().clone();
    let text_key = ctx.hover_key("text");
    let reduced = ctx.reduced;
    let start_editing = ctx.start_editing.clone();

    let paragraph = div()
        .id("text")
        .mx(px(-6.))
        .my(px(-4.))
        .px(px(6.))
        .py(px(4.))
        .rounded(radius)
        .text_size(size)
        .line_height(line)
        .text_color(p.body)
        .cursor(if over_link.get() {
            CursorStyle::PointingHand
        } else {
            CursorStyle::IBeam
        })
        .bg(motion::hover_blend(
            &text_key,
            p.body_hover.opacity(0.),
            p.body_hover,
        ))
        .on_hover({
            let over_link = over_link.clone();
            let link_key = link_key.clone();
            move |hovered, window, _| {
                motion::set_hover(&text_key, *hovered, reduced);
                if !hovered && over_link.replace(false) {
                    motion::set_hover(&link_key, false, reduced);
                }
                window.refresh();
            }
        })
        .on_mouse_move({
            let over_link = over_link.clone();
            move |event, window, _| {
                let over = layout
                    .index_for_position(event.position)
                    .is_ok_and(|ix| ix >= link_start);
                if over != over_link.replace(over) {
                    motion::set_hover(&link_key, over, reduced);
                    window.refresh();
                }
            }
        })
        .when(!sending, |d| {
            d.on_click(move |_, window, cx| {
                // The link itself goes to its target, not into editing
                if !over_link.get() {
                    start_editing(window, cx);
                }
            })
        })
        .child(styled);
    fade(("text-in", tone_key), TEXT_FADE, ctx.reduced, paragraph)
}

/// Friendly / Short / Formal and the pencil.
fn tone_buttons(ctx: &Ctx, p: &Palette, cx: &mut App) -> Div {
    let clean = ctx.clean;
    let s = ctx.state.read(cx);
    let (tone, edited, editing, sending) = (
        s.tone.clone(),
        s.edited.is_some(),
        s.editing,
        s.phase == Phase::Sending,
    );

    let buttons = ctx.tones.iter().map(|t| {
        let pressed = t.id == tone && !edited;
        let pick = {
            let state = ctx.state.clone();
            let id = t.id.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| s.pick_tone(id.clone(), cx))
            }
        };
        let (rest_bg, hover_bg, rest_fg, hover_fg) = if pressed {
            (p.fill, p.fill, p.ink, p.ink)
        } else {
            (
                p.tone_hover_bg.opacity(0.),
                p.tone_hover_bg,
                p.tone_fg,
                p.ink,
            )
        };
        let button = div()
            .id(SharedString::from(format!("tone-{}", t.id)))
            .flex()
            .flex_none()
            .items_center()
            .h(px(28.))
            .whitespace_nowrap()
            .cursor_pointer()
            .hover_fill_fg(
                ctx.hover_key(&format!("tone-{}", t.id)),
                rest_bg,
                hover_bg,
                rest_fg,
                hover_fg,
            )
            .when(!sending, |d| d.on_click(pick))
            .child(t.label.clone());
        if clean {
            button
                .px(px(10. - ctx.hair))
                .rounded(px(6.))
                .border(px(ctx.hair))
                .border_color(ctx.ring(p.line))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .line_height(px(20.))
        } else {
            button
                .px(px(if pressed { 8. } else { 9. }))
                .rounded(px(8.))
                .text_size(px(12.5))
                .when(pressed, |d| d.border_1().border_color(p.chip_line))
        }
    });

    let edit_key = ctx.hover_key("edit");
    let (rest_bg, hover_bg, rest_fg, hover_fg) = if editing {
        (p.fill, p.fill, p.ink, p.ink)
    } else if clean {
        (p.wash.opacity(0.), p.wash, p.edit_btn_fg, p.ink)
    } else {
        (p.wash.opacity(0.), p.wash.opacity(0.), p.edit_btn_fg, p.ink)
    };
    let pencil = motion::hover_blend(&edit_key, rest_fg, hover_fg);
    let start_editing = ctx.start_editing.clone();
    let edit = div()
        .id("edit")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .ml(px(4.))
        .rounded(px(if clean { 6. } else { 8. }))
        .cursor_pointer()
        .hover_fill(edit_key, rest_bg, hover_bg)
        .when(!clean && editing, |d| {
            d.border_1().border_color(p.chip_line)
        })
        .when(!sending, |d| {
            d.on_click(move |_, window, cx| start_editing(window, cx))
        })
        .child(
            svg()
                .path(icon(ctx.look, "pencil"))
                .size(px(if clean { 16. } else { 15. }))
                .text_color(pencil),
        );

    div()
        .flex()
        .items_center()
        .gap(px(if clean { 4. } else { 2. }))
        .min_w_0()
        .children(buttons)
        .child(edit)
}

/// Discard and Send email.
fn decision_buttons(ctx: &Ctx, p: &Palette, phase: Phase, since: Option<Instant>) -> Div {
    let clean = ctx.clean;
    let sending = phase == Phase::Sending;
    let (height, pad, radius, size, gap) = if clean {
        (28., 10., 6., 13., 6.)
    } else {
        (36., 14., 10., 14., 7.)
    };
    let button = |id: &'static str| {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(gap))
            .h(px(height))
            .px(px(pad))
            .rounded(px(radius))
            .text_size(px(size))
            .font_weight(FontWeight::MEDIUM)
            .line_height(px(20.))
            .whitespace_nowrap()
            .when(!sending, |d| d.cursor_pointer())
    };

    let discard_key = ctx.hover_key("discard");
    let discard_hover = if sending { 0. } else { 1. };
    let discard = {
        let state = ctx.state.clone();
        let on_discard = ctx.on_discard.clone();
        move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
            if state.update(cx, |s, cx| s.discard(cx))
                && let Some(on_discard) = &on_discard
            {
                on_discard(window, cx);
            }
        }
    };
    let secondary = button("discard")
        .bg(motion::mix(
            p.secondary_bg,
            p.secondary_hover,
            motion::hover_t(&discard_key) * discard_hover,
        ))
        .on_hover(motion::hover_listener(discard_key))
        .text_color(p.secondary_fg)
        .when(clean, |d| d.shadow(p.card_shadow.clone()))
        .when_some(p.card_border.filter(|_| !clean), |d, line| {
            d.px(px(pad - 1.)).border_1().border_color(line)
        })
        .when(!sending, |d| d.on_click(discard))
        .child("Discard");

    let send_key = ctx.hover_key("send");
    let t = if sending {
        0.
    } else {
        motion::hover_t(&send_key)
    };
    let (top, bottom) = (
        motion::mix(p.primary.0, p.primary_hover.0, t),
        motion::mix(p.primary.1, p.primary_hover.1, t),
    );
    let send = ctx.send.clone();
    let primary = button("send")
        .bg(linear_gradient(
            180.,
            linear_color_stop(top, 0.),
            linear_color_stop(bottom, 1.),
        ))
        .on_hover(motion::hover_listener(send_key))
        .text_color(p.primary_fg)
        .when_some(p.primary_ring, |d, ring| {
            // `inset 0 1px 0 rgba(255,255,255,.12)` becomes a top edge
            d.shadow(vec![shadow(0., 0., 0., 1., ring)])
                .border_t_1()
                .border_color(white_a(0.12))
        })
        .when(!sending, |d| {
            d.on_click(move |_, window, cx| send(window, cx))
        })
        .when(sending, |d| {
            d.child(spinner(
                if clean { 12. } else { 13. },
                if clean { 1.5 } else { 1.8 },
                p.primary_fg,
                since,
            ))
            .child("Sending")
        })
        .when(!sending, |d| d.child("Send email"));

    div()
        .flex()
        .flex_none()
        .gap(px(8.))
        .child(secondary)
        .child(primary)
}

/// A ring with a bright quarter turning once every 700ms.
fn spinner(size: f32, width: f32, color: Hsla, since: Option<Instant>) -> impl IntoElement {
    let ms = since.map_or(0., |since| since.elapsed().as_secs_f32() * 1000.);
    let turn = (ms / motion::speed_scale() % SPIN_MS) / SPIN_MS * 2. * PI;
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let center = bounds.center();
            let radius = (size - width) / 2.;
            let arc = |from: f32, to: f32, color: Hsla, window: &mut Window| {
                let steps = 24;
                let mut path = PathBuilder::stroke(px(width));
                for k in 0..=steps {
                    let a = from + (to - from) * k as f32 / steps as f32 + turn;
                    let p = point(
                        center.x + px(radius * a.sin()),
                        center.y - px(radius * a.cos()),
                    );
                    if k == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            };
            // `border: 1.5px solid 30%` with `border-top-color` at full strength
            arc(PI / 4., 7. * PI / 4., color.opacity(0.3), window);
            arc(-PI / 4., PI / 4., color, window);
        },
    )
    .flex_none()
    .size(px(size))
}

/// The one-line receipt after send / discard, with Undo.
fn receipt(ctx: &Ctx, p: &Palette, phase: Phase, cx: &mut App) -> Div {
    let clean = ctx.clean;
    let s = ctx.state.read(cx);
    let (to, file) = (s.to.clone(), s.file.clone());
    let sent = phase == Phase::Sent;

    let badge = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .bg(if sent { p.ok_bg } else { p.quiet_bg })
        .child(
            svg()
                .path(icon(ctx.look, if sent { "check-bold" } else { "x-bold" }))
                .size(px(14.))
                .text_color(if sent { p.ok } else { p.receipt_icon }),
        );
    let badge = if clean {
        badge.size(px(24.)).rounded(px(6.))
    } else {
        badge.size(px(26.)).rounded_full()
    };

    let text = if sent {
        let lead = "Sent to ";
        let line = format!(
            "{lead}{}{}",
            to.name,
            file.map(|f| format!(" with {f}")).unwrap_or_default()
        );
        let name = lead.len()..lead.len() + to.name.len();
        StyledText::new(line).with_highlights([(
            name,
            HighlightStyle {
                color: Some(p.strong),
                font_weight: Some(FontWeight::MEDIUM),
                ..Default::default()
            },
        )])
    } else {
        StyledText::new("Draft discarded")
    };

    let undo_key = ctx.hover_key("undo");
    let undo = {
        let state = ctx.state.clone();
        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| s.undo(cx))
        }
    };
    let undo = div()
        .id("undo")
        .flex()
        .flex_none()
        .items_center()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.ink)
        .cursor_pointer()
        .hover_fill(undo_key, p.wash.opacity(0.), p.wash)
        .on_click(undo)
        .when(clean, |d| d.h(px(28.)).px(px(10.)).rounded(px(6.)))
        .when(!clean, |d| {
            d.px(px(8.))
                .py(px(4.))
                .rounded(px(8.))
                .line_height(inherited(13.))
        })
        .child("Undo");

    let row = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .text_color(p.text)
        .child(badge)
        .child(div().flex_1().min_w_0().truncate().child(text))
        .child(undo);
    if clean {
        row.pt(px(12.))
            .pb(px(12.))
            .pl(px(16.))
            .pr(px(12.))
            .text_size(px(13.))
            .line_height(px(20.))
    } else {
        row.pt(px(11.))
            .pb(px(11.))
            .pl(px(11.))
            .pr(px(9.))
            .border_1()
            .border_color(p.card_border.unwrap_or(p.line))
            .rounded(px(16.))
            .bg(p.card)
            .shadow(p.card_shadow.clone())
            .text_size(px(14.))
            .line_height(inherited(14.))
    }
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

    #[test]
    fn initials_take_two_words() {
        assert_eq!(
            DraftContact::new("Jonas Weber", "").initials(),
            "JW".to_string()
        );
        assert_eq!(DraftContact::new("Finance team", "").initials(), "Ft");
    }
}
