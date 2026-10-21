//! Comment Thread – one comment with its replies, including an assistant
//! that reports what it did. Its status line folds open to show the
//! details, and asking it in a reply shows a shimmering "Thinking…" while
//! the answer comes back.
//!
//! Clean: one flat card – heading, conversation and the reply field lying
//! right in it as its last row. Classic: the white conversation card sits on
//! a gray tray that carries the heading and the reply field. The
//! conversation glides to its new height, follows new messages down, and
//! each message builds up piece by piece.
//!
//! ```ignore
//! CommentThread::new("thread", messages, ThreadAuthor::new("Sofia Marin"))
//!     .assistant(ThreadAuthor::new("Atlas").avatar(ASSISTANT_MARK).assistant())
//!     .on_reply(|text, cx| {
//!         let reply = ThreadMessage::new("a-1", atlas.clone(), "On it.");
//!         text.contains("@atlas").then(|| Task::ready(Some(reply)))
//!     })
//! ```

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    f32::consts::PI,
    rc::Rc,
    time::{Duration, Instant, SystemTime},
};

use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, App, Bounds, BoxShadow, Context, ElementId,
    Entity, FocusHandle, Focusable, FontWeight, HighlightStyle, Hsla, IntoElement, KeyDownEvent,
    MouseButton, Pixels, Rgba, ScrollHandle, SharedString, StyledText, Subscription, Task, TextRun,
    Transformation, Window, div, font, img, linear_color_stop, linear_gradient, point, prelude::*,
    px, radians, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, EASE, EASE_IN_OUT, EASE_OUT_QUINT, MotionSpec},
    primitives::{InputEvent, SANS, TextInput, black_a, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Comment Thread";
pub const DESCRIPTION: &str = "One comment with replies – an assistant reports what it did, folds open details and answers @mentions";

/// The assistant mark from the component: a circle with a blue-to-violet
/// gradient. Pass it as an author's avatar, or use your own logo.
pub const ASSISTANT_MARK: &str = "icons/comment-thread/assistant-mark.svg";

/// `ui-ct*-in`: every message part rises 4px into place over 420ms.
const MSG_IN: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT);
/// Assistant replies that are there from the start come in a beat later.
const FIRST_REPLY_DELAY: u64 = 700;
/// Name row, text and status follow each other 80ms apart.
const STAGGER: u64 = 80;
/// `.ui-ct*__morph`: the conversation glides to its new height.
const MORPH: MotionSpec = MotionSpec::new(380, EASE_OUT_QUINT);
/// `scrollTo({ behavior: "smooth" })` after the conversation grows.
const SCROLL: MotionSpec = MotionSpec::new(450, EASE_IN_OUT);
/// The details fold: `grid-template-rows 320ms`.
const DETAILS_FOLD: MotionSpec = MotionSpec::new(320, EASE_OUT_QUINT);
/// The chevron turns: `transform 260ms`.
const CHEVRON_TURN: MotionSpec = MotionSpec::new(260, EASE_OUT_QUINT);
/// The send button lights up: `background-color 180ms ease` (Classic: opacity 160ms).
const SEND_FADE: MotionSpec = MotionSpec::new(180, EASE);
/// How often the "3min ago" stamps refresh.
const TICK: Duration = Duration::from_secs(30);

/// Who wrote a message.
#[derive(Clone, Debug)]
pub struct ThreadAuthor {
    pub name: SharedString,
    /// An image under `assets/` (SVG, PNG, …) – without one the initials show.
    pub avatar: Option<SharedString>,
    /// Marks the assistant: its first replies come in a beat after the comment.
    pub assistant: bool,
}

impl ThreadAuthor {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            avatar: None,
            assistant: false,
        }
    }

    pub fn avatar(mut self, path: impl Into<SharedString>) -> Self {
        self.avatar = Some(path.into());
        self
    }

    pub fn assistant(mut self) -> Self {
        self.assistant = true;
        self
    }

    fn initials(&self) -> String {
        self.name
            .split(' ')
            .filter_map(|part| part.chars().next())
            .take(2)
            .collect()
    }
}

/// One message in the thread.
#[derive(Clone, Debug)]
pub struct ThreadMessage {
    pub id: SharedString,
    pub author: ThreadAuthor,
    pub created_at: SystemTime,
    pub text: SharedString,
    /// Assistant replies: a short status line that folds open to show `details`.
    pub status: Option<SharedString>,
    pub details: Vec<SharedString>,
}

impl ThreadMessage {
    pub fn new(
        id: impl Into<SharedString>,
        author: ThreadAuthor,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            author,
            created_at: SystemTime::now(),
            text: text.into(),
            status: None,
            details: Vec::new(),
        }
    }

    pub fn created_at(mut self, time: SystemTime) -> Self {
        self.created_at = time;
        self
    }

    pub fn status(mut self, status: impl Into<SharedString>) -> Self {
        self.status = Some(status.into());
        self
    }

    pub fn details(mut self, details: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.details = details.into_iter().map(Into::into).collect();
        self
    }
}

/// The assistant's answer to a reply, resolved later (`Task::ready` for an
/// immediate one). `None` posts nothing.
pub type ReplyTask = Task<Option<ThreadMessage>>;
type ReplyHandler = Rc<dyn Fn(&str, &mut App) -> Option<ReplyTask>>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type Heights = Rc<RefCell<HashMap<String, f32>>>;

/// "just now", "3min ago", "2h ago", "4d ago".
fn ago(time: SystemTime, now: SystemTime) -> String {
    let s = now
        .duration_since(time)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.)
        .round();
    if s < 45. {
        return "just now".into();
    }
    let m = (s / 60.).round();
    if m < 60. {
        return format!("{m}min ago");
    }
    let h = (m / 60.).round();
    if h < 24. {
        format!("{h}h ago")
    } else {
        format!("{}d ago", (h / 24.).round())
    }
}

