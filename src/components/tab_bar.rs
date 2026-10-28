//! Tab Bar – Pink's workbench tab row: one strip of chat tabs per split
//! group, sitting in the titlebar over its own column of the canvas.
//!
//! Every tab is a 28 pt chip: the agent's mark, a Main badge or the role
//! glyph of a side chat, the title, and the chat's status as its own shape –
//! the working spinner, a shield for an approval, a bubble for a question, a
//! triangle for a failure, an accent dot for a result nobody has read. The
//! active tab takes the only selection mark, its own fill: a quiet accent
//! tint in the group that holds the keyboard, the neutral selected wash in
//! the others. Close buttons wait under the pointer (always on the active
//! tab), `+` opens a side chat, overflowing tabs scroll under faded edges, a
//! narrow group folds its controls into one Group actions button, and a
//! right click opens the tab menu – close, close others, move to a split,
//! balance, reopen. Tabs drag to reorder while their neighbours slide aside,
//! or into another group; ←/→/Home/End walk a focused strip.
//!
//! ```ignore
//! TabBar::new("tabs")
//!     .project("billing-api")
//!     .group(TabGroup::new("main")
//!         .tab(Tab::new("plan", "Retire expired coupon codes").role(TabRole::Main))
//!         .tab(Tab::new("tests", "Write the migration tests").status(TabStatus::Working)))
//!     .on_select(|tab, _, _| println!("show {tab}"))
//! ```

use std::{collections::HashMap, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnimationExt, AnyElement, AnyView, App, Bounds, ClickEvent, Context, Div,
    DragMoveEvent, ElementId, Entity, FocusHandle, FontWeight, Hsla, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, Pixels, Point, Render, Role, ScrollHandle, SharedString, Stateful,
    Svg, Task, Window, anchored, canvas, deferred, div, linear_color_stop, linear_gradient,
    prelude::*, px, svg,
};

use super::tab_chip;
use crate::{
    HoverFill,
    motion::{self, INSERT, MENU_OUT, TAB_SLIDE},
    pink::{
        self, FillTier, Pink, TextStyle, TypeRole, TypeStyleExt, chrome, layout, loaders, radius,
    },
    story,
};

pub const NAME: &str = "Tab Bar";
pub const DESCRIPTION: &str = "A workspace's tab row: one strip of chat tabs per split group with live status glyphs, the focused group's tab in a quiet accent tint, tabs that drag to reorder as their neighbours slide aside, and a menu to close, split or reopen them";

const ICON_HOME: &str = "icons/tab-bar/home.svg";
const ICON_BOT: &str = "icons/tab-bar/bot.svg";
const ICON_FORK: &str = "icons/tab-bar/arrow-fork.svg";
const ICON_COLUMNS: &str = "icons/tab-bar/columns-2.svg";
const ICON_SHIELD: &str = "icons/tab-bar/shield-warning.svg";
const ICON_CHAT: &str = "icons/tab-bar/chat-round-line.svg";
const ICON_TRIANGLE: &str = "icons/tab-bar/danger-triangle.svg";
const ICON_CLOSE: &str = "icons/tab-bar/close.svg";
const ICON_CLOSE_CIRCLE: &str = "icons/tab-bar/close-circle.svg";
const ICON_PLUS: &str = "icons/tab-bar/plus.svg";
const ICON_MORE: &str = "icons/tab-bar/more-horizontal.svg";
const ICON_WIDGET: &str = "icons/tab-bar/widget.svg";
const ICON_RESTART: &str = "icons/tab-bar/restart.svg";
const ICON_CLAUDE: &str = "icons/tab-bar/claude-mark.svg";
const ICON_OPENAI: &str = "icons/tab-bar/openai-mark.svg";
const ICON_OPENCODE: &str = "icons/tab-bar/opencode-mark.svg";

/// Pink: workbench/render.rs `STRIP_HEIGHT` – the titlebar below its top pad.
const STRIP_HEIGHT: f32 = layout::TITLEBAR_HEIGHT - layout::TITLEBAR_TOP_PAD;
/// Pink: workbench/render.rs `TAB_HEIGHT`, from the `pink-tab-chip` package.
const TAB_H: f32 = tab_chip::TAB_HEIGHT;
/// Pink: workbench/render.rs `TAB_MIN_WIDTH` / `TAB_MIN_WIDTH_COMPACT`.
const TAB_MIN: f32 = tab_chip::TAB_MIN_WIDTH;
const TAB_MIN_COMPACT: f32 = tab_chip::TAB_MIN_WIDTH_COMPACT;
/// Pink: workbench/render.rs `TAB_MAX_WIDTH`.
const TAB_MAX: f32 = tab_chip::TAB_MAX_WIDTH;
/// Pink: workbench/render.rs `STRIP_COMPACT_WIDTH` – narrower groups fold
/// their controls into Group actions.
const STRIP_COMPACT: f32 = 220.0;
/// Pink: workbench/render.rs `TAB_STRIP_EDGE_FADE`.
const EDGE_FADE: f32 = 24.0;
/// Pink: workbench/render.rs `CANVAS_INSET`, `CANVAS_RADIUS`, `VIEWPORT_INSET`.
const CANVAS_INSET: f32 = 8.0;
const CANVAS_RADIUS: f32 = 14.0;
const VIEWPORT_INSET: f32 = 6.0;
/// Pink: workbench/render.rs `SPLIT_SEAM`.
const SPLIT_SEAM: f32 = 1.0;
/// Pink: workbench.rs `TAB_EXIT_MS` – a closed tab narrows and fades.
const TAB_EXIT_MS: f32 = 150.0;
/// Pink: workbench/render.rs `render_workbench_menu` – the tab menu's width.
const MENU_W: f32 = 232.0;
/// The gap between tabs (Pink: the tab list's `gap 2`).
const TAB_GAP: f32 = 2.0;
/// Pink: icons.rs `claude_brand` – the Claude mark's own orange.
const CLAUDE_BRAND: u32 = 0xD97757;
/// How long the demo plays before it starts over.
const DEMO_LOOP: f32 = 24.0;

// ---------------------------------------------------------------------------
// Public data
// ---------------------------------------------------------------------------

/// Where a tab's chat came from (Pink `TabRole`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabRole {
    /// The workbench's own chat: it stays open.
    Main,
    /// A side chat that carries its parent's context (a fork).
    Side,
    /// A side chat started fresh.
    Child,
    /// Your own chat, listed beside Main.
    Alongside,
}

impl TabRole {
    fn display(self) -> &'static str {
        match self {
            TabRole::Main => "Main",
            TabRole::Side | TabRole::Child => "Side chat",
            TabRole::Alongside => "Alongside",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            TabRole::Main => ICON_HOME,
            TabRole::Child => ICON_BOT,
            TabRole::Side => ICON_FORK,
            TabRole::Alongside => ICON_COLUMNS,
        }
    }
}

/// A tab's chat status (Pink `ChatIndicator`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabStatus {
    Idle,
    Working,
    NeedsApproval,
    NeedsInput,
    Failed,
    /// Finished while you were elsewhere.
    Unread,
}

/// The agent that runs a tab's chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harness {
    ClaudeCode,
    Codex,
    OpenCode,
}

impl Harness {
    /// Pink: pickers.rs `harness_brand_icon`.
    fn mark(self) -> &'static str {
        match self {
            Harness::ClaudeCode => ICON_CLAUDE,
            Harness::Codex => ICON_OPENAI,
            Harness::OpenCode => ICON_OPENCODE,
        }
    }

    fn tint(self) -> Option<Hsla> {
        match self {
            Harness::ClaudeCode => Some(gpui::rgb(CLAUDE_BRAND).into()),
            Harness::Codex | Harness::OpenCode => None,
        }
    }
}

/// One chat tab.
#[derive(Clone, Debug)]
pub struct Tab {
    pub id: SharedString,
    pub title: SharedString,
    pub role: TabRole,
    pub harness: Option<Harness>,
    pub status: TabStatus,
    /// The chat a side chat came from, named in its tooltip.
    pub source: Option<SharedString>,
}

impl Tab {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            role: TabRole::Child,
            harness: Some(Harness::ClaudeCode),
            status: TabStatus::Idle,
            source: None,
        }
    }

    pub fn role(mut self, role: TabRole) -> Self {
        self.role = role;
        self
    }

    /// The agent (Claude Code by default); `None` hides the mark.
    pub fn harness(mut self, harness: Option<Harness>) -> Self {
        self.harness = harness;
        self
    }

    pub fn status(mut self, status: TabStatus) -> Self {
        self.status = status;
        self
    }

    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// The tooltip's heading and detail (Pink: workbench.rs `tab_help`).
    fn help(&self) -> (SharedString, SharedString) {
        let parent = self.source.clone().unwrap_or_else(|| "Main".into());
        match self.role {
            TabRole::Main => (
                self.title.clone(),
                "This workbench's own chat. It stays open.".into(),
            ),
            TabRole::Side => (
                format!("Side chat of {parent}").into(),
                "Side chats carry the context of the parent thread. Context is captured when created; later parent messages are not copied.".into(),
            ),
            TabRole::Child => (
                format!("Side chat of {parent}").into(),
                "Started fresh: it does not carry the main chat's conversation.".into(),
            ),
            TabRole::Alongside => (
                self.title.clone(),
                "Your own chat, listed beside Main".into(),
            ),
        }
    }
}

