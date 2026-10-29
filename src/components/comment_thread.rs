//! Comment Thread – a Pink component.
//!
//! An inline review comment grown into a conversation. The thread is Pink's
//! comment card (Pink: comment_ui.rs `render_comment_card`, drawn by the
//! `pink-comment-card` package, [`comment_card`]): a flat band
//! with a 3px bar, the anchor it is pinned to in the header, and every
//! message under it with its author. An assistant's reply carries Pink's
//! tool-group summary plate; it folds open onto the activity rail to list
//! what was done. Mentioning the assistant in a reply shows Pink's working
//! line (breathing orb, shimmering "Thinking", elapsed timer) until the
//! answer arrives. The reply field is Pink's draft card (Pink: comment_ui.rs
//! `render_comment_draft`) with Cancel and Comment. The conversation grows
//! to its new height when a message lands and glides down to it.
//!
//! ```ignore
//! CommentThread::new("thread", messages, ThreadAuthor::new("Sofia Marin"))
//!     .anchor("site/pricing.md:14")
//!     .assistant(ThreadAuthor::new("Atlas").assistant())
//!     .on_reply(|text, cx| {
//!         let reply = ThreadMessage::new("a-1", atlas.clone(), "On it.");
//!         text.contains("@atlas").then(|| Task::ready(Some(reply)))
//!     })
//! ```

use std::{
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant, SystemTime};

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FillOptions, FillRule, Focusable,
    FontWeight, Hsla, IntoElement, KeyDownEvent, PathBuilder, PathStyle, Pixels, Point, Role,
    ScrollHandle, SharedString, Stateful, Subscription, Task, TextRun, Transformation, Window,
    canvas, div, font, img, point, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    components::comment_card,
    motion::{self, MotionSpec},
    pink::{self, FillTier, Pink, loaders, radius, space},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Comment Thread";
pub const DESCRIPTION: &str = "An inline review thread pinned to a line: replies with their authors, an assistant whose summary folds open onto the activity rail, a shimmering working line while it answers @mentions, and Pink's draft card to reply";

/// The glyph an assistant without an avatar image wears (Pink `icons::BOT`).
pub const ASSISTANT_MARK: &str = "icons/comment-thread/bot.svg";
/// Status glyph for edits (Pink `icons::FILE_PEN`, the edit tool's glyph).
pub const EDIT_GLYPH: &str = "icons/comment-thread/file-pen.svg";
/// Status glyph for mixed work, the default (Pink `icons::LAYERS_MINIMALISTIC`).
pub const STEPS_GLYPH: &str = "icons/comment-thread/layers-minimalistic.svg";

const ICON_REMOVE: &str = comment_card::ICON_REMOVE;
const ICON_CHEVRON: &str = "icons/comment-thread/alt-arrow-right.svg";

/// Pink comments.rs card geometry, from the `pink-comment-card` package:
/// the vertical padding (split top and bottom), a body line and the slack
/// below the body.
const CARD_PAD_V: f32 = comment_card::PAD_V;
const CARD_LINE_HEIGHT: f32 = comment_card::LINE_HEIGHT;
const CARD_GAP: f32 = comment_card::GAP;

/// Pink transcript.rs tool group and activity rail geometry.
const TOOL_GROUP_HEADER_HEIGHT: f32 = 26.0;
const ACTIVITY_ICON_SIZE: f32 = 14.0;
const ACTIVITY_TRUNK_X: f32 = 12.5;
const ACTIVITY_BEND_RADIUS: f32 = 6.0;
const ACTIVITY_BRANCH_END_X: f32 = 28.0;
const DISCLOSURE_CHEVRON_SIZE: f32 = 12.0;
const TOOL_TREE_ROW_HEIGHT: f32 = 32.0;
const CHIPS_TOP_PAD: f32 = 2.0;
/// Detail text starts after Pink's 4px break past the branch tip.
const DETAIL_TEXT_X: f32 = ACTIVITY_BRANCH_END_X + 4.0;

/// Pink history.rs author avatar; message text starts past it and an 8px gap.
const AVATAR: f32 = 20.0;
const TEXT_INDENT: f32 = AVATAR + space::S8;
/// Pink markdown/render.rs inline code plate.
const INLINE_CODE_RADIUS: f32 = 4.5;
const INLINE_CODE_PAD_X: f32 = 2.0;
const INLINE_CODE_INSET_Y: f32 = 2.0;

/// How often the "3m" stamps refresh.
const TICK: Duration = Duration::from_secs(30);

/// Who wrote a message.
#[derive(Clone, Debug)]
pub struct ThreadAuthor {
    pub name: SharedString,
    /// An image under `assets/` (SVG, PNG, …) – without one the initial shows
    /// (an assistant shows [`ASSISTANT_MARK`]).
    pub avatar: Option<SharedString>,
    /// Marks the assistant, which answers mentions.
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

