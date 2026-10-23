//! Comments Panel – a luna component.
//!
//! The comment sidebar of a document. Each comment shows who wrote it, the
//! passage it is anchored to and the note itself. A comment that starts with
//! a slash command (`/rewrite`, `/translate`, …) carries a small edit card:
//! run it and a spinner gives way to a before/after preview with an undo.
//! Resolving folds a comment away (it glides shut) into the Resolved tab of
//! a segmented switch whose thumb slides between the two. The composer at
//! the bottom completes slash commands from a menu that follows the arrow
//! keys, and new comments rise in at the end of the list.
//!
//! ```ignore
//! CommentsPanel::new("comments", comments)
//!     .document("Pricing page · v3")
//!     .on_submit(|text, _, cx| { /* post it */ })
//!     .on_resolve(|id, _, cx| { /* mark resolved */ })
//!     .on_run(|id, _, cx| { /* apply the edit */ })
//! ```

use std::{cell::Cell, collections::HashMap, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, Hsla, IntoElement,
    ScrollHandle, SharedString, Subscription, Task, Transformation, Window, canvas, div,
    percentage, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, AnimationExt, EASE_IN_OUT, GLIDE, MENU_IN, MotionSpec, ROW_IN},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Comments Panel";
pub const DESCRIPTION: &str = "A document's comment sidebar: anchored notes, slash-command comments that run as edits with a before/after preview and undo, resolving that folds comments away, and a composer that completes commands";

const ICON_RESOLVE: &str = "icons/comments-panel/resolve.svg";
const ICON_SEND: &str = "icons/comments-panel/send.svg";
const ICON_RUN: &str = "icons/comments-panel/run.svg";
const ICON_UNDO: &str = "icons/comments-panel/undo.svg";
const ICON_SPARK: &str = "icons/comments-panel/spark.svg";
const ICON_CHECK: &str = "icons/comments-panel/check.svg";
const ICON_BUBBLE: &str = "icons/comments-panel/bubble.svg";
const ICON_RING: &str = "icons/comments-panel/ring.svg";
const ICON_ARC: &str = "icons/comments-panel/arc.svg";

/// A resolved comment takes this long to fold away.
const FOLD: MotionSpec = MotionSpec::new(380, EASE_IN_OUT);
const BODY_SIZE: f32 = text::BODY;
const WORD_GAP: f32 = BODY_SIZE * 0.25;

type IdHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// Where an edit command is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditStatus {
    Ready,
    Running,
    Applied,
}

/// The edit a slash-command comment proposes.
#[derive(Clone)]
pub struct EditCommand {
    pub before: SharedString,
    pub after: SharedString,
    pub status: EditStatus,
}

impl EditCommand {
    pub fn new(before: impl Into<SharedString>, after: impl Into<SharedString>) -> Self {
        Self {
            before: before.into(),
            after: after.into(),
            status: EditStatus::Ready,
        }
    }

    pub fn status(mut self, status: EditStatus) -> Self {
        self.status = status;
        self
    }
}

#[derive(Clone)]
pub struct Comment {
    pub id: SharedString,
    pub author: SharedString,
    pub time: SharedString,
    /// The passage the comment is pinned to.
    pub anchor: Option<SharedString>,
    /// The note; a leading `/command` is drawn as a command.
    pub body: SharedString,
    pub edit: Option<EditCommand>,
    pub resolved: bool,
    pub tone: Tone,
}

impl Comment {
    pub fn new(
        id: impl Into<SharedString>,
        author: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            author: author.into(),
            time: SharedString::default(),
            anchor: None,
            body: body.into(),
            edit: None,
            resolved: false,
            tone: Tone::Accent,
        }
    }

    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = time.into();
        self
    }

    pub fn anchor(mut self, passage: impl Into<SharedString>) -> Self {
        self.anchor = Some(passage.into());
        self
    }

    pub fn edit(mut self, edit: EditCommand) -> Self {
        self.edit = Some(edit);
        self
    }

    pub fn resolved(mut self, resolved: bool) -> Self {
        self.resolved = resolved;
        self
    }

    /// Avatar color.
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

