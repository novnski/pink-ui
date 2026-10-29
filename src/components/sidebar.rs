//! Sidebar: Pink's app sidebar after UI rethink milestone 1 (novnski/pink#173,
//! PRs #180, #181, #182 and #185), without the Board page PINK removed
//! (novnski/pink#261): one slim scope row, the navigation rows,
//! the thread list and the Inactive and Archived shelves, one column on the
//! shell surface.
//!
//! The scope row names the project the list shows (or All projects, a bare
//! folder glyph) and opens a searchable project picker; the list's view
//! options and the Activity bell sit beside it, and its accessible name
//! carries the counts ("All projects, 3 open threads"). Under it sit New
//! thread and the sidebar's pages. A single project lists Pink's two-line
//! thread rows: the title over a quiet harness and branch line, the
//! time-ago in the corner (pin, Mark as inactive and archive under the
//! pointer) and the status at the right edge: a spinner while it works, a
//! dot when it is done, a pill naming the next step when it waits on you.
//! All projects lists one folding group per project of one-line rows, with
//! no indent: the header carries the grouping. Pins lead, sections page
//! behind "Show N more", and finished work drops into the Inactive and
//! Archived shelves at the bottom. Holding ⌘ (Ctrl elsewhere) labels the
//! first nine rows with their jump keys.
//!
//! Every element draws on two shared columns, the title column 16 pt in from
//! the column's edge and the time column 16 pt in from the other, at every
//! Interface size (`pink::ui_scale`); `scripts/site/sidebar-columns.mjs`
//! measures them. The host owns what Pink keeps in the app (restoring the
//! open thread's scope, persisting folds, Command Shift J): set the scope
//! with [`Sidebar::filter`], restore folds with [`Sidebar::folded`] and save
//! them from [`SidebarCommand::Fold`].
//!
//! ```ignore
//! Sidebar::new("sidebar")
//!     .project(SidebarProject::new("billing", "billing-api").path("~/code/billing-api"))
//!     .thread(SidebarThread::new("coupons", "Retire expired coupon codes", "billing")
//!         .status(ThreadStatus::NeedsApproval)
//!         .branch("coupons/retire"))
//!     .filter("billing")
//!     .on_select(|thread, _, _| println!("open {thread}"))
//! ```

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnimationExt, AnyElement, AnyView, App, ClickEvent, Context, DispatchPhase, Div,
    ElementId, Entity, FocusHandle, FontWeight, Hsla, IntoElement, KeyDownEvent,
    ModifiersChangedEvent, MouseButton, MouseDownEvent, Render, Role, ScrollHandle, SharedString,
    Stateful, Subscription, Svg, Task, Toggled, Transformation, Window, anchored, canvas, deferred,
    div, linear_color_stop, linear_gradient, prelude::*, px, radians, svg,
};

use super::section_header as header;
use crate::{
    HoverFill,
    components::empty_state,
    motion::{self, COLLAPSE, MENU_OUT, MENU_RISE_DOWN, RESORT},
    pink::{self, FillTier, Pink, TypeRole, chrome, layout, loaders, radius, space},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Sidebar";
pub const DESCRIPTION: &str = "The app's thread sidebar: one slim scope row that opens a searchable project picker beside the view options and Activity, New thread and page rows, every thread with its live status, time and hover actions in folding groups on one title column, and quiet Inactive and Archived shelves underneath";

const ICON_NEW_THREAD: &str = "icons/sidebar/pen-new-square.svg";
const ICON_AUTOMATIONS: &str = "icons/sidebar/stopwatch-play.svg";
const ICON_SORT: &str = "icons/sidebar/sort.svg";
const ICON_BELL: &str = "icons/sidebar/bell.svg";
const ICON_PIN: &str = "icons/sidebar/pin.svg";
const ICON_CHECK: &str = "icons/sidebar/check.svg";
const ICON_ARCHIVE: &str = "icons/sidebar/archive-minimalistic.svg";
const ICON_UNARCHIVE: &str = "icons/sidebar/archive-up-minimalistic.svg";
const ICON_SHIELD: &str = "icons/sidebar/shield-warning.svg";
const ICON_QUESTION: &str = "icons/sidebar/chat-round-question-mark.svg";
const ICON_DANGER: &str = "icons/sidebar/danger-circle.svg";
const ICON_CHAT: &str = "icons/sidebar/chat-round-line.svg";
const ICON_BRANCH: &str = "icons/sidebar/git-branch.svg";
const ICON_EYE: &str = "icons/sidebar/eye.svg";
const ICON_CHEVRON: &str = "icons/sidebar/alt-arrow-right.svg";
const ICON_ARROW_DOWN: &str = "icons/sidebar/alt-arrow-down.svg";
const ICON_ARROW_UP: &str = "icons/sidebar/alt-arrow-up.svg";
const ICON_PLUS: &str = "icons/sidebar/plus.svg";
const ICON_ALL_PROJECTS: &str = "icons/sidebar/folder-with-files.svg";
const ICON_FOLDER: &str = "icons/sidebar/folder.svg";
const ICON_LAPTOP: &str = "icons/sidebar/laptop.svg";
const ICON_WIDGET: &str = "icons/sidebar/widget.svg";
const ICON_LIST: &str = "icons/sidebar/list.svg";
const ICON_CLOCK: &str = "icons/sidebar/clock-circle.svg";
const ICON_CALENDAR: &str = "icons/sidebar/calendar.svg";
const ICON_SORT_VERTICAL: &str = "icons/sidebar/sort-vertical.svg";
const ICON_PULL_REQUEST: &str = "icons/sidebar/git-pull-request.svg";
const ICON_BOT: &str = "icons/sidebar/bot.svg";
const ICON_CLOSE_CIRCLE: &str = "icons/sidebar/close-circle.svg";
const ICON_SETTINGS: &str = "icons/sidebar/settings.svg";
const ICON_SETTINGS_GEAR: &str = "icons/sidebar/settings-gear.svg";
const ICON_SMARTPHONE: &str = "icons/sidebar/smartphone.svg";
const ICON_RESTART: &str = "icons/sidebar/restart.svg";
const ICON_CLAUDE: &str = "icons/sidebar/claude-mark.svg";
const ICON_OPENAI: &str = "icons/sidebar/openai-mark.svg";
const ICON_OPENCODE: &str = "icons/sidebar/opencode-mark.svg";

/// Pink: shell.rs `chat_row_height` – a title line and one meta line.
const ROW_H: f32 = 52.0;
/// Pink: shell.rs `compact_chat_row_height` – a one-line row in All projects.
const COMPACT_ROW_H: f32 = 32.0;
/// Pink: spaces.rs `SIDEBAR_SHELF_ROW_HEIGHT` – Settled and Archived rows.
const SHELF_ROW_H: f32 = 36.0;
/// Pink: spaces.rs `SIDEBAR_MORE_ROW_HEIGHT`.
const MORE_ROW_H: f32 = 28.0;
/// Pink: spaces.rs `SIDEBAR_SECTION_GAP`, above every group.
const SECTION_GAP: f32 = 12.0;
/// Pink: spaces.rs `SIDEBAR_DISCLOSURE_BODY_INSET`.
const BODY_INSET: f32 = 4.0;
/// Pink: the Pinned group's rule – `pt 8`, `pb 8` and one pixel.
const DIVIDER_H: f32 = 17.0;
/// Pink: spaces.rs `SIDEBAR_SECTION_PAGE` – rows a section shows per page.
const SECTION_PAGE: usize = 5;
/// Pink: shell.rs `SIDEBAR_GUTTER` (`SPACE_SM`).
const GUTTER: f32 = space::SM;
/// Pink: shell.rs `SIDEBAR_MARK_SLOT` – one leading column for every row kind.
const MARK_SLOT: f32 = 16.0;
/// Pink: shell.rs `SIDEBAR_MARK_GAP`.
const MARK_GAP: f32 = space::SM;
/// Pink: shell.rs `SIDEBAR_META_HARNESS_ICON_SIZE`.
const META_HARNESS: f32 = 11.0;
/// Pink: shell.rs `SIDEBAR_ARCHIVED_HARNESS_ICON_SIZE`.
const SHELF_HARNESS: f32 = 14.0;
/// Pink: shell.rs `sidebar_faded_label` – the fade that stands in for an ellipsis.
const LABEL_FADE: f32 = 20.0;
/// Pink: shell.rs `SIDEBAR_GLASS_FADE_BAND` – the list's scroll-edge fade.
const LIST_FADE: f32 = 24.0;
/// Pink: activity_center.rs `UNREAD_DOT`.
const UNREAD_DOT: f32 = 6.0;
/// Icon to label in a navigation row (Pink: spaces.rs `SIDEBAR_NAV_GAP`,
/// UI rethink D48): the 16 pt icon on the 16 pt column puts labels on 40.
const NAV_GAP: f32 = 8.0;
/// The scope row and the navigation rows (Pink: spaces.rs `SCOPE_ROW`).
const SCOPE_ROW: f32 = 32.0;
/// The scope row's project tile (Pink: spaces.rs `SCOPE_TILE`).
const SCOPE_TILE: f32 = 18.0;
/// New thread and Automations glyphs, and the All projects folder above
/// them (Pink: spaces.rs `SIDEBAR_NAV_ICON`).
const NAV_ICON: f32 = 16.0;
/// The footer's buttons and glyphs (Pink: shell.rs `SIDEBAR_FOOTER_BUTTON`,
/// `SIDEBAR_FOOTER_ICON`) and the gear's own empty space before its first
/// stroke (`SIDEBAR_FOOTER_BEARING`).
const FOOTER_BUTTON: f32 = chrome::CONTROL_L;
const FOOTER_ICON: f32 = chrome::ICON_L;
const FOOTER_BEARING: f32 = 1.5;
/// Pink: settings.rs `JUMP_SLOTS` – ⌘1 … ⌘9.
const JUMP_SLOTS: usize = 9;
/// Pink: project_picker.rs – the picker list's scroll budget.
const PICKER_LIST_MAX: f32 = 224.0;
/// Pink: spaces.rs `render_sidebar_view_menu` – the view menu's scroll budget.
const VIEW_MENU_MAX: f32 = 320.0;
/// Pink: icons.rs `claude_brand` – the Claude mark's own orange.
const CLAUDE_BRAND: u32 = 0xD97757;
const DAY: f32 = 86_400.0;
/// How long the demo plays before it starts over.
const DEMO_LOOP: f32 = 26.0;

// ---------------------------------------------------------------------------
// Public data
// ---------------------------------------------------------------------------

/// Where a task stands (Pink proto `ChatIndicator` plus the send-truth
/// override for a queued send).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadStatus {
    /// Nothing going on.
    Idle,
    Working,
    /// Paused until you approve something.
    NeedsApproval,
    /// Asked you a question.
    NeedsInput,
    Failed,
    /// Finished; a fresh result until someone reads it.
    Done,
    /// A send waiting for its delivery path.
    Queued,
}

/// The agent that runs a task; its mark opens the row's meta line.
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

    /// The mark's brand tint; the others paint in the row's quiet tone.
    fn tint(self) -> Option<Hsla> {
        match self {
            Harness::ClaudeCode => Some(gpui::rgb(CLAUDE_BRAND).into()),
            Harness::Codex | Harness::OpenCode => None,
        }
    }

    fn provider(self) -> &'static str {
        match self {
            Harness::ClaudeCode => "Claude Code",
            Harness::Codex => "Codex",
            Harness::OpenCode => "OpenCode",
        }
    }
}

/// A pull request's state, which tints its badge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Merged,
    Closed,
}

/// One project: a folder on a machine.
#[derive(Clone, Debug)]
pub struct SidebarProject {
    pub id: SharedString,
    pub name: SharedString,
    /// The folder, shown in the picker (home as `~`).
    pub path: SharedString,
}

impl SidebarProject {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            path: SharedString::default(),
        }
    }

    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = path.into();
        self
    }
}

/// One task (Pink: a chat in the sidebar).
#[derive(Clone, Debug)]
pub struct SidebarThread {
    pub id: SharedString,
    pub title: SharedString,
    /// The project id it belongs to.
    pub project: SharedString,
    pub branch: Option<SharedString>,
    pub harness: Option<Harness>,
    pub model: Option<SharedString>,
    /// The machine it runs on; `None` is this one.
    pub device: Option<SharedString>,
    pub status: ThreadStatus,
    /// Its result (or failure) was already read.
    pub seen: bool,
    pub pinned: bool,
    /// Marked inactive: it waits on the Inactive shelf.
    pub settled: bool,
    pub archived: bool,
    /// Seconds since its last message.
    pub age: f32,
    /// Seconds since it was created (defaults to its age).
    pub created: Option<f32>,
    /// Related chats it started.
    pub related: usize,
    /// Open in a workbench tab.
    pub open_tab: bool,
    pub pull_request: Option<(u32, PullRequestState)>,
}

impl SidebarThread {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        project: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            project: project.into(),
            branch: None,
            harness: Some(Harness::ClaudeCode),
            model: None,
            device: None,
            status: ThreadStatus::Idle,
            seen: false,
            pinned: false,
            settled: false,
            archived: false,
            age: 0.0,
            created: None,
            related: 0,
            open_tab: false,
            pull_request: None,
        }
    }

    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// The agent running it (Claude Code by default); `None` hides the mark.
    pub fn harness(mut self, harness: Option<Harness>) -> Self {
        self.harness = harness;
        self
    }

    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn device(mut self, device: impl Into<SharedString>) -> Self {
        self.device = Some(device.into());
        self
    }

    pub fn status(mut self, status: ThreadStatus) -> Self {
        self.status = status;
        self
    }

    pub fn seen(mut self, seen: bool) -> Self {
        self.seen = seen;
        self
    }

    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    pub fn inactive(mut self, settled: bool) -> Self {
        self.settled = settled;
        self
    }

    pub fn archived(mut self, archived: bool) -> Self {
        self.archived = archived;
        self
    }

    pub fn age(mut self, seconds: f32) -> Self {
        self.age = seconds;
        self
    }

    pub fn created(mut self, seconds_ago: f32) -> Self {
        self.created = Some(seconds_ago);
        self
    }

    pub fn related(mut self, count: usize) -> Self {
        self.related = count;
        self
    }

    pub fn open_tab(mut self, open: bool) -> Self {
        self.open_tab = open;
        self
    }

    pub fn pull_request(mut self, number: u32, state: PullRequestState) -> Self {
        self.pull_request = Some((number, state));
        self
    }
}

/// A navigation row under the project pill (Pink `SidebarNavItem`).
#[derive(Clone, Debug)]
pub struct SidebarNavItem {
    pub key: SharedString,
    pub label: SharedString,
    pub icon: SharedString,
    /// A shortcut shown under the pointer.
    pub hint: Option<SharedString>,
    /// A page: selecting it marks the row until a task opens. New thread
    /// is an action, not a page.
    pub page: bool,
}

impl SidebarNavItem {
    pub fn new(
        key: impl Into<SharedString>,
        label: impl Into<SharedString>,
        icon: impl Into<SharedString>,
    ) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            icon: icon.into(),
            hint: None,
            page: true,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn page(mut self, page: bool) -> Self {
        self.page = page;
        self
    }

    /// Pink's New thread row, ⌘N.
    pub fn new_thread() -> Self {
        Self::new("new-thread", "New thread", ICON_NEW_THREAD)
            .hint("⌘N")
            .page(false)
    }

    pub fn automations() -> Self {
        Self::new("automations", "Automations", ICON_AUTOMATIONS)
    }
}

/// What a person did to one task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarAction {
    Pin,
    Unpin,
    /// "Mark as inactive": moved to the Inactive shelf.
    MarkInactive,
    /// "Mark as active": back from the Inactive shelf into the list.
    MarkActive,
    Archive,
    Unarchive,
    /// Its related-threads chip.
    OpenRelated,
}

/// Everything else the sidebar asks its host to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidebarCommand {
    /// A navigation row (its key).
    Nav(SharedString),
    /// A project group's + button.
    NewThread(SharedString),
    /// The project filter changed (`None`: All projects).
    Filter(Option<SharedString>),
    NewProject,
    ManageProjects,
    /// The bell turned the Activity view on or off.
    Activity(bool),
    /// The footer's Settings button.
    OpenSettings,
    /// The footer's Devices button.
    OpenDevices,
    /// A group folded or opened (`"pinned"`, `"project:<id>"` or
    /// `"device:<name>"`), for the host to persist (Pink keeps folds in its
    /// settings, #182).
    Fold {
        group: SharedString,
        folded: bool,
    },
}

/// The list's order (Pink `SidebarSort`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarSort {
    LastUpdated,
    Created,
    /// The order the tasks were given in.
    Manual,
}

/// How a single project's list groups (Pink `SidebarOrganization`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarGrouping {
    InOneList,
    ByDevice,
}

type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&SharedString, SidebarAction, &mut Window, &mut App)>;
type CommandHandler = Rc<dyn Fn(&SidebarCommand, &mut Window, &mut App)>;

#[derive(Clone, Default)]
struct Handlers {
    select: Option<SelectHandler>,
    action: Option<ActionHandler>,
    command: Option<CommandHandler>,
}

/// Something to tell the host once the state has settled.
enum Effect {
    Select(SharedString),
    Action(SharedString, SidebarAction),
    Command(SidebarCommand),
}

