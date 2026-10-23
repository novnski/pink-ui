//! Memory Panel – what the assistant remembers about you, and the controls
//! to change it.
//!
//! Pink has no memory surface, so the panel is drawn from pink's own pieces.
//! The card is a settings section card (pink: settings/widgets.rs
//! `section_card` / `section_header`) whose trailing control is the pause
//! switch (`switch_control`); category filters are pink chips with their
//! counts, and while memory is paused the amber `warning_strip` folds open
//! under them and the list rests at the disabled opacity.
//!
//! A memory is a sidebar task row (pink: shell.rs `render_chat_row`): 52
//! tall, `px 8 / py 6`, radius 8, the memory on a 13/18 line that fades out
//! at its right edge instead of ending in an ellipsis, and a quiet 11/14 meta
//! line with the category glyph. Under the pointer the row takes the
//! sidebar's hover wash and its corner swaps in the 20 px actions – pin
//! (accent while pinned), edit, forget. Pinned memories gather under their
//! own heading, as the sidebar's sections do (pink: activity_center.rs
//! `activity_section_heading`), and pinning moves a row there with the
//! `DISCLOSURE` fold. Clicking a row edits it in place the way pink's goal
//! editor does (composer/goals.rs `render_goal_editor`): a field, then the
//! source and Cancel / Save; Enter saves, Esc cancels. A forgotten memory
//! folds away and the footer offers pink's notice capsule Undo with a
//! working-trailer countdown; "Remember something" adds one of your own.
//!
//! ```ignore
//! MemoryPanel::new("memory", vec![MemoryCategory::new("Work", ICON_WORK)])
//!     .memory(Memory::new("m1", "Leads the payments team", 0).source("From chat · Oct 9"))
//!     .on_change(|change, _, _| println!("{change:?}"))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, Hsla, IntoElement, Role,
    SharedString, Stateful, Subscription, Task, Window, div, linear_color_stop, linear_gradient,
    prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, DISCLOSURE, TOOL_ROW_REVEAL},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, chrome, layout, loaders, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Memory Panel";
pub const DESCRIPTION: &str = "What the assistant remembers about you on a pink settings card – filter by category, pin, edit in place or forget a memory with an Undo countdown, add your own, or pause memory with its switch";

/// pink `icons::PIN`, `icons::PEN` and `icons::TRASH_BIN_MINIMALISTIC`: the
/// row's actions.
const ICON_PIN: &str = "icons/memory-panel/pin.svg";
const ICON_EDIT: &str = "icons/memory-panel/pen.svg";
const ICON_FORGET: &str = "icons/memory-panel/trash-bin-minimalistic.svg";
/// pink `icons::PLUS` and `icons::PAUSE`.
const ICON_PLUS: &str = "icons/memory-panel/plus.svg";
const ICON_PAUSE: &str = "icons/memory-panel/pause.svg";
/// Category glyphs for the demo (and for your own categories): pink
/// `icons::TOOLBOX` and `icons::PALETTE`, and a person drawn in their style.
pub const ICON_PERSON: &str = "icons/memory-panel/person.svg";
pub const ICON_WORK: &str = "icons/memory-panel/toolbox.svg";
pub const ICON_STYLE: &str = "icons/memory-panel/palette.svg";

