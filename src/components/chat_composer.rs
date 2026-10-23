//! Chat Composer – a pink component (pink: `composer.rs` `Composer`).
//!
//! PINK's prompt pill: a 26 pt rounded plate with a hairline edge. Staged
//! attachments sit above the text as 56 pt square thumbnails, the field
//! grows from one line to 260 pt, and a 46 pt controls row holds the
//! paperclip, ghost picker chips (model, access, web search) and the
//! Send disc, which is recessed while the draft is empty, solid once there
//! is something to send, and turns into Queue or Stop while a turn is
//! running. A project chip sits in the checkout row above the pill. Every
//! menu opens upward from its chip and works from the keyboard. Voice input
//! keeps the field's place with a breathing orb, the words heard so far and
//! a timer; they land in the field for editing. Enter sends, Shift+Enter
//! adds a line.
//!
//! ```ignore
//! ChatComposer::new("composer")
//!     .attachments(files)
//!     .running(agent_busy)
//!     .on_attach(|_, cx| { /* open a file picker */ })
//!     .on_send(|message, _, cx| println!("{} via {}", message.text, message.model))
//!     .on_stop(|_, cx| { /* interrupt the turn */ })
//! ```

use std::{cell::Cell, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnyElement, AnyView, App, Context, Div, ElementId, Entity, FocusHandle, FontWeight,
    IntoElement, KeyDownEvent, MouseButton, ObjectFit, Role, SharedString, Stateful, Subscription,
    Task, Toggled, Window, anchored, canvas, deferred, div, img, point, prelude::*, px, relative,
    svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt},
    pink::{self, FillTier, Pink, TextStyle, TypeStyleExt, layout, loaders, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Chat Composer";
pub const DESCRIPTION: &str = "The composer's prompt pill – square attachment thumbnails over a field that grows as you type, quiet model, access and web search chips whose menus open upward, voice input, a project chip above, and a Send disc that turns into Queue or Stop while a turn runs";

const ICON_PAPERCLIP: &str = "icons/chat-composer/paperclip.svg";
const ICON_ARROW_UP: &str = "icons/chat-composer/arrow-up.svg";
const ICON_QUEUE_SEND: &str = "icons/chat-composer/queue-send.svg";
const ICON_CLOSE: &str = "icons/chat-composer/close.svg";
const ICON_FOLDER: &str = "icons/chat-composer/folder.svg";
const ICON_GLOBE: &str = "icons/chat-composer/globe.svg";
const ICON_LOCK: &str = "icons/chat-composer/lock-keyhole.svg";
const ICON_UNLOCKED: &str = "icons/chat-composer/lock-keyhole-unlocked.svg";
const ICON_MIC: &str = "icons/chat-composer/mic.svg";
const ICON_FILE: &str = "icons/chat-composer/file-text.svg";
const ICON_FILE_IMAGE: &str = "icons/chat-composer/file-image.svg";
pub const ICON_QUICK: &str = "icons/chat-composer/bolt-fast.svg";
pub const ICON_BALANCED: &str = "icons/chat-composer/magic-wand.svg";
pub const ICON_DEEP: &str = "icons/chat-composer/brain.svg";
const DEMO_HERO: &str = "icons/chat-composer/launch-hero.svg";

// Pill geometry (pink `composer.rs` layout constants, points at 16 px).

/// `TEXTAREA_PAD_TOP` / `TEXTAREA_PAD_V`: the field's `pt-4 pb-1`.
const TEXTAREA_PAD_TOP: f32 = 16.0;
const TEXTAREA_PAD_BOTTOM: f32 = 4.0;
/// `SESSION_TEXTAREA_MIN` and `TEXTAREA_MAX`: the field box (padding
/// included) grows between one line with room above the controls and 260.
const TEXTAREA_MIN: f32 = 60.0;
const TEXTAREA_MAX: f32 = 260.0;
/// `input_side_padding` at a full-width composer.
const INPUT_SIDE_PAD: f32 = 16.0;
/// `INPUT_TEXT_SIZE` / `INPUT_LINE_HEIGHT`: `text-[14px] leading-relaxed`.
const INPUT_TEXT_SIZE: f32 = 14.0;
const INPUT_LINE_HEIGHT: f32 = 22.75;
/// `ACTIONS_ROW_HEIGHT`, `ACTIONS_ROW_INSET`, `ACTION_UTILITY_GAP`,
/// `ACTION_PRIMARY_GAP`, `CHIP_GAP` and `ROW_BUTTON`.
const ACTIONS_ROW_HEIGHT: f32 = 46.0;
const ACTIONS_ROW_INSET: f32 = 10.0;
const ACTION_UTILITY_GAP: f32 = 2.0;
const ACTION_PRIMARY_GAP: f32 = 8.0;
const CHIP_GAP: f32 = 4.0;
const ROW_BUTTON: f32 = layout::COMPOSER_CONTROL;
/// `STRIP_THUMB`, `STRIP_GAP`, `STRIP_PAD_TOP`, `STRIP_PAD_X`.
const STRIP_THUMB: f32 = 56.0;
const STRIP_GAP: f32 = 8.0;
const STRIP_PAD_TOP: f32 = 12.0;
const STRIP_PAD_X: f32 = 16.0;
/// `CONTEXT_ROW_HEIGHT`: the checkout row above the pill.
const CONTEXT_ROW_HEIGHT: f32 = 24.0;
/// Column gap between the checkout row and the pill (`Theme::SPACE_SM`).
const COLUMN_GAP: f32 = 8.0;
/// pink `pickers.rs` `TARGET_CHIP_*` / `FOOTER_CHIP_RADIUS`.
const TARGET_CHIP_HEIGHT: f32 = 22.0;
const TARGET_CHIP_PAD_X: f32 = 6.0;
const TARGET_CHIP_TEXT: f32 = 11.5;
const TARGET_CHIP_GLYPH: f32 = 12.0;
const TARGET_CHIP_GAP: f32 = 5.0;
const TARGET_CHIP_CHEVRON: f32 = 10.0;
const TARGET_CHIP_RADIUS: f32 = 6.0;
/// pink `goals.rs` `GOAL_CHIP_ICON`: the mode-on chip's glyph.
const MODE_CHIP_ICON: f32 = 15.0;
/// pink `pickers.rs`: the permission popover is 340 wide.
const ACCESS_MENU_W: f32 = 340.0;
/// pink `max-w-3xl` (768) less the column's 16 pt gutters.
const DEFAULT_WIDTH: f32 = layout::COMPOSER_MAX_WIDTH - 32.0;
/// A press on an open menu's own chip closes it (pink
/// `note_trigger_press_matching`); the click that follows must not reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(250);

type SendHandler = Rc<dyn Fn(&Message, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// A model in the menu.
#[derive(Clone)]
pub struct ComposerModel {
    pub name: SharedString,
    pub description: SharedString,
    pub icon: SharedString,
}

impl ComposerModel {
    pub fn new(
        name: impl Into<SharedString>,
        description: impl Into<SharedString>,
        icon: impl Into<SharedString>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            icon: icon.into(),
        }
    }
}

/// Quick, balanced and deep.
pub fn default_models() -> Vec<ComposerModel> {
    vec![
        ComposerModel::new("Quick", "Fast replies for everyday questions", ICON_QUICK),
        ComposerModel::new("Balanced", "Careful writing and analysis", ICON_BALANCED),
        ComposerModel::new("Deep", "Thinks longer on hard problems", ICON_DEEP),
    ]
}

/// A file waiting to go with the message.
#[derive(Clone)]
pub struct Attachment {
    pub name: SharedString,
    /// Kind and size ("PDF · 2.4 MB"), shown in the thumbnail's tooltip.
    pub detail: SharedString,
    pub image: bool,
    /// An image path or asset the thumbnail shows, cropped to fill.
    pub preview: Option<SharedString>,
}

impl Attachment {
    pub fn file(name: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            detail: detail.into(),
            image: false,
            preview: None,
        }
    }

    pub fn image(name: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self {
            image: true,
            ..Self::file(name, detail)
        }
    }

    /// The picture the thumbnail shows (pink stages image thumbnails).
    pub fn preview(mut self, source: impl Into<SharedString>) -> Self {
        self.preview = Some(source.into());
        self
    }
}