impl Handlers {
    fn emit(&self, effect: Effect, window: &mut Window, cx: &mut App) {
        match effect {
            Effect::Select(id) => {
                if let Some(f) = &self.select {
                    f(&id, window, cx)
                }
            }
            Effect::Action(id, action) => {
                if let Some(f) = &self.action {
                    f(&id, action, window, cx)
                }
            }
            Effect::Command(command) => {
                if let Some(f) = &self.command {
                    f(&command, window, cx)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The builder
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    projects: Vec<SidebarProject>,
    tasks: Vec<SidebarThread>,
    nav: Vec<SidebarNavItem>,
    filter: Option<SharedString>,
    selected: Option<SharedString>,
    page: Option<SharedString>,
    activity: bool,
    sort: SidebarSort,
    grouping: SidebarGrouping,
    show_branch: bool,
    show_pull_request: bool,
    show_harness: bool,
    auto_settle: Option<u16>,
    local_device: SharedString,
    events: Vec<(f32, SidebarThread)>,
    folded: Vec<SharedString>,
    autoplay: bool,
    width: f32,
    height: f32,
    handlers: Handlers,
}

impl Sidebar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            projects: Vec::new(),
            tasks: Vec::new(),
            nav: vec![SidebarNavItem::new_thread(), SidebarNavItem::automations()],
            filter: None,
            selected: None,
            page: None,
            activity: false,
            sort: SidebarSort::LastUpdated,
            grouping: SidebarGrouping::InOneList,
            show_branch: true,
            show_pull_request: true,
            show_harness: true,
            auto_settle: Some(1),
            local_device: "This Mac".into(),
            events: Vec::new(),
            folded: Vec::new(),
            autoplay: false,
            width: layout::SIDEBAR_DEFAULT,
            height: 720.0,
            handlers: Handlers::default(),
        }
    }

    /// Projects take Pink's six hues in the order they are added.
    pub fn project(mut self, project: SidebarProject) -> Self {
        self.projects.push(project);
        self
    }

    pub fn thread(mut self, task: SidebarThread) -> Self {
        self.tasks.push(task);
        self
    }

    /// Replaces the navigation rows (New thread, Automations).
    pub fn nav(mut self, items: Vec<SidebarNavItem>) -> Self {
        self.nav = items;
        self
    }

    /// Starts on one project's list; without it, All projects.
    pub fn filter(mut self, project_id: impl Into<SharedString>) -> Self {
        self.filter = Some(project_id.into());
        self
    }

    /// The task open when the sidebar first appears.
    pub fn selected(mut self, task_id: impl Into<SharedString>) -> Self {
        self.selected = Some(task_id.into());
        self
    }

    /// A page open when the sidebar first appears (its nav key).
    pub fn current_page(mut self, key: impl Into<SharedString>) -> Self {
        self.page = Some(key.into());
        self
    }

    /// Groups that start folded, as the host saved them from
    /// [`SidebarCommand::Fold`].
    pub fn folded(mut self, groups: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.folded = groups.into_iter().map(Into::into).collect();
        self
    }

    /// Starts in the Activity view.
    pub fn activity_view(mut self, on: bool) -> Self {
        self.activity = on;
        self
    }

    pub fn sort(mut self, sort: SidebarSort) -> Self {
        self.sort = sort;
        self
    }

    pub fn grouping(mut self, grouping: SidebarGrouping) -> Self {
        self.grouping = grouping;
        self
    }

    pub fn show_branch(mut self, show: bool) -> Self {
        self.show_branch = show;
        self
    }

    pub fn show_pull_request(mut self, show: bool) -> Self {
        self.show_pull_request = show;
        self
    }

    pub fn show_harness(mut self, show: bool) -> Self {
        self.show_harness = show;
        self
    }

    /// Idle tasks settle on their own after this many days (Pink's default
    /// is one); `None` keeps them in the list.
    pub fn auto_inactive(mut self, days: Option<u16>) -> Self {
        self.auto_settle = days;
        self
    }

    /// The name of this machine, for tasks without a device.
    pub fn local_device(mut self, name: impl Into<SharedString>) -> Self {
        self.local_device = name.into();
        self
    }

    /// During autoplay, replaces (or adds) the task with the same id `at`
    /// seconds in.
    pub fn event(mut self, at: f32, task: SidebarThread) -> Self {
        self.events.push((at, task));
        self
    }

    /// Plays the scripted events in a loop until someone clicks.
    pub fn autoplay(mut self, autoplay: bool) -> Self {
        self.autoplay = autoplay;
        self
    }

    /// The column's width (Pink: 224 to 400, 256 by default).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.clamp(layout::SIDEBAR_MIN, layout::SIDEBAR_MAX);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// A task row was clicked (or reached with ⌘1–9 / Ctrl-Tab).
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.select = Some(Rc::new(handler));
        self
    }

    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, SidebarAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.action = Some(Rc::new(handler));
        self
    }

    pub fn on_command(
        mut self,
        handler: impl Fn(&SidebarCommand, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.command = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// What a row shows (Pink: proto `ChatIndicator` after the send override).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Indicator {
    Working,
    Approval,
    Input,
    Errored,
    Completed,
    Queued,
    Idle,
}

impl SidebarThread {
    fn indicator(&self) -> Indicator {
        match self.status {
            ThreadStatus::Working => Indicator::Working,
            ThreadStatus::NeedsApproval => Indicator::Approval,
            ThreadStatus::NeedsInput => Indicator::Input,
            ThreadStatus::Failed => Indicator::Errored,
            ThreadStatus::Done if !self.seen => Indicator::Completed,
            ThreadStatus::Queued => Indicator::Queued,
            ThreadStatus::Done | ThreadStatus::Idle => Indicator::Idle,
        }
    }

    /// Waiting on you (Pink: spaces.rs `needs_you`).
    fn needs_you(&self) -> bool {
        matches!(
            self.status,
            ThreadStatus::NeedsApproval | ThreadStatus::NeedsInput | ThreadStatus::Failed
        )
    }

    /// Lights the bell's dot (Pink: activity_center.rs `has_attention`).
    fn wants_you(&self) -> bool {
        !self.archived
            && match self.status {
                ThreadStatus::NeedsApproval | ThreadStatus::NeedsInput => true,
                ThreadStatus::Done | ThreadStatus::Failed => !self.seen,
                _ => false,
            }
    }

    /// Pink: spaces.rs `can_manually_settle_chat`.
    fn can_settle(&self) -> bool {
        !self.archived
            && !self.pinned
            && !self.needs_you()
            && matches!(self.indicator(), Indicator::Idle | Indicator::Completed)
    }
}

/// Activity's sections, most urgent first (Pink: spaces.rs `ActivitySection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Section {
    NeedsYou,
    Ready,
    Running,
}

impl Section {
    fn key(self) -> &'static str {
        match self {
            Section::NeedsYou => "activity:needs-you",
            Section::Ready => "activity:ready",
            Section::Running => "activity:running",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::NeedsYou => "Needs you",
            Section::Ready => "Ready",
            Section::Running => "Running",
        }
    }

    /// Pink: spaces.rs `activity_section`.
    fn of(task: &SidebarThread) -> Option<Section> {
        match task.indicator() {
            Indicator::Errored if task.seen => None,
            Indicator::Approval | Indicator::Input | Indicator::Errored => Some(Section::NeedsYou),
            Indicator::Completed => Some(Section::Ready),
            Indicator::Working | Indicator::Queued => Some(Section::Running),
            Indicator::Idle => None,
        }
    }
}

/// A section's trailing row (Pink: spaces.rs `MoreRow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoreRow {
    More(usize),
    Less,
}

/// Pink: spaces.rs `SectionPage::more_row` – `visible` can exceed `limit`
/// by the kept selected row.
fn more_row(total: usize, visible: usize, limit: usize) -> Option<MoreRow> {
    let hidden = total.saturating_sub(visible);
    if hidden > 0 {
        Some(MoreRow::More(hidden.min(SECTION_PAGE)))
    } else if limit > SECTION_PAGE && total > SECTION_PAGE {
        Some(MoreRow::Less)
    } else {
        None
    }
}

/// A project's live counts (Pink: sidebar_projects.rs `ProjectActivity`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Counts {
    /// Threads still in the list, running or not: open.
    active: usize,
    /// Waiting on you.
    need: usize,
    /// Running and not also waiting on you.
    working: usize,
    /// Seconds since the most recent activity, settled tasks included.
    last: Option<f32>,
}

impl Counts {
    /// Pink `ProjectActivity::spoken`: open threads, then what needs you
    /// and what runs, each only when there is some. The scope row speaks
    /// it; the list shows the counts.
    fn summary(&self) -> String {
        if self.active == 0 {
            return "No open threads".into();
        }
        let mut parts = vec![open_threads(self.active)];
        if self.need > 0 {
            parts.push(format!(
                "{} need{} you",
                self.need,
                if self.need == 1 { "s" } else { "" }
            ));
        }
        if self.working > 0 {
            parts.push(format!("{} running", self.working));
        }
        parts.join(", ")
    }
}

/// "1 open thread", "3 open threads" (Pink: sidebar_projects.rs
/// `open_threads`): the size of the active list, worded so it never reads
/// as threads still running.
fn open_threads(count: usize) -> String {
    format!("{count} open thread{}", if count == 1 { "" } else { "s" })
}

