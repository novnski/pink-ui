//! Settings Page – Pink's settings window (Pink: shell.rs
//! `render_settings_nav` / `settings_nav_row`, settings/search.rs, the
//! contained content plane of shell/workbench/render.rs and the page
//! scaffolding of settings/widgets.rs).
//!
//! The window's chrome holds the titlebar's control cluster (sidebar toggle,
//! Back, Forward) and the settings navigation: a "Settings" heading, the
//! search field, the section rows in their three groups, and the account
//! action and Back pinned to the bottom. The page sits on the rounded content
//! plane: a title and subtitle over section cards, each card's title inside
//! it, rows of a title and description beside their control (a switch, a
//! segmented track, a pop-up menu, a badge or an action). A row that differs
//! from its default grows a reset; a card's header switch can hold its rows;
//! a row can wait for another to turn on. Searching replaces the sections
//! with results; opening one goes to its page, marks the row on an accent
//! plate with who owns it and when it applies, and scrolls it into view.
//!
//! ```ignore
//! SettingsPage::new("settings")
//!     .section(
//!         SettingsSection::new("general", "General", glyphs::GENERAL)
//!             .subtitle("Set default behavior for conversations and this device.")
//!             .card(
//!                 SettingsCard::new("links", "Links")
//!                     .row(SettingRow::switch("open-links", "Open web links in PINK", true)),
//!             ),
//!     )
//!     .on_change(|change, _, _| persist(change))
//! ```

use std::{cell::Cell, collections::HashMap, rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

use gpui::{
    Anchor, AnyElement, App, Context, DispatchPhase, Div, ElementId, Entity, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Render, Role, ScrollHandle, SharedString, Stateful, Subscription, Task, Window,
    anchored, canvas, deferred, div, point, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, MENU_OUT, MENU_RISE_DOWN, RESIZE},
    pink::{
        self, ButtonKind, ControlSize, Pink, Segment, TextStyle, Tone, TypeRole, TypeStyleExt,
        layout,
    },
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Settings Page";
pub const DESCRIPTION: &str = "Pink's settings window: sections in a sidebar with a search that jumps to any setting and marks it, and pages of titled cards whose rows hold switches, segmented tracks and pop-up menus, each with a reset back to its default";

/// The navigation glyphs of Pink's built-in sections (Pink
/// `SettingsSection::icon`), Solar Linear.
pub mod glyphs {
    pub const GENERAL: &str = "icons/settings-page/settings.svg";
    pub const PROJECTS: &str = "icons/settings-page/folder-with-files.svg";
    pub const PROVIDERS: &str = "icons/settings-page/server-2.svg";
    pub const EXTENSIONS: &str = "icons/settings-page/widget.svg";
    pub const APPEARANCE: &str = "icons/settings-page/paint-brush.svg";
    pub const NOTIFICATIONS: &str = "icons/settings-page/bell.svg";
    pub const SHORTCUTS: &str = "icons/settings-page/keyboard.svg";
    pub const FILES: &str = "icons/settings-page/folder.svg";
    pub const APPSHOTS: &str = "icons/settings-page/camera-minimalistic.svg";
    pub const USAGE: &str = "icons/settings-page/chart.svg";
    pub const UPDATES: &str = "icons/settings-page/round-arrow-down.svg";
    pub const CONNECTIONS: &str = "icons/settings-page/global.svg";
    pub const DEVICES: &str = "icons/settings-page/monitor.svg";
    pub const BACKUPS: &str = "icons/settings-page/database.svg";
    pub const DIAGNOSTICS: &str = "icons/settings-page/ssd-square.svg";
    pub const ARCHIVED: &str = "icons/settings-page/archive-minimalistic.svg";
}

const ICON_BACK: &str = "icons/settings-page/alt-arrow-left.svg";
const ICON_LOGOUT: &str = "icons/settings-page/logout-2.svg";
const ICON_RESTART: &str = "icons/settings-page/restart.svg";
const ICON_MAGNIFER: &str = "icons/settings-page/magnifer.svg";
const ICON_SIDEBAR: &str = "icons/settings-page/sidebar-minimalistic-left.svg";
const ICON_ARROW_LEFT: &str = "icons/settings-page/arrow-left.svg";
const ICON_ARROW_RIGHT: &str = "icons/settings-page/arrow-right.svg";
const ICON_DANGER: &str = "icons/settings-page/danger-triangle.svg";

/// The window this demo frames (Pink's settings at a compact window).
const DEFAULT_W: f32 = 920.0;
const DEFAULT_H: f32 = 744.0;
/// The window's corner (macOS document windows).
const WINDOW_RADIUS: f32 = 10.0;

/// The navigation's left gutter; the plane's own inset is its right one
/// (Pink shell.rs `SIDEBAR_GUTTER = SPACE_SM`).
const SIDEBAR_GUTTER: f32 = 8.0;
/// Space above a navigation group's first row (Pink `render_settings_nav`).
const NAV_GROUP_GAP: f32 = 16.0;
/// A navigation row's least height (Pink `settings_nav_row`).
const NAV_ROW_MIN_H: f32 = 30.0;

/// The contained content plane (Pink shell/workbench/render.rs
/// `CANVAS_INSET`, `CANVAS_RADIUS`, `VIEWPORT_INSET`).
const CANVAS_INSET: f32 = 8.0;
const CANVAS_RADIUS: f32 = 14.0;
const VIEWPORT_INSET: f32 = 6.0;

/// Where the titlebar's control cluster starts with the traffic lights
/// hidden (Pink shell.rs `titlebar_cluster_start(true)`). Its inset, rhythm
/// and Interface-scaled sizes are the kit's (`layout::TITLEBAR_*`,
/// `layout::titlebar_control_size`).
const CLUSTER_START: f32 = 12.0;

/// The page column (Pink `widgets::page_column`, `page_subtitle`).
const PAGE_PAD_X: f32 = 32.0;
const PAGE_PAD_TOP: f32 = 28.0;
const PAGE_PAD_BOTTOM: f32 = 72.0;
const SUBTITLE_MAX_W: f32 = 600.0;

/// Pop-up buttons and their menus (Pink provider_defaults.rs `SELECT_WIDTH`,
/// `MENU_WIDTH`, the list's 320 cap).
const SELECT_W: f32 = 220.0;
const MENU_W: f32 = 260.0;
const MENU_MAX_H: f32 = 320.0;

/// The search destination's plate (Pink widgets.rs `ROW_PLATE_INSET`,
/// `CARD_PLATE_INSET`) and the room kept above it when revealed
/// (`REVEAL_MARGIN`).
const ROW_PLATE_INSET: f32 = -6.0;
const CARD_PLATE_INSET: f32 = 4.0;
const REVEAL_MARGIN: f32 = 24.0;

/// The floating page scrollbar (Pink popover.rs `MENU_SCROLLBAR_*`).
const RAIL_HIT_W: f32 = 10.0;
const RAIL_THUMB_W: f32 = 3.0;
const RAIL_THUMB_W_ACTIVE: f32 = 5.0;
const RAIL_TRACK_INSET: f32 = 4.0;
const RAIL_MIN_THUMB: f32 = 24.0;
const RAIL_LINGER_MS: u64 = 1400;
const RAIL_FADE_MS: u64 = 260;

/// Who owns a value, under a revealed row (Pink registry `Scope::Device`).
const SCOPE_LABEL: &str = "This device";
const SCOPE_SENTENCE: &str = "Saved on this device; other devices keep their own";

// ---------------------------------------------------------------------------
// Public data
// ---------------------------------------------------------------------------

/// A setting's value: a switch's state or a choice's index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingValue {
    On(bool),
    Choice(usize),
}

impl SettingValue {
    fn is_on(self) -> bool {
        matches!(self, SettingValue::On(true))
    }

    fn index(self) -> usize {
        match self {
            SettingValue::Choice(ix) => ix,
            SettingValue::On(on) => usize::from(on),
        }
    }
}

/// When a change takes effect (Pink registry `SettingEffect`), said under a
/// revealed row and in its search result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingEffect {
    #[default]
    Immediately,
    NewChats,
    NextTime,
}

impl SettingEffect {
    fn label(self) -> &'static str {
        match self {
            Self::Immediately => "Applies immediately",
            Self::NewChats => "Applies to new chats; existing chats keep theirs",
            Self::NextTime => "Applies the next time it happens",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Self::Immediately => "Immediately",
            Self::NewChats => "New chats",
            Self::NextTime => "Next time",
        }
    }
}

/// One edit, as `on_change` reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingChange {
    pub key: SharedString,
    pub value: SettingValue,
}

/// The account row above Back (Pink `AccountAction`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountAction {
    EnableSync,
    FinishSyncSetup,
    SignOut,
}

impl AccountAction {
    fn label(self) -> &'static str {
        match self {
            Self::EnableSync => "Enable sync",
            Self::FinishSyncSetup => "Finish sync setup",
            Self::SignOut => "Sign out",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            Self::EnableSync => glyphs::CONNECTIONS,
            Self::FinishSyncSetup => ICON_RESTART,
            Self::SignOut => ICON_LOGOUT,
        }
    }
}

#[derive(Clone)]
enum Control {
    Switch,
    Segmented {
        options: Vec<SharedString>,
        width: Option<f32>,
    },
    Select {
        options: Vec<SharedString>,
        width: f32,
    },
    Badge {
        label: SharedString,
        tone: Tone,
    },
    Action {
        label: SharedString,
    },
}

/// One settings row: a title and description beside its control (Pink
/// `widgets::card_row` + `row_text` + `row_controls`).
#[derive(Clone)]
pub struct SettingRow {
    key: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    control: Control,
    value: Option<SettingValue>,
    default: Option<SettingValue>,
    choice_descriptions: Vec<SharedString>,
    companion: Option<(SharedString, SharedString, bool)>,
    dependent: bool,
    shown_with: Option<SharedString>,
    warn_on: Option<(usize, SharedString)>,
    effect: SettingEffect,
}

