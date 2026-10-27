//! Chat Composer – a Pink component (Pink: `composer.rs` `Composer`).
//!
//! PINK's prompt pill after UI rethink milestone 2 slices 1 and 2
//! (novnski/pink#174, PRs #183 and #184): a 26 pt rounded plate with a
//! hairline edge. Staged attachments sit above the text as 56 pt square
//! thumbnails, the field grows from one line to 260 pt, and a 46 pt
//! controls row holds the paperclip, ghost picker chips (model, access, web
//! search) and the submit disc. The access chip names its mode in words
//! ("Approve for me", "Full access") and the model chip its effort ("Low");
//! on a narrow row the pieces yield in PINK's order (PINK 0.30.4,
//! novnski/pink#187): the search label, then the access words, then the
//! effort, then the model name, each kept in a tooltip. An existing thread
//! says "Message this thread", a new one "Do anything…". The submit disc is
//! named for what it does: Send with no live
//! run; while one is live, a separate Stop sits beside Queue for after this
//! turn, or Steer this turn when the follow-up preference is Steer and the
//! provider steers natively. With an empty draft Stop shows alone, and the
//! row keeps its width. The checkout chip names the branch first ("main",
//! "main in storefront-2", "main on Studio Mac") beside every pull request
//! recorded for the thread, each with its own state. In an existing thread
//! they ride the controls row (PINK 0.30.3, novnski/pink#186): the branch
//! keeps its natural width up to its share and the pull requests get their
//! own, so listing every one never cuts the branch short; a new thread keeps
//! its checkout in a row above the pill. The branch menu ticks the checkout
//! the thread actually runs in, even when two are on the same branch. Every
//! menu opens upward from its chip and works from the keyboard. Voice input
//! keeps the field's place with a breathing orb, the words heard so far and
//! a timer; they land in the field for editing. Enter sends, Shift+Enter
//! adds a line.
//!
//! ```ignore
//! ChatComposer::new("composer")
//!     .attachments(files)
//!     .running(agent_busy)
//!     .follow_up(FollowUp::Steer)
//!     .steer_available(provider_steers)
//!     .in_thread(true)
//!     .checkout(Checkout::branch("main").worktree("storefront-2").path(root))
//!     .branches([BranchRef::new("main").current(project), BranchRef::new("main").worktree(root)])
//!     .pull_requests(prs)
//!     .on_attach(|_, cx| { /* open a file picker */ })
//!     .on_send(|message, _, cx| println!("{} via {}", message.text, message.model))
//!     .on_stop(|_, cx| { /* interrupt the turn */ })
//! ```

use std::{cell::Cell, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnyElement, AnyView, App, Context, Div, ElementId, Entity, FocusHandle, FontWeight,
    IntoElement, KeyDownEvent, MouseButton, ObjectFit, Role, SharedString, Stateful, Subscription,
    Toggled, Window, anchored, canvas, deferred, div, img, point, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt},
    pink::{self, FillTier, Pink, TextStyle, TypeStyleExt, layout, loaders, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Chat Composer";
pub const DESCRIPTION: &str = "The composer's prompt pill: square attachment thumbnails over a field that grows as you type, quiet model, access and web search chips that name the model's effort and the access mode in words until the row runs out of room, with menus that open upward, voice input, a branch-first checkout chip beside every pull request (in the controls row of a thread, above the pill of a new one), and a submit button named Send, Queue for after this turn or Steer this turn, with a separate Stop while a run is live";

const ICON_PAPERCLIP: &str = "icons/chat-composer/paperclip.svg";
const ICON_ARROW_UP: &str = "icons/chat-composer/arrow-up.svg";
const ICON_QUEUE_SEND: &str = "icons/chat-composer/queue-send.svg";
const ICON_ARROW_UP_RIGHT: &str = "icons/chat-composer/arrow-right-up.svg";
const ICON_BRANCH: &str = "icons/chat-composer/git-branch.svg";
const ICON_MONITOR: &str = "icons/chat-composer/monitor.svg";
const ICON_PULL_REQUEST: &str = "icons/chat-composer/git-pull-request.svg";
const ICON_CLOSE: &str = "icons/chat-composer/close.svg";
const ICON_GLOBE: &str = "icons/chat-composer/globe.svg";
const ICON_SHIELD_CHECK: &str = "icons/chat-composer/shield-check.svg";
const ICON_UNLOCKED: &str = "icons/chat-composer/lock-keyhole-unlocked.svg";
const ICON_MIC: &str = "icons/chat-composer/mic.svg";
const ICON_FILE: &str = "icons/chat-composer/file-text.svg";
const ICON_FILE_IMAGE: &str = "icons/chat-composer/file-image.svg";
pub const ICON_QUICK: &str = "icons/chat-composer/bolt-fast.svg";
pub const ICON_BALANCED: &str = "icons/chat-composer/magic-wand.svg";
pub const ICON_DEEP: &str = "icons/chat-composer/brain.svg";
const DEMO_HERO: &str = "icons/chat-composer/launch-hero.svg";

// Pill geometry (Pink `composer.rs` layout constants, points at 16 px).

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
/// The separate Stop never drops below this, at any Interface size (Pink
/// composer.rs `STOP_MIN`, UI rethink D36).
const STOP_MIN: f32 = 28.0;
/// Between Stop and the submit disc (Pink `STOP_GAP`, `space::ICON_LABEL`).
const STOP_GAP: f32 = pink::space::ICON_LABEL;
/// `STRIP_THUMB`, `STRIP_GAP`, `STRIP_PAD_TOP`, `STRIP_PAD_X`.
const STRIP_THUMB: f32 = 56.0;
const STRIP_GAP: f32 = 8.0;
const STRIP_PAD_TOP: f32 = 12.0;
const STRIP_PAD_X: f32 = 16.0;
/// `CONTEXT_ROW_HEIGHT`: the checkout row above the pill.
const CONTEXT_ROW_HEIGHT: f32 = 24.0;
/// Column gap between the checkout row and the pill (`Theme::SPACE_SM`).
const COLUMN_GAP: f32 = 8.0;
/// Pink `PILL_BORDER_V`: the pill's hairline, both sides.
const PILL_BORDER: f32 = 2.0;
/// Pink pickers.rs `CHIP_TEXT_SIZE`: the controls row measures chip text at
/// 12 px medium.
const CHIP_TEXT: f32 = 12.0;
/// The checkout row's chips draw at the kit's 16 pt baseline; only Stop
/// scales with the Interface size (see the module docs of `crate::pink`).
const CHIP_SCALE: f32 = 1.0;
/// Pink `pickers.rs` `TARGET_CHIP_*` / `FOOTER_CHIP_RADIUS`.
const TARGET_CHIP_HEIGHT: f32 = 22.0;
const TARGET_CHIP_PAD_X: f32 = 6.0;
const TARGET_CHIP_TEXT: f32 = 11.5;
const TARGET_CHIP_GLYPH: f32 = 12.0;
const TARGET_CHIP_GAP: f32 = 5.0;
const TARGET_CHIP_CHEVRON: f32 = 10.0;
const TARGET_CHIP_RADIUS: f32 = 6.0;
/// Pink `goals.rs` `GOAL_CHIP_ICON`: the mode-on chip's glyph.
const MODE_CHIP_ICON: f32 = 15.0;
/// Pink `pickers.rs`: the permission popover is 340 wide.
const ACCESS_MENU_W: f32 = 340.0;
/// Pink `max-w-3xl` (768) less the column's 16 pt gutters.
const DEFAULT_WIDTH: f32 = layout::COMPOSER_MAX_WIDTH - 32.0;
/// A press on an open menu's own chip closes it (Pink
/// `note_trigger_press_matching`); the click that follows must not reopen it.
const REOPEN_GUARD: Duration = Duration::from_millis(250);

type SendHandler = Rc<dyn Fn(&Message, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type NameHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type PullRequestHandler = Rc<dyn Fn(&PullRequest, &mut Window, &mut App)>;

/// A model in the menu.
#[derive(Clone)]
pub struct ComposerModel {
    pub name: SharedString,
    pub description: SharedString,
    pub icon: SharedString,
    /// The reasoning effort ("Low"), the chip's muted suffix while the row
    /// has room (Pink `model_chip_content` `suffix`).
    pub effort: Option<SharedString>,
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
            effort: None,
        }
    }

    pub fn effort(mut self, effort: impl Into<SharedString>) -> Self {
        self.effort = Some(effort.into());
        self
    }

    /// The model and its effort as one label: "Opus 5.5, Low" (Pink
    /// `execution_settings_label`, no dot joiners).
    pub fn label(&self) -> SharedString {
        match &self.effort {
            Some(effort) => format!("{}, {effort}", self.name).into(),
            None => self.name.clone(),
        }
    }
}