/// Compact relative time, no "ago" (Pink: proto `format_time_ago`).
fn time_ago(seconds: f32) -> String {
    let s = seconds.max(0.0) as u64;
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

#[derive(Clone)]
struct Row {
    task: SidebarThread,
    /// The order it was given in (manual sort).
    order: usize,
    /// Clock time of its last message.
    updated: f32,
    created: f32,
    /// Returned to the list by hand: auto-settling leaves it alone.
    kept: bool,
}

/// Labels that overflowed at their last paint: they wear the edge fade.
type Clips = Rc<RefCell<HashSet<String>>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PopupKind {
    Picker,
    View,
}

/// A popover's lifecycle (Pink `Popup`).
#[derive(Default)]
struct Popup {
    open: bool,
    /// When the exit fade began; the card stays mounted until it ends.
    closing: Option<Instant>,
    /// The trigger press found it mounted, so the click leaves it closed.
    pressed_while_open: bool,
    /// The one highlight the arrows and the pointer share.
    cursor: Option<usize>,
    pending_focus: bool,
    reap: Option<Task<()>>,
}

impl Popup {
    fn mounted(&self) -> bool {
        self.open || self.closing.is_some()
    }
}

struct SidebarState {
    projects: Vec<SidebarProject>,
    nav: Vec<SidebarNavItem>,
    local_device: SharedString,
    initial: Vec<SidebarThread>,
    initial_selected: Option<SharedString>,
    events: Vec<(f32, SidebarThread)>,
    played: usize,
    autoplay: bool,
    clock: f32,
    loop_start: f32,
    last_tick: Instant,
    rows: Vec<Row>,
    next_order: usize,
    filter: Option<SharedString>,
    selected: Option<SharedString>,
    page: Option<SharedString>,
    activity: bool,
    /// The row under the pointer: its corner trades the time for actions.
    hovered: Option<SharedString>,
    /// The archived row under the pointer.
    shelf_hovered: Option<SharedString>,
    collapsed: HashSet<String>,
    settled_open: bool,
    archived_open: bool,
    /// Show more: rows per section key beyond the first page.
    pages: HashMap<String, usize>,
    sort: SidebarSort,
    grouping: SidebarGrouping,
    show_branch: bool,
    show_pull_request: bool,
    show_harness: bool,
    auto_settle: Option<u16>,
    picker: Popup,
    view: Popup,
    search: Entity<TextInput>,
    query: SharedString,
    jump_hints: bool,
    focus: FocusHandle,
    /// The view options trigger, and its open menu.
    view_focus: FocusHandle,
    view_menu_focus: FocusHandle,
    scroll: ScrollHandle,
    clips: Clips,
    // The resort glide (Pink: shell.rs `render_chat_sidebar`).
    prev_order: Vec<(String, f32)>,
    resort: HashMap<String, f32>,
    new_keys: HashSet<String>,
    epoch: usize,
    /// A disclosure animates its own height, so its toggle is no resort.
    skip_resort: bool,
    handlers: Handlers,
    _search_events: Subscription,
    _ticker: Task<()>,
}

impl SidebarState {
    fn new(config: &Sidebar, window: &mut Window, cx: &mut Context<Self>) -> Self {
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
        let search = cx.new(|cx| TextInput::new(window, cx).placeholder("Search projects…"));
        let search_events = cx.subscribe_in(
            &search,
            window,
            |s: &mut SidebarState, _, event: &InputEvent, window, cx| {
                s.on_search(event, window, cx)
            },
        );
        let mut state = Self {
            projects: config.projects.clone(),
            nav: config.nav.clone(),
            local_device: config.local_device.clone(),
            initial: config.tasks.clone(),
            initial_selected: config.selected.clone(),
            events: config.events.clone(),
            played: 0,
            autoplay: config.autoplay,
            clock: 0.0,
            loop_start: 0.0,
            last_tick: Instant::now(),
            rows: Vec::new(),
            next_order: 0,
            filter: config.filter.clone(),
            selected: None,
            page: config.page.clone(),
            activity: config.activity,
            hovered: None,
            shelf_hovered: None,
            collapsed: config.folded.iter().map(|g| g.to_string()).collect(),
            settled_open: false,
            archived_open: false,
            pages: HashMap::new(),
            sort: config.sort,
            grouping: config.grouping,
            show_branch: config.show_branch,
            show_pull_request: config.show_pull_request,
            show_harness: config.show_harness,
            auto_settle: config.auto_settle,
            picker: Popup::default(),
            view: Popup::default(),
            search,
            query: SharedString::default(),
            jump_hints: false,
            focus: cx.focus_handle(),
            view_focus: cx.focus_handle().tab_stop(true),
            view_menu_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            clips: Rc::default(),
            prev_order: Vec::new(),
            resort: HashMap::new(),
            new_keys: HashSet::new(),
            epoch: 0,
            skip_resort: false,
            handlers: config.handlers.clone(),
            _search_events: search_events,
            _ticker: ticker,
        };
        state.load();
        state
    }

    /// (Re)loads the configured tasks and opens the configured row.
    fn load(&mut self) {
        self.played = 0;
        self.loop_start = self.clock;
        let clock = self.clock;
        self.rows = self
            .initial
            .iter()
            .enumerate()
            .map(|(order, task)| Row {
                updated: clock - task.age,
                created: clock - task.created.unwrap_or(task.age),
                task: task.clone(),
                order,
                kept: false,
            })
            .collect();
        self.next_order = self.rows.len();
        self.selected = self.initial_selected.clone();
        if let Some(id) = self.selected.clone() {
            self.mark_seen(&id);
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.clock += now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .min(1.0);
        self.last_tick = now;
        if self.autoplay {
            let t = self.clock - self.loop_start;
            while let Some((at, task)) = self.events.get(self.played).cloned() {
                if t < at {
                    break;
                }
                self.played += 1;
                self.apply(task);
            }
            if t >= DEMO_LOOP {
                self.load();
            }
        }
        cx.notify();
    }

    /// Replaces the task with the same id, or adds it.
    fn apply(&mut self, task: SidebarThread) {
        let updated = self.clock - task.age;
        let reading = self.selected.as_ref() == Some(&task.id);
        if let Some(row) = self.rows.iter_mut().find(|r| r.task.id == task.id) {
            // What arrives in the task being read is read as it arrives.
            let seen = task.seen || reading;
            row.task = SidebarThread {
                seen,
                pinned: row.task.pinned,
                ..task
            };
            row.updated = updated;
        } else {
            let created = self.clock - task.created.unwrap_or(task.age);
            self.rows.push(Row {
                task,
                order: self.next_order,
                updated,
                created,
                kept: false,
            });
            self.next_order += 1;
        }
    }

    fn row_mut(&mut self, id: &SharedString) -> Option<&mut Row> {
        self.rows.iter_mut().find(|r| &r.task.id == id)
    }

    fn mark_seen(&mut self, id: &SharedString) {
        if let Some(row) = self.row_mut(id) {
            row.task.seen = true;
        }
    }

    fn project_index(&self, id: &str) -> Option<usize> {
        self.projects.iter().position(|p| p.id.as_ref() == id)
    }

    fn project_name(&self, id: &str) -> SharedString {
        self.projects
            .iter()
            .find(|p| p.id.as_ref() == id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Unknown project".into())
    }

    fn device_of(&self, task: &SidebarThread) -> SharedString {
        task.device
            .clone()
            .unwrap_or_else(|| self.local_device.clone())
    }

    /// Pink: spaces.rs `is_settled_chat` – a hand-settled finished task, or
    /// an idle one past the auto-settle age.
    fn is_settled(&self, row: &Row) -> bool {
        let task = &row.task;
        if task.archived
            || task.pinned
            || task.needs_you()
            || !matches!(task.indicator(), Indicator::Idle | Indicator::Completed)
        {
            return false;
        }
        if task.settled {
            return true;
        }
        if row.kept || task.indicator() != Indicator::Idle {
            return false;
        }
        self.auto_settle
            .is_some_and(|days| self.clock - row.updated >= f32::from(days) * DAY)
    }

    fn in_filter(&self, row: &Row) -> bool {
        self.filter
            .as_ref()
            .is_none_or(|filter| &row.task.project == filter)
    }

    /// The rows the active list draws, before grouping: filtered, neither
    /// archived nor settled, sorted.
    fn active_rows(&self, filtered: bool) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .rows
            .iter()
            .filter(|r| !r.task.archived && !self.is_settled(r))
            .filter(|r| !filtered || self.in_filter(r))
            .cloned()
            .collect();
        self.sort_rows(&mut rows, self.sort);
        rows
    }

    fn sort_rows(&self, rows: &mut [Row], sort: SidebarSort) {
        rows.sort_by(|l, r| {
            let primary = match sort {
                SidebarSort::LastUpdated => r.updated.total_cmp(&l.updated),
                SidebarSort::Created => r.created.total_cmp(&l.created),
                SidebarSort::Manual => l.order.cmp(&r.order),
            };
            primary.then(l.task.id.cmp(&r.task.id))
        });
    }

    fn settled_rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .rows
            .iter()
            .filter(|r| self.in_filter(r) && self.is_settled(r))
            .cloned()
            .collect();
        self.sort_rows(&mut rows, SidebarSort::LastUpdated);
        rows
    }

    fn archived_rows(&self) -> Vec<Row> {
        let mut rows: Vec<Row> = self
            .rows
            .iter()
            .filter(|r| self.in_filter(r) && r.task.archived)
            .cloned()
            .collect();
        self.sort_rows(&mut rows, SidebarSort::LastUpdated);
        rows
    }

    /// Live counts for one project, or every project together.
    fn counts(&self, project: Option<&SharedString>) -> Counts {
        let mut counts = Counts::default();
        for row in &self.rows {
            if project.is_some_and(|p| &row.task.project != p) {
                continue;
            }
            let age = self.clock - row.updated;
            counts.last = Some(counts.last.map_or(age, |last: f32| last.min(age)));
            if row.task.archived || self.is_settled(row) {
                continue;
            }
            counts.active += 1;
            if row.task.needs_you() {
                counts.need += 1;
            } else if row.task.status == ThreadStatus::Working {
                counts.working += 1;
            }
        }
        counts
    }

    fn limit(&self, key: &str) -> usize {
        self.pages
            .get(key)
            .copied()
            .unwrap_or(SECTION_PAGE)
            .max(SECTION_PAGE)
    }

    /// One section's page: its first `limit` rows, plus the selected row
    /// wherever it falls (Pink: spaces.rs `active_page`).
    fn page_rows(&self, key: &str, rows: Vec<Row>) -> (Vec<Row>, Option<MoreRow>, usize) {
        let limit = self.limit(key);
        let total = rows.len();
        let visible: Vec<Row> = rows
            .into_iter()
            .enumerate()
            .filter(|(ix, r)| *ix < limit || Some(&r.task.id) == self.selected.as_ref())
            .map(|(_, r)| r)
            .collect();
        let more = more_row(total, visible.len(), limit);
        (visible, more, total)
    }

    /// Every keyed entry of the list, in order, with the jump order.
    fn items(&self) -> (Vec<Item>, Vec<SharedString>) {
        let mut items = Vec::new();
        let mut jump: Vec<SharedString> = Vec::new();
        let slot = |id: &SharedString, jump: &mut Vec<SharedString>| {
            jump.push(id.clone());
            (jump.len() <= JUMP_SLOTS).then(|| jump.len() - 1)
        };

        if self.activity {
            // Activity spans every project and is never paged.
            let mut rows: Vec<(Section, Row)> = self
                .rows
                .iter()
                .filter(|r| !r.task.archived)
                .filter_map(|r| {
                    let reading = self.selected.as_ref() == Some(&r.task.id);
                    Section::of(&r.task)
                        .or(reading.then_some(Section::Ready))
                        .map(|s| (s, r.clone()))
                })
                .collect();
            rows.sort_by(|(ls, l), (rs, r)| {
                ls.cmp(rs)
                    .then(r.updated.total_cmp(&l.updated))
                    .then(l.task.id.cmp(&r.task.id))
            });
            let mut current: Option<Section> = None;
            for (ix, (section, row)) in rows.iter().enumerate() {
                if current != Some(*section) {
                    let count = rows[ix..].iter().take_while(|(s, _)| s == section).count();
                    items.push(Item::Heading {
                        section: *section,
                        count,
                        first: current.is_none(),
                    });
                    current = Some(*section);
                }
                let slot = slot(&row.task.id, &mut jump);
                items.push(Item::Row {
                    row: row.clone(),
                    slot,
                });
            }
            return (items, jump);
        }

        let tree = self.filter.is_none();
        let (pinned, rest): (Vec<Row>, Vec<Row>) = self
            .active_rows(true)
            .into_iter()
            .partition(|r| r.task.pinned);

        let group = |key: String,
                     kind: GroupKind,
                     label: SharedString,
                     rows: Vec<Row>,
                     more: Option<MoreRow>,
                     total: usize,
                     counts: Counts,
                     jump: &mut Vec<SharedString>| {
            let collapsed = self.collapsed.contains(&key);
            let rows = rows
                .into_iter()
                .map(|r| {
                    let slot = if collapsed {
                        None
                    } else {
                        slot(&r.task.id, jump)
                    };
                    (r, slot)
                })
                .collect();
            Item::Group(Group {
                key,
                kind,
                label,
                rows,
                collapsed,
                compact: tree,
                more,
                total,
                counts,
            })
        };

        let has_pinned = !pinned.is_empty();
        if has_pinned {
            let total = pinned.len();
            items.push(group(
                "pinned".into(),
                GroupKind::Pinned,
                "Pinned".into(),
                pinned.clone(),
                None,
                total,
                Counts::default(),
                &mut jump,
            ));
            if !rest.is_empty() && !self.collapsed.contains("pinned") {
                items.push(Item::Divider);
            }
        }

        if tree {
            // One group per project, in the picker's order (by name).
            let mut projects: Vec<&SidebarProject> = self.projects.iter().collect();
            projects.sort_by_key(|p| (p.name.to_lowercase(), p.id.clone()));
            for project in projects {
                let rows: Vec<Row> = rest
                    .iter()
                    .filter(|r| r.task.project == project.id)
                    .cloned()
                    .collect();
                if rows.is_empty() {
                    continue;
                }
                let key = format!("project:{}", project.id);
                // The header counts its group, pins aside.
                let mut counts = Counts::default();
                for row in &rows {
                    counts.active += 1;
                    if row.task.needs_you() {
                        counts.need += 1;
                    } else if row.task.status == ThreadStatus::Working {
                        counts.working += 1;
                    }
                }
                let (rows, more, total) = self.page_rows(&key, rows);
                items.push(group(
                    key,
                    GroupKind::Project(project.id.clone()),
                    project.name.clone(),
                    rows,
                    more,
                    total,
                    counts,
                    &mut jump,
                ));
            }
        } else if self.grouping == SidebarGrouping::ByDevice {
            // This machine's group first, then the others as they appear.
            let mut devices: Vec<SharedString> = Vec::new();
            for row in &rest {
                let device = self.device_of(&row.task);
                if !devices.contains(&device) {
                    devices.push(device);
                }
            }
            if let Some(ix) = devices.iter().position(|d| d == &self.local_device) {
                let local = devices.remove(ix);
                devices.insert(0, local);
            }
            for device in devices {
                let rows: Vec<Row> = rest
                    .iter()
                    .filter(|r| self.device_of(&r.task) == device)
                    .cloned()
                    .collect();
                let key = format!("device:{device}");
                let (rows, more, total) = self.page_rows(&key, rows);
                items.push(group(
                    key,
                    GroupKind::Device,
                    device.clone(),
                    rows,
                    more,
                    total,
                    Counts::default(),
                    &mut jump,
                ));
            }
        } else {
            let (rows, more, _) = self.page_rows("active", rest);
            for row in rows {
                let slot = slot(&row.task.id, &mut jump);
                items.push(Item::Row { row, slot });
            }
            if let Some(more) = more {
                items.push(Item::More {
                    section: "active".into(),
                    more,
                });
            }
        }
        (items, jump)
    }

    /// Records the rendered order; a reorder glides the rows that moved and
    /// fades in the new ones (Pink: shell.rs `resort_offsets`).
    fn note_order(&mut self, order: Vec<(String, f32)>) {
        if self.prev_order == order {
            return;
        }
        let skip = std::mem::take(&mut self.skip_resort);
        let reordered = self.prev_order.len() != order.len()
            || self
                .prev_order
                .iter()
                .zip(&order)
                .any(|((old, _), (new, _))| old != new);
        if !self.prev_order.is_empty() && reordered && !skip {
            let mut old_y = HashMap::new();
            let mut y = 0.0;
            for (key, height) in &self.prev_order {
                old_y.insert(key.as_str(), y);
                y += height;
            }
            let mut offsets = HashMap::new();
            let mut new_keys = HashSet::new();
            let mut y = 0.0;
            for (key, height) in &order {
                match old_y.get(key.as_str()) {
                    Some(prev) if (prev - y).abs() > 0.5 => {
                        offsets.insert(key.clone(), prev - y);
                    }
                    Some(_) => {}
                    None => {
                        new_keys.insert(key.clone());
                    }
                }
                y += height;
            }
            if !offsets.is_empty() || !new_keys.is_empty() {
                self.epoch += 1;
                self.resort = offsets;
                self.new_keys = new_keys;
            }
        }
        self.prev_order = order;
    }

    // -- Actions --------------------------------------------------------------

    fn select(&mut self, id: &SharedString) -> Vec<Effect> {
        self.selected = Some(id.clone());
        self.page = None;
        self.mark_seen(id);
        vec![Effect::Select(id.clone())]
    }

    fn act(&mut self, id: &SharedString, action: SidebarAction) -> Vec<Effect> {
        let Some(row) = self.row_mut(id) else {
            return Vec::new();
        };
        match action {
            SidebarAction::Pin => row.task.pinned = true,
            SidebarAction::Unpin => row.task.pinned = false,
            SidebarAction::MarkInactive => {
                row.task.settled = true;
                row.kept = false;
            }
            SidebarAction::MarkActive => {
                row.task.settled = false;
                row.kept = true;
            }
            SidebarAction::Archive => {
                row.task.archived = true;
                row.task.pinned = false;
            }
            SidebarAction::Unarchive => {
                row.task.archived = false;
                row.task.settled = false;
                row.kept = true;
            }
            SidebarAction::OpenRelated => {}
        }
        if matches!(action, SidebarAction::Archive | SidebarAction::MarkInactive)
            && self.hovered.as_ref() == Some(id)
        {
            self.hovered = None;
        }
        vec![Effect::Action(id.clone(), action)]
    }

    fn set_filter(&mut self, filter: Option<SharedString>) -> Vec<Effect> {
        if self.filter == filter {
            return Vec::new();
        }
        // A different project starts every section at five again.
        self.filter = filter.clone();
        self.pages.clear();
        self.hovered = None;
        vec![Effect::Command(SidebarCommand::Filter(filter))]
    }

    fn nav(&mut self, key: &SharedString) -> Vec<Effect> {
        let page = self
            .nav
            .iter()
            .find(|item| &item.key == key)
            .is_some_and(|item| item.page);
        if page {
            self.page = Some(key.clone());
        } else {
            // A new thread opens the empty canvas: no row is current.
            self.page = None;
            self.selected = None;
        }
        vec![Effect::Command(SidebarCommand::Nav(key.clone()))]
    }

    fn toggle_group(&mut self, key: &str) {
        if !self.collapsed.remove(key) {
            self.collapsed.insert(key.to_owned());
        }
        // Collapsing (or reopening) a group shows five rows again.
        self.pages.remove(key);
        self.skip_resort = true;
    }

    fn show_more(&mut self, section: &str, more: MoreRow) {
        match more {
            MoreRow::More(_) => {
                let next = self.limit(section) + SECTION_PAGE;
                self.pages.insert(section.to_owned(), next);
            }
            MoreRow::Less => {
                self.pages.remove(section);
            }
        }
    }

    /// The visible rows top to bottom, for ⌘1–9 and Ctrl-Tab.
    fn jump_order(&self) -> Vec<SharedString> {
        self.items().1
    }

    // -- Popovers -----------------------------------------------------------

    fn popup(&mut self, kind: PopupKind) -> &mut Popup {
        match kind {
            PopupKind::Picker => &mut self.picker,
            PopupKind::View => &mut self.view,
        }
    }

    fn open_popup(&mut self, kind: PopupKind, cursor: Option<usize>, cx: &mut Context<Self>) {
        // One popover at a time.
        let other = match kind {
            PopupKind::Picker => PopupKind::View,
            PopupKind::View => PopupKind::Picker,
        };
        self.begin_close(other, cx);
        let popup = self.popup(kind);
        popup.open = true;
        popup.closing = None;
        popup.reap = None;
        popup.cursor = cursor;
        popup.pending_focus = true;
    }

    /// Starts the exit; `false` when it wasn't open.
    fn begin_close(&mut self, kind: PopupKind, cx: &mut Context<Self>) -> bool {
        let popup = self.popup(kind);
        if !popup.open {
            return false;
        }
        popup.open = false;
        popup.pending_focus = false;
        let since = Instant::now();
        popup.closing = Some(since);
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        popup.reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                let popup = s.popup(kind);
                if popup.closing == Some(since) {
                    popup.closing = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        true
    }

    /// The picker's rows (Pink: project_picker.rs `rows`): All projects on
    /// an empty query, then the projects matching it, names starting with
    /// the query first.
    fn picker_rows(&self) -> Vec<Option<SharedString>> {
        let query = self.query.trim().to_lowercase();
        let mut rows: Vec<Option<SharedString>> = Vec::new();
        if query.is_empty() {
            rows.push(None);
        }
        let mut projects: Vec<&SidebarProject> = self
            .projects
            .iter()
            .filter(|p| query.is_empty() || p.name.to_lowercase().contains(&query))
            .collect();
        projects.sort_by_key(|p| {
            let name = p.name.to_lowercase();
            (
                !query.is_empty() && !name.starts_with(&query),
                name,
                p.id.clone(),
            )
        });
        rows.extend(projects.into_iter().map(|p| Some(p.id.clone())));
        rows
    }

    /// Opens the picker on the current filter's row (Pink: the highlight
    /// starts on the current choice).
    /// The search field takes the keyboard on the next frame.
    fn open_picker(&mut self, cx: &mut Context<Self>) {
        self.search.update(cx, |input, cx| input.clear(cx));
        self.query = SharedString::default();
        let rows = self.picker_rows();
        let cursor = rows.iter().position(|row| row == &self.filter).unwrap_or(0);
        self.open_popup(PopupKind::Picker, Some(cursor), cx);
    }

    /// The picker's slots: its rows, then New project… and Manage projects….
    fn picker_slots(&self) -> usize {
        self.picker_rows().len() + 2
    }

    fn choose_picker_slot(&mut self, ix: usize, cx: &mut Context<Self>) -> Vec<Effect> {
        let rows = self.picker_rows();
        self.begin_close(PopupKind::Picker, cx);
        if let Some(row) = rows.get(ix) {
            self.set_filter(row.clone())
        } else if ix == rows.len() {
            vec![Effect::Command(SidebarCommand::NewProject)]
        } else {
            vec![Effect::Command(SidebarCommand::ManageProjects)]
        }
    }

    /// The picker's keys arrive from its search field.
    fn on_search(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.picker.open {
            return;
        }
        let mut effects = Vec::new();
        match event {
            InputEvent::Change(text) => {
                self.query = text.clone();
                self.picker.cursor = Some(0);
            }
            InputEvent::MoveUp | InputEvent::MoveDown => {
                let delta = if *event == InputEvent::MoveUp { -1 } else { 1 };
                self.picker.cursor = step(self.picker.cursor, self.picker_slots(), delta);
            }
            InputEvent::Submit(_) => {
                if let Some(ix) = self.picker.cursor {
                    effects = self.choose_picker_slot(ix, cx);
                }
                window.focus(&self.focus, cx);
            }
            InputEvent::Escape => {
                self.begin_close(PopupKind::Picker, cx);
                window.focus(&self.focus, cx);
            }
            _ => {}
        }
        cx.notify();
        if !effects.is_empty() {
            let handlers = self.handlers.clone();
            window.defer(cx, move |window, cx| {
                for effect in effects {
                    handlers.emit(effect, window, cx);
                }
            });
        }
    }

    /// The view menu's rows, in Pink's order.
    fn view_rows(&self) -> Vec<ViewRow> {
        ViewRow::ALL.to_vec()
    }

    fn view_checked(&self, row: ViewRow) -> bool {
        match row {
            ViewRow::ByDevice => self.grouping == SidebarGrouping::ByDevice,
            ViewRow::InOneList => self.grouping == SidebarGrouping::InOneList,
            ViewRow::LastUpdated => self.sort == SidebarSort::LastUpdated,
            ViewRow::Created => self.sort == SidebarSort::Created,
            ViewRow::Manual => self.sort == SidebarSort::Manual,
            ViewRow::ShowBranch => self.show_branch,
            ViewRow::ShowPullRequest => self.show_pull_request,
            ViewRow::ShowHarness => self.show_harness,
            ViewRow::SettleOff => self.auto_settle.is_none(),
            ViewRow::Settle1 => self.auto_settle == Some(1),
            ViewRow::Settle3 => self.auto_settle == Some(3),
            ViewRow::Settle7 => self.auto_settle == Some(7),
        }
    }

    /// Pink: spaces.rs `activate_sidebar_view_row` – radio choices close the
    /// menu, Show toggles keep it open for batch changes.
    fn activate_view_row(&mut self, row: ViewRow, cx: &mut Context<Self>) {
        match row {
            ViewRow::ByDevice => self.grouping = SidebarGrouping::ByDevice,
            ViewRow::InOneList => self.grouping = SidebarGrouping::InOneList,
            ViewRow::LastUpdated => self.sort = SidebarSort::LastUpdated,
            ViewRow::Created => self.sort = SidebarSort::Created,
            ViewRow::Manual => self.sort = SidebarSort::Manual,
            ViewRow::ShowBranch => self.show_branch = !self.show_branch,
            ViewRow::ShowPullRequest => self.show_pull_request = !self.show_pull_request,
            ViewRow::ShowHarness => self.show_harness = !self.show_harness,
            ViewRow::SettleOff => self.auto_settle = None,
            ViewRow::Settle1 => self.auto_settle = Some(1),
            ViewRow::Settle3 => self.auto_settle = Some(3),
            ViewRow::Settle7 => self.auto_settle = Some(7),
        }
        if row.closes_menu() {
            self.begin_close(PopupKind::View, cx);
        }
    }
}

/// Pink `popover::menu_step`: wraps, and a first press lands on the first
/// (or, going up, the last) row.
fn step(cursor: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match cursor {
        None if delta < 0 => count - 1,
        None => 0,
        Some(ix) => (ix as isize + delta).rem_euclid(count as isize) as usize,
    })
}

/// The view menu (Pink: spaces.rs `SidebarViewRow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewRow {
    ByDevice,
    InOneList,
    LastUpdated,
    Created,
    Manual,
    ShowBranch,
    ShowPullRequest,
    ShowHarness,
    SettleOff,
    Settle1,
    Settle3,
    Settle7,
}

impl ViewRow {
    const ALL: [ViewRow; 12] = [
        ViewRow::ByDevice,
        ViewRow::InOneList,
        ViewRow::LastUpdated,
        ViewRow::Created,
        ViewRow::Manual,
        ViewRow::ShowBranch,
        ViewRow::ShowPullRequest,
        ViewRow::ShowHarness,
        ViewRow::SettleOff,
        ViewRow::Settle1,
        ViewRow::Settle3,
        ViewRow::Settle7,
    ];