impl SettingRow {
    fn new(key: impl Into<SharedString>, title: impl Into<SharedString>, control: Control) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            description: None,
            control,
            value: None,
            default: None,
            choice_descriptions: Vec::new(),
            companion: None,
            dependent: false,
            shown_with: None,
            warn_on: None,
            effect: SettingEffect::default(),
        }
    }

    /// A switch (Pink `widgets::switch_control`).
    pub fn switch(key: impl Into<SharedString>, title: impl Into<SharedString>, on: bool) -> Self {
        let mut row = Self::new(key, title, Control::Switch);
        row.value = Some(SettingValue::On(on));
        row
    }

    /// Two to five fixed choices on a segmented track (Pink
    /// `button::segmented`). Sized for the longest label unless
    /// [`Self::width`] says otherwise.
    pub fn segmented(
        key: impl Into<SharedString>,
        title: impl Into<SharedString>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
    ) -> Self {
        let options = options.into_iter().map(Into::into).collect();
        let mut row = Self::new(
            key,
            title,
            Control::Segmented {
                options,
                width: None,
            },
        );
        row.value = Some(SettingValue::Choice(selected));
        row
    }

    /// More choices as a pop-up button with its menu (Pink
    /// provider_defaults.rs `select_menu`).
    pub fn select(
        key: impl Into<SharedString>,
        title: impl Into<SharedString>,
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        selected: usize,
    ) -> Self {
        let options = options.into_iter().map(Into::into).collect();
        let mut row = Self::new(
            key,
            title,
            Control::Select {
                options,
                width: SELECT_W,
            },
        );
        row.value = Some(SettingValue::Choice(selected));
        row
    }

    /// A status pill in the control column (Pink `widgets::badge` /
    /// `badge_active`).
    pub fn badge(
        key: impl Into<SharedString>,
        title: impl Into<SharedString>,
        label: impl Into<SharedString>,
        tone: Tone,
    ) -> Self {
        Self::new(
            key,
            title,
            Control::Badge {
                label: label.into(),
                tone,
            },
        )
    }

    /// A ghost action in the control column (Pink `widgets::ghost_action`);
    /// reported through `on_action`.
    pub fn action(
        key: impl Into<SharedString>,
        title: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        Self::new(
            key,
            title,
            Control::Action {
                label: label.into(),
            },
        )
    }

    pub fn description(mut self, copy: impl Into<SharedString>) -> Self {
        self.description = Some(copy.into());
        self
    }

    /// The switch's first-install value: while the switch differs, the row
    /// shows a reset (Pink general.rs `reset_control`).
    pub fn default_on(mut self, on: bool) -> Self {
        self.default = Some(SettingValue::On(on));
        self
    }

    /// The choice's first-install index, as [`Self::default_on`].
    pub fn default_choice(mut self, ix: usize) -> Self {
        self.default = Some(SettingValue::Choice(ix));
        self
    }

    /// The segmented track's or pop-up button's width.
    pub fn width(mut self, width: f32) -> Self {
        match &mut self.control {
            Control::Segmented { width: w, .. } => *w = Some(width),
            Control::Select { width: w, .. } => *w = width,
            _ => {}
        }
        self
    }

    /// One description per choice, replacing the row's as the choice
    /// changes (Pink notifications.rs `policy_description`).
    pub fn choice_descriptions(
        mut self,
        copy: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.choice_descriptions = copy.into_iter().map(Into::into).collect();
        self
    }

    /// A second, labelled switch after the control (Pink notifications.rs
    /// task row's "Keep awake").
    pub fn companion(
        mut self,
        key: impl Into<SharedString>,
        label: impl Into<SharedString>,
        on: bool,
    ) -> Self {
        self.companion = Some((key.into(), label.into(), on));
        self
    }

    /// Held by its card's header switch: dimmed and fixed while it is off
    /// (Pink notifications.rs `dependent_row`).
    pub fn dependent(mut self) -> Self {
        self.dependent = true;
        self
    }

    /// Drawn only while the row `key` in the same card is on, or not on its
    /// first choice (Pink files.rs "Autosave delay").
    pub fn shown_with(mut self, key: impl Into<SharedString>) -> Self {
        self.shown_with = Some(key.into());
        self
    }

    /// A warning strip under the row while choice `ix` is picked (Pink
    /// project_defaults.rs checkout warning).
    pub fn warn_on(mut self, ix: usize, copy: impl Into<SharedString>) -> Self {
        self.warn_on = Some((ix, copy.into()));
        self
    }

    pub fn effect(mut self, effect: SettingEffect) -> Self {
        self.effect = effect;
        self
    }
}

/// A section card: its title inside it, then rows (Pink
/// `widgets::section_card` + `section_header`).
#[derive(Clone)]
pub struct SettingsCard {
    key: SharedString,
    title: SharedString,
    description: Option<SharedString>,
    master: Option<bool>,
    warning: Option<SharedString>,
    rows: Vec<SettingRow>,
    empty: Option<SharedString>,
    footer: Option<SharedString>,
}

impl SettingsCard {
    pub fn new(key: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            description: None,
            master: None,
            warning: None,
            rows: Vec::new(),
            empty: None,
            footer: None,
        }
    }

    pub fn description(mut self, copy: impl Into<SharedString>) -> Self {
        self.description = Some(copy.into());
        self
    }

    /// A switch on the header's line that holds the card's
    /// [`SettingRow::dependent`] rows (Pink notifications.rs
    /// `header_switch`); reported under the card's key.
    pub fn master(mut self, on: bool) -> Self {
        self.master = Some(on);
        self
    }

    /// An amber strip under the header (Pink accounts.rs `card_strip`).
    pub fn warning(mut self, copy: impl Into<SharedString>) -> Self {
        self.warning = Some(copy.into());
        self
    }

    pub fn row(mut self, row: SettingRow) -> Self {
        self.rows.push(row);
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = SettingRow>) -> Self {
        self.rows.extend(rows);
        self
    }

    /// What the card says when it has no rows.
    pub fn empty(mut self, copy: impl Into<SharedString>) -> Self {
        self.empty = Some(copy.into());
        self
    }

    /// Fine print under the card (Pink `widgets::section_footer`).
    pub fn footer(mut self, copy: impl Into<SharedString>) -> Self {
        self.footer = Some(copy.into());
        self
    }
}

/// One settings section: its navigation row and its page.
#[derive(Clone)]
pub struct SettingsSection {
    key: SharedString,
    label: SharedString,
    glyph: SharedString,
    group_start: bool,
    subtitle: Option<SharedString>,
    count: Option<usize>,
    cards: Vec<SettingsCard>,
}

impl SettingsSection {
    /// `glyph` is a 16 px SVG asset; [`glyphs`] has Pink's.
    pub fn new(
        key: impl Into<SharedString>,
        label: impl Into<SharedString>,
        glyph: impl Into<SharedString>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            glyph: glyph.into(),
            group_start: false,
            subtitle: None,
            count: None,
            cards: Vec::new(),
        }
    }

    /// The row opens a navigation group: 16 above it instead of 2.
    pub fn group_start(mut self) -> Self {
        self.group_start = true;
        self
    }

    pub fn subtitle(mut self, copy: impl Into<SharedString>) -> Self {
        self.subtitle = Some(copy.into());
        self
    }

    /// A count beside the page title (Pink `widgets::page_header`).
    pub fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    pub fn card(mut self, card: SettingsCard) -> Self {
        self.cards.push(card);
        self
    }
}

type ChangeHandler = Rc<dyn Fn(&SettingChange, &mut Window, &mut App)>;
type KeyHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type AccountHandler = Rc<dyn Fn(&AccountAction, &mut Window, &mut App)>;

#[derive(Clone, Default)]
struct Handlers {
    on_change: Option<ChangeHandler>,
    on_action: Option<KeyHandler>,
    on_section: Option<KeyHandler>,
    on_back: Option<Handler>,
    on_account: Option<AccountHandler>,
}

/// Pink's settings window. See the module docs.
#[derive(IntoElement)]
pub struct SettingsPage {
    id: ElementId,
    sections: Vec<SettingsSection>,
    selected: Option<SharedString>,
    width: f32,
    height: f32,
    account: Option<AccountAction>,
    handlers: Handlers,
}

impl SettingsPage {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            sections: Vec::new(),
            selected: None,
            width: DEFAULT_W,
            height: DEFAULT_H,
            account: None,
            handlers: Handlers::default(),
        }
    }

    pub fn section(mut self, section: SettingsSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn sections(mut self, sections: impl IntoIterator<Item = SettingsSection>) -> Self {
        self.sections.extend(sections);
        self
    }

    /// The section shown first (by key); the first otherwise.
    pub fn selected(mut self, key: impl Into<SharedString>) -> Self {
        self.selected = Some(key.into());
        self
    }

    /// The window's size.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// The account row above Back.
    pub fn account(mut self, action: AccountAction) -> Self {
        self.account = Some(action);
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&SettingChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.on_change = Some(Rc::new(handler));
        self
    }

    /// A row's action button, by the row's key.
    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.on_action = Some(Rc::new(handler));
        self
    }

    /// The section shown changed, by its key.
    pub fn on_section(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.on_section = Some(Rc::new(handler));
        self
    }

    /// The navigation's Back row (Pink: leave Settings for the chat).
    pub fn on_back(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.handlers.on_back = Some(Rc::new(handler));
        self
    }

    pub fn on_account(
        mut self,
        handler: impl Fn(&AccountAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.on_account = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// A segmented control's width for `labels`: equal segments sized for the
/// longest label at the Md type size (Pink provider_defaults.rs
/// `segmented_width`).
fn segmented_width(labels: &[SharedString]) -> f32 {
    let widest = labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(4) as f32;
    (widest * 6.6 + 24.0).max(56.0) * labels.len().max(1) as f32
}

/// Back/Forward over the sections visited (Pink `NavHistory`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct History {
    entries: Vec<usize>,
    at: usize,
}

impl History {
    fn start(section: usize) -> Self {
        Self {
            entries: vec![section],
            at: 0,
        }
    }

    fn current(&self) -> usize {
        self.entries.get(self.at).copied().unwrap_or(0)
    }

    /// A visit: drops the forward entries; a visit to the current section
    /// records nothing.
    fn push(&mut self, section: usize) -> bool {
        if self.current() == section && !self.entries.is_empty() {
            return false;
        }
        self.entries.truncate(self.at + 1);
        self.entries.push(section);
        self.at = self.entries.len() - 1;
        true
    }

    fn can_back(&self) -> bool {
        self.at > 0
    }

    fn can_forward(&self) -> bool {
        self.at + 1 < self.entries.len()
    }

    fn back(&mut self) -> Option<usize> {
        self.can_back().then(|| {
            self.at -= 1;
            self.current()
        })
    }

    fn forward(&mut self) -> Option<usize> {
        self.can_forward().then(|| {
            self.at += 1;
            self.current()
        })
    }
}

/// The floating thumb's geometry (Pink `MenuScrollbarMetrics::from_viewport`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct RailMetrics {
    track: f32,
    thumb_top: f32,
    thumb: f32,
    max_scroll: f32,
}

impl RailMetrics {
    fn from_viewport(viewport: f32, max_scroll: f32, scroll: f32) -> Option<Self> {
        let max_scroll = max_scroll.max(0.0);
        if viewport <= 0.0 || max_scroll <= 0.0 {
            return None;
        }
        let track = (viewport - RAIL_TRACK_INSET * 2.0).max(0.0);
        if track <= 0.0 {
            return None;
        }
        let thumb = (track * viewport / (viewport + max_scroll))
            .max(RAIL_MIN_THUMB)
            .min(track);
        let travel = (track - thumb).max(0.0);
        Some(Self {
            track,
            thumb_top: travel * scroll.clamp(0.0, max_scroll) / max_scroll,
            thumb,
            max_scroll,
        })
    }

    fn travel(self) -> f32 {
        (self.track - self.thumb).max(0.0)
    }

    fn of(handle: &ScrollHandle) -> Option<Self> {
        let max = f32::from(handle.max_offset().y).max(0.0);
        let scroll = (-f32::from(handle.offset().y)).clamp(0.0, max);
        Self::from_viewport(f32::from(handle.bounds().size.height), max, scroll)
    }
}

/// What a search result opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Page,
    Card(usize),
    Row(usize, usize),
}

/// One search result (Pink settings/search.rs `result_row`).
#[derive(Clone, Debug, PartialEq)]
struct Hit {
    id: SharedString,
    section: usize,
    target: Target,
    title: SharedString,
    breadcrumb: SharedString,
    detail: SharedString,
    condition: Option<SharedString>,
    rank: u8,
}

/// How well `title` answers `query` when every word is somewhere in `hay`:
/// a title that starts with the query first, then one that holds it, then
/// one that holds every word, then a match elsewhere.
fn rank(title: &str, hay: &str, query: &str, words: &[&str]) -> Option<u8> {
    if !words.iter().all(|word| hay.contains(word)) {
        return None;
    }
    let title = title.to_lowercase();
    Some(if title.starts_with(query) {
        0
    } else if title.contains(query) {
        1
    } else if words.iter().all(|word| title.contains(word)) {
        2
    } else {
        3
    })
}

/// "Shown while Autosave is on." for a row drawn only with another.
fn condition_for(row: &SettingRow, card: &SettingsCard) -> Option<SharedString> {
    let parent_key = row.shown_with.as_ref()?;
    let parent = card.rows.iter().find(|r| &r.key == parent_key)?;
    Some(
        match &parent.control {
            Control::Segmented { options, .. } | Control::Select { options, .. } => format!(
                "Shown when {} isn't {}.",
                parent.title,
                options.first().cloned().unwrap_or_default()
            ),
            _ => format!("Shown while {} is on.", parent.title),
        }
        .into(),
    )
}