/// pink shell.rs `chat_row_height`: a title line over one meta line.
const ROW_H: f32 = 52.0;
/// pink shell.rs `sidebar_faded_label`: only the last few glyphs recede.
const TITLE_FADE: f32 = 20.0;
/// pink shell.rs chat row: the glyph beside the 11 px meta text.
const META_GLYPH: f32 = 11.0;
/// pink spaces.rs `SIDEBAR_DISCLOSURE_HEADER_HEIGHT` and
/// `SIDEBAR_SECTION_GAP`.
const HEADING_H: f32 = 28.0;
const SECTION_GAP: f32 = 12.0;
/// pink settings/widgets.rs `warning_strip`: `py 10` around a 16 px line,
/// inside its 1 px edge.
const STRIP_H: f32 = 38.0;
/// The footer: a Sm control with 10 px above and below.
const FOOTER_H: f32 = 44.0;
/// The list's side inset: with a row's own `px 8` it puts every memory on
/// the header's 16 px edge (pink settings/widgets.rs `ROW_PAD_X`).
const LIST_INSET: f32 = 8.0;
/// How long Undo stays in the footer after forgetting.
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
    enabled: bool,
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
            enabled: true,
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

    /// Who remembers, for the description ("Atlas", default "the assistant").
    pub fn assistant(mut self, name: impl Into<SharedString>) -> Self {
        self.assistant = name.into();
        self
    }

    /// Whether memory starts switched on (default); the switch owns it after
    /// that.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
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
    /// Added in this session: it rises in instead of appearing at once.
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
    /// Counts footer swaps, so each one plays its entrance.
    serial: usize,
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
        if self.editing.as_ref().is_some_and(|e| e.id == id) {
            return;
        }
        // One editor at a time: an unsaved one is dropped first.
        if self.editing.is_some() {
            self.cancel(cx);
        }
        let Some(ix) = self.index(&id) else { return };
        let t = Pink::get(cx);
        let text = self.items[ix].memory.text.clone();
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .multiline()
                .submit_on_enter()
                .default_text(text)
                .placeholder("Something to remember…")
                .placeholder_color(t.placeholder)
                .caret_color(t.caret)
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
        self.serial += 1;
        self.undo = Some(Pending {
            id,
            text: self.items[ix].memory.text.clone(),
            at: Instant::now(),
        });
        self._undo_timer = Some(cx.spawn_in(window, async move |this, cx| {
            // Tick the footer's countdown, then let the memory go.
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
            self.serial += 1;
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
        self.serial += 1;
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

/// The list's two groups: pinned memories on top, the rest below.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Pinned,
    Recent,
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Section::Pinned => "pinned",
            Section::Recent => "recent",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::Pinned => "Pinned",
            Section::Recent => "Recent",
        }
    }

    fn holds(self, memory: &Memory) -> bool {
        memory.pinned == (self == Section::Pinned)
    }
}