/// One split group: its tabs, the one it shows, and its share of the row.
#[derive(Clone, Debug)]
pub struct TabGroup {
    pub id: SharedString,
    pub tabs: Vec<Tab>,
    pub active: Option<SharedString>,
    /// Its width relative to the other groups' (1 each by default).
    pub share: f32,
}

impl TabGroup {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            active: None,
            share: 1.0,
        }
    }

    pub fn tab(mut self, tab: Tab) -> Self {
        self.tabs.push(tab);
        self
    }

    /// The tab it shows (its first by default).
    pub fn active(mut self, tab_id: impl Into<SharedString>) -> Self {
        self.active = Some(tab_id.into());
        self
    }

    pub fn share(mut self, share: f32) -> Self {
        self.share = share.max(0.1);
        self
    }

    fn holds_main(&self) -> bool {
        self.tabs.iter().any(|t| t.role == TabRole::Main)
    }

    fn active_ix(&self) -> Option<usize> {
        self.active
            .as_ref()
            .and_then(|id| self.tabs.iter().position(|t| &t.id == id))
    }
}

type TabHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type NewHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type LayoutHandler = Rc<dyn Fn(&[TabGroup], &mut Window, &mut App)>;

#[derive(Clone, Default)]
struct Handlers {
    select: Option<TabHandler>,
    close: Option<TabHandler>,
    new: Option<NewHandler>,
    layout: Option<LayoutHandler>,
}

/// Something to tell the host once the state has settled.
enum Effect {
    Select(SharedString),
    Close(SharedString),
    New(SharedString, SharedString),
    Layout,
}

/// A scripted change for the demo.
#[derive(Clone, Debug)]
enum Script {
    /// Replaces the tab with the same id, or adds it to the group.
    Upsert(SharedString, Tab),
    Close(SharedString),
    Focus(SharedString),
    Select(SharedString),
}

// ---------------------------------------------------------------------------
// The builder
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct TabBar {
    id: ElementId,
    project: Option<SharedString>,
    groups: Vec<TabGroup>,
    focused: Option<SharedString>,
    width: f32,
    canvas_height: f32,
    script: Vec<(f32, Script)>,
    autoplay: bool,
    handlers: Handlers,
}