/// What `on_send` receives.
#[derive(Clone)]
pub struct Message {
    pub text: SharedString,
    pub attachments: Vec<Attachment>,
    pub model: SharedString,
    pub project: Option<SharedString>,
    pub web_search: bool,
    pub ask_first: bool,
    /// Sent while a turn was running: it waits for that turn to end.
    pub queued: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Model,
    Access,
    Project,
}

/// The Send disc's job (pink `SendButtonMode`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum SendMode {
    Send,
    Queue,
    Stop,
}

/// pink `send_button_mode`.
fn send_mode(running: bool, has_content: bool) -> SendMode {
    match (running, has_content) {
        (false, _) => SendMode::Send,
        (true, true) => SendMode::Queue,
        (true, false) => SendMode::Stop,
    }
}

struct State {
    input: Entity<TextInput>,
    menu_focus: FocusHandle,
    has_text: bool,
    model: usize,
    project: usize,
    web: bool,
    ask_first: bool,
    menu: Option<Menu>,
    /// The menu that closed last and when: it fades out over `MENU_OUT`
    /// (pink's `Popup` closing phase), and its chip ignores the click that
    /// closed it.
    closed: Option<(Menu, Instant)>,
    highlight: usize,
    listen_started: Option<Instant>,
    placeholder: SharedString,
    running: bool,
    /// The pill's natural content height, measured after layout; the pill
    /// eases toward it (pink's committed-height morph).
    natural: Rc<Cell<Option<f32>>>,
    /// What a send needs besides the text, refreshed every render.
    snapshot: Option<(Vec<Attachment>, SharedString, Option<SharedString>)>,
    on_send: Option<SendHandler>,
    _input_events: Subscription,
}

impl State {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).multiline().submit_on_enter());
        let sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut State, _, ev, window, cx| match ev {
                InputEvent::Change(text) => {
                    this.has_text = !text.trim().is_empty();
                    cx.notify();
                }
                InputEvent::Submit(_) => this.send(window, cx),
                _ => {}
            },
        );
        State {
            input,
            menu_focus: cx.focus_handle(),
            has_text: false,
            model: 1,
            project: 0,
            web: false,
            ask_first: true,
            menu: None,
            closed: None,
            highlight: 0,
            listen_started: None,
            placeholder: SharedString::default(),
            running: false,
            natural: Rc::default(),
            snapshot: None,
            on_send: None,
            _input_events: sub,
        }
    }

    /// pink `on_submit`: Enter never stops a run, and an empty draft sends
    /// nothing; a running turn queues the message.
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = self.input.read(cx).text().trim().to_string().into();
        let Some((attachments, model, project)) = self.snapshot.clone() else {
            return;
        };
        if self.listen_started.is_some() || (text.is_empty() && attachments.is_empty()) {
            return;
        }
        self.input.update(cx, |i, cx| i.clear(cx));
        self.has_text = false;
        let message = Message {
            text,
            attachments,
            model,
            project,
            web_search: self.web,
            ask_first: self.ask_first,
            queued: self.running,
        };
        if let Some(f) = self.on_send.clone() {
            f(&message, window, cx);
        }
        cx.notify();
    }

    fn close_menu(&mut self) {
        if let Some(menu) = self.menu.take() {
            self.closed = Some((menu, Instant::now()));
        }
    }

    fn selected(&self, menu: Menu) -> usize {
        match menu {
            Menu::Model => self.model,
            Menu::Access => usize::from(!self.ask_first),
            Menu::Project => self.project,
        }
    }

    /// `None` while `menu` is closed and settled, `Some(None)` while open,
    /// `Some(Some(t))` while fading out at exit progress `t`.
    fn mounted(&self, menu: Menu) -> Option<Option<f32>> {
        if self.menu == Some(menu) {
            return Some(None);
        }
        let (closed, at) = self.closed?;
        let t = motion::menu_exit_progress(at);
        (closed == menu && t < 1.0).then_some(Some(t))
    }
}

#[derive(IntoElement)]
pub struct ChatComposer {
    id: ElementId,
    models: Vec<ComposerModel>,
    projects: Vec<SharedString>,
    attachments: Vec<Attachment>,
    placeholder: SharedString,
    listening: Option<SharedString>,
    running: bool,
    width: f32,
    on_send: Option<SendHandler>,
    on_stop: Option<Handler>,
    on_attach: Option<Handler>,
    on_remove: Option<IndexHandler>,
    on_listen: Option<ToggleHandler>,
}