    /// Pink history.rs `history_author_initial`: the first letter, upper-cased.
    fn initial(&self) -> SharedString {
        self.name
            .chars()
            .find(|c| !c.is_whitespace())
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_else(|| "?".to_string())
            .into()
    }
}

/// One message in the thread.
#[derive(Clone, Debug)]
pub struct ThreadMessage {
    pub id: SharedString,
    pub author: ThreadAuthor,
    pub created_at: SystemTime,
    /// The note. A leading `/command` reads as inline code, `@names` as mentions.
    pub text: SharedString,
    /// Assistant replies: a short summary that folds open to show `details`.
    pub status: Option<SharedString>,
    /// The summary's glyph, [`STEPS_GLYPH`] by default.
    pub status_icon: Option<SharedString>,
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
            status_icon: None,
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

    /// An SVG path for the summary's glyph, like [`EDIT_GLYPH`].
    pub fn status_icon(mut self, path: impl Into<SharedString>) -> Self {
        self.status_icon = Some(path.into());
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

/// Pink proto view.rs `format_time_ago`: "now", "3m", "2h", "4d", "1w", "2mo", "1y".
fn time_ago(then: SystemTime, now: SystemTime) -> String {
    let s = now.duration_since(then).map_or(0, |d| d.as_secs());
    if s < 60 {
        return "now".into();
    }
    let m = s / 60;
    if m < 60 {
        return format!("{m}m");
    }
    let h = m / 60;
    if h < 24 {
        return format!("{h}h");
    }
    let d = h / 24;
    if d < 7 {
        return format!("{d}d");
    }
    let w = d / 7;
    if w < 5 {
        return format!("{w}w");
    }
    let mo = d / 30;
    if mo < 12 {
        return format!("{mo}mo");
    }
    format!("{}y", d / 365)
}

/// Pink comments.rs `chip_label`.
fn count_label(count: usize) -> String {
    if count == 1 {
        "1 comment".into()
    } else {
        format!("{count} comments")
    }
}

fn eid(id: String) -> ElementId {
    ElementId::Name(id.into())
}

/// Eased progress of `spec` since `start` (1 under reduced motion).
fn since(spec: MotionSpec, start: Instant, now: Instant, reduced: bool) -> f32 {
    if reduced {
        1.
    } else {
        spec.progress_at(now.saturating_duration_since(start))
    }
}

struct ThreadState {
    messages: Vec<ThreadMessage>,
    /// Messages there at mount: they cascade in together.
    initial: usize,
    open: HashSet<SharedString>,
    toggled: HashMap<SharedString, Instant>,
    /// When the assistant started answering.
    thinking: Option<Instant>,
    /// A reply that took over from the working line: its author row is
    /// already on screen, so it doesn't rise in again.
    handoff: Option<SharedString>,
    now: SystemTime,
    input: Entity<TextInput>,
    scroll: ScrollHandle,
    /// Glide to the bottom: start offset and time.
    scroll_anim: Option<(f32, Instant)>,
    /// The conversation's height as last shown, and a growth in flight.
    visible: f32,
    grow: Option<(f32, Instant)>,
    sent: usize,
    /// Props mirrored from the latest render, for replies sent with Enter.
    on_reply: Option<ReplyHandler>,
    current_user: Option<ThreadAuthor>,
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
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder(placeholder)
                .multiline()
                .submit_on_enter()
        });
        let subscription =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Change(_) => cx.notify(),
                InputEvent::Submit(_) => this.send(cx),
                // Pink's draft: Escape cancels
                InputEvent::Escape => this.cancel(cx),
                InputEvent::Tab => window.focus_next(cx),
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
            initial: messages.len(),
            messages,
            open: HashSet::new(),
            toggled: HashMap::new(),
            thinking: None,
            handoff: None,
            now: SystemTime::now(),
            input,
            scroll: ScrollHandle::new(),
            scroll_anim: None,
            visible: 0.,
            grow: None,
            sent: 0,
            on_reply: None,
            current_user: None,
            reply: None,
            _tick: tick,
            _input: subscription,
        }
    }

    /// A message landed: grow to the new height and glide down to it.
    fn arrive(&mut self) {
        let now = Instant::now();
        if self.visible > 0. {
            self.grow = Some((self.visible, now));
        }
        self.scroll_anim = Some((f32::from(self.scroll.offset().y), now));
    }

    fn send(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().trim().to_string();
        let Some(author) = self.current_user.clone() else {
            return;
        };
        if text.is_empty() || self.thinking.is_some() {
            return;
        }
        self.input.update(cx, |input, cx| input.clear(cx));
        self.now = SystemTime::now();
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
        self.arrive();
        if let Some(answer) = self.on_reply.clone().and_then(|f| f(&text, cx)) {
            self.thinking = Some(Instant::now());
            self.reply = Some(cx.spawn(async move |this, cx| {
                let reply = answer.await;
                this.update(cx, |s, cx| {
                    s.now = SystemTime::now();
                    if let Some(reply) = reply {
                        s.handoff = Some(reply.id.clone());
                        s.messages.push(reply.created_at(s.now));
                    }
                    s.thinking = None;
                    s.arrive();
                    cx.notify();
                })
                .ok();
            }));
        }
        cx.notify();
    }

    /// Pink's draft Cancel: drops what was typed.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.clear(cx));
        cx.notify();
    }

    /// The working line's Stop: drops the pending answer.
    fn stop(&mut self, cx: &mut Context<Self>) {
        self.reply = None;
        self.thinking = None;
        cx.notify();
    }

    fn toggle(&mut self, id: &SharedString, cx: &mut Context<Self>) {
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
    anchor: Option<SharedString>,
    messages: Vec<ThreadMessage>,
    current_user: ThreadAuthor,
    assistant: Option<ThreadAuthor>,
    on_reply: Option<ReplyHandler>,
    on_close: Option<Handler>,
    min_height: Pixels,
    max_height: Pixels,
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
            anchor: None,
            messages,
            current_user,
            assistant: None,
            on_reply: None,
            on_close: None,
            min_height: px(0.),
            max_height: px(340.),
        }
    }

    /// The header label when there is no [`anchor`](Self::anchor), "Comment"
    /// by default.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Where the thread is pinned (`path:line`), cited in mono in the header
    /// and the reply card like Pink's comment cards.
    pub fn anchor(mut self, anchor: impl Into<SharedString>) -> Self {
        self.anchor = Some(anchor.into());
        self
    }

    /// The assistant that answers – used for the working line and the placeholder.
    pub fn assistant(mut self, assistant: ThreadAuthor) -> Self {
        self.assistant = Some(assistant);
        self
    }

    /// Called with every new reply. Return a task resolving to the
    /// assistant's message to let it answer (the working line shows
    /// meanwhile); return `None` to just post the reply.
    pub fn on_reply(
        mut self,
        handler: impl Fn(&str, &mut App) -> Option<ReplyTask> + 'static,
    ) -> Self {
        self.on_reply = Some(Rc::new(handler));
        self
    }

    /// Shows a close button in the header, revealed while the thread is hovered.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// The conversation never gets shorter than this.
    pub fn min_height(mut self, height: Pixels) -> Self {
        self.min_height = height;
        self
    }

    /// Taller conversations scroll (340px by default).
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }
}