    fn label(self) -> &'static str {
        match self {
            ViewRow::ByDevice => "By device",
            ViewRow::InOneList => "In one list",
            ViewRow::LastUpdated => "Last updated",
            ViewRow::Created => "Created",
            ViewRow::Manual => "Manual",
            ViewRow::ShowBranch => "Branch",
            ViewRow::ShowPullRequest => "Pull request",
            ViewRow::ShowHarness => "Harness",
            ViewRow::SettleOff => "Off",
            ViewRow::Settle1 => "After 1 day",
            ViewRow::Settle3 => "After 3 days",
            ViewRow::Settle7 => "After 7 days",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            ViewRow::ByDevice => ICON_LAPTOP,
            ViewRow::InOneList => ICON_LIST,
            ViewRow::LastUpdated => ICON_CLOCK,
            ViewRow::Created => ICON_CALENDAR,
            ViewRow::Manual => ICON_SORT_VERTICAL,
            ViewRow::ShowBranch => ICON_BRANCH,
            ViewRow::ShowPullRequest => ICON_PULL_REQUEST,
            ViewRow::ShowHarness => ICON_BOT,
            ViewRow::SettleOff => ICON_CLOSE_CIRCLE,
            ViewRow::Settle1 | ViewRow::Settle3 | ViewRow::Settle7 => ICON_CLOCK,
        }
    }

    /// The heading a group of rows opens with.
    fn heading(self) -> Option<&'static str> {
        match self {
            ViewRow::ByDevice => Some("Organize"),
            ViewRow::LastUpdated => Some("Sort"),
            ViewRow::ShowBranch => Some("Show"),
            ViewRow::SettleOff => Some("Move idle threads to Inactive"),
            _ => None,
        }
    }

    fn closes_menu(self) -> bool {
        !matches!(
            self,
            ViewRow::ShowBranch | ViewRow::ShowPullRequest | ViewRow::ShowHarness
        )
    }
}

#[derive(Clone)]
enum GroupKind {
    Pinned,
    Project(SharedString),
    Device,
}

struct Group {
    /// Its collapse and paging key.
    key: String,
    kind: GroupKind,
    label: SharedString,
    rows: Vec<(Row, Option<usize>)>,
    collapsed: bool,
    /// One-line rows (All projects).
    compact: bool,
    more: Option<MoreRow>,
    /// Rows in the group, the ones behind Show more included.
    total: usize,
    counts: Counts,
}

impl Group {
    fn body_height(&self) -> f32 {
        let row_h = if self.compact { COMPACT_ROW_H } else { ROW_H };
        (BODY_INSET + row_h * self.rows.len() as f32) * scale()
            + if self.more.is_some() { more_h() } else { 0.0 }
    }
}

/// One keyed entry of the list.
enum Item {
    /// Pink: activity_center.rs `activity_section_heading`.
    Heading {
        section: Section,
        count: usize,
        first: bool,
    },
    Row {
        row: Row,
        slot: Option<usize>,
    },
    Group(Group),
    /// Pink: the rule under the Pinned group.
    Divider,
    More {
        section: String,
        more: MoreRow,
    },
}

impl Item {
    fn key(&self) -> String {
        match self {
            Item::Heading { section, .. } => format!("h:{}", section.key()),
            Item::Row { row, .. } => format!("c:{}", row.task.id),
            Item::Group(group) => format!("g:{}", group.key),
            Item::Divider => "divider:pinned".into(),
            Item::More { section, .. } => format!("more:{section}"),
        }
    }

    fn height(&self) -> f32 {
        match self {
            Item::Heading { first, .. } => {
                header_h() + if *first { 0.0 } else { SECTION_GAP * scale() }
            }
            Item::Row { .. } => ROW_H * scale(),
            Item::Group(group) => {
                SECTION_GAP * scale()
                    + header_h()
                    + if group.collapsed {
                        0.0
                    } else {
                        group.body_height()
                    }
            }
            Item::Divider => DIVIDER_H * scale(),
            Item::More { .. } => more_h(),
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

thread_local! {
    /// The Interface size over 16 for the sidebar being drawn, set at the
    /// top of its render (Pink: `Theme::ui_scale`, which its `ui_rems` and
    /// row-height math read).
    static SCALE: std::cell::Cell<f32> = const { std::cell::Cell::new(1.0) };
}

/// The Interface scale of the sidebar being drawn.
fn scale() -> f32 {
    SCALE.with(|s| s.get())
}

/// A size at the 16 px default, grown with the Interface size (Pink
/// `typography::ui_rems`). Gutters, horizontal row padding, radii and the
/// `MIN_HIT` and 28 pt chrome buttons stay in plain px, as in Pink.
fn u(n: f32) -> gpui::Pixels {
    px(n * scale())
}

/// A disclosure or shelf header's height (Pink spaces.rs
/// `disclosure_header_height`): it scales with the text but never drops
/// below the 24 pt minimum hit target.
fn header_h() -> f32 {
    header::header_height(scale())
}

/// The Show N more row (Pink spaces.rs: `SIDEBAR_MORE_ROW_HEIGHT`, scaled,
/// at least `MIN_HIT`).
fn more_h() -> f32 {
    (MORE_ROW_H * scale()).max(chrome::MIN_HIT)
}

fn icon(path: &'static str, size: f32, color: Hsla) -> Svg {
    svg().path(path).size(u(size)).flex_none().text_color(color)
}

fn eid(id: String) -> ElementId {
    ElementId::Name(id.into())
}

/// Pink: spaces.rs `status_dot_color` – every state but Done sits well below
/// full strength; a queued send is plain warning.
fn status_color(indicator: Indicator, t: &Pink) -> Hsla {
    match indicator {
        Indicator::Working => t.busy.opacity(0.55),
        Indicator::Approval => t.warning.opacity(0.72),
        Indicator::Input => t.accent.opacity(0.6),
        Indicator::Errored => t.danger.opacity(0.65),
        Indicator::Completed => t.success.opacity(0.9),
        Indicator::Queued => t.warning,
        Indicator::Idle => t.ink(0.14),
    }
}

/// Pink: shell.rs `sidebar_faded_label` – no ellipsis: a clipped label's last
/// glyphs fade out over 20 px into `fade_to`, the row's painted color.
fn faded_label(
    id: String,
    fill: bool,
    label: impl IntoElement,
    fade_to: Hsla,
    clips: &Clips,
) -> Div {
    let clipped = clips.borrow().contains(&id);
    let clips = clips.clone();
    div()
        .when(fill, |el| el.flex_1())
        .relative()
        .min_w_0()
        .overflow_hidden()
        .flex()
        .child(div().flex_none().whitespace_nowrap().child(label))
        // A probe the size of the visible box, read against the label.
        .child(div().absolute().inset_0())
        .when(clipped, |el| {
            el.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(LABEL_FADE))
                    .bg(linear_gradient(
                        90.0,
                        linear_color_stop(fade_to.opacity(0.0), 0.0),
                        linear_color_stop(fade_to, 1.0),
                    )),
            )
        })
        .on_children_prepainted(move |bounds, window, _| {
            let (Some(label), Some(probe)) = (bounds.first(), bounds.get(1)) else {
                return;
            };
            let clipped = label.size.width > probe.size.width + px(0.5);
            let mut clips = clips.borrow_mut();
            if clipped != clips.contains(&id) {
                if clipped {
                    clips.insert(id.clone());
                } else {
                    clips.remove(&id);
                }
                // Read a frame late: draw once more with the fade settled.
                window.request_animation_frame();
            }
        })
}

/// A 20 px row action (Pink: shell.rs `render_chat_row` pin / settle /
/// archive): radius 4, a quiet wash under the pointer, a 12 px glyph.
fn corner_action(
    id: String,
    label: &'static str,
    glyph: &'static str,
    tone: Hsla,
    t: &Pink,
) -> Stateful<Div> {
    div()
        .id(eid(id.clone()))
        .role(Role::Button)
        .aria_label(label)
        .size(px(chrome::MIN_HIT))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_S))
        .cursor_pointer()
        .hover_wash(id, t.fill(FillTier::Tertiary))
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(icon(glyph, chrome::ICON_S, tone))
}

/// What stands for every project at once (Pink: sidebar_projects.rs
/// `all_projects_tile`, #185): a bare folder glyph drawn like the
/// navigation icons, no tile behind it, in a box as wide as a project tile
/// so the label beside it never moves when the scope changes. `glyph` is
/// the folder's size; `leading` starts it on the box's leading edge (the
/// title column) instead of centring it.
fn all_projects_tile(size: f32, glyph: f32, leading: bool, t: &Pink) -> Div {
    div()
        .size(u(size))
        .flex_none()
        .flex()
        .items_center()
        .when(leading, |el| el.justify_start())
        .when(!leading, |el| el.justify_center())
        .child(icon(ICON_ALL_PROJECTS, glyph, t.text_muted))
}

/// A project's tile: its monogram on its hue (Pink `project_tile`), grown
/// with the Interface size.
fn project_tile(index: Option<usize>, name: &str, size: f32, t: &Pink) -> Div {
    pink::monogram_tile(
        pink::project_tint(index.unwrap_or(0)),
        name,
        size * scale(),
        t,
    )
}

/// One live count – a glyph and a tabular number in the tone's floor
/// (Pink: sidebar_projects.rs `project_group_header` `count`).
fn count(tone: Hsla, glyph: AnyElement, n: usize, t: &Pink) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(u(3.0))
        .text_size(u(11.0))
        .font_weight(FontWeight::MEDIUM)
        .font_features(pink::tabular_figures())
        .text_color(t.readable_tone(tone, 1.0))
        .child(glyph)
        .child(SharedString::from(n.to_string()))
}

/// The running count's glyph: a small square, the loader at rest.
fn working_square(t: &Pink) -> AnyElement {
    div()
        .size(u(6.0))
        .rounded(px(1.5))
        .bg(t.busy)
        .into_any_element()
}

/// The row's details card after the hover delay (Pink: shell.rs
/// `SidebarThreadTooltip`): 244 wide, radius 9, the title over its project,
/// machine, agent, branch and state.
#[derive(Clone)]
struct TaskDetails {
    title: SharedString,
    project: SharedString,
    device: SharedString,
    model: Option<SharedString>,
    branch: Option<SharedString>,
    status: Option<SharedString>,
}

impl Render for TaskDetails {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let detail = |glyph: &'static str, label: SharedString| {
            div()
                .flex()
                .items_center()
                .gap(u(8.0))
                .min_w_0()
                .child(icon(glyph, 13.0, t.text_muted))
                .child(div().min_w_0().truncate().child(label))
        };
        div()
            .w(u(244.0))
            .px(px(12.0))
            .py(u(11.0))
            .flex()
            .flex_col()
            .gap(u(7.0))
            .rounded(px(9.0))
            .border_1()
            .border_color(t.border_strong)
            .bg(t.tooltip_surface())
            .shadow(t.tooltip_shadows())
            .font_family(pink::SANS)
            .text_size(u(11.0))
            .text_color(t.text_muted)
            .child(
                div()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_size(u(12.0))
                    .text_color(t.text)
                    .child(self.title.clone()),
            )
            .child(detail(ICON_FOLDER, self.project.clone()))
            .child(detail(ICON_LAPTOP, self.device.clone()))
            .when_some(self.model.clone(), |el, model| {
                el.child(detail(ICON_WIDGET, model))
            })
            .when_some(self.branch.clone(), |el, branch| {
                el.child(detail(ICON_BRANCH, branch))
            })
            .when_some(self.status.clone(), |el, status| {
                el.child(detail(ICON_CHECK, status))
            })
    }
}

/// What every part of one render shares.
struct RowCx<'a> {
    key: &'a str,
    t: &'a Pink,
    state: &'a Entity<SidebarState>,
    s: &'a SidebarState,
    clips: &'a Clips,
    /// The sidebar surface the rows sit on.
    surface: Hsla,
}

impl RowCx<'_> {
    fn details(&self, row: &Row, status: Option<&str>) -> TaskDetails {
        let task = &row.task;
        TaskDetails {
            title: task.title.clone(),
            project: self.s.project_name(&task.project),
            device: self.s.device_of(task),
            model: task.harness.map(|h| match &task.model {
                Some(model) => format!("{model} on {}", h.provider()).into(),
                None => h.provider().into(),
            }),
            branch: task.branch.clone(),
            status: status.map(SharedString::from),
        }
    }
}

/// Runs a state change from a listener, then tells the host.
fn run(
    state: &Entity<SidebarState>,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut SidebarState, &mut Context<SidebarState>) -> Vec<Effect>,
) {
    let (effects, handlers) = state.update(cx, |s, cx| {
        s.autoplay = false;
        let effects = f(s, cx);
        cx.notify();
        (effects, s.handlers.clone())
    });
    for effect in effects {
        handlers.emit(effect, window, cx);
    }
}

// ---------------------------------------------------------------------------
// Rows
// ---------------------------------------------------------------------------