fn row_shown(row: &SettingRow, values: &HashMap<SharedString, SettingValue>) -> bool {
    match &row.shown_with {
        None => true,
        Some(parent) => match values.get(parent) {
            Some(SettingValue::On(on)) => *on,
            Some(SettingValue::Choice(ix)) => *ix != 0,
            None => true,
        },
    }
}

/// The settings search (Pink `registry::search`): sections, cards and rows
/// whose words hold every query word, best first.
fn search_hits(
    query: &str,
    sections: &[SettingsSection],
    values: &HashMap<SharedString, SettingValue>,
) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut hits = Vec::new();
    for (si, section) in sections.iter().enumerate() {
        let hay = format!(
            "{} {}",
            section.label,
            section.subtitle.clone().unwrap_or_default()
        )
        .to_lowercase();
        if let Some(rank) = rank(&section.label, &hay, &query, &words) {
            hits.push(Hit {
                id: format!("section:{}", section.key).into(),
                section: si,
                target: Target::Page,
                title: section.label.clone(),
                breadcrumb: "Settings".into(),
                detail: SCOPE_LABEL.into(),
                condition: None,
                rank,
            });
        }
        for (ci, card) in section.cards.iter().enumerate() {
            let hay = format!(
                "{} {} {}",
                card.title,
                card.description.clone().unwrap_or_default(),
                section.label
            )
            .to_lowercase();
            if let Some(rank) = rank(&card.title, &hay, &query, &words) {
                hits.push(Hit {
                    id: format!("card:{}", card.key).into(),
                    section: si,
                    target: Target::Card(ci),
                    title: card.title.clone(),
                    breadcrumb: section.label.clone(),
                    detail: SCOPE_LABEL.into(),
                    condition: None,
                    rank,
                });
            }
            for (ri, row) in card.rows.iter().enumerate() {
                let hay = format!(
                    "{} {} {} {}",
                    row.title,
                    row.description.clone().unwrap_or_default(),
                    card.title,
                    section.label
                )
                .to_lowercase();
                let Some(rank) = rank(&row.title, &hay, &query, &words) else {
                    continue;
                };
                let mut detail = vec![SCOPE_LABEL.to_owned(), row.effect.short().to_owned()];
                if let Some(default) = row.default {
                    let current = values.get(&row.key).copied().or(row.value);
                    detail.push(if current == Some(default) {
                        "Default".into()
                    } else {
                        "Changed".into()
                    });
                }
                hits.push(Hit {
                    id: format!("row:{}", row.key).into(),
                    section: si,
                    target: Target::Row(ci, ri),
                    title: row.title.clone(),
                    breadcrumb: format!("{} › {}", section.label, card.title).into(),
                    detail: detail.join(", ").into(),
                    condition: condition_for(row, card),
                    rank,
                });
            }
        }
    }
    hits.sort_by_key(|hit| hit.rank);
    hits
}

/// Wrapping step through `count` results (Pink `popover::menu_step`).
fn step(at: usize, count: usize, delta: isize) -> usize {
    if count == 0 {
        return 0;
    }
    (at as isize + delta).rem_euclid(count as isize) as usize
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The row or card a search result opened (Pink widgets.rs `RowFocus`).
#[derive(Clone)]
struct Destination {
    section: usize,
    card: SharedString,
    row: Option<SharedString>,
    note: SharedString,
    /// How far the one-time reveal has gone: 0 nothing, 1 the card, 2 the
    /// row (Pink `Reached`). It only moves forward.
    reached: Rc<Cell<u8>>,
    /// The keyboard moves to the plate once, after its first reveal.
    focus_pending: Rc<Cell<bool>>,
}

/// The page scrollbar's pointer and fade state (Pink `MenuScrollbarState`).
#[derive(Default)]
struct Rail {
    section: Option<usize>,
    last_y: Option<f32>,
    last_at: Option<Instant>,
    bar_hovered: bool,
    /// Where in the thumb the press landed, while dragging.
    grab: Option<f32>,
    wake: Option<Task<()>>,
    wake_at: Option<Instant>,
}

impl Rail {
    /// Records scroll motion; a fresh section only sets the baseline.
    fn note(&mut self, section: usize, y: f32, now: Instant) {
        if self.section != Some(section) {
            self.section = Some(section);
            self.last_y = Some(y);
            self.last_at = None;
            return;
        }
        match self.last_y {
            None => self.last_y = Some(y),
            Some(previous) if (previous - y).abs() > f32::EPSILON => {
                self.last_y = Some(y);
                self.last_at = Some(now);
            }
            Some(_) => {}
        }
    }

    fn held(&self) -> bool {
        self.grab.is_some() || self.bar_hovered
    }

    /// 1 while held or within the linger, then 1 → 0 over the fade.
    fn fade(&self, now: Instant) -> f32 {
        if self.held() {
            return 1.0;
        }
        let Some(at) = self.last_at else {
            return 0.0;
        };
        let elapsed = now.duration_since(at).as_millis() as f32;
        (1.0 - (elapsed - RAIL_LINGER_MS as f32) / RAIL_FADE_MS as f32).clamp(0.0, 1.0)
    }
}

struct PageState {
    seeded: bool,
    history: History,
    collapsed: bool,
    values: HashMap<SharedString, SettingValue>,
    input: Entity<TextInput>,
    _subscription: Subscription,
    query: SharedString,
    active: usize,
    opened: Option<SharedString>,
    destination: Option<Destination>,
    scrolls: HashMap<usize, ScrollHandle>,
    nav_scroll: ScrollHandle,
    results_scroll: ScrollHandle,
    rail: Rail,
    menu: Option<SharedString>,
    menu_closing: Option<(SharedString, Instant)>,
    pressed_while_open: bool,
    reap: Option<Task<()>>,
    plate_focus: FocusHandle,
    sections: Rc<Vec<SettingsSection>>,
    handlers: Handlers,
}

impl PageState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Search settings"));
        let subscription = cx.subscribe_in(&input, window, |s: &mut Self, _, event, window, cx| {
            s.on_input(event, window, cx)
        });
        Self {
            seeded: false,
            history: History::start(0),
            collapsed: false,
            values: HashMap::new(),
            input,
            _subscription: subscription,
            query: SharedString::default(),
            active: 0,
            opened: None,
            destination: None,
            scrolls: HashMap::new(),
            nav_scroll: ScrollHandle::new(),
            results_scroll: ScrollHandle::new(),
            rail: Rail::default(),
            menu: None,
            menu_closing: None,
            pressed_while_open: false,
            reap: None,
            plate_focus: cx.focus_handle(),
            sections: Rc::new(Vec::new()),
            handlers: Handlers::default(),
        }
    }

    fn seed(&mut self, sections: &[SettingsSection], selected: Option<&str>) {
        for card in sections.iter().flat_map(|s| s.cards.iter()) {
            if let Some(on) = card.master {
                self.values.insert(card.key.clone(), SettingValue::On(on));
            }
            for row in &card.rows {
                if let Some(value) = row.value {
                    self.values.insert(row.key.clone(), value);
                }
                if let Some((key, _, on)) = &row.companion {
                    self.values.insert(key.clone(), SettingValue::On(*on));
                }
            }
        }
        let start = selected
            .and_then(|key| sections.iter().position(|s| s.key.as_ref() == key))
            .unwrap_or(0);
        self.history = History::start(start);
        self.seeded = true;
    }

    fn section(&self) -> usize {
        self.history.current()
    }

    fn searching(&self) -> bool {
        !self.query.trim().is_empty()
    }

    fn hits(&self) -> Vec<Hit> {
        search_hits(&self.query, &self.sections, &self.values)
    }

    fn scroll_for(&mut self, section: usize) -> ScrollHandle {
        self.scrolls.entry(section).or_default().clone()
    }

    /// Stores `value`; the change to report, when it is one.
    fn set(&mut self, key: &SharedString, value: SettingValue) -> Option<SettingChange> {
        if self.values.get(key) == Some(&value) {
            return None;
        }
        self.values.insert(key.clone(), value);
        Some(SettingChange {
            key: key.clone(),
            value,
        })
    }

    /// A visit from the navigation retires the marked row (Pink
    /// `open_settings` → `clear_row_focus`).
    fn visit(&mut self, section: usize) -> bool {
        self.destination = None;
        self.close_menu_now();
        self.history.push(section)
    }

    fn close_menu_now(&mut self) {
        self.menu = None;
        self.menu_closing = None;
        self.reap = None;
    }

    fn open_menu(&mut self, key: SharedString) {
        self.menu = Some(key);
        self.menu_closing = None;
        self.reap = None;
    }

    /// Starts the menu's exit fade; the card unmounts when it has run.
    fn close_menu(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.menu.take() else {
            return;
        };
        let since = Instant::now();
        self.menu_closing = Some((key, since));
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        self.reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                if s.menu_closing.as_ref().is_some_and(|(_, at)| *at == since) {
                    s.menu_closing = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// `on_section` and the key of the section now shown.
    fn section_report(&self) -> Option<(KeyHandler, SharedString)> {
        let handler = self.handlers.on_section.clone()?;
        let section = self.sections.get(self.section())?;
        Some((handler, section.key.clone()))
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                // Pink `query_changed`: a new query starts at the top.
                self.query = text.clone();
                self.active = 0;
                self.opened = None;
                self.results_scroll.set_offset(point(px(0.0), px(0.0)));
                cx.notify();
            }
            InputEvent::MoveUp | InputEvent::MoveDown if self.searching() => {
                let delta = if matches!(event, InputEvent::MoveUp) {
                    -1
                } else {
                    1
                };
                self.active = step(self.active, self.hits().len(), delta);
                self.results_scroll.scroll_to_item(self.active);
                cx.notify();
            }
            InputEvent::Submit(_) if self.searching() => self.open_hit(self.active, window, cx),
            InputEvent::Escape if self.searching() => self.clear_query(cx),
            _ => {}
        }
    }

    fn clear_query(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.clear(cx));
        self.query = SharedString::default();
        self.active = 0;
        self.opened = None;
        cx.notify();
    }

    /// Opens result `ix`: its page through the ordinary visit, then the
    /// row or card it names, marked for the page to reveal (Pink
    /// `open_settings_entry`). Nothing is written.
    fn open_hit(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hit) = self.hits().into_iter().nth(ix) else {
            return;
        };
        self.active = ix;
        self.opened = Some(hit.id.clone());
        let moved = self.visit(hit.section);
        let section = self.sections.get(hit.section).cloned();
        self.destination = section.and_then(|section| match hit.target {
            Target::Page => None,
            Target::Card(ci) => section.cards.get(ci).map(|card| Destination {
                section: hit.section,
                card: card.key.clone(),
                row: None,
                note: format!("{SCOPE_SENTENCE}.").into(),
                reached: Rc::default(),
                focus_pending: Rc::new(Cell::new(true)),
            }),
            Target::Row(ci, ri) => section.cards.get(ci).and_then(|card| {
                card.rows.get(ri).map(|row| Destination {
                    section: hit.section,
                    card: card.key.clone(),
                    row: Some(row.key.clone()),
                    note: format!("{SCOPE_SENTENCE}. {}.", row.effect.label()).into(),
                    reached: Rc::default(),
                    focus_pending: Rc::new(Cell::new(true)),
                })
            }),
        });
        if hit.target == Target::Page {
            self.scroll_for(hit.section)
                .set_offset(point(px(0.0), px(0.0)));
        }
        cx.notify();
        if moved && let Some((handler, key)) = self.section_report() {
            handler(&key, window, cx);
        }
    }
}

/// Stores a value and reports it.
fn commit(
    state: &Entity<PageState>,
    key: &SharedString,
    value: SettingValue,
    window: &mut Window,
    cx: &mut App,
) {
    let (change, handler) = state.update(cx, |s, cx| {
        let change = s.set(key, value);
        if change.is_some() {
            cx.notify();
        }
        (change, s.handlers.on_change.clone())
    });
    if let (Some(change), Some(handler)) = (change, handler) {
        handler(&change, window, cx);
    }
}