impl RenderOnce for MemoryPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial = self.memories.clone();
        let start_enabled = self.enabled;
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
                enabled: start_enabled,
                editing: None,
                undo: None,
                next_id: 0,
                serial: 0,
                on_change: None,
                _undo_timer: None,
            },
        );
        // The latest handler, so changes made from async work can report too.
        let on_change = self.on_change.clone();
        state.update(cx, |s, _| s.on_change = on_change);

        let (items, filter, enabled, editing, undo, serial) = {
            let s = state.read(cx);
            (
                s.items.clone(),
                s.filter,
                s.enabled,
                s.editing.as_ref().map(|e| (e.id.clone(), e.input.clone())),
                s.undo.as_ref().map(|p| (p.text.clone(), p.at.elapsed())),
                s.serial,
            )
        };
        let in_filter = |m: &Memory| filter.is_none_or(|c| m.category == c);
        let live: Vec<&Item> = items.iter().filter(|i| !i.forgotten).collect();
        let pinned = live.iter().filter(|i| i.memory.pinned).count();
        let on = motion::follow(
            &format!("{key}-on"),
            if enabled { 1. } else { 0. },
            DISCLOSURE,
        );

        // ---- header: pink's section header with the switch as its control --
        let switch = {
            let state = state.clone();
            pink::switch_control(format!("{key}-switch"), "Memory", enabled, &t).on_click(
                move |_, window, cx| {
                    state.update(cx, |s, cx| s.set_enabled(!s.enabled, window, cx))
                },
            )
        };
        let header = pink::section_header(
            "Memory",
            Some(format!("What {} keeps in mind across chats.", self.assistant).into()),
            &t,
        )
        .child(pink::row_controls().child(switch));

        // ---- category filters ---------------------------------------------
        let count_of = |cat: Option<usize>| {
            live.iter()
                .filter(|i| cat.is_none_or(|c| i.memory.category == c))
                .count()
        };
        let chip = |ix: Option<usize>, label: SharedString| {
            let state = state.clone();
            let id = match ix {
                Some(ix) => format!("{key}-filter-{ix}"),
                None => format!("{key}-filter-all"),
            };
            pink::chip(id, filter == ix, ControlSize::Sm, &t)
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.filter = ix;
                        cx.notify();
                    })
                })
                .child(label)
                .child(
                    div()
                        .font_features(pink::tabular_figures())
                        .opacity(0.7)
                        .child(count_of(ix).to_string()),
                )
        };
        let filters = div()
            .px(px(layout::ROW_PAD_X))
            .pb(px(12.))
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .child(chip(None, "All".into()))
            .children(
                self.categories
                    .iter()
                    .enumerate()
                    .map(|(ix, c)| chip(Some(ix), c.name.clone())),
            );

        // ---- paused: pink's warning strip folds open under the filters -----
        let paused = 1. - on;
        let strip = (paused > 0.005).then(|| {
            let (plate, edge, copy) = pink::strip_colors(t.warning, t.warning_muted, &t);
            div()
                .h(px((STRIP_H + 12.) * paused))
                .overflow_hidden()
                .opacity(paused)
                .child(
                    div()
                        .mx(px(layout::ROW_PAD_X))
                        .h(px(STRIP_H))
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .px(px(16.))
                        .py(px(10.))
                        .rounded(px(radius::SECTION_CARD))
                        .border_1()
                        .border_color(edge)
                        .bg(plate)
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(copy)
                        .child(
                            svg()
                                .path(ICON_PAUSE)
                                .size(px(14.))
                                .mt(px(1.))
                                .flex_none()
                                .text_color(copy),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(format!(
                            "Paused – {} won't save or use memories.",
                            self.assistant
                        ))),
                )
        });

        // ---- list: a pinned section over the rest ------------------------
        // Every memory has a slot in both sections; pinning folds it out of
        // one and into the other, forgetting folds it out of both.
        let pinned_here = live
            .iter()
            .filter(|i| i.memory.pinned && in_filter(&i.memory))
            .count();
        let mut list: Vec<AnyElement> = Vec::new();
        let mut any_row = false;
        for section in [Section::Pinned, Section::Recent] {
            let count = live
                .iter()
                .filter(|i| section.holds(&i.memory) && in_filter(&i.memory))
                .count();
            // A lone "Recent" heading would say nothing: it shows only under
            // a pinned section.
            let titled = count > 0 && (section == Section::Pinned || pinned_here > 0);
            let heading = motion::follow(
                &format!("{key}-{}-heading", section.key()),
                if titled { 1. } else { 0. },
                DISCLOSURE,
            );
            if heading > 0.005 {
                let gap = if section == Section::Pinned {
                    0.
                } else {
                    SECTION_GAP
                };
                list.push(section_heading(
                    format!("{key}-{}-heading", section.key()),
                    section.label(),
                    count,
                    gap,
                    heading,
                    &t,
                ));
            }
            for item in &items {
                let m = &item.memory;
                let shown = !item.forgotten && section.holds(m) && in_filter(m);
                let vis_key = format!("{key}-{}-{}-vis", section.key(), m.id);
                let target = if shown { 1. } else { 0. };
                let vis = if item.fresh {
                    motion::follow_from(&vis_key, 0., target, TOOL_ROW_REVEAL)
                } else {
                    motion::follow(&vis_key, target, DISCLOSURE)
                };
                if vis < 0.005 {
                    continue;
                }
                any_row = true;
                let editor = editing
                    .as_ref()
                    .filter(|(id, _)| id == &m.id)
                    .map(|(_, input)| input.clone());
                list.push(memory_row(
                    RowParts {
                        key: format!("{key}-{}-{}", section.key(), m.id),
                        category: self.categories.get(m.category),
                        name_category: filter.is_none(),
                        vis,
                        editor,
                        state: state.clone(),
                    },
                    m,
                    &t,
                ));
            }
        }
        let empty = (!any_row).then(|| {
            motion::fade_in(
                SharedString::from(format!("{key}-empty")),
                pink::menu_empty("Nothing remembered here yet.", &t),
            )
        });
        let list = div()
            .flex()
            .flex_col()
            .px(px(LIST_INSET))
            .py(px(6.))
            .border_t_1()
            .border_color(t.border)
            // Paused, the list rests at pink's disabled opacity.
            .opacity(motion::lerp(0.45, 1., on))
            .children(list)
            .children(empty);

        // ---- footer: the count and Add, or Undo after forgetting -----------
        let footer: AnyElement = match undo {
            Some((text, elapsed)) => {
                // Whole seconds left, never "0s" while Undo still works.
                let left = (UNDO_FOR.saturating_sub(elapsed).as_secs_f32().ceil() as i64).max(1);
                let state = state.clone();
                motion::status_swap(
                    SharedString::from(format!("{key}-footer-{serial}")),
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(t.readable_muted(0.85))
                                        .child("Forgot"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_color(t.text)
                                        .child(format!("“{text}”")),
                                ),
                        )
                        .child(loaders::working_timer(left, t.content_faint))
                        .child(
                            pink::notice_action(format!("{key}-undo"), &t)
                                .aria_label("Undo forget")
                                .text_size(px(12.))
                                .child("Undo")
                                .on_click(move |_, _, cx| state.update(cx, |s, cx| s.restore(cx))),
                        ),
                )
                .into_any_element()
            }
            None => {
                let state = state.clone();
                let add_id = format!("{key}-add");
                let glyph = motion::hover_blend(
                    &format!("pink-btn-{add_id}"),
                    t.readable_muted(0.95),
                    t.text,
                );
                let summary = format!(
                    "{} {}{}",
                    live.len(),
                    if live.len() == 1 {
                        "memory"
                    } else {
                        "memories"
                    },
                    if pinned > 0 {
                        format!(" · {pinned} pinned")
                    } else {
                        String::new()
                    }
                );
                motion::status_swap(
                    SharedString::from(format!("{key}-footer-{serial}")),
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_size(px(11.))
                                .line_height(px(14.))
                                .font_features(pink::tabular_figures())
                                .text_color(t.readable_faint(0.9))
                                .child(summary),
                        )
                        .child(
                            pink::button(add_id, ButtonKind::Ghost, ControlSize::Sm, &t)
                                .child(pink::glyph(ICON_PLUS, ControlSize::Sm).text_color(glyph))
                                .child("Remember something")
                                .on_click(move |_, window, cx| {
                                    state.update(cx, |s, cx| s.add(window, cx))
                                }),
                        ),
                )
                .into_any_element()
            }
        };

        motion::keep_alive(window);
        pink::card(&t)
            .id(self.id.clone())
            .w(px(self.width))
            .overflow_hidden()
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(header)
            .child(filters)
            .children(strip)
            .child(list)
            .child(
                div()
                    .h(px(FOOTER_H))
                    .flex_none()
                    .flex()
                    .items_center()
                    .pl(px(layout::ROW_PAD_X))
                    .pr(px(10.))
                    .border_t_1()
                    .border_color(t.border)
                    .child(footer),
            )
    }
}