/// One task row (Pink: shell.rs `Shell::render_chat_row`). Two-line: 52
/// tall, `px 8 py 6`, radius 8; the title over the meta line, the time (or
/// the hover actions) in the corner, the status at the right edge. Compact
/// (All projects): 32 tall, `pl 23`, the status on the title line.
fn task_row(
    c: &RowCx,
    row: &Row,
    compact: bool,
    device_tag: Option<SharedString>,
    slot: Option<usize>,
) -> AnyElement {
    let t = c.t;
    let s = c.s;
    let task = &row.task;
    let row_id = format!("{}-row-{}", c.key, task.id);
    let fade_key = format!("{row_id}-hover");
    let selected = s.page.is_none() && s.selected.as_ref() == Some(&task.id);
    let corner_hovered = s.hovered.as_ref() == Some(&task.id);
    let indicator = task.indicator();
    let tone = status_color(indicator, t);

    // Working keeps the square loader and Done its dot; a task waiting on
    // you (or a send that did not go out) names the next step in one pill
    // with the glyph of what it waits for. A compact row spends no width on
    // the pill's cloth: the glyph and coloured word alone.
    let pill = |word: &'static str, glyph: Option<&'static str>| {
        let tone = tone.opacity(1.0);
        let backing = tone.opacity(if compact { 0.0 } else { 0.12 });
        div()
            .h(u(16.0))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(u(3.0))
            .when(!compact, |pill| pill.px(px(5.0)))
            .rounded(px(radius::XS))
            .bg(backing)
            .text_size(u(11.0))
            .line_height(u(14.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.readable_on(tone, backing, pink::TEXT_CONTRAST_TARGET))
            .when_some(glyph, |pill, glyph| {
                pill.child(icon(
                    glyph,
                    11.0,
                    t.readable_on(tone, backing, pink::NON_TEXT_CONTRAST_TARGET),
                ))
            })
            .child(word)
            .into_any_element()
    };
    let status_badge = match indicator {
        Indicator::Working => Some(
            loaders::mini_glyph_spinner(format!("{row_id}-working"), 2.0, t.glyph)
                .into_any_element(),
        ),
        Indicator::Completed => Some(
            div()
                .size(u(6.0))
                .flex_none()
                .rounded_full()
                .bg(tone)
                .into_any_element(),
        ),
        Indicator::Approval => Some(pill("Approve", Some(ICON_SHIELD))),
        Indicator::Input => Some(pill("Reply", Some(ICON_QUESTION))),
        Indicator::Errored => Some(pill("Failed", Some(ICON_DANGER))),
        Indicator::Queued => Some(pill("Queued", None)),
        Indicator::Idle => None,
    }
    .map(|badge| div().flex_none().flex().items_center().child(badge));
    let (line_status, meta_status) = if compact {
        (status_badge, None)
    } else {
        (None, status_badge)
    };

    // The wash and the title brighten ride one 150 ms blend; a selected row
    // never drifts toward the hover wash.
    let text = t.text;
    let selected_wash = t.glass_selected_bg();
    let rest_bg = if selected { selected_wash } else { t.wash(0.0) };
    let hover_bg = if selected {
        selected_wash
    } else {
        t.sidebar_hover_bg()
    };
    let rest_text = if selected {
        text
    } else if indicator == Indicator::Completed {
        text.opacity(0.95)
    } else {
        text.opacity(0.8)
    };
    let hover = motion::hover_t_or(&fade_key, if corner_hovered { 1.0 } else { 0.0 });
    let row_bg = motion::mix(rest_bg, hover_bg, hover);
    let fade_to = pink::flatten(row_bg, c.surface);
    // Project, branch and note lines are information: quiet, but held to
    // the text floor.
    let subline = t.readable_muted(0.5);

    // The corner: the jump key while ⌘ is held, the row's actions under the
    // pointer, the time otherwise.
    let corner_body = if let Some(slot) = slot.filter(|_| s.jump_hints) {
        let tone = t.text_muted;
        div()
            .h(u(16.0))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .px(px(4.0))
            .rounded(px(radius::XS))
            .bg(tone.opacity(0.08))
            .text_size(u(11.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.readable_tone(tone, 0.85))
            .font_family(pink::MONO)
            .child(format!("⌘{}", slot + 1))
            .into_any_element()
    } else if corner_hovered {
        let pinned = task.pinned;
        let action = |id: &SharedString, action: SidebarAction| {
            let state = c.state.clone();
            let id = id.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                run(&state, window, cx, |s, _| s.act(&id, action));
            }
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .gap(u(2.0))
            .h(u(18.0))
            .when(!task.archived, |el| {
                el.child(
                    corner_action(
                        format!("{row_id}-pin"),
                        if pinned { "Unpin thread" } else { "Pin thread" },
                        ICON_PIN,
                        if pinned {
                            t.readable_icon(t.accent)
                        } else {
                            t.text_muted
                        },
                        t,
                    )
                    .on_click(action(
                        &task.id,
                        if pinned {
                            SidebarAction::Unpin
                        } else {
                            SidebarAction::Pin
                        },
                    )),
                )
            })
            .when(task.can_settle(), |el| {
                el.child(
                    corner_action(
                        format!("{row_id}-settle"),
                        "Mark as inactive",
                        ICON_CHECK,
                        t.text_muted,
                        t,
                    )
                    .on_click(action(&task.id, SidebarAction::MarkInactive)),
                )
            })
            .child(
                corner_action(
                    format!("{row_id}-archive"),
                    "Archive thread",
                    ICON_ARCHIVE,
                    t.text_muted,
                    t,
                )
                .on_click(action(&task.id, SidebarAction::Archive)),
            )
            .into_any_element()
    } else {
        // At rest the corner is the time alone.
        div()
            .h(u(18.0))
            .flex()
            .items_center()
            .text_size(u(11.0))
            .font_features(pink::tabular_figures())
            .child(time_ago(s.clock - row.updated))
            .into_any_element()
    };
    let corner = div()
        .id(eid(format!("{row_id}-corner")))
        .flex_none()
        .h(u(18.0))
        .flex()
        .items_center()
        .justify_end()
        .child(corner_body);

    // A chip that opens the threads this one started. It draws 16 tall but
    // takes a `MIN_HIT` target, reaching into the row's padding so the
    // title line keeps its height.
    let related = (task.related > 0).then(|| {
        let tone = t.text_muted;
        let n = task.related;
        let label: SharedString =
            format!("Open {n} related thread{}", if n == 1 { "" } else { "s" }).into();
        let chip_id = format!("{row_id}-related");
        let ring = t.focus_ring();
        let state = c.state.clone();
        let id = task.id.clone();
        let outset = ((chrome::MIN_HIT - 16.0 * scale()) / 2.0).max(0.0);
        div()
            .id(eid(chip_id.clone()))
            .role(Role::Button)
            .aria_label(label.clone())
            .tab_index(0)
            .h(px(chrome::MIN_HIT.max(16.0 * scale())))
            .my(px(-outset))
            .flex_none()
            .flex()
            .items_center()
            .rounded(px(radius::XS))
            .cursor_pointer()
            .focus_visible(move |st| pink::focus_ring(st, ring))
            .on_hover(motion::hover_listener(chip_id.clone()))
            .tooltip(pink::text_tooltip(label))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                run(&state, window, cx, |s, _| {
                    s.act(&id, SidebarAction::OpenRelated)
                });
            })
            .on_key_down(pink::swallow_activation_key)
            .child(
                div()
                    .h(u(16.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(u(3.0))
                    .px(px(4.0))
                    .rounded(px(radius::XS))
                    .border_1()
                    .border_color(gpui::transparent_black())
                    .bg(motion::hover_blend(
                        &chip_id,
                        tone.opacity(0.08),
                        tone.opacity(0.16),
                    ))
                    .text_size(u(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.readable_tone(tone, 0.85))
                    .child(icon(ICON_CHAT, 10.0, t.readable_tone(tone, 0.7)))
                    .child(n.to_string()),
            )
    });

    // An open workbench tab.
    let open_chip = task.open_tab.then(|| {
        let tone = t.accent;
        let backing = tone.opacity(0.10);
        div()
            .h(u(16.0))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(u(3.0))
            .px(px(4.0))
            .rounded(px(radius::XS))
            .bg(backing)
            .text_size(u(11.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.readable_on(tone, backing, pink::TEXT_CONTRAST_TARGET))
            .child(icon(
                ICON_EYE,
                10.0,
                t.readable_on(tone, backing, pink::NON_TEXT_CONTRAST_TARGET),
            ))
            .child("Open")
    });

    // A pin names its project, a task elsewhere its machine (compact only).
    let device_chip = device_tag.filter(|_| compact).map(|tag| {
        div()
            .h(u(16.0))
            .flex_none()
            .flex()
            .items_center()
            .px(px(4.0))
            .rounded(px(radius::XS))
            .bg(t.text.opacity(0.06))
            .text_size(u(10.5))
            .text_color(subline)
            .child(tag)
    });

    // Line 2: the harness, project and branch on the title's left edge,
    // the PR badge and the status at the right.
    let meta = (!compact).then(|| {
        let show_project = s.filter.is_none();
        let branch = task.branch.clone().filter(|_| s.show_branch);
        let harness = task.harness.filter(|_| s.show_harness).map(|h| {
            icon(
                h.mark(),
                META_HARNESS,
                if t.dark {
                    h.tint().unwrap_or(t.readable_muted(0.5)).opacity(0.8)
                } else {
                    t.text_muted
                },
            )
        });
        let pull_request =
            task.pull_request
                .filter(|_| s.show_pull_request)
                .map(|(number, state)| {
                    let color = match state {
                        PullRequestState::Open => t.success,
                        PullRequestState::Merged => t.code_text,
                        PullRequestState::Closed => t.danger,
                    };
                    let id = format!("{row_id}-pr");
                    div()
                        .id(eid(id.clone()))
                        .h(u(16.0))
                        .flex_none()
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(px(4.0))
                        .rounded(px(radius::XS))
                        .text_size(u(10.0))
                        .font_weight(FontWeight::MEDIUM)
                        .cursor_pointer()
                        .hover_fill_fg(
                            id,
                            color.opacity(0.08),
                            color.opacity(0.16),
                            t.readable_tone(color, 0.85),
                            color,
                        )
                        .tooltip(pink::text_tooltip(format!(
                            "Pull request #{number}, {}",
                            match state {
                                PullRequestState::Open => "Open",
                                PullRequestState::Merged => "Merged",
                                PullRequestState::Closed => "Closed",
                            }
                        )))
                        .tooltip_show_delay(layout::TOOLTIP_DELAY)
                        .child(div().font_family(pink::MONO).child(format!("#{number}")))
                });
        div()
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(u(6.0))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(u(4.0))
                    .text_size(u(11.0))
                    .line_height(u(14.0))
                    .text_color(subline)
                    .children(harness)
                    .when(show_project, |el| {
                        el.child(faded_label(
                            format!("{row_id}-project"),
                            false,
                            s.project_name(&task.project),
                            fade_to,
                            c.clips,
                        ))
                    })
                    .when_some(branch, |el, branch| {
                        el.child(icon(ICON_BRANCH, 11.0, subline))
                            .child(faded_label(
                                format!("{row_id}-branch"),
                                false,
                                branch,
                                fade_to,
                                c.clips,
                            ))
                    }),
            )
            .children(pull_request)
            .children(meta_status)
    });

    let details = c.details(row, (indicator == Indicator::Completed).then_some("Done"));
    let open_id = task.id.clone();
    let hover_id = task.id.clone();
    div()
        .id(eid(row_id.clone()))
        .h(u(if compact { COMPACT_ROW_H } else { ROW_H }))
        .flex()
        .flex_col()
        .justify_center()
        .gap(u(2.0))
        .rounded(px(radius::MD))
        .px(px(space::SM))
        // One title column in every scope (Pink #181, D48 revised): a row
        // under a project header is not indented, so a selected row has the
        // same text inset on both sides and switching scope never moves a
        // title.
        .py(u(6.0))
        .text_color(motion::mix(rest_text, text, hover))
        .bg(row_bg)
        // One listener drives both the wash blend and the corner's swap.
        .on_hover({
            let fade = motion::hover_listener(fade_key);
            let state = c.state.clone();
            move |hovered: &bool, window, cx| {
                fade(hovered, window, cx);
                state.update(cx, |s, cx| {
                    if *hovered {
                        if s.hovered.as_ref() != Some(&hover_id) {
                            s.hovered = Some(hover_id.clone());
                            cx.notify();
                        }
                    } else if s.hovered.as_ref() == Some(&hover_id) {
                        s.hovered = None;
                        cx.notify();
                    }
                });
            }
        })
        .cursor_pointer()
        .role(Role::Button)
        .aria_selected(selected)
        .aria_label(task.title.clone())
        .on_click({
            let state = c.state.clone();
            move |_, window, cx| run(&state, window, cx, |s, _| s.select(&open_id))
        })
        .tooltip(move |_, cx| cx.new(|_| details.clone()).into())
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        // Line 1: the title, then the time (or the hover actions).
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(u(MARK_GAP))
                .child(faded_label(
                    format!("{row_id}-title"),
                    true,
                    div()
                        .text_size(u(13.0))
                        .line_height(u(17.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(task.title.clone()),
                    fade_to,
                    c.clips,
                ))
                .children(related)
                .children(open_chip)
                .children(line_status)
                .children(device_chip)
                .child(div().text_color(subline).child(corner)),
        )
        .children(meta)
        .into_any_element()
}

/// A disclosure chevron: alt-arrow-right 12, turned down as the group
/// opens (Pink: spaces.rs `sidebar_disclosure_chevron`).
fn chevron(reveal: f32, t: &Pink) -> Svg {
    icon(ICON_CHEVRON, 12.0, t.readable_icon(t.readable_muted(0.5)))
        .with_transformation(Transformation::rotate(radians(reveal * FRAC_PI_2)))
}

/// A chevron at a row's trailing edge (Pink: spaces.rs `trailing_chevron`,
/// #185): the glyph's empty space is pulled back so the arrow ends on the
/// time column above it.
fn trailing_chevron(chevron: Svg) -> Div {
    header::trailing_chevron().child(chevron)
}

/// Empty space a leading glyph draws before its first stroke at its shelf
/// size, measured from the rendered sidebar and pulled back so the stroke
/// starts on the title column (Pink: spaces.rs `leading_bearing`). Glyphs
/// that fill their box need none.
fn leading_bearing(glyph: &str) -> f32 {
    if glyph == ICON_CHECK { 2.5 } else { 0.0 }
}

/// A group of rows under a folding header (Pink: spaces.rs
/// `render_active_rows` groups): Pinned and device groups wear the quiet
/// disclosure header, project groups the project header.
fn group(c: &RowCx, group: &Group) -> AnyElement {
    let header_id = format!("{}-group-{}", c.key, group.key);
    let reveal = motion::follow(
        &format!("{header_id}-open"),
        if group.collapsed { 0.0 } else { 1.0 },
        COLLAPSE,
    );
    let toggle = {
        let state = c.state.clone();
        let key = group.key.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            run(&state, window, cx, |s, _| {
                s.toggle_group(&key);
                vec![Effect::Command(SidebarCommand::Fold {
                    group: key.clone().into(),
                    folded: s.collapsed.contains(&key),
                })]
            })
        }
    };
    let header = match &group.kind {
        GroupKind::Project(project) => {
            project_header(c, &header_id, project, group, reveal).on_click(toggle)
        }
        GroupKind::Pinned | GroupKind::Device => {
            let label: SharedString = if group.collapsed {
                format!("{} ({})", group.label, group.total).into()
            } else {
                group.label.clone()
            };
            // A named button with its count and state (Pink #182):
            // "Pinned, 1 thread".
            let spoken = format!(
                "{}, {} {}",
                group.label,
                group.total,
                if group.total == 1 {
                    "thread"
                } else {
                    "threads"
                }
            );
            disclosure_header(c, &header_id, label, reveal, !group.collapsed)
                .aria_label(spoken)
                .on_click(toggle)
        }
    };
    let pinned = matches!(group.kind, GroupKind::Pinned);
    let rows = group.rows.iter().map(|(row, slot)| {
        let tag = if !group.compact {
            None
        } else if pinned {
            Some(c.s.project_name(&row.task.project))
        } else {
            row.task.device.clone()
        };
        task_row(c, row, group.compact, tag, *slot)
    });
    // A folded body at rest draws nothing, so its rows also leave the
    // accessibility tree instead of being read while hidden (Pink #182).
    let folded = group.collapsed && reveal < 0.001;
    let body = div()
        .w_full()
        .flex_none()
        .overflow_hidden()
        .h(px(group.body_height() * reveal))
        .opacity(0.35 + 0.65 * reveal)
        .relative()
        .top(px(-3.0 * (1.0 - reveal)))
        .when(!folded, |body| {
            body.child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .pt(u(BODY_INSET))
                    .children(rows)
                    .children(group.more.map(|more| more_row_el(c, &group.key, more))),
            )
        });
    div()
        .w_full()
        .flex()
        .flex_col()
        .pt(u(SECTION_GAP))
        .child(header)
        .child(body)
        .into_any_element()
}

/// Pink: spaces.rs `sidebar_disclosure_header` – a small semibold label, no
/// rule, and the chevron only under the pointer.
fn disclosure_header(
    c: &RowCx,
    id: &str,
    label: SharedString,
    reveal: f32,
    open: bool,
) -> Stateful<Div> {
    let t = c.t;
    let group: SharedString = format!("{id}-hdr").into();
    // The package's header (`pink-section-header`).
    header::disclosure_header(scale())
        .id(eid(id.to_owned()))
        .group(group.clone())
        .role(Role::Button)
        .aria_expanded(open)
        .hover_wash(id.to_owned(), t.fill(FillTier::Tertiary))
        .child(faded_label(
            format!("{id}-label"),
            false,
            header::disclosure_label(t.readable_faint(0.9), label),
            c.surface,
            c.clips,
        ))
        .child(div().flex_1())
        .child(header::reveal_on_hover(group).child(chevron(reveal, t)))
}

/// An All projects group's header (Pink: sidebar_projects.rs
/// `project_group_header`, #181 and #185): the project's tile and name on
/// the rows' title column, what needs you and what runs, the open count
/// while folded, a New thread button for that project under the pointer
/// (a full `MIN_HIT` target), and the disclosure chevron at the trailing
/// end, shown while folded or under the pointer. The rows under it are not
/// indented: the header carries the grouping.
fn project_header(
    c: &RowCx,
    id: &str,
    project: &SharedString,
    group: &Group,
    reveal: f32,
) -> Stateful<Div> {
    let t = c.t;
    let counts = group.counts;
    let hover_group: SharedString = format!("{id}-hdr").into();
    let new_id = format!("{id}-new-thread");
    let new_label: SharedString = format!("New thread in {}", group.label).into();
    let spoken = if counts.active == 0 {
        "No open threads".to_owned()
    } else {
        Counts {
            active: group.total,
            ..counts
        }
        .summary()
    };
    let state = c.state.clone();
    let new_project = project.clone();
    let hover_group_chevron = hover_group.clone();
    div()
        .id(eid(id.to_owned()))
        .group(hover_group.clone())
        .role(Role::Button)
        .aria_label(format!("{}, {spoken}", group.label))
        .aria_expanded(!group.collapsed)
        .flex()
        .flex_row()
        .items_center()
        .gap(u(7.0))
        .h(px(header_h()))
        .px(px(space::SM))
        .rounded(px(radius::SM))
        .cursor_pointer()
        .hover_wash(id.to_owned(), t.fill(FillTier::Tertiary))
        .child(project_tile(
            c.s.project_index(project),
            &group.label,
            18.0,
            t,
        ))
        .child(faded_label(
            format!("{id}-label"),
            false,
            div()
                .text_size(u(12.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(group.label.clone()),
            c.surface,
            c.clips,
        ))
        .when(counts.need > 0, |el| {
            el.child(count(
                t.warning,
                icon(ICON_SHIELD, 11.0, t.readable_icon(t.warning)).into_any_element(),
                counts.need,
                t,
            ))
        })
        .when(counts.working > 0, |el| {
            el.child(count(t.busy, working_square(t), counts.working, t))
        })
        .child(div().flex_1())
        .when(group.collapsed, |el| {
            el.child(
                div()
                    .flex_none()
                    .text_size(u(11.0))
                    .text_color(t.readable_faint(0.9))
                    .child(format!("{} open", group.total)),
            )
        })
        .child(
            div()
                .id(eid(new_id.clone()))
                .role(Role::Button)
                .aria_label(new_label.clone())
                .size(px(chrome::MIN_HIT))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(chrome::RADIUS_S))
                .opacity(0.0)
                .group_hover(hover_group, |st| st.opacity(1.0))
                .hover_wash(new_id, t.fill(FillTier::Tertiary))
                .tooltip(pink::text_tooltip(new_label))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let project = new_project.clone();
                    run(&state, window, cx, |_, _| {
                        vec![Effect::Command(SidebarCommand::NewThread(project))]
                    });
                })
                .child(icon(ICON_PLUS, chrome::ICON_S, t.text_muted)),
        )
        // Trailing disclosure: always shown while folded so a folded group
        // reads as folded, otherwise only under the pointer.
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .size(u(16.0))
                .opacity(if group.collapsed { 1.0 } else { 0.0 })
                .group_hover(hover_group_chevron, |st| st.opacity(1.0))
                .child(trailing_chevron(chevron(reveal, t))),
        )
}

/// A section's trailing "Show N more" or "Show less" row (Pink: spaces.rs
/// `render_sidebar_more_row`, #185): the glyph starts on the title column at
/// the leading edge of its mark slot, and the row is never shorter than the
/// minimum hit target.
fn more_row_el(c: &RowCx, section: &str, more: MoreRow) -> AnyElement {
    let t = c.t;
    let id = format!("{}-more-{section}", c.key);
    let (glyph, label, aria): (&'static str, SharedString, SharedString) = match more {
        MoreRow::More(n) => (
            ICON_ARROW_DOWN,
            format!("Show {n} more").into(),
            format!("Show {n} more threads").into(),
        ),
        MoreRow::Less => (
            ICON_ARROW_UP,
            "Show less".into(),
            "Show fewer threads".into(),
        ),
    };
    let rest = t.readable_muted(0.75);
    let state = c.state.clone();
    let section = section.to_owned();
    let ring = t.focus_ring();
    div()
        .id(eid(id.clone()))
        .role(Role::Button)
        .aria_label(aria)
        .tab_index(0)
        .h(px(more_h()))
        .w_full()
        .flex_none()
        .px(px(space::SM))
        .flex()
        .flex_row()
        .items_center()
        .gap(u(MARK_GAP))
        .rounded(px(radius::SM))
        .cursor_pointer()
        .text_size(u(12.0))
        .focus_visible(move |st| pink::focus_ring(st, ring))
        .hover_fill_fg(
            id.clone(),
            gpui::transparent_black(),
            t.glass_hover(),
            rest,
            t.text,
        )
        .on_click(move |_, window, cx| {
            run(&state, window, cx, |s, _| {
                s.show_more(&section, more);
                Vec::new()
            })
        })
        .on_key_down(pink::swallow_activation_key)
        .child(
            div()
                .w(u(MARK_SLOT))
                .flex_none()
                .flex()
                // The glyph starts on the title column, like the titles and
                // project tiles above it.
                .justify_start()
                .child(icon(glyph, 12.0, motion::hover_blend(&id, rest, t.text))),
        )
        .child(label)
        .into_any_element()
}