impl ChatComposer {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            models: default_models(),
            projects: Vec::new(),
            attachments: Vec::new(),
            placeholder: "Do anything…".into(),
            listening: None,
            running: false,
            width: DEFAULT_WIDTH,
            on_send: None,
            on_stop: None,
            on_attach: None,
            on_remove: None,
            on_listen: None,
        }
    }

    pub fn models(mut self, models: Vec<ComposerModel>) -> Self {
        self.models = models;
        self
    }

    /// Projects for the chip above the pill; the row hides without them.
    pub fn projects(mut self, projects: Vec<SharedString>) -> Self {
        self.projects = projects;
        self
    }

    pub fn attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// The empty field's hint (pink: "Do anything…").
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Voice input in progress, with the words heard so far.
    pub fn listening(mut self, transcript: Option<SharedString>) -> Self {
        self.listening = transcript;
        self
    }

    /// A turn is running: Send queues a draft, and an empty draft offers Stop.
    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn on_send(mut self, handler: impl Fn(&Message, &mut Window, &mut App) + 'static) -> Self {
        self.on_send = Some(Rc::new(handler));
        self
    }

    /// The Stop disc of a running turn.
    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }

    pub fn on_attach(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_attach = Some(Rc::new(handler));
        self
    }

    pub fn on_remove_attachment(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }

    /// `true` to start listening; `false` to stop (the transcript is then
    /// placed in the field).
    pub fn on_listen(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_listen = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------

/// A 28 pt round utility button (pink: `Composer::render` attach): clear at
/// rest, `ink(0.10)` on hover or while `open`, a 16 pt glyph in
/// `text_muted`.
fn row_button(
    id: impl Into<SharedString>,
    icon: &'static str,
    label: &'static str,
    open: bool,
    nudge: f32,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let key = id.to_string();
    let ring = t.focus_ring();
    let wash = t.ink(0.10);
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .size(px(ROW_BUTTON))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .cursor_pointer()
        .bg(if open {
            wash
        } else {
            motion::hover_blend(&key, gpui::transparent_black(), wash)
        })
        .on_hover(motion::hover_listener(key))
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            svg()
                .path(icon)
                .size(px(16.))
                // The paperclip's ink sits left of its viewbox center.
                .relative()
                .left(px(nudge))
                .text_color(if open { t.text } else { t.text_muted }),
        )
}

/// The Send / Queue / Stop disc (pink `Composer::render_send_button`): one
/// stable 28 pt circle. Solid `text` with a `bg` glyph when it can act
/// (hover 0.88, press 0.76); recessed `ink(0.08)` with the glyph at 35%
/// while there is nothing to send, the arrow taking the accent when the
/// field is focused and empty. Only the glyph changes between modes,
/// fading in over `GLYPH_SWAP`.
fn send_disc(key: &str, mode: SendMode, recessed: bool, focused: bool, t: &Pink) -> Stateful<Div> {
    let ready = recessed && focused;
    let glyph_color = if ready {
        t.readable_tone(t.accent, 1.0)
    } else if recessed {
        t.text.opacity(0.35)
    } else {
        t.bg
    };
    let state_key = match (mode, recessed) {
        (SendMode::Stop, _) => "stop",
        (SendMode::Queue, false) => "queue",
        (_, false) => "send",
        (_, true) if ready => "ready",
        (_, true) => "rest",
    };
    let label = match mode {
        SendMode::Stop => "Stop",
        SendMode::Queue => "Queue (⏎)",
        SendMode::Send => "Send (⏎)",
    };
    let glyph: AnyElement = match mode {
        SendMode::Stop => div()
            .size(px(11.))
            .rounded(px(3.))
            .bg(glyph_color)
            .into_any_element(),
        SendMode::Queue => svg()
            .path(ICON_QUEUE_SEND)
            .size(px(14.))
            .text_color(glyph_color)
            .into_any_element(),
        SendMode::Send => svg()
            .path(ICON_ARROW_UP)
            .size(px(14.))
            .text_color(glyph_color)
            .into_any_element(),
    };
    let glyph = div()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph)
        .with_animation(
            SharedString::from(format!("{key}-glyph-{state_key}")),
            motion::GLYPH_SWAP.animation(),
            |el, p| el.opacity(p),
        );
    let fade = format!("{key}-send");
    let ring = t.focus_ring();
    let disc = div()
        .id(SharedString::from(fade.clone()))
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .size(px(ROW_BUTTON))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(glyph);
    if recessed {
        disc.bg(t.ink(0.08))
    } else {
        disc.cursor_pointer()
            .bg(motion::hover_blend(&fade, t.text, t.text.opacity(0.88)))
            .on_hover(motion::hover_listener(fade))
            .active(|s| s.opacity(0.76))
    }
}

/// The staged-attachment strip (pink `Composer::render_attachment_strip`):
/// wrapping 56 pt thumbnails, radius 8, a `hairline(0.10)` edge, 8 apart
/// inside `px 16 pt 12`. A picture fills its thumbnail; other files show
/// their kind on a resting plate. The 20 pt raised remove disc overhangs the
/// corner and shows on hover or keyboard focus.
fn attachment_strip(
    key: &str,
    attachments: &[Attachment],
    on_remove: Option<IndexHandler>,
    t: &Pink,
) -> Option<Div> {
    if attachments.is_empty() {
        return None;
    }
    let ring = t.focus_ring();
    Some(
        div()
            .w_full()
            .flex_none()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(STRIP_GAP))
            .px(px(STRIP_PAD_X))
            .pt(px(STRIP_PAD_TOP))
            .children(attachments.iter().enumerate().map(|(ix, file)| {
                let group: SharedString = format!("{key}-att-{ix}").into();
                let on_remove = on_remove.clone();
                let thumb = match &file.preview {
                    // 56 − 2: the frame minus its 1 pt edge; 7 = 8 − edge.
                    Some(source) => img(source.clone())
                        .w(px(STRIP_THUMB - 2.))
                        .h(px(STRIP_THUMB - 2.))
                        .rounded(px(radius::MD - 1.))
                        .object_fit(ObjectFit::Cover)
                        .into_any_element(),
                    None => file_plate(file, t).into_any_element(),
                };
                div()
                    .group(group.clone())
                    .flex_none()
                    .relative()
                    .child(
                        div()
                            .id(ElementId::NamedInteger("cc-thumb".into(), ix as u64))
                            .size(px(STRIP_THUMB))
                            .rounded(px(radius::MD))
                            .overflow_hidden()
                            .border_1()
                            .border_color(t.hairline(0.10))
                            .tooltip(pink::text_tooltip(format!(
                                "{} · {}",
                                file.name, file.detail
                            )))
                            .tooltip_show_delay(layout::TOOLTIP_DELAY)
                            .child(thumb),
                    )
                    .child(
                        div()
                            .id(ElementId::NamedInteger("cc-thumb-x".into(), ix as u64))
                            .role(Role::Button)
                            .aria_label("Remove attachment")
                            .tab_index(0)
                            .on_key_down(pink::swallow_activation_key)
                            .absolute()
                            .top(px(-7.))
                            .right(px(-7.))
                            .size(px(20.))
                            .rounded_full()
                            .bg(t.surface_raised)
                            .border_1()
                            .border_color(t.border_strong)
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .shadow(pink::shadow_sm())
                            .opacity(0.)
                            .group_hover(group, |s| s.opacity(1.))
                            .focus_visible(move |s| pink::focus_ring(s.opacity(1.), ring))
                            .on_click(move |_, window, cx| {
                                // The disc overhangs the thumbnail: this click
                                // is the disc's alone.
                                cx.stop_propagation();
                                if let Some(f) = &on_remove {
                                    f(ix, window, cx)
                                }
                            })
                            .child(svg().path(ICON_CLOSE).size(px(10.)).text_color(t.text)),
                    )
            })),
    )
}

