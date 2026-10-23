//! Memory Panel – what the assistant remembers about you, and the controls
//! to change it.
//!
//! Memories are grouped into categories you can filter by. Hover a memory to
//! pin it, edit it in place (Enter saves, Esc cancels) or forget it; a
//! forgotten memory folds away and the tray at the bottom offers Undo while a
//! bar runs down. The switch in the header pauses memory as a whole. "Remember
//! something" adds a memory of your own.
//!
//! ```ignore
//! MemoryPanel::new("memory", vec![MemoryCategory::new("Work", ICON)])
//!     .memory(Memory::new("m1", "Leads the payments team", 0).source("From chat · Oct 9"))
//!     .on_change(|change, _, _| println!("{change:?}"))
//! ```

use std::sync::Arc;
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, IntoElement, SharedString,
    Subscription, Task, Window, div, linear_color_stop, linear_gradient, prelude::*, px, relative,
    svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, radius, space, text},
    motion::{self, GLIDE, HOVER_FADE, LINEAR, MENU_IN, MotionSpec, ROW_IN},
    primitives::{InputEvent, TextInput, color_mix},
    story,
};

pub const NAME: &str = "Memory Panel";
pub const DESCRIPTION: &str = "Everything the assistant remembers about you, by category – pin, edit in place or forget each memory with Undo in the tray, add your own, or pause memory with one switch";

const ICON_MEMORY: &str = "icons/memory-panel/memory.svg";
const ICON_PIN: &str = "icons/memory-panel/pin.svg";
const ICON_EDIT: &str = "icons/memory-panel/edit.svg";
const ICON_FORGET: &str = "icons/memory-panel/forget.svg";
const ICON_PLUS: &str = "icons/memory-panel/plus.svg";
const ICON_PAUSE: &str = "icons/memory-panel/pause.svg";
/// Category glyphs for the demo (and for your own categories).
pub const ICON_PERSON: &str = "icons/memory-panel/person.svg";
pub const ICON_WORK: &str = "icons/memory-panel/work.svg";
pub const ICON_STYLE: &str = "icons/memory-panel/style.svg";

const ROW_H: f32 = 52.0;
/// How long Undo stays on the tray after forgetting.
const UNDO_FOR: Duration = Duration::from_secs(6);

/// A group of memories with a glyph.
#[derive(Clone, Debug)]
pub struct MemoryCategory {
    pub name: SharedString,
    pub icon: SharedString,
}

impl MemoryCategory {
    pub fn new(name: impl Into<SharedString>, icon: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            icon: icon.into(),
        }
    }
}

/// One thing the assistant remembers.
#[derive(Clone, Debug)]
pub struct Memory {
    pub id: SharedString,
    pub text: SharedString,
    /// Index into the panel's categories.
    pub category: usize,
    /// Where it came from and when, e.g. "From chat · Oct 9".
    pub source: SharedString,
    pub pinned: bool,
}

impl Memory {
    pub fn new(
        id: impl Into<SharedString>,
        text: impl Into<SharedString>,
        category: usize,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            category,
            source: SharedString::default(),
            pinned: false,
        }
    }

    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = source.into();
        self
    }

    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }
}

/// What changed, reported to [`MemoryPanel::on_change`].
#[derive(Clone, Debug, PartialEq)]
pub enum MemoryChange {
    Added {
        id: SharedString,
        text: SharedString,
    },
    Edited {
        id: SharedString,
        text: SharedString,
    },
    Pinned {
        id: SharedString,
        pinned: bool,
    },
    /// Undo ran out: the memory is gone for good.
    Forgotten {
        id: SharedString,
    },
    /// Memory as a whole was switched on or off.
    Enabled(bool),
}