/// Quick, balanced and deep.
pub fn default_models() -> Vec<ComposerModel> {
    vec![
        ComposerModel::new("Quick", "Fast replies for everyday questions", ICON_QUICK)
            .effort("Low"),
        ComposerModel::new("Balanced", "Careful writing and analysis", ICON_BALANCED)
            .effort("Medium"),
        ComposerModel::new("Deep", "Thinks longer on hard problems", ICON_DEEP).effort("High"),
    ]
}

/// A file waiting to go with the message.
#[derive(Clone)]
pub struct Attachment {
    pub name: SharedString,
    /// Kind and size ("PDF, 2.4 MB"), shown under the name in the
    /// thumbnail's tooltip.
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

    /// The picture the thumbnail shows (Pink stages image thumbnails).
    pub fn preview(mut self, source: impl Into<SharedString>) -> Self {
        self.preview = Some(source.into());
        self
    }
}

/// What a follow-up typed during a live run does (Pink's follow-up
/// preference).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FollowUp {
    /// Delivered after the running turn ends.
    #[default]
    Queue,
    /// Delivered into the running turn, when the provider steers natively.
    Steer,
}

/// The access mode chosen in the composer (Pink pickers.rs
/// `PERMISSION_OPTIONS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccessMode {
    /// The provider's own reviewer decides within workspace limits.
    #[default]
    ApproveForMe,
    /// The provider's approval prompts are skipped.
    FullAccess,
}

/// How much of the model chip the controls row shows (Pink pickers.rs
/// `ModelChipFit`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModelFit {
    /// Glyph, name and the effort suffix.
    #[default]
    Full,
    /// The effort moves into the tooltip.
    NameOnly,
    /// Only the glyph; the tooltip names the model and its effort.
    IconOnly,
}

/// What the controls row shows at its width (Pink composer.rs
/// `ControlsRowFit`). The library's web search chip takes the Goal chip's
/// place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlsRowFit {
    pub model: ModelFit,
    pub goal_icon_only: bool,
    /// The context-usage chip shows its ring only.
    pub usage_icon_only: bool,
    /// The access chip shows its glyph only, its mode in the tooltip.
    pub access_icon_only: bool,
}

/// Choose the richest controls row that fits `budget` without truncating
/// (Pink composer.rs `controls_row_fit`, PINK 0.30.4): the goal label yields
/// first, then the usage figure, then the access words, then the model's
/// effort, then its name. Every dropped piece stays in a tooltip. `goal`,
/// `usage` and `access` are `(label, icon-only)` widths; the first two only
/// when those chips are shown. Fixed chip geometry scales with `ui_scale`.
pub fn controls_row_fit(
    budget: f32,
    model_full: f32,
    model_name_only: f32,
    goal: Option<(f32, f32)>,
    usage: Option<(f32, f32)>,
    access: (f32, f32),
    ui_scale: f32,
) -> ControlsRowFit {
    let gap = CHIP_GAP * ui_scale;
    let icon_chip = ROW_BUTTON * ui_scale;
    let need = |model: f32, access: f32, goal: Option<f32>, usage: Option<f32>| {
        model
            + gap
            + access
            + goal.map_or(0.0, |goal| gap + goal)
            + usage.map_or(0.0, |usage| gap + usage)
    };
    // (model, goal icon-only, usage icon-only, access icon-only)
    let candidates = [
        (ModelFit::Full, false, false, false),
        (ModelFit::Full, true, false, false),
        (ModelFit::Full, true, true, false),
        (ModelFit::Full, true, true, true),
        (ModelFit::NameOnly, true, true, true),
        (ModelFit::IconOnly, true, true, true),
    ];
    for (model, goal_icon_only, usage_icon_only, access_icon_only) in candidates {
        let model_width = match model {
            ModelFit::Full => model_full,
            ModelFit::NameOnly => model_name_only,
            ModelFit::IconOnly => icon_chip,
        };
        let access_width = if access_icon_only { access.1 } else { access.0 };
        let goal_width = goal.map(|(label, icon)| if goal_icon_only { icon } else { label });
        let usage_width = usage.map(|(label, icon)| if usage_icon_only { icon } else { label });
        if need(model_width, access_width, goal_width, usage_width) <= budget {
            // A chip that isn't shown has nothing to collapse.
            return ControlsRowFit {
                model,
                goal_icon_only: goal.is_some() && goal_icon_only,
                usage_icon_only: usage.is_some() && usage_icon_only,
                access_icon_only,
            };
        }
    }
    ControlsRowFit {
        model: ModelFit::IconOnly,
        goal_icon_only: goal.is_some(),
        usage_icon_only: usage.is_some(),
        access_icon_only: true,
    }
}

/// The field's hint (Pink composer/goals.rs): an existing thread's composer
/// says "Message this thread" (`THREAD_PLACEHOLDER`, PINK 0.30.4), a new
/// thread "Do anything…" (`MESSAGE_PLACEHOLDER`). A host's own hint wins.
pub fn placeholder_for(in_thread: bool, custom: Option<SharedString>) -> SharedString {
    custom.unwrap_or_else(|| {
        if in_thread {
            "Message this thread".into()
        } else {
            "Do anything…".into()
        }
    })
}

/// How a message left the composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// No run was live.
    Send,
    /// Waits for the running turn to end.
    Queue,
    /// Steers the running turn.
    Steer,
}

/// The checkout the thread works in, for the chip above the pill (Pink
/// pickers.rs `render_context_row`, UI rethink D62). Branch first: the
/// worktree folder and another machine follow as plain phrases ("main",
/// "main in pink-ui-2", "main on Studio Mac"), never a prefix or a dot.
#[derive(Clone, Debug)]
pub struct Checkout {
    pub branch: Option<SharedString>,
    /// The worktree's folder, when the checkout is not the project's own.
    pub worktree: Option<SharedString>,
    /// Another machine the thread runs on; `None` is this one.
    pub machine: Option<SharedString>,
    /// The checkout's root folder, which the branch menu ticks (Pink
    /// `source_context.repo_root`).
    pub path: Option<SharedString>,
}

impl Checkout {
    pub fn branch(branch: impl Into<SharedString>) -> Self {
        Self {
            branch: Some(branch.into()),
            worktree: None,
            machine: None,
            path: None,
        }
    }

    /// A thread on another machine with no branch to show.
    pub fn machine_only(machine: impl Into<SharedString>) -> Self {
        Self {
            branch: None,
            worktree: None,
            machine: Some(machine.into()),
            path: None,
        }
    }

    pub fn worktree(mut self, folder: impl Into<SharedString>) -> Self {
        self.worktree = Some(folder.into());
        self
    }

    pub fn machine(mut self, machine: impl Into<SharedString>) -> Self {
        self.machine = Some(machine.into());
        self
    }