/// A thumbnail without a picture: the file's glyph over its extension on a
/// resting plate.
fn file_plate(file: &Attachment, t: &Pink) -> Div {
    let kind = file
        .name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_uppercase())
        .unwrap_or_default();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .bg(t.fill(FillTier::Quaternary))
        .child(
            svg()
                .path(if file.image {
                    ICON_FILE_IMAGE
                } else {
                    ICON_FILE
                })
                .size(px(pink::icon_size::LG))
                .text_color(t.text_muted),
        )
        .when(!kind.is_empty(), |d| {
            d.child(
                div()
                    .type_style(TextStyle::Caption2)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.readable_muted(0.85))
                    .child(kind),
            )
        })
}

/// A chip in the checkout row above the pill (pink `Pickers::target_chip`):
/// 22 pt, `px 6`, radius 6, a 12 pt glyph and 11.5 pt label in
/// `readable_muted(0.7)` brightening to `text@0.85`, the Tertiary wash on
/// hover or while open, and a 10 pt chevron.
fn target_chip(
    id: impl Into<SharedString>,
    glyph: &'static str,
    label: SharedString,
    open: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let key = id.to_string();
    let rest = t.readable_muted(0.7);
    let lit = t.text.opacity(0.85);
    let ink = if open {
        lit
    } else {
        motion::hover_blend(&key, rest, lit)
    };
    let wash = t.fill(FillTier::Tertiary);
    let ring = t.focus_ring();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(SharedString::from(format!("Project: {label}")))
        .aria_expanded(open)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .relative()
        .h(px(TARGET_CHIP_HEIGHT))
        .min_w_0()
        .max_w(px(180.))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(TARGET_CHIP_GAP))
        .px(px(TARGET_CHIP_PAD_X))
        .rounded(px(TARGET_CHIP_RADIUS))
        .text_size(px(TARGET_CHIP_TEXT))
        .text_color(ink)
        .cursor_pointer()
        .bg(if open {
            wash
        } else {
            motion::hover_blend(&key, wash.opacity(0.), wash)
        })
        .on_hover(motion::hover_listener(key))
        .child(
            svg()
                .path(glyph)
                .size(px(TARGET_CHIP_GLYPH))
                .flex_none()
                .text_color(ink),
        )
        .child(div().min_w_0().truncate().child(label))
        .child(pink::line_glyph(
            pink::glyphs::ALT_ARROW_DOWN,
            TARGET_CHIP_CHEVRON,
            t.readable_muted(0.5),
        ))
}

/// A menu floating above its chip, from the chip's leading edge (pink
/// `popover::anchored_menu_above_leading`): shifted left by the card inset
/// so row text lines up with the chip glyph, 6 pt above it, kept 8 pt inside
/// the window. It rises 6 pt in over `MENU_IN`; closing (`exit`) fades it
/// over `MENU_OUT` under an occluder so its rows take no clicks.
fn menu_above(id: &str, card: AnyElement, exit: Option<f32>) -> AnyElement {
    let inner = div().occlude().pb(px(layout::POPOVER_GAP)).child(card);
    let moving = match exit {
        Some(t) => motion::menu_out(
            SharedString::from(format!("{id}-out")),
            t,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(
            SharedString::from(id.to_string()),
            motion::MENU_RISE_UP,
            inner,
        )
        .into_any_element(),
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_0()
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::BottomLeft)
                    .offset(point(px(-layout::CARD_INSET), px(0.)))
                    .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                    .child(moving),
            )
            .priority(1),
        )
        .into_any_element()
}

/// The popover card around a menu's rows (pink `Pickers::popover_frame`):
/// the kit card at `width`, keyboard driven while open — arrows, Home/End,
/// Enter or Space to pick, Escape to close, ⌘1–9 for a model — and closed
/// by a press outside it.
fn menu_card(
    kind: Menu,
    count: usize,
    width: f32,
    open: bool,
    focus: &FocusHandle,
    state: &Entity<State>,
    t: &Pink,
) -> Stateful<Div> {
    let card = pink::popover_card(t)
        .id(ElementId::NamedInteger("cc-menu".into(), kind as u64))
        .w(px(width))
        .max_h(px(640.))
        .flex()
        .flex_col();
    if !open {
        return card;
    }
    let keys = state.clone();
    let out = state.clone();
    card.track_focus(focus)
        .on_key_down(move |ev: &KeyDownEvent, window, cx| {
            let key = ev.keystroke.key.as_str();
            let digit = ev
                .keystroke
                .modifiers
                .platform
                .then(|| key.parse::<usize>().ok())
                .flatten()
                .filter(|n| kind == Menu::Model && (1..=count.min(9)).contains(n));
            let mut chosen = None;
            let mut escaped = false;
            match key {
                "down" | "up" | "home" | "end" if count > 0 => keys.update(cx, |s, cx| {
                    s.highlight = match key {
                        "down" => (s.highlight + 1) % count,
                        "up" => (s.highlight + count - 1) % count,
                        "home" => 0,
                        _ => count - 1,
                    };
                    cx.notify();
                }),
                "enter" | "space" if count > 0 => chosen = Some(keys.read(cx).highlight),
                "escape" => escaped = true,
                _ if digit.is_some() => chosen = digit.map(|n| n - 1),
                _ => return,
            }
            cx.stop_propagation();
            if let Some(ix) = chosen {
                choose(&keys, kind, ix, window, cx);
            } else if escaped {
                let input = keys.update(cx, |s, cx| {
                    s.close_menu();
                    cx.notify();
                    s.input.clone()
                });
                TextInput::focus(&input, window, cx);
            }
        })
        .on_mouse_down_out(move |_, _, cx| {
            out.update(cx, |s, cx| {
                if s.menu == Some(kind) {
                    s.close_menu();
                    cx.notify();
                }
            })
        })
}

/// A popover's section label (pink `render_permission_popover` header):
/// `px 10 pt 8 pb 6`, 11 SEMIBOLD `text_muted`, sentence case.
fn menu_label(label: &'static str, t: &Pink) -> Div {
    div()
        .px(px(10.))
        .pt(px(8.))
        .pb(px(6.))
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text_muted)
        .child(label)
}

fn set_highlight(state: &Entity<State>, ix: usize, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.highlight != ix {
            s.highlight = ix;
            cx.notify();
        }
    })
}