type ChangeHandler = Rc<dyn Fn(MemoryChange, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct MemoryPanel {
    id: ElementId,
    categories: Vec<MemoryCategory>,
    memories: Vec<Memory>,
    assistant: SharedString,
    width: f32,
    on_change: Option<ChangeHandler>,
}

impl MemoryPanel {
    pub fn new(id: impl Into<ElementId>, categories: Vec<MemoryCategory>) -> Self {
        Self {
            id: id.into(),
            categories,
            memories: Vec::new(),
            assistant: "the assistant".into(),
            width: 440.,
            on_change: None,
        }
    }

    /// The memories the panel starts with; after that it owns the list.
    pub fn memories(mut self, memories: impl IntoIterator<Item = Memory>) -> Self {
        self.memories.extend(memories);
        self
    }

    pub fn memory(mut self, memory: Memory) -> Self {
        self.memories.push(memory);
        self
    }

    /// Who remembers, for the subtitle ("Atlas", default "the assistant").
    pub fn assistant(mut self, name: impl Into<SharedString>) -> Self {
        self.assistant = name.into();
        self
    }

    /// Panel width in px (default 440).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(MemoryChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

#[derive(Clone)]
struct Item {
    memory: Memory,
    /// Forgotten but still undoable.
    forgotten: bool,
    /// Added in this session: it grows in instead of appearing at once.
    fresh: bool,
}

struct Editor {
    id: SharedString,
    input: Entity<TextInput>,
    _events: Subscription,
}

struct Pending {
    id: SharedString,
    text: SharedString,
    at: Instant,
}

struct MemoryState {
    items: Vec<Item>,
    filter: Option<usize>,
    enabled: bool,
    editing: Option<Editor>,
    undo: Option<Pending>,
    next_id: usize,
    on_change: Option<ChangeHandler>,
    _undo_timer: Option<Task<()>>,
}

impl MemoryState {
    fn report(&self, change: MemoryChange, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(f) = self.on_change.clone() {
            f(change, window, cx);
        }
    }

    fn index(&self, id: &SharedString) -> Option<usize> {
        self.items.iter().position(|i| &i.memory.id == id)
    }

    fn edit(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.index(&id) else { return };
        let t = Luna::get(cx);
        let text = self.items[ix].memory.text.clone();
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multiline()
                .submit_on_enter()
                .default_text(text)
                .placeholder("Something to remember…")
                .placeholder_color(t.text_faint)
                .caret_color(t.accent)
                .selection_color(t.selection)
        });
        let events = cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            InputEvent::Submit(_) => this.commit(window, cx),
            InputEvent::Escape => this.cancel(cx),
            _ => {}
        });
        TextInput::focus(&input, window, cx);
        self.editing = Some(Editor {
            id,
            input,
            _events: events,
        });
        cx.notify();
    }

    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = self.editing.take() else {
            return;
        };
        let text: SharedString = editor.input.read(cx).text().trim().to_string().into();
        if let Some(ix) = self.index(&editor.id) {
            let item = &mut self.items[ix];
            if text.is_empty() {
                // An empty new memory is dropped; an emptied old one stays.
                if item.fresh && item.memory.text.is_empty() {
                    self.items.remove(ix);
                }
            } else if text != item.memory.text {
                let added = item.memory.text.is_empty();
                item.memory.text = text.clone();
                let id = item.memory.id.clone();
                self.report(
                    if added {
                        MemoryChange::Added { id, text }
                    } else {
                        MemoryChange::Edited { id, text }
                    },
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = self.editing.take()
            && let Some(ix) = self.index(&editor.id)
            && self.items[ix].memory.text.is_empty()
        {
            self.items.remove(ix);
        }
        cx.notify();
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            self.cancel(cx);
        }
        self.next_id += 1;
        let id: SharedString = format!("new-{}", self.next_id).into();
        let category = self.filter.unwrap_or(0);
        self.items.insert(
            0,
            Item {
                memory: Memory::new(id.clone(), "", category).source("You added · just now"),
                forgotten: false,
                fresh: true,
            },
        );
        self.edit(id, window, cx);
    }

    fn forget(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_undo(window, cx);
        let Some(ix) = self.index(&id) else { return };
        if self.editing.as_ref().is_some_and(|e| e.id == id) {
            self.editing = None;
        }
        self.items[ix].forgotten = true;
        self.undo = Some(Pending {
            id,
            text: self.items[ix].memory.text.clone(),
            at: Instant::now(),
        });
        self._undo_timer = Some(cx.spawn_in(window, async move |this, cx| {
            // Tick the tray's countdown, then let the memory go.
            let start = Instant::now();
            while start.elapsed() < UNDO_FOR {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
            this.update_in(cx, |s, window, cx| s.finish_undo(window, cx))
                .ok();
        }));
        cx.notify();
    }

    /// Ends the undo window: the forgotten memory is removed for good.
    fn finish_undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(pending) = self.undo.take() {
            if let Some(ix) = self.index(&pending.id) {
                self.items.remove(ix);
            }
            self.report(MemoryChange::Forgotten { id: pending.id }, window, cx);
        }
        self._undo_timer = None;
        cx.notify();
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        if let Some(pending) = self.undo.take()
            && let Some(ix) = self.index(&pending.id)
        {
            self.items[ix].forgotten = false;
        }
        self._undo_timer = None;
        cx.notify();
    }

    fn toggle_pin(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.index(&id) {
            let m = &mut self.items[ix].memory;
            m.pinned = !m.pinned;
            let change = MemoryChange::Pinned {
                id,
                pinned: m.pinned,
            };
            self.report(change, window, cx);
        }
        cx.notify();
    }

    fn set_enabled(&mut self, enabled: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.enabled = enabled;
        self.report(MemoryChange::Enabled(enabled), window, cx);
        cx.notify();
    }
}