    /// The folder the thread runs in; the branch menu ticks the row whose
    /// checkout this is.
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// The chip's label: "main", "main in pink-ui-2", "main on Studio Mac".
    pub fn label(&self) -> SharedString {
        let label = self.branch.as_ref().map(|branch| match &self.worktree {
            Some(folder) => format!("{branch} in {folder}"),
            None => branch.to_string(),
        });
        match (label, &self.machine) {
            (Some(label), Some(machine)) => format!("{label} on {machine}").into(),
            (Some(label), None) => label.into(),
            (None, Some(machine)) => machine.clone(),
            (None, None) => SharedString::default(),
        }
    }

    /// What the chip is, for its accessible name (Pink `TargetChip::noun`).
    fn noun(&self) -> &'static str {
        match (&self.branch, &self.worktree) {
            (Some(_), Some(_)) => "Worktree",
            (Some(_), None) => "Branch",
            (None, _) => "Machine",
        }
    }

    fn glyph(&self) -> &'static str {
        if self.branch.is_some() {
            ICON_BRANCH
        } else {
            ICON_MONITOR
        }
    }
}

/// A row of the checkout chip's branch menu (Pink `pickers.rs` branch menu
/// rows): a branch, and the folder it is checked out in when it is checked
/// out at all. Two rows can share a name: the project folder and a worktree
/// both on `main`.
#[derive(Clone, Debug)]
pub struct BranchRef {
    pub name: SharedString,
    /// The folder this branch is checked out in.
    pub path: Option<SharedString>,
    /// Checked out in the project's own folder (tagged `current`); otherwise
    /// a path makes it a worktree (tagged `worktree`).
    pub current: bool,
}

impl BranchRef {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            path: None,
            current: false,
        }
    }

    /// Checked out in the project's own folder, at `path`.
    pub fn current(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self.current = true;
        self
    }

    /// Checked out in a worktree at `path`.
    pub fn worktree(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self.current = false;
        self
    }

    /// The row's right-aligned muted tag: current beats worktree.
    fn tag(&self) -> Option<&'static str> {
        if self.current {
            Some("current")
        } else if self.path.is_some() {
            Some("worktree")
        } else {
            None
        }
    }

    /// Its accessible name: "main, current", "main, worktree" or "main".
    fn spoken(&self) -> SharedString {
        match self.tag() {
            Some(tag) => format!("{}, {tag}", self.name).into(),
            None => self.name.clone(),
        }
    }

    /// The worktree's folder name, for the chip's "main in storefront-2".
    fn folder(&self) -> Option<SharedString> {
        let path = self.path.as_ref().filter(|_| !self.current)?;
        let trimmed = path.trim_end_matches('/');
        Some(
            trimmed
                .rsplit('/')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or(trimmed)
                .to_string()
                .into(),
        )
    }
}

impl From<&str> for BranchRef {
    fn from(name: &str) -> Self {
        Self::new(name.to_string())
    }
}

impl From<String> for BranchRef {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl From<SharedString> for BranchRef {
    fn from(name: SharedString) -> Self {
        Self::new(name)
    }
}

/// Whether two checkout paths name the same folder, ignoring a trailing
/// slash (Pink pickers.rs `same_checkout_path`).
fn same_checkout_path(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// The branch menu row to tick (Pink pickers.rs branch menu, PINK 0.30.3):
/// with a known checkout path, the row checked out in that folder, since two
/// checkouts can both be on `main`, and a row with no folder is never it;
/// without one, the row named like the checkout's branch.
pub fn ticked_branch(rows: &[BranchRef], checkout: Option<&Checkout>) -> Option<usize> {
    let checkout = checkout?;
    match &checkout.path {
        Some(root) => rows.iter().position(|row| {
            row.path
                .as_ref()
                .is_some_and(|path| same_checkout_path(root, path))
        }),
        None => {
            let branch = checkout.branch.as_ref()?;
            rows.iter().position(|row| row.name == *branch)
        }
    }
}

/// A pull request's state; an open draft says Draft (Pink
/// change_requests.rs `association_state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullRequestState {
    Open,
    Draft,
    Merged,
    Closed,
}

impl PullRequestState {
    fn word(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Draft => "Draft",
            Self::Merged => "Merged",
            Self::Closed => "Closed",
        }
    }

    /// Pink `ChangeRequestBadgeTone::color`.
    fn tone(self, t: &Pink) -> gpui::Hsla {
        match self {
            Self::Open => t.success,
            Self::Draft => t.text_muted,
            Self::Merged => t.code_text,
            Self::Closed => t.danger,
        }
    }
}

/// One pull request recorded for the thread and its checkout.
#[derive(Clone, Debug)]
pub struct PullRequest {
    /// `owner/repo`.
    pub repository: SharedString,
    pub number: u32,
    pub title: SharedString,
    pub state: PullRequestState,
}

impl PullRequest {
    pub fn new(
        repository: impl Into<SharedString>,
        number: u32,
        title: impl Into<SharedString>,
        state: PullRequestState,
    ) -> Self {
        Self {
            repository: repository.into(),
            number,
            title: title.into(),
            state,
        }
    }

    /// Its accessible name (Pink `composer_task_pull_request_chip`):
    /// "Pull request owner/repo#101, Draft: Fix the flaky login test".
    pub fn spoken(&self) -> SharedString {
        format!(
            "Pull request {}#{}, {}: {}",
            self.repository,
            self.number,
            self.state.word(),
            self.title.replace(['\r', '\n'], " ")
        )
        .into()
    }
}

/// What `on_send` receives.
#[derive(Clone)]
pub struct Message {
    pub text: SharedString,
    pub attachments: Vec<Attachment>,
    pub model: SharedString,
    /// The branch the chip names when it was sent.
    pub branch: Option<SharedString>,
    pub web_search: bool,
    /// The access mode it was sent with.
    pub access: AccessMode,
    /// Sent while a turn was running: it waits for that turn to end, or
    /// steers it (`delivery`).
    pub queued: bool,
    pub delivery: Delivery,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Model,
    Access,
    Branch,
}

/// The primary button's job (Pink `SendButtonMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SendMode {
    /// No live run: plain send.
    Send,
    /// Live run, a draft typed, delivered after the current turn.
    Queue,
    /// Live run, a draft typed, delivered into the running turn: the
    /// follow-up preference is Steer and the provider steers natively now.
    Steer,
    /// Live run, nothing typed: the stop square alone.
    Stop,
}

/// Pink `send_button_mode`: the button is named for the delivery that
/// actually happens. `steers` is whether a follow-up sent now would steer
/// the running turn (preference and provider both allow it).
fn send_mode(running: bool, has_content: bool, steers: bool) -> SendMode {
    match (running, has_content) {
        (false, _) => SendMode::Send,
        (true, true) if steers => SendMode::Steer,
        (true, true) => SendMode::Queue,
        (true, false) => SendMode::Stop,
    }
}

/// Room the separate Stop takes in the controls row while a run is live
/// (Pink `stop_reserve`, UI rethink D70: Stop stays reachable while a draft
/// is typed).
fn stop_reserve(running: bool, scale: f32) -> f32 {
    if running {
        stop_size(scale) + STOP_GAP * scale
    } else {
        0.0
    }
}

/// Stop's size: it scales with the interface but never drops below 28 pt.
fn stop_size(scale: f32) -> f32 {
    (ROW_BUTTON * scale).max(STOP_MIN)
}