/// One Activity section's heading (Pink: activity_center.rs
/// `activity_section_heading`): a heading, not a disclosure – what needs you
/// never folds away.
fn section_heading(c: &RowCx, section: Section, count: usize, first: bool) -> AnyElement {
    let t = c.t;
    let threads = if count == 1 { "thread" } else { "threads" };
    div()
        .w_full()
        .flex_none()
        .flex()
        .flex_col()
        .pt(u(if first { 0.0 } else { SECTION_GAP }))
        .child(
            div()
                .id(eid(format!("{}-section-{}", c.key, section.key())))
                .role(Role::Heading)
                .aria_label(format!("{}, {count} {threads}", section.label()))
                .h(px(header_h()))
                .px(px(space::SM))
                .flex()
                .flex_row()
                .items_center()
                .gap(u(8.0))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(u(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.readable_faint(0.9))
                        .child(section.label()),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(u(11.0))
                        .text_color(t.readable_faint(0.7))
                        .child(count.to_string()),
                ),
        )
        .into_any_element()
}

/// Pink: the quiet rule shared by sidebar groups (spaces.rs
/// `sidebar_separator`): one pixel of `border @ 0.6`.
fn separator(t: &Pink) -> Div {
    div().h(u(1.0)).bg(t.border.opacity(0.6))
}

// ---------------------------------------------------------------------------
// Shelves
// ---------------------------------------------------------------------------

/// A shelf's disclosure row (Pink: spaces.rs `sidebar_shelf_header`, #185):
/// the glyph starting on the title column, a 12 medium label, the count and
/// the chevron ending on the time column.
fn shelf_header(
    c: &RowCx,
    key: &'static str,
    label: &'static str,
    glyph: &'static str,
    total: usize,
    open: bool,
) -> Stateful<Div> {
    let t = c.t;
    let id = format!("{}-{key}-toggle", c.key);
    let rest = t.readable_muted(0.68);
    let reveal = motion::follow(
        &format!("{}-{key}-open", c.key),
        if open { 1.0 } else { 0.0 },
        COLLAPSE,
    );
    let ring = t.focus_ring();
    let state = c.state.clone();
    // The package's header (`pink-section-header`).
    header::shelf_header(scale())
        .id(eid(id.clone()))
        .role(Role::Button)
        .aria_label(format!(
            "{label}, {total} {}",
            if total == 1 { "thread" } else { "threads" }
        ))
        .aria_expanded(open)
        .tab_index(0)
        .focus_visible(move |st| pink::focus_ring(st, ring))
        .hover_fill_fg(
            id.clone(),
            gpui::transparent_black(),
            t.glass_hover(),
            rest,
            t.text,
        )
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, window, cx| {
            run(&state, window, cx, |s, _| {
                match key {
                    "settled" => s.settled_open = !s.settled_open,
                    _ => s.archived_open = !s.archived_open,
                }
                // Either way it comes back at five rows.
                s.pages.remove(key);
                Vec::new()
            })
        })
        .child(
            // The glyph's stroke starts on the title column.
            header::mark_slot().child(
                icon(
                    glyph,
                    header::SHELF_GLYPH,
                    t.readable_icon(motion::hover_blend(
                        &id,
                        t.readable_muted(0.5),
                        t.text_muted,
                    )),
                )
                .ml(-u(leading_bearing(glyph))),
            ),
        )
        .child(header::shelf_label(label))
        .child(
            header::shelf_count(t.readable_faint(0.9))
                .font_features(pink::tabular_figures())
                .child(total.to_string()),
        )
        .child(trailing_chevron(chevron(reveal, t)))
}

/// A shelf's folding body: its rows and its more row.
fn shelf_body(
    c: &RowCx,
    key: &'static str,
    open: bool,
    rows: Vec<AnyElement>,
    more: Option<MoreRow>,
) -> AnyElement {
    let reveal = motion::follow(
        &format!("{}-{key}-open", c.key),
        if open { 1.0 } else { 0.0 },
        COLLAPSE,
    );
    // Pink: spaces.rs `shelf_body_height`.
    let height = (2.0 * BODY_INSET + rows.len() as f32 * SHELF_ROW_H) * scale()
        + if more.is_some() { more_h() } else { 0.0 };
    div()
        .w_full()
        .flex_none()
        .overflow_hidden()
        .h(px(height * reveal))
        .opacity(0.35 + 0.65 * reveal)
        .relative()
        .top(px(-3.0 * (1.0 - reveal)))
        // Folded at rest, the rows leave the accessibility tree (Pink #182).
        .when(open || reveal > 0.001, |body| {
            body.child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .py(u(BODY_INSET))
                    .children(rows)
                    .children(more.map(|more| more_row_el(c, key, more))),
            )
        })
        .into_any_element()
}

/// The shared mark slot holding a task's harness mark (Pink: the settled
/// and archived rows' leading column).
fn shelf_mark(c: &RowCx, task: &SidebarThread, alpha: f32) -> Div {
    let t = c.t;
    div()
        .w(u(MARK_SLOT))
        .flex_none()
        .flex()
        .justify_center()
        .when_some(task.harness.filter(|_| c.s.show_harness), |el, h| {
            el.child(icon(
                h.mark(),
                SHELF_HARNESS,
                h.tint().unwrap_or(t.text_muted).opacity(alpha),
            ))
        })
}

/// One settled row (Pink: spaces.rs `render_settled_section`): one line, 36
/// tall, the mark slot, the title and the time.
fn settled_row(c: &RowCx, row: &Row) -> AnyElement {
    let t = c.t;
    let task = &row.task;
    let id = format!("{}-settled-{}", c.key, task.id);
    let selected = c.s.page.is_none() && c.s.selected.as_ref() == Some(&task.id);
    let wash = if selected {
        t.glass_selected_bg()
    } else {
        gpui::transparent_black()
    };
    let hover = if selected {
        t.glass_selected_bg()
    } else {
        t.glass_hover()
    };
    let fade_to = pink::flatten(motion::hover_blend(&id, wash, hover), c.surface);
    let details = c.details(row, Some("Inactive"));
    let state = c.state.clone();
    let open_id = task.id.clone();
    let hovered = c.s.shelf_hovered.as_ref() == Some(&task.id);
    // Under the pointer, Mark as active takes the time's place (Pink
    // shell.rs: the restart glyph on an inactive thread).
    let right = if hovered {
        let state = c.state.clone();
        let restore = task.id.clone();
        let restore_id = format!("{id}-activate");
        div()
            .id(eid(restore_id.clone()))
            .role(Role::Button)
            .aria_label("Mark as active")
            .flex()
            .items_center()
            .justify_center()
            .size(px(chrome::MIN_HIT))
            .rounded(px(chrome::RADIUS_S))
            .hover_wash(restore_id, t.fill(FillTier::Tertiary))
            .tooltip(pink::text_tooltip("Mark as active"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                run(&state, window, cx, |s, _| {
                    s.shelf_hovered = None;
                    s.act(&restore, SidebarAction::MarkActive)
                });
            })
            .child(icon(ICON_RESTART, 13.0, t.text_muted))
            .into_any_element()
    } else {
        div()
            .text_size(u(11.0))
            .text_color(t.readable_muted(0.55))
            .child(time_ago(c.s.clock - row.updated))
            .into_any_element()
    };
    let hover_state = c.state.clone();
    let hover_id = task.id.clone();
    let fade_key = id.clone();
    div()
        .id(eid(id.clone()))
        .on_hover({
            let fade = motion::hover_listener(fade_key);
            move |entered: &bool, window, cx| {
                fade(entered, window, cx);
                hover_state.update(cx, |s, cx| {
                    if *entered {
                        if s.shelf_hovered.as_ref() != Some(&hover_id) {
                            s.shelf_hovered = Some(hover_id.clone());
                            cx.notify();
                        }
                    } else if s.shelf_hovered.as_ref() == Some(&hover_id) {
                        s.shelf_hovered = None;
                        cx.notify();
                    }
                });
            }
        })
        .flex_none()
        .h(u(SHELF_ROW_H))
        .flex()
        .flex_row()
        .items_center()
        .gap(u(MARK_GAP))
        .px(px(space::SM))
        .rounded(px(radius::MD))
        .cursor_pointer()
        .tooltip(move |_, cx| cx.new(|_| details.clone()).into())
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .bg(motion::hover_blend(&id, wash, hover))
        .on_click(move |_, window, cx| run(&state, window, cx, |s, _| s.select(&open_id)))
        .child(shelf_mark(c, task, 0.7))
        .child(faded_label(
            format!("{id}-title"),
            true,
            div()
                .text_size(u(13.0))
                .text_color(t.text.opacity(0.86))
                .child(task.title.clone()),
            fade_to,
            c.clips,
        ))
        .child(right)
        .into_any_element()
}

/// One archived row (Pink: spaces.rs `render_archived_section`): history
/// recedes – a dimmed mark and title at rest, restored under the pointer,
/// where Unarchive takes the time's place.
fn archived_row(c: &RowCx, row: &Row) -> AnyElement {
    let t = c.t;
    let task = &row.task;
    let id = format!("{}-archived-{}", c.key, task.id);
    let selected = c.s.page.is_none() && c.s.selected.as_ref() == Some(&task.id);
    let hovered = c.s.shelf_hovered.as_ref() == Some(&task.id);
    let bg = if selected {
        t.glass_selected_bg()
    } else {
        motion::hover_blend(&id, gpui::transparent_black(), t.sidebar_hover_bg())
    };
    let fade_to = pink::flatten(bg, c.surface);
    let right = if hovered {
        let state = c.state.clone();
        let restore = task.id.clone();
        let restore_id = format!("{id}-restore");
        div()
            .id(eid(restore_id.clone()))
            .role(Role::Button)
            .aria_label("Unarchive thread")
            .flex()
            .items_center()
            .justify_center()
            .size(px(chrome::MIN_HIT))
            .rounded(px(chrome::RADIUS_S))
            .hover_wash(restore_id, t.fill(FillTier::Tertiary))
            .tooltip(pink::text_tooltip("Unarchive thread"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                run(&state, window, cx, |s, _| {
                    s.shelf_hovered = None;
                    s.act(&restore, SidebarAction::Unarchive)
                });
            })
            .child(icon(ICON_UNARCHIVE, 13.0, t.text_muted))
            .into_any_element()
    } else {
        div()
            .text_size(u(11.0))
            .text_color(if t.dark {
                t.text_muted.opacity(0.55)
            } else {
                t.text_muted
            })
            .child(time_ago(c.s.clock - row.updated))
            .into_any_element()
    };
    let details = c.details(row, Some("Archived"));
    let state = c.state.clone();
    let hover_state = c.state.clone();
    let open_id = task.id.clone();
    let hover_id = task.id.clone();
    let fade_key = id.clone();
    div()
        .id(eid(id.clone()))
        .flex_none()
        .h(u(SHELF_ROW_H))
        .flex()
        .flex_row()
        .items_center()
        .gap(u(MARK_GAP))
        .px(px(space::SM))
        .rounded(px(radius::MD))
        .cursor_pointer()
        .tooltip(move |_, cx| cx.new(|_| details.clone()).into())
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .bg(bg)
        .on_hover({
            let fade = motion::hover_listener(fade_key);
            move |entered: &bool, window, cx| {
                fade(entered, window, cx);
                hover_state.update(cx, |s, cx| {
                    if *entered {
                        if s.shelf_hovered.as_ref() != Some(&hover_id) {
                            s.shelf_hovered = Some(hover_id.clone());
                            cx.notify();
                        }
                    } else if s.shelf_hovered.as_ref() == Some(&hover_id) {
                        s.shelf_hovered = None;
                        cx.notify();
                    }
                });
            }
        })
        .on_click(move |_, window, cx| run(&state, window, cx, |s, _| s.select(&open_id)))
        .child(shelf_mark(
            c,
            task,
            if hovered || selected { 1.0 } else { 0.4 },
        ))
        .child(faded_label(
            format!("{id}-title"),
            true,
            div()
                .text_size(u(13.0))
                .text_color(if hovered || selected {
                    t.text
                } else {
                    t.legible(t.text.opacity(0.55), pink::TEXT_CONTRAST_TARGET)
                })
                .child(task.title.clone()),
            fade_to,
            c.clips,
        ))
        .child(right)
        .into_any_element()
}

/// The Settled and Archived shelves as one tray at the bottom of the list
/// (Pink: spaces.rs `render_sidebar_shelves`): a rule, then each shelf's
/// quiet disclosure row and its (paged) rows. `None` when both are empty.
fn shelves(c: &RowCx) -> Option<AnyElement> {
    let s = c.s;
    let settled = s.settled_rows();
    let archived = s.archived_rows();
    if settled.is_empty() && archived.is_empty() {
        return None;
    }
    let shelf = |key: &'static str,
                 label: &'static str,
                 glyph: &'static str,
                 rows: Vec<Row>,
                 open: bool,
                 render: fn(&RowCx, &Row) -> AnyElement| {
        (!rows.is_empty()).then(|| {
            let total = rows.len();
            let limit = s.limit(key);
            let shown: Vec<AnyElement> = rows.iter().take(limit).map(|r| render(c, r)).collect();
            let more = more_row(total, shown.len(), limit);
            div()
                .w_full()
                .flex()
                .flex_col()
                .child(shelf_header(c, key, label, glyph, total, open))
                .child(shelf_body(c, key, open, shown, more))
        })
    };
    Some(
        div()
            .w_full()
            .flex_none()
            .flex()
            .flex_col()
            .pb(u(BODY_INSET))
            .child(
                div()
                    .w_full()
                    .pt(u(SECTION_GAP))
                    .pb(u(BODY_INSET))
                    .px(px(space::SM))
                    .child(separator(c.t)),
            )
            .children(shelf(
                "settled",
                "Inactive",
                ICON_CHECK,
                settled,
                s.settled_open,
                settled_row,
            ))
            .children(shelf(
                "archived",
                "Archived",
                ICON_ARCHIVE,
                archived,
                s.archived_open,
                archived_row,
            ))
            .into_any_element(),
    )
}

// ---------------------------------------------------------------------------
// Header: project pill, navigation, Threads header
// ---------------------------------------------------------------------------

/// Swallows the press that dismisses a menu: the card closes on it during
/// capture, and this stops it in the bubble phase so nothing behind the menu
/// (the trigger included) acts on it too (Pink `frosted_menu_rounded`).
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

/// A dropdown hung 6 pt under its trigger, kept 8 pt inside the window
/// (Pink: popover.rs `anchored_menu_below` / `anchored_menu_below_end`). It
/// drops 6 pt into place on `MENU_IN`; while `exit` runs it fades on
/// `MENU_OUT` under an occluder so the dying rows take no clicks.
fn anchored_menu(
    id: String,
    end: bool,
    exit: Option<f32>,
    card: impl IntoElement,
) -> impl IntoElement {
    let inner = div()
        .occlude()
        .pt(u(layout::POPOVER_GAP))
        .child(div().relative().child(press_guard()).child(card));
    let layer = match exit {
        Some(p) => motion::menu_out(
            eid(format!("{id}-out")),
            p,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(eid(id), MENU_RISE_DOWN, inner).into_any_element(),
    };
    div()
        .absolute()
        .bottom_0()
        .when(end, |el| el.right_0())
        .when(!end, |el| el.left_0())
        .size_0()
        .child(
            deferred(
                anchored()
                    .anchor(if end {
                        Anchor::TopRight
                    } else {
                        Anchor::TopLeft
                    })
                    .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                    .child(layer),
            )
            .priority(1),
        )
}

fn exit_progress(popup: &Popup, cx: &App) -> Option<f32> {
    popup.closing.map(|since| {
        if motion::reduced_motion(cx) {
            1.0
        } else {
            motion::menu_exit_progress(since)
        }
    })
}

/// The scope row (Pink: spaces.rs `render_spaces_filter`, UI rethink D1,
/// #180 and #185): one slim 32 pt row, scaled with the Interface size, that
/// names the project the list shows (or All projects) and opens the
/// project picker, then the list's view options and the Activity bell. No
/// border or card: a hover wash and the `CONTROL` radius. The tile or the
/// All projects folder starts on the title column and the name on the
/// navigation labels' column; the bell's glyph ends on the time column.
/// Its accessible name carries the scope's counts ("All projects, 3 open
/// threads").
fn scope_row(c: &RowCx, width: f32, cx: &App) -> AnyElement {
    let t = c.t;
    let s = c.s;
    let id = format!("{}-filter", c.key);
    let open = s.picker.open;
    let (label, tile): (SharedString, Div) = match &s.filter {
        Some(project) => {
            let name = s.project_name(project);
            let tile = project_tile(s.project_index(project), &name, SCOPE_TILE, t);
            (name, tile)
        }
        None => (
            "All projects".into(),
            all_projects_tile(SCOPE_TILE, NAV_ICON, true, t),
        ),
    };
    let spoken = s.counts(s.filter.as_ref()).summary();
    let trigger_bg = if open {
        t.glass_hover()
    } else {
        motion::hover_blend(&id, t.glass_hover().opacity(0.0), t.glass_hover())
    };
    // The label fades into the row's painted wash.
    let fade_to = pink::flatten(trigger_bg, c.surface);
    let ring = t.focus_ring();
    let state = c.state.clone();
    let press_state = c.state.clone();
    let mut trigger = div()
        .id(eid(id.clone()))
        .role(Role::Button)
        .aria_label(format!("{label}, {spoken}"))
        .aria_expanded(open)
        .tab_index(0)
        .flex_1()
        .min_w_0()
        .h(u(SCOPE_ROW))
        .flex()
        .flex_row()
        .items_center()
        .gap(u(space::ICON_LABEL))
        .rounded(px(radius::CONTROL))
        .px(px(space::SM))
        // `TypeRole::Chrome`, grown with the Interface size.
        .text_size(u(TypeRole::Chrome.size()))
        .line_height(u(TypeRole::Chrome.line_height()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text)
        .bg(trigger_bg)
        .on_hover(motion::hover_listener(id.clone()))
        .focus_visible(move |st| pink::focus_ring(st, ring))
        .cursor_pointer()
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            press_state.update(cx, |s, _| s.picker.pressed_while_open = s.picker.mounted());
        })
        .on_click(move |event: &ClickEvent, window, cx| {
            // A press that found the picker open closes it; Return and
            // Space arrive without a press note.
            let close = state.update(cx, |s, _| {
                if event.is_keyboard() {
                    s.picker.open
                } else {
                    std::mem::take(&mut s.picker.pressed_while_open)
                }
            });
            run(&state, window, cx, |s, cx| {
                if close {
                    s.begin_close(PopupKind::Picker, cx);
                } else {
                    s.open_picker(cx);
                }
                Vec::new()
            });
        })
        .on_key_down(pink::swallow_activation_key)
        .child(tile)
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .child(faded_label(
                    format!("{id}-label"),
                    false,
                    label,
                    fade_to,
                    c.clips,
                )),
        )
        .child(icon(ICON_ARROW_DOWN, 12.0, t.readable_muted(0.6)))
        .child(div().flex_1());
    if s.picker.mounted() {
        trigger = trigger.relative().child(anchored_menu(
            format!("{id}-menu"),
            false,
            exit_progress(&s.picker, cx),
            picker_card(c, width),
        ));
    }
    let (view, bell) = scope_controls(c, width, cx);
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(u(2.0))
        // The bell's glyph (centred in a fixed 28 pt button, growing with
        // the text) ends on the rows' trailing column like their times.
        .pr(px(
            space::SM - (chrome::CONTROL_M - chrome::ICON_M * scale()) / 2.0
        ))
        .pt(u(8.0))
        .pb(u(2.0))
        .child(trigger)
        .children(view)
        .child(bell)
        .into_any_element()
}