impl TabBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            project: None,
            groups: Vec::new(),
            focused: None,
            width: 880.0,
            canvas_height: 0.0,
            script: Vec::new(),
            autoplay: false,
            handlers: Handlers::default(),
        }
    }

    /// The project crumb that leads the first strip ("billing-api ›").
    pub fn project(mut self, name: impl Into<SharedString>) -> Self {
        self.project = Some(name.into());
        self
    }

    /// One split group (left to right).
    pub fn group(mut self, group: TabGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// The group that holds the keyboard (the first by default).
    pub fn focused(mut self, group_id: impl Into<SharedString>) -> Self {
        self.focused = Some(group_id.into());
        self
    }

    /// The row's width; the groups share it.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Draws the workbench canvas under the row, `height` tall, one column
    /// per group under its strip.
    pub fn canvas(mut self, height: f32) -> Self {
        self.canvas_height = height.max(0.0);
        self
    }

    /// During autoplay, replaces (or adds to `group`) the tab with the same
    /// id `at` seconds in.
    pub fn event(mut self, at: f32, group: impl Into<SharedString>, tab: Tab) -> Self {
        self.script.push((at, Script::Upsert(group.into(), tab)));
        self
    }

    /// During autoplay, closes a tab `at` seconds in.
    pub fn close_event(mut self, at: f32, tab_id: impl Into<SharedString>) -> Self {
        self.script.push((at, Script::Close(tab_id.into())));
        self
    }

    /// During autoplay, moves the keyboard to a group `at` seconds in.
    pub fn focus_event(mut self, at: f32, group_id: impl Into<SharedString>) -> Self {
        self.script.push((at, Script::Focus(group_id.into())));
        self
    }

    /// During autoplay, shows a tab in its group `at` seconds in.
    pub fn select_event(mut self, at: f32, tab_id: impl Into<SharedString>) -> Self {
        self.script.push((at, Script::Select(tab_id.into())));
        self
    }

    /// Plays the scripted events in a loop until someone clicks.
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    /// A tab was chosen (click, ←/→, a reopen).
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.select = Some(Rc::new(handler));
        self
    }

    /// A tab closed (its ×, a middle click or the menu).
    pub fn on_close(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.close = Some(Rc::new(handler));
        self
    }

    /// `+` (or Side chat… in the menu) opened a side chat: the group id and
    /// the new tab's id.
    pub fn on_new(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.new = Some(Rc::new(handler));
        self
    }

    /// The groups after a reorder, a move, a split, Close group, Balance
    /// panes or a reopen.
    pub fn on_layout(
        mut self,
        handler: impl Fn(&[TabGroup], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.layout = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// A closed tab narrowing out where it was (Pink `LeavingTab`).
#[derive(Clone)]
struct Leaving {
    group: SharedString,
    index: usize,
    title: SharedString,
    width: f32,
    started: Instant,
}

/// A drag in flight (Pink: the terminal drawer's `DragState`).
#[derive(Clone)]
struct DragState {
    group: SharedString,
    from: usize,
    /// The strip it is over, and the slot it would land in.
    target: SharedString,
    over: usize,
    /// The dragged tab's width: what its neighbours slide by.
    width: f32,
    /// One id per drag, so every drag starts its slides from rest.
    session: usize,
}

/// The dragged tab.
#[derive(Clone)]
struct TabDrag {
    bar: SharedString,
    group: SharedString,
    id: SharedString,
    title: SharedString,
    role: TabRole,
    harness: Option<Harness>,
}

/// The tab menu: which group and tab it acts on, and where it opened.
#[derive(Clone)]
struct Menu {
    group: SharedString,
    tab: Option<SharedString>,
    /// Opened from a compact strip's Group actions: the group's own
    /// actions lead.
    group_actions: bool,
    position: Point<Pixels>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    Left,
    Right,
}

struct BarState {
    initial: Vec<TabGroup>,
    initial_focused: Option<SharedString>,
    script: Vec<(f32, Script)>,
    played: usize,
    autoplay: bool,
    clock: f32,
    loop_start: f32,
    last_tick: Instant,
    groups: Vec<TabGroup>,
    focused: SharedString,
    /// Closed tabs, newest last, for Reopen closed tab.
    closed: Vec<(SharedString, usize, Tab)>,
    leaving: Vec<Leaving>,
    /// Each tab's laid-out box (its slot, never its slide), window coords.
    bounds: HashMap<SharedString, Bounds<Pixels>>,
    /// Each group's strip box and its room for tabs.
    strips: HashMap<SharedString, Bounds<Pixels>>,
    slots: HashMap<SharedString, f32>,
    scrolls: HashMap<SharedString, ScrollHandle>,
    focus: HashMap<SharedString, FocusHandle>,
    drag: Option<DragState>,
    drag_sessions: usize,
    menu: Option<Menu>,
    menu_closing: Option<(Menu, Instant)>,
    menu_focus: FocusHandle,
    menu_reap: Option<Task<()>>,
    next_id: usize,
    handlers: Handlers,
    _ticker: Task<()>,
}

impl BarState {
    fn new(config: &TabBar, cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if this.update(cx, |state, cx| state.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let mut state = Self {
            initial: config.groups.clone(),
            initial_focused: config.focused.clone(),
            script: config.script.clone(),
            played: 0,
            autoplay: config.autoplay,
            clock: 0.0,
            loop_start: 0.0,
            last_tick: Instant::now(),
            groups: Vec::new(),
            focused: SharedString::default(),
            closed: Vec::new(),
            leaving: Vec::new(),
            bounds: HashMap::new(),
            strips: HashMap::new(),
            slots: HashMap::new(),
            scrolls: HashMap::new(),
            focus: HashMap::new(),
            drag: None,
            drag_sessions: 0,
            menu: None,
            menu_closing: None,
            menu_focus: cx.focus_handle(),
            menu_reap: None,
            next_id: 0,
            handlers: config.handlers.clone(),
            _ticker: ticker,
        };
        state.load();
        state
    }

    fn load(&mut self) {
        self.played = 0;
        self.loop_start = self.clock;
        self.groups = self.initial.clone();
        for group in &mut self.groups {
            if group.active_ix().is_none() {
                group.active = group.tabs.first().map(|t| t.id.clone());
            }
        }
        self.focused = self
            .initial_focused
            .clone()
            .filter(|id| self.groups.iter().any(|g| &g.id == id))
            .or_else(|| self.groups.first().map(|g| g.id.clone()))
            .unwrap_or_default();
        self.closed.clear();
        self.leaving.clear();
        self.drag = None;
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.clock += now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .min(1.0);
        self.last_tick = now;
        if self.autoplay && self.drag.is_none() && self.menu.is_none() {
            let t = self.clock - self.loop_start;
            while let Some((at, script)) = self.script.get(self.played).cloned() {
                if t < at {
                    break;
                }
                self.played += 1;
                self.play(script);
            }
            if t >= DEMO_LOOP {
                self.load();
            }
        }
        cx.notify();
    }

    fn play(&mut self, script: Script) {
        match script {
            Script::Upsert(group, tab) => {
                if let Some(existing) = self
                    .groups
                    .iter_mut()
                    .flat_map(|g| g.tabs.iter_mut())
                    .find(|t| t.id == tab.id)
                {
                    *existing = tab;
                } else if let Some(group) = self.groups.iter_mut().find(|g| g.id == group) {
                    group.tabs.push(tab);
                }
            }
            Script::Close(id) => {
                self.close_tab(&id);
            }
            Script::Focus(group) => {
                if self.groups.iter().any(|g| g.id == group) {
                    self.focused = group;
                }
            }
            Script::Select(id) => {
                self.select(&id);
            }
        }
    }

    fn multi(&self) -> bool {
        self.groups.len() > 1
    }

    fn group_of(&self, tab: &SharedString) -> Option<usize> {
        self.groups
            .iter()
            .position(|g| g.tabs.iter().any(|t| &t.id == tab))
    }

    fn fresh_id(&mut self, prefix: &str) -> SharedString {
        self.next_id += 1;
        format!("{prefix}-{}", self.next_id).into()
    }

    // -- Actions --------------------------------------------------------------

    /// Shows a tab in its group and moves the keyboard there; reading it
    /// clears its unread dot.
    fn select(&mut self, id: &SharedString) -> Vec<Effect> {
        let Some(gix) = self.group_of(id) else {
            return Vec::new();
        };
        let group = &mut self.groups[gix];
        group.active = Some(id.clone());
        // Bring it into view in its strip.
        if let (Some(ix), Some(scroll)) = (
            group.tabs.iter().position(|t| &t.id == id),
            self.scrolls.get(&group.id),
        ) {
            scroll.scroll_to_item(ix);
        }
        if let Some(tab) = group.tabs.iter_mut().find(|t| &t.id == id)
            && tab.status == TabStatus::Unread
        {
            tab.status = TabStatus::Idle;
        }
        self.focused = group.id.clone();
        vec![Effect::Select(id.clone())]
    }

    /// Closes a tab where it stands; its group shows a neighbour. Main never
    /// closes.
    fn close_tab(&mut self, id: &SharedString) -> Vec<Effect> {
        let Some(gix) = self.group_of(id) else {
            return Vec::new();
        };
        let group = &mut self.groups[gix];
        let Some(ix) = group.tabs.iter().position(|t| &t.id == id) else {
            return Vec::new();
        };
        if group.tabs[ix].role == TabRole::Main {
            return Vec::new();
        }
        let tab = group.tabs.remove(ix);
        let width = self
            .bounds
            .get(id)
            .map(|b| f32::from(b.size.width))
            .unwrap_or(TAB_MIN);
        self.leaving.push(Leaving {
            group: group.id.clone(),
            index: ix,
            title: tab.title.clone(),
            width,
            started: Instant::now(),
        });
        if group.active.as_ref() == Some(id) {
            group.active = group
                .tabs
                .get(ix)
                .or_else(|| ix.checked_sub(1).and_then(|i| group.tabs.get(i)))
                .map(|t| t.id.clone());
        }
        self.closed.push((group.id.clone(), ix, tab));
        let mut effects = vec![Effect::Close(id.clone())];
        // An emptied group (never Main's) goes with its last tab.
        if self.groups[gix].tabs.is_empty() && self.multi() {
            self.remove_group(gix);
            effects.push(Effect::Layout);
        }
        effects
    }

    fn remove_group(&mut self, gix: usize) {
        let group = self.groups.remove(gix);
        if self.focused == group.id {
            self.focused = self.groups[gix.saturating_sub(1).min(self.groups.len() - 1)]
                .id
                .clone();
        }
    }

    /// Pink `workbench_close_others`: every tab but this one and Main.
    fn close_others(&mut self, group: &SharedString, keep: &SharedString) -> Vec<Effect> {
        let Some(g) = self.groups.iter().find(|g| &g.id == group) else {
            return Vec::new();
        };
        let ids: Vec<SharedString> = g
            .tabs
            .iter()
            .filter(|t| &t.id != keep && t.role != TabRole::Main)
            .map(|t| t.id.clone())
            .collect();
        let mut effects = Vec::new();
        for id in ids {
            effects.extend(self.close_tab(&id));
        }
        self.select(keep);
        effects
    }

    /// `+`: a fresh side chat in the group, shown at once.
    fn new_tab(&mut self, group: &SharedString) -> Vec<Effect> {
        let id = self.fresh_id("side");
        let source = self
            .groups
            .iter()
            .flat_map(|g| &g.tabs)
            .find(|t| t.role == TabRole::Main)
            .map(|t| t.title.clone());
        let Some(g) = self.groups.iter_mut().find(|g| &g.id == group) else {
            return Vec::new();
        };
        let mut tab = Tab::new(id.clone(), "New side chat").status(TabStatus::Working);
        tab.source = source;
        g.tabs.push(tab);
        g.active = Some(id.clone());
        self.focused = group.clone();
        // Scroll the new tab into view.
        if let Some(scroll) = self.scrolls.get(group) {
            scroll.scroll_to_item(g.tabs.len() - 1);
        }
        vec![Effect::New(group.clone(), id)]
    }

    fn close_group(&mut self, group: &SharedString) -> Vec<Effect> {
        let Some(gix) = self.groups.iter().position(|g| &g.id == group) else {
            return Vec::new();
        };
        if !self.multi() || self.groups[gix].holds_main() {
            return Vec::new();
        }
        let group = self.groups[gix].clone();
        for (ix, tab) in group.tabs.into_iter().enumerate() {
            self.closed.push((group.id.clone(), ix, tab));
        }
        self.remove_group(gix);
        vec![Effect::Layout]
    }

    /// Moves a tab into a new group beside its own (Pink
    /// `workbench_move_to_split`).
    fn move_to_split(&mut self, tab: &SharedString, edge: Edge) -> Vec<Effect> {
        let Some(gix) = self.group_of(tab) else {
            return Vec::new();
        };
        if self.groups[gix].tabs.len() == 1 {
            return Vec::new();
        }
        let ix = self.groups[gix]
            .tabs
            .iter()
            .position(|t| &t.id == tab)
            .unwrap_or(0);
        let moved = self.groups[gix].tabs.remove(ix);
        let source = &mut self.groups[gix];
        if source.active.as_ref() == Some(tab) {
            source.active = source
                .tabs
                .get(ix)
                .or_else(|| ix.checked_sub(1).and_then(|i| source.tabs.get(i)))
                .map(|t| t.id.clone());
        }
        let id = self.fresh_id("group");
        let group = TabGroup::new(id.clone()).tab(moved).active(tab.clone());
        let at = if edge == Edge::Right { gix + 1 } else { gix };
        self.groups.insert(at, group);
        self.focused = id;
        vec![Effect::Layout, Effect::Select(tab.clone())]
    }

    /// A new side chat in a new group to the right (Pink
    /// `workbench_new_child_in_split`).
    fn new_in_split(&mut self, group: &SharedString) -> Vec<Effect> {
        let Some(gix) = self.groups.iter().position(|g| &g.id == group) else {
            return Vec::new();
        };
        let id = self.fresh_id("group");
        self.groups.insert(gix + 1, TabGroup::new(id.clone()));
        let mut effects = self.new_tab(&id);
        effects.push(Effect::Layout);
        effects
    }

    fn balance(&mut self) -> Vec<Effect> {
        for group in &mut self.groups {
            group.share = 1.0;
        }
        vec![Effect::Layout]
    }

    /// Pink `workbench_reopen`: the newest closed tab, back where it was.
    fn reopen(&mut self) -> Vec<Effect> {
        let Some((group, ix, tab)) = self.closed.pop() else {
            return Vec::new();
        };
        let id = tab.id.clone();
        let gix = match self.groups.iter().position(|g| g.id == group) {
            Some(gix) => gix,
            None => {
                self.groups.push(TabGroup::new(group));
                self.groups.len() - 1
            }
        };
        let g = &mut self.groups[gix];
        let at = ix.min(g.tabs.len());
        g.tabs.insert(at, tab);
        let mut effects = self.select(&id);
        effects.push(Effect::Layout);
        effects
    }

    /// Moves the dragged tab into its slot (Pink `reorder` / a strip drop).
    fn drop_drag(&mut self) -> Vec<Effect> {
        let Some(drag) = self.drag.take() else {
            return Vec::new();
        };
        let Some(sgix) = self.groups.iter().position(|g| g.id == drag.group) else {
            return Vec::new();
        };
        let Some(tgix) = self.groups.iter().position(|g| g.id == drag.target) else {
            return Vec::new();
        };
        if drag.from >= self.groups[sgix].tabs.len() {
            return Vec::new();
        }
        if sgix == tgix {
            if drag.from == drag.over {
                return Vec::new();
            }
            let group = &mut self.groups[sgix];
            let tab = group.tabs.remove(drag.from);
            let at = drag.over.min(group.tabs.len());
            group.tabs.insert(at, tab);
            return vec![Effect::Layout];
        }
        // Into another group: it leaves its own (an emptied group goes) and
        // shows in the new one.
        let tab = self.groups[sgix].tabs.remove(drag.from);
        let id = tab.id.clone();
        let source = &mut self.groups[sgix];
        if source.active.as_ref() == Some(&id) {
            source.active = source
                .tabs
                .get(drag.from)
                .or_else(|| drag.from.checked_sub(1).and_then(|i| source.tabs.get(i)))
                .map(|t| t.id.clone());
        }
        let target = &mut self.groups[tgix];
        let at = drag.over.min(target.tabs.len());
        target.tabs.insert(at, tab);
        if self.groups[sgix].tabs.is_empty() {
            self.remove_group(sgix);
        }
        let mut effects = self.select(&id);
        effects.push(Effect::Layout);
        effects
    }

    /// Where a pointer over `group`'s strip would drop: the slot whose box
    /// it is in (Pink: terminal drawer `drop_index`, by box).
    fn slot_at(&self, group: &SharedString, x: Pixels) -> usize {
        let Some(g) = self.groups.iter().find(|g| &g.id == group) else {
            return 0;
        };
        let mut boxes: Vec<Bounds<Pixels>> = g
            .tabs
            .iter()
            .filter_map(|t| self.bounds.get(&t.id).copied())
            .collect();
        boxes.sort_by(|a, b| a.left().partial_cmp(&b.left()).unwrap());
        let same = self.drag.as_ref().is_some_and(|d| &d.group == group);
        let count = boxes.len();
        let ix = boxes.iter().position(|b| x < b.right()).unwrap_or(count);
        if same {
            ix.min(count.saturating_sub(1))
        } else {
            ix
        }
    }

    // -- The menu -------------------------------------------------------------

    fn open_menu(&mut self, menu: Menu) {
        self.menu = Some(menu);
        self.menu_closing = None;
        self.menu_reap = None;
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else {
            return;
        };
        let since = Instant::now();
        self.menu_closing = Some((menu, since));
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        self.menu_reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                if s.menu_closing.as_ref().is_some_and(|(_, at)| *at == since) {
                    s.menu_closing = None;
                    cx.notify();
                }
            })
            .ok();
        }));
    }
}