/// A command the composer can complete.
#[derive(Clone)]
pub struct SlashCommand {
    pub name: SharedString,
    pub hint: SharedString,
}

impl SlashCommand {
    pub fn new(name: impl Into<SharedString>, hint: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            hint: hint.into(),
        }
    }
}

pub fn default_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand::new("rewrite", "Rewrite the anchored text"),
        SlashCommand::new("shorten", "Make it shorter"),
        SlashCommand::new("translate", "Translate into another language"),
        SlashCommand::new("assign", "Hand the comment to someone"),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Open,
    Resolved,
}

struct State {
    input: Entity<TextInput>,
    text: SharedString,
    tab: Tab,
    highlight: usize,
    scroll: ScrollHandle,
    seen: usize,
    /// Comments folding away after being resolved.
    leaving: HashMap<SharedString, Instant>,
    heights: HashMap<SharedString, Rc<Cell<f32>>>,
    commands: Vec<SlashCommand>,
    on_submit: Option<IdHandler>,
    _input_events: Subscription,
}

impl State {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input =
            cx.new(|cx| TextInput::new(window, cx).placeholder("Comment, or type / for a command"));
        let sub = cx.subscribe_in(&input, window, |this: &mut State, input, ev, window, cx| {
            let matches = this.matching().len();
            match ev {
                InputEvent::Change(text) => {
                    this.text = text.clone();
                    this.highlight = 0;
                    cx.notify();
                }
                InputEvent::MoveDown if matches > 0 => {
                    this.highlight = (this.highlight + 1) % matches;
                    cx.notify();
                }
                InputEvent::MoveUp if matches > 0 => {
                    this.highlight = (this.highlight + matches - 1) % matches;
                    cx.notify();
                }
                InputEvent::Tab | InputEvent::Submit(_) if matches > 0 => {
                    let name = this.matching()[this.highlight.min(matches - 1)]
                        .name
                        .clone();
                    let text: SharedString = format!("/{name} ").into();
                    input.update(cx, |i, cx| i.set_text(text.clone(), cx));
                    this.text = text;
                    cx.notify();
                }
                InputEvent::Submit(text) => {
                    let text = text.trim().to_string();
                    if text.is_empty() {
                        return;
                    }
                    input.update(cx, |i, cx| i.clear(cx));
                    this.text = SharedString::default();
                    this.tab = Tab::Open;
                    if let Some(f) = this.on_submit.clone() {
                        f(text.into(), window, cx);
                    }
                    cx.notify();
                }
                InputEvent::Escape if !this.text.is_empty() => {
                    input.update(cx, |i, cx| i.clear(cx));
                    this.text = SharedString::default();
                    cx.notify();
                }
                _ => {}
            }
        });
        State {
            input,
            text: SharedString::default(),
            tab: Tab::Open,
            highlight: 0,
            scroll: ScrollHandle::new(),
            seen: 0,
            leaving: HashMap::new(),
            heights: HashMap::new(),
            commands: Vec::new(),
            on_submit: None,
            _input_events: sub,
        }
    }

    /// Commands matching a `/prefix` still being typed.
    fn matching(&self) -> Vec<SlashCommand> {
        let Some(prefix) = self.text.strip_prefix('/') else {
            return Vec::new();
        };
        if prefix.contains(' ') {
            return Vec::new();
        }
        self.commands
            .iter()
            .filter(|c| c.name.starts_with(prefix))
            .cloned()
            .collect()
    }
}

#[derive(IntoElement)]
pub struct CommentsPanel {
    id: ElementId,
    comments: Vec<Comment>,
    document: Option<SharedString>,
    me: SharedString,
    commands: Vec<SlashCommand>,
    width: f32,
    height: f32,
    on_submit: Option<IdHandler>,
    on_resolve: Option<IdHandler>,
    on_run: Option<IdHandler>,
    on_undo: Option<IdHandler>,
}