impl RenderOnce for MemoryPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let initial = self.memories.clone();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |_, _| MemoryState {
                items: initial
                    .into_iter()
                    .map(|memory| Item {
                        memory,
                        forgotten: false,
                        fresh: false,
                    })
                    .collect(),
                filter: None,
                enabled: true,
                editing: None,
                undo: None,
                next_id: 0,
                on_change: None,
                _undo_timer: None,
            },
        );
        // The latest handler, so changes made from async work can report too.
        let on_change = self.on_change.clone();
        state.update(cx, |s, _| s.on_change = on_change);

        let (items, filter, enabled, editing, undo) = {
            let s = state.read(cx);
            (
                s.items.clone(),
                s.filter,
                s.enabled,
                s.editing.as_ref().map(|e| (e.id.clone(), e.input.clone())),
                s.undo
                    .as_ref()
                    .map(|p| (p.text.clone(), p.at.elapsed().as_secs_f32())),
            )
        };
        let live: Vec<&Item> = items.iter().filter(|i| !i.forgotten).collect();
        let pinned = live.iter().filter(|i| i.memory.pinned).count();
        let on = motion::follow(
            &format!("{key}-enabled"),
            if enabled { 1. } else { 0. },
            GLIDE,
        );

        // ---- header -----------------------------------------------------------
        let switch = {
            let state = state.clone();
            let hover_key = format!("{key}-switch");
            let track = motion::mix(t.fill(FillTier::Primary), t.accent_strong, on);
            div()
                .id(SharedString::from(hover_key.clone()))
                .flex()
                .items_center()
                .gap(px(space::S8))
                .cursor_pointer()
                .text_size(px(text::FOOTNOTE))
                .hover_fg(hover_key, t.text_muted, t.text)
                .on_click(move |_, window, cx| {
                    state.update(cx, |s, cx| s.set_enabled(!s.enabled, window, cx))
                })
                .child(if enabled { "On" } else { "Paused" })
                .child(
                    div()
                        .relative()
                        .w(px(30.))
                        .h(px(18.))
                        .rounded_full()
                        .bg(track)
                        .child(
                            div()
                                .absolute()
                                .top(px(2.))
                                .left(px(2. + 12. * on))
                                .size(px(14.))
                                .rounded_full()
                                .bg(gpui::white())
                                .shadow(t.card_shadow()),
                        ),
                )
        };
        let count_of = |cat: Option<usize>| {
            live.iter()
                .filter(|i| cat.is_none_or(|c| i.memory.category == c))
                .count()
        };
        let chip = |ix: Option<usize>, label: SharedString| {
            let state = state.clone();
            let selected = filter == ix;
            let id = format!("{key}-filter-{}", ix.map_or(-1, |i| i as i64));
            luna::chip(id, selected, ControlSize::Sm, &t)
                .gap(px(space::S6))
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.filter = ix;
                        cx.notify();
                    })
                })
                .child(label)
                .child(
                    div()
                        .font_family(luna::MONO)
                        .text_size(px(10.))
                        .opacity(0.7)
                        .child(count_of(ix).to_string()),
                )
        };
        let header = div()
            .flex()
            .flex_col()
            .gap(px(space::S12))
            .px(px(space::S16))
            .pt(px(space::S16))
            .pb(px(space::S12))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(space::S10))
                    .child(
                        div()
                            .size(px(32.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::MD))
                            .bg(t.accent_wash)
                            .child(svg().path(ICON_MEMORY).size(px(16.)).text_color(if t.dark {
                                t.text
                            } else {
                                t.accent
                            })),
                    )
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
                                    .child("Memory"),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(text::FOOTNOTE))
                                    .line_height(px(16.))
                                    .text_color(t.text_faint)
                                    .child(format!(
                                        "What {} keeps in mind across chats",
                                        self.assistant
                                    )),
                            ),
                    )
                    .child(div().pt(px(6.)).child(switch)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(space::S6))
                    .child(chip(None, "All".into()))
                    .children(
                        self.categories
                            .iter()
                            .enumerate()
                            .map(|(ix, c)| chip(Some(ix), c.name.clone())),
                    ),
            );

        let paused = (1. - on > 0.01).then(|| {
            div()
                .mx(px(space::S16))
                .mb(px(space::S12))
                .h(px(40. * (1. - on)))
                .opacity(1. - on)
                .overflow_hidden()
                .child(
                    div()
                        .h(px(40.))
                        .flex()
                        .items_center()
                        .gap(px(space::S8))
                        .px(px(space::S10))
                        .rounded(px(radius::MD))
                        .bg(t.warning.opacity(if t.dark { 0.12 } else { 0.08 }))
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child(
                            svg()
                                .path(ICON_PAUSE)
                                .size(px(14.))
                                .flex_none()
                                .text_color(t.warning),
                        )
                        .child("Paused – nothing new is saved and nothing here is used."),
                )
        });

        // ---- list -------------------------------------------------------------
        let mut rows: Vec<AnyElement> = Vec::new();
        for item in &items {
            let m = &item.memory;
            let shown = !item.forgotten && filter.is_none_or(|c| m.category == c);
            let vis_key = format!("{key}-row-{}-vis", m.id);
            let vis = if item.fresh {
                motion::follow_from(&vis_key, 0., if shown { 1. } else { 0. }, ROW_IN)
            } else {
                motion::follow(&vis_key, if shown { 1. } else { 0. }, GLIDE)
            };
            if vis < 0.005 {
                continue;
            }
            let edit = editing
                .as_ref()
                .filter(|(id, _)| id == &m.id)
                .map(|(_, input)| input.clone());
            rows.push(memory_row(
                &key,
                m,
                self.categories.get(m.category),
                vis,
                edit,
                state.clone(),
                &t,
            ));
        }
        let empty = rows.is_empty().then(|| {
            div()
                .h(px(ROW_H * 1.5))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child("Nothing remembered here yet.")
        });
        let list = div()
            .flex()
            .flex_col()
            .py(px(space::S4))
            .border_t_1()
            .border_color(t.border)
            .opacity(0.45 + 0.55 * on)
            .children(rows)
            .children(empty);

        // ---- tray -------------------------------------------------------------
        let tray: AnyElement = match undo {
            Some((text, elapsed)) => {
                let left = (1. - elapsed / UNDO_FOR.as_secs_f32()).clamp(0., 1.);
                let drain =
                    motion::follow(&format!("{key}-drain"), left, MotionSpec::new(250, LINEAR));
                let state = state.clone();
                let el = div()
                    .relative()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .gap(px(space::S4))
                            .text_size(px(text::FOOTNOTE))
                            .child(div().flex_none().text_color(t.text_muted).child("Forgot"))
                            .child(
                                div()
                                    .truncate()
                                    .text_color(t.text)
                                    .child(format!("“{text}”")),
                            ),
                    )
                    .child(
                        luna::button(
                            format!("{key}-undo"),
                            ButtonKind::Secondary,
                            ControlSize::Sm,
                            &t,
                        )
                        .child("Undo")
                        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.restore(cx))),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(-space::S16))
                            .right(px(-space::S16))
                            .top(px(-11.))
                            .h(px(2.))
                            .child(div().h_full().w(relative(drain)).bg(t.accent.opacity(0.7))),
                    );
                rise(format!("{key}-tray-undo"), el)
            }
            None => {
                let state = state.clone();
                let el = div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(format!(
                                "{} {} · {pinned} pinned",
                                live.len(),
                                if live.len() == 1 {
                                    "memory"
                                } else {
                                    "memories"
                                }
                            )),
                    )
                    .child(
                        luna::button(format!("{key}-add"), ButtonKind::Ghost, ControlSize::Sm, &t)
                            .child(luna::glyph(ICON_PLUS, ControlSize::Sm).text_color(t.text_muted))
                            .child("Remember something")
                            .on_click(move |_, window, cx| {
                                state.update(cx, |s, cx| s.add(window, cx))
                            }),
                    );
                rise(format!("{key}-tray-idle"), el)
            }
        };

        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .w(px(self.width))
            .overflow_hidden()
            .font_family(luna::SANS)
            .text_color(t.text)
            .child(header)
            .children(paused)
            .child(list)
            .child(
                div()
                    .h(px(44.))
                    .flex()
                    .items_center()
                    .px(px(space::S16))
                    .border_t_1()
                    .border_color(t.border)
                    .bg(t.fill(FillTier::Quaternary))
                    .child(tray),
            )
    }
}