/// Runs a state change from a listener, then tells the host.
fn run(
    state: &Entity<BarState>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut BarState, &mut Context<BarState>) -> Vec<Effect>,
) {
    let (effects, handlers, groups) = state.update(cx, |s, cx| {
        s.autoplay = false;
        let effects = f(s, cx);
        cx.notify();
        (effects, s.handlers.clone(), s.groups.clone())
    });
    for effect in effects {
        match effect {
            Effect::Select(id) => {
                if let Some(f) = &handlers.select {
                    f(&id, window, cx)
                }
            }
            Effect::Close(id) => {
                if let Some(f) = &handlers.close {
                    f(&id, window, cx)
                }
            }
            Effect::New(group, id) => {
                if let Some(f) = &handlers.new {
                    f(&group, &id, window, cx)
                }
            }
            Effect::Layout => {
                if let Some(f) = &handlers.layout {
                    f(&groups, window, cx)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tab colors (Pink: theme.rs tab helpers)
// ---------------------------------------------------------------------------

/// Pink: workbench/render.rs `tab_selected_fill` – the focused group's
/// active tab a quiet accent tint, the others the neutral selected wash.
pub(crate) fn selected_fill(t: &Pink, focused: bool) -> Hsla {
    if focused {
        t.tab_focused_fill()
    } else {
        t.tab_chip_washes()[2]
    }
}

/// Pink: theme.rs `tab_title` – muted at rest; full text, floored on its
/// own fill, on the active tab.
pub(crate) fn tab_title_color(t: &Pink, active: bool, focused: bool) -> Hsla {
    if !active {
        return t.text_muted;
    }
    t.readable_on_stacks(
        t.text,
        &[&[selected_fill(t, focused)]],
        pink::TEXT_CONTRAST_TARGET,
    )
}

/// Pink: theme.rs `tab_role_icon` – the accent on the focused group's
/// active tab, else quiet.
pub(crate) fn role_icon_color(t: &Pink, focused_active: bool) -> Hsla {
    if focused_active {
        t.readable_on_stacks(
            t.accent,
            &[&[t.tab_focused_fill()]],
            pink::NON_TEXT_CONTRAST_TARGET,
        )
    } else {
        t.readable_muted(0.6)
    }
}

/// Pink: theme.rs `tab_unread_dot` – the accent floored on every chip fill.
pub(crate) fn unread_dot(t: &Pink) -> Hsla {
    let [rest, hover, active] = t.tab_chip_washes();
    t.readable_on_stacks(
        t.accent,
        &[&[rest], &[hover], &[active], &[t.tab_focused_fill()]],
        pink::NON_TEXT_CONTRAST_TARGET,
    )
}

/// Pink: theme.rs `readable_on_tab_badge` – the Main badge's accent floored
/// on its wash over every chip fill.
pub(crate) fn on_tab_badge(t: &Pink, min: f32) -> Hsla {
    let [rest, hover, active] = t.tab_chip_washes();
    let badge = t.accent_wash;
    t.readable_on_stacks(
        t.accent,
        &[
            &[rest, badge],
            &[hover, badge],
            &[active, badge],
            &[t.tab_focused_fill(), badge],
        ],
        min,
    )
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

fn eid(id: String) -> ElementId {
    ElementId::Name(id.into())
}

/// A tab's tooltip (Pink: pickers.rs `ControlTooltip`): a medium title over
/// 11 pt detail lines on the tooltip card.
struct TabTooltip {
    title: SharedString,
    detail: Option<SharedString>,
}

impl Render for TabTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let wraps = self
            .detail
            .as_ref()
            .is_some_and(|d| d.chars().count() > layout::TOOLTIP_WRAP_CHARS);
        pink::tooltip_card(&t)
            .when(wraps, |card| card.w(px(layout::TOOLTIP_MAX_WIDTH)))
            .when(!wraps, |card| card.whitespace_nowrap())
            .px(px(10.0))
            .py(px(7.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .text_size(px(12.0))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(self.title.clone()),
            )
            .when_some(self.detail.clone(), |card, detail| {
                card.child(
                    div()
                        .text_size(px(11.0))
                        .text_color(t.text_muted)
                        .child(detail),
                )
            })
    }
}

fn tooltip(
    title: impl Into<SharedString>,
    detail: Option<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let title = title.into();
    move |_, cx| {
        cx.new(|_| TabTooltip {
            title: title.clone(),
            detail: detail.clone(),
        })
        .into()
    }
}

/// The chip under the pointer while a tab drags (Pink: workbench/render.rs
/// `WorkbenchTabGhost`): a raised 26 pt chip with the mark, role, title and
/// the role's name.
struct TabGhost {
    title: SharedString,
    role: TabRole,
    harness: Option<Harness>,
}

impl Render for TabGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        div()
            .h(px(26.0))
            .max_w(px(TAB_MAX))
            .px(px(8.0))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .rounded(px(7.0))
            .bg(t.surface_raised)
            .border_1()
            .border_color(t.border_strong)
            .shadow(pink::shadow_md())
            .font_family(pink::SANS)
            .text_size(px(12.0))
            .text_color(t.text)
            .when_some(self.harness, |el, h| {
                el.child(icon(h.mark(), 13.0, h.tint().unwrap_or(t.text_muted)))
            })
            .child(icon(self.role.icon(), 11.0, t.text_muted))
            .child(div().min_w_0().truncate().child(self.title.clone()))
            .child(
                div()
                    .flex_none()
                    .text_size(px(11.0))
                    .text_color(t.text_muted)
                    .child(self.role.display()),
            )
    }
}

/// What every part of one render shares.
struct Cx<'a> {
    key: &'a str,
    t: &'a Pink,
    state: &'a Entity<BarState>,
    s: &'a BarState,
    /// The band the strips sit on.
    band: Hsla,
}

/// One chip (Pink: workbench/render.rs `render_workbench_tab`): 28 tall,
/// `pl 8 pr 4` (Main `pr 8`), radius 8, gap 6, 12.5 medium for every tab;
/// the mark, the Main badge or role glyph, the title, the status and the
/// close button.
fn tab_chip(
    c: &Cx,
    group: &TabGroup,
    ix: usize,
    tab: &Tab,
    strip_focused: bool,
    max_width: f32,
    focus: Option<&FocusHandle>,
) -> AnyElement {
    let t = c.t;
    let active = group.active.as_ref() == Some(&tab.id);
    let is_main = tab.role == TabRole::Main;
    let chip_id = format!("{}-tab-{}", c.key, tab.id);
    let hover_group: SharedString = chip_id.clone().into();
    let focused_active = active && strip_focused;
    let ring = t.focus_ring();
    let min_w = if max_width < TAB_MIN {
        TAB_MIN_COMPACT
    } else {
        TAB_MIN
    };

    // Each status has its own shape, never colour alone, at the non-text
    // contrast floor.
    let status = match tab.status {
        TabStatus::Working => Some(
            loaders::mini_glyph_spinner(format!("{chip_id}-working"), 2.0, t.glyph)
                .into_any_element(),
        ),
        TabStatus::NeedsApproval => Some(
            icon(
                ICON_SHIELD,
                tab_chip::STATUS_GLYPH,
                t.readable_icon(t.warning),
            )
            .into_any_element(),
        ),
        TabStatus::NeedsInput => Some(
            icon(
                ICON_CHAT,
                tab_chip::STATUS_GLYPH,
                t.readable_icon(t.warning),
            )
            .into_any_element(),
        ),
        TabStatus::Failed => Some(
            icon(
                ICON_TRIANGLE,
                tab_chip::STATUS_GLYPH,
                t.readable_icon(t.danger),
            )
            .into_any_element(),
        ),
        // Finished while you were elsewhere: a small accent dot, gone once
        // it is seen.
        TabStatus::Unread => Some(tab_chip::unread_dot(unread_dot(t)).into_any_element()),
        TabStatus::Idle => None,
    };
    let main_badge = is_main.then(|| {
        tab_chip::badge(&tab_chip::BadgeStyle {
            fill: t.accent_wash,
            text: on_tab_badge(t, pink::TEXT_CONTRAST_TARGET),
        })
        .child(icon(
            ICON_HOME,
            tab_chip::BADGE_GLYPH,
            on_tab_badge(t, pink::NON_TEXT_CONTRAST_TARGET),
        ))
        .child("Main")
    });
    let close = (!is_main).then(|| {
        let close_id = format!("{chip_id}-close");
        let state = c.state.clone();
        let id = tab.id.clone();
        tab_chip::close_slot(hover_group.clone(), active)
            .id(eid(close_id.clone()))
            .role(Role::Button)
            .aria_label(format!("Close tab: {}", tab.title))
            .hover_fill(
                close_id,
                gpui::transparent_black(),
                t.fill(FillTier::Secondary),
            )
            // The tab owns a drag payload: claim the close press first.
            .on_mouse_down(MouseButton::Left, |_, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                run(&state, window, cx, |s, _| s.close_tab(&id));
            })
            .child(icon(ICON_CLOSE, tab_chip::CLOSE_GLYPH, t.text_muted))
    });

    let (heading, detail) = tab.help();
    let accessible = format!(
        "{}{}: {}",
        tab.role.display(),
        match tab.status {
            TabStatus::Working => ", running",
            TabStatus::NeedsApproval => ", needs approval",
            TabStatus::NeedsInput => ", needs input",
            TabStatus::Failed => ", failed",
            TabStatus::Unread => ", unread",
            TabStatus::Idle => "",
        },
        tab.title
    );

    // While a tab of this group drags, its neighbours slide aside over
    // TAB_SLIDE and the dragged tab leaves its slot empty (the ghost
    // carries it). Each drag keys its own followers, so it starts at rest.
    let (slide, hidden) = match c.s.drag.as_ref().filter(|d| d.group == group.id) {
        Some(drag) if ix == drag.from => (0.0, true),
        Some(drag) => {
            let over = if drag.target == group.id {
                drag.over
            } else {
                // Dragged out to another strip: the gap closes.
                group.tabs.len().saturating_sub(1)
            };
            let units = slide_offset(ix, drag.from, over);
            let target = units * (drag.width + TAB_GAP);
            (
                motion::follow(
                    &format!("{chip_id}-slide-{}", drag.session),
                    target,
                    TAB_SLIDE,
                ),
                false,
            )
        }
        None => (0.0, false),
    };

    let drag = TabDrag {
        bar: c.key.to_owned().into(),
        group: group.id.clone(),
        id: tab.id.clone(),
        title: tab.title.clone(),
        role: tab.role,
        harness: tab.harness,
    };
    let click_state = c.state.clone();
    let middle_state = c.state.clone();
    let menu_state = c.state.clone();
    let key_state = c.state.clone();
    let click_id = tab.id.clone();
    let middle_id = tab.id.clone();
    let menu_tab = tab.id.clone();
    let menu_group = group.id.clone();
    let key_id = tab.id.clone();
    // The package's chip (`pink-tab-chip`): one weight for every tab, so
    // activation reads through the fill and the text colour and the strip
    // never reflows; never wider than the strip, so the title truncates and
    // the close control stays inside.
    let chip = tab_chip::tab(&tab_chip::TabStyle {
        label: tab_chip::Label::Tab,
        text: tab_title_color(t, active, strip_focused),
        fill: active.then(|| selected_fill(t, strip_focused)),
        closable: !is_main,
        min_width: min_w,
        max_width,
    })
    .id(eid(chip_id.clone()))
    .group(hover_group)
    .role(Role::Tab)
    .aria_label(accessible)
    .aria_selected(active)
    .when_some(focus, |el, focus| el.track_focus(focus))
    .relative()
    .left(px(slide))
    .focus_visible(move |st| pink::focus_ring(st, ring))
    .when(!active, |el| {
        el.hover_wash(format!("{chip_id}-wash"), t.fill(FillTier::Tertiary))
    })
    .when(hidden, |el| el.opacity(0.0))
    .tooltip(tooltip(heading, Some(detail)))
    .tooltip_show_delay(layout::TOOLTIP_DELAY)
    .on_click(move |_, window, cx| {
        run(&click_state, window, cx, |s, _| s.select(&click_id));
    })
    .on_key_down(pink::swallow_activation_key)
    // Arrow keys move between this group's tabs; selection follows.
    .on_key_down(move |event: &KeyDownEvent, window, cx| {
        let mods = &event.keystroke.modifiers;
        if mods.platform || mods.control || mods.alt || mods.shift {
            return;
        }
        let target = {
            let s = key_state.read(cx);
            let Some(g) = s.group_of(&key_id).map(|g| &s.groups[g]) else {
                return;
            };
            let ids: Vec<SharedString> = g.tabs.iter().map(|t| t.id.clone()).collect();
            let Some(at) = ids.iter().position(|id| id == &key_id) else {
                return;
            };
            let last = ids.len() - 1;
            let next = match event.keystroke.key.as_str() {
                "left" => at.checked_sub(1).unwrap_or(last),
                "right" => (at + 1) % ids.len(),
                "home" => 0,
                "end" => last,
                _ => return,
            };
            ids[next].clone()
        };
        cx.stop_propagation();
        let focus = key_state.read(cx).focus.get(&target).cloned();
        run(&key_state, window, cx, |s, _| s.select(&target));
        if let Some(focus) = focus {
            window.focus(&focus, cx);
        }
    })
    // Middle-click closes, like every tab strip.
    .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
        run(&middle_state, window, cx, |s, _| s.close_tab(&middle_id));
    })
    .on_mouse_down(
        MouseButton::Right,
        move |event: &MouseDownEvent, window, cx| {
            cx.stop_propagation();
            let position = event.position;
            run(&menu_state, window, cx, |s, _| {
                s.open_menu(Menu {
                    group: menu_group.clone(),
                    tab: Some(menu_tab.clone()),
                    group_actions: false,
                    position,
                });
                Vec::new()
            });
        },
    )
    .on_drag(drag, |payload, _, _, cx| {
        cx.stop_propagation();
        let payload = payload.clone();
        cx.new(|_| TabGhost {
            title: payload.title,
            role: payload.role,
            harness: payload.harness,
        })
    })
    .when_some(tab.harness, |el, h| {
        el.child(icon(
            h.mark(),
            tab_chip::HARNESS_MARK,
            h.tint().unwrap_or(t.text_muted),
        ))
    })
    .children(main_badge)
    .when(!is_main, |el| {
        el.child(icon(
            tab.role.icon(),
            tab_chip::ROLE_GLYPH,
            role_icon_color(t, focused_active),
        ))
    })
    .child(tab_chip::title(tab.title.clone()))
    .children(status)
    .children(close);
    // A new tab fades in where it lands (Pink `INSERT_MS`).
    let chip = chip.with_animation(eid(format!("{chip_id}-in")), INSERT.animation(), |el, p| {
        el.opacity(p)
    });
    // The slot, measured where layout put it (the slide is paint-only).
    let measure_state = c.state.clone();
    let measure_id = tab.id.clone();
    div()
        .relative()
        .flex_none()
        .child(chip)
        .child(
            canvas(
                move |bounds, _, cx| {
                    measure_state.update(cx, |s, _| {
                        s.bounds.insert(measure_id.clone(), bounds);
                    })
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        .into_any_element()
}

/// Pink: terminal/panel.rs `slide_offset` – tabs between the dragged one
/// and its slot shift one place toward the gap it left.
fn slide_offset(ix: usize, from: usize, over: usize) -> f32 {
    if from < over && ix > from && ix <= over {
        -1.0
    } else if over < from && ix >= over && ix < from {
        1.0
    } else {
        0.0
    }
}

/// A closed tab narrowing and fading where it was (Pink:
/// `render_workbench_strip` leaving tabs).
fn leaving_ghost(leaving: &Leaving, t: &Pink) -> AnyElement {
    let elapsed = leaving.started.elapsed().as_secs_f32() * 1000.0;
    let p = (elapsed / (TAB_EXIT_MS * motion::speed_scale())).clamp(0.0, 1.0);
    let eased = motion::EASE_OUT.eval(p);
    div()
        .h(px(TAB_H))
        .w(px(leaving.width * (1.0 - eased)))
        .flex_none()
        .overflow_hidden()
        .opacity(1.0 - eased)
        .rounded(px(7.0))
        .pl(px(8.0))
        .flex()
        .items_center()
        .text_size(px(12.5))
        .text_color(t.text_muted)
        .child(
            div()
                .flex_none()
                .w(px(leaving.width))
                .truncate()
                .child(leaving.title.clone()),
        )
        .into_any_element()
}

/// A small icon button for the strip's trailing controls (Pink:
/// workbench/render.rs `strip_button`): 28 square, radius 6, a quiet wash
/// under the pointer, a 16 glyph.
fn strip_button(
    id: String,
    label: &'static str,
    detail: Option<&'static str>,
    glyph: &'static str,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    div()
        .id(eid(id.clone()))
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .relative()
        .size(px(chrome::CONTROL_M))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_M))
        .cursor_pointer()
        .hover_wash(id, t.fill(FillTier::Tertiary))
        .focus_visible(move |st| pink::focus_ring(st, ring))
        .tooltip(tooltip(label, detail.map(SharedString::from)))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_key_down(pink::swallow_activation_key)
        .child(icon(glyph, chrome::ICON_M, t.text_muted))
}