impl CommentsPanel {
    pub fn new(id: impl Into<ElementId>, comments: Vec<Comment>) -> Self {
        Self {
            id: id.into(),
            comments,
            document: None,
            me: "You".into(),
            commands: default_commands(),
            width: 420.,
            height: 560.,
            on_submit: None,
            on_resolve: None,
            on_run: None,
            on_undo: None,
        }
    }

    pub fn document(mut self, name: impl Into<SharedString>) -> Self {
        self.document = Some(name.into());
        self
    }

    /// The signed-in author, for the composer avatar.
    pub fn me(mut self, name: impl Into<SharedString>) -> Self {
        self.me = name.into();
        self
    }

    pub fn commands(mut self, commands: Vec<SlashCommand>) -> Self {
        self.commands = commands;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn on_submit(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }

    /// Resolve (or reopen) the comment with this id.
    pub fn on_resolve(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resolve = Some(Rc::new(handler));
        self
    }

    /// Run the comment's edit command.
    pub fn on_run(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }

    pub fn on_undo(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }
}

fn measure(cell: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            if (cell.get() - h).abs() > 0.5 {
                cell.set(h);
                window.on_next_frame(|window, _| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn avatar(name: &str, tone: Tone, size: f32, t: &Luna) -> impl IntoElement {
    let color = tone.color(t);
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(color.opacity(if t.dark { 0.24 } else { 0.14 }))
        .text_size(px((size * 0.4).round()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(initials(name))
}

fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla) -> impl IntoElement {
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(
            svg()
                .path(ICON_RING)
                .absolute()
                .size(px(size))
                .text_color(color.opacity(0.22)),
        )
        .child(
            svg()
                .path(ICON_ARC)
                .size(px(size))
                .text_color(color)
                .with_animation(
                    id,
                    Animation::new(Duration::from_millis(800)).repeat(),
                    |s, d| s.with_transformation(Transformation::rotate(percentage(d))),
                ),
        )
}

/// Wrapping words, with a leading `/command` drawn as a pill.
fn body_flow(body: &str, t: &Luna) -> impl IntoElement {
    let mut words: Vec<AnyElement> = Vec::new();
    for (i, word) in body.split_whitespace().enumerate() {
        if i == 0 && word.starts_with('/') && word.len() > 1 {
            words.push(
                div()
                    .flex_none()
                    .mr(px(WORD_GAP + 1.))
                    .px(px(5.))
                    .h(px(19.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::XS))
                    .bg(t.accent_wash)
                    .font_family(luna::MONO)
                    .text_size(px(text::FOOTNOTE))
                    .text_color(if t.dark { t.text } else { t.accent })
                    .child(word.to_string())
                    .into_any_element(),
            );
        } else {
            words.push(
                div()
                    .flex_none()
                    .pr(px(WORD_GAP))
                    .child(word.to_string())
                    .into_any_element(),
            );
        }
    }
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .text_size(px(BODY_SIZE))
        .line_height(px(20.))
        .text_color(t.text)
        .children(words)
}

impl RenderOnce for CommentsPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            State::new,
        );

        let count = self.comments.len();
        let ids: Vec<SharedString> = self.comments.iter().map(|c| c.id.clone()).collect();
        let commands = self.commands.clone();
        let on_submit = self.on_submit.clone();
        let colors = (t.text_faint, t.accent, t.selection);
        let now = Instant::now();
        state.update(cx, |s, cx| {
            s.commands = commands;
            s.on_submit = on_submit;
            if count > s.seen {
                if s.seen > 0 {
                    s.scroll.scroll_to_bottom();
                }
                s.seen = count;
            }
            for id in &ids {
                s.heights
                    .entry(id.clone())
                    .or_insert_with(|| Rc::new(Cell::new(0.)));
            }
            s.leaving
                .retain(|_, at| now.saturating_duration_since(*at) < FOLD.total());
            s.input.update(cx, |i, _| {
                i.set_colors(Some(colors.0), Some(colors.1), Some(colors.2))
            });
        });
        let s = state.read(cx);
        let tab = s.tab;
        let scroll = s.scroll.clone();
        let leaving = s.leaving.clone();
        let heights = s.heights.clone();
        let input = s.input.clone();
        let matches = s.matching();
        let highlight = s.highlight.min(matches.len().saturating_sub(1));
        let has_text = !s.text.trim().is_empty();
        if !leaving.is_empty() {
            window.request_animation_frame();
        }

        let open_count = self.comments.iter().filter(|c| !c.resolved).count();
        let resolved_count = count - open_count;

        // ---- header with the Open / Resolved switch ---------------------------
        let seg_w = 92.;
        let thumb = motion::follow(
            &format!("{key}-thumb"),
            if tab == Tab::Open { 0. } else { seg_w },
            GLIDE,
        );
        let segment = |label: &'static str, n: usize, which: Tab| {
            let active = tab == which;
            let state = state.clone();
            let seg_key = format!("{key}-tab-{label}");
            div()
                .id(SharedString::from(seg_key.clone()))
                .relative()
                .w(px(seg_w))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(space::S6))
                .cursor_pointer()
                .text_size(px(text::FOOTNOTE))
                .font_weight(FontWeight::MEDIUM)
                .hover_fg(seg_key, if active { t.text } else { t.text_muted }, t.text)
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.tab = which;
                        cx.notify();
                    })
                })
                .child(label)
                .child(
                    div()
                        .min_w(px(16.))
                        .h(px(16.))
                        .px(px(4.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::XS))
                        .bg(if active {
                            t.accent_wash
                        } else {
                            t.fill(FillTier::Tertiary)
                        })
                        .text_size(px(10.5))
                        .text_color(if active {
                            if t.dark { t.text } else { t.accent }
                        } else {
                            t.text_faint
                        })
                        .child(format!("{n}")),
                )
        };
        let switch = div()
            .relative()
            .flex_none()
            .h(px(28.))
            .p(px(2.))
            .rounded(px(radius::MD))
            .bg(t.fill(FillTier::Tertiary))
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(2. + thumb))
                    .w(px(seg_w))
                    .h(px(24.))
                    .rounded(px(radius::SM))
                    .bg(if t.dark {
                        t.surface_raised
                    } else {
                        t.surface_card
                    })
                    .shadow(t.card_shadow()),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .h_full()
                    .child(segment("Open", open_count, Tab::Open))
                    .child(segment("Resolved", resolved_count, Tab::Resolved)),
            );
        let header = div()
            .flex_none()
            .h(px(56.))
            .px(px(space::S16))
            .flex()
            .items_center()
            .gap(px(space::S10))
            .border_b_1()
            .border_color(t.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(text::CALLOUT))
                            .line_height(px(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child("Comments"),
                    )
                    .when_some(self.document.clone(), |d, doc| {
                        d.child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(doc),
                        )
                    }),
            )
            .child(switch);

        // ---- the list ---------------------------------------------------------------
        let visible: Vec<&Comment> = self
            .comments
            .iter()
            .filter(|c| match tab {
                Tab::Open => !c.resolved || leaving.contains_key(&c.id),
                Tab::Resolved => c.resolved,
            })
            .collect();
        let empty = visible.is_empty();
        let items = visible.into_iter().enumerate().map(|(n, c)| {
            let item_key = format!("{key}-c-{}", c.id);
            let hover = motion::hover_t(&item_key);
            let fold = leaving
                .get(&c.id)
                .filter(|_| tab == Tab::Open)
                .map(|at| {
                    let raw = now.saturating_duration_since(*at).as_secs_f32()
                        / FOLD.total().as_secs_f32();
                    FOLD.curve.eval(raw.clamp(0., 1.))
                })
                .unwrap_or(0.);
            let cell = heights.get(&c.id).cloned().unwrap_or_default();
            let natural = cell.get();
            let on_resolve = self.on_resolve.clone();
            let resolve_state = state.clone();
            let cid = c.id.clone();
            let resolved = c.resolved;
            let resolve_button = div()
                .id(ElementId::Name(format!("cp-resolve-{}", c.id).into()))
                .flex_none()
                .size(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::SM))
                .cursor_pointer()
                .opacity(if resolved { 1. } else { hover })
                .hover_fill(
                    format!("{item_key}-resolve"),
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                )
                .on_click(move |_, window, cx| {
                    if !resolved {
                        resolve_state.update(cx, |s, cx| {
                            s.leaving.insert(cid.clone(), Instant::now());
                            cx.notify();
                        });
                    }
                    if let Some(f) = &on_resolve {
                        f(cid.clone(), window, cx);
                    }
                })
                .child(
                    svg()
                        .path(if resolved { ICON_UNDO } else { ICON_RESOLVE })
                        .size(px(15.))
                        .text_color(motion::hover_blend(
                            &format!("{item_key}-resolve"),
                            t.text_faint,
                            if resolved { t.text } else { t.success },
                        )),
                );

            let edit = c.edit.clone().map(|edit| {
                edit_card(
                    &t,
                    &c.id,
                    edit,
                    self.on_run.clone(),
                    self.on_undo.clone(),
                    resolved,
                )
            });

            let content = div()
                .relative()
                .flex_none()
                .px(px(space::S16))
                .py(px(space::S12))
                .flex()
                .gap(px(space::S10))
                .child(avatar(&c.author, c.tone, 26., &t))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(space::S6))
                        .child(
                            div()
                                .h(px(18.))
                                .flex()
                                .items_center()
                                .gap(px(space::S6))
                                .child(
                                    div()
                                        .text_size(px(text::BODY))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(t.text)
                                        .child(c.author.clone()),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_size(px(text::FOOTNOTE))
                                        .text_color(t.text_faint)
                                        .child(c.time.clone()),
                                )
                                .child(resolve_button),
                        )
                        .when_some(c.anchor.clone(), |d, anchor| {
                            d.child(
                                div()
                                    .flex()
                                    .gap(px(space::S8))
                                    .child(
                                        div()
                                            .flex_none()
                                            .w(px(2.))
                                            .rounded(px(1.))
                                            .bg(c.tone.color(&t).opacity(0.5)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .text_size(px(text::FOOTNOTE))
                                            .line_height(px(17.))
                                            .text_color(t.text_faint)
                                            .line_clamp(2)
                                            .child(format!("“{anchor}”")),
                                    ),
                            )
                        })
                        .child(body_flow(&c.body, &t))
                        .children(edit),
                )
                .child(measure(cell));

            let row = div()
                .id(ElementId::Name(format!("cp-item-{}", c.id).into()))
                .flex_none()
                .overflow_hidden()
                .when(fold > 0. && natural > 0., |d| {
                    d.h(px(natural * (1. - fold))).opacity(1. - fold)
                })
                .when(n > 0, |d| d.border_t_1().border_color(t.border))
                .hover_fill(
                    item_key.clone(),
                    t.fill(FillTier::Quaternary).opacity(0.),
                    t.fill(FillTier::Quaternary),
                )
                .child(content);
            motion::rise_in(
                ElementId::Name(format!("cp-in-{}-{}", c.id, tab == Tab::Open).into()),
                ROW_IN.with_delay(if n < 6 { 40 * n as u64 } else { 0 }),
                8.,
                row,
            )
        });
        let list = div()
            .id("cp-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .flex()
            .flex_col()
            .children(items)
            .when(empty, |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(space::S8))
                        .py(px(space::S32))
                        .child(
                            svg()
                                .path(ICON_BUBBLE)
                                .size(px(22.))
                                .text_color(t.text_faint),
                        )
                        .child(
                            div()
                                .text_size(px(text::BODY))
                                .text_color(t.text_muted)
                                .child(match tab {
                                    Tab::Open => "All caught up",
                                    Tab::Resolved => "Nothing resolved yet",
                                }),
                        ),
                )
            });

        // ---- composer with the command menu ---------------------------------------
        let menu = (!matches.is_empty()).then(|| {
            let rows = matches.iter().enumerate().map(|(i, c)| {
                let pick_input = input.clone();
                let pick_state = state.clone();
                let name = c.name.clone();
                div()
                    .id(ElementId::NamedInteger("cp-cmd".into(), i as u64))
                    .h(px(30.))
                    .px(px(space::S8))
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .rounded(px(radius::SM))
                    .cursor_pointer()
                    .bg(if i == highlight {
                        t.fill(FillTier::Tertiary)
                    } else {
                        t.fill(FillTier::Tertiary).opacity(0.)
                    })
                    .on_click(move |_, window, cx| {
                        let text: SharedString = format!("/{name} ").into();
                        pick_input.update(cx, |i, cx| i.set_text(text.clone(), cx));
                        TextInput::focus(&pick_input, window, cx);
                        pick_state.update(cx, |s, cx| {
                            s.text = text;
                            cx.notify();
                        });
                    })
                    .child(
                        div()
                            .w(px(84.))
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(if i == highlight {
                                if t.dark { t.text } else { t.accent }
                            } else {
                                t.text
                            })
                            .child(format!("/{}", c.name)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(c.hint.clone()),
                    )
            });
            div()
                .absolute()
                .bottom(px(58.))
                .left(px(space::S12))
                .right(px(space::S12))
                .child(motion::rise_in(
                    "cp-cmd-menu",
                    MENU_IN,
                    4.,
                    luna::overlay(&t)
                        .p(px(space::S4))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .px(px(space::S8))
                                .pt(px(space::S4))
                                .pb(px(2.))
                                .flex()
                                .items_center()
                                .gap(px(space::S6))
                                .text_size(px(text::CAPTION))
                                .text_color(t.text_faint)
                                .child(svg().path(ICON_SPARK).size(px(12.)).text_color(t.accent))
                                .child("Commands")
                                .child(div().flex_1())
                                .child(luna::kbd("↑↓", &t))
                                .child(luna::kbd("Tab", &t)),
                        )
                        .children(rows),
                ))
        });
        let send_input = input.clone();
        let send_state = state.clone();
        let lit = motion::follow(
            &format!("{key}-send"),
            if has_text { 1. } else { 0. },
            GLIDE,
        );
        let composer = div()
            .flex_none()
            .h(px(56.))
            .px(px(space::S12))
            .flex()
            .items_center()
            .gap(px(space::S10))
            .border_t_1()
            .border_color(t.border)
            .child(avatar(&self.me, Tone::Accent, 24., &t))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(34.))
                    .px(px(space::S10))
                    .flex()
                    .items_center()
                    .rounded(px(radius::MD))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.input_bg)
                    .text_size(px(text::BODY))
                    .line_height(px(20.))
                    .text_color(t.text)
                    .child(input.clone()),
            )
            .child(
                div()
                    .id("cp-send")
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
                    .when(has_text, |d| d.cursor_pointer())
                    .on_click(move |_, window, cx| {
                        let text = send_input.read(cx).text();
                        let text = text.trim().to_string();
                        if text.is_empty() {
                            return;
                        }
                        send_input.update(cx, |i, cx| i.clear(cx));
                        let f = send_state.update(cx, |s, cx| {
                            s.text = SharedString::default();
                            s.tab = Tab::Open;
                            cx.notify();
                            s.on_submit.clone()
                        });
                        if let Some(f) = f {
                            f(text.into(), window, cx);
                        }
                    })
                    .child(svg().path(ICON_SEND).size(px(15.)).text_color(motion::mix(
                        t.text_faint,
                        t.on_accent,
                        lit,
                    ))),
            );

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .h(px(self.height))
            .max_w_full()
            .font_family(luna::SANS)
            .child(
                luna::card(&t)
                    .relative()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(header)
                    .child(list)
                    .child(composer)
                    .children(menu),
            )
    }
}