impl RenderOnce for CommentThread {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let reduced = motion::reduced_motion(cx);
        let key = self.id.to_string();
        let placeholder: SharedString = match &self.assistant {
            Some(a) => format!("Reply, or ask @{}…", a.name).into(),
            None => "Reply…".into(),
        };
        let initial = self.messages;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| ThreadState::new(initial, placeholder, window, cx),
        );

        // Mirror the props the state needs to send with Enter, and the colors
        // (Pink `ComposerInput`: placeholder `text_faint`, accent caret)
        let input = state.update(cx, |s, cx| {
            s.on_reply = self.on_reply.clone();
            s.current_user = Some(self.current_user.clone());
            s.input.update(cx, |input, _| {
                input.set_colors(Some(t.text_faint), Some(t.caret), Some(t.selection))
            });
            s.input.clone()
        });
        // The field is a tab stop between the thread's controls and the buttons
        let _ = input.read(cx).focus_handle(cx).tab_stop(true);

        let now = Instant::now();
        let s = state.read(cx);
        let scroll = s.scroll.clone();
        let mut moving = false;

        // Glide to the bottom; the target is re-read as layout catches up
        let mut scrolled = false;
        if let Some((from, started)) = s.scroll_anim {
            let p = since(motion::SCROLL_GLIDE, started, now, reduced);
            let target = -f32::from(scroll.max_offset().y);
            scroll.set_offset(point(px(0.), px(motion::lerp(from, target, p))));
            moving |= p < 1.;
            scrolled = p >= 1.;
        }

        let space = space_width(window);
        let mut items: Vec<AnyElement> = Vec::new();
        // Messages there at mount cascade in one part after another; later
        // ones bring their own short stagger (Pink tool-row reveal)
        let mut cascade = 0;
        for (ix, msg) in s.messages.iter().enumerate() {
            let mut parts: Vec<AnyElement> = vec![
                meta_row(&msg.author, Some(time_ago(msg.created_at, s.now)), &t).into_any_element(),
            ];
            if !msg.text.is_empty() {
                parts.push(
                    div()
                        .mt(px(space::S2))
                        .ml(px(TEXT_INDENT))
                        .child(body(&msg.text, &t, space))
                        .into_any_element(),
                );
            }
            if let Some(status) = msg.status.clone() {
                let toggle: Option<Handler> = (!msg.details.is_empty()).then(|| {
                    let state = state.clone();
                    let id = msg.id.clone();
                    Rc::new(move |_: &mut Window, cx: &mut App| {
                        state.update(cx, |s, cx| s.toggle(&id, cx))
                    }) as Handler
                });
                let (block, active) = status_block(StatusBlock {
                    key: format!("{key}-status-{}", msg.id),
                    status,
                    icon: msg
                        .status_icon
                        .clone()
                        .unwrap_or_else(|| STEPS_GLYPH.into()),
                    details: &msg.details,
                    open: s.open.contains(&msg.id),
                    toggled: s.toggled.get(&msg.id).copied(),
                    now,
                    reduced,
                    toggle,
                    t: &t,
                });
                moving |= active;
                parts.push(block);
            }
            let handoff = s.handoff.as_ref() == Some(&msg.id);
            let parts = parts.into_iter().enumerate().map(|(k, part)| {
                let delay = if ix < s.initial {
                    cascade += 1;
                    Some(motion::tool_row_arrival_delay(cascade - 1, true))
                } else if handoff && k == 0 {
                    None
                } else {
                    Some(motion::tool_row_arrival_delay(k, false))
                };
                reveal(format!("{key}-in-{}-{k}", msg.id), delay, part)
            });
            items.push(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .children(parts.collect::<Vec<_>>())
                    .into_any_element(),
            );
        }
        if let (Some(started), Some(assistant)) = (s.thinking, &self.assistant) {
            let stop = {
                let state = state.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                    cx.stop_propagation();
                    state.update(cx, |s, cx| s.stop(cx));
                }
            };
            let secs = now.saturating_duration_since(started).as_secs() as i64;
            items.push(
                div()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(reveal(
                        format!("{key}-in-working-{}", s.sent),
                        Some(motion::tool_row_arrival_delay(0, false)),
                        meta_row(assistant, None, &t).into_any_element(),
                    ))
                    .child(div().mt(px(space::S2)).ml(px(TEXT_INDENT)).child(
                        working_line(&format!("{key}-working"), secs, &t).child(stop_button(
                            &format!("{key}-working"),
                            &t,
                            stop,
                        )),
                    ))
                    .into_any_element(),
            );
        }

        // The conversation: scrolls past the max height, never shorter than the min
        let list = div()
            .id(eid(format!("{key}-scroll")))
            .track_scroll(&scroll)
            .overflow_y_scroll()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(space::S12))
            .min_h(self.min_height)
            .max_h(self.max_height.max(self.min_height))
            .pt(px(space::S4))
            .pb(px(CARD_PAD_V / 2. + CARD_GAP))
            .children(items);
        // A message landing grows the band to its new height; the distance
        // sets the time (Pink `user_resize_spec`)
        let measured = f32::from(scroll.bounds().size.height);
        let mut grown = false;
        let height = s.grow.and_then(|(from, started)| {
            let spec = motion::user_resize_spec((measured - from).abs());
            let p = since(spec, started, now, reduced);
            grown = p >= 1.;
            moving |= p < 1.;
            (measured > 0. && p < 1.).then(|| motion::lerp(from, measured, p))
        });
        let visible = height.unwrap_or(measured);
        let morph = div()
            .flex_none()
            .overflow_hidden()
            .when_some(height, |d, h| d.h(px(h)))
            .child(list);

        // Pink: comment card header – the glyph, where it's pinned, and here
        // the count and a close revealed on hover (Pink's remove)
        let group: SharedString = format!("{key}-thread").into();
        let ring = t.focus_ring();
        let count = s.messages.len();
        let header = card_header(self.anchor.clone(), self.title.clone(), &t)
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(pink::text::CAPTION))
                    .line_height(px(pink::TextStyle::Caption.line_height()))
                    .font_features(pink::tabular_figures())
                    .text_color(t.text_faint)
                    .child(count_label(count)),
            )
            .when_some(self.on_close.clone(), |d, on_close| {
                d.child(
                    // Pink's remove: a 24 pt hit box, 16 pt footprint (DS-19)
                    comment_card::remove(svg().path(ICON_REMOVE), t.text_muted, group.clone())
                        .id(eid(format!("{key}-close")))
                        .role(Role::Button)
                        .aria_label("Close thread")
                        .tab_index(0)
                        .focus_visible(move |s| pink::focus_ring(s.opacity(1.), ring))
                        .on_click(move |_, window, cx| on_close(window, cx)),
                )
            });
        let thread = comment_card::band(&comment_card::posted_style(&t), None)
            .id(eid(format!("{key}-band")))
            .group(group)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .px(px(comment_card::PAD_X))
                    .pt(px(CARD_PAD_V / 2.))
                    .child(header)
                    .child(morph),
            );

        // Pink: comment draft – header, the 46px field, Cancel and Comment
        let cancel = {
            let state = state.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| s.cancel(cx))
            }
        };
        let commit = {
            let state = state.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| s.send(cx))
            }
        };
        let draft = comment_card::draft(&comment_card::draft_style(&t), None).child(
            comment_card::content(None)
                .child(card_header(self.anchor.clone(), self.title.clone(), &t))
                .child(comment_card::field(
                    div()
                        .text_size(px(pink::text::FOOTNOTE))
                        .line_height(px(CARD_LINE_HEIGHT))
                        .text_color(t.text)
                        .child(input),
                ))
                .child(
                    comment_card::actions()
                        .child(
                            comment_action(format!("{key}-cancel"), "Cancel", false, &t)
                                .on_click(cancel),
                        )
                        .child(
                            comment_action(format!("{key}-commit"), "Comment", true, &t)
                                .on_click(commit),
                        ),
                ),
        );

        if moving {
            window.request_animation_frame();
        }
        // Hover fades started in this render need their frames
        motion::keep_alive(window);
        state.update(cx, |s, _| {
            s.visible = visible;
            if scrolled {
                s.scroll_anim = None;
            }
            if grown {
                s.grow = None;
            }
        });

        div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .w_full()
            .font_family(pink::SANS)
            .text_size(px(pink::text::FOOTNOTE))
            .line_height(px(CARD_LINE_HEIGHT))
            .text_color(t.text)
            // Tab walks the thread's controls in order; the field hands on by itself
            .on_key_down(|event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" {
                    if event.keystroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                }
            })
            .child(thread)
            .child(draft)
    }
}