/// Pink: tabs.rs `render_title_crumb` – the project in 13 semibold and the
/// "›" the tabs follow.
fn crumb(project: &SharedString, t: &Pink) -> AnyElement {
    div()
        .flex_none()
        .min_w_0()
        .max_w(px(420.0))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .mr(px(2.0))
        .child(
            div()
                .min_w_0()
                .max_w(px(200.0))
                .truncate()
                .text_size(px(13.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text.opacity(0.9))
                .child(project.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(13.0))
                .text_color(t.decorative_muted(0.4))
                .child("›"),
        )
        .into_any_element()
}

/// One group's strip (Pink: workbench/render.rs `render_workbench_strip`):
/// its tabs in a scrolling viewport faded at the edges, `+`, Close group,
/// or – when narrow – one Group actions button.
fn strip(c: &Cx, group: &TabGroup, width: f32, lead: Option<AnyElement>) -> AnyElement {
    let t = c.t;
    let s = c.s;
    let compact = width < STRIP_COMPACT;
    let strip_focused = !s.multi() || s.focused == group.id;
    let tab_min = if compact { TAB_MIN_COMPACT } else { TAB_MIN };
    // `+` and its gap follow the tabs in a wide strip.
    let reserved = if compact {
        0.0
    } else {
        chrome::CONTROL_M + 4.0
    };
    // A hair under the room: tab gaps and rounding never overflow it.
    let cap = s
        .slots
        .get(&group.id)
        .map_or(TAB_MAX, |room| {
            (room - reserved - 4.0).clamp(tab_min, TAB_MAX)
        })
        .floor();
    let gap = if compact { 2.0 } else { 4.0 };
    let scroll = s.scrolls.get(&group.id).cloned().unwrap_or_default();

    let mut tabs: Vec<AnyElement> = group
        .tabs
        .iter()
        .enumerate()
        .map(|(ix, tab)| tab_chip(c, group, ix, tab, strip_focused, cap, s.focus.get(&tab.id)))
        .collect();
    // Closed tabs narrow and fade where they were.
    for leaving in s.leaving.iter().filter(|l| l.group == group.id).rev() {
        let at = leaving.index.min(tabs.len());
        tabs.insert(at, leaving_ghost(leaving, t));
    }

    // Edge fades on whichever side hides tabs (opaque: painted gradients in
    // the band's tone).
    let offset = -f32::from(scroll.offset().x);
    let max = f32::from(scroll.max_offset().x);
    let fade_left = offset > 0.5;
    let fade_right = max - offset > 0.5;
    let band = c.band;

    // A cross-group drop shows Pink's insertion marker at its slot.
    let marker = s
        .drag
        .as_ref()
        .filter(|d| d.target == group.id && d.group != group.id)
        .and_then(|d| {
            let strip_left = s.strips.get(&group.id)?.left();
            let mut boxes: Vec<Bounds<Pixels>> = group
                .tabs
                .iter()
                .filter_map(|tab| s.bounds.get(&tab.id).copied())
                .collect();
            boxes.sort_by(|a, b| a.left().partial_cmp(&b.left()).unwrap());
            let x = match boxes.get(d.over) {
                Some(b) => b.left() - px(2.0),
                None => boxes
                    .last()
                    .map(|b| b.right() + px(1.0))
                    .unwrap_or(strip_left + px(8.0)),
            };
            Some(f32::from(x - strip_left))
        });

    let new_state = c.state.clone();
    let new_group = group.id.clone();
    let plus = strip_button(
        format!("{}-new-{}", c.key, group.id),
        "Side chat…",
        Some("A side chat from this group's chat. Nothing is created until you choose."),
        ICON_PLUS,
        t,
    )
    .on_click(move |_, window, cx| {
        cx.stop_propagation();
        run(&new_state, window, cx, |s, _| s.new_tab(&new_group));
    });
    let close_group = (s.multi() && !group.holds_main()).then(|| {
        let state = c.state.clone();
        let id = group.id.clone();
        strip_button(
            format!("{}-close-group-{}", c.key, group.id),
            "Close group",
            Some("Hides its chats; reopen them from the tab menu"),
            ICON_CLOSE,
            t,
        )
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            run(&state, window, cx, |s, _| s.close_group(&id));
        })
    });
    let more = {
        let state = c.state.clone();
        let id = group.id.clone();
        let active = group.active.clone();
        strip_button(
            format!("{}-more-{}", c.key, group.id),
            "Group actions",
            Some("Side chat, Side chat in a split, Close group, and this tab's actions"),
            ICON_MORE,
            t,
        )
        .on_click(move |event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            let position = event.position();
            run(&state, window, cx, |s, _| {
                s.open_menu(Menu {
                    group: id.clone(),
                    tab: active.clone(),
                    group_actions: true,
                    position,
                });
                Vec::new()
            });
        })
    };

    let has_lead = lead.is_some();
    let focus_state = c.state.clone();
    let focus_group = group.id.clone();
    let move_state = c.state.clone();
    let move_group = group.id.clone();
    let drop_state = c.state.clone();
    let strip_state = c.state.clone();
    let strip_group = group.id.clone();
    let slot_state = c.state.clone();
    let slot_group = group.id.clone();
    let bar_key: SharedString = c.key.to_owned().into();
    let drop_key = bar_key.clone();
    div()
        .id(eid(format!("{}-strip-{}", c.key, group.id)))
        .relative()
        .h(px(STRIP_HEIGHT))
        .flex_none()
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(gap))
        .pl(px(if has_lead {
            2.0
        } else if compact {
            2.0
        } else {
            4.0
        }))
        .pr(px(if compact { 2.0 } else { 4.0 }))
        // A press on the strip focuses its group (controls still get their
        // own press).
        .capture_any_mouse_down(move |_, _, cx| {
            focus_state.update(cx, |s, cx| {
                if s.focused != focus_group {
                    s.focused = focus_group.clone();
                    cx.notify();
                }
            })
        })
        .on_drag_move::<TabDrag>(move |event: &DragMoveEvent<TabDrag>, _, cx| {
            if !event.bounds.contains(&event.event.position) {
                return;
            }
            let drag = event.drag(cx).clone();
            if drag.bar != bar_key {
                return;
            }
            let x = event.event.position.x;
            move_state.update(cx, |s, cx| {
                let over = s.slot_at(&move_group, x);
                let width = s
                    .bounds
                    .get(&drag.id)
                    .map(|b| f32::from(b.size.width))
                    .unwrap_or(TAB_MIN);
                let from = s
                    .groups
                    .iter()
                    .find(|g| g.id == drag.group)
                    .and_then(|g| g.tabs.iter().position(|t| t.id == drag.id));
                let Some(from) = from else {
                    return;
                };
                let session = match &s.drag {
                    Some(d) if d.group == drag.group && d.from == from => d.session,
                    _ => {
                        s.drag_sessions += 1;
                        s.drag_sessions
                    }
                };
                let next = DragState {
                    group: drag.group.clone(),
                    from,
                    target: move_group.clone(),
                    over,
                    width,
                    session,
                };
                let changed = s.drag.as_ref().is_none_or(|d| {
                    d.target != next.target || d.over != next.over || d.session != next.session
                });
                s.drag = Some(next);
                s.autoplay = false;
                if changed {
                    cx.notify();
                }
            });
        })
        .on_drop::<TabDrag>(move |drag: &TabDrag, window, cx| {
            if drag.bar != drop_key {
                return;
            }
            run(&drop_state, window, cx, |s, _| s.drop_drag());
        })
        // The strip, for the drop marker (window coordinates).
        .child(
            canvas(
                move |bounds, _, cx| {
                    strip_state.update(cx, |s, _| {
                        s.strips.insert(strip_group.clone(), bounds);
                    })
                },
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        )
        // A narrow strip gives the crumb's room to its tabs.
        .when(!compact, |el| el.children(lead))
        .child(
            // The strip's room for its tabs and `+`, measured: it bounds one
            // tab's width.
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(gap))
                .child(
                    canvas(
                        move |bounds, _, cx| {
                            let width = f32::from(bounds.size.width);
                            slot_state.update(cx, |s, cx| {
                                let old = s.slots.insert(slot_group.clone(), width);
                                if old.is_none_or(|w| (w - width).abs() > 0.5) {
                                    cx.notify();
                                }
                            })
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .inset_0(),
                )
                .child(
                    // The visible tab viewport: as wide as its tabs, so `+`
                    // follows the last one, shrinking (tabs scroll) once
                    // they fill the strip.
                    div()
                        .min_w_0()
                        .h_full()
                        .relative()
                        .child(
                            div()
                                .id(eid(format!("{}-tabs-{}", c.key, group.id)))
                                .role(Role::TabList)
                                .aria_label("Chat tabs")
                                .size_full()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(TAB_GAP))
                                .overflow_x_scroll()
                                .track_scroll(&scroll)
                                .on_scroll_wheel({
                                    let state = c.state.clone();
                                    move |_, _, cx| state.update(cx, |_, cx| cx.notify())
                                })
                                .children(tabs),
                        )
                        // Overflowing tabs dissolve at the strip's edges
                        // instead of being cut mid-glyph.
                        .when(fade_left, |el| {
                            el.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .left_0()
                                    .w(px(EDGE_FADE))
                                    .bg(linear_gradient(
                                        90.0,
                                        linear_color_stop(band, 0.0),
                                        linear_color_stop(band.opacity(0.0), 1.0),
                                    )),
                            )
                        })
                        .when(fade_right, |el| {
                            el.child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .bottom_0()
                                    .right_0()
                                    .w(px(EDGE_FADE))
                                    .bg(linear_gradient(
                                        90.0,
                                        linear_color_stop(band.opacity(0.0), 0.0),
                                        linear_color_stop(band, 1.0),
                                    )),
                            )
                        }),
                )
                .when(!compact, |el| el.child(plus)),
        )
        .when(!compact, |el| el.children(close_group))
        .when(compact, |el| el.child(more))
        .when_some(marker, |el, x| {
            el.child(
                div()
                    .absolute()
                    .top(px((STRIP_HEIGHT - TAB_H) / 2.0 - 1.0))
                    .left(px(x))
                    .w(px(2.0))
                    .h(px(TAB_H + 2.0))
                    .rounded_full()
                    .bg(t.accent),
            )
        })
        .into_any_element()
}