/// A model row (pink `Pickers::render_model_row`, two-line layout): `px 8
/// py 6`, radius 8; the current model wears the selected wash and its inset
/// ring, the cursor row `ink(0.05)`. A 12.5 MEDIUM name over the model's
/// 11 pt glyph and description, and its ⌘ digit.
fn model_row(
    ix: usize,
    model: &ComposerModel,
    selected: bool,
    active: bool,
    state: &Entity<State>,
    t: &Pink,
) -> Div {
    let hover = state.clone();
    let pick = state.clone();
    let row = div()
        .id(ElementId::NamedInteger("cc-model-row".into(), ix as u64))
        .role(Role::ListBoxOption)
        .aria_label(model.name.clone())
        .px(px(8.))
        .py(px(6.))
        .rounded(px(radius::MENU_ITEM))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .cursor_pointer()
        .map(|row| {
            if selected {
                row.bg(t.card_selected_bg())
                    .shadow(t.card_selected_shadows())
            } else if active {
                row.bg(t.ink(0.05))
            } else {
                row
            }
        })
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                set_highlight(&hover, ix, cx)
            }
        })
        .on_click(move |_, window, cx| choose(&pick, Menu::Model, ix, window, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(model.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            svg()
                                .path(model.icon.clone())
                                .size(px(11.))
                                .flex_none()
                                .text_color(t.text_muted),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(11.))
                                .text_color(t.text_muted)
                                .child(model.description.clone()),
                        ),
                ),
        )
        .when(ix < 9, |row| {
            row.child(pink::kbd_hint(format!("⌘{}", ix + 1), t))
        });
    div().pb(px(2.)).child(row)
}

/// The access choices (pink `PERMISSION_OPTIONS` row anatomy): label,
/// description and whether the choice wears the warning tone.
const ACCESS: [(&str, &str, bool); 2] = [
    (
        "Ask first",
        "The assistant asks before it runs tools or changes files.",
        false,
    ),
    (
        "Auto-run",
        "Runs tools and edits without stopping to ask; app and system controls still apply.",
        true,
    ),
];

/// An access row (pink `Pickers::render_permission_popover`): the shared
/// keyboard highlight, a check on the current choice, a label over an 11 pt
/// description in `text_faint`; the unguarded choice reads in the warning
/// tone.
fn access_row(
    ix: usize,
    selected: bool,
    active: bool,
    state: &Entity<State>,
    t: &Pink,
) -> Stateful<Div> {
    let (label, description, warning) = ACCESS[ix];
    let hover = state.clone();
    let pick = state.clone();
    pink::menu_row_nav(selected, active, t)
        .id(ElementId::NamedInteger("cc-access-row".into(), ix as u64))
        .on_mouse_move(move |_, _, cx| set_highlight(&hover, ix, cx))
        .when(warning, |row| {
            row.text_color(t.readable_tone(t.warning, 1.0))
        })
        .on_click(move |_, window, cx| choose(&pick, Menu::Access, ix, window, cx))
        .child(
            div()
                .w_full()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(label)
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(if warning {
                            t.warning.opacity(0.72)
                        } else {
                            t.text_faint
                        })
                        .child(description),
                ),
        )
}

fn choose(state: &Entity<State>, kind: Menu, ix: usize, window: &mut Window, cx: &mut App) {
    let input = state.update(cx, |s, cx| {
        match kind {
            Menu::Model => s.model = ix,
            Menu::Access => s.ask_first = ix == 0,
            Menu::Project => s.project = ix,
        }
        s.close_menu();
        cx.notify();
        s.input.clone()
    });
    TextInput::focus(&input, window, cx);
}

fn toggle_menu(state: &Entity<State>, kind: Menu, window: &mut Window, cx: &mut App) {
    let focus = state.update(cx, |s, cx| {
        let just_closed = s
            .closed
            .is_some_and(|(menu, at)| menu == kind && at.elapsed() < REOPEN_GUARD);
        if s.menu == Some(kind) {
            s.close_menu();
        } else if !just_closed {
            s.close_menu();
            s.menu = Some(kind);
            s.highlight = s.selected(kind);
        }
        cx.notify();
        s.menu.is_some().then(|| s.menu_focus.clone())
    });
    if let Some(focus) = focus {
        window.focus(&focus, cx);
    }
}