/// The before/after card under a slash-command comment.
fn edit_card(
    t: &Luna,
    id: &SharedString,
    edit: EditCommand,
    on_run: Option<IdHandler>,
    on_undo: Option<IdHandler>,
    resolved: bool,
) -> impl IntoElement {
    let id_run = id.clone();
    let id_undo = id.clone();
    let shell = div()
        .mt(px(2.))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .px(px(space::S10))
        .py(px(space::S8))
        .flex()
        .flex_col()
        .gap(px(space::S6))
        .text_size(px(text::FOOTNOTE))
        .line_height(px(17.));
    match edit.status {
        EditStatus::Ready => shell
            .py(px(6.))
            .pr(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .child(svg().path(ICON_SPARK).size(px(13.)).text_color(t.accent))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(t.text_muted)
                            .child("Edits the anchored text"),
                    )
                    .child(
                        luna::button(
                            format!("cp-run-{id}"),
                            ButtonKind::Secondary,
                            ControlSize::Sm,
                            t,
                        )
                        .when(resolved, |b| b.opacity(0.5))
                        .child(luna::glyph(ICON_RUN, ControlSize::Sm).text_color(t.accent))
                        .child("Run edit")
                        .when_some(on_run.filter(|_| !resolved), |b, f| {
                            b.on_click(move |_, window, cx| f(id_run.clone(), window, cx))
                        }),
                    ),
            )
            .into_any_element(),
        EditStatus::Running => shell
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .text_color(t.text_muted)
                    .child(spinner(
                        ElementId::Name(format!("cp-spin-{id}").into()),
                        12.,
                        t.accent,
                    ))
                    .child("Applying to the page…"),
            )
            .child(
                div()
                    .h(px(8.))
                    .w(gpui::relative(0.7))
                    .rounded(px(radius::XS))
                    .bg(t.fill(FillTier::Secondary))
                    .with_animation(
                        ElementId::Name(format!("cp-skel-{id}").into()),
                        Animation::new(Duration::from_millis(1200)).repeat(),
                        |el, d| el.opacity(0.5 + 0.5 * (d * std::f32::consts::TAU).cos()),
                    ),
            )
            .into_any_element(),
        EditStatus::Applied => {
            let removed = t.danger.opacity(if t.dark { 0.14 } else { 0.07 });
            let added = t.success.opacity(if t.dark { 0.16 } else { 0.10 });
            shell
                .child(motion::rise_in(
                    ElementId::Name(format!("cp-diff-{id}").into()),
                    ROW_IN,
                    4.,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(space::S4))
                        .child(
                            div()
                                .px(px(space::S6))
                                .py(px(2.))
                                .rounded(px(radius::XS))
                                .bg(removed)
                                .text_color(t.text_faint)
                                .line_through()
                                .child(edit.before.clone()),
                        )
                        .child(
                            div()
                                .px(px(space::S6))
                                .py(px(2.))
                                .rounded(px(radius::XS))
                                .bg(added)
                                .text_color(t.text)
                                .child(edit.after.clone()),
                        ),
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .child(svg().path(ICON_CHECK).size(px(13.)).text_color(t.success))
                        .child(
                            div()
                                .flex_1()
                                .text_color(t.text_muted)
                                .child("Applied to the page"),
                        )
                        .child(
                            luna::button(
                                format!("cp-undo-{id}"),
                                ButtonKind::Ghost,
                                ControlSize::Sm,
                                t,
                            )
                            .child(luna::glyph(ICON_UNDO, ControlSize::Sm).text_color(t.text_muted))
                            .child("Undo")
                            .when_some(on_undo, |b, f| {
                                b.on_click(move |_, window, cx| f(id_undo.clone(), window, cx))
                            }),
                        ),
                )
                .into_any_element()
        }
    }
}