/// The project picker (Pink: shell/project_picker.rs): a search field over
/// All projects and the project rows – tile, name, folder and live counts,
/// a check on the current filter – with New project… and Manage projects…
/// pinned under the list.
fn picker_card(c: &RowCx, width: f32) -> AnyElement {
    let t = c.t;
    let s = c.s;
    let rows = s.picker_rows();
    let cursor = s.picker.cursor;
    let all = s.counts(None);
    let state = c.state;
    let hover = |ix: usize| {
        let state = state.clone();
        move |_: &gpui::MouseMoveEvent, _: &mut Window, cx: &mut App| {
            state.update(cx, |s, cx| {
                if s.picker.open && s.picker.cursor != Some(ix) {
                    s.picker.cursor = Some(ix);
                    cx.notify();
                }
            })
        }
    };
    let choose = |ix: usize| {
        let state = state.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            run(&state, window, cx, |s, cx| s.choose_picker_slot(ix, cx));
            let focus = state.read(cx).focus.clone();
            window.focus(&focus, cx);
        }
    };
    let set_size = rows.len();
    let list = rows.iter().enumerate().map(|(ix, row)| {
        let (tile, label, subtitle, mono, trailing): (
            Div,
            SharedString,
            SharedString,
            bool,
            Option<Div>,
        ) = match row {
            None => (
                all_projects_tile(28.0, 20.0, false, t),
                "All projects".into(),
                format!(
                    "{} across {} project{}",
                    open_threads(all.active),
                    s.projects.len(),
                    if s.projects.len() == 1 { "" } else { "s" }
                )
                .into(),
                false,
                None,
            ),
            Some(id) => {
                let project = s.projects.iter().find(|p| &p.id == id);
                let name = project.map(|p| p.name.clone()).unwrap_or_default();
                let path = project.map(|p| p.path.clone()).unwrap_or_default();
                let counts = s.counts(Some(id));
                // What needs you, what runs, or when a quiet project
                // was last used.
                let trailing = div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(u(8.0))
                    .text_size(u(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .font_features(pink::tabular_figures())
                    .when(counts.need > 0, |el| {
                        el.child(count(
                            t.warning,
                            icon(ICON_SHIELD, 12.0, t.readable_icon(t.warning)).into_any_element(),
                            counts.need,
                            t,
                        ))
                    })
                    .when(counts.working > 0, |el| {
                        el.child(count(t.busy, working_square(t), counts.working, t))
                    })
                    .when(counts.need == 0 && counts.working == 0, |el| {
                        el.child(
                            div()
                                .font_weight(FontWeight::NORMAL)
                                .text_color(t.readable_faint(0.9))
                                .child(counts.last.map(time_ago).unwrap_or_default()),
                        )
                    });
                (
                    project_tile(s.project_index(id), &name, 28.0, t),
                    name,
                    path,
                    true,
                    Some(trailing),
                )
            }
        };
        // Pink project_picker.rs (#180): a list option named with its counts,
        // its selection and its place in the list.
        let spoken: SharedString = match row {
            Some(id) => format!("{label}, {}", s.counts(Some(id)).summary()).into(),
            None => format!("{label}, {subtitle}").into(),
        };
        pink::menu_row_nav(row == &s.filter, cursor == Some(ix), t)
            .id(eid(format!("{}-picker-row-{ix}", c.key)))
            .role(Role::ListBoxOption)
            .aria_label(spoken)
            .aria_selected(row == &s.filter)
            .aria_position_in_set(ix + 1)
            .aria_size_of_set(set_size)
            .on_mouse_move(hover(ix))
            .on_click(choose(ix))
            .child(tile)
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
                            .font_weight(FontWeight::MEDIUM)
                            .child(label),
                    )
                    .when(!subtitle.is_empty(), |el| {
                        el.child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(u(11.0))
                                .font_weight(FontWeight::NORMAL)
                                .when(mono, |el| el.font_family(pink::MONO))
                                .text_color(t.readable_faint(0.9))
                                .child(subtitle),
                        )
                    }),
            )
            .children(trailing)
    });
    let footer = |ix: usize, glyph: &'static str, label: &'static str| {
        pink::menu_row_nav(false, cursor == Some(ix), t)
            .id(eid(format!("{}-picker-row-{ix}", c.key)))
            .role(Role::Button)
            .aria_label(label)
            .on_mouse_move(hover(ix))
            .on_click(choose(ix))
            .child(icon(glyph, 12.0, t.text_muted))
            .child(div().flex_1().min_w_0().truncate().child(label))
    };
    let empty = rows.is_empty();
    let add = rows.len();
    let close_state = state.clone();
    pink::popover_card(t)
        .id(eid(format!("{}-picker", c.key)))
        .role(Role::Menu)
        .w(px(width))
        .font_weight(FontWeight::NORMAL)
        .flex()
        .flex_col()
        .on_mouse_down_out(move |_, _, cx| {
            close_state.update(cx, |s, cx| {
                s.begin_close(PopupKind::Picker, cx);
                cx.notify();
            })
        })
        .child(
            pink::search_input_frame(t)
                .text_color(t.text)
                .child(s.search.clone()),
        )
        .child(
            div()
                .id(eid(format!("{}-picker-list", c.key)))
                .role(Role::ListBox)
                .aria_label("Projects")
                .flex()
                .flex_col()
                .gap(u(2.0))
                .max_h(px(PICKER_LIST_MAX))
                .overflow_y_scroll()
                .children(list),
        )
        .when(empty, |card| {
            card.child(
                div()
                    .px(px(8.0))
                    .py(u(6.0))
                    .text_size(u(12.0))
                    .text_color(t.readable_faint(0.9))
                    .child("No matching projects"),
            )
        })
        // Pinned under the list, full-bleed through the card's inset.
        .child(
            div()
                .my(u(2.0))
                .mx(px(-layout::CARD_INSET))
                .h(u(1.0))
                .flex_none()
                .bg(t.border.opacity(0.6)),
        )
        .child(footer(add, ICON_PLUS, "New project…"))
        .child(footer(add + 1, ICON_SETTINGS, "Manage projects…"))
        .into_any_element()
}

/// The navigation section (Pink: spaces.rs `render_sidebar_nav`, #180 and
/// #185): one 32 pt row per item, scaled with the Interface size, the
/// 16 pt icon on the title column and the label 8 after it, a page's row
/// marked while its page is open, then a rule.
fn nav_rows(c: &RowCx) -> AnyElement {
    let t = c.t;
    let s = c.s;
    let rows = s.nav.iter().map(|item| {
        let id = format!("{}-nav-{}", c.key, item.key);
        let current = s.page.as_ref() == Some(&item.key);
        let ring = t.focus_ring();
        let state = c.state.clone();
        let key = item.key.clone();
        let hover_group: SharedString = id.clone().into();
        div()
            .id(eid(id.clone()))
            .group(hover_group.clone())
            .role(Role::Button)
            .aria_label(item.label.clone())
            .aria_selected(current)
            .tab_index(0)
            .w_full()
            .h(u(SCOPE_ROW))
            .flex_none()
            .flex()
            .flex_row()
            .items_center()
            .gap(u(NAV_GAP))
            .px(px(space::SM))
            .rounded(px(radius::MD))
            .cursor_pointer()
            .text_size(u(13.0))
            .font_weight(FontWeight::MEDIUM)
            .when(current, |row| {
                row.bg(t.glass_selected_bg()).text_color(t.text)
            })
            .when(!current, |row| {
                row.hover_fill_fg(
                    id.clone(),
                    gpui::transparent_black(),
                    t.sidebar_hover_bg(),
                    t.text.opacity(0.85),
                    t.text,
                )
            })
            .focus_visible(move |st| pink::focus_ring(st, ring))
            .on_click(move |_, window, cx| run(&state, window, cx, |s, _| s.nav(&key)))
            .on_key_down(pink::swallow_activation_key)
            .child(
                svg()
                    .path(item.icon.clone())
                    .size(u(NAV_ICON))
                    .flex_none()
                    .text_color(if current { t.text } else { t.text_muted }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(item.label.clone()),
            )
            .when_some(item.hint.clone(), |row, hint| {
                // The shortcut shows under the pointer only.
                row.child(
                    div()
                        .flex_none()
                        .opacity(0.0)
                        .group_hover(hover_group, |st| st.opacity(1.0))
                        .text_size(u(11.0))
                        .font_weight(FontWeight::NORMAL)
                        .font_family(pink::MONO)
                        .text_color(t.readable_faint(0.9))
                        .child(hint),
                )
            })
    });
    div()
        .flex_none()
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .pt(u(6.0))
                .children(rows),
        )
        .child(
            div()
                .pt(u(10.0))
                .child(div().px(px(space::SM)).child(separator(t))),
        )
        .into_any_element()
}

/// The footer (Pink: shell.rs `render_chat_sidebar` footer actions, #185):
/// Settings and Devices, one 32 pt button recipe with an 18 pt glyph, both
/// growing with the Interface size. It starts where the first glyph's
/// stroke lands on the title column at every size
/// (`sidebar_footer_leading`).
fn footer(c: &RowCx) -> AnyElement {
    let t = c.t;
    let scale = scale();
    // Pink: `2 × SIDEBAR_GUTTER − glyph_inset × scale`, from the sidebar's
    // edge; here from inside the column's own gutter.
    let glyph_inset = (FOOTER_BUTTON - FOOTER_ICON) / 2.0 + FOOTER_BEARING;
    let leading = (2.0 * GUTTER - glyph_inset * scale).max(0.0) - GUTTER;
    let ring = t.focus_ring();
    let button =
        |name: &'static str, glyph: &'static str, label: &'static str, command: SidebarCommand| {
            let id = format!("{}-footer-{name}", c.key);
            let state = c.state.clone();
            div()
                .id(eid(id.clone()))
                .role(Role::Button)
                .aria_label(label)
                .tab_index(0)
                .size(u(FOOTER_BUTTON))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(chrome::RADIUS_L))
                .cursor_pointer()
                .hover_fill(id, t.glass_hover().opacity(0.0), t.glass_hover())
                .focus_visible(move |st| pink::focus_ring(st, ring))
                .tooltip(pink::text_tooltip(label))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
                .on_click(move |_, window, cx| {
                    let command = command.clone();
                    run(&state, window, cx, |_, _| vec![Effect::Command(command)])
                })
                .on_key_down(pink::swallow_activation_key)
                .child(icon(glyph, FOOTER_ICON, t.text_muted))
        };
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .ml(px(leading))
        .mt(px(4.0))
        .child(button(
            "settings",
            ICON_SETTINGS_GEAR,
            "Open settings",
            SidebarCommand::OpenSettings,
        ))
        .child(button(
            "devices",
            ICON_SMARTPHONE,
            "Open devices",
            SidebarCommand::OpenDevices,
        ))
        .into_any_element()
}

/// The view options menu (Pink: spaces.rs `render_sidebar_view_menu`):
/// Organize, Sort, Show and Move idle threads to Inactive groups of
/// checkable rows.
fn view_menu(c: &RowCx, width: f32) -> AnyElement {
    let t = c.t;
    let s = c.s;
    let cursor = s.view.cursor;
    let rows = s.view_rows().into_iter().enumerate().map(|(ix, row)| {
        let checked = s.view_checked(row);
        let move_state = c.state.clone();
        let click_state = c.state.clone();
        let control = pink::menu_row_nav(checked, cursor == Some(ix), t)
            .id(eid(format!("{}-view-row-{ix}", c.key)))
            .role(Role::Button)
            .aria_label(row.label())
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .on_mouse_move(move |_, _, cx| {
                move_state.update(cx, |s, cx| {
                    if s.view.open && s.view.cursor != Some(ix) {
                        s.view.cursor = Some(ix);
                        cx.notify();
                    }
                })
            })
            .on_click(move |_, window, cx| {
                run(&click_state, window, cx, |s, cx| {
                    s.activate_view_row(row, cx);
                    Vec::new()
                })
            })
            .child(icon(row.glyph(), 15.0, t.text_muted))
            .child(div().flex_1().min_w_0().truncate().child(row.label()));
        div()
            .flex()
            .flex_col()
            .when(ix != 0 && row.heading().is_some(), |el| {
                el.child(pink::menu_separator(t))
            })
            .when_some(row.heading(), |el, title| {
                el.child(pink::menu_heading(title, t))
            })
            .child(control)
    });
    let key_state = c.state.clone();
    let close_state = c.state.clone();
    pink::popover_card(t)
        .id(eid(format!("{}-view-menu", c.key)))
        .role(Role::Menu)
        .w(px(width))
        .font_weight(FontWeight::NORMAL)
        .track_focus(&s.view_menu_focus)
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let handled = key_state.update(cx, |s, cx| {
                if !s.view.open {
                    return false;
                }
                let count = s.view_rows().len();
                match event.keystroke.key.as_str() {
                    "up" | "down" => {
                        let delta = if event.keystroke.key == "up" { -1 } else { 1 };
                        s.view.cursor = step(s.view.cursor, count, delta);
                    }
                    "enter" | "space" => {
                        if let Some(row) =
                            s.view.cursor.and_then(|ix| s.view_rows().get(ix).copied())
                        {
                            s.activate_view_row(row, cx);
                        }
                    }
                    "escape" => {
                        s.begin_close(PopupKind::View, cx);
                    }
                    _ => return false,
                }
                cx.notify();
                true
            });
            if handled {
                cx.stop_propagation();
                if !key_state.read(cx).view.open {
                    let focus = key_state.read(cx).view_focus.clone();
                    window.focus(&focus, cx);
                }
            }
        })
        .on_mouse_down_out(move |_, _, cx| {
            close_state.update(cx, |s, cx| {
                s.begin_close(PopupKind::View, cx);
                cx.notify();
            })
        })
        .child(
            div()
                .id(eid(format!("{}-view-scroll", c.key)))
                .max_h(px(VIEW_MENU_MAX))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(u(2.0))
                .children(rows),
        )
        .into_any_element()
}