/// Pink comment_ui.rs card header: the comment glyph and the location in
/// mono (a plain title in sans when there's no anchor).
fn card_header(anchor: Option<SharedString>, title: SharedString, t: &Pink) -> gpui::Div {
    let label = match anchor {
        Some(anchor) => comment_card::location(anchor, t.text_faint, pink::MONO),
        None => comment_card::location(title, t.text_faint, pink::SANS),
    };
    comment_card::header(svg().path(comment_card::ICON_COMMENT), t.text_faint).child(label)
}

/// Pink comment_ui.rs `comment_action`: the primary is the `solid` plate;
/// the other is a ghost whose label and wash brighten under the pointer.
fn comment_action(id: String, label: &'static str, primary: bool, t: &Pink) -> Stateful<gpui::Div> {
    let ring = t.focus_ring();
    let action = comment_card::action(&comment_card::action_style(&id, primary, t), label)
        .id(eid(id.clone()))
        .role(Role::Button)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring));
    if primary {
        action
    } else {
        action.on_hover(motion::hover_listener(id))
    }
}

/// Width of a space in the body text, for laying out tagged text word by word.
fn space_width(window: &Window) -> Pixels {
    let run = TextRun {
        len: 1,
        font: font(pink::SANS),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(" ".into(), px(pink::text::FOOTNOTE), &[run], None)
        .width
}

/// Avatar, name and time (Pink history.rs author avatar; the time is the
/// sidebar's 11px tabular stamp).
fn meta_row(author: &ThreadAuthor, time: Option<String>, t: &Pink) -> gpui::Div {
    div()
        .h(px(AVATAR))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S8))
        .min_w_0()
        .child(author_avatar(author, t))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(pink::text::FOOTNOTE))
                .line_height(px(pink::TextStyle::Footnote.line_height()))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(author.name.clone()),
        )
        .when_some(time, |d, time| {
            d.child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(pink::text::CAPTION))
                    .line_height(px(pink::TextStyle::Caption.line_height()))
                    .font_features(pink::tabular_figures())
                    .text_color(t.content_faint)
                    .child(time),
            )
        })
}