/// The `--ui-ct-*` / `--ui-ctc-*` tokens.
struct Palette {
    tray: Option<Hsla>,
    card: Hsla,
    /// Classic's conversation card edge.
    card_border: Option<Hsla>,
    shadow: Vec<BoxShadow>,
    line: Hsla,
    title: Hsla,
    name: Hsla,
    ink: Hsla,
    /// Message text.
    text: Hsla,
    /// Detail lines.
    detail: Hsla,
    muted: Hsla,
    time: Hsla,
    icon: Hsla,
    close_hover: Hsla,
    status_hover: Hsla,
    chevron: Hsla,
    chevron_hover: Hsla,
    tree: Hsla,
    /// The `/edit` tag (Clean) and the initials fill.
    fill: Hsla,
    initials_fg: Hsla,
    command: Hsla,
    ok: Hsla,
    avatar_ring: Hsla,
    /// Thinking: the base color and the band that sweeps across it.
    thinking: Hsla,
    shine: Hsla,
    input: Hsla,
    placeholder: Hsla,
    caret: Hsla,
    /// Send: gradient top/bottom (Clean: one solid), its ring and icon.
    send: (Hsla, Hsla),
    send_off: Hsla,
    send_fg: Hsla,
    send_fg_off: Hsla,
    send_shadow: Vec<BoxShadow>,
    send_inset: Option<Hsla>,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => Palette {
                tray: None,
                card: hex(0xffffff),
                card_border: None,
                shadow: vec![
                    ring(0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                line: black_a(0.06),
                title: hex(0x000000),
                name: hex(0x000000),
                ink: black_a(0.88),
                text: black_a(0.88),
                detail: black_a(0.61),
                muted: black_a(0.45),
                time: black_a(0.27),
                icon: black_a(0.447),
                close_hover: black_a(0.03),
                status_hover: black_a(0.03),
                chevron: black_a(0.447),
                chevron_hover: black_a(0.88),
                tree: black_a(0.1),
                fill: black_a(0.06),
                initials_fg: black_a(0.61),
                command: black_a(0.61),
                ok: hex(0x1a9a52),
                avatar_ring: black_a(0.08),
                thinking: black_a(0.45),
                shine: white_a(0.82),
                input: hex(0x000000),
                placeholder: black_a(0.27),
                caret: hexa(0x0060e2, 0.765),
                send: (black_a(0.88), black_a(0.88)),
                send_off: black_a(0.06),
                send_fg: hex(0xfcfcfc),
                send_fg_off: black_a(0.27),
                send_shadow: vec![],
                send_inset: None,
                focus: hexa(0x0060e2, 0.35),
            },
            (Look::Clean, true) => Palette {
                tray: None,
                card: hex(0x1c1c1e),
                card_border: None,
                shadow: vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                line: white_a(0.08),
                title: hex(0xffffff),
                name: hex(0xffffff),
                ink: white_a(0.9),
                text: white_a(0.9),
                detail: white_a(0.62),
                muted: white_a(0.46),
                time: white_a(0.3),
                icon: white_a(0.5),
                close_hover: white_a(0.04),
                status_hover: white_a(0.04),
                chevron: white_a(0.5),
                chevron_hover: white_a(0.9),
                tree: white_a(0.14),
                fill: white_a(0.08),
                initials_fg: white_a(0.62),
                command: white_a(0.62),
                ok: hex(0x3ec27a),
                avatar_ring: white_a(0.1),
                thinking: white_a(0.46),
                shine: hex(0xffffff),
                input: hex(0xffffff),
                placeholder: white_a(0.3),
                caret: hex(0x4d8dff),
                send: (white_a(0.92), white_a(0.92)),
                send_off: white_a(0.08),
                send_fg: hex(0x161618),
                send_fg_off: white_a(0.3),
                send_shadow: vec![],
                send_inset: None,
                focus: hexa(0x4d8dff, 0.5),
            },
            (Look::Classic, false) => Palette {
                tray: Some(hex(0xf0f0f0)),
                card: hex(0xffffff),
                card_border: Some(hex(0xe5e5e5)),
                shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
                line: hex(0xe5e5e5),
                title: hex(0x171717),
                name: hex(0x171717),
                ink: hex(0x171717),
                text: hex(0x404040),
                detail: hex(0x8f8f8f),
                muted: hex(0x8f8f8f),
                time: hex(0x8f8f8f),
                icon: hex(0x8f8f8f),
                close_hover: black_a(0.05),
                status_hover: hex(0xf5f5f5),
                chevron: hex(0x8f8f8f),
                chevron_hover: hex(0x8f8f8f),
                tree: hex(0xd4d4d4),
                fill: hex(0xeef3ff),
                initials_fg: hex(0x2f6bff),
                command: hex(0x8f8f8f),
                ok: hex(0x1f8a4c),
                avatar_ring: black_a(0.06),
                thinking: hex(0xa3a3a3),
                shine: white_a(0.9),
                input: hex(0x171717),
                placeholder: hex(0xa3a3a3),
                caret: hex(0x171717),
                send: (hex(0x323137), hex(0x201e25)),
                send_off: hex(0x323137),
                send_fg: hex(0xffffff),
                send_fg_off: hex(0xffffff),
                send_shadow: vec![
                    ring(1., hex(0x0d0d0d)),
                    shadow(0., 2., 4., 0., black_a(0.1)),
                ],
                send_inset: None,
                focus: hex(0x2f6bff),
            },
            (Look::Classic, true) => Palette {
                tray: Some(hex(0x161618)),
                card: hex(0x232326),
                card_border: Some(white_a(0.08)),
                shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                line: white_a(0.08),
                title: hex(0xededed),
                name: hex(0xededed),
                ink: hex(0xededed),
                text: hex(0xc4c4c4),
                detail: hex(0x8f8f8f),
                muted: hex(0x8f8f8f),
                time: hex(0x8f8f8f),
                icon: hex(0x8f8f8f),
                close_hover: white_a(0.06),
                status_hover: white_a(0.06),
                chevron: hex(0x8f8f8f),
                chevron_hover: hex(0x8f8f8f),
                tree: white_a(0.14),
                fill: hexa(0x2f6bff, 0.16),
                initials_fg: hex(0x6b95ff),
                command: hex(0x8f8f8f),
                ok: hex(0x2a9d5c),
                avatar_ring: white_a(0.08),
                thinking: hex(0x6b6b70),
                shine: white_a(0.9),
                input: hex(0xededed),
                placeholder: hex(0x6b6b70),
                caret: hex(0xededed),
                send: (hex(0xf4f4f5), hex(0xe4e4e7)),
                send_off: hex(0xf4f4f5),
                send_fg: hex(0x121213),
                send_fg_off: hex(0x121213),
                send_shadow: vec![
                    ring(1., black_a(0.6)),
                    shadow(0., 2., 4., 0., black_a(0.35)),
                ],
                send_inset: Some(hex(0xffffff)),
                focus: hex(0x2f6bff),
            },
        }
    }
}

