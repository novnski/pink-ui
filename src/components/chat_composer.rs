//! Chat Composer – an original component in luna style.
//!
//! A prompt box for an assistant. Attachments sit above the text as small
//! file cards, the empty field cycles through example prompts, and the
//! toolbar holds web search and "Ask first" (approval before the assistant
//! acts) toggles, a model menu that opens upward and works from the keyboard,
//! voice input with a live level meter whose transcript lands in the field
//! for editing, and a send button that lights up once there is something to
//! send. A slim footer scopes the conversation to a project and shows the
//! keys. Enter sends, Shift+Enter adds a line.
//!
//! ```ignore
//! ChatComposer::new("composer")
//!     .attachments(files)
//!     .on_attach(|_, cx| { /* open a file picker */ })
//!     .on_send(|message, _, cx| println!("{} via {}", message.text, message.model))
//! ```

use std::sync::Arc;
use std::{
    f32::consts::TAU,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Anchor, Animation, AnyElement, AnyView, App, Bounds, Context, ElementId, Entity, FocusHandle,
    FontWeight, Hsla, IntoElement, KeyDownEvent, SharedString, Subscription, Task, Window,
    anchored, canvas, deferred, div, fill, point, prelude::*, px, size, svg,
};

use crate::{
    HoverFill,
    luna::{self, ControlSize, FillTier, Luna, radius, space, text},
    motion::{self, AnimationExt, GLIDE, MENU_IN, ROW_IN, VIEW_IN},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Chat Composer";
pub const DESCRIPTION: &str = "A prompt box with file cards, rotating example prompts, web search and ask-first toggles, a keyboard-driven model menu, voice input with a live level meter and a project-scoped footer";

const ICON_PLUS: &str = "icons/chat-composer/plus.svg";
const ICON_GLOBE: &str = "icons/chat-composer/globe.svg";
const ICON_SHIELD: &str = "icons/chat-composer/shield.svg";
const ICON_BOLT: &str = "icons/chat-composer/bolt.svg";
const ICON_CHEVRON: &str = "icons/chat-composer/chevron.svg";
const ICON_MIC: &str = "icons/chat-composer/mic.svg";
const ICON_STOP: &str = "icons/chat-composer/stop.svg";
const ICON_SEND: &str = "icons/chat-composer/send.svg";
const ICON_CLOSE: &str = "icons/chat-composer/close.svg";
const ICON_CHECK: &str = "icons/chat-composer/check.svg";
const ICON_FILE: &str = "icons/chat-composer/file.svg";
const ICON_IMAGE: &str = "icons/chat-composer/image.svg";
const ICON_FOLDER: &str = "icons/chat-composer/folder.svg";
pub const ICON_SPARK: &str = "icons/chat-composer/spark.svg";
pub const ICON_CLOCK: &str = "icons/chat-composer/clock.svg";
pub const ICON_DEEP: &str = "icons/chat-composer/deep.svg";

/// How long each example prompt stays in the empty field.
const PLACEHOLDER_EVERY: Duration = Duration::from_millis(3400);

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
        ComposerModel::new("Quick", "Fast replies for everyday questions", ICON_CLOCK),
        ComposerModel::new("Balanced", "Careful writing and analysis", ICON_SPARK),
        ComposerModel::new("Deep", "Thinks longer on hard problems", ICON_DEEP),
    ]
}

/// A file waiting to go with the message.
#[derive(Clone)]
pub struct Attachment {
    pub name: SharedString,
    /// Size or kind ("2.4 MB").
    pub detail: SharedString,
    pub image: bool,
}