/// Pink history.rs author avatar: a 20px disc, `hairline(0.12)` edge over
/// `wash(0.08)`, holding the photo, the initial, or an assistant's mark.
fn author_avatar(author: &ThreadAuthor, t: &Pink) -> AnyElement {
    match (&author.avatar, author.assistant) {
        (Some(path), _) => div()
            .size(px(AVATAR))
            .flex_none()
            .overflow_hidden()
            .rounded_full()
            .border_1()
            .border_color(t.hairline(0.12))
            .bg(t.wash(0.08))
            .child(img(path.clone()).size_full().rounded_full())
            .into_any_element(),
        (None, true) => div()
            .size(px(AVATAR))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .border_1()
            .border_color(t.hairline(0.12))
            .bg(t.wash(0.08))
            .child(
                svg()
                    .path(ASSISTANT_MARK)
                    .size(px(pink::icon_size::XS))
                    .text_color(t.text_faint),
            )
            .into_any_element(),
        (None, false) => pink::avatar(author.initial(), t).into_any_element(),
    }
}

/// A word of the body: plain, a leading `/command`, or an `@mention`.
enum Word<'a> {
    Plain(&'a str),
    Command(&'a str),
    /// The name and any punctuation after it.
    Mention(&'a str, &'a str),
}

fn words(text: &str) -> Vec<Word<'_>> {
    let name_end = |w: &str| {
        w.char_indices()
            .skip(1)
            .find(|(_, c)| !(c.is_alphanumeric() || *c == '_' || *c == '-'))
            .map_or(w.len(), |(i, _)| i)
    };
    text.split_whitespace()
        .enumerate()
        .map(|(ix, w)| {
            let end = name_end(w);
            if ix == 0 && w.starts_with('/') && end == w.len() && w.len() > 1 {
                Word::Command(w)
            } else if w.starts_with('@') && end > 1 {
                Word::Mention(&w[..end], &w[end..])
            } else {
                Word::Plain(w)
            }
        })
        .collect()
}