/// The tab menu (Pink: workbench/render.rs `render_workbench_menu`): the
/// group's actions when opened from Group actions, then the tab's: close,
/// close others, move to a split, balance, reopen.
fn menu_card(c: &Cx, menu: &Menu) -> AnyElement {
    let t = c.t;
    let s = c.s;
    let Some(group) = s.groups.iter().find(|g| g.id == menu.group) else {
        return div().into_any_element();
    };
    let tab = menu
        .tab
        .as_ref()
        .and_then(|id| group.tabs.iter().find(|tab| &tab.id == id));
    let is_main = tab.is_some_and(|tab| tab.role == TabRole::Main);
    let others = group.tabs.len() > 1;
    let row = |id: &str,
               glyph: &'static str,
               text: &'static str,
               action: Rc<dyn Fn(&mut BarState) -> Vec<Effect>>| {
        let state = c.state.clone();
        pink::menu_row(format!("{}-menu-{id}", c.key), false, t)
            .role(Role::MenuItem)
            .aria_label(text)
            .on_click(move |_, window, cx| {
                let action = action.clone();
                run(&state, window, cx, move |s, cx| {
                    s.close_menu(cx);
                    action(s)
                });
            })
            .child(icon(glyph, 16.0, t.text_muted))
            .child(text)
    };
    let gid = group.id.clone();
    let mut rows: Vec<AnyElement> = Vec::new();
    if menu.group_actions {
        let g = gid.clone();
        rows.push(
            row(
                "new",
                ICON_PLUS,
                "Side chat…",
                Rc::new(move |s| s.new_tab(&g)),
            )
            .into_any_element(),
        );
        let g = gid.clone();
        rows.push(
            row(
                "new-split",
                ICON_COLUMNS,
                "Side chat in a split",
                Rc::new(move |s| s.new_in_split(&g)),
            )
            .into_any_element(),
        );
        if s.multi() && !group.holds_main() {
            let g = gid.clone();
            rows.push(
                row(
                    "close-group",
                    ICON_CLOSE,
                    "Close group",
                    Rc::new(move |s| s.close_group(&g)),
                )
                .into_any_element(),
            );
        }
        if tab.is_some() {
            rows.push(pink::menu_separator(t).into_any_element());
        }
    }
    if let Some(tab) = tab {
        let id = tab.id.clone();
        if !is_main {
            let id = id.clone();
            rows.push(
                row(
                    "close",
                    ICON_CLOSE,
                    "Close tab",
                    Rc::new(move |s| s.close_tab(&id)),
                )
                .into_any_element(),
            );
        }
        if others {
            let (g, keep) = (gid.clone(), id.clone());
            rows.push(
                row(
                    "close-others",
                    ICON_CLOSE_CIRCLE,
                    "Close other tabs (keeps Main)",
                    Rc::new(move |s| s.close_others(&g, &keep)),
                )
                .into_any_element(),
            );
        }
        rows.push(pink::menu_separator(t).into_any_element());
        for (key, edge, text) in [
            ("split-right", Edge::Right, "Move to split right"),
            ("split-left", Edge::Left, "Move to split left"),
        ] {
            let id = id.clone();
            rows.push(
                row(
                    key,
                    ICON_COLUMNS,
                    text,
                    Rc::new(move |s| s.move_to_split(&id, edge)),
                )
                .into_any_element(),
            );
        }
    }
    if s.multi() {
        rows.push(pink::menu_separator(t).into_any_element());
        rows.push(
            row(
                "balance",
                ICON_WIDGET,
                "Balance panes",
                Rc::new(|s| s.balance()),
            )
            .into_any_element(),
        );
    }
    if !s.closed.is_empty() {
        rows.push(pink::menu_separator(t).into_any_element());
        rows.push(
            row(
                "reopen",
                ICON_RESTART,
                "Reopen closed tab",
                Rc::new(|s| s.reopen()),
            )
            .into_any_element(),
        );
    }
    let close_state = c.state.clone();
    let key_state = c.state.clone();
    pink::popover_card(t)
        .id(eid(format!("{}-menu", c.key)))
        .role(Role::Menu)
        .w(px(MENU_W))
        .font_weight(FontWeight::NORMAL)
        .flex()
        .flex_col()
        .track_focus(&s.menu_focus)
        .on_mouse_down_out(move |_, _, cx| {
            close_state.update(cx, |s, cx| {
                s.close_menu(cx);
                cx.notify();
            })
        })
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
                cx.stop_propagation();
                key_state.update(cx, |s, cx| {
                    s.close_menu(cx);
                    cx.notify();
                });
            }
        })
        .children(rows)
        .into_any_element()
}