/// Sizes from the two stylesheets.
struct Metrics {
    head_gap: Pixels,
    head_min_h: Pixels,
    title_size: Pixels,
    title_line: Pixels,
    count_size: Pixels,
    count_line: Pixels,
    close_radius: Pixels,
    gap: Pixels,
    pad: (Pixels, Pixels, Pixels),
    avatar: Pixels,
    meta_gap: Pixels,
    name_size: Pixels,
    time_size: Pixels,
    time_line: Pixels,
    text_size: Pixels,
    text_line: Pixels,
    /// Text margin: top, left.
    text_margin: (Pixels, Pixels),
    input_h: Pixels,
    input_size: Pixels,
    send: Pixels,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                head_gap: px(8.),
                head_min_h: px(48.),
                title_size: px(13.),
                title_line: px(20.),
                count_size: px(12.),
                count_line: px(16.),
                close_radius: px(6.),
                gap: px(16.),
                pad: (px(16.), px(16.), px(12.)),
                avatar: px(20.),
                meta_gap: px(8.),
                name_size: px(13.),
                time_size: px(12.),
                time_line: px(16.),
                text_size: px(13.),
                text_line: px(20.),
                text_margin: (px(4.), px(28.)),
                input_h: px(28.),
                input_size: px(13.),
                send: px(28.),
            },
            Look::Classic => Metrics {
                head_gap: px(10.),
                head_min_h: px(46.),
                title_size: px(15.),
                title_line: px(22.),
                count_size: px(12.5),
                count_line: px(15.),
                close_radius: px(8.),
                gap: px(18.),
                pad: (px(16.), px(18.), px(18.)),
                avatar: px(24.),
                meta_gap: px(9.),
                name_size: px(14.),
                time_size: px(12.5),
                time_line: px(15.),
                text_size: px(14.),
                text_line: px(21.),
                text_margin: (px(8.), px(0.)),
                input_h: px(32.),
                input_size: px(14.),
                send: px(30.),
            },
        }
    }
}

fn icon_path(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/comment-thread/clean/{name}.svg").into(),
        Look::Classic => format!("icons/comment-thread/{name}.svg").into(),
    }
}

/// `box-shadow: 0 0 0 <width> color`. GPUI's shadow shader draws nothing for
/// a zero blur (`primitives::ring`), so the edge gets a hair of blur – about
/// the anti-aliasing a browser gives it.
fn ring(width: f32, color: Hsla) -> BoxShadow {
    shadow(0., 0., 0.35, width, color)
}

fn eid(id: String) -> ElementId {
    ElementId::Name(id.into())
}

/// A `0.5px` border the way browsers draw it: at least one device pixel
/// (GPUI clips thinner top borders away at 1x).
fn hairline(window: &Window) -> Pixels {
    let scale = window.scale_factor();
    px((0.5 * scale).round().max(1.) / scale)
}

/// Eased progress of a timeline that started at `since`, after `delay_ms`.
fn progress(since: Instant, now: Instant, spec: MotionSpec) -> f32 {
    let total = spec.total().mul_f32(motion::speed_scale()).as_secs_f32();
    if total <= 0. {
        return 1.;
    }
    spec.progress(now.saturating_duration_since(since).as_secs_f32() / total)
}

/// `top` painted over `base` with `alpha` (both made opaque against `card`).
fn over(base: Hsla, top: Hsla, alpha: f32, card: Hsla) -> Hsla {
    let flat = |c: Hsla| -> Rgba {
        let (c, k): (Rgba, Rgba) = (c.into(), card.into());
        Rgba {
            r: c.r * c.a + k.r * (1. - c.a),
            g: c.g * c.a + k.g * (1. - c.a),
            b: c.b * c.a + k.b * (1. - c.a),
            a: 1.,
        }
    };
    let (b, t) = (flat(base), Rgba::from(top));
    let a = (t.a * alpha).clamp(0., 1.);
    Rgba {
        r: b.r * (1. - a) + t.r * a,
        g: b.g * (1. - a) + t.g * a,
        b: b.b * (1. - a) + t.b * a,
        a: 1.,
    }
    .into()
}

/// Stores the natural height of the measured element's first child under `key`.
fn measure(
    heights: &Heights,
    key: String,
) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + use<> {
    let heights = heights.clone();
    move |bounds, window, _| {
        let Some(bounds) = bounds.first() else {
            return;
        };
        let h = f32::from(bounds.size.height);
        let old = heights.borrow_mut().insert(key.clone(), h);
        if old.is_none_or(|old| (old - h).abs() > 0.01) {
            window.request_animation_frame();
        }
    }
}

struct ThreadState {
    messages: Vec<ThreadMessage>,
    /// Assistant replies that were there from the start.
    first_ids: HashSet<SharedString>,
    open: HashSet<SharedString>,
    toggled: HashMap<SharedString, Instant>,
    thinking: bool,
    now: SystemTime,
    input: Entity<TextInput>,
    has_draft: bool,
    /// New messages keep the conversation at its height (it scrolls instead).
    locked: Option<f32>,
    scroll: ScrollHandle,
    /// Smooth scroll to the bottom: start offset and time.
    scroll_anim: Option<(f32, Instant)>,
    heights: Heights,
    sent: usize,
    /// Props mirrored from the latest render, for replies sent with Enter.
    on_reply: Option<ReplyHandler>,
    current_user: Option<ThreadAuthor>,
    focus: HashMap<String, FocusHandle>,
    keyboard: Rc<Cell<bool>>,
    reply: Option<Task<()>>,
    _tick: Task<()>,
    _input: Subscription,
}