/// A navigation step (a section row, Back, Forward), then `on_section`.
fn navigate(
    state: &Entity<PageState>,
    window: &mut Window,
    cx: &mut App,
    go: impl FnOnce(&mut PageState) -> bool,
) {
    let moved = state.update(cx, |s, cx| {
        let moved = go(s);
        cx.notify();
        moved
    });
    if moved && let Some((handler, key)) = state.read(cx).section_report() {
        handler(&key, window, cx);
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

/// What one frame draws from, read once from the state.
struct Frame<'a> {
    key: &'a str,
    t: &'a Pink,
    state: &'a Entity<PageState>,
    sections: &'a [SettingsSection],
    values: &'a HashMap<SharedString, SettingValue>,
    destination: Option<&'a Destination>,
    menu: Option<SharedString>,
    menu_closing: Option<(SharedString, Instant)>,
    page_scroll: ScrollHandle,
    plate_focus: FocusHandle,
    on_action: Option<KeyHandler>,
    reduced: bool,
    /// The Interface size over 16 (`pink::ui_scale`).
    scale: f32,
}

impl Frame<'_> {
    fn on(&self, key: &SharedString) -> bool {
        self.values.get(key).is_some_and(|v| v.is_on())
    }
}

impl RenderOnce for SettingsPage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let sections = Rc::new(self.sections);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            PageState::new,
        );
        let selected = self.selected.clone();
        let handlers = self.handlers.clone();
        let input = state.update(cx, |s, _| {
            if !s.seeded {
                s.seed(&sections, selected.as_deref());
            }
            s.sections = sections.clone();
            s.handlers = handlers;
            if s.section() >= sections.len() {
                s.history = History::start(0);
            }
            s.input.clone()
        });
        // Pink's settings search field: placeholder tone, the accent caret.
        input.update(cx, |input, _| {
            input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
        });

        let now = Instant::now();
        let (section_ix, page_scroll) = state.update(cx, |s, _| {
            let section = s.section();
            (section, s.scroll_for(section))
        });
        // The page rail notes scroll motion once per frame.
        let rail_y = -f32::from(page_scroll.offset().y);
        state.update(cx, |s, _| s.rail.note(section_ix, rail_y, now));
        arm_rail_wake(&state, now, cx);

        let s = state.read(cx);
        let values = s.values.clone();
        let destination = s.destination.clone();
        let collapsed = s.collapsed;
        let can_back = s.history.can_back();
        let can_forward = s.history.can_forward();
        let active = s.active;
        let opened = s.opened.clone();
        let searching = s.searching();
        let hits = if searching { s.hits() } else { Vec::new() };
        let nav_scroll = s.nav_scroll.clone();
        let results_scroll = s.results_scroll.clone();
        let menu = s.menu.clone();
        let menu_closing = s.menu_closing.clone();
        let plate_focus = s.plate_focus.clone();
        let rail_fade = s.rail.fade(now);
        let rail_active = s.rail.held();
        let rail_grab = s.rail.grab;

        let frame = Frame {
            key: &key,
            t: &t,
            state: &state,
            sections: &sections,
            values: &values,
            destination: destination.as_ref().filter(|d| d.section == section_ix),
            menu,
            menu_closing,
            page_scroll: page_scroll.clone(),
            plate_focus,
            on_action: self.handlers.on_action.clone(),
            reduced: motion::reduced_motion(cx),
            scale: pink::ui_scale(cx),
        };

        // Pink `sidebar_now`: the navigation's width glides on RESIZE when
        // the titlebar toggle hides or shows it.
        let full = layout::SIDEBAR_DEFAULT;
        let nav_w = motion::follow(
            &format!("{key}-nav-width"),
            if collapsed { 0.0 } else { full },
            RESIZE,
        );
        let ratio = (nav_w / full).clamp(0.0, 1.0);
        let nav = (nav_w > 0.5).then(|| {
            render_nav(
                &frame,
                section_ix,
                searching,
                &hits,
                active,
                opened.as_ref(),
                &input,
                &nav_scroll,
                &results_scroll,
                self.account,
            )
        });
        let sidebar = div()
            .h_full()
            .flex_none()
            .overflow_hidden()
            .w(px(nav_w))
            .children(nav.map(|nav| {
                // While the width tweens, the content rides the moving edge
                // and fades over the last 40% of travel (Pink audit SH-16).
                div()
                    .h_full()
                    .w(px(full))
                    .pt(px(layout::TITLEBAR_HEIGHT))
                    .when(ratio < 1.0, |el| {
                        el.relative()
                            .left(px(nav_w - full))
                            .opacity((ratio / 0.4).clamp(0.0, 1.0))
                    })
                    .child(nav)
            }));

        let plane = render_plane(
            &frame,
            section_ix,
            RailView {
                fade: rail_fade,
                active: rail_active,
                grab: rail_grab,
            },
        );
        let titlebar = render_titlebar(&frame, collapsed, can_back, can_forward);

        // Pink's window chords: Find in Settings (⌘F), the sidebar (⌘B),
        // Back (⌘[) and Forward (⌘]).
        let on_key = {
            let state = state.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let keystroke = &event.keystroke;
                if !keystroke.modifiers.secondary() || keystroke.modifiers.shift {
                    return;
                }
                match keystroke.key.as_str() {
                    "f" => {
                        let input = state.update(cx, |s, cx| {
                            s.collapsed = false;
                            cx.notify();
                            s.input.clone()
                        });
                        TextInput::focus(&input, window, cx);
                    }
                    "b" => state.update(cx, |s, cx| {
                        s.collapsed = !s.collapsed;
                        cx.notify();
                    }),
                    "[" => navigate(&state, window, cx, |s| {
                        s.destination = None;
                        s.close_menu_now();
                        s.history.back().is_some()
                    }),
                    "]" => navigate(&state, window, cx, |s| {
                        s.destination = None;
                        s.close_menu_now();
                        s.history.forward().is_some()
                    }),
                    _ => return,
                }
                cx.stop_propagation();
            }
        };

        div()
            .id(self.id)
            .relative()
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_row()
            .overflow_hidden()
            .rounded(px(WINDOW_RADIUS))
            .border_1()
            .border_color(t.border)
            .bg(t.chrome_surface())
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_key_down(on_key)
            .child(sidebar)
            .child(plane)
            .child(titlebar)
    }
}

/// While the rail lingers or fades, wake the page so the fade lands
/// without further input (Pink `schedule_scrollbar_hide`).
fn arm_rail_wake(state: &Entity<PageState>, now: Instant, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.rail.held() {
            return;
        }
        let Some(at) = s.rail.last_at else {
            return;
        };
        let elapsed = now.duration_since(at);
        let linger = Duration::from_millis(RAIL_LINGER_MS);
        let total = Duration::from_millis(RAIL_LINGER_MS + RAIL_FADE_MS);
        if elapsed >= total {
            return;
        }
        let wake = if elapsed < linger {
            at + linger
        } else {
            now + Duration::from_millis(16)
        };
        if s.rail.wake_at.is_some_and(|pending| pending > now) {
            return;
        }
        s.rail.wake_at = Some(wake);
        let delay = wake.saturating_duration_since(now);
        s.rail.wake = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |s, cx| {
                s.rail.wake_at = None;
                cx.notify();
            })
            .ok();
        }));
    });
}

// ---------------------------------------------------------------------------
// Titlebar
// ---------------------------------------------------------------------------

/// A control's name with its shortcut (Pink pickers.rs `ControlTooltip`):
/// the tooltip card at `px 10 / py 7`, 12, the title medium beside a
/// `kbd_hint`.
struct ShortcutTooltip {
    title: SharedString,
    keys: SharedString,
}

impl Render for ShortcutTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        pink::tooltip_card(&t)
            .whitespace_nowrap()
            .px(px(10.0))
            .py(px(7.0))
            .text_size(px(12.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(10.0))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(self.title.clone()),
                    )
                    .child(pink::kbd_hint(self.keys.clone(), &t)),
            )
    }
}

fn shortcut_tooltip(
    title: impl Into<SharedString>,
    keys: &'static str,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    let title = title.into();
    move |_, cx| {
        cx.new(|_| ShortcutTooltip {
            title: title.clone(),
            keys: keys.into(),
        })
        .into()
    }
}

/// A titlebar button (Pink shell.rs `window_control_button` /
/// `nav_history_button`): square at `titlebar_control_size` (28 at the
/// default Interface size, 24 to 32 as it scales), radius 6, the
/// `glass_hover` wash fading in, a `scaled_icon` glyph (16 by default) in
/// `text_muted`. Tab reaches it and the focus ring shows (UI rethink D58);
/// disabled it keeps its name but takes no pointer and dims its glyph.
fn chrome_button(
    id: String,
    glyph: &'static str,
    label: &'static str,
    keys: &'static str,
    enabled: bool,
    scale: f32,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    // Drawn by the `pink-icon-button` package, as in PINK; the kit adds the
    // semantics, tooltip, focus and hover wash.
    let button = crate::components::icon_button::titlebar_button(
        id.clone(),
        glyph,
        false,
        enabled,
        scale,
        t,
    )
    .role(Role::Button)
    .aria_label(label)
    .tooltip(shortcut_tooltip(label, keys))
    .tooltip_show_delay(layout::TOOLTIP_DELAY);
    if enabled {
        button
            .tab_index(0)
            .cursor_pointer()
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .on_key_down(pink::swallow_activation_key)
            .hover_wash(format!("{id}-wash"), t.glass_hover())
    } else {
        button
    }
}