/// A short rise for content swapping into the tray.
fn rise(id: String, el: gpui::Div) -> AnyElement {
    motion::rise_in(SharedString::from(id), MENU_IN, 3., el).into_any_element()
}

/// A glyph-only row action whose glyph brightens with its hover wash.
fn action(id: String, icon: &'static str, rest: gpui::Hsla, t: &Luna) -> gpui::Stateful<gpui::Div> {
    let tone = motion::hover_blend(&id, rest, if rest == t.text_muted { t.text } else { rest });
    div()
        .id(SharedString::from(id.clone()))
        .size(px(24.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .cursor_pointer()
        .hover_fill(
            id,
            t.fill(FillTier::Tertiary).opacity(0.),
            t.fill(FillTier::Tertiary),
        )
        .child(svg().path(icon).size(px(14.)).text_color(tone))
}

fn memory_row(
    key: &str,
    m: &Memory,
    category: Option<&MemoryCategory>,
    vis: f32,
    editor: Option<Entity<TextInput>>,
    state: Entity<MemoryState>,
    t: &Luna,
) -> AnyElement {
    let row_key = format!("{key}-row-{}", m.id);
    let hover = motion::hover_t(&row_key);
    let editing = editor.is_some();
    let pin_on = motion::follow(
        &format!("{row_key}-pin"),
        if m.pinned { 1. } else { 0. },
        HOVER_FADE,
    );
    let tile = div()
        .size(px(28.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .bg(t.fill(FillTier::Tertiary))
        .when_some(category, |d, c| {
            d.child(
                svg()
                    .path(c.icon.clone())
                    .size(px(14.))
                    .text_color(t.text_muted),
            )
        });

    let action = |name: &str, icon: &'static str, rest: gpui::Hsla| {
        action(format!("{row_key}-{name}"), icon, rest, t)
    };

    let body: AnyElement = match editor {
        Some(input) => {
            let s1 = state.clone();
            let s2 = state.clone();
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(space::S8))
                .child(
                    div()
                        .px(px(space::S8))
                        .py(px(space::S6))
                        .rounded(px(radius::MD))
                        .bg(t.input_bg)
                        .border_1()
                        .border_color(t.accent.opacity(0.6))
                        .shadow(vec![crate::primitives::ring(
                            2.,
                            t.focus_ring().opacity(0.25),
                        )])
                        .text_size(px(text::BODY))
                        .line_height(px(20.))
                        .text_color(t.text)
                        .child(input),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .child(luna::kbd("↵", t))
                        .child(
                            div()
                                .text_size(px(text::CAPTION))
                                .text_color(t.text_faint)
                                .child("save"),
                        )
                        .child(luna::kbd("esc", t))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(text::CAPTION))
                                .text_color(t.text_faint)
                                .child("cancel"),
                        )
                        .child(
                            luna::button(
                                format!("{row_key}-cancel"),
                                ButtonKind::Ghost,
                                ControlSize::Sm,
                                t,
                            )
                            .child("Cancel")
                            .on_click(move |_, _, cx| s1.update(cx, |s, cx| s.cancel(cx))),
                        )
                        .child(
                            luna::button(
                                format!("{row_key}-save"),
                                ButtonKind::Primary,
                                ControlSize::Sm,
                                t,
                            )
                            .child("Save")
                            .on_click(move |_, window, cx| {
                                s2.update(cx, |s, cx| s.commit(window, cx))
                            }),
                        ),
                )
                .into_any_element()
        }
        None => div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(space::S8))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .text_color(t.text)
                            .child(m.text.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S4))
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(t.text_faint)
                            .when(pin_on > 0.01, |d| {
                                d.child(
                                    svg()
                                        .path(ICON_PIN)
                                        .size(px(11.))
                                        .flex_none()
                                        .text_color(t.accent.opacity(pin_on)),
                                )
                                .child(div().text_color(t.accent.opacity(pin_on)).child("Pinned ·"))
                            })
                            .child(div().truncate().child(m.source.clone())),
                    ),
            )
            .into_any_element(),
    };

    // Row actions float over the end of the text on hover, on an opaque
    // copy of the hover wash with a short fade in front.
    let overlay = (!editing && hover > 0.01).then(|| {
        let (pin_state, edit_state, forget_state) = (state.clone(), state.clone(), state.clone());
        let (pin_id, edit_id, forget_id) = (m.id.clone(), m.id.clone(), m.id.clone());
        let wash = t.fill(FillTier::Quaternary);
        let solid = color_mix(gpui::Hsla { a: 1., ..wash }, t.surface_card, wash.a);
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .right(px(space::S6))
            .flex()
            .items_center()
            .opacity(hover)
            .child(div().w(px(28.)).h_full().bg(linear_gradient(
                90.,
                linear_color_stop(solid.opacity(0.), 0.),
                linear_color_stop(solid, 1.),
            )))
            .child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(space::S2))
                    .bg(solid)
                    .child(
                        action(
                            "pin",
                            ICON_PIN,
                            if m.pinned { t.accent } else { t.text_muted },
                        )
                        .on_click(move |_, window, cx| {
                            pin_state.update(cx, |s, cx| s.toggle_pin(pin_id.clone(), window, cx))
                        }),
                    )
                    .child(action("edit", ICON_EDIT, t.text_muted).on_click(
                        move |_, window, cx| {
                            edit_state.update(cx, |s, cx| s.edit(edit_id.clone(), window, cx))
                        },
                    ))
                    .child(action("forget", ICON_FORGET, t.text_muted).on_click(
                        move |_, window, cx| {
                            forget_state.update(cx, |s, cx| s.forget(forget_id.clone(), window, cx))
                        },
                    )),
            )
    });

    let row = div()
        .id(SharedString::from(row_key.clone()))
        .relative()
        .mx(px(space::S6))
        .flex()
        .gap(px(space::S10))
        .px(px(space::S10))
        .rounded(px(radius::MD))
        .when(!editing, |d| {
            d.items_center().h(px(ROW_H - 4.)).hover_fill(
                row_key,
                t.fill(FillTier::Quaternary).opacity(0.),
                t.fill(FillTier::Quaternary),
            )
        })
        .when(editing, |d| {
            d.items_start()
                .py(px(space::S10))
                .bg(t.fill(FillTier::Quaternary))
        })
        .child(div().when(editing, |d| d.pt(px(2.))).child(tile))
        .child(body)
        .children(overlay);

    div()
        .when(!editing && vis < 0.999, |d| d.h(px(ROW_H * vis)))
        .when(editing, |d| d.my(px(2.)))
        .py(px(2.))
        .overflow_hidden()
        .opacity(vis)
        .child(row)
        .into_any_element()
}