// ---------------------------------------------------------------------------
// Story: review comments on a pricing page draft.
// ---------------------------------------------------------------------------

struct Demo {
    comments: Vec<Comment>,
    next_id: usize,
    tasks: Vec<Task<()>>,
}

impl Demo {
    fn new() -> Self {
        Demo {
            comments: vec![
                Comment::new(
                    "c1",
                    "Priya Natarajan",
                    "/rewrite make the headline shorter, under six words",
                )
                .time("12 min ago")
                .anchor("Simple pricing that scales with your whole team")
                .edit(EditCommand::new(
                    "Simple pricing that scales with your whole team",
                    "Pricing that grows with you",
                ))
                .tone(Tone::Accent),
                Comment::new(
                    "c2",
                    "Tomás Ortega",
                    "Do we still offer the nonprofit discount? It's missing from the FAQ.",
                )
                .time("25 min ago")
                .anchor("Frequently asked questions")
                .tone(Tone::Warning),
                Comment::new(
                    "c3",
                    "Ada Kim",
                    "/translate the plan names into German for the DE page",
                )
                .time("1 h ago")
                .anchor("Starter · Team · Enterprise")
                .edit(
                    EditCommand::new(
                        "Starter · Team · Enterprise",
                        "Starter · Team · Unternehmen",
                    )
                    .status(EditStatus::Applied),
                )
                .tone(Tone::Success),
                Comment::new(
                    "c4",
                    "Felix Braun",
                    "Swapped the screenshots for the new dashboard.",
                )
                .time("2 h ago")
                .resolved(true)
                .tone(Tone::Danger),
            ],
            next_id: 5,
            tasks: Vec::new(),
        }
    }