/// The titlebar's control cluster over the chrome (Pink shell.rs
/// `render_titlebar_cluster`), with the traffic lights hidden: the sidebar
/// toggle, then Back and Forward.
fn render_titlebar(f: &Frame, collapsed: bool, can_back: bool, can_forward: bool) -> Div {
    let t = f.t;
    let toggle = {
        let state = f.state.clone();
        chrome_button(
            format!("{}-toggle-sidebar", f.key),
            ICON_SIDEBAR,
            if collapsed {
                "Show left sidebar"
            } else {
                "Hide left sidebar"
            },
            "⌘B",
            true,
            f.scale,
            t,
        )
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                s.collapsed = !s.collapsed;
                cx.notify();
            })
        })
    };
    let back = {
        let state = f.state.clone();
        chrome_button(
            format!("{}-nav-back", f.key),
            ICON_ARROW_LEFT,
            "Back",
            "⌘[",
            can_back,
            f.scale,
            t,
        )
        .when(can_back, |el| {
            el.on_click(move |_, window, cx| {
                navigate(&state, window, cx, |s| {
                    s.destination = None;
                    s.close_menu_now();
                    s.history.back().is_some()
                })
            })
        })
    };
    let forward = {
        let state = f.state.clone();
        chrome_button(
            format!("{}-nav-forward", f.key),
            ICON_ARROW_RIGHT,
            "Forward",
            "⌘]",
            can_forward,
            f.scale,
            t,
        )
        .when(can_forward, |el| {
            el.on_click(move |_, window, cx| {
                navigate(&state, window, cx, |s| {
                    s.destination = None;
                    s.close_menu_now();
                    s.history.forward().is_some()
                })
            })
        })
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .h(px(layout::TITLEBAR_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .pt(px(layout::TITLEBAR_TOP_PAD))
        .px(px(layout::TITLEBAR_CLUSTER_PAD))
        .child(
            div()
                .flex_none()
                .h_full()
                .w(px(CLUSTER_START - layout::TITLEBAR_CLUSTER_PAD)),
        )
        .child(toggle)
        .child(
            div()
                .ml(px(layout::TITLEBAR_GROUP_GAP))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(layout::TITLEBAR_CONTROL_GAP))
                .child(back)
                .child(forward),
        )
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

/// One settings nav row (Pink shell.rs `settings_nav_row`): a 16 glyph and
/// a 13/18 label on a row at least 30 tall, `px 8 / py 5`, gap 8, radius 8.
/// Selected rows rest on `glass_selected_bg()` in medium `text`; others are
/// transparent in `readable_muted(0.95)` and ease to `sidebar_hover_bg()`
/// and `text`.
fn nav_row(
    id: SharedString,
    glyph: SharedString,
    label: SharedString,
    selected: bool,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    let selected_fill = t.glass_selected_bg();
    div()
        .id(id.clone())
        .role(Role::Button)
        .aria_label(label.clone())
        .aria_selected(selected)
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .w_full()
        .min_h(px(NAV_ROW_MIN_H))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .rounded(px(8.0))
        .px(px(8.0))
        .py(px(5.0))
        .type_role(TypeRole::Chrome)
        .font_weight(if selected {
            gpui::FontWeight::MEDIUM
        } else {
            gpui::FontWeight::NORMAL
        })
        .cursor_pointer()
        .hover_fill_fg(
            id,
            if selected {
                selected_fill
            } else {
                gpui::transparent_black()
            },
            if selected {
                selected_fill
            } else {
                t.sidebar_hover_bg()
            },
            if selected {
                t.text
            } else {
                t.readable_muted(0.95)
            },
            t.text,
        )
        .on_key_down(pink::swallow_activation_key)
        .child(
            svg()
                .path(glyph)
                .size(px(pink::icon_size::MD))
                .flex_none()
                .text_color(if selected { t.text } else { t.text_muted }),
        )
        .child(div().flex_1().min_w_0().truncate().child(label))
}

/// One search result (Pink settings/search.rs `result_row`): a
/// `menu_row_nav` with the section's 15 glyph, the title in medium, the
/// breadcrumb in `readable_faint(0.9)` and the scope line in
/// `readable_muted(0.8)` at 11; the opened result keeps the check.
fn result_row(f: &Frame, ix: usize, hit: &Hit, highlighted: bool, opened: bool) -> AnyElement {
    let t = f.t;
    let glyph = f
        .sections
        .get(hit.section)
        .map(|s| s.glyph.clone())
        .unwrap_or_default();
    let state = f.state.clone();
    let state_move = f.state.clone();
    pink::menu_row_nav(opened, highlighted, t)
        .id(SharedString::from(format!("{}-result-{ix}", f.key)))
        .role(Role::Button)
        .aria_label(format!("{}, {}, {}", hit.title, hit.breadcrumb, hit.detail))
        .aria_selected(highlighted)
        .on_mouse_move(move |_: &MouseMoveEvent, _, cx| {
            state_move.update(cx, |s, cx| {
                if s.active != ix {
                    s.active = ix;
                    cx.notify();
                }
            })
        })
        .on_click(move |_, window, cx| state.update(cx, |s, cx| s.open_hit(ix, window, cx)))
        .child(
            svg()
                .path(glyph)
                .size(px(15.0))
                .flex_none()
                .text_color(t.text_muted),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(hit.title.clone()),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.0))
                        .text_color(t.readable_faint(0.9))
                        .child(hit.breadcrumb.clone()),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.0))
                        .text_color(t.readable_muted(0.8))
                        .child(hit.detail.clone()),
                )
                .children(hit.condition.clone().map(|condition| {
                    div()
                        .min_w_0()
                        .text_size(px(11.0))
                        .text_color(t.readable_muted(0.8))
                        .child(condition)
                })),
        )
        .into_any_element()
}

/// The settings navigation (Pink shell.rs `render_settings_nav`): the
/// "Settings" heading on the rows' text edge, the search field, the section
/// rows (or, while a query is open, its results), and the account action
/// and Back under a hairline at the bottom.
#[allow(clippy::too_many_arguments)]
fn render_nav(
    f: &Frame,
    section_ix: usize,
    searching: bool,
    hits: &[Hit],
    active: usize,
    opened: Option<&SharedString>,
    input: &Entity<TextInput>,
    nav_scroll: &ScrollHandle,
    results_scroll: &ScrollHandle,
    account: Option<AccountAction>,
) -> Div {
    let t = f.t;
    let gutter = px(SIDEBAR_GUTTER);

    // Pink settings/search.rs: `search_input_frame` around a 14 magnifier
    // and the field, gap 6; no settings match → a quiet line.
    let field = pink::search_input_frame(t).line_height(px(18.0)).child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .child(
                svg()
                    .path(ICON_MAGNIFER)
                    .size(px(14.0))
                    .flex_none()
                    .text_color(t.text_muted),
            )
            .child(div().flex_1().min_w_0().child(input.clone())),
    );
    let search = div()
        .id(SharedString::from(format!("{}-search", f.key)))
        .w_full()
        .flex()
        .flex_col()
        .when(searching, |el| el.flex_1().min_h_0())
        .child(field)
        .when(searching && hits.is_empty(), |el| {
            el.child(
                div()
                    .px(px(8.0))
                    .py(px(6.0))
                    .text_size(px(12.0))
                    .text_color(t.readable_muted(0.8))
                    .child("No settings match."),
            )
        })
        .when(searching && !hits.is_empty(), |el| {
            el.child(
                div()
                    .id(SharedString::from(format!("{}-results", f.key)))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(results_scroll)
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .pb(px(8.0))
                    .children(hits.iter().enumerate().map(|(ix, hit)| {
                        result_row(f, ix, hit, ix == active, opened == Some(&hit.id))
                    })),
            )
        });

    let rows = f.sections.iter().enumerate().map(|(ix, section)| {
        let selected = ix == section_ix;
        let state = f.state.clone();
        nav_row(
            format!("{}-nav-{}", f.key, section.key).into(),
            section.glyph.clone(),
            section.label.clone(),
            selected,
            t,
        )
        .when(section.group_start && ix > 0, |row| {
            row.mt(px(NAV_GROUP_GAP))
        })
        .on_click(move |_, window, cx| navigate(&state, window, cx, |s| s.visit(ix)))
    });

    let account_row = account.map(|action| {
        let state = f.state.clone();
        nav_row(
            format!("{}-account", f.key).into(),
            action.glyph().into(),
            action.label().into(),
            false,
            t,
        )
        .on_click(move |_, window, cx| {
            let handler = state.read(cx).handlers.on_account.clone();
            if let Some(handler) = handler {
                handler(&action, window, cx);
            }
        })
    });
    // AltArrowLeft, not the straight history arrow (Pink
    // settings-sidebar.tsx).
    let back = {
        let state = f.state.clone();
        nav_row(
            format!("{}-back", f.key).into(),
            ICON_BACK.into(),
            "Back".into(),
            false,
            t,
        )
        .aria_label("Back to chat")
        .on_click(move |_, window, cx| {
            let handler = state.read(cx).handlers.on_back.clone();
            if let Some(handler) = handler {
                handler(window, cx);
            }
        })
    };

    div()
        .w(px(layout::SIDEBAR_DEFAULT))
        .h_full()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_1()
                .min_h_0()
                .pl(gutter)
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .px(px(8.0))
                        .pt(px(12.0))
                        .pb(px(6.0))
                        .type_style(TextStyle::Caption)
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(t.readable_muted(0.6))
                        .child("Settings"),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .when(searching, |el| el.flex_1().min_h_0())
                        .child(search),
                )
                .when(!searching, |column| {
                    column.child(
                        div()
                            .id(SharedString::from(format!("{}-sections", f.key)))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(nav_scroll)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.0))
                                    .pb(px(8.0))
                                    .children(rows),
                            ),
                    )
                }),
        )
        .child(
            div()
                .flex_none()
                .pl(gutter)
                .pb(px(12.0))
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .h(px(1.0))
                        .mx(px(8.0))
                        .mb(px(6.0))
                        .bg(t.separator(false)),
                )
                .children(account_row)
                .child(back),
        )
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct RailView {
    fade: f32,
    active: bool,
    grab: Option<f32>,
}

/// The contained content plane (Pink shell/workbench/render.rs
/// `contained_plane`): below the titlebar, 8 in from the chrome, a radius
/// 14 hairline canvas on `canvas_fill()`, its viewport 6 inside the edge.
fn render_plane(f: &Frame, section_ix: usize, rail: RailView) -> Div {
    let t = f.t;
    let page = f
        .sections
        .get(section_ix)
        .map(|section| render_page(f, section))
        .unwrap_or_else(|| div().into_any_element());
    let host = div()
        .id(SharedString::from(format!("{}-page-host", f.key)))
        .relative()
        .size_full()
        .child(
            div()
                .id(SharedString::from(format!("{}-page-{section_ix}", f.key)))
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&f.page_scroll)
                .child(page),
        )
        .children(render_rail(f, rail));
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .pt(px(layout::TITLEBAR_HEIGHT))
        .px(px(CANVAS_INSET))
        .pb(px(CANVAS_INSET))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .w_full()
                .flex()
                .flex_col()
                .rounded(px(CANVAS_RADIUS))
                .border_1()
                .border_color(t.hairline(0.08))
                .bg(t.canvas_fill())
                .overflow_hidden()
                .p(px(VIEWPORT_INSET - 1.0))
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(host),
                ),
        )
}

/// The page's floating scrollbar (Pink popover.rs `render_rail` + `rail`):
/// a 10 wide strip on the right whose 3 px thumb (5 while the strip is
/// hovered or dragged) shows with scroll motion, lingers 1.4 s and fades
/// over 260 ms; pressing the track jumps, dragging the thumb scrolls.
fn render_rail(f: &Frame, rail: RailView) -> Option<AnyElement> {
    let metrics = RailMetrics::of(&f.page_scroll)?;
    let visible = rail.fade > 0.0;
    let t = f.t;
    let width = if rail.active {
        RAIL_THUMB_W_ACTIVE
    } else {
        RAIL_THUMB_W
    };
    let handle = f.page_scroll.clone();
    let state = f.state.clone();
    let hover_state = f.state.clone();
    let press = move |event: &MouseDownEvent, window: &mut Window, cx: &mut App| {
        let Some(m) = RailMetrics::of(&handle) else {
            return;
        };
        let track_top = f32::from(handle.bounds().top()) + RAIL_TRACK_INSET;
        let pointer = f32::from(event.position.y) - track_top;
        // Pink `begin_press`: on the thumb keep the grab point; on the
        // track centre the thumb under the pointer first.
        let grab = if pointer >= m.thumb_top && pointer <= m.thumb_top + m.thumb {
            pointer - m.thumb_top
        } else {
            m.thumb / 2.0
        };
        scroll_rail_to(&handle, m, pointer - grab);
        state.update(cx, |s, cx| {
            s.rail.grab = Some(grab);
            cx.notify();
        });
        window.refresh();
        cx.stop_propagation();
    };
    // While dragging, the window's pointer drives the thumb until release.
    let drag = rail.grab.map(|grab| {
        let handle = f.page_scroll.clone();
        let state = f.state.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                let move_handle = handle.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    if let Some(m) = RailMetrics::of(&move_handle) {
                        let track_top = f32::from(move_handle.bounds().top()) + RAIL_TRACK_INSET;
                        let pointer = f32::from(event.position.y) - track_top;
                        scroll_rail_to(&move_handle, m, pointer - grab);
                        window.refresh();
                    }
                });
                let state = state.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    state.update(cx, |s, cx| {
                        s.rail.grab = None;
                        s.rail.last_at = Some(Instant::now());
                        cx.notify();
                    });
                    window.refresh();
                });
            },
        )
        .absolute()
        .size_0()
    });
    Some(
        div()
            .id(SharedString::from(format!("{}-rail", f.key)))
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(RAIL_HIT_W))
            .opacity(rail.fade)
            .when(visible || rail.grab.is_some(), |el| {
                el.on_hover(move |hovered, _, cx| {
                    hover_state.update(cx, |s, cx| {
                        if s.rail.bar_hovered != *hovered {
                            s.rail.bar_hovered = *hovered;
                            if !*hovered {
                                // Leaving restarts the wait; it never hides
                                // under the pointer.
                                s.rail.last_at = Some(Instant::now());
                            }
                            cx.notify();
                        }
                    })
                })
                .on_mouse_down(MouseButton::Left, press)
            })
            .child(
                div()
                    .id(SharedString::from(format!("{}-rail-thumb", f.key)))
                    .absolute()
                    .top(px(RAIL_TRACK_INSET + metrics.thumb_top))
                    .right(px(2.0))
                    .w(px(width))
                    .h(px(metrics.thumb))
                    .rounded(px(width / 2.0))
                    .bg(t.text_faint.opacity(if rail.active { 0.68 } else { 0.5 }))
                    .hover(|s| s.bg(t.text_faint.opacity(0.85))),
            )
            .children(drag)
            .into_any_element(),
    )
}