struct State {
    input: Entity<TextInput>,
    menu_focus: FocusHandle,
    has_text: bool,
    model: usize,
    /// The branch row picked from the menu; `None` follows the checkout.
    branch: Option<usize>,
    /// The row the checkout runs in (`ticked_branch`), refreshed every render.
    ticked: Option<usize>,
    web: bool,
    ask_first: bool,
    menu: Option<Menu>,
    /// The menu that closed last and when: it fades out over `MENU_OUT`
    /// (Pink's `Popup` closing phase), and its chip ignores the click that
    /// closed it.
    closed: Option<(Menu, Instant)>,
    highlight: usize,
    listen_started: Option<Instant>,
    placeholder: SharedString,
    running: bool,
    /// A follow-up sent now steers the running turn.
    steers: bool,
    /// The draft the host asked for, applied once.
    draft_applied: bool,
    /// The access mode the host asked for, applied once.
    access_applied: bool,
    /// The pill's natural content height, measured after layout; the pill
    /// eases toward it (Pink's committed-height morph).
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
            branch: None,
            ticked: None,
            web: false,
            ask_first: true,
            menu: None,
            closed: None,
            highlight: 0,
            listen_started: None,
            placeholder: SharedString::default(),
            running: false,
            steers: false,
            draft_applied: false,
            access_applied: false,
            natural: Rc::default(),
            snapshot: None,
            on_send: None,
            _input_events: sub,
        }
    }

    /// Pink `on_submit`: Enter never stops a run, and an empty draft sends
    /// nothing; a running turn queues the message.
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text: SharedString = self.input.read(cx).text().trim().to_string().into();
        let Some((attachments, model, branch)) = self.snapshot.clone() else {
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
            branch,
            web_search: self.web,
            access: if self.ask_first {
                AccessMode::ApproveForMe
            } else {
                AccessMode::FullAccess
            },
            queued: self.running,
            delivery: match (self.running, self.steers) {
                (false, _) => Delivery::Send,
                (true, true) => Delivery::Steer,
                (true, false) => Delivery::Queue,
            },
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
            Menu::Branch => self.branch.or(self.ticked).unwrap_or(0),
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
    checkout: Option<Checkout>,
    branches: Vec<BranchRef>,
    in_thread: bool,
    pull_requests: Vec<PullRequest>,
    attachments: Vec<Attachment>,
    placeholder: Option<SharedString>,
    draft: Option<SharedString>,
    access: Option<AccessMode>,
    listening: Option<SharedString>,
    running: bool,
    follow_up: FollowUp,
    steer_available: bool,
    width: f32,
    on_send: Option<SendHandler>,
    on_stop: Option<Handler>,
    on_attach: Option<Handler>,
    on_remove: Option<IndexHandler>,
    on_listen: Option<ToggleHandler>,
    on_branch: Option<NameHandler>,
    on_pull_request: Option<PullRequestHandler>,
}