/// A section's heading (pink: activity_center.rs
/// `activity_section_heading`): 28 tall, `px 8`, the name in 11 semibold
/// `readable_faint(0.9)` and its count in 11 `readable_faint(0.7)`; sections
/// after the first keep the sidebar's 12 px gap above. `vis` folds it.
fn section_heading(
    id: String,
    label: &'static str,
    count: usize,
    gap: f32,
    vis: f32,
    t: &Pink,
) -> AnyElement {
    div()
        .h(px((gap + HEADING_H) * vis))
        .flex_none()
        .overflow_hidden()
        .opacity(vis)
        .flex()
        .flex_col()
        .justify_end()
        .child(
            div()
                .id(SharedString::from(id))
                .role(Role::Heading)
                .aria_label(SharedString::from(format!("{label}, {count}")))
                .h(px(HEADING_H))
                .flex_none()
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.readable_faint(0.9))
                        .child(label),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .font_features(pink::tabular_figures())
                        .text_color(t.readable_faint(0.7))
                        .child(count.to_string()),
                ),
        )
        .into_any_element()
}

/// A corner action on a row (pink: shell.rs chat row pin / settle /
/// archive): 20 square, radius 4, the quiet hover wash, a 12 px glyph and a
/// tooltip.
fn row_action(
    id: String,
    icon: &'static str,
    label: &'static str,
    tone: Hsla,
    t: &Pink,
) -> Stateful<gpui::Div> {
    let ring = t.focus_ring();
    div()
        .id(SharedString::from(id.clone()))
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .size(px(chrome::CONTROL_S))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_S))
        .cursor_pointer()
        .hover_wash(id, t.fill(FillTier::Tertiary))
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            svg()
                .path(icon)
                .size(px(chrome::ICON_S))
                .flex_none()
                .text_color(tone),
        )
}