impl ThreadState {
    fn new(
        messages: Vec<ThreadMessage>,
        placeholder: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder(placeholder));
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Change(text) => {
                    this.has_draft = !text.trim().is_empty();
                    cx.notify();
                }
                InputEvent::Submit(_) => this.send(cx),
                // Tab leaves the field: to the send button, or around to the top
                InputEvent::Tab => {
                    let next = if this.has_draft { "send" } else { "first" };
                    if let Some(handle) = this.focus.get(next) {
                        this.keyboard.set(true);
                        window.focus(handle);
                    }
                }
                _ => {}
            });
        let tick = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let alive = this.update(cx, |s, cx| {
                    s.now = SystemTime::now();
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Self {
            first_ids: messages
                .iter()
                .filter(|m| m.author.assistant)
                .map(|m| m.id.clone())
                .collect(),
            messages,
            open: HashSet::new(),
            toggled: HashMap::new(),
            thinking: false,
            now: SystemTime::now(),
            input,
            has_draft: false,
            locked: None,
            scroll: ScrollHandle::new(),
            scroll_anim: None,
            heights: Rc::default(),
            sent: 0,
            on_reply: None,
            current_user: None,
            focus: HashMap::new(),
            keyboard: Rc::default(),
            reply: None,
            _tick: tick,
            _input: subscription,
        }
    }

    fn focus_handle(&mut self, key: &str, cx: &mut App) -> FocusHandle {
        self.focus
            .entry(key.to_string())
            .or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone()
    }

    /// Follow the conversation as it grows.
    fn scroll_down(&mut self) {
        self.scroll_anim = Some((f32::from(self.scroll.offset().y), Instant::now()));
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().trim().to_string();
        let Some(author) = self.current_user.clone() else {
            return;
        };
        if text.is_empty() || self.thinking {
            return;
        }
        self.input.update(cx, |input, cx| input.clear(cx));
        self.has_draft = false;
        self.now = SystemTime::now();
        let height = f32::from(self.scroll.bounds().size.height);
        if height > 0. {
            self.locked = Some(height);
        }
        self.sent += 1;
        let millis = self
            .now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        self.messages.push(ThreadMessage::new(
            format!("m-{millis}-{}", self.sent),
            author,
            text.clone(),
        ));
        self.scroll_down();
        if let Some(answer) = self.on_reply.clone().and_then(|f| f(&text, cx)) {
            self.thinking = true;
            self.reply = Some(cx.spawn(async move |this, cx| {
                let reply = answer.await;
                this.update(cx, |s, cx| {
                    s.now = SystemTime::now();
                    if let Some(reply) = reply {
                        s.messages.push(reply.created_at(s.now));
                    }
                    s.thinking = false;
                    s.scroll_down();
                    cx.notify();
                })
                .ok();
            }));
        }
        cx.notify();
    }

    /// Folding details lets the conversation grow again.
    fn toggle(&mut self, id: &SharedString, cx: &mut Context<Self>) {
        self.locked = None;
        if !self.open.remove(id) {
            self.open.insert(id.clone());
        }
        self.toggled.insert(id.clone(), Instant::now());
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct CommentThread {
    id: ElementId,
    title: SharedString,
    messages: Vec<ThreadMessage>,
    current_user: ThreadAuthor,
    assistant: Option<ThreadAuthor>,
    on_reply: Option<ReplyHandler>,
    on_close: Option<Handler>,
    min_height: Pixels,
    max_height: Pixels,
    look: Option<Look>,
}

impl CommentThread {
    pub fn new(
        id: impl Into<ElementId>,
        messages: Vec<ThreadMessage>,
        current_user: ThreadAuthor,
    ) -> Self {
        Self {
            id: id.into(),
            title: "Comment".into(),
            messages,
            current_user,
            assistant: None,
            on_reply: None,
            on_close: None,
            min_height: px(0.),
            max_height: px(340.),
            look: None,
        }
    }

    /// The heading, "Comment" by default.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The assistant that answers – used for the thinking state and the placeholder.
    pub fn assistant(mut self, assistant: ThreadAuthor) -> Self {
        self.assistant = Some(assistant);
        self
    }

    /// Called with every new reply. Return a task resolving to the
    /// assistant's message to let it answer (a "Thinking…" row shows
    /// meanwhile); return `None` to just post the reply.
    pub fn on_reply(
        mut self,
        handler: impl Fn(&str, &mut App) -> Option<ReplyTask> + 'static,
    ) -> Self {
        self.on_reply = Some(Rc::new(handler));
        self
    }

    /// Shows a close button in the heading.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// `--ui-ct-min-height`: the conversation never gets shorter than this.
    pub fn min_height(mut self, height: Pixels) -> Self {
        self.min_height = height;
        self
    }

    /// `--ui-ct-max-height` (340px): taller conversations scroll.
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }

    /// Overrides the global Clean/Classic look for this thread.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for CommentThread {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx);
        let m = Metrics::get(look);
        let key = self.id.to_string();
        let placeholder: SharedString = match &self.assistant {
            Some(a) => format!("Reply, or ask @{}…", a.name).into(),
            None => "Add comment…".into(),
        };
        let initial = self.messages;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| ThreadState::new(initial, placeholder, window, cx),
        );

        // Mirror the props the state needs to send with Enter, and the colors
        let (input, close_focus, send_focus, root_focus) = state.update(cx, |s, cx| {
            s.on_reply = self.on_reply.clone();
            s.current_user = Some(self.current_user.clone());
            s.input.update(cx, |input, _| {
                input.set_colors(Some(p.placeholder), Some(p.caret), None)
            });
            // The card itself takes focus on click, so Tab can enter its controls
            let root = s
                .focus
                .entry("root".into())
                .or_insert_with(|| cx.focus_handle())
                .clone();
            (
                s.input.clone(),
                s.focus_handle("close", cx),
                s.focus_handle("send", cx),
                root,
            )
        });
        let status_focus: HashMap<SharedString, FocusHandle> = state.update(cx, |s, cx| {
            let ids: Vec<_> = s
                .messages
                .iter()
                .filter(|m| m.status.is_some() && !m.details.is_empty())
                .map(|m| m.id.clone())
                .collect();
            ids.into_iter()
                .map(|id| {
                    let handle = s.focus_handle(&format!("status-{id}"), cx);
                    (id, handle)
                })
                .collect()
        });

        let now = Instant::now();
        let s = state.read(cx);
        let keyboard = s.keyboard.clone();
        let heights = s.heights.clone();
        let scroll = s.scroll.clone();
        let can_send = s.has_draft && !s.thinking;
        let count = s.messages.len();
        let mut moving = false;

        // Smooth-scroll to the bottom; the target is re-read as layout catches
        // up, and the wheel takes over again once it has arrived
        let mut scrolled = false;
        if let Some((from, started)) = s.scroll_anim {
            let t = progress(started, now, SCROLL);
            let target = -f32::from(scroll.max_offset().height);
            scroll.set_offset(point(px(0.), px(motion::lerp(from, target, t))));
            moving = t < 1.;
            scrolled = t >= 1.;
        }

        let space = space_width(window, m.text_size);
        let mut items: Vec<AnyElement> = Vec::new();
        for msg in &s.messages {
            let delay = if s.first_ids.contains(&msg.id) {
                FIRST_REPLY_DELAY
            } else {
                0
            };
            let open = s.open.contains(&msg.id);
            let toggled = s.toggled.get(&msg.id).copied();
            let ring_on = status_focus
                .get(&msg.id)
                .is_some_and(|f| keyboard.get() && f.is_focused(window));
            let mut parts: Vec<AnyElement> = vec![meta(
                &msg.author,
                Some(ago(msg.created_at, s.now)),
                &p,
                &m,
                look,
            )];
            if !msg.text.is_empty() {
                parts.push(text(&msg.text, &p, &m, look, space));
            }
            if let Some(status) = msg.status.clone() {
                let ids = (key.clone(), msg.id.clone());
                let toggle: Option<Handler> = (!msg.details.is_empty()).then(|| {
                    let state = state.clone();
                    let id = msg.id.clone();
                    Rc::new(move |_: &mut Window, cx: &mut App| {
                        state.update(cx, |s, cx| s.toggle(&id, cx))
                    }) as Handler
                });
                let (status, active) = status_line(StatusLine {
                    ids,
                    status,
                    details: &msg.details,
                    open,
                    toggled,
                    now,
                    toggle,
                    focus: status_focus.get(&msg.id).cloned(),
                    ring_on,
                    heights: &heights,
                    p: &p,
                    look,
                });
                moving |= active;
                parts.push(status);
            }
            items.push(build_up(&key, &msg.id, delay, parts));
        }
        if s.thinking
            && let Some(assistant) = &self.assistant
        {
            let base = p.thinking;
            let (shine, card) = (p.shine, p.card);
            let (period, half) = if clean { (1400, 0.36) } else { (1600, 0.45) };
            let thinking = StyledText::new("Thinking…").with_animation(
                eid(format!("{key}-thinking-sweep")),
                Animation::new(Duration::from_millis(period).mul_f32(motion::speed_scale()))
                    .repeat(),
                move |text, t| {
                    text.with_highlights(sweep("Thinking…", t, base, shine, half, card, clean))
                },
            );
            let line = div()
                .mt(m.text_margin.0)
                .ml(m.text_margin.1)
                .text_size(m.text_size)
                .line_height(if clean { m.text_line } else { px(17.) })
                .child(thinking);
            items.push(build_up(
                &key,
                &SharedString::from("thinking"),
                0,
                vec![meta(assistant, None, &p, &m, look), line.into_any_element()],
            ));
        }

        // The conversation: scrolls past the max height, never shorter than the min
        let max_h = s.locked.map_or(self.max_height, px);
        let list = div()
            .id(eid(format!("{key}-scroll")))
            .track_scroll(&scroll)
            .overflow_y_scroll()
            .flex_none()
            .flex()
            .flex_col()
            .gap(m.gap)
            .min_h(self.min_height)
            .max_h(max_h.max(self.min_height))
            .pt(m.pad.0)
            .px(m.pad.1)
            .pb(m.pad.2)
            .children(items);
        // Soft edges at the top and towards the reply row (Clean's mask)
        let list = div().relative().flex_none().child(list).when(clean, |d| {
            d.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .h(px(12.))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(p.card, 0.),
                        linear_color_stop(p.card.opacity(0.), 1.),
                    )),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h(px(10.))
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(p.card.opacity(0.), 0.),
                        linear_color_stop(p.card, 1.),
                    )),
            )
        });
        // Only the conversation glides to its new height
        let measured = f32::from(scroll.bounds().size.height);
        let morph_h =
            (measured > 0.).then(|| motion::follow(&format!("{key}-morph"), measured, MORPH));
        let morph = div()
            .flex_none()
            .overflow_hidden()
            .when_some(morph_h, |d, h| d.h(px(h)))
            .child(list);
        let morph = match p.card_border {
            Some(border) => morph
                .bg(p.card)
                .border_1()
                .border_color(border)
                .rounded(px(16.))
                .shadow(p.shadow.clone()),
            None => morph,
        };

        // Heading: title, count and the close button
        let hair = hairline(window);
        let head = div()
            .flex()
            .items_center()
            .gap(m.head_gap)
            .min_h(m.head_min_h)
            .map(|d| {
                if clean {
                    d.py(px(10.))
                        .pr(px(10.))
                        .pl(px(16.))
                        .border_b(hair)
                        .border_color(p.line)
                } else {
                    d.py(px(6.)).pr(px(10.)).pl(px(18.))
                }
            })
            .child(
                div()
                    .text_size(m.title_size)
                    .line_height(m.title_line)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.title)
                    .child(self.title.clone()),
            )
            // `margin-right: auto` – as a growing box, since taffy drops the
            // gaps of a row with auto margins
            .child(
                div()
                    .flex_1()
                    .text_size(m.count_size)
                    .line_height(m.count_line)
                    .text_color(p.muted)
                    .child(format!(
                        "{count} {}",
                        if count == 1 { "message" } else { "messages" }
                    )),
            )
            .when_some(self.on_close.clone(), |d, on_close| {
                let hover_key = format!("{key}-close");
                let t = motion::hover_t(&hover_key);
                let ring_on = keyboard.get() && close_focus.is_focused(window);
                d.child(
                    div()
                        .id(eid(hover_key.clone()))
                        .track_focus(&close_focus)
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(28.))
                        .rounded(m.close_radius)
                        .cursor_pointer()
                        .hover_fill(hover_key, p.close_hover.opacity(0.), p.close_hover)
                        .when(ring_on, |b| b.shadow(focus_ring(&p, clean)))
                        // Enter / Space on the focused button click it (GPUI)
                        .on_click(move |_, window, cx| on_close(window, cx))
                        .child(
                            svg()
                                .path(icon_path(look, "close"))
                                .size(px(16.))
                                .text_color(motion::mix(p.icon, p.ink, t)),
                        ),
                )
            });

        // The reply row
        let send_t = motion::follow(
            &format!("{key}-send"),
            if can_send { 1. } else { 0. },
            if clean {
                SEND_FADE
            } else {
                SEND_FADE.with_duration(160)
            },
        );
        let send_ring = keyboard.get() && send_focus.is_focused(window);
        // Classic fades the whole chip to 30% while there is nothing to send.
        // GPUI's opacity applies per layer (the ring would show through the
        // chip), so the chip is flattened against the tray instead.
        let tray = p.tray.unwrap_or(p.card);
        let fade = if clean {
            1.
        } else {
            motion::lerp(0.3, 1., send_t)
        };
        let flat = |c: Hsla| over(tray, c, fade, tray);
        let send = {
            let state = state.clone();
            let mut shadows: Vec<BoxShadow> = p
                .send_shadow
                .iter()
                .map(|s| BoxShadow {
                    color: s.color.opacity(s.color.a * fade),
                    ..s.clone()
                })
                .collect();
            if send_ring {
                shadows.extend(focus_ring(&p, clean));
            }
            div()
                .id(eid(format!("{key}-send")))
                .track_focus(&send_focus)
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(m.send)
                .map(|b| {
                    if clean {
                        b.rounded(px(6.))
                            .bg(motion::mix(p.send_off, p.send.0, send_t))
                    } else {
                        b.rounded_full()
                            .bg(linear_gradient(
                                180.,
                                linear_color_stop(flat(p.send.0), 0.),
                                linear_color_stop(flat(p.send.1), 1.),
                            ))
                            .when_some(p.send_inset, |b, inset| {
                                b.border_t_1().border_color(flat(inset))
                            })
                    }
                })
                .shadow(shadows)
                .when(can_send, |b| b.cursor_pointer())
                .on_click(move |_, _, cx| state.update(cx, |s, cx| s.send(cx)))
                .child(
                    svg()
                        .path(icon_path(look, "send"))
                        .size(px(if clean { 16. } else { 14. }))
                        .text_color(if clean {
                            motion::mix(p.send_fg_off, p.send_fg, send_t)
                        } else {
                            flat(p.send_fg)
                        }),
                )
        };
        let composer = div()
            .flex_none()
            .flex()
            .items_center()
            .map(|d| {
                if clean {
                    d.gap(px(8.)).pt(px(4.)).pr(px(10.)).pb(px(10.)).pl(px(16.))
                } else {
                    d.gap(px(10.)).py(px(9.)).pr(px(9.)).pl(px(18.))
                }
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h(m.input_h)
                    .items_center()
                    .text_size(m.input_size)
                    .line_height(px(20.))
                    .text_color(p.input)
                    .child(input),
            )
            .child(send);

        if moving {
            window.request_animation_frame();
        }
        // Followers and hover fades started in this render need their frames
        motion::keep_alive(window);

        // Tab moves through the controls, like the page's tab order
        let mut tab_order: Vec<FocusHandle> = Vec::new();
        if self.on_close.is_some() {
            tab_order.push(close_focus.clone());
        }
        for msg in &s.messages {
            if let Some(f) = status_focus.get(&msg.id) {
                tab_order.push(f.clone());
            }
        }
        let input_focus = s.input.read(cx).focus_handle(cx);
        tab_order.push(input_focus);
        if can_send {
            tab_order.push(send_focus.clone());
        }
        let first = tab_order[0].clone();
        state.update(cx, |s, _| {
            s.focus.insert("first".into(), first);
            if scrolled {
                s.scroll_anim = None;
            }
        });
        let on_key = {
            let keyboard = keyboard.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if event.keystroke.key != "tab" {
                    return;
                }
                let len = tab_order.len();
                let current = tab_order.iter().position(|f| f.is_focused(window));
                let next = match (current, event.keystroke.modifiers.shift) {
                    (Some(i), false) => (i + 1) % len,
                    (Some(i), true) => (i + len - 1) % len,
                    (None, false) => 0,
                    (None, true) => len - 1,
                };
                keyboard.set(true);
                window.focus(&tab_order[next]);
                cx.stop_propagation();
            }
        };

        let root = div()
            .id(self.id.clone())
            .track_focus(&root_focus)
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(420.))
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(p.ink)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, move |_, _, _| keyboard.set(false));
        match p.tray {
            // One card – the edge is a shadow, the reply row lies right in it
            None => root
                .overflow_hidden()
                .rounded(px(10.))
                .bg(p.card)
                .shadow(p.shadow.clone())
                .child(head)
                .child(morph)
                .child(composer),
            // The gray tray frames the white card on every side
            Some(tray) => root
                .p(px(6.))
                .rounded(px(22.))
                .bg(tray)
                .child(head)
                .child(morph)
                .child(composer),
        }
    }
}