impl Attachment {
    pub fn file(name: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            detail: detail.into(),
            image: false,
        }
    }

    pub fn image(name: impl Into<SharedString>, detail: impl Into<SharedString>) -> Self {
        Self {
            image: true,
            ..Self::file(name, detail)
        }
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Model,
    Project,
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
    highlight: usize,
    menu_closed_at: Option<Instant>,
    mounted: Instant,
    listen_started: Option<Instant>,
    /// What a send needs besides the text, refreshed every render.
    snapshot: Option<(Vec<Attachment>, SharedString, Option<SharedString>)>,
    on_send: Option<SendHandler>,
    _ticker: Task<()>,
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
        // Wake up when the example prompt changes.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PLACEHOLDER_EVERY).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        State {
            input,
            menu_focus: cx.focus_handle(),
            has_text: false,
            model: 1,
            project: 0,
            web: false,
            ask_first: true,
            menu: None,
            highlight: 0,
            menu_closed_at: None,
            mounted: Instant::now(),
            listen_started: None,
            snapshot: None,
            on_send: None,
            _ticker: ticker,
            _input_events: sub,
        }
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = self.input.read(cx).text().trim().to_string().into();
        let Some((attachments, model, project)) = self.snapshot.clone() else {
            return;
        };
        if text.is_empty() && attachments.is_empty() {
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
        };
        if let Some(f) = self.on_send.clone() {
            f(&message, window, cx);
        }
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct ChatComposer {
    id: ElementId,
    models: Vec<ComposerModel>,
    projects: Vec<SharedString>,
    attachments: Vec<Attachment>,
    placeholders: Vec<SharedString>,
    listening: Option<SharedString>,
    width: f32,
    on_send: Option<SendHandler>,
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
            placeholders: vec!["Ask anything".into()],
            listening: None,
            width: 640.,
            on_send: None,
            on_attach: None,
            on_remove: None,
            on_listen: None,
        }
    }

    pub fn models(mut self, models: Vec<ComposerModel>) -> Self {
        self.models = models;
        self
    }

    /// Projects for the footer scope; the footer hides without them.
    pub fn projects(mut self, projects: Vec<SharedString>) -> Self {
        self.projects = projects;
        self
    }

    pub fn attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// Example prompts the empty field cycles through.
    pub fn placeholders(mut self, placeholders: Vec<SharedString>) -> Self {
        if !placeholders.is_empty() {
            self.placeholders = placeholders;
        }
        self
    }

    /// Voice input in progress, with the words heard so far.
    pub fn listening(mut self, transcript: Option<SharedString>) -> Self {
        self.listening = transcript;
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

fn tool(id: impl Into<SharedString>, icon: &'static str, t: &Luna) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let key = format!("cc-tool-{id}");
    div()
        .id(id)
        .flex_none()
        .size(px(30.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .cursor_pointer()
        .hover_fill(
            key.clone(),
            t.fill(FillTier::Tertiary).opacity(0.),
            t.fill(FillTier::Tertiary),
        )
        .child(
            svg()
                .path(icon)
                .size(px(16.))
                .text_color(motion::hover_blend(&key, t.text_muted, t.text)),
        )
}

/// A toggle chip with a leading glyph.
fn toggle(
    id: impl Into<SharedString>,
    icon: &'static str,
    label: &'static str,
    on: bool,
    t: &Luna,
) -> gpui::Stateful<gpui::Div> {
    let color = if on {
        if t.dark { t.text } else { t.accent }
    } else {
        t.text_muted
    };
    luna::chip(id, on, ControlSize::Md, t)
        .px(px(space::S8))
        .child(svg().path(icon).size(px(14.)).text_color(color))
        .child(label)
}

fn level_meter(started: Instant, color: Hsla) -> impl IntoElement {
    canvas(
        move |_, _, _| started.elapsed().as_secs_f32(),
        move |bounds, time, window, _| {
            let bars = 18;
            let (bar_w, gap) = (2., 2.5);
            let height = f32::from(bounds.size.height);
            for i in 0..bars {
                let f = i as f32;
                let x = f32::from(bounds.origin.x) + f * (bar_w + gap);
                let level =
                    0.5 + 0.5 * (time * 6.3 + f * 1.1).sin() * (time * 1.9 + f * 0.43).cos();
                let envelope = 0.4 + 0.6 * (time * 1.3 + f * 0.27).sin().abs();
                let h = (3. + (height - 3.) * level * envelope).max(3.);
                let y = f32::from(bounds.origin.y) + (height - h) / 2.;
                window.paint_quad(
                    fill(
                        Bounds::new(point(px(x), px(y)), size(px(bar_w), px(h))),
                        color,
                    )
                    .corner_radii(px(1.)),
                );
            }
        },
    )
    .w(px(18. * 4.5 - 2.5))
    .h(px(18.))
    .flex_none()
}

/// The rows of an upward menu: icon, name, description, check.
struct MenuRow {
    icon: Option<SharedString>,
    name: SharedString,
    detail: Option<SharedString>,
}

#[allow(clippy::too_many_arguments)]
fn menu_popover(
    kind: Menu,
    rows: Vec<MenuRow>,
    selected: usize,
    highlight: usize,
    width: f32,
    corner: Anchor,
    focus: &FocusHandle,
    state: &Entity<State>,
    t: &Luna,
) -> impl IntoElement {
    let count = rows.len();
    let key_state = state.clone();
    let out_state = state.clone();
    let items =
        rows.into_iter().enumerate().map(|(i, row)| {
            let hovered = i == highlight;
            let pick = state.clone();
            div()
                .id(ElementId::NamedInteger("cc-menu-row".into(), i as u64))
                .px(px(space::S8))
                .py(px(space::S6))
                .flex()
                .items_center()
                .gap(px(space::S10))
                .rounded(px(radius::MD))
                .cursor_pointer()
                .bg(if hovered {
                    t.fill(FillTier::Tertiary)
                } else {
                    t.fill(FillTier::Tertiary).opacity(0.)
                })
                .on_hover({
                    let s = state.clone();
                    move |h, _, cx| {
                        if *h {
                            s.update(cx, |s, cx| {
                                s.highlight = i;
                                cx.notify();
                            })
                        }
                    }
                })
                .on_click(move |_, window, cx| choose(&pick, kind, i, window, cx))
                .when_some(row.icon, |d, icon| {
                    d.child(
                        div()
                            .flex_none()
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::SM))
                            .bg(t.fill(FillTier::Tertiary))
                            .child(svg().path(icon).size(px(15.)).text_color(if i == selected {
                                t.accent
                            } else {
                                t.text_muted
                            })),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(text::BODY))
                                .line_height(px(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(row.name),
                        )
                        .when_some(row.detail, |d, detail| {
                            d.child(
                                div()
                                    .truncate()
                                    .text_size(px(text::FOOTNOTE))
                                    .line_height(px(16.))
                                    .text_color(t.text_faint)
                                    .child(detail),
                            )
                        }),
                )
                .child(svg().path(ICON_CHECK).size(px(14.)).flex_none().text_color(
                    if i == selected {
                        t.accent
                    } else {
                        t.accent.opacity(0.)
                    },
                ))
        });
    let card = luna::overlay(t)
        .id(ElementId::NamedInteger("cc-menu".into(), kind as u64))
        .track_focus(focus)
        .w(px(width))
        .p(px(space::S4))
        .flex()
        .flex_col()
        .gap(px(2.))
        .font_family(luna::SANS)
        .on_key_down(move |ev: &KeyDownEvent, window, cx| {
            let key = ev.keystroke.key.as_str();
            let mut chosen = None;
            key_state.update(cx, |s, cx| {
                match key {
                    "down" => s.highlight = (s.highlight + 1) % count,
                    "up" => s.highlight = (s.highlight + count - 1) % count,
                    "home" => s.highlight = 0,
                    "end" => s.highlight = count - 1,
                    "enter" | "space" => chosen = Some(s.highlight),
                    "escape" => {
                        s.menu = None;
                        s.menu_closed_at = Some(Instant::now());
                    }
                    _ => return,
                }
                cx.notify();
            });
            if let Some(i) = chosen {
                choose(&key_state, kind, i, window, cx);
            } else if key == "escape" {
                let input = key_state.read(cx).input.clone();
                TextInput::focus(&input, window, cx);
            }
            cx.stop_propagation();
        })
        .on_mouse_down_out(move |_, _, cx| {
            out_state.update(cx, |s, cx| {
                if s.menu == Some(kind) {
                    s.menu = None;
                    s.menu_closed_at = Some(Instant::now());
                    cx.notify();
                }
            })
        })
        .children(items);
    let (top, bottom) = match corner {
        Anchor::BottomLeft | Anchor::BottomRight => (None, Some(())),
        _ => (Some(()), None),
    };
    div()
        .absolute()
        .when(top.is_some(), |d| d.bottom_0())
        .when(bottom.is_some(), |d| d.top_0())
        .when(
            matches!(corner, Anchor::BottomRight | Anchor::TopRight),
            |d| d.right_0(),
        )
        .when(
            matches!(corner, Anchor::BottomLeft | Anchor::TopLeft),
            |d| d.left_0(),
        )
        .child(deferred(
            anchored()
                .anchor(corner)
                .offset(point(px(0.), px(-6.)))
                .snap_to_window_with_margin(px(8.))
                .child(motion::rise_in(
                    ElementId::NamedInteger("cc-menu-in".into(), kind as u64),
                    MENU_IN,
                    4.,
                    card,
                )),
        ))
}

fn choose(state: &Entity<State>, kind: Menu, i: usize, window: &mut Window, cx: &mut App) {
    let input = state.update(cx, |s, cx| {
        match kind {
            Menu::Model => s.model = i,
            Menu::Project => s.project = i,
        }
        s.menu = None;
        s.menu_closed_at = Some(Instant::now());
        cx.notify();
        s.input.clone()
    });
    TextInput::focus(&input, window, cx);
}

fn open_menu(state: &Entity<State>, kind: Menu, window: &mut Window, cx: &mut App) {
    let focus = state.update(cx, |s, cx| {
        let just_closed = s
            .menu_closed_at
            .is_some_and(|at| at.elapsed() < Duration::from_millis(250));
        if s.menu == Some(kind) || just_closed {
            s.menu = None;
        } else {
            s.menu = Some(kind);
            s.highlight = match kind {
                Menu::Model => s.model,
                Menu::Project => s.project,
            };
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
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            State::new,
        );

        let listening = self.listening.is_some();
        let model_count = self.models.len().max(1);
        let project_count = self.projects.len();
        let colors = (t.text_faint, t.accent, t.selection);
        let attachments = self.attachments.clone();
        let models = self.models.clone();
        let projects = self.projects.clone();
        let on_send = self.on_send.clone();
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
            match (listening, s.listen_started) {
                (true, None) => s.listen_started = Some(Instant::now()),
                (false, Some(_)) => s.listen_started = None,
                _ => {}
            }
            s.input.update(cx, |i, _| {
                i.set_colors(Some(colors.0), Some(colors.1), Some(colors.2))
            });
        });
        let s = state.read(cx);
        let input = s.input.clone();
        let (model_ix, project_ix, web, ask_first, menu, highlight) =
            (s.model, s.project, s.web, s.ask_first, s.menu, s.highlight);
        let has_text = s.has_text;
        let mounted = s.mounted;
        let listen_started = s.listen_started;
        let menu_focus = s.menu_focus.clone();
        let input_empty = input.read(cx).is_empty();

        // ---- attachments -------------------------------------------------------
        let on_remove = self.on_remove.clone();
        let files = (!self.attachments.is_empty()).then(|| {
            div()
                .flex()
                .flex_wrap()
                .gap(px(space::S8))
                .px(px(space::S12))
                .pt(px(space::S12))
                .children(self.attachments.iter().enumerate().map(|(ix, file)| {
                    let card_key = format!("{key}-file-{ix}");
                    let hover = motion::hover_t(&card_key);
                    let on_remove = on_remove.clone();
                    motion::rise_in(
                        ElementId::Name(format!("cc-file-{}", file.name).into()),
                        ROW_IN,
                        6.,
                        div()
                            .id(ElementId::NamedInteger("cc-file".into(), ix as u64))
                            .relative()
                            .h(px(44.))
                            .pl(px(6.))
                            .pr(px(space::S12))
                            .flex()
                            .items_center()
                            .gap(px(space::S8))
                            .rounded(px(radius::MD))
                            .border_1()
                            .hover_border(card_key.clone(), t.border, t.border_strong)
                            .bg(t.fill(FillTier::Quaternary))
                            .child(
                                div()
                                    .flex_none()
                                    .size(px(32.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(radius::SM))
                                    .bg(if file.image {
                                        t.success.opacity(if t.dark { 0.18 } else { 0.12 })
                                    } else {
                                        t.accent.opacity(if t.dark { 0.2 } else { 0.1 })
                                    })
                                    .child(
                                        svg()
                                            .path(if file.image { ICON_IMAGE } else { ICON_FILE })
                                            .size(px(16.))
                                            .text_color(if file.image {
                                                t.success
                                            } else {
                                                t.accent
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(text::FOOTNOTE))
                                            .line_height(px(16.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(t.text)
                                            .child(file.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(text::CAPTION))
                                            .line_height(px(14.))
                                            .text_color(t.text_faint)
                                            .child(file.detail.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .id(ElementId::NamedInteger("cc-file-x".into(), ix as u64))
                                    .absolute()
                                    .top(px(-7.))
                                    .right(px(-7.))
                                    .size(px(18.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .bg(t.solid)
                                    .border_2()
                                    .border_color(t.surface_card)
                                    .opacity(hover)
                                    .cursor_pointer()
                                    .when_some(on_remove, |d, f| {
                                        d.on_click(move |_, window, cx| f(ix, window, cx))
                                    })
                                    .child(
                                        svg().path(ICON_CLOSE).size(px(10.)).text_color(t.on_solid),
                                    ),
                            ),
                    )
                }))
        });

        // ---- the field -----------------------------------------------------------
        let field: AnyElement = if let (true, Some(started)) = (listening, listen_started) {
            window.request_animation_frame();
            let secs = started.elapsed().as_secs();
            div()
                .h(px(44.))
                .px(px(space::S16))
                .flex()
                .items_center()
                .gap(px(space::S12))
                .child(
                    div()
                        .flex_none()
                        .size(px(8.))
                        .rounded_full()
                        .bg(t.danger)
                        .with_animation(
                            "cc-rec",
                            Animation::new(Duration::from_millis(1200)).repeat(),
                            |el, d| el.opacity(0.55 + 0.45 * (d * TAU).cos()),
                        ),
                )
                .child(level_meter(started, t.danger.opacity(0.8)))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(text::CALLOUT))
                        .text_color(t.text_muted)
                        .child(match &self.listening {
                            Some(w) if !w.is_empty() => w.clone(),
                            _ => "Listening…".into(),
                        }),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .font_family(luna::MONO)
                        .text_color(t.text_faint)
                        .child(format!("{}:{:02}", secs / 60, secs % 60)),
                )
                .into_any_element()
        } else {
            let n = self.placeholders.len().max(1);
            let tick = (mounted.elapsed().as_millis() / PLACEHOLDER_EVERY.as_millis()) as usize;
            let hint = self.placeholders[tick % n].clone();
            div()
                .relative()
                .px(px(space::S16))
                .pt(px(space::S12 + 2.))
                .pb(px(space::S6))
                .min_h(px(48.))
                .text_size(px(text::CALLOUT))
                .line_height(px(22.))
                .text_color(t.text)
                .child(input.clone())
                .when(input_empty, |d| {
                    d.child(
                        div()
                            .absolute()
                            .top(px(space::S12 + 2.))
                            .left(px(space::S16))
                            .text_color(t.text_faint)
                            .child(motion::rise_in(
                                ElementId::NamedInteger("cc-hint".into(), tick as u64),
                                VIEW_IN,
                                6.,
                                div().child(hint),
                            )),
                    )
                })
                .into_any_element()
        };

        // ---- toolbar -----------------------------------------------------------------
        let on_attach = self.on_attach.clone();
        let plus = tool(format!("{key}-attach"), ICON_PLUS, &t).when_some(on_attach, |d, f| {
            d.on_click(move |_, window, cx| f(window, cx))
        });
        let web_toggle = toggle(format!("{key}-web"), ICON_GLOBE, "Search", web, &t).on_click({
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.web = !s.web;
                    cx.notify();
                })
            }
        });
        let approval = toggle(
            format!("{key}-ask"),
            if ask_first { ICON_SHIELD } else { ICON_BOLT },
            if ask_first { "Ask first" } else { "Auto-run" },
            ask_first,
            &t,
        )
        .on_click({
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.ask_first = !s.ask_first;
                    cx.notify();
                })
            }
        });

        let model = self.models.get(model_ix).cloned();
        let model_key = format!("{key}-model");
        let model_button = div()
            .relative()
            .child(
                div()
                    .id("cc-model")
                    .h(px(30.))
                    .pl(px(space::S8))
                    .pr(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .rounded(px(radius::MD))
                    .cursor_pointer()
                    .hover_fill(
                        model_key.clone(),
                        if menu == Some(Menu::Model) {
                            t.fill(FillTier::Tertiary)
                        } else {
                            t.fill(FillTier::Tertiary).opacity(0.)
                        },
                        t.fill(FillTier::Tertiary),
                    )
                    .on_click({
                        let state = state.clone();
                        move |_, window, cx| open_menu(&state, Menu::Model, window, cx)
                    })
                    .when_some(model.clone(), |d, m| {
                        d.child(svg().path(m.icon).size(px(14.)).text_color(t.text_muted))
                            .child(motion::rise_in(
                                ElementId::NamedInteger("cc-model-name".into(), model_ix as u64),
                                MENU_IN,
                                -4.,
                                div()
                                    .text_size(px(text::FOOTNOTE))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(motion::hover_blend(
                                        &model_key,
                                        t.text_muted,
                                        t.text,
                                    ))
                                    .child(m.name),
                            ))
                    })
                    .child(
                        svg()
                            .path(ICON_CHEVRON)
                            .size(px(14.))
                            .text_color(t.text_faint),
                    ),
            )
            .when(menu == Some(Menu::Model), |d| {
                d.child(menu_popover(
                    Menu::Model,
                    self.models
                        .iter()
                        .map(|m| MenuRow {
                            icon: Some(m.icon.clone()),
                            name: m.name.clone(),
                            detail: Some(m.description.clone()),
                        })
                        .collect(),
                    model_ix,
                    highlight,
                    296.,
                    Anchor::BottomRight,
                    &menu_focus,
                    &state,
                    &t,
                ))
            });

        let on_listen = self.on_listen.clone();
        let transcript = self.listening.clone();
        let mic_input = input.clone();
        let mic = div()
            .id("cc-mic")
            .flex_none()
            .h(px(30.))
            .min_w(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(space::S6))
            .rounded(px(radius::MD))
            .cursor_pointer()
            .map(|d| {
                if listening {
                    d.px(px(space::S10))
                        .bg(t.danger.opacity(if t.dark { 0.2 } else { 0.1 }))
                        .text_size(px(text::FOOTNOTE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.danger)
                        .child(svg().path(ICON_STOP).size(px(12.)).text_color(t.danger))
                        .child("Done")
                } else {
                    let mk = format!("{key}-mic");
                    d.hover_fill(
                        mk.clone(),
                        t.fill(FillTier::Tertiary).opacity(0.),
                        t.fill(FillTier::Tertiary),
                    )
                    .child(
                        svg()
                            .path(ICON_MIC)
                            .size(px(16.))
                            .text_color(motion::hover_blend(&mk, t.text_muted, t.text)),
                    )
                }
            })
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

        let ready = (has_text || !self.attachments.is_empty()) && !listening;
        let lit = motion::follow(&format!("{key}-send"), if ready { 1. } else { 0. }, GLIDE);
        let send = div()
            .id("cc-send")
            .flex_none()
            .size(px(30.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(motion::mix(
                t.fill(FillTier::Secondary),
                t.accent_strong,
                lit,
            ))
            .when(ready, |d| d.cursor_pointer())
            .on_click({
                let state = state.clone();
                move |_, window, cx| state.update(cx, |s, cx| s.send(window, cx))
            })
            .child(svg().path(ICON_SEND).size(px(15.)).text_color(motion::mix(
                t.text_faint,
                t.on_accent,
                lit,
            )));

        let toolbar = div()
            .px(px(space::S8))
            .pb(px(space::S8))
            .flex()
            .items_center()
            .gap(px(space::S4))
            .child(plus)
            .child(web_toggle)
            .child(approval)
            .child(div().flex_1())
            .child(model_button)
            .child(mic)
            .child(div().w(px(2.)))
            .child(send);

        // ---- footer: project scope and keys ---------------------------------------
        let footer = (!self.projects.is_empty()).then(|| {
            let project = self.projects[project_ix].clone();
            let pkey = format!("{key}-project");
            div()
                .h(px(36.))
                .px(px(space::S8))
                .flex()
                .items_center()
                .gap(px(space::S6))
                .border_t_1()
                .border_color(t.border)
                .bg(t.fill(FillTier::Quaternary))
                .rounded_b(px(radius::LG - 1.))
                .text_size(px(text::FOOTNOTE))
                .child(
                    div()
                        .pl(px(space::S4))
                        .text_color(t.text_faint)
                        .child("Working in"),
                )
                .child(
                    div()
                        .relative()
                        .child(
                            div()
                                .id("cc-project")
                                .h(px(24.))
                                .px(px(6.))
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .rounded(px(radius::SM))
                                .cursor_pointer()
                                .hover_fill(
                                    pkey.clone(),
                                    t.fill(FillTier::Tertiary).opacity(0.),
                                    t.fill(FillTier::Tertiary),
                                )
                                .on_click({
                                    let state = state.clone();
                                    move |_, window, cx| {
                                        open_menu(&state, Menu::Project, window, cx)
                                    }
                                })
                                .child(
                                    svg()
                                        .path(ICON_FOLDER)
                                        .size(px(13.))
                                        .text_color(t.text_muted),
                                )
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(motion::hover_blend(
                                            &pkey,
                                            t.text_muted,
                                            t.text,
                                        ))
                                        .child(project),
                                )
                                .child(
                                    svg()
                                        .path(ICON_CHEVRON)
                                        .size(px(12.))
                                        .text_color(t.text_faint),
                                ),
                        )
                        .when(menu == Some(Menu::Project), |d| {
                            d.child(menu_popover(
                                Menu::Project,
                                self.projects
                                    .iter()
                                    .map(|p| MenuRow {
                                        icon: Some(ICON_FOLDER.into()),
                                        name: p.clone(),
                                        detail: None,
                                    })
                                    .collect(),
                                project_ix,
                                highlight,
                                240.,
                                Anchor::BottomLeft,
                                &menu_focus,
                                &state,
                                &t,
                            ))
                        }),
                )
                .child(div().flex_1())
                .child(luna::kbd("↵", &t))
                .child(div().text_color(t.text_faint).child("send"))
                .child(div().w(px(space::S4)))
                .child(luna::kbd("⇧↵", &t))
                .child(
                    div()
                        .pr(px(space::S4))
                        .text_color(t.text_faint)
                        .child("new line"),
                )
        });

        let focus_input = input.clone();
        let focus_state = state.clone();
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .font_family(luna::SANS)
            .child(
                luna::card(&t)
                    .id("cc-card")
                    .flex()
                    .flex_col()
                    .on_click(move |_, window, cx| {
                        // A click that just opened a menu keeps the focus there.
                        if focus_state.read(cx).menu.is_none() {
                            TextInput::focus(&focus_input, window, cx)
                        }
                    })
                    .children(files)
                    .child(field)
                    .child(toolbar)
                    .children(footer),
            )
    }
}

// ---------------------------------------------------------------------------
// Story: a product manager's workspace, with sent messages stacking above.
// ---------------------------------------------------------------------------

const DICTATION: &str = "and pull out the three riskiest dependencies for the leadership sync";

struct Demo {
    attachments: Vec<Attachment>,
    next_file: usize,
    sent: Vec<Message>,
    listening: Option<Instant>,
    _ticker: Task<()>,
}

fn demo_files() -> Vec<Attachment> {
    vec![
        Attachment::file("Q4-roadmap.pdf", "PDF · 2.4 MB"),
        Attachment::image("launch-hero@2x.png", "PNG · 1.1 MB"),
        Attachment::file("pricing-test.csv", "CSV · 88 KB"),
    ]
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(80))
                    .await;
                let alive = this.update(cx, |demo, cx| {
                    if demo.listening.is_some() {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Demo {
            attachments: vec![demo_files()[0].clone()],
            next_file: 1,
            sent: Vec::new(),
            listening: None,
            _ticker: ticker,
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

fn bubble(ix: usize, message: &Message, t: &Luna) -> impl IntoElement {
    let mut meta = vec![message.model.to_string()];
    if message.web_search {
        meta.push("web".into());
    }
    meta.push(
        if message.ask_first {
            "asks first"
        } else {
            "auto-run"
        }
        .into(),
    );
    motion::rise_in(
        ElementId::NamedInteger("cc-sent".into(), ix as u64),
        ROW_IN,
        10.,
        div()
            .flex()
            .flex_col()
            .items_end()
            .gap(px(space::S4))
            .child(
                div()
                    .max_w(px(460.))
                    .px(px(space::S12 + 2.))
                    .py(px(space::S8))
                    .rounded(px(radius::XL))
                    .rounded_br(px(radius::XS))
                    .bg(t.fill(FillTier::Secondary))
                    .text_size(px(text::CALLOUT))
                    .line_height(px(21.))
                    .text_color(t.text)
                    .when(!message.attachments.is_empty(), |d| {
                        d.child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(space::S4))
                                .mb(px(space::S4))
                                .children(message.attachments.iter().map(|a| {
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(space::S4))
                                        .text_size(px(text::FOOTNOTE))
                                        .text_color(t.text_muted)
                                        .child(
                                            svg()
                                                .path(if a.image { ICON_IMAGE } else { ICON_FILE })
                                                .size(px(12.))
                                                .text_color(t.text_faint),
                                        )
                                        .child(a.name.clone())
                                })),
                        )
                    })
                    .when(!message.text.is_empty(), |d| d.child(message.text.clone())),
            )
            .child(
                div()
                    .text_size(px(text::CAPTION))
                    .text_color(t.text_faint)
                    .child(meta.join(" · ")),
            ),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Luna::get(cx);
        let demo = window.use_keyed_state("chat-composer-demo", cx, |_, cx| Demo::new(cx));
        let d = demo.read(cx);
        let sent: Vec<_> = d
            .sent
            .iter()
            .enumerate()
            .rev()
            .take(2)
            .rev()
            .map(|(ix, m)| bubble(ix, m, &t))
            .collect();
        let (on_send, on_attach, on_remove, on_listen) =
            (demo.clone(), demo.clone(), demo.clone(), demo.clone());
        div()
            .w(px(640.))
            .flex()
            .flex_col()
            .gap(px(space::S16))
            .children(sent)
            .child(
                ChatComposer::new("chat-composer")
                    .attachments(d.attachments.clone())
                    .projects(vec![
                        "Storefront".into(),
                        "Billing service".into(),
                        "Personal notes".into(),
                    ])
                    .placeholders(vec![
                        "Summarize the attached roadmap in five bullets".into(),
                        "Draft release notes for version 4.12".into(),
                        "Which experiments should we stop this quarter?".into(),
                    ])
                    .listening(d.transcript())
                    .on_send(move |message, _, cx| {
                        let message = message.clone();
                        on_send.update(cx, |d, cx| {
                            d.sent.push(message);
                            d.attachments.clear();
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
            )
    })
}