    fn set_status(&mut self, id: &str, status: EditStatus) {
        if let Some(edit) = self
            .comments
            .iter_mut()
            .find(|c| c.id.as_ref() == id)
            .and_then(|c| c.edit.as_mut())
        {
            edit.status = status;
        }
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("comments-panel-demo", cx, |_, _| Demo::new());
        let (on_submit, on_resolve, on_run, on_undo) =
            (demo.clone(), demo.clone(), demo.clone(), demo.clone());
        CommentsPanel::new("comments-panel", demo.read(cx).comments.clone())
            .document("Pricing page · v3 draft")
            .me("Jun Park")
            .on_submit(move |text, _, cx| {
                on_submit.update(cx, |d, cx| {
                    let id: SharedString = format!("c{}", d.next_id).into();
                    d.next_id += 1;
                    let mut comment = Comment::new(id, "Jun Park", text.clone())
                        .time("Just now")
                        .tone(Tone::Accent);
                    if text.starts_with("/rewrite") || text.starts_with("/shorten") {
                        comment = comment.anchor("Cancel anytime, no questions asked").edit(
                            EditCommand::new(
                                "Cancel anytime, no questions asked",
                                "Cancel whenever you like",
                            ),
                        );
                    }
                    d.comments.push(comment);
                    cx.notify();
                })
            })
            .on_resolve(move |id, _, cx| {
                on_resolve.update(cx, |d, cx| {
                    if let Some(c) = d.comments.iter_mut().find(|c| c.id == id) {
                        c.resolved = !c.resolved;
                    }
                    cx.notify();
                })
            })
            .on_run(move |id, _, cx| {
                let entity = on_run.clone();
                on_run.update(cx, |d, cx| {
                    d.set_status(&id, EditStatus::Running);
                    let task = cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(1300))
                            .await;
                        let _ = entity.update(cx, |d, cx| {
                            d.set_status(&id, EditStatus::Applied);
                            cx.notify();
                        });
                    });
                    d.tasks.push(task);
                    cx.notify();
                })
            })
            .on_undo(move |id, _, cx| {
                on_undo.update(cx, |d, cx| {
                    d.set_status(&id, EditStatus::Ready);
                    cx.notify();
                })
            })
    })
}