/// The note at Pink's card body (12/18 `text_dim`). Commands read as Pink's
/// inline code (`code_text` on a rounded `code_wash`, mono); mentions as
/// Pink's composer chat chips (accent on an accent wash). Tagged text is laid
/// out word by word (a GPUI text run can't hold a rounded plate) with the
/// space as the gap.
fn body(text: &SharedString, t: &Pink, space: Pixels) -> AnyElement {
    let words = words(text);
    let base = div()
        .text_size(px(pink::text::FOOTNOTE))
        .line_height(px(CARD_LINE_HEIGHT))
        .text_color(t.text_dim);
    if words.iter().all(|w| matches!(w, Word::Plain(_))) {
        return base.child(text.clone()).into_any_element();
    }
    let mention_wash = t.accent.opacity(if t.dark { 0.14 } else { 0.10 });
    let plate = |word: &str, fg: Hsla, wash: Hsla, mono: bool| {
        div().h(px(CARD_LINE_HEIGHT)).flex().items_center().child(
            div()
                .h(px(CARD_LINE_HEIGHT - 2. * INLINE_CODE_INSET_Y))
                .flex()
                .items_center()
                .px(px(INLINE_CODE_PAD_X))
                .rounded(px(INLINE_CODE_RADIUS))
                .bg(wash)
                .text_color(fg)
                .when(mono, |d| d.font_family(pink::MONO))
                .child(word.to_string()),
        )
    };
    base.flex()
        .flex_wrap()
        .items_center()
        .gap_x(space)
        .children(words.into_iter().map(|word| {
            match word {
                Word::Plain(w) => div().whitespace_nowrap().child(w.to_string()),
                Word::Command(w) => plate(w, t.code_text, t.code_wash, true),
                Word::Mention(name, rest) => div()
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .child(plate(name, t.accent, mention_wash, false))
                    .when(!rest.is_empty(), |d| d.child(rest.to_string())),
            }
        }))
        .into_any_element()
}

/// Rises a message part into place (Pink transcript.rs new-row reveal:
/// `TOOL_ROW_REVEAL` 360ms expo-out, 4px); `None` shows it as it is.
fn reveal(id: String, delay: Option<Duration>, part: AnyElement) -> AnyElement {
    match delay {
        Some(delay) => motion::rise_in(
            eid(id),
            motion::TOOL_ROW_REVEAL.with_delay(delay.as_millis() as u64),
            motion::TRANSCRIPT_IN_RISE,
            div().child(part),
        )
        .into_any_element(),
        None => part,
    }
}

/// Pink transcript.rs `render_working_trailer`: the breathing orb in its line
/// box, the shimmering verb and the elapsed timer, top-aligned at gap 8.
fn working_line(key: &str, secs: i64, t: &Pink) -> Stateful<gpui::Div> {
    div()
        .id(eid(key.to_string()))
        .group(SharedString::from(key.to_string()))
        .flex()
        .flex_row()
        .items_start()
        .gap(px(space::S8))
        .min_w_0()
        .child(loaders::orb_slot(t.accent))
        .child(loaders::working_status("Thinking", t.content_muted, t.text))
        .child(loaders::working_timer(secs, t.content_faint))
}