/// `outline: 2px solid; outline-offset: 1px` (Clean) / `2px` (Classic) as rings.
fn focus_ring(p: &Palette, clean: bool) -> Vec<BoxShadow> {
    let gap = if clean { 1. } else { 2. };
    vec![ring(gap + 2., p.focus), ring(gap, p.card)]
}

/// Each message builds up piece by piece: name row, text, status – 80ms apart.
fn build_up(key: &str, id: &SharedString, delay: u64, parts: Vec<AnyElement>) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .flex_col()
        .min_w_0()
        .children(parts.into_iter().enumerate().map(|(ix, part)| {
            let spec = MSG_IN.with_delay(delay + STAGGER * ix as u64);
            motion::rise_in(
                eid(format!("{key}-in-{id}-{ix}")),
                spec,
                4.,
                div().child(part),
            )
            .into_any_element()
        }))
        .into_any_element()
}

/// Avatar, name and time.
fn meta(
    author: &ThreadAuthor,
    time: Option<String>,
    p: &Palette,
    m: &Metrics,
    look: Look,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(m.meta_gap)
        .min_w_0()
        .child(avatar(author, p, m, look))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(m.name_size)
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.name)
                .child(author.name.clone()),
        )
        .when_some(time, |d, time| {
            d.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(m.time_size)
                    .line_height(m.time_line)
                    .text_color(p.time)
                    .child(time),
            )
        })
        .into_any_element()
}