/// The menu at the pointer, kept 8 pt inside the window (Pink:
/// popover.rs `menu_at`), easing in on `MENU_IN` and out on `MENU_OUT`.
fn menu_layer(c: &Cx, cx: &App) -> Option<AnyElement> {
    let s = c.s;
    let (menu, exit) = match (&s.menu, &s.menu_closing) {
        (Some(menu), _) => (menu.clone(), None),
        (None, Some((menu, since))) => (
            menu.clone(),
            Some(if motion::reduced_motion(cx) {
                1.0
            } else {
                motion::menu_exit_progress(*since)
            }),
        ),
        (None, None) => return None,
    };
    let card = div().occlude().child(menu_card(c, &menu));
    let layer = match exit {
        Some(p) => motion::menu_out(
            eid(format!("{}-menu-out", c.key)),
            p,
            card.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in(eid(format!("{}-menu-in", c.key)), card).into_any_element(),
    };
    Some(
        deferred(
            anchored()
                .position(menu.position)
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(layer),
        )
        .priority(1)
        .into_any_element(),
    )
}

/// One canvas column: the shown chat's heading and what it carries.
fn canvas_column(group: &TabGroup, t: &Pink) -> AnyElement {
    let tab = group
        .active
        .as_ref()
        .and_then(|id| group.tabs.iter().find(|tab| &tab.id == id));
    div()
        .h_full()
        .min_w_0()
        .overflow_hidden()
        .p(px(16.0))
        .flex()
        .flex_col()
        .gap(px(4.0))
        .when_some(tab, |el, tab| {
            let (heading, detail) = tab.help();
            el.child(
                div()
                    .type_style(TextStyle::Title3)
                    .text_color(t.text)
                    .truncate()
                    .child(tab.title.clone()),
            )
            .child(
                div()
                    .type_role(TypeRole::Metadata)
                    .text_color(t.readable_muted(0.85))
                    .child(match tab.role {
                        TabRole::Side | TabRole::Child => heading,
                        TabRole::Main | TabRole::Alongside => detail,
                    }),
            )
        })
        .into_any_element()
}

impl RenderOnce for TabBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "tab-bar".into()),
            cx,
            |_, cx| BarState::new(&self, cx),
        );
        let handlers = self.handlers.clone();
        // Keep the per-group scrolls and per-tab focus handles; forget
        // finished exits and a drag that ended outside every strip.
        let has_drag = cx.has_active_drag();
        let missing: Vec<SharedString> = {
            let s = state.read(cx);
            s.groups
                .iter()
                .flat_map(|g| &g.tabs)
                .filter(|tab| !s.focus.contains_key(&tab.id))
                .map(|tab| tab.id.clone())
                .collect()
        };
        let handles: Vec<(SharedString, FocusHandle)> = missing
            .into_iter()
            .map(|id| (id, cx.focus_handle().tab_stop(true)))
            .collect();
        let menu_focus_due = state.update(cx, |s, _| {
            s.handlers = handlers;
            s.focus.extend(handles);
            for group in &s.groups {
                s.scrolls.entry(group.id.clone()).or_default();
            }
            let exit = Duration::from_secs_f32(TAB_EXIT_MS / 1000.0 * motion::speed_scale());
            s.leaving.retain(|l| l.started.elapsed() < exit);
            if s.drag.is_some() && !has_drag {
                s.drag = None;
            }
            s.menu.is_some()
        });
        let menu_focus = state.read(cx).menu_focus.clone();
        if menu_focus_due && !menu_focus.is_focused(window) {
            window.focus(&menu_focus, cx);
        }

        let s = state.read(cx);
        let band = t.chrome_surface();
        let c = Cx {
            key: &key,
            t: &t,
            state: &state,
            s,
            band,
        };
        // The groups share the row (and the canvas under it) by their
        // shares, a 1 pt seam between neighbours.
        let inset = CANVAS_INSET + VIEWPORT_INSET;
        let avail =
            (self.width - 2.0 * inset - SPLIT_SEAM * (s.groups.len().max(1) - 1) as f32).max(0.0);
        let total: f32 = s.groups.iter().map(|g| g.share).sum::<f32>().max(0.1);
        let widths: Vec<f32> = s
            .groups
            .iter()
            .map(|g| (avail * g.share / total).floor())
            .collect();
        let groups = s.groups.clone();
        let mut row = div().h(px(STRIP_HEIGHT)).w_full().flex().flex_row();
        for (ix, group) in groups.iter().enumerate() {
            if ix > 0 {
                // The canvas seam carried into the row.
                row = row.child(
                    div()
                        .flex_none()
                        .w(px(SPLIT_SEAM))
                        .h_full()
                        .flex()
                        .items_center()
                        .child(div().w_full().h(px(TAB_H - 8.0)).bg(t.hairline(0.10))),
                );
            }
            let lead = (ix == 0)
                .then(|| self.project.as_ref().map(|p| crumb(p, &t)))
                .flatten();
            row = row.child(
                div()
                    .id(eid(format!("{key}-scope-{}", group.id)))
                    .flex_none()
                    .w(px(widths[ix]))
                    .h_full()
                    .min_w_0()
                    .overflow_hidden()
                    .child(strip(&c, group, widths[ix], lead)),
            );
        }
        let canvas = (self.canvas_height > 0.0).then(|| {
            let mut columns = div().size_full().flex().flex_row();
            for (ix, group) in groups.iter().enumerate() {
                if ix > 0 {
                    columns = columns.child(
                        div()
                            .flex_none()
                            .w(px(SPLIT_SEAM))
                            .h_full()
                            .bg(t.hairline(0.08)),
                    );
                }
                columns = columns.child(
                    div()
                        .flex_none()
                        .w(px(widths[ix]))
                        .h_full()
                        .child(canvas_column(group, &t)),
                );
            }
            div().px(px(CANVAS_INSET)).pb(px(CANVAS_INSET)).child(
                div()
                    .h(px(self.canvas_height))
                    .rounded(px(CANVAS_RADIUS))
                    .border_1()
                    .border_color(t.hairline(0.08))
                    .bg(t.canvas_fill())
                    .overflow_hidden()
                    .p(px(VIEWPORT_INSET - 1.0))
                    .child(columns),
            )
        });
        let menu = menu_layer(&c, cx);
        // A closing tab narrows frame by frame.
        if !s.leaving.is_empty() {
            window.request_animation_frame();
        }
        motion::keep_alive(window);
        // The titlebar band on the shell surface: the row sits 4 pt down,
        // its strips over their canvas columns.
        div()
            .w(px(self.width))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .bg(band)
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(
                div()
                    .h(px(layout::TITLEBAR_HEIGHT))
                    .pt(px(layout::TITLEBAR_TOP_PAD))
                    .px(px(inset))
                    .child(row),
            )
            .children(canvas)
            .children(menu)
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        TabBar::new("tab-bar")
            .width(900.0)
            .canvas(300.0)
            .project("billing-api")
            .group(
                TabGroup::new("left")
                    .share(1.0)
                    .tab(
                        Tab::new("coupons", "Retire expired coupon codes")
                            .role(TabRole::Main)
                            .status(TabStatus::Working),
                    )
                    .tab(
                        Tab::new("tests", "Write the migration tests")
                            .role(TabRole::Side)
                            .source("Retire expired coupon codes")
                            .status(TabStatus::Working),
                    )
                    .tab(
                        Tab::new("audit", "Audit discount usage")
                            .harness(Some(Harness::Codex))
                            .source("Retire expired coupon codes")
                            .status(TabStatus::NeedsInput),
                    )
                    .tab(
                        Tab::new("rollback", "Rollback plan for coupon removal")
                            .role(TabRole::Side)
                            .source("Retire expired coupon codes"),
                    )
                    .tab(
                        Tab::new("emails", "Customer notice emails")
                            .source("Retire expired coupon codes")
                            .status(TabStatus::Unread),
                    )
                    .active("coupons"),
            )
            .group(
                TabGroup::new("middle")
                    .share(0.62)
                    .tab(
                        Tab::new("ledger", "Reconcile the ledger export")
                            .role(TabRole::Alongside)
                            .status(TabStatus::NeedsApproval),
                    )
                    .tab(
                        Tab::new("pg", "Upgrade Postgres driver")
                            .harness(Some(Harness::OpenCode))
                            .source("Reconcile the ledger export")
                            .status(TabStatus::Failed),
                    ),
            )
            .group(
                TabGroup::new("right").share(0.4).tab(
                    Tab::new("docs", "Pricing page copy")
                        .role(TabRole::Alongside)
                        .harness(Some(Harness::Codex)),
                ),
            )
            .focused("left")
            .event(
                3.0,
                "left",
                Tab::new("tests", "Write the migration tests")
                    .role(TabRole::Side)
                    .source("Retire expired coupon codes")
                    .status(TabStatus::Unread),
            )
            .select_event(5.5, "emails")
            .event(
                8.0,
                "middle",
                Tab::new("ledger", "Reconcile the ledger export")
                    .role(TabRole::Alongside)
                    .status(TabStatus::Working),
            )
            .focus_event(8.0, "middle")
            .event(
                11.0,
                "middle",
                Tab::new("fx", "Check FX rounding")
                    .source("Reconcile the ledger export")
                    .status(TabStatus::Working),
            )
            .select_event(11.0, "fx")
            .event(
                14.5,
                "left",
                Tab::new("audit", "Audit discount usage")
                    .harness(Some(Harness::Codex))
                    .source("Retire expired coupon codes")
                    .status(TabStatus::Working),
            )
            .close_event(17.0, "rollback")
            .focus_event(19.0, "left")
            .autoplay(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_slide_toward_the_gap() {
        // Dragging the first tab over the third: the second and third
        // shift left.
        assert_eq!(slide_offset(1, 0, 2), -1.0);
        assert_eq!(slide_offset(2, 0, 2), -1.0);
        assert_eq!(slide_offset(3, 0, 2), 0.0);
        // Dragging the last tab back over the first: the rest shift right.
        assert_eq!(slide_offset(0, 3, 0), 1.0);
        assert_eq!(slide_offset(2, 3, 0), 1.0);
        assert_eq!(slide_offset(3, 3, 0), 0.0);
    }

    #[test]
    fn main_stays_open_and_side_chats_name_their_source() {
        let main = Tab::new("m", "Plan").role(TabRole::Main);
        assert_eq!(main.help().0.as_ref(), "Plan");
        let side = Tab::new("s", "Tests").role(TabRole::Side).source("Plan");
        assert_eq!(side.help().0.as_ref(), "Side chat of Plan");
        assert_eq!(TabRole::Child.display(), "Side chat");
        assert_eq!(TabRole::Alongside.icon(), ICON_COLUMNS);
    }
}