/// Pink's working-line Stop: a quiet way out that appears under the pointer.
fn stop_button(
    group: &str,
    t: &Pink,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<gpui::Div> {
    let ring = t.focus_ring();
    div()
        .id(eid(format!("{group}-stop")))
        .role(Role::Button)
        .aria_label("Stop the reply")
        .tab_index(0)
        .flex_none()
        .ml(px(space::S4))
        .px(px(space::S6))
        .rounded(px(radius::SM))
        .cursor_pointer()
        .text_size(px(loaders::WORKING_TIMER_SIZE))
        .line_height(px(loaders::WORKING_STATUS_LINE_HEIGHT))
        .text_color(t.content_muted)
        .opacity(0.)
        .group_hover(SharedString::from(group.to_string()), |s| s.opacity(1.))
        .focus_visible(move |s| pink::focus_ring(s.opacity(1.), ring))
        .hover_wash(format!("{group}-stop"), t.fill(FillTier::Tertiary))
        .child("Stop")
        .on_click(on_click)
}

struct StatusBlock<'a> {
    key: String,
    status: SharedString,
    icon: SharedString,
    details: &'a [SharedString],
    open: bool,
    toggled: Option<Instant>,
    now: Instant,
    reduced: bool,
    toggle: Option<Handler>,
    t: &'a Pink,
}

/// The assistant's summary: Pink's tool-group header plate (glyph on the
/// rail's trunk, the summary, a chevron turning 0 → 90°), folding open onto
/// Pink's activity rail with one 32px row per detail. Chevron, height and
/// body fade share Pink's fold clock. Returns whether it is animating.
fn status_block(block: StatusBlock) -> (AnyElement, bool) {
    let StatusBlock {
        key,
        status,
        icon,
        details,
        open,
        toggled,
        now,
        reduced,
        toggle,
        t,
    } = block;
    let has_details = toggle.is_some();
    let elapsed = toggled
        .map(|at| now.saturating_duration_since(at))
        .filter(|e| !reduced && *e < motion::FOLD_TWEEN_WINDOW.mul_f32(motion::speed_scale()));
    let (progress, body_alpha) = motion::fold_progress(open, elapsed);
    let ring = t.focus_ring();
    let (rest, hover) = (t.fill(FillTier::Quaternary), t.fill(FillTier::Tertiary));
    let tone = if has_details {
        motion::hover_blend(&key, t.content_muted, t.text)
    } else {
        t.content_muted
    };

    let plate = div()
        .id(eid(key.clone()))
        .rounded(px(radius::SM))
        .flex_none()
        .max_w_full()
        .min_w_0()
        .h(px(TOOL_GROUP_HEADER_HEIGHT - 2.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S6))
        .pl(px(ACTIVITY_TRUNK_X - ACTIVITY_ICON_SIZE / 2.))
        .pr(px(space::S8))
        .text_size(px(loaders::TOOL_LABEL_SIZE))
        .line_height(px(loaders::TOOL_LABEL_LINE_HEIGHT))
        .map(|p| match toggle {
            Some(toggle) => p
                .role(Role::Button)
                .aria_label(format!("Details: {status}"))
                .aria_expanded(open)
                .tab_index(0)
                .focus_visible(move |s| pink::focus_ring(s, ring))
                .cursor_pointer()
                .hover_fill_fg(key.clone(), rest, hover, t.content_muted, t.text)
                .on_click(move |_, window, cx| toggle(window, cx)),
            None => p.bg(rest).text_color(t.content_muted),
        })
        .child(
            svg()
                .path(icon)
                .flex_none()
                .size(px(ACTIVITY_ICON_SIZE))
                .text_color(tone),
        )
        .child(
            div()
                .min_w_0()
                .h(px(loaders::TOOL_LABEL_LINE_HEIGHT))
                .flex()
                .items_center()
                .truncate()
                .child(status),
        )
        .when(has_details, |p| {
            p.child(
                svg()
                    .path(ICON_CHEVRON)
                    .flex_none()
                    .size(px(DISCLOSURE_CHEVRON_SIZE))
                    .with_transformation(Transformation::rotate(radians(FRAC_PI_2 * progress)))
                    .text_color(t.content_faint),
            )
        });

    let mut column = div()
        .mt(px(space::S4))
        .ml(px(TEXT_INDENT))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(TOOL_GROUP_HEADER_HEIGHT))
                .flex()
                .flex_row()
                .items_center()
                .child(plate),
        );
    if has_details && progress > 0. {
        let full = CHIPS_TOP_PAD + details.len() as f32 * TOOL_TREE_ROW_HEIGHT;
        let last = details.len().saturating_sub(1);
        column = column.child(
            div().h(px(full * progress)).overflow_hidden().child(
                div()
                    .relative()
                    .top(px(motion::TOOL_BODY_RISE * (1. - body_alpha)))
                    .opacity(body_alpha)
                    .pt(px(CHIPS_TOP_PAD))
                    .flex()
                    .flex_col()
                    .children(
                        details
                            .iter()
                            .enumerate()
                            .map(|(ix, detail)| detail_row(detail.clone(), ix < last, t)),
                    ),
            ),
        );
    }
    (column.into_any_element(), elapsed.is_some())
}