/// The scope row's controls (Pink: spaces.rs `render_sidebar_view_trigger`
/// and activity_center.rs `render_activity_action`): the view options,
/// hidden while Activity is on since it has its own sections, and the
/// Activity bell.
fn scope_controls(c: &RowCx, width: f32, cx: &App) -> (Option<AnyElement>, AnyElement) {
    let t = c.t;
    let s = c.s;
    let activity = s.activity;
    let ring = t.focus_ring();

    // The view options trigger: a 28 pt square with the sort glyph.
    let view_id = format!("{}-view-options", c.key);
    let view_open = s.view.open;
    let press_state = c.state.clone();
    let click_state = c.state.clone();
    let mut view_trigger = div()
        .id(eid(view_id.clone()))
        .role(Role::Button)
        .aria_label("Sidebar view options")
        .aria_expanded(view_open)
        .track_focus(&s.view_focus)
        .size(px(chrome::CONTROL_M))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_M))
        .focus_visible(move |st| pink::focus_ring(st, ring))
        .cursor_pointer()
        .hover_fill(
            view_id.clone(),
            if view_open {
                t.glass_hover()
            } else {
                t.glass_hover().opacity(0.0)
            },
            t.glass_hover(),
        )
        .tooltip(pink::text_tooltip("Sidebar view options"))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            press_state.update(cx, |s, _| s.view.pressed_while_open = s.view.mounted());
        })
        .on_click(move |event: &ClickEvent, window, cx| {
            // Enter/Space arrive as a keyboard click with no press note.
            let close = click_state.update(cx, |s, _| {
                if event.is_keyboard() {
                    s.view.open
                } else {
                    std::mem::take(&mut s.view.pressed_while_open)
                }
            });
            run(&click_state, window, cx, |s, cx| {
                if close {
                    s.begin_close(PopupKind::View, cx);
                } else {
                    let cursor = event.is_keyboard().then_some(0);
                    s.open_popup(PopupKind::View, cursor, cx);
                }
                Vec::new()
            });
        })
        .on_key_down(pink::swallow_activation_key)
        .child(icon(ICON_SORT, 16.0, t.text_muted));
    if s.view.mounted() {
        view_trigger = view_trigger.relative().child(anchored_menu(
            format!("{view_id}-menu"),
            true,
            exit_progress(&s.view, cx),
            view_menu(c, width),
        ));
    }

    // The Activity bell (Pink: activity_center.rs `render_activity_action`):
    // accent while the view is on, a 6 pt accent dot on its shoulder while
    // something needs you.
    let bell_id = format!("{}-activity", c.key);
    let attention = s.rows.iter().any(|r| r.task.wants_you());
    let bell_label = if activity {
        "Turn off activity view"
    } else {
        "View activity"
    };
    let bell_state = c.state.clone();
    let bell = div()
        .id(eid(bell_id.clone()))
        .relative()
        .size(px(chrome::CONTROL_M))
        .flex_none()
        .rounded(px(chrome::RADIUS_M))
        .border_1()
        .border_color(t.border.opacity(0.0))
        .flex()
        .items_center()
        .justify_center()
        .role(Role::Button)
        .aria_label(bell_label)
        .aria_toggled(if activity {
            Toggled::True
        } else {
            Toggled::False
        })
        .tab_index(0)
        .focus_visible(move |st| pink::focus_ring(st, ring))
        .cursor_pointer()
        .hover_fill(
            bell_id,
            if activity {
                t.glass_hover()
            } else {
                t.glass_hover().opacity(0.0)
            },
            t.glass_hover(),
        )
        .tooltip(pink::text_tooltip(bell_label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_click(move |_, window, cx| {
            run(&bell_state, window, cx, |s, cx| {
                s.activity = !s.activity;
                s.begin_close(PopupKind::View, cx);
                s.hovered = None;
                vec![Effect::Command(SidebarCommand::Activity(s.activity))]
            })
        })
        .on_key_down(pink::swallow_activation_key)
        .child(icon(
            ICON_BELL,
            16.0,
            if activity { t.accent } else { t.text_muted },
        ))
        .when(attention, |el| {
            el.child(
                div()
                    .absolute()
                    .top(u(4.5))
                    .right(px(5.5))
                    .size(u(UNREAD_DOT))
                    .rounded_full()
                    .bg(t.accent),
            )
        });

    (
        (!activity).then(|| view_trigger.into_any_element()),
        bell.into_any_element(),
    )
}

/// A centred empty list with the next step (Pink: shell.rs
/// `render_chat_sidebar`, audit SH-24, #180). Truthful: with only inactive
/// or archived threads it says where they are, and "No threads yet" is
/// only for a scope with none at all. Announced as a status. Drawn by the
/// `pink-empty-state` package's note.
fn empty_state(key: &str, activity: bool, has_shelves: bool, t: &Pink) -> AnyElement {
    let (title, hint) = if activity {
        (
            "Nothing needs attention",
            "Running and waiting threads appear here.",
        )
    } else if has_shelves {
        (
            "No open threads",
            "Inactive and archived threads are listed below.",
        )
    } else {
        ("No threads yet", "Start one with ⌘N.")
    };
    empty_state::note(
        &empty_state::NoteStyle {
            title: t.readable_muted(0.9),
            hint: t.readable_faint(0.9),
        },
        title,
        hint,
    )
    .id(eid(format!("{key}-empty-state")))
    .role(Role::Status)
    .aria_label(format!("{title}. {hint}"))
    .into_any_element()
}

// ---------------------------------------------------------------------------
// Keyboard
// ---------------------------------------------------------------------------

/// Pink's sidebar shortcuts: ⌘1–9 jump to a visible row, Ctrl-Tab and
/// Ctrl-Shift-Tab cycle through them, ⌘N starts a new thread.
fn on_key(state: &Entity<SidebarState>, event: &KeyDownEvent, window: &mut Window, cx: &mut App) {
    let s = state.read(cx);
    if s.picker.mounted() || s.view.mounted() {
        return;
    }
    let keystroke = &event.keystroke;
    let mods = keystroke.modifiers;
    let key = keystroke.key.as_str();
    if mods.secondary() && !mods.shift && !mods.alt {
        if let Some(digit) = key
            .parse::<usize>()
            .ok()
            .filter(|d| (1..=JUMP_SLOTS).contains(d))
        {
            if let Some(id) = s.jump_order().get(digit - 1).cloned() {
                cx.stop_propagation();
                run(state, window, cx, |s, _| s.select(&id));
            }
            return;
        }
        if key == "n" {
            let new_thread = s
                .nav
                .iter()
                .find(|item| !item.page)
                .map(|item| item.key.clone());
            if let Some(key) = new_thread {
                cx.stop_propagation();
                run(state, window, cx, |s, _| s.nav(&key));
            }
            return;
        }
    }
    if mods.control && key == "tab" {
        let order = s.jump_order();
        if order.is_empty() {
            return;
        }
        let current = s
            .selected
            .as_ref()
            .and_then(|id| order.iter().position(|o| o == id));
        let delta = if mods.shift { -1 } else { 1 };
        if let Some(next) = step(current, order.len(), delta) {
            let id = order[next].clone();
            cx.stop_propagation();
            run(state, window, cx, |s, _| s.select(&id));
        }
    }
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        SCALE.with(|s| s.set(pink::ui_scale(cx)));
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "sidebar".into()),
            cx,
            |window, cx| SidebarState::new(&self, window, cx),
        );
        let handlers = self.handlers.clone();
        state.update(cx, |s, _| s.handlers = handlers);

        // Focus a popover once it has opened.
        let focus_view = state.update(cx, |s, _| {
            let due = s.view.pending_focus && s.view.open;
            if due {
                s.view.pending_focus = false;
            }
            due.then(|| s.view_menu_focus.clone())
        });
        if let Some(handle) = focus_view {
            window.focus(&handle, cx);
        }
        let search = state.update(cx, |s, _| {
            let due = s.picker.pending_focus && s.picker.open;
            if due {
                s.picker.pending_focus = false;
            }
            due.then(|| s.search.clone())
        });
        if let Some(search) = search {
            TextInput::focus(&search, window, cx);
        }
        // The search field's placeholder is quiet, its caret the accent.
        let input = state.read(cx).search.clone();
        input.update(cx, |input, _| {
            input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
        });

        // -- What lists, in order -----------------------------------------------
        let (items, _) = state.read(cx).items();
        let order: Vec<(String, f32)> = items.iter().map(|i| (i.key(), i.height())).collect();
        let (epoch, resort, new_keys) = state.update(cx, |s, _| {
            s.note_order(order);
            (s.epoch, s.resort.clone(), s.new_keys.clone())
        });

        let s = state.read(cx);
        let surface = t.chrome_surface();
        let clips = s.clips.clone();
        let c = RowCx {
            key: &key,
            t: &t,
            state: &state,
            s,
            clips: &clips,
            surface,
        };
        let menu_width = self.width - 2.0 * GUTTER;
        let scope = scope_row(&c, menu_width, cx);
        let nav = nav_rows(&c);
        let footer = footer(&c);
        let shelves = (!s.activity).then(|| shelves(&c)).flatten();
        let has_shelves = shelves.is_some();
        let is_empty = items.is_empty();
        let list_items: Vec<AnyElement> = items
            .iter()
            .map(|item| {
                let item_key = item.key();
                let element = match item {
                    Item::Heading {
                        section,
                        count,
                        first,
                    } => section_heading(&c, *section, *count, *first),
                    Item::Row { row, slot } => task_row(&c, row, false, None, *slot),
                    Item::Group(g) => group(&c, g),
                    Item::Divider => div()
                        .w_full()
                        .pt(u(space::SM))
                        .pb(u(space::SM))
                        .px(px(space::SM))
                        .child(separator(&t))
                        .into_any_element(),
                    Item::More { section, more } => more_row_el(&c, section, *more),
                };
                // Surviving rows glide from their old place; new rows fade
                // in; removals just go.
                if let Some(dy) = resort.get(&item_key).copied() {
                    div()
                        .child(element)
                        .with_animation(
                            eid(format!("{key}-resort-{epoch}-{item_key}")),
                            RESORT.animation(),
                            move |el, p| el.relative().top(px(dy * (1.0 - p))),
                        )
                        .into_any_element()
                } else if new_keys.contains(&item_key) {
                    motion::fade_quick(
                        eid(format!("{key}-row-in-{epoch}-{item_key}")),
                        div().child(element),
                    )
                    .into_any_element()
                } else {
                    element
                }
            })
            .collect();

        // The list scrolls under 24 pt edge fades into the surface.
        let scroll = s.scroll.clone();
        let offset = -f32::from(scroll.offset().y);
        let max = f32::from(scroll.max_offset().y);
        let fade_top = offset > 0.5;
        let fade_bottom = offset < max - 0.5;
        let scroll_state = state.clone();
        let list = div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id(eid(format!("{key}-list")))
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .on_scroll_wheel(move |_, _, cx| {
                        scroll_state.update(cx, |_, cx| cx.notify());
                    })
                    .flex()
                    .flex_col()
                    .pt(u(4.0))
                    .when(is_empty, |el| {
                        el.child(empty_state(&key, s.activity, has_shelves, &t))
                    })
                    .children(list_items)
                    // A short list leaves the shelves on the bottom edge; a
                    // long one lets them flow after the rows.
                    .when(has_shelves, |el| el.child(div().flex_1().min_h_0()))
                    .children(shelves),
            )
            .when(fade_top, |el| {
                el.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(LIST_FADE))
                        .bg(linear_gradient(
                            180.0,
                            linear_color_stop(surface, 0.0),
                            linear_color_stop(surface.opacity(0.0), 1.0),
                        )),
                )
            })
            .when(fade_bottom, |el| {
                el.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h(px(LIST_FADE))
                        .bg(linear_gradient(
                            180.0,
                            linear_color_stop(surface.opacity(0.0), 0.0),
                            linear_color_stop(surface, 1.0),
                        )),
                )
            });

        let focus = s.focus.clone();
        let key_state = state.clone();
        let mods_state = state.clone();
        let down_focus = focus.clone();
        motion::keep_alive(window);
        // The sidebar column on the shell surface, with Pink's left gutter
        // (and the canvas inset standing in on the right).
        div()
            .id(eid(format!("{key}-root")))
            .track_focus(&focus)
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .px(px(GUTTER))
            .pb(u(GUTTER))
            .rounded(px(radius::PANEL))
            .overflow_hidden()
            .bg(surface)
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !down_focus.contains_focused(window, cx) {
                    window.focus(&down_focus, cx);
                }
            })
            .on_key_down(move |event, window, cx| on_key(&key_state, event, window, cx))
            .on_modifiers_changed(move |event: &ModifiersChangedEvent, _, cx| {
                let mods = event.modifiers;
                let visible = mods.secondary() && !mods.shift && !mods.alt;
                mods_state.update(cx, |s, cx| {
                    if s.jump_hints != visible {
                        s.jump_hints = visible;
                        cx.notify();
                    }
                });
            })
            .child(scope)
            .child(nav)
            .child(list)
            .child(footer)
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// Both scopes side by side: All projects groups every project under its
/// header, a single project lists its two-line rows, and every title,
/// glyph and time lands on the same two columns in each.
pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .flex()
            .flex_row()
            .gap(px(24.0))
            .child(demo("sidebar-all"))
            .child(demo("sidebar-project").filter("billing"))
    })
}

fn demo(id: &'static str) -> Sidebar {
    const H: f32 = 3600.0;
    {
        Sidebar::new(id)
            .width(288.0)
            .height(720.0)
            .project(SidebarProject::new("billing", "billing-api").path("~/code/billing-api"))
            .project(SidebarProject::new("web", "web-app").path("~/code/web-app"))
            .project(SidebarProject::new("reports", "reports-web").path("~/code/reports-web"))
            .project(SidebarProject::new("docs", "docs").path("~/code/docs"))
            .thread(
                SidebarThread::new("ledger", "Reconcile the March ledger export", "billing")
                    .branch("ledger/march")
                    .model("Opus")
                    .pinned(true)
                    .age(2.0 * H),
            )
            .thread(
                SidebarThread::new("invoices", "Backfill invoice PDFs", "billing")
                    .branch("invoices/backfill")
                    .model("Opus")
                    .status(ThreadStatus::Working)
                    .age(40.0),
            )
            .thread(
                SidebarThread::new("coupons", "Retire expired coupon codes", "billing")
                    .branch("coupons/retire")
                    .status(ThreadStatus::NeedsApproval)
                    .pull_request(482, PullRequestState::Open)
                    .age(150.0),
            )
            .thread(
                SidebarThread::new("tax", "Which VAT rate applies to Norway?", "billing")
                    .harness(Some(Harness::Codex))
                    .model("gpt-5")
                    .status(ThreadStatus::NeedsInput)
                    .age(420.0),
            )
            .thread(
                SidebarThread::new("webhooks", "Retry failed Stripe webhooks", "billing")
                    .branch("webhooks/retry")
                    .status(ThreadStatus::Done)
                    .related(2)
                    .age(900.0),
            )
            .thread(
                SidebarThread::new("pg", "Upgrade the Postgres driver", "billing")
                    .branch("deps/pg")
                    .device("build-box")
                    .harness(Some(Harness::OpenCode))
                    .status(ThreadStatus::Failed)
                    .age(1800.0),
            )
            .thread(
                SidebarThread::new("refunds", "Partial refunds for annual plans", "billing")
                    .branch("refunds/partial")
                    .pull_request(479, PullRequestState::Merged)
                    .open_tab(true)
                    .seen(true)
                    .status(ThreadStatus::Done)
                    .age(2.5 * H),
            )
            .thread(
                SidebarThread::new("dunning", "Draft dunning email copy", "billing")
                    .seen(true)
                    .status(ThreadStatus::Done)
                    .age(5.0 * H),
            )
            .thread(
                SidebarThread::new("currency", "Add currency to usage exports", "billing")
                    .branch("exports/currency")
                    .seen(true)
                    .status(ThreadStatus::Done)
                    .age(8.0 * H),
            )
            .thread(
                SidebarThread::new("proration", "Explain proration on upgrades", "billing")
                    .harness(Some(Harness::Codex))
                    .age(30.0 * H),
            )
            .thread(
                SidebarThread::new("audit", "Audit the refund permissions", "billing")
                    .seen(true)
                    .status(ThreadStatus::Done)
                    .inactive(true)
                    .age(26.0 * H),
            )
            .thread(
                SidebarThread::new("seed", "Seed the staging database", "billing")
                    .archived(true)
                    .age(6.0 * 24.0 * H),
            )
            .thread(
                SidebarThread::new("spike", "Spike: usage-based pricing", "billing")
                    .harness(Some(Harness::Codex))
                    .archived(true)
                    .age(12.0 * 24.0 * H),
            )
            .thread(
                SidebarThread::new("flags", "Audit unused feature flags", "web")
                    .branch("chore/flag-audit")
                    .status(ThreadStatus::Working)
                    .age(90.0),
            )
            .thread(
                SidebarThread::new("nav", "Tighten the mobile nav spacing", "web")
                    .branch("ui/mobile-nav")
                    .seen(true)
                    .status(ThreadStatus::Done)
                    .age(3.0 * H),
            )
            .thread(
                SidebarThread::new("exports", "Pick a date format for exports", "reports")
                    .status(ThreadStatus::NeedsInput)
                    .age(600.0),
            )
            .thread(
                SidebarThread::new("notes", "Draft the 4.2 release notes", "docs")
                    .seen(true)
                    .status(ThreadStatus::Done)
                    .age(4.0 * H),
            )
            .selected("invoices")
            .event(
                3.0,
                SidebarThread::new("invoices", "Backfill invoice PDFs", "billing")
                    .branch("invoices/backfill")
                    .model("Opus")
                    .status(ThreadStatus::NeedsApproval),
            )
            .event(
                6.5,
                SidebarThread::new(
                    "chargebacks",
                    "Summarize this week's chargebacks",
                    "billing",
                )
                .status(ThreadStatus::Working),
            )
            .event(
                10.0,
                SidebarThread::new("coupons", "Retire expired coupon codes", "billing")
                    .branch("coupons/retire")
                    .pull_request(482, PullRequestState::Open)
                    .status(ThreadStatus::Working),
            )
            .event(
                14.0,
                SidebarThread::new(
                    "chargebacks",
                    "Summarize this week's chargebacks",
                    "billing",
                )
                .status(ThreadStatus::Done),
            )
            .event(
                18.0,
                SidebarThread::new("tax", "Which VAT rate applies to Norway?", "billing")
                    .harness(Some(Harness::Codex))
                    .model("gpt-5")
                    .status(ThreadStatus::Working),
            )
            .autoplay(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_ago_matches_pinks_compact_form() {
        assert_eq!(time_ago(12.0), "now");
        assert_eq!(time_ago(150.0), "2m");
        assert_eq!(time_ago(3.0 * 3600.0), "3h");
        assert_eq!(time_ago(2.0 * DAY), "2d");
        assert_eq!(time_ago(15.0 * DAY), "2w");
        assert_eq!(time_ago(60.0 * DAY), "2mo");
        assert_eq!(time_ago(800.0 * DAY), "2y");
    }

    #[test]
    fn more_row_pages_five_at_a_time() {
        assert_eq!(more_row(3, 3, 5), None);
        assert_eq!(more_row(8, 5, 5), Some(MoreRow::More(3)));
        assert_eq!(more_row(13, 5, 5), Some(MoreRow::More(5)));
        // The kept selected row past the cap shrinks what is hidden.
        assert_eq!(more_row(8, 6, 5), Some(MoreRow::More(2)));
        assert_eq!(more_row(8, 8, 10), Some(MoreRow::Less));
    }

    #[test]
    fn summary_names_open_then_needs_then_running() {
        assert_eq!(Counts::default().summary(), "No open threads");
        let counts = Counts {
            active: 4,
            need: 1,
            working: 2,
            last: None,
        };
        assert_eq!(counts.summary(), "4 open threads, 1 needs you, 2 running");
        let counts = Counts {
            active: 1,
            need: 2,
            working: 0,
            last: None,
        };
        assert_eq!(counts.summary(), "1 open thread, 2 need you");
    }

    #[test]
    fn only_quiet_finished_work_settles() {
        let task = SidebarThread::new("a", "A", "p");
        assert!(task.clone().status(ThreadStatus::Done).can_settle());
        assert!(!task.clone().status(ThreadStatus::Working).can_settle());
        assert!(
            !task
                .clone()
                .status(ThreadStatus::NeedsApproval)
                .can_settle()
        );
        assert!(!task.clone().pinned(true).can_settle());
        assert!(!task.archived(true).can_settle());
    }

    #[test]
    fn menu_step_wraps() {
        assert_eq!(step(None, 3, 1), Some(0));
        assert_eq!(step(None, 3, -1), Some(2));
        assert_eq!(step(Some(2), 3, 1), Some(0));
        assert_eq!(step(Some(0), 3, -1), Some(2));
        assert_eq!(step(None, 0, 1), None);
    }
}