impl RenderOnce for ChatComposer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            State::new,
        );

        let listening = self.listening.is_some();
        let model_count = self.models.len().max(1);
        let project_count = self.projects.len();
        let colors = (t.placeholder, t.caret, t.selection);
        let attachments = self.attachments.clone();
        let models = self.models.clone();
        let projects = self.projects.clone();
        let on_send = self.on_send.clone();
        let placeholder = self.placeholder.clone();
        let running = self.running;
        state.update(cx, |s, cx| {
            s.model = s.model.min(model_count - 1);
            s.project = s.project.min(project_count.saturating_sub(1));
            s.snapshot = Some((
                attachments,
                models
                    .get(s.model)
                    .map(|m| m.name.clone())
                    .unwrap_or_default(),
                projects.get(s.project).cloned(),
            ));
            s.on_send = on_send;
            s.running = running;
            match (listening, s.listen_started) {
                (true, None) => s.listen_started = Some(Instant::now()),
                (false, Some(_)) => s.listen_started = None,
                _ => {}
            }
            let fresh = s.placeholder != placeholder;
            s.placeholder = placeholder.clone();
            s.input.update(cx, |i, cx| {
                i.set_colors(Some(colors.0), Some(colors.1), Some(colors.2));
                if fresh {
                    i.set_placeholder(placeholder, cx);
                }
            });
        });
        let s = state.read(cx);
        let input = s.input.clone();
        let (model_ix, project_ix, web, ask_first, menu, highlight) =
            (s.model, s.project, s.web, s.ask_first, s.menu, s.highlight);
        let mounted = |kind| s.mounted(kind);
        let (model_menu, access_menu, project_menu) = (
            mounted(Menu::Model),
            mounted(Menu::Access),
            mounted(Menu::Project),
        );
        let has_content = s.has_text || !self.attachments.is_empty();
        let listen_started = s.listen_started;
        let natural = s.natural.clone();
        let menu_focus = s.menu_focus.clone();
        let focused = input.read(cx).is_focused(window);
        if [model_menu, access_menu, project_menu]
            .iter()
            .any(|m| matches!(m, Some(Some(_))))
        {
            // Keep the exit fade moving to its end.
            window.request_animation_frame();
        }

        // ---- attachments ------------------------------------------------------
        let strip = attachment_strip(&key, &self.attachments, self.on_remove.clone(), &t);

        // ---- the field (pink: textarea `px-4 pt-4 pb-1`, 76/60 → 260) ----------
        let field: AnyElement = if let (true, Some(started)) = (listening, listen_started) {
            let secs = started.elapsed().as_secs() as i64;
            let words = self.listening.clone().unwrap_or_default();
            div()
                .flex_none()
                .min_h(px(TEXTAREA_MIN))
                .px(px(INPUT_SIDE_PAD))
                .pt(px(TEXTAREA_PAD_TOP))
                .pb(px(TEXTAREA_PAD_BOTTOM))
                .flex()
                .flex_row()
                .items_start()
                .gap(px(8.))
                .child(
                    div()
                        .flex_none()
                        .h(px(INPUT_LINE_HEIGHT))
                        .flex()
                        .items_center()
                        .child(loaders::breathing_orb(t.accent)),
                )
                .child(if words.is_empty() {
                    loaders::shimmer_text(
                        "Listening",
                        INPUT_TEXT_SIZE,
                        INPUT_LINE_HEIGHT,
                        t.placeholder,
                        t.text,
                    )
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .into_any_element()
                } else {
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(INPUT_TEXT_SIZE))
                        .line_height(px(INPUT_LINE_HEIGHT))
                        .text_color(t.text)
                        .child(words)
                        .into_any_element()
                })
                .child(
                    div()
                        .flex_none()
                        .pt(px((INPUT_LINE_HEIGHT
                            - loaders::WORKING_STATUS_LINE_HEIGHT)
                            / 2.))
                        .child(loaders::working_timer(secs, t.content_faint)),
                )
                .into_any_element()
        } else {
            div()
                .id(SharedString::from(format!("{key}-field")))
                .flex_none()
                .min_h(px(TEXTAREA_MIN))
                .max_h(px(TEXTAREA_MAX))
                .overflow_y_scroll()
                .px(px(INPUT_SIDE_PAD))
                .pt(px(TEXTAREA_PAD_TOP))
                .pb(px(TEXTAREA_PAD_BOTTOM))
                .text_size(px(INPUT_TEXT_SIZE))
                .line_height(px(INPUT_LINE_HEIGHT))
                .text_color(t.text)
                .child(input.clone())
                .into_any_element()
        };

        // ---- controls row (pink: actions row) ---------------------------------
        let on_attach = self.on_attach.clone();
        let attach = row_button(
            format!("{key}-attach"),
            ICON_PAPERCLIP,
            "Attach files",
            false,
            1.,
            &t,
        )
        .when_some(on_attach, |d, f| {
            d.on_click(move |_, window, cx| f(window, cx))
        });

        let model = self.models.get(model_ix).cloned();
        let model_chip =
            pink::trigger_chip(format!("{key}-model"), true, menu == Some(Menu::Model), &t)
                .relative()
                .on_click({
                    let state = state.clone();
                    move |_, window, cx| toggle_menu(&state, Menu::Model, window, cx)
                })
                .when_some(model, |d, m| {
                    d.when(menu != Some(Menu::Model), |d| {
                        d.tooltip(pink::text_tooltip(m.name.clone()))
                            .tooltip_show_delay(layout::TOOLTIP_DELAY)
                    })
                    .child(
                        svg()
                            .path(m.icon)
                            .size(px(16.))
                            .flex_none()
                            .text_color(t.text_muted),
                    )
                    .child(div().min_w_0().truncate().child(m.name))
                })
                .when_some(model_menu, |d, exit| {
                    let card = menu_card(
                        Menu::Model,
                        self.models.len(),
                        layout::MENU_W_L,
                        exit.is_none(),
                        &menu_focus,
                        &state,
                        &t,
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .p(px(layout::CARD_INSET))
                            .child(menu_label("Model", &t))
                            .children(self.models.iter().enumerate().map(|(ix, m)| {
                                model_row(ix, m, ix == model_ix, ix == highlight, &state, &t)
                            })),
                    );
                    d.child(menu_above("cc-model-menu", card.into_any_element(), exit))
                });

        let (access_title, access_icon, access_tint) = if ask_first {
            (ACCESS[0].0, ICON_LOCK, t.text_muted)
        } else {
            (ACCESS[1].0, ICON_UNLOCKED, t.warning)
        };
        let access_chip = pink::trigger_chip(
            format!("{key}-access"),
            true,
            menu == Some(Menu::Access),
            &t,
        )
        .relative()
        .flex_none()
        .w(px(ROW_BUTTON))
        .px(px(0.))
        .justify_center()
        .aria_label(access_title)
        .when(menu != Some(Menu::Access), |d| {
            d.tooltip(pink::text_tooltip(access_title))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
        })
        .on_click({
            let state = state.clone();
            move |_, window, cx| toggle_menu(&state, Menu::Access, window, cx)
        })
        .child(
            svg()
                .path(access_icon)
                .size(px(16.))
                .flex_none()
                .text_color(access_tint),
        )
        .when_some(access_menu, |d, exit| {
            let selected = usize::from(!ask_first);
            let card = menu_card(
                Menu::Access,
                ACCESS.len(),
                ACCESS_MENU_W,
                exit.is_none(),
                &menu_focus,
                &state,
                &t,
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(layout::CARD_INSET))
                    .child(menu_label("Permission mode", &t))
                    .children(
                        (0..ACCESS.len())
                            .map(|ix| access_row(ix, ix == selected, ix == highlight, &state, &t)),
                    ),
            );
            d.child(menu_above("cc-access-menu", card.into_any_element(), exit))
        });

        let search_chip = search_chip(format!("{key}-search"), web, &t).on_click({
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.web = !s.web;
                    cx.notify();
                })
            }
        });

        let on_listen = self.on_listen.clone();
        let transcript = self.listening.clone();
        let mic_input = input.clone();
        let mic = row_button(
            format!("{key}-mic"),
            ICON_MIC,
            if listening {
                "Stop dictation"
            } else {
                "Dictate"
            },
            listening,
            0.,
            &t,
        )
        .on_click(move |_, window, cx| {
            if listening && let Some(words) = &transcript {
                let words = words.clone();
                mic_input.update(cx, |i, cx| {
                    let existing = i.text();
                    let joined = if existing.trim().is_empty() {
                        words.to_string()
                    } else {
                        format!("{} {}", existing.trim_end(), words)
                    };
                    i.set_text(joined, cx);
                });
                TextInput::focus(&mic_input, window, cx);
            }
            if let Some(f) = &on_listen {
                f(!listening, window, cx);
            }
        });

        let mode = send_mode(running, has_content);
        let recessed = mode != SendMode::Stop && (!has_content || listening);
        let on_stop = self.on_stop.clone();
        let send =
            send_disc(&key, mode, recessed, focused && !has_content, &t).when(!recessed, |d| {
                let state = state.clone();
                d.on_click(move |_, window, cx| match mode {
                    SendMode::Stop => {
                        if let Some(f) = &on_stop {
                            f(window, cx)
                        }
                    }
                    _ => state.update(cx, |s, cx| s.send(window, cx)),
                })
            });

        let controls = div()
            .flex_none()
            .h(px(ACTIONS_ROW_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(ACTION_PRIMARY_GAP))
            .pl(px(ACTIONS_ROW_INSET))
            .pr(px(ACTIONS_ROW_INSET))
            .pt(px(2.))
            .pb(px(12.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(ACTION_UTILITY_GAP))
                    .child(attach)
                    .child(
                        div()
                            .min_w_0()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(CHIP_GAP))
                            .child(model_chip)
                            .child(access_chip)
                            .child(search_chip),
                    ),
            )
            .child(mic)
            .child(send);

        // ---- the pill (pink `composer_surface`, opaque treatment) -------------
        // The pill's height eases to its content over `COLLAPSE` (pink's
        // committed-height morph): the content hangs from the pill's
        // stationary bottom edge, so the controls stay put while the top
        // edge sweeps.
        let measured = natural.clone();
        let body = div()
            .flex()
            .flex_col()
            .children(strip)
            .child(field)
            .child(controls)
            .child(
                canvas(
                    move |bounds, window, _| {
                        let h = f32::from(bounds.size.height);
                        if measured.get().is_none_or(|old| (old - h).abs() > 0.5) {
                            measured.set(Some(h));
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        let height = natural
            .get()
            .map(|h| motion::follow(&format!("{key}-pill"), h, motion::COLLAPSE));
        let focus_input = input.clone();
        let focus_state = state.clone();
        let pill = div()
            .id(SharedString::from(format!("{key}-pill")))
            .relative()
            .w_full()
            .flex_none()
            .rounded(px(radius::COMPOSER))
            .border_1()
            .border_color(t.border)
            .bg(t.input_glass_bg())
            .shadow(pink::shadow_lg())
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                // Padding and controls belong to the text; an open menu keeps
                // its own focus.
                if focus_state.read(cx).menu.is_none() {
                    TextInput::focus(&focus_input, window, cx)
                }
            })
            .map(|pill| match height {
                Some(h) => pill
                    .h(px(h + 2.))
                    .overflow_hidden()
                    .child(body.absolute().left_0().right_0().bottom_0()),
                None => pill.child(body),
            });

        // ---- the checkout row above the pill ------------------------------------
        let context = (!self.projects.is_empty()).then(|| {
            let label = self.projects[project_ix].clone();
            let chip = target_chip(
                format!("{key}-project"),
                ICON_FOLDER,
                label,
                menu == Some(Menu::Project),
                &t,
            )
            .on_click({
                let state = state.clone();
                move |_, window, cx| toggle_menu(&state, Menu::Project, window, cx)
            })
            .when_some(project_menu, |d, exit| {
                let card = menu_card(
                    Menu::Project,
                    self.projects.len(),
                    layout::MENU_W_M,
                    exit.is_none(),
                    &menu_focus,
                    &state,
                    &t,
                )
                .child(
                    div()
                        .id("cc-project-list")
                        .flex()
                        .flex_col()
                        .gap(px(layout::MENU_GAP))
                        .max_h(px(224.))
                        .overflow_y_scroll()
                        .children(self.projects.iter().enumerate().map(|(ix, name)| {
                            let hover = state.clone();
                            let pick = state.clone();
                            pink::menu_row_nav(ix == project_ix, ix == highlight, &t)
                                .id(ElementId::NamedInteger("cc-project-row".into(), ix as u64))
                                .on_mouse_move(move |_, _, cx| set_highlight(&hover, ix, cx))
                                .on_click(move |_, window, cx| {
                                    choose(&pick, Menu::Project, ix, window, cx)
                                })
                                .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                        })),
                );
                d.child(menu_above("cc-project-menu", card.into_any_element(), exit))
            });
            div()
                .w_full()
                .h(px(CONTEXT_ROW_HEIGHT))
                // Close to the pill it describes: half the column gap.
                .mb(px(-COLUMN_GAP / 2.))
                .px(px(6.))
                .flex()
                .flex_row()
                .items_center()
                .child(chip)
        });

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .flex()
            .flex_col()
            .gap(px(COLUMN_GAP))
            .font_family(pink::SANS)
            .children(context)
            .child(pill)
    }
}