/// What a row needs besides its memory.
struct RowParts<'a> {
    key: String,
    category: Option<&'a MemoryCategory>,
    /// The meta line names the category unless the list is filtered to one.
    name_category: bool,
    /// 0..1: the row's fold.
    vis: f32,
    editor: Option<Entity<TextInput>>,
    state: Entity<MemoryState>,
}

fn memory_row(row: RowParts, m: &Memory, t: &Pink) -> AnyElement {
    match row.editor.clone() {
        Some(input) => div()
            .opacity(row.vis)
            .child(edit_row(&row, m, input, t))
            .into_any_element(),
        None => div()
            .h(px(ROW_H * row.vis))
            .flex_none()
            .overflow_hidden()
            .opacity(row.vis)
            .child(rest_row(&row, m, t))
            .into_any_element(),
    }
}

/// A memory at rest (pink: shell.rs `render_chat_row`): the memory over a
/// meta line, the hover wash and title brighten on one 150 ms fade, and the
/// corner's actions under the pointer. The row is the edit button.
fn rest_row(row: &RowParts, m: &Memory, t: &Pink) -> Stateful<gpui::Div> {
    let key = &row.key;
    let hover = motion::hover_t(key);
    let wash = motion::mix(t.wash(0.), t.fill(FillTier::Tertiary), hover);
    // The title's fade ends in what lies under it: the wash over the card.
    let fade_to = pink::flatten(wash, t.card_glass_bg());
    let subline = t.readable_muted(0.5);
    let ring = t.focus_ring();

    let title = div()
        .flex_1()
        .min_w_0()
        .relative()
        .overflow_hidden()
        .flex()
        .child(
            div()
                .flex_none()
                .whitespace_nowrap()
                .text_size(px(13.))
                .line_height(px(18.))
                .child(m.text.clone()),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(px(TITLE_FADE))
                .bg(linear_gradient(
                    90.,
                    linear_color_stop(fade_to.opacity(0.), 0.),
                    linear_color_stop(fade_to, 1.),
                )),
        );

    // Only the hovered row's title yields room to the actions; its fade
    // absorbs the change.
    let corner = (hover > 0.01).then(|| {
        let (pin_state, edit_state, forget_state) =
            (row.state.clone(), row.state.clone(), row.state.clone());
        let (pin_id, edit_id, forget_id) = (m.id.clone(), m.id.clone(), m.id.clone());
        div()
            .flex_none()
            .h(px(18.))
            .flex()
            .items_center()
            .gap(px(2.))
            .opacity(hover)
            .child(
                row_action(
                    format!("{key}-pin"),
                    ICON_PIN,
                    if m.pinned { "Unpin" } else { "Pin" },
                    if m.pinned {
                        t.readable_icon(t.accent)
                    } else {
                        t.text_muted
                    },
                    t,
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    pin_state.update(cx, |s, cx| s.toggle_pin(pin_id.clone(), window, cx))
                }),
            )
            .child(
                row_action(format!("{key}-edit"), ICON_EDIT, "Edit", t.text_muted, t).on_click(
                    move |_, window, cx| {
                        cx.stop_propagation();
                        edit_state.update(cx, |s, cx| s.edit(edit_id.clone(), window, cx))
                    },
                ),
            )
            .child(
                row_action(
                    format!("{key}-forget"),
                    ICON_FORGET,
                    "Forget",
                    t.text_muted,
                    t,
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    forget_state.update(cx, |s, cx| s.forget(forget_id.clone(), window, cx))
                }),
            )
    });

    let category = row.category;
    let meta = div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(subline)
        .when_some(category, |d, c| {
            d.child(
                svg()
                    .path(c.icon.clone())
                    .size(px(META_GLYPH))
                    .flex_none()
                    .text_color(subline),
            )
        })
        .child(div().flex_1().min_w_0().truncate().child(
            match category.filter(|_| row.name_category) {
                Some(c) if !m.source.is_empty() => format!("{} · {}", c.name, m.source).into(),
                Some(c) => c.name.clone(),
                None => m.source.clone(),
            },
        ));

    let (state, id) = (row.state.clone(), m.id.clone());
    div()
        .id(SharedString::from(key.clone()))
        .role(Role::Button)
        .aria_label(SharedString::from(format!("Edit memory: {}", m.text)))
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .h(px(ROW_H))
        .px(px(8.))
        .py(px(6.))
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.))
        .rounded(px(radius::MD))
        .bg(wash)
        .text_color(motion::mix(t.text.opacity(0.9), t.text, hover))
        .cursor_pointer()
        .on_hover(motion::hover_listener(key.clone()))
        .on_click(move |_, window, cx| state.update(cx, |s, cx| s.edit(id.clone(), window, cx)))
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(title)
                .children(corner),
        )
        .child(meta)
}