/// Scrolls so the thumb's top sits at `thumb_top` along the track.
fn scroll_rail_to(handle: &ScrollHandle, m: RailMetrics, thumb_top: f32) {
    let travel = m.travel();
    if travel <= 0.0 {
        return;
    }
    let scroll = (thumb_top.clamp(0.0, travel) / travel) * m.max_scroll;
    handle.set_offset(point(px(0.0), px(-scroll)));
}

/// One page (Pink `widgets::page_column` + `page_title_bar` +
/// `page_subtitle`, then its cards): a column up to 720 wide, `px 32 /
/// pt 28 / pb 72`.
fn render_page(f: &Frame, section: &SettingsSection) -> AnyElement {
    let t = f.t;
    let header = div()
        .w_full()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_x(px(16.0))
        .gap_y(px(8.0))
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(px(10.0))
                .child(
                    div()
                        .type_style(TextStyle::Title1)
                        .text_color(t.text)
                        .child(section.label.clone()),
                )
                .children(section.count.map(|count| {
                    div()
                        .text_size(px(13.0))
                        .text_color(t.readable_muted(0.7))
                        .child(SharedString::from(count.to_string()))
                })),
        );
    let subtitle = section.subtitle.clone().map(|copy| {
        div()
            .mt(px(6.0))
            .max_w(px(SUBTITLE_MAX_W))
            .text_size(px(13.0))
            .line_height(px(19.0))
            .text_color(t.readable_muted(0.9))
            .child(copy)
    });
    let mut column = div()
        .w_full()
        .max_w(px(layout::PAGE_MAX_WIDTH))
        .mx_auto()
        .px(px(PAGE_PAD_X))
        .pt(px(PAGE_PAD_TOP))
        .pb(px(PAGE_PAD_BOTTOM))
        .flex()
        .flex_col()
        .child(header)
        .children(subtitle);
    for card in &section.cards {
        column = column.child(render_card(f, card));
        if let Some(footer) = &card.footer {
            column = column.child(pink::section_footer(footer.clone(), t));
        }
    }
    column.into_any_element()
}