/// One detail on Pink's activity rail: the trunk (continuing past the row
/// when more follow) rounding its 6px elbow to the branch tip, then the
/// detail at the tool rows' 12/18 `content_muted`.
fn detail_row(detail: SharedString, continues: bool, t: &Pink) -> gpui::Div {
    div()
        .h(px(TOOL_TREE_ROW_HEIGHT))
        .w_full()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .relative()
                .w(px(DETAIL_TEXT_X))
                .h_full()
                .flex_none()
                .child(rail(continues, t.hairline(0.12))),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(loaders::TOOL_LABEL_SIZE))
                .line_height(px(loaders::TOOL_LABEL_LINE_HEIGHT))
                .text_color(t.content_muted)
                .child(detail),
        )
}

/// Pink transcript.rs `activity_rail`, settled: the trunk and the elbow as
/// 1px ribbons unioned into one nonzero fill, so the fork never darkens.
fn rail(continues: bool, color: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let x = bounds.origin.x + px(ACTIVITY_TRUNK_X);
            let row_h = f32::from(bounds.size.height);
            let bend_y = bounds.origin.y + px(row_h / 2. - ACTIVITY_BEND_RADIUS);
            let mut tree = PathBuilder::fill().with_style(PathStyle::Fill(
                FillOptions::default().with_fill_rule(FillRule::NonZero),
            ));
            let bottom = if continues {
                bounds.origin.y + bounds.size.height
            } else {
                bend_y
            };
            ribbon(&mut tree, &[point(x, bounds.origin.y), point(x, bottom)]);
            let branch: Vec<_> = branch_points()
                .into_iter()
                .map(|p| point(x + px(p.x), bend_y + px(p.y)))
                .collect();
            ribbon(&mut tree, &branch);
            if let Ok(path) = tree.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .inset_0()
}

/// Pink transcript.rs `activity_branch_points`: the quarter bend from the
/// trunk, then straight out to the branch tip.
fn branch_points() -> Vec<Point<f32>> {
    let mut path: Vec<_> = (0..=24)
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
    path
}

/// Pink transcript.rs `activity_ribbon`: a clockwise 1px contour along `points`.
fn ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
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

/// Drawn stand-ins for the demo's profile photos.
const LENA: &str = "icons/comment-thread/lena.svg";
const SOFIA: &str = "icons/comment-thread/sofia.svg";

/// What Atlas answers when it is asked, in turn.
const REPLIES: [&str; 3] = [
    "Done. I also updated the meta description.",
    "Good catch. I'll leave a note for the copy review.",
    "On it. You'll see the change in a minute.",
];

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    let atlas = ThreadAuthor::new("Atlas").assistant();
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
            .status("Edited pricing.md")
            .status_icon(EDIT_GLYPH)
            .details([
                "Renamed 12 mentions in 3 sections",
                "Kept “customer support” as it is",
                "Left the legal pages untouched",
            ]),
    ];
    let asked = Rc::new(std::cell::Cell::new(0usize));
    story::view(cx, move |_, _| {
        let (atlas, asked) = (atlas.clone(), asked.clone());
        div().w(px(480.)).child(
            CommentThread::new(
                "comment-thread",
                messages.clone(),
                ThreadAuthor::new("Sofia Marin").avatar(SOFIA),
            )
            .anchor("site/pricing.md:14")
            .assistant(atlas.clone())
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
            }),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_ago_matches_pink() {
        let now = SystemTime::now();
        let before = |s: u64| now - Duration::from_secs(s);
        assert_eq!(time_ago(before(10), now), "now");
        assert_eq!(time_ago(before(59), now), "now");
        assert_eq!(time_ago(before(3 * 60), now), "3m");
        assert_eq!(time_ago(before(2 * 3600), now), "2h");
        assert_eq!(time_ago(before(3 * 86400), now), "3d");
        assert_eq!(time_ago(before(7 * 86400), now), "1w");
        assert_eq!(time_ago(before(35 * 86400), now), "1mo");
        assert_eq!(time_ago(before(400 * 86400), now), "1y");
        // Clock skew never shows a negative time
        assert_eq!(time_ago(now + Duration::from_secs(5), now), "now");
    }

    #[test]
    fn initial_is_pinks() {
        assert_eq!(ThreadAuthor::new("Lena Lindqvist").initial(), "L");
        assert_eq!(ThreadAuthor::new("  josé").initial(), "J");
        assert_eq!(ThreadAuthor::new("   ").initial(), "?");
    }

    #[test]
    fn commands_and_mentions() {
        let w = words("/edit Ask @atlas, then /edit");
        assert!(matches!(w[0], Word::Command("/edit")));
        assert!(matches!(w[2], Word::Mention("@atlas", ",")));
        assert!(matches!(w[4], Word::Plain("/edit")));
        assert!(
            words("a / b") // joiners: allow parser input, not a label
                .iter()
                .all(|w| matches!(w, Word::Plain(_)))
        );
    }
}