/// A memory being edited (pink: composer/goals.rs `render_goal_editor`):
/// the field, then the source with Cancel and Save.
fn edit_row(row: &RowParts, m: &Memory, input: Entity<TextInput>, t: &Pink) -> gpui::Div {
    let key = &row.key;
    let (cancel_state, save_state) = (row.state.clone(), row.state.clone());
    div()
        .px(px(8.))
        .pt(px(6.))
        .pb(px(8.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            pink::input_frame(t)
                .line_height(px(18.))
                .text_color(t.text)
                .child(input),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(t.readable_faint(0.9))
                        .child(m.source.clone()),
                )
                .child(
                    pink::button(
                        format!("{key}-cancel"),
                        ButtonKind::Ghost,
                        ControlSize::Sm,
                        t,
                    )
                    .child("Cancel")
                    .on_click(move |_, _, cx| cancel_state.update(cx, |s, cx| s.cancel(cx))),
                )
                .child(
                    pink::button(
                        format!("{key}-save"),
                        ButtonKind::Primary,
                        ControlSize::Sm,
                        t,
                    )
                    .child("Save")
                    .on_click(move |_, window, cx| {
                        save_state.update(cx, |s, cx| s.commit(window, cx))
                    }),
                ),
        )
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
        Memory::new("m2", "Vegetarian – leave meat out of recipe ideas", 0)
            .source("You added · Sep 21")
            .pinned(true),
        Memory::new(
            "m3",
            "Writes Go services; reviews TypeScript on the side",
            1,
        )
        .source("From chat · Oct 14"),
        Memory::new("m4", "Uses metric units and 24-hour time", 2).source("From chat · Oct 11"),
        Memory::new("m5", "Leads the payments team at a 40-person fintech", 1)
            .source("From chat · Oct 9"),
        Memory::new("m6", "Based in Lisbon – schedule things in Lisbon time", 0)
            .source("From chat · Oct 3"),
        Memory::new("m7", "Weekly planning happens on Monday mornings", 1)
            .source("From calendar · Oct 1"),
        Memory::new(
            "m8",
            "Prefers a short RFC with the decision up top over a long Slack thread",
            1,
        )
        .source("You added · Sep 12"),
    ]
}

fn paused_memories() -> Vec<Memory> {
    vec![
        Memory::new("p1", "Training for a half marathon in March", 0)
            .source("From chat · Oct 12")
            .pinned(true),
        Memory::new(
            "p2",
            "Keeps a focus block after lunch – no meetings then",
            1,
        )
        .source("From calendar · Oct 2"),
        Memory::new(
            "p3",
            "Likes replies in plain sentences, not bullet lists",
            2,
        )
        .source("You added · Sep 28"),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .flex()
            .items_start()
            .gap(px(24.))
            .child(
                MemoryPanel::new("memory-panel", demo_categories())
                    .memories(demo_memories())
                    .assistant("Atlas"),
            )
            .child(
                MemoryPanel::new("memory-panel-paused", demo_categories())
                    .memories(paused_memories())
                    .assistant("Juno")
                    .enabled(false)
                    .width(400.),
            )
    })
}