fn avatar(author: &ThreadAuthor, p: &Palette, m: &Metrics, look: Look) -> AnyElement {
    let base = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(m.avatar)
        .rounded_full();
    match &author.avatar {
        // Photos get a faint ring; a mark (the assistant) stands on its own
        Some(path) => base
            .when(!author.assistant, |d| {
                d.shadow(vec![ring(
                    if look == Look::Clean { 0.5 } else { 1. },
                    p.avatar_ring,
                )])
            })
            .child(img(path.clone()).size(m.avatar).rounded_full())
            .into_any_element(),
        None => base
            .bg(p.fill)
            .text_size(px(9.))
            .line_height(px(9.))
            .font_weight(if look == Look::Clean {
                FontWeight::MEDIUM
            } else {
                FontWeight::SEMIBOLD
            })
            .text_color(p.initials_fg)
            .child(author.initials())
            .into_any_element(),
    }
}

/// Width of a space in the body text, for laying out tagged text word by word.
fn space_width(window: &Window, size: Pixels) -> Pixels {
    let run = TextRun {
        len: 1,
        font: font(SANS),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(" ".into(), size, &[run], None)
        .width
}

/// The message text; a leading `/edit` becomes a small gray tag (Clean) or
/// a muted word (Classic).
fn text(text: &SharedString, p: &Palette, m: &Metrics, look: Look, space: Pixels) -> AnyElement {
    let trimmed = text.trim();
    let is_command = trimmed
        .get(..5)
        .is_some_and(|head| head.eq_ignore_ascii_case("/edit"))
        && trimmed[5..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_');
    let rest = trimmed.get(5..).unwrap_or("").trim_start();
    let paragraph = div()
        .mt(m.text_margin.0)
        .ml(m.text_margin.1)
        .text_size(m.text_size)
        .line_height(m.text_line)
        .text_color(p.text);
    if !is_command {
        return paragraph.child(text.clone()).into_any_element();
    }
    match look {
        // An inline tag can't sit in a GPUI text run: lay the words out in a
        // wrapping row instead, with the space as the gap
        Look::Clean => paragraph
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(space)
            .child(
                div()
                    .mr(px(1.))
                    .px(px(5.))
                    .rounded(px(5.))
                    .bg(p.fill)
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.command)
                    .child("/edit"),
            )
            .children(
                rest.split_whitespace()
                    .map(|word| div().whitespace_nowrap().child(word.to_string())),
            )
            .into_any_element(),
        Look::Classic => {
            let full = format!("/Edit {rest}");
            paragraph
                .child(StyledText::new(full).with_highlights([(
                    0..5,
                    HighlightStyle {
                        color: Some(p.command),
                        ..Default::default()
                    },
                )]))
                .into_any_element()
        }
    }
}

/// Per-letter colors for the "Thinking…" sweep: a lighter band (`half` of
/// the text width to each side) moves across over one period.
fn sweep(
    text: &str,
    t: f32,
    base: Hsla,
    shine: Hsla,
    half: f32,
    card: Hsla,
    clean: bool,
) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let count = text.chars().count().max(1) as f32;
    // `background-position: 100% → 0%` on a 300%-wide gradient
    let center = -0.5 + 2. * t;
    text.char_indices()
        .enumerate()
        .map(|(k, (ix, c))| {
            let x = (k as f32 + 0.5) / count;
            let band = (1. - (x - center).abs() / half).max(0.);
            let color = if clean {
                over(base, shine, band, card)
            } else {
                over(motion::mix(base, shine, band), shine, 0., card)
            };
            (
                ix..ix + c.len_utf8(),
                HighlightStyle {
                    color: Some(color),
                    ..Default::default()
                },
            )
        })
        .collect()
}