/// The web search switch. Off it is a ghost trigger chip (pink
/// `Pickers::trigger_chip`); on it wears the mode-on capsule of pink's Goal
/// chip (`goals.rs` `render_goal_mode_chip`): `accent@0.14` easing to 0.20 on
/// hover, label and 15 pt glyph in `readable_tone(accent)`.
fn search_chip(id: String, on: bool, t: &Pink) -> Stateful<Div> {
    if !on {
        return pink::trigger_chip(id, false, false, t)
            .flex_none()
            .role(Role::Switch)
            .aria_toggled(Toggled::False)
            .tooltip(pink::text_tooltip("Search the web"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .child(
                svg()
                    .path(ICON_GLOBE)
                    .size(px(16.))
                    .flex_none()
                    .text_color(t.text_muted),
            )
            .child("Search");
    }
    let tone = t.readable_tone(t.accent, 0.95);
    let ring = t.focus_ring();
    let key = format!("{id}-on");
    div()
        .id(SharedString::from(id))
        .role(Role::Switch)
        .aria_label("Search the web")
        .aria_toggled(Toggled::True)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .flex_none()
        .h(px(ROW_BUTTON))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(tone)
        .cursor_pointer()
        .hover_fill(key, t.accent.opacity(0.14), t.accent.opacity(0.2))
        .tooltip(pink::text_tooltip("Web search is on"))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            svg()
                .path(ICON_GLOBE)
                .size(px(MODE_CHIP_ICON))
                .flex_none()
                .text_color(tone),
        )
        .child("Search")
}

// ---------------------------------------------------------------------------
// Story: a product manager's chat, with each turn running before it replies.
// ---------------------------------------------------------------------------

const DICTATION: &str = "and pull out the three riskiest dependencies for the leadership sync";
/// How long a demo turn runs before it answers.
const TURN: Duration = Duration::from_millis(5200);

const REPLIES: [&str; 3] = [
    "The roadmap's riskiest items are the payments migration, the new onboarding flow and the EU data residency work. All three depend on the platform team, which is already at capacity through November.",
    "Draft is ready: three highlights for customers, two fixes worth calling out, and a short note on the pricing page changes. I kept it under 200 words.",
    "Stop the checkout copy test and the referral banner: neither moved conversion after three weeks. Keep annual billing running one more week; its sample is still small.",
];

struct Turn {
    message: Message,
    reply: Option<&'static str>,
}

struct Demo {
    attachments: Vec<Attachment>,
    next_file: usize,
    turns: Vec<Turn>,
    run_started: Option<Instant>,
    listening: Option<Instant>,
    _ticker: Task<()>,
}

fn demo_files() -> Vec<Attachment> {
    vec![
        Attachment::image("launch-hero@2x.png", "PNG · 1.1 MB").preview(DEMO_HERO),
        Attachment::file("Q4-roadmap.pdf", "PDF · 2.4 MB"),
        Attachment::file("pricing-test.csv", "CSV · 88 KB"),
    ]
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let alive = this.update(cx, |demo, cx| {
                    if demo.run_started.is_some_and(|at| at.elapsed() >= TURN) {
                        demo.finish(true);
                        cx.notify();
                    } else if demo.listening.is_some() || demo.run_started.is_some() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        let files = demo_files();
        Demo {
            attachments: vec![files[0].clone(), files[1].clone()],
            next_file: 2,
            turns: vec![Turn {
                message: Message {
                    text: "Where did the annual billing test land last week?".into(),
                    attachments: Vec::new(),
                    model: "Balanced".into(),
                    project: Some("Storefront".into()),
                    web_search: false,
                    ask_first: true,
                    queued: false,
                },
                reply: Some(
                    "Annual billing at $96 converted 4.1% better than monthly, with no change in refunds. The cohort is still small (1,240 checkouts), so I'd let it run one more week before calling it.",
                ),
            }],
            run_started: None,
            listening: None,
            _ticker: ticker,
        }
    }

    /// Ends the running turn, answering it unless it was stopped.
    fn finish(&mut self, answer: bool) {
        self.run_started = None;
        let ix = self.turns.len();
        if let Some(turn) = self.turns.last_mut()
            && turn.reply.is_none()
        {
            turn.reply = Some(if answer {
                REPLIES[ix % REPLIES.len()]
            } else {
                "Stopped."
            });
        }
    }

    fn transcript(&self) -> Option<SharedString> {
        self.listening.map(|at| {
            let n = (at.elapsed().as_secs_f32() / 0.26) as usize;
            DICTATION
                .split(' ')
                .take(n)
                .collect::<Vec<_>>()
                .join(" ")
                .into()
        })
    }
}

/// A sent message (pink `transcript.rs` user bubble): right-aligned, at most
/// 80% wide, the bubble wash, radius 16, `px 16 py 10`, 14/22; its files ride
/// above it as message badges.
fn user_bubble(ix: usize, message: &Message, t: &Pink) -> impl IntoElement {
    let edge = t.user_bubble_edge();
    let badge_tone = t.readable_muted(0.7);
    motion::rise_in(
        ElementId::NamedInteger("cc-sent".into(), ix as u64),
        motion::TOOL_ROW_REVEAL,
        4.,
        div()
            .w_full()
            .flex()
            .flex_col()
            .items_end()
            .when(!message.attachments.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap(px(6.))
                        .pb(px(6.))
                        .children(message.attachments.iter().map(|a| {
                            pink::message_badge(t)
                                .child(
                                    svg()
                                        .path(if a.image { ICON_FILE_IMAGE } else { ICON_FILE })
                                        .size(px(12.))
                                        .text_color(badge_tone),
                                )
                                .child(a.name.clone())
                        })),
                )
            })
            .when(!message.text.is_empty(), |d| {
                d.child(
                    div()
                        .max_w(relative(0.8))
                        .px(px(16.))
                        .py(px(10.))
                        .rounded(px(radius::BUBBLE))
                        .bg(t.user_bubble_bg())
                        .when_some(edge, |d, edge| d.border_1().border_color(edge))
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .text_color(t.text)
                        .child(message.text.clone()),
                )
            }),
    )
}