impl ChatComposer {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            models: default_models(),
            checkout: None,
            branches: Vec::new(),
            in_thread: false,
            pull_requests: Vec::new(),
            attachments: Vec::new(),
            placeholder: None,
            draft: None,
            access: None,
            listening: None,
            running: false,
            follow_up: FollowUp::Queue,
            steer_available: false,
            width: DEFAULT_WIDTH,
            on_send: None,
            on_stop: None,
            on_attach: None,
            on_remove: None,
            on_listen: None,
            on_branch: None,
            on_pull_request: None,
        }
    }

    pub fn models(mut self, models: Vec<ComposerModel>) -> Self {
        self.models = models;
        self
    }

    /// The thread's checkout, named by the chip above the pill; the row
    /// hides without one (or without `branches`).
    pub fn checkout(mut self, checkout: Checkout) -> Self {
        self.checkout = Some(checkout);
        self
    }

    /// Branches the checkout chip's menu offers, each with the folder it is
    /// checked out in (plain names convert); picking one renames the chip
    /// and calls `on_branch`. The menu ticks the row the checkout runs in.
    pub fn branches(mut self, branches: impl IntoIterator<Item = impl Into<BranchRef>>) -> Self {
        self.branches = branches.into_iter().map(Into::into).collect();
        self
    }

    /// The composer writes to an existing thread: its checkout chip and pull
    /// requests ride the controls row (Pink `render_context_row`). A new
    /// thread's checkout sits in a row above the pill (Pink
    /// `render_target_footer`).
    pub fn in_thread(mut self, in_thread: bool) -> Self {
        self.in_thread = in_thread;
        self
    }

    /// Every pull request recorded for the thread and its checkout, each a
    /// chip with its own state beside the checkout chip.
    pub fn pull_requests(mut self, pull_requests: Vec<PullRequest>) -> Self {
        self.pull_requests = pull_requests;
        self
    }

    /// The access mode chosen when the composer first appears (Approve for
    /// me by default); the chip's menu changes it from there.
    pub fn access(mut self, access: AccessMode) -> Self {
        self.access = Some(access);
        self
    }

    /// Text placed in the field when the composer first appears.
    pub fn draft(mut self, text: impl Into<SharedString>) -> Self {
        self.draft = Some(text.into());
        self
    }

    pub fn attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = attachments;
        self
    }

    /// The empty field's hint. By default "Message this thread" in an
    /// existing thread and "Do anything…" in a new one ([`placeholder_for`]).
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Voice input in progress, with the words heard so far.
    pub fn listening(mut self, transcript: Option<SharedString>) -> Self {
        self.listening = transcript;
        self
    }

    /// A run is live: a separate Stop sits beside the submit button, which
    /// queues or steers a draft; with an empty draft Stop shows alone.
    pub fn running(mut self, running: bool) -> Self {
        self.running = running;
        self
    }

    /// The follow-up preference for a draft sent during a live run.
    pub fn follow_up(mut self, follow_up: FollowUp) -> Self {
        self.follow_up = follow_up;
        self
    }

    /// The provider steers the running turn natively right now. Only then
    /// does the Steer preference name the button Steer this turn.
    pub fn steer_available(mut self, available: bool) -> Self {
        self.steer_available = available;
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

    /// Stop, beside the submit button or alone. Stopping keeps the draft.
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

    /// A branch picked from the checkout chip's menu.
    pub fn on_branch(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_branch = Some(Rc::new(handler));
        self
    }

    /// A pull request chip was clicked: open it (Pink opens its URL).
    pub fn on_pull_request(
        mut self,
        handler: impl Fn(&PullRequest, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_pull_request = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------

/// A 28 pt round utility button (Pink: `Composer::render` attach): clear at
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

/// The submit disc (Pink `Composer::render_send_button`): one stable 28 pt
/// circle named for what it does. Send (arrow up) with no live run; Queue
/// for after this turn (queue glyph) or Steer this turn (arrow up-right)
/// while one is; Stop (the square) when a live run has an empty draft.
/// Solid `text` with a `bg` glyph when it can act (hover 0.88, press 0.76);
/// recessed `ink(0.08)` with the glyph at 35% while there is nothing to
/// send, the arrow taking the accent when the field is focused and empty.
/// Only the glyph changes between modes, fading in over `GLYPH_SWAP`.
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
        (SendMode::Steer, false) => "steer",
        (_, false) => "send",
        (_, true) if ready => "ready",
        (_, true) => "rest",
    };
    let label = match mode {
        SendMode::Stop => "Stop",
        SendMode::Queue => "Queue for after this turn (⏎)",
        SendMode::Steer => "Steer this turn (⏎)",
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
        SendMode::Steer => svg()
            .path(ICON_ARROW_UP_RIGHT)
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
    let selector = fade.clone();
    let disc = div()
        .id(SharedString::from(fade.clone()))
        .debug_selector(move || selector)
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

/// The separate Stop shown beside Queue or Steer while a run is live (Pink
/// `Composer::render_stop_button`, UI rethink D70): the secondary action, a
/// quiet `ink(0.08)` fill easing to 0.14 with the 11 pt stop square in
/// `text`; at least 28 pt at every Interface size, named, and reachable by
/// Tab and Return or Space. Stopping keeps the draft.
fn stop_button(key: &str, scale: f32, t: &Pink) -> Stateful<Div> {
    let fade = format!("{key}-stop");
    let ring = t.focus_ring();
    let selector = fade.clone();
    div()
        .id(SharedString::from(fade.clone()))
        .debug_selector(move || selector)
        .role(Role::Button)
        .aria_label("Stop")
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .size(px(stop_size(scale)))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .bg(motion::hover_blend(&fade, t.ink(0.08), t.ink(0.14)))
        .on_hover(motion::hover_listener(fade))
        .active(|s| s.opacity(0.76))
        .tooltip(pink::text_tooltip("Stop"))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(div().size(px(11.)).rounded(px(3.)).bg(t.text))
}

/// The staged-attachment strip (Pink `Composer::render_attachment_strip`):
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
                                "{}\n{}",
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
                            // A 20 px disc 7 px over the corner, in a
                            // `MIN_HIT` box so the target is never under 24.
                            .top(px(-7. - pink::chrome::hit_outset(20.)))
                            .right(px(-7. - pink::chrome::hit_outset(20.)))
                            .size(px(pink::chrome::MIN_HIT))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
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
                            .child(
                                div()
                                    .size(px(20.))
                                    .rounded_full()
                                    .bg(t.surface_raised)
                                    .border_1()
                                    .border_color(t.border_strong)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .shadow(pink::shadow_sm())
                                    .child(svg().path(ICON_CLOSE).size(px(10.)).text_color(t.text)),
                            ),
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

/// A chip in the checkout row above the pill (Pink `Pickers::target_chip`):
/// 22 pt, `px 6`, radius 6, a 12 pt glyph and 11.5 pt label in
/// `readable_muted(0.7)` brightening to `text@0.85`, the Tertiary wash on
/// hover or while open, and a 10 pt chevron. Named "{noun}: {label}"
/// ("Branch: main", "Worktree: main in pink-ui-2"), up to 360 wide.
fn target_chip(
    id: impl Into<SharedString>,
    glyph: &'static str,
    noun: &'static str,
    label: SharedString,
    open: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let key = id.to_string();
    let label_selector = format!("{key}-label");
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
        .aria_label(SharedString::from(format!("{noun}: {label}")))
        .aria_expanded(open)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .relative()
        // Drawn 22 tall, it takes a `MIN_HIT` target reaching into the row.
        .h(px(pink::chrome::MIN_HIT))
        .my(px(-(pink::chrome::MIN_HIT - TARGET_CHIP_HEIGHT) / 2.))
        .min_w_0()
        .max_w(px(360.))
        .flex()
        .items_center()
        .rounded(px(TARGET_CHIP_RADIUS))
        .cursor_pointer()
        .on_hover(motion::hover_listener(key.clone()))
        .child(
            div()
                .h(px(TARGET_CHIP_HEIGHT))
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(TARGET_CHIP_GAP))
                .px(px(TARGET_CHIP_PAD_X))
                .rounded(px(TARGET_CHIP_RADIUS))
                .text_size(px(TARGET_CHIP_TEXT))
                .text_color(ink)
                .bg(if open {
                    wash
                } else {
                    motion::hover_blend(&key, wash.opacity(0.), wash)
                })
                .child(
                    svg()
                        .path(glyph)
                        .size(px(TARGET_CHIP_GLYPH))
                        .flex_none()
                        .text_color(ink),
                )
                .child(
                    div()
                        .debug_selector(move || label_selector)
                        .min_w_0()
                        .truncate()
                        .child(label),
                )
                .child(pink::line_glyph(
                    pink::glyphs::ALT_ARROW_DOWN,
                    TARGET_CHIP_CHEVRON,
                    t.readable_muted(0.5),
                )),
        )
}

/// One pull request of the thread's collection (Pink change_requests.rs
/// `composer_task_pull_request_chip`): a 20 pt chip, `px 7`, radius 6, an
/// 11 pt pull request glyph and the number in mono, and the word Draft for
/// an open draft, in its state's tone (Open green, Draft muted, Merged the
/// code tone, Closed red) washed 0.08, 0.16 on hover. A named link:
/// "Pull request owner/repo#101, Draft: Fix the flaky login test". It draws
/// 20 tall but takes a 24 pt target, reaching into the row's height.
fn pull_request_chip(key: &str, pr: &PullRequest, t: &Pink) -> Stateful<Div> {
    let id = format!("{key}-pr-{}", pr.number);
    let tone = pr.state.tone(t);
    let ink = t.readable_tone(tone, 0.85);
    let selector = id.clone();
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || selector)
        .role(Role::Link)
        .aria_label(pr.spoken())
        .tab_index(0)
        .focus_visible({
            let ring = t.focus_ring();
            move |s| pink::focus_ring(s, ring)
        })
        .on_key_down(pink::swallow_activation_key)
        .h(px(pink::chrome::MIN_HIT))
        .my(px(-(pink::chrome::MIN_HIT - 20.) / 2.))
        .flex_none()
        .flex()
        .items_center()
        .rounded(px(6.))
        .cursor_pointer()
        .on_hover(motion::hover_listener(id.clone()))
        .tooltip(pink::text_tooltip(format!(
            "{}#{}, {}\n{}",
            pr.repository,
            pr.number,
            pr.state.word(),
            pr.title
        )))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            div()
                .h(px(20.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(5.))
                .px(px(7.))
                .rounded(px(6.))
                .bg(motion::hover_blend(
                    &id,
                    tone.opacity(0.08),
                    tone.opacity(0.16),
                ))
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(ink)
                .child(
                    svg()
                        .path(ICON_PULL_REQUEST)
                        .size(px(11.))
                        .flex_none()
                        .text_color(ink),
                )
                .child(
                    div()
                        .font_family(pink::MONO)
                        .child(format!("#{}", pr.number)),
                )
                .when(pr.state == PullRequestState::Draft, |el| el.child("Draft")),
        )
}

/// A menu floating above its chip, from the chip's leading edge (Pink
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

/// The popover card around a menu's rows (Pink `Pickers::popover_frame`):
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

/// A popover's section label (Pink `render_permission_popover` header):
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

/// A model row (Pink `Pickers::render_model_row`, two-line layout): `px 8
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

/// The access choices (Pink pickers.rs `PERMISSION_OPTIONS`, PINK 0.30.4):
/// label, description and whether the choice wears the warning tone.
const ACCESS: [(&str, &str, bool); 2] = [
    (
        "Approve for me",
        "Keep workspace limits and let the provider's native reviewer decide; anything it won't approve is declined, never asked.",
        false,
    ),
    (
        "Full access",
        "Skip this execution provider's approval prompts; app, OS, and external controls still apply.",
        true,
    ),
];

/// An access row (Pink `Pickers::render_permission_popover`): the shared
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

/// A branch menu row (Pink pickers.rs branch menu, PINK 0.30.3): the name
/// and a right-aligned muted 11 pt tag, `current` beating `worktree`. The
/// ticked row is the checkout the thread runs in. Named "main, current" or
/// "main, worktree" as a selectable option.
fn branch_row(
    ix: usize,
    row: &BranchRef,
    ticked: bool,
    active: bool,
    state: &Entity<State>,
    on_branch: Option<NameHandler>,
    t: &Pink,
) -> Stateful<Div> {
    let hover = state.clone();
    let pick = state.clone();
    let picked = row.name.clone();
    pink::menu_row_nav(ticked, active, t)
        .id(ElementId::NamedInteger("cc-branch-row".into(), ix as u64))
        .role(Role::ListBoxOption)
        .aria_label(row.spoken())
        .aria_selected(ticked)
        .on_mouse_move(move |_, _, cx| set_highlight(&hover, ix, cx))
        .on_click(move |_, window, cx| {
            choose(&pick, Menu::Branch, ix, window, cx);
            if let Some(f) = &on_branch {
                f(&picked, window, cx)
            }
        })
        .child(div().flex_1().min_w_0().truncate().child(row.name.clone()))
        .when_some(row.tag(), |el, tag| {
            el.child(
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(tag),
            )
        })
}

fn choose(state: &Entity<State>, kind: Menu, ix: usize, window: &mut Window, cx: &mut App) {
    let input = state.update(cx, |s, cx| {
        match kind {
            Menu::Model => s.model = ix,
            Menu::Access => s.ask_first = ix == 0,
            Menu::Branch => s.branch = Some(ix),
        }
        s.close_menu();
        cx.notify();
        s.input.clone()
    });
    TextInput::focus(&input, window, cx);
}

/// Pink pickers.rs `chip_text_width`: `text` at the chips' 12 px medium.
fn chip_text_width(text: &str, window: &Window) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let run = gpui::TextRun {
        len: text.len(),
        font: gpui::Font {
            weight: FontWeight::MEDIUM,
            ..gpui::font(pink::SANS)
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window.text_system().shape_line(
        SharedString::from(text.to_string()),
        px(CHIP_TEXT),
        &[run],
        None,
    );
    f32::from(line.width)
}

/// The branch chip's natural width (Pink pickers.rs `render_context_row`
/// `branch_width`): its label, both paddings, the glyph, the chevron and
/// the two gaps between them.
fn branch_chip_width(label: &str, window: &Window, scale: f32) -> f32 {
    chip_text_width(label, window)
        + (2.0 * TARGET_CHIP_PAD_X
            + TARGET_CHIP_GLYPH
            + TARGET_CHIP_CHEVRON
            + 2.0 * TARGET_CHIP_GAP)
            * scale
}

/// What a PR chip adds beyond its texts (Pink change_requests.rs
/// `composer_task_pull_request_chrome`): both side paddings, the glyph and
/// the gap after it, the gap before Draft and the row's gap before the chip.
pub fn pull_request_chrome(draft: bool, scale: f32) -> f32 {
    let gap = 5.0;
    2.0 * 7.0 + 11.0 * scale + gap + if draft { gap } else { 0.0 } + 2.0 * scale
}

/// The PR chips' natural width together (Pink `render_context_row`
/// `pr_width`): each chip's `#N`, its Draft word and its chrome.
fn pull_requests_width(prs: &[PullRequest], window: &Window, scale: f32) -> f32 {
    prs.iter()
        .map(|pr| {
            let draft = pr.state == PullRequestState::Draft;
            chip_text_width(&format!("#{}", pr.number), window)
                + if draft {
                    chip_text_width("Draft", window)
                } else {
                    0.0
                }
                + pull_request_chrome(draft, scale)
        })
        .sum()
}

/// The controls row's room for the branch chip and for the PR chips (Pink
/// composer.rs `context_width`, PINK 0.30.3): the branch keeps its natural
/// width up to 35% of the row and 240 pt, and the PRs reserve their own room
/// on top, up to 30% of the row, so listing every PR never truncates the
/// branch.
pub fn context_reserve(branch: f32, prs: f32, row_budget: f32, scale: f32) -> (f32, f32) {
    (
        branch.min(row_budget * 0.35).min(240.0 * scale),
        prs.min(row_budget * 0.3),
    )
}

/// The controls row's room for chips in a pill `width` wide (Pink
/// composer.rs `controls_row_budget` less `stop_reserve`): the hairline, the
/// row's insets, attach, the mic and the submit disc, and Stop while a run
/// is live.
fn controls_row_budget(width: f32, running: bool, scale: f32) -> f32 {
    width
        - PILL_BORDER
        - 2.0 * ACTIONS_ROW_INSET
        - (ROW_BUTTON + ACTION_UTILITY_GAP)
        - 2.0 * (ACTION_PRIMARY_GAP + ROW_BUTTON)
        - stop_reserve(running, scale)
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
        let branch_count = self.branches.len();
        let colors = (t.placeholder, t.caret, t.selection);
        let attachments = self.attachments.clone();
        let models = self.models.clone();
        let branches = self.branches.clone();
        let checkout_branch = self
            .checkout
            .as_ref()
            .and_then(|c| c.branch.clone())
            .or_else(|| {
                // No checkout: the menu's first branch names the chip.
                self.checkout
                    .is_none()
                    .then(|| self.branches.first().map(|b| b.name.clone()))
                    .flatten()
            });
        let ticked = ticked_branch(&self.branches, self.checkout.as_ref());
        let on_send = self.on_send.clone();
        let placeholder = placeholder_for(self.in_thread, self.placeholder.clone());
        let running = self.running;
        // A follow-up steers only when the preference is Steer and the
        // provider steers the running turn natively.
        let steers = self.follow_up == FollowUp::Steer && self.steer_available;
        let draft = self.draft.clone();
        let access = self.access;
        state.update(cx, |s, cx| {
            if let Some(access) = access.filter(|_| !s.access_applied) {
                s.access_applied = true;
                s.ask_first = access == AccessMode::ApproveForMe;
            }
            s.model = s.model.min(model_count - 1);
            s.branch = s.branch.filter(|&ix| ix < branch_count);
            s.ticked = ticked;
            s.snapshot = Some((
                attachments,
                models
                    .get(s.model)
                    .map(|m| m.name.clone())
                    .unwrap_or_default(),
                s.branch
                    .and_then(|ix| branches.get(ix))
                    .map(|b| b.name.clone())
                    .or(checkout_branch),
            ));
            s.on_send = on_send;
            s.running = running;
            s.steers = steers;
            if let Some(draft) = draft.filter(|_| !s.draft_applied) {
                s.draft_applied = true;
                s.has_text = !draft.trim().is_empty();
                s.input
                    .update(cx, |i, cx| i.set_text(draft.to_string(), cx));
            }
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
        let (model_ix, branch_ix, web, ask_first, menu, highlight) =
            (s.model, s.branch, s.web, s.ask_first, s.menu, s.highlight);
        let mounted = |kind| s.mounted(kind);
        let (model_menu, access_menu, branch_menu) = (
            mounted(Menu::Model),
            mounted(Menu::Access),
            mounted(Menu::Branch),
        );
        let has_content = s.has_text || !self.attachments.is_empty();
        let listen_started = s.listen_started;
        let natural = s.natural.clone();
        let menu_focus = s.menu_focus.clone();
        let focused = input.read(cx).is_focused(window);
        if [model_menu, access_menu, branch_menu]
            .iter()
            .any(|m| matches!(m, Some(Some(_))))
        {
            // Keep the exit fade moving to its end.
            window.request_animation_frame();
        }

        // ---- attachments ------------------------------------------------------
        let strip = attachment_strip(&key, &self.attachments, self.on_remove.clone(), &t);

        // ---- the field (Pink: textarea `px-4 pt-4 pb-1`, 76/60 → 260) ----------
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

        let scale = pink::ui_scale(cx);
        // ---- the checkout chip and the thread's PRs (Pink `render_context_row`)
        let picked = branch_ix.and_then(|ix| self.branches.get(ix));
        let checkout = match (&self.checkout, picked) {
            // A pick renames the branch; a row checked out elsewhere moves
            // the chip to that folder.
            (Some(checkout), Some(row)) => Some(Checkout {
                branch: Some(row.name.clone()),
                worktree: if row.current {
                    None
                } else {
                    row.folder().or_else(|| checkout.worktree.clone())
                },
                path: row.path.clone().or_else(|| checkout.path.clone()),
                ..checkout.clone()
            }),
            (Some(checkout), None) => Some(checkout.clone()),
            (None, row) => row.or(self.branches.first()).map(|row| Checkout {
                worktree: row.folder(),
                path: row.path.clone(),
                ..Checkout::branch(row.name.clone())
            }),
        }
        .filter(|checkout| !checkout.label().is_empty());
        let menu_ticked = branch_ix.or(ticked);
        let context = checkout.map(|checkout| {
            let label = checkout.label();
            let chip_selector = format!("{key}-checkout");
            let chip = target_chip(
                chip_selector.clone(),
                checkout.glyph(),
                checkout.noun(),
                label.clone(),
                menu == Some(Menu::Branch),
                &t,
            )
            .debug_selector(move || chip_selector)
            .when(!self.branches.is_empty(), |chip| {
                chip.on_click({
                    let state = state.clone();
                    move |_, window, cx| toggle_menu(&state, Menu::Branch, window, cx)
                })
            })
            .when_some(branch_menu, |d, exit| {
                let card = menu_card(
                    Menu::Branch,
                    self.branches.len(),
                    layout::MENU_W_M,
                    exit.is_none(),
                    &menu_focus,
                    &state,
                    &t,
                )
                .child(
                    div()
                        .id("cc-branch-list")
                        .flex()
                        .flex_col()
                        .gap(px(layout::MENU_GAP))
                        .max_h(px(224.))
                        .overflow_y_scroll()
                        .children(self.branches.iter().enumerate().map(|(ix, row)| {
                            branch_row(
                                ix,
                                row,
                                menu_ticked == Some(ix),
                                ix == highlight,
                                &state,
                                self.on_branch.clone(),
                                &t,
                            )
                        })),
                );
                d.child(menu_above("cc-branch-menu", card.into_any_element(), exit))
            });
            let prs: Vec<_> = self
                .pull_requests
                .iter()
                .map(|pr| {
                    let on_pull_request = self.on_pull_request.clone();
                    let opened = pr.clone();
                    pull_request_chip(&key, pr, &t).on_click(move |_, window, cx| {
                        if let Some(f) = &on_pull_request {
                            f(&opened, window, cx)
                        }
                    })
                })
                .collect();
            (label, chip, prs)
        });
        let in_thread = self.in_thread;
        let mut context_width = 0.0;
        let (thread_context, context_row) = match context {
            // An existing thread (Pink `render_context_row` in the controls
            // row): the branch chip keeps its natural width up to its share,
            // and the PRs get their own share beside it.
            Some((label, chip, prs)) if in_thread => {
                let budget = controls_row_budget(self.width, running, scale);
                let (branch_share, pr_share) = context_reserve(
                    branch_chip_width(&label, window, CHIP_SCALE),
                    pull_requests_width(&self.pull_requests, window, CHIP_SCALE),
                    budget,
                    CHIP_SCALE,
                );
                context_width = branch_share + pr_share;
                // The reserve never yields to the pickers. Inside it the PR
                // chips keep their size; only when they outgrow their own
                // share does the branch chip (shrinkable, up to 360) give way,
                // and nothing paints past the reserve.
                let row = div()
                    .w(px(branch_share + pr_share))
                    .flex_none()
                    .overflow_hidden()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .child(chip)
                    .children(prs);
                (Some(row), None)
            }
            // A new thread: its checkout in the row above the pill (Pink
            // `render_target_footer`).
            Some((_, chip, prs)) => (
                None,
                Some(
                    div()
                        .w_full()
                        .h(px(CONTEXT_ROW_HEIGHT))
                        // Close to the pill it describes: half the column gap.
                        .mb(px(-COLUMN_GAP / 2.))
                        .px(px(6.))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(2.))
                        .child(chip)
                        .children(prs),
                ),
            ),
            None => (None, None),
        };

        // ---- the controls row's fit (Pink `controls_row_fit`) ---------------
        // Trigger chips are `px 10`, a 16 pt glyph and a 6 pt gap before the
        // text (Pink pickers.rs `CHIP_PAD_X`, `CHIP_ICON`, `CHIP_GAP`); one
        // pixel of slack per measured run absorbs subpixel rounding.
        let chip_chrome = (2.0 * 10.0 + 16.0 + 6.0) * CHIP_SCALE;
        let model = self.models.get(model_ix).cloned();
        let model_name_only = model.as_ref().map_or(ROW_BUTTON, |m| {
            chip_chrome + chip_text_width(&m.name, window) + 1.0
        });
        let model_full = model_name_only
            + model
                .as_ref()
                .and_then(|m| m.effort.as_ref())
                .map_or(0.0, |effort| {
                    6.0 * CHIP_SCALE + chip_text_width(effort, window) + 1.0
                });
        let access_ix = usize::from(!ask_first);
        let access_widths = (
            chip_chrome + chip_text_width(ACCESS[access_ix].0, window) + 1.0,
            ROW_BUTTON * CHIP_SCALE,
        );
        let search_widths = (
            chip_chrome + chip_text_width("Search", window) + 1.0,
            ROW_BUTTON * CHIP_SCALE,
        );
        let picker_budget = controls_row_budget(self.width, running, scale)
            - if context_width > 0.0 {
                context_width + ACTION_UTILITY_GAP
            } else {
                0.0
            };
        let fit = controls_row_fit(
            picker_budget,
            model_full,
            model_name_only,
            Some(search_widths),
            None,
            access_widths,
            CHIP_SCALE,
        );

        // ---- controls row (Pink: actions row) ---------------------------------
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

        // The model chip (Pink `model_chip_content`): glyph, name and the
        // effort as a muted suffix while the row has room; the tooltip always
        // names both, "Balanced, Medium".
        let model_chip =
            pink::trigger_chip(format!("{key}-model"), true, menu == Some(Menu::Model), &t)
                .relative()
                .on_click({
                    let state = state.clone();
                    move |_, window, cx| toggle_menu(&state, Menu::Model, window, cx)
                })
                .when(fit.model == ModelFit::IconOnly, |d| {
                    d.flex_none().w(px(ROW_BUTTON)).px(px(0.)).justify_center()
                })
                .when_some(model, |d, m| {
                    let label = m.label();
                    d.aria_label(label.clone())
                        .when(menu != Some(Menu::Model), |d| {
                            d.tooltip(pink::text_tooltip(label))
                                .tooltip_show_delay(layout::TOOLTIP_DELAY)
                        })
                        .child(
                            svg()
                                .path(m.icon)
                                .size(px(16.))
                                .flex_none()
                                .text_color(t.text_muted),
                        )
                        .when(fit.model != ModelFit::IconOnly, |d| {
                            d.child(div().min_w_0().truncate().child(m.name))
                        })
                        .when_some(m.effort.filter(|_| fit.model == ModelFit::Full), |d, e| {
                            d.child(div().flex_none().text_color(t.readable_muted(0.7)).child(e))
                        })
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

        // The access chip names its mode in words beside its glyph (Pink
        // pickers.rs `access_chip_content`, PINK 0.30.4): Approve for me a
        // shield check, Full access an open lock in the warning tone. Glyph
        // only when the row is too narrow; the tooltip names it either way.
        let (access_title, access_icon, access_tint) = if ask_first {
            (ACCESS[0].0, ICON_SHIELD_CHECK, t.text_muted)
        } else {
            (ACCESS[1].0, ICON_UNLOCKED, t.warning)
        };
        let access_selector = format!("{key}-access");
        let access_label_selector = format!("{key}-access-label");
        let access_chip = pink::trigger_chip(
            format!("{key}-access"),
            true,
            menu == Some(Menu::Access),
            &t,
        )
        .debug_selector(move || access_selector)
        .relative()
        .flex_none()
        .when(fit.access_icon_only, |d| {
            d.w(px(ROW_BUTTON)).px(px(0.)).justify_center()
        })
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
        .when(!fit.access_icon_only, |d| {
            d.child(
                div()
                    .debug_selector(move || access_label_selector)
                    .flex_none()
                    .child(access_title),
            )
        })
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

        let search_chip = search_chip(format!("{key}-search"), web, fit.goal_icon_only, &t)
            .on_click({
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

        let mode = send_mode(running, has_content, steers);
        let recessed = mode != SendMode::Stop && (!has_content || listening);
        let on_stop = self.on_stop.clone();
        let submit =
            send_disc(&key, mode, recessed, focused && !has_content, &t).when(!recessed, |d| {
                let state = state.clone();
                let on_stop = on_stop.clone();
                d.on_click(move |_, window, cx| match mode {
                    SendMode::Stop => {
                        if let Some(f) = &on_stop {
                            f(window, cx)
                        }
                    }
                    _ => state.update(cx, |s, cx| s.send(window, cx)),
                })
            });
        // A typed draft never hides Stop (Pink `Composer::render`): while a
        // run is live, the submit button (Queue or Steer) sits beside a
        // separate Stop, `STOP_GAP` apart. With an empty draft Stop shows
        // alone, and the row still reserves the separate Stop's width so
        // nothing moves when typing starts (`stop_reserve`).
        let send = match mode {
            SendMode::Queue | SendMode::Steer => div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(STOP_GAP * scale))
                .child(stop_button(&key, scale, &t).on_click(move |_, window, cx| {
                    if let Some(f) = &on_stop {
                        f(window, cx)
                    }
                }))
                .child(submit),
            SendMode::Stop => div()
                .flex_none()
                .flex()
                .flex_row()
                .items_center()
                .child(div().flex_none().w(px(stop_reserve(running, scale))))
                .child(submit),
            SendMode::Send => div().flex_none().child(submit),
        };

        let controls_selector = format!("{key}-controls");
        let controls = div()
            .debug_selector(move || controls_selector)
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
                    )
                    .children(thread_context),
            )
            .child(mic)
            .child(send);

        // ---- the pill (Pink `composer_surface`, the frosted plate) ------------
        // The pill's height eases to its content over `COLLAPSE` (Pink's
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
        let pill_selector = format!("{key}-pill");
        let pill = div()
            .id(SharedString::from(format!("{key}-pill")))
            .debug_selector(move || pill_selector)
            .relative()
            .w_full()
            .flex_none()
            .rounded(px(radius::COMPOSER_PILL))
            .border_1()
            .border_color(t.border)
            .bg(t.surface_bg())
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

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .flex()
            .flex_col()
            .gap(px(COLUMN_GAP))
            .font_family(pink::SANS)
            .children(context_row)
            .child(pill)
    }
}

/// The web search switch. Off it is a ghost trigger chip (Pink
/// `Pickers::trigger_chip`); on it wears the mode-on capsule of Pink's Goal
/// chip (`goals.rs` `render_goal_mode_chip`): `accent@0.14` easing to 0.20 on
/// hover, label and 15 pt glyph in `readable_tone(accent)`.
fn search_chip(id: String, on: bool, icon_only: bool, t: &Pink) -> Stateful<Div> {
    if !on {
        return pink::trigger_chip(id, false, false, t)
            .flex_none()
            .aria_label("Search the web")
            .when(icon_only, |d| {
                d.w(px(ROW_BUTTON)).px(px(0.)).justify_center()
            })
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
            .when(!icon_only, |d| d.child("Search"));
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
        .map(|d| {
            if icon_only {
                d.w(px(ROW_BUTTON)).justify_center()
            } else {
                d.px(px(10.)).gap(px(6.))
            }
        })
        .flex()
        .items_center()
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
        .when(!icon_only, |d| d.child("Search"))
}

// ---------------------------------------------------------------------------
// Story: the four states of the primary controls (Pink UI rethink milestone
// 2, novnski/pink#174): idle, a live run with an empty draft, and a live
// run with a draft that queues or steers.
// ---------------------------------------------------------------------------

fn demo_files() -> Vec<Attachment> {
    vec![
        Attachment::image("launch-hero@2x.png", "PNG, 1.1 MB").preview(DEMO_HERO),
        Attachment::file("Q4-roadmap.pdf", "PDF, 2.4 MB"),
        Attachment::file("pricing-test.csv", "CSV, 88 KB"),
    ]
}

fn demo_pull_requests() -> Vec<PullRequest> {
    vec![
        PullRequest::new(
            "acme/storefront",
            101,
            "Retire the checkout copy test",
            PullRequestState::Draft,
        ),
        PullRequest::new(
            "acme/storefront",
            98,
            "Annual billing toggle",
            PullRequestState::Open,
        ),
        PullRequest::new(
            "acme/storefront",
            94,
            "Referral banner experiment",
            PullRequestState::Merged,
        ),
        PullRequest::new(
            "acme/billing",
            37,
            "Prorate seat upgrades",
            PullRequestState::Closed,
        ),
    ]
}

/// One state of the story: its caption over a live composer.
fn state_cell(caption: &'static str, composer: ChatComposer, t: &Pink) -> Div {
    div()
        .w(px(STATE_W))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .px(px(6.))
                .type_style(TextStyle::Footnote)
                .text_color(t.readable_muted(0.8))
                .child(caption),
        )
        .child(composer.width(STATE_W))
}

const STATE_W: f32 = 420.;
const DEMO_PROJECT: &str = "/Users/lena/storefront";
const DEMO_WORKTREE: &str = "/Users/lena/storefront-2";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        // Two checkouts on main, the project folder and a worktree: the menu
        // ticks the one the thread runs in.
        let branches = vec![
            BranchRef::new("main").current(DEMO_PROJECT),
            BranchRef::new("main").worktree(DEMO_WORKTREE),
            BranchRef::new("checkout-copy"),
            BranchRef::new("annual-billing").worktree("/Users/lena/storefront-3"),
        ];
        let base = |id: &'static str| {
            ChatComposer::new(id)
                .branches(branches.clone())
                .on_attach(|_, _| {})
                .on_stop(|_, _| {})
        };
        let thread = |id: &'static str| base(id).in_thread(true);
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(24.))
                    .child(state_cell(
                        "New thread: Send, checkout above the pill",
                        base("cc-idle")
                            .checkout(
                                Checkout::branch("main")
                                    .worktree("storefront-2")
                                    .path(DEMO_WORKTREE),
                            )
                            .attachments(demo_files()[..2].to_vec()),
                        &t,
                    ))
                    .child(state_cell(
                        "Running, empty draft: Stop alone",
                        thread("cc-stop")
                            .checkout(Checkout::branch("main").path(DEMO_PROJECT))
                            .pull_requests(demo_pull_requests()[1..2].to_vec())
                            .running(true),
                        &t,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(24.))
                    .child(state_cell(
                        "Running, draft: Stop and Queue for after this turn",
                        thread("cc-queue")
                            .checkout(Checkout::branch("main").machine("Studio Mac"))
                            .running(true)
                            .follow_up(FollowUp::Steer)
                            .steer_available(false)
                            .draft("Also check the refund rate for the annual cohort"),
                        &t,
                    ))
                    .child(state_cell(
                        "Running, draft: Stop and Steer this turn",
                        thread("cc-steer")
                            .checkout(
                                Checkout::branch("annual-billing")
                                    .worktree("storefront-3")
                                    .path("/Users/lena/storefront-3"),
                            )
                            .running(true)
                            .follow_up(FollowUp::Steer)
                            .steer_available(true)
                            .draft("Skip the EU cohort, its sample is too small"),
                        &t,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .px(px(6.))
                            .type_style(TextStyle::Footnote)
                            .text_color(t.readable_muted(0.8))
                            .child(
                                "In a thread: the branch keeps its name beside every pull request",
                            ),
                    )
                    .child(
                        thread("cc-prs")
                            .width(2. * STATE_W + 24.)
                            .checkout(
                                Checkout::branch("main")
                                    .worktree("storefront-2")
                                    .path(DEMO_WORKTREE),
                            )
                            .access(AccessMode::FullAccess)
                            .pull_requests(demo_pull_requests()[..3].to_vec()),
                    ),
            )
    })
}