/// The quiet accent plate behind a search destination (Pink widgets.rs
/// `Claimed::plate`): radius 8, `accent @ 0.55` edge on `accent @ 0.08`,
/// painted under the text. It takes the keyboard once revealed, and its
/// marker scrolls it 24 below the page's top once (Pink `record_reveal`).
fn destination_plate(
    f: &Frame,
    dest: &Destination,
    inset: f32,
    level: u8,
    label: String,
) -> AnyElement {
    let t = f.t;
    let handle = f.page_scroll.clone();
    let reached = dest.reached.clone();
    let focus_pending = dest.focus_pending.clone();
    let focus = f.plate_focus.clone();
    let marker = canvas(
        move |bounds, window, _| {
            if reached.get() >= level {
                return;
            }
            reached.set(level);
            let viewport = handle.bounds();
            let unscrolled = bounds.origin.y - handle.offset().y;
            let max = handle.max_offset().y.max(px(0.0));
            let target = (viewport.top() - (unscrolled - px(REVEAL_MARGIN)))
                .min(px(0.0))
                .max(-max);
            handle.set_offset(point(px(0.0), target));
            if focus_pending.replace(false) {
                let focus = focus.clone();
                window.on_next_frame(move |window, cx| window.focus(&focus, cx));
            }
            window.refresh();
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_0();
    div()
        .id("settings-row-focus")
        .absolute()
        .top(px(inset))
        .bottom(px(inset))
        .left(px(inset - 2.0))
        .right(px(inset - 2.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(t.accent.opacity(0.55))
        .bg(t.accent.opacity(0.08))
        .role(Role::Group)
        .aria_label(label)
        .track_focus(&f.plate_focus)
        .tab_index(0)
        .child(marker)
        .into_any_element()
}

/// Who owns the value and when it applies, under a marked row (Pink
/// `Claimed::note_line`): the description in `readable_tone(accent, 0.95)`.
fn note_line(copy: SharedString, t: &Pink) -> Div {
    pink::row_description(copy, t).text_color(t.readable_tone(t.accent, 0.95))
}

/// A section card (Pink `widgets::section_card` + `section_header`), with
/// the header switch, a warning strip, and its rows.
fn render_card(f: &Frame, card: &SettingsCard) -> AnyElement {
    let t = f.t;
    let master_on = card.master.map(|_| f.on(&card.key));
    let available = master_on.unwrap_or(true);

    // A row the destination names that is not drawn now: the card stands
    // in for it and says when it appears.
    let dest = f.destination.filter(|d| d.card == card.key);
    let hidden_row = dest.and_then(|d| {
        let row_key = d.row.as_ref()?;
        let row = card.rows.iter().find(|r| &r.key == row_key)?;
        (!row_shown(row, f.values)).then(|| {
            let condition = condition_for(row, card).unwrap_or_default();
            SharedString::from(format!("{}: {condition}", row.title))
        })
    });
    let card_mark = dest.and_then(|d| match (&d.row, &hidden_row) {
        (None, _) => Some((d, d.note.clone())),
        (Some(_), Some(note)) => Some((d, note.clone())),
        _ => None,
    });

    // Pink `section_header`: `px 16 / pt 14 / pb 12`, Title3 over the
    // description (gap 3), trailing actions on the title's line.
    let mut header = div()
        .w_full()
        .px(px(layout::ROW_PAD_X))
        .pt(px(14.0))
        .pb(px(12.0))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_start()
        .gap_x(px(12.0))
        .gap_y(px(8.0));
    if let Some((d, _)) = card_mark {
        header = header.relative().child(destination_plate(
            f,
            d,
            CARD_PLATE_INSET,
            1,
            format!("{}. {}", card.title, d.note),
        ));
    }
    let header = header
        .child(
            div()
                .flex_1()
                .min_w(px(layout::ROW_TEXT_MIN))
                .flex()
                .flex_col()
                .gap(px(3.0))
                .child(
                    div()
                        .type_style(TextStyle::Title3)
                        .text_color(t.text)
                        .child(card.title.clone()),
                )
                .children(
                    card.description
                        .clone()
                        .map(|copy| pink::row_description(copy, t)),
                )
                .children(card_mark.map(|(_, note)| note_line(note, t))),
        )
        .children(master_on.map(|on| {
            // Pink notifications.rs `header_switch`: held on the title's
            // line box.
            let state = f.state.clone();
            let key = card.key.clone();
            pink::row_controls().child(
                div()
                    .flex_none()
                    .h(px(TextStyle::Title3.line_height()))
                    .flex()
                    .items_center()
                    .child(
                        pink::switch_control(
                            format!("{}-master-{}", f.key, card.key),
                            card.title.clone(),
                            on,
                            t,
                        )
                        .on_click(move |_, window, cx| {
                            commit(&state, &key, SettingValue::On(!on), window, cx)
                        }),
                    ),
            )
        }));

    let warning = card.warning.clone().map(|copy| {
        div()
            .w_full()
            .px(px(layout::ROW_PAD_X))
            .pb(px(12.0))
            .child(warning_strip(copy, t).mt(px(0.0)))
    });

    let rows: Vec<AnyElement> = card
        .rows
        .iter()
        .filter(|row| row_shown(row, f.values))
        .map(|row| render_row(f, card, row, available))
        .collect();
    let empty = (card.rows.is_empty())
        .then(|| card.empty.clone())
        .flatten()
        .map(|copy| pink::card_row(false, t).child(pink::row_description(copy, t).flex_1()));

    pink::section_card(t)
        .child(header)
        .children(warning)
        .children(rows)
        .children(empty)
        .into_any_element()
}

/// Pink `widgets::warning_strip`: `mt 8 / px 16 / py 10`, radius 12, an
/// amber `@ 0.2` edge on `@ 0.06`, 12 copy in `readable_tone(warning_muted,
/// 0.9)` after a 14 danger triangle.
fn warning_strip(copy: SharedString, t: &Pink) -> Div {
    let (plate, edge, tone) = pink::strip_colors(t.warning, t.warning_muted, t);
    div()
        .mt(px(8.0))
        .px(px(16.0))
        .py(px(10.0))
        .rounded(px(12.0))
        .border_1()
        .border_color(edge)
        .bg(plate)
        .text_size(px(12.0))
        .text_color(tone)
        .flex()
        .flex_row()
        .items_start()
        .gap(px(8.0))
        .child(
            div()
                .flex_none()
                .mt(px(2.0))
                .child(svg().path(ICON_DANGER).size(px(14.0)).text_color(tone)),
        )
        .child(div().min_w_0().child(copy))
}

/// The label a reset names (Pink general.rs `reset_control`'s "Reset to …").
fn value_label(row: &SettingRow, value: SettingValue) -> SharedString {
    match (&row.control, value) {
        (Control::Segmented { options, .. } | Control::Select { options, .. }, v) => {
            options.get(v.index()).cloned().unwrap_or_default()
        }
        (_, SettingValue::On(true)) => "On".into(),
        _ => "Off".into(),
    }
}

/// One row (Pink `widgets::card_row` + `row_text` + `row_controls`): the
/// text column, then the reset (only while the value differs from its
/// default) and the control. A dependent row whose card switch is off
/// dims to 0.5 and holds still.
fn render_row(f: &Frame, card: &SettingsCard, row: &SettingRow, card_on: bool) -> AnyElement {
    let t = f.t;
    let available = !row.dependent || card_on;
    let current = f.values.get(&row.key).copied().or(row.value);
    let id = |part: &str| SharedString::from(format!("{}-{part}-{}", f.key, row.key));

    let description = match current {
        Some(SettingValue::Choice(ix)) if !row.choice_descriptions.is_empty() => {
            row.choice_descriptions.get(ix).cloned()
        }
        _ => row.description.clone(),
    };
    let dest = f
        .destination
        .filter(|d| d.card == card.key && d.row.as_ref() == Some(&row.key));
    let mut text = div()
        .flex_1()
        .min_w(px(layout::ROW_TEXT_MIN))
        .flex()
        .flex_col()
        .gap(px(2.0));
    if let Some(d) = dest {
        text = text.relative().child(destination_plate(
            f,
            d,
            ROW_PLATE_INSET,
            2,
            format!("{}. {}", row.title, d.note),
        ));
    }
    let text = text
        .child(pink::row_title(row.title.clone(), t))
        .children(description.map(|copy| pink::row_description(copy, t)))
        .children(dest.map(|d| note_line(d.note.clone(), t)))
        .when(!available, |el| el.opacity(0.5));

    let reset = match (row.default, current) {
        (Some(default), Some(value)) if value != default && available => {
            let state = f.state.clone();
            let key = row.key.clone();
            Some(
                pink::icon_button(id("reset"), ICON_RESTART, ControlSize::Md, t)
                    .aria_label(format!("Reset {}", row.title))
                    .tooltip(pink::text_tooltip(format!(
                        "Reset to {}",
                        value_label(row, default)
                    )))
                    .tooltip_show_delay(layout::TOOLTIP_DELAY)
                    .on_click(move |_, window, cx| commit(&state, &key, default, window, cx))
                    .into_any_element(),
            )
        }
        _ => None,
    };

    let control = render_control(f, row, current, available);
    let companion = row.companion.as_ref().map(|(key, label, _)| {
        let on = f.on(key);
        let state = f.state.clone();
        let key = key.clone();
        div()
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                pink::row_description(label.clone(), t)
                    .flex_none()
                    .whitespace_nowrap(),
            )
            .child(
                pink::switch_control(
                    id(&format!("companion-{key}")),
                    format!("{label} for {}", row.title),
                    on,
                    t,
                )
                .on_click(move |_, window, cx| {
                    commit(&state, &key, SettingValue::On(!on), window, cx)
                }),
            )
    });

    let warning = row.warn_on.as_ref().and_then(|(ix, copy)| {
        (current.map(|v| v.index()) == Some(*ix))
            .then(|| warning_strip(copy.clone(), t).mt(px(0.0)).w_full())
    });

    pink::card_row(false, t)
        .child(text)
        .child(
            pink::row_controls()
                .children(reset)
                .child(control)
                .children(companion)
                .when(!available, |el| el.opacity(0.5)),
        )
        .children(warning)
        .into_any_element()
}

/// A row's control, from its kind.
fn render_control(
    f: &Frame,
    row: &SettingRow,
    current: Option<SettingValue>,
    available: bool,
) -> AnyElement {
    let t = f.t;
    let id = |part: &str| SharedString::from(format!("{}-{part}-{}", f.key, row.key));
    match &row.control {
        Control::Switch => {
            let on = current.is_some_and(|v| v.is_on());
            if !available {
                return pink::switch_control_disabled(
                    id("switch"),
                    row.title.clone(),
                    on,
                    "Unavailable while its parent setting is off",
                    t,
                )
                .into_any_element();
            }
            let state = f.state.clone();
            let key = row.key.clone();
            pink::switch_control(id("switch"), row.title.clone(), on, t)
                .on_click(move |_, window, cx| {
                    commit(&state, &key, SettingValue::On(!on), window, cx)
                })
                .into_any_element()
        }
        Control::Segmented { options, width } => {
            let selected = current.map_or(0, |v| v.index());
            let width = width.unwrap_or_else(|| segmented_width(options));
            let state = f.state.clone();
            let key = row.key.clone();
            pink::segmented(
                id("segmented"),
                options
                    .iter()
                    .enumerate()
                    .map(|(ix, label)| Segment::new(ix.to_string(), label.clone()))
                    .collect(),
                selected,
                width,
                ControlSize::Md,
                t,
                move |ix, window, cx| {
                    if available {
                        commit(&state, &key, SettingValue::Choice(*ix), window, cx)
                    }
                },
            )
            .aria_label(row.title.clone())
            .into_any_element()
        }
        Control::Select { options, width } => {
            render_select(f, row, options, *width, current, available)
        }
        Control::Badge { label, tone } => pink::badge(label.clone(), *tone, t).into_any_element(),
        Control::Action { label } => {
            let handler = f.on_action.clone();
            let key = row.key.clone();
            pink::button_enabled(
                id("action"),
                ButtonKind::Ghost,
                ControlSize::Md,
                available,
                t,
            )
            .child(label.clone())
            .when(available, |el| {
                el.on_click(move |_, window, cx| {
                    if let Some(handler) = &handler {
                        handler(&key, window, cx);
                    }
                })
            })
            .into_any_element()
        }
    }
}

/// A pop-up button with its menu right-aligned under it (Pink
/// provider_defaults.rs `select_menu`): the Md select, 220 wide; the menu a
/// 260 popover of `menu_row`s, the current one checked in
/// `selected_row_icon(accent)`.
fn render_select(
    f: &Frame,
    row: &SettingRow,
    options: &[SharedString],
    width: f32,
    current: Option<SettingValue>,
    available: bool,
) -> AnyElement {
    let t = f.t;
    let menu_key = row.key.clone();
    let open = f.menu.as_ref() == Some(&menu_key);
    let closing = f
        .menu_closing
        .as_ref()
        .filter(|(key, _)| key == &menu_key)
        .map(|(_, since)| *since);
    let selected = current.map_or(0, |v| v.index());
    let label = options.get(selected).cloned().unwrap_or_default();
    let trigger_id = SharedString::from(format!("{}-select-{}", f.key, row.key));
    let mut trigger = pink::select(trigger_id.clone(), label.clone(), ControlSize::Md, open, t)
        .aria_label(format!("{}: {label}", row.title))
        .relative()
        .w(px(width))
        .max_w_full()
        .on_key_down(pink::swallow_activation_key);
    if available {
        let press_state = f.state.clone();
        let click_state = f.state.clone();
        let press_key = menu_key.clone();
        let click_key = menu_key.clone();
        trigger = trigger
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                press_state.update(cx, |s, _| {
                    s.pressed_while_open = s.menu.as_ref() == Some(&press_key);
                })
            })
            .on_click(move |event, _, cx| {
                click_state.update(cx, |s, cx| {
                    let showing = s.menu.as_ref() == Some(&click_key);
                    let was_open = std::mem::take(&mut s.pressed_while_open);
                    // GPUI's keyboard click has no press, so it toggles.
                    if (event.is_keyboard() && showing) || (!event.is_keyboard() && was_open) {
                        s.close_menu(cx);
                    } else if !showing {
                        s.open_menu(click_key.clone());
                        cx.notify();
                    }
                })
            });
    }
    if open || closing.is_some() {
        let rows: Vec<AnyElement> = options
            .iter()
            .enumerate()
            .map(|(ix, option)| {
                let active = ix == selected;
                let ring = t.focus_ring();
                let state = f.state.clone();
                let key = row.key.clone();
                pink::menu_row(format!("{trigger_id}-row-{ix}"), active, t)
                    .role(Role::MenuItemRadio)
                    .aria_label(option.clone())
                    .aria_selected(active)
                    .tab_index(0)
                    .focus_visible(move |style| pink::focus_ring(style, ring))
                    .on_key_down(pink::swallow_activation_key)
                    .on_click(move |_, window, cx| {
                        state.update(cx, |s, cx| s.close_menu(cx));
                        commit(&state, &key, SettingValue::Choice(ix), window, cx);
                    })
                    .child(div().flex_1().min_w_0().truncate().child(option.clone()))
                    .when(active, |el| {
                        el.child(
                            pink::line_glyph(
                                pink::glyphs::CHECK,
                                14.0,
                                t.selected_row_icon(t.accent),
                            )
                            .flex_none(),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        let close_state = f.state.clone();
        let key_state = f.state.clone();
        let card = pink::popover_card(t)
            .id(SharedString::from(format!("{trigger_id}-card")))
            .role(Role::Menu)
            .w(px(MENU_W))
            .flex()
            .flex_col()
            .on_mouse_down_out(move |_, _, cx| close_state.update(cx, |s, cx| s.close_menu(cx)))
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    key_state.update(cx, |s, cx| s.close_menu(cx));
                    cx.stop_propagation();
                }
            })
            .child(
                div()
                    .id(SharedString::from(format!("{trigger_id}-list")))
                    .max_h(px(MENU_MAX_H))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(layout::MENU_GAP))
                    .children(rows),
            );
        let exit = closing.map(|since| {
            if f.reduced {
                1.0
            } else {
                motion::menu_exit_progress(since)
            }
        });
        trigger = trigger.child(anchored_menu_below_end(
            format!("{trigger_id}-menu").into(),
            exit,
            card,
        ));
    }
    if !available {
        trigger = trigger.cursor_default();
    }
    trigger.into_any_element()
}

/// A dropdown hung from the trigger's bottom-right corner, 6 below it and
/// 8 inside the window (Pink popover.rs `anchored_menu_below_end`): it drops
/// 6 into place on `MENU_IN`; while `exit` runs it fades on `MENU_OUT`
/// under an occluder so the dying rows take no clicks.
fn anchored_menu_below_end(
    id: SharedString,
    exit: Option<f32>,
    card: impl IntoElement,
) -> impl IntoElement {
    let inner = div()
        .occlude()
        .pt(px(layout::POPOVER_GAP))
        .child(div().relative().child(press_guard()).child(card));
    let layer = match exit {
        Some(t) => motion::menu_out(
            SharedString::from(format!("{id}-out")),
            t,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(id, MENU_RISE_DOWN, inner).into_any_element(),
    };
    div().absolute().bottom_0().right_0().size_0().child(
        deferred(
            anchored()
                .anchor(Anchor::TopRight)
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(layer),
        )
        .priority(1),
    )
}

/// Swallows the press that dismisses the menu, so nothing behind it acts
/// on it too (Pink `frosted_menu_rounded`'s guard).
fn press_guard() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && !bounds.contains(&event.position) {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

fn sections() -> Vec<SettingsSection> {
    let general = SettingsSection::new("general", "General", glyphs::GENERAL)
        .subtitle("Set default behavior for conversations and this device.")
        .card(
            SettingsCard::new("composer", "Composer")
                .description("How messages leave the conversation composer.")
                .row(
                    SettingRow::segmented(
                        "send-shortcut",
                        "Send shortcut",
                        ["Enter", "⌘ Enter"],
                        0,
                    )
                    .description("Choose whether Enter sends or inserts a line break.")
                    .width(184.0)
                    .default_choice(0),
                )
                .row(
                    SettingRow::segmented("follow-up", "Follow-up behavior", ["Queue", "Steer"], 1)
                        .description("Choose what sending a message does while a run is live.")
                        .width(184.0)
                        .default_choice(0),
                )
                .row(
                    SettingRow::switch("context-usage", "Show context window usage", true)
                        .description("Show the context indicator beside the conversation composer.")
                        .default_on(true),
                )
                .footer(
                    "During a live run, ⌘ Shift Enter does the opposite of your follow-up \
                     choice once. Shift Enter always inserts a line break.",
                ),
        )
        .card(
            SettingsCard::new("links", "Links")
                .description("Where links in a conversation open.")
                .row(
                    SettingRow::switch("open-links", "Open web links in PINK", true)
                        .description(
                            "Open conversation links in PINK's browser; turn off to use your \
                             default browser.",
                        )
                        .default_on(true),
                ),
        )
        .card(
            SettingsCard::new("running", "Running tasks")
                .description("What PINK does while an agent is working.")
                .row(
                    SettingRow::switch("automatic-retries", "Automatic retries", true)
                        .description(
                            "Retry temporary overloads and known subscription resets, up to \
                             four attempts within six hours. Other failures still need your \
                             review.",
                        )
                        .default_on(true)
                        .effect(SettingEffect::NextTime),
                )
                .row(
                    SettingRow::segmented(
                        "answer-for-me",
                        "Answer for me after",
                        ["Off", "15s", "30s", "1m", "2m"],
                        2,
                    )
                    .description(
                        "When an agent is waiting for your answer, PINK can pick its \
                         recommended option after this delay. Questions you can answer later \
                         stay open. Approvals and questions without a recommendation always \
                         wait for you.",
                    )
                    .width(240.0)
                    .default_choice(2),
                )
                .row(
                    SettingRow::switch("keep-awake", "Prevent sleep while running", true)
                        .description(
                            "Keep this Mac awake while a local task is working. The assertion \
                             ends when work stops or PINK closes.",
                        )
                        .default_on(false),
                ),
        )
        .card(
            SettingsCard::new("confirmations", "Task confirmations")
                .description("Ask before actions that move a task or remove it.")
                .row(
                    SettingRow::switch("confirm-archive", "Confirm archive", true)
                        .description("Ask before moving a task to Archived.")
                        .default_on(true),
                )
                .row(
                    SettingRow::switch("confirm-settle", "Confirm settle", true)
                        .description("Ask before moving a task to Settled.")
                        .default_on(true),
                )
                .row(
                    SettingRow::switch("confirm-delete", "Confirm delete", true)
                        .description(
                            "Ask before permanently deleting a task and its PINK-owned records.",
                        )
                        .default_on(true),
                ),
        );

    let projects = SettingsSection::new("projects", "Projects", glyphs::PROJECTS)
        .subtitle(
            "Choose what new chats in each project start with, and the commands you can run \
             in its checkouts.",
        )
        .card(
            SettingsCard::new("new-chat-defaults", "New chat defaults")
                .description(
                    "Apply to new chats started in this project from the desktop app. \
                     Existing tasks keep their own settings.",
                )
                .row(
                    SettingRow::segmented(
                        "project-provider",
                        "Provider",
                        ["Global", "Codex", "Claude Code", "OpenCode"],
                        2,
                    )
                    .description(
                        "Global uses your app-wide provider and remembered model. Choose one \
                         to override it here.",
                    )
                    .width(360.0)
                    .default_choice(0)
                    .effect(SettingEffect::NewChats),
                )
                .row(
                    SettingRow::select(
                        "project-model",
                        "Model",
                        [
                            "Provider default",
                            "Opus",
                            "Sonnet",
                            "Sonnet with 1M context",
                            "Haiku",
                        ],
                        2,
                    )
                    .description(
                        "Provider default follows the provider's catalog. A pinned model \
                         must stay available.",
                    )
                    .width(360.0)
                    .default_choice(0)
                    .shown_with("project-provider")
                    .effect(SettingEffect::NewChats),
                )
                .row(
                    SettingRow::segmented(
                        "project-permission",
                        "Permissions",
                        ["Global", "Approve for me", "Full access"],
                        0,
                    )
                    .description(
                        "Provider capabilities and machine limits still apply. A choice the \
                         provider can't honour blocks the new chat.",
                    )
                    .width(360.0)
                    .default_choice(0)
                    .effect(SettingEffect::NewChats),
                )
                .row(
                    SettingRow::segmented(
                        "project-checkout",
                        "Checkout",
                        ["Global", "Current checkout", "New worktree"],
                        2,
                    )
                    .description(
                        "Global uses Current checkout. New worktree uses the Git isolation flow.",
                    )
                    .width(360.0)
                    .default_choice(0)
                    .warn_on(
                        2,
                        "This folder has no Git checkout. New tasks will require Current \
                         checkout.",
                    )
                    .effect(SettingEffect::NewChats),
                ),
        );

    let notifications = SettingsSection::new(
        "notifications",
        "Notifications",
        glyphs::NOTIFICATIONS,
    )
    .subtitle(
        "Choose which session events can play a sound, and when desktop notifications \
             appear.",
    )
    .card(
        SettingsCard::new("session-sounds", "Session sounds")
            .description("Play a sound for the session events you choose below.")
            .master(true)
            .row(
                SettingRow::switch("sound-completed", "Task completed", true)
                    .description("Play a sound when an agent finishes a run.")
                    .dependent(),
            )
            .row(
                SettingRow::switch("sound-input", "Input required", true)
                    .description("Play a sound when an agent needs your response.")
                    .dependent(),
            )
            .row(
                SettingRow::switch("sound-errors", "Errors and disconnections", false)
                    .description(
                        "Play a sound when a run fails or the connection remains unavailable.",
                    )
                    .dependent(),
            ),
    )
    .card(
        SettingsCard::new("desktop-notifications", "Desktop notifications")
            .description(
                "Show a system banner when a task needs your attention. Completed tasks \
                     stay in Activity.",
            )
            .master(false)
            .row(
                SettingRow::switch("background-only", "Only when in the background", true)
                    .description("Skip the banner while a PINK window is focused.")
                    .dependent(),
            ),
    )
    .card(
        SettingsCard::new("tasks", "Tasks")
            .description(
                "Choose which alerts each task on this device sends. Keep awake stops this \
                     device from sleeping while that task is selected and working; General → \
                     Prevent sleep while running covers every task.",
            )
            .rows([
                task_row("task-migrate", "Migrate billing exports to v2", 0, true),
                task_row("task-flaky", "Fix flaky checkout e2e test", 1, false),
                task_row("task-docs", "Draft the Q3 onboarding docs", 2, false),
            ]),
    );

    let files = SettingsSection::new("files", "Files", glyphs::FILES)
        .group_start()
        .subtitle("Control how workspace files are displayed and saved while you edit.")
        .card(
            SettingsCard::new("saving", "Saving")
                .row(
                    SettingRow::switch("autosave", "Autosave", false)
                        .description("Save edited workspace files to disk automatically.")
                        .default_on(false),
                )
                .row(
                    SettingRow::segmented(
                        "autosave-delay",
                        "Autosave delay",
                        ["300 ms", "600 ms", "900 ms", "1.5 s", "3 s"],
                        2,
                    )
                    .description("Save files after editing has been idle for this long.")
                    .width(56.0 * 5.0)
                    .default_choice(2)
                    .shown_with("autosave"),
                ),
        )
        .card(
            SettingsCard::new("display", "Display")
                .row(
                    SettingRow::switch("word-wrap", "Word wrap", false)
                        .description("Wrap long lines in every workspace file.")
                        .default_on(false),
                )
                .row(
                    SettingRow::switch("show-all-files", "Show all files", false)
                        .description("Include hidden and ignored files in every file tree.")
                        .default_on(false),
                ),
        );

    let updates = SettingsSection::new("updates", "Updates", glyphs::UPDATES)
        .subtitle("Keep PINK current. Updates download in the background and apply on restart.")
        .card(
            SettingsCard::new("version", "PINK 0.42.3")
                .description("Released October 21. Checked 12 minutes ago.")
                .row(SettingRow::badge(
                    "update-status",
                    "Status",
                    "Up to date",
                    Tone::Success,
                ))
                .row(
                    SettingRow::action("check-updates", "Check for updates", "Check now")
                        .description("Look for a newer build on the channel below."),
                )
                .row(
                    SettingRow::segmented("update-channel", "Channel", ["Stable", "Beta"], 0)
                        .description("Beta builds arrive about a week earlier.")
                        .default_choice(0)
                        .effect(SettingEffect::NextTime),
                ),
        );

    vec![
        general,
        projects,
        SettingsSection::new("providers", "Providers", glyphs::PROVIDERS)
            .subtitle("Sign in to the agents PINK can run, and choose what each one starts with."),
        SettingsSection::new("extensions", "Extensions", glyphs::EXTENSIONS)
            .subtitle("Packages installed on this device and what they can reach."),
        SettingsSection::new("appearance", "Appearance", glyphs::APPEARANCE)
            .subtitle("Theme, accent and the size of the interface."),
        notifications,
        SettingsSection::new("shortcuts", "Shortcuts", glyphs::SHORTCUTS)
            .subtitle("Change the keys for PINK's commands. Click a shortcut to record a new one."),
        files,
        SettingsSection::new("appshots", "Appshots", glyphs::APPSHOTS)
            .subtitle("Capture another app's window and attach it to the composer."),
        SettingsSection::new("usage", "Usage", glyphs::USAGE)
            .subtitle("What your agents spent, by provider, model and project."),
        updates,
        SettingsSection::new("connections", "Connections", glyphs::CONNECTIONS)
            .subtitle("Reach this device from your others, on the same network or through sync."),
        SettingsSection::new("devices", "Devices", glyphs::DEVICES)
            .subtitle("The devices signed in to this workspace."),
        SettingsSection::new("backups", "Backups", glyphs::BACKUPS)
            .subtitle("Snapshots of your chats and settings, kept on this device."),
        SettingsSection::new("diagnostics", "Diagnostics", glyphs::DIAGNOSTICS)
            .subtitle("Processes, connections, versions and storage, read-only."),
        SettingsSection::new("archived", "Archived sessions", glyphs::ARCHIVED)
            .group_start()
            .count(14)
            .subtitle("Sessions you archived. Restore one to bring it back to the sidebar."),
    ]
}

/// A Notifications task row: its alert policy and Keep awake.
fn task_row(key: &str, title: &str, policy: usize, keep_awake: bool) -> SettingRow {
    SettingRow::segmented(
        key.to_owned(),
        title.to_owned(),
        ["All", "Important", "Muted"],
        policy,
    )
    .choice_descriptions([
        "Every sound and banner you allow above.",
        "Completions stay quiet; input requests and failures still alert.",
        "No sounds or banners from this task.",
    ])
    .width(228.0)
    .companion(format!("{key}-awake"), "Keep awake", keep_awake)
}

pub fn story(_window: &mut Window, cx: &mut App) -> gpui::AnyView {
    story::view(cx, |_, _| {
        SettingsPage::new("settings-page")
            .sections(sections())
            .selected("general")
            .account(AccountAction::SignOut)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values_of(sections: &[SettingsSection]) -> HashMap<SharedString, SettingValue> {
        let mut values = HashMap::new();
        for row in sections
            .iter()
            .flat_map(|s| s.cards.iter())
            .flat_map(|c| c.rows.iter())
        {
            if let Some(value) = row.value {
                values.insert(row.key.clone(), value);
            }
        }
        values
    }

    #[test]
    fn search_ranks_titles_first_and_names_the_path() {
        let sections = sections();
        let values = values_of(&sections);
        let hits = search_hits("sleep", &sections, &values);
        let first = hits.first().expect("a result");
        assert_eq!(first.title.as_ref(), "Prevent sleep while running");
        assert_eq!(first.breadcrumb.as_ref(), "General › Running tasks");
        // The story turned it on; its default is off.
        assert_eq!(first.detail.as_ref(), "This device, Immediately, Changed");
        // Every word must match somewhere.
        assert!(search_hits("sleep banana", &sections, &values).is_empty());
        assert!(search_hits("   ", &sections, &values).is_empty());
    }

    #[test]
    fn hidden_rows_say_when_they_appear() {
        let sections = sections();
        let values = values_of(&sections);
        let hit = search_hits("autosave delay", &sections, &values)
            .into_iter()
            .next()
            .expect("a result");
        assert_eq!(
            hit.condition.as_deref(),
            Some("Shown while Autosave is on.")
        );
        let files = &sections[7];
        let delay = &files.cards[0].rows[1];
        assert!(!row_shown(delay, &values));
        let mut on = values.clone();
        on.insert("autosave".into(), SettingValue::On(true));
        assert!(row_shown(delay, &on));
    }

    #[test]
    fn history_goes_back_and_forward_and_drops_the_future() {
        let mut history = History::start(0);
        assert!(!history.can_back());
        assert!(history.push(3));
        assert!(history.push(5));
        assert!(!history.push(5));
        assert_eq!(history.back(), Some(3));
        assert_eq!(history.back(), Some(0));
        assert_eq!(history.back(), None);
        assert_eq!(history.forward(), Some(3));
        assert!(history.push(7));
        assert!(!history.can_forward());
        assert_eq!(history.entries, vec![0, 3, 7]);
    }

    #[test]
    fn segmented_width_follows_pink() {
        let labels: Vec<SharedString> = vec!["Enter".into(), "⌘ Enter".into()];
        assert!((segmented_width(&labels) - 2.0 * (7.0 * 6.6 + 24.0)).abs() < 0.01);
        let short: Vec<SharedString> = vec!["A".into(), "B".into(), "C".into()];
        assert_eq!(segmented_width(&short), 3.0 * 56.0);
    }

    #[test]
    fn rail_thumb_tracks_the_scroll() {
        assert_eq!(RailMetrics::from_viewport(400.0, 0.0, 0.0), None);
        let top = RailMetrics::from_viewport(400.0, 400.0, 0.0).unwrap();
        assert_eq!(top.track, 392.0);
        assert_eq!(top.thumb, 196.0);
        assert_eq!(top.thumb_top, 0.0);
        let bottom = RailMetrics::from_viewport(400.0, 400.0, 400.0).unwrap();
        assert_eq!(bottom.thumb_top, bottom.travel());
        let long = RailMetrics::from_viewport(100.0, 100_000.0, 0.0).unwrap();
        assert_eq!(long.thumb, RAIL_MIN_THUMB);
    }

    #[test]
    fn steps_wrap() {
        assert_eq!(step(0, 3, -1), 2);
        assert_eq!(step(2, 3, 1), 0);
        assert_eq!(step(0, 0, 1), 0);
    }
}