/// The assistant's answer: bare prose on the canvas.
fn reply(ix: usize, text: &'static str, t: &Pink) -> impl IntoElement {
    motion::rise_in(
        ElementId::NamedInteger("cc-reply".into(), ix as u64),
        motion::TOOL_ROW_REVEAL,
        4.,
        div()
            .text_size(px(14.))
            .line_height(px(22.))
            .text_color(t.text)
            .child(text),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("chat-composer-demo", cx, |_, cx| Demo::new(cx));
        let d = demo.read(cx);
        let first = d.turns.len().saturating_sub(2);
        let mut transcript: Vec<AnyElement> = Vec::new();
        for (ix, turn) in d.turns.iter().enumerate().skip(first) {
            transcript.push(user_bubble(ix, &turn.message, &t).into_any_element());
            if let Some(text) = turn.reply {
                transcript.push(reply(ix, text, &t).into_any_element());
            }
        }
        let running = d.run_started;
        let (on_send, on_stop, on_attach, on_remove, on_listen) = (
            demo.clone(),
            demo.clone(),
            demo.clone(),
            demo.clone(),
            demo.clone(),
        );
        div()
            .w(px(DEFAULT_WIDTH))
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(transcript)
            .when_some(running, |col, at| {
                // pink's working trailer while the turn runs.
                col.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(8.))
                        .child(loaders::orb_slot(t.accent))
                        .child(loaders::working_status("Thinking", t.content_muted, t.text))
                        .child(loaders::working_timer(
                            at.elapsed().as_secs() as i64,
                            t.content_faint,
                        )),
                )
            })
            .child(
                div().pt(px(8.)).child(
                    ChatComposer::new("chat-composer")
                        .attachments(d.attachments.clone())
                        .projects(vec![
                            "Storefront".into(),
                            "Billing service".into(),
                            "Personal notes".into(),
                        ])
                        .listening(d.transcript())
                        .running(running.is_some())
                        .on_send(move |message, _, cx| {
                            let message = message.clone();
                            on_send.update(cx, |d, cx| {
                                d.finish(true);
                                d.turns.push(Turn {
                                    message,
                                    reply: None,
                                });
                                d.attachments.clear();
                                d.run_started = Some(Instant::now());
                                cx.notify();
                            })
                        })
                        .on_stop(move |_, cx| {
                            on_stop.update(cx, |d, cx| {
                                d.finish(false);
                                cx.notify();
                            })
                        })
                        .on_attach(move |_, cx| {
                            on_attach.update(cx, |d, cx| {
                                let files = demo_files();
                                let file = files[d.next_file % files.len()].clone();
                                d.next_file += 1;
                                if !d.attachments.iter().any(|a| a.name == file.name) {
                                    d.attachments.push(file);
                                }
                                cx.notify();
                            })
                        })
                        .on_remove_attachment(move |ix, _, cx| {
                            on_remove.update(cx, |d, cx| {
                                if ix < d.attachments.len() {
                                    d.attachments.remove(ix);
                                }
                                cx.notify();
                            })
                        })
                        .on_listen(move |on, _, cx| {
                            on_listen.update(cx, |d, cx| {
                                d.listening = on.then(Instant::now);
                                cx.notify();
                            })
                        }),
                ),
            )
    })
}