fn demo_categories() -> Vec<MemoryCategory> {
    vec![
        MemoryCategory::new("About you", ICON_PERSON),
        MemoryCategory::new("Work", ICON_WORK),
        MemoryCategory::new("Style", ICON_STYLE),
    ]
}

fn demo_memories() -> Vec<Memory> {
    vec![
        Memory::new("m1", "Wants the answer first, then the reasoning", 2)
            .source("You added · Sep 5")
            .pinned(true),
        Memory::new("m2", "Leads the payments team at a 40-person fintech", 1)
            .source("From chat · Oct 9"),
        Memory::new("m3", "Based in Lisbon – schedule things in Lisbon time", 0)
            .source("From chat · Oct 3"),
        Memory::new(
            "m4",
            "Writes Go services; reviews TypeScript on the side",
            1,
        )
        .source("From chat · Oct 14"),
        Memory::new("m5", "Vegetarian – leave meat out of recipe ideas", 0)
            .source("You added · Sep 21"),
        Memory::new("m6", "Weekly planning happens on Monday mornings", 1)
            .source("From calendar · Oct 1"),
        Memory::new("m7", "Uses metric units and 24-hour time", 2).source("From chat · Oct 11"),
        Memory::new("m8", "Prefers short RFCs over long Slack threads", 1)
            .source("You added · Sep 12"),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        MemoryPanel::new("memory-panel", demo_categories())
            .memories(demo_memories())
            .assistant("Atlas")
    })
}