struct StatusLine<'a> {
    ids: (String, SharedString),
    status: SharedString,
    details: &'a [SharedString],
    open: bool,
    toggled: Option<Instant>,
    now: Instant,
    toggle: Option<Handler>,
    focus: Option<FocusHandle>,
    ring_on: bool,
    heights: &'a Heights,
    p: &'a Palette,
    look: Look,
}

/// The assistant's status: a green check, the line and a chevron that folds
/// the details open under thin tree lines. Returns whether it is animating.
fn status_line(line: StatusLine) -> (AnyElement, bool) {
    let StatusLine {
        ids: (key, id),
        status,
        details,
        open,
        toggled,
        now,
        toggle,
        focus,
        ring_on,
        heights,
        p,
        look,
    } = line;
    let clean = look == Look::Clean;
    let has_details = toggle.is_some();
    let hover_key = format!("{key}-status-{id}");
    let hover = if has_details {
        motion::hover_t(&hover_key)
    } else {
        0.
    };
    let turn = motion::follow(
        &format!("{key}-chevron-{id}"),
        if open { PI } else { 0. },
        CHEVRON_TURN,
    );
    let mut moving = false;

    let check = if clean {
        svg()
            .path(icon_path(look, "check"))
            .size(px(16.))
            .flex_none()
            .text_color(p.ok)
            .into_any_element()
    } else {
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(16.))
            .rounded_full()
            .bg(p.ok)
            .child(
                svg()
                    .path(icon_path(look, "check"))
                    .size(px(12.))
                    .text_color(hex(0xffffff)),
            )
            .into_any_element()
    };
    let button = div()
        .id(eid(hover_key.clone()))
        .flex()
        .flex_none()
        .items_center()
        .max_w_full()
        .map(|b| {
            if clean {
                b.gap(px(6.))
                    .h(px(28.))
                    .pl(px(6.))
                    .pr(px(4.))
                    .rounded(px(6.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
            } else {
                // Classic's button reset (`font: inherit`) keeps the weight regular
                b.gap(px(7.))
                    .h(px(26.))
                    .pl(px(8.))
                    .pr(px(6.))
                    .ml(px(-4.))
                    .rounded(px(8.))
                    .text_size(px(14.))
                    .line_height(px(20.))
            }
        })
        .text_color(p.ink)
        .bg(motion::mix(
            p.status_hover.opacity(0.),
            p.status_hover,
            hover,
        ))
        .when_some(focus.filter(|_| has_details), |b, f| b.track_focus(&f))
        .when(ring_on, |b| b.shadow(focus_ring(p, clean)))
        .when_some(toggle, |b, toggle| {
            b.cursor_pointer()
                .on_hover(motion::hover_listener(SharedString::from(
                    hover_key.clone(),
                )))
                .on_click(move |_, window, cx| toggle(window, cx))
        })
        .child(check)
        .child(div().min_w_0().truncate().child(status))
        .when(has_details, |b| {
            let size = px(if clean { 16. } else { 14. });
            b.child(
                svg()
                    .path(icon_path(look, "chevron"))
                    .size(size)
                    .flex_none()
                    .with_transformation(Transformation::rotate(radians(turn)))
                    .text_color(motion::mix(p.chevron, p.chevron_hover, hover)),
            )
        });

    let mut block = div()
        .flex()
        .flex_col()
        .items_start()
        .map(|d| {
            if clean {
                d.mt(px(4.)).ml(px(22.))
            } else {
                d.mt(px(8.))
            }
        })
        .child(button);

    if has_details {
        // The fold: lines fade in one after another while the box opens, and
        // fade out quickly while it closes
        let fold_key = format!("{key}-details-{id}");
        let natural = heights.borrow().get(&fold_key).copied();
        let height = match (open, natural) {
            (true, Some(h)) => Some(motion::follow(&fold_key, h, DETAILS_FOLD)),
            (true, None) => None,
            (false, _) => Some(motion::follow(&fold_key, 0., DETAILS_FOLD)),
        };
        let height = height.filter(|h| !(open && Some(*h) == natural));
        let elapsed = |delay: f32, spec: MotionSpec| {
            toggled.map_or(1., |at| progress(at, now, spec.with_delay(delay as u64)))
        };
        let lines: Vec<_> = details
            .iter()
            .enumerate()
            .map(|(ix, detail)| {
                let (opacity, rise) = if open {
                    let delay = 60. + ix as f32 * 60.;
                    (
                        elapsed(delay, MotionSpec::new(320, EASE)),
                        elapsed(delay, MotionSpec::new(420, EASE_OUT_QUINT)),
                    )
                } else if toggled.is_none() {
                    (0., 0.)
                } else {
                    (
                        1. - elapsed(0., MotionSpec::new(160, EASE)),
                        1. - elapsed(0., MotionSpec::new(220, EASE_OUT_QUINT)),
                    )
                };
                if toggled.is_some() && (opacity > 0. && opacity < 1. || rise > 0. && rise < 1.) {
                    moving = true;
                }
                let first = ix == 0;
                // Each line gets its own corner: down on the axis of the
                // check icon, then a rounded turn right
                let (left, width, border) = if clean {
                    (-14.5, 9., px(1.))
                } else {
                    (-16.625, 10.5, px(1.25))
                };
                let (top, tall) = match (clean, first) {
                    (true, true) => (-6., 16.5),
                    (true, false) => (-3., 13.5),
                    (false, true) => (-5., 15.),
                    (false, false) => (-3., 13.),
                };
                div()
                    .relative()
                    .mt(px(match (clean, first) {
                        (true, true) => 2.,
                        (true, false) => 4.,
                        (false, true) => 5.,
                        (false, false) => 6.,
                    }))
                    .top(px(-4. * (1. - rise)))
                    .opacity(opacity)
                    .text_size(px(13.))
                    .line_height(px(if clean { 20. } else { 19. }))
                    .text_color(p.detail)
                    .child(
                        div()
                            .absolute()
                            .left(px(left))
                            .top(px(top))
                            .w(px(width))
                            .h(px(tall))
                            .border_l(border)
                            .border_b(border)
                            .border_color(p.tree)
                            .rounded_bl(px(5.)),
                    )
                    .child(detail.clone())
            })
            .collect();
        let list = div()
            .flex_none()
            .flex()
            .flex_col()
            .pl(px(28.))
            .children(lines);
        block = block.child(
            div()
                .w_full()
                .overflow_hidden()
                .when_some(height, |d, h| d.h(px(h)))
                .on_children_prepainted(measure(heights, fold_key))
                .child(list),
        );
    }
    (block.into_any_element(), moving)
}

/// Drawn stand-ins for the demo's profile photos.
const LENA: &str = "icons/comment-thread/lena.svg";
const SOFIA: &str = "icons/comment-thread/sofia.svg";

/// What Atlas answers when it is asked, in turn.
const REPLIES: [&str; 3] = [
    "Done – I also updated the meta description.",
    "Good catch. I'll leave a note for the copy review.",
    "On it. You'll see the change in a minute.",
];

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    let atlas = ThreadAuthor::new("Atlas")
        .avatar(ASSISTANT_MARK)
        .assistant();
    let minutes_ago = |m: u64| SystemTime::now() - Duration::from_secs(m * 60);
    let messages = vec![
        ThreadMessage::new(
            "1",
            ThreadAuthor::new("Lena Lindqvist").avatar(LENA),
            "/edit Rename every “customers” to “members” on the pricing page.",
        )
        .created_at(minutes_ago(3)),
        ThreadMessage::new("2", atlas.clone(), "")
            .created_at(minutes_ago(1))
            .status("Edit completed")
            .details([
                "Renamed 12 mentions in 3 sections",
                "Kept “customer support” as it is",
                "Left the legal pages untouched",
            ]),
    ];
    let asked = Rc::new(Cell::new(0usize));
    story::view(cx, move |_, _| {
        let (atlas, asked) = (atlas.clone(), asked.clone());
        CommentThread::new(
            "comment-thread",
            messages.clone(),
            ThreadAuthor::new("Sofia Marin").avatar(SOFIA),
        )
        .assistant(atlas.clone())
        .min_height(px(290.))
        .max_height(px(340.))
        .on_close(|_, _| {})
        .on_reply(move |text, cx| {
            // Atlas only answers when it is mentioned
            if !text.to_lowercase().contains("@atlas") {
                return None;
            }
            let ix = asked.get();
            asked.set(ix + 1);
            let reply = ThreadMessage::new(format!("a-{ix}"), atlas.clone(), REPLIES[ix % 3]);
            Some(cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(1400))
                    .await;
                Some(reply)
            }))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ago_matches_the_component() {
        let now = SystemTime::now();
        let before = |s: u64| now - Duration::from_secs(s);
        assert_eq!(ago(before(10), now), "just now");
        assert_eq!(ago(before(50), now), "1min ago");
        assert_eq!(ago(before(3 * 60), now), "3min ago");
        assert_eq!(ago(before(2 * 3600), now), "2h ago");
        assert_eq!(ago(before(3 * 86400), now), "3d ago");
        // Clock skew never shows a negative time
        assert_eq!(ago(now + Duration::from_secs(5), now), "just now");
    }

    #[test]
    fn initials() {
        assert_eq!(ThreadAuthor::new("Lena Lindqvist").initials(), "LL");
        assert_eq!(ThreadAuthor::new("Atlas").initials(), "A");
    }
}
