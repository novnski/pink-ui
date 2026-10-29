//! Git Actions – the Git card Pink opens at the top of its changes pane
//! (Pink: changes/git_actions.rs `render_git_card`).
//!
//! The card reads like Pink's: the branch on its remote with Refresh, the one next
//! step it recommends and why (git_actions/next_action.rs `resolve`), the
//! pull request's status with its badge (change_requests.rs
//! `pull_request_badge`), the PR panel – details and checks, or a draft PR
//! form (pull_requests.rs `render_pr_panel`) – file checkpoints
//! (`render_checkpoint_section`), the changed files with their pick boxes and
//! Stage / Unstage, the commit message, and Generate message, Commit, Push and
//! Commit & Push. The button that performs the recommended step wears the
//! accent plate (`git_step_button`); every other one stays secondary. While a
//! read or write runs the card says "Working…" and ignores its buttons; what
//! landed or failed shows as a notice line.
//!
//! The card never touches Git itself. Feed it the checkout's preview and PR
//! facts, and run what it asks for:
//!
//! ```ignore
//! GitActions::new("git")
//!     .preview(preview)
//!     .pr_status(PrStatus::NoMatch)
//!     .busy(running)
//!     .on_request(|request, _, _| match request {
//!         GitRequest::Commit { paths, message } => { /* git commit */ }
//!         _ => {}
//!     })
//! ```

use std::{collections::HashSet, rc::Rc, sync::Arc};
use web_time::Duration;

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, Hsla, IntoElement, Role,
    SharedString, Stateful, Subscription, Task, Window, div, prelude::*, px,
};

use crate::{
    HoverFill as _, motion,
    pink::{self, Pink, layout},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Git Actions";
pub const DESCRIPTION: &str = "The Git card from Pink's changes pane: branch and remote, the one next step it recommends with that button lit, the pull request's state, checks and draft form, file checkpoints, staged files to pick for a commit, and Commit and Push that say when they run, land or fail";

const ICON_PULL_REQUEST: &str = "icons/git-actions/git-pull-request.svg";

// Pink: changes/git_actions.rs `render_git_card`, `git_button_base`.
const CARD_MAX_HEIGHT: f32 = 530.0;
const CARD_GAP: f32 = 6.0;
const BUTTON_HEIGHT: f32 = 26.0;
const BUTTON_PAD_X: f32 = 10.0;
/// Pink `Theme::CONTROL_RADIUS`.
const CONTROL_RADIUS: f32 = 6.0;
const FILE_LIST_MAX_HEIGHT: f32 = 130.0;
const MESSAGE_HEIGHT: f32 = 30.0;
/// Pink's composer input type (`INPUT_TEXT_SIZE`, `INPUT_LINE_HEIGHT`).
const INPUT_TEXT_SIZE: f32 = 14.0;
const INPUT_LINE_HEIGHT: f32 = 22.75;
// Pink: pull_requests.rs `render_pr_panel`.
const PR_DESCRIPTION_MAX_HEIGHT: f32 = 120.0;
const PR_FIELD_RADIUS: f32 = 4.0;
const CHECKPOINT_ROWS: usize = 5;
const CHECKPOINT_DIFF_MAX_HEIGHT: f32 = 140.0;

// ---------------------------------------------------------------------------
// The checkout facts (Pink: pink_proto `GitActionPreview` and friends)
// ---------------------------------------------------------------------------

/// One changed file and which halves of it are staged (Pink `GitFileState`).
#[derive(Clone, Debug, PartialEq)]
pub struct GitFile {
    pub path: SharedString,
    pub staged: bool,
    pub unstaged: bool,
}

impl GitFile {
    /// A file with unstaged changes.
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            staged: false,
            unstaged: true,
        }
    }

    pub fn staged(mut self, staged: bool) -> Self {
        self.staged = staged;
        self
    }

    pub fn unstaged(mut self, unstaged: bool) -> Self {
        self.unstaged = unstaged;
        self
    }
}

/// Where the branch stands on its push remote (Pink `GitPublishState`).
#[derive(Clone, Debug, PartialEq)]
pub struct GitPublish {
    /// The remote branch it is compared with, e.g. `origin/feature`.
    pub tracking_ref: SharedString,
    pub published: bool,
    pub ahead: u32,
    pub behind: u32,
    /// The configured `@{upstream}`.
    pub upstream: Option<SharedString>,
    /// The remote's default branch, when known.
    pub default_branch: Option<SharedString>,
}

impl GitPublish {
    /// A published branch, level with `tracking_ref`.
    pub fn new(tracking_ref: impl Into<SharedString>) -> Self {
        Self {
            tracking_ref: tracking_ref.into(),
            published: true,
            ahead: 0,
            behind: 0,
            upstream: None,
            default_branch: None,
        }
    }

    pub fn published(mut self, published: bool) -> Self {
        self.published = published;
        self
    }

    pub fn ahead(mut self, ahead: u32) -> Self {
        self.ahead = ahead;
        self
    }

    pub fn behind(mut self, behind: u32) -> Self {
        self.behind = behind;
        self
    }

    pub fn upstream(mut self, upstream: impl Into<SharedString>) -> Self {
        self.upstream = Some(upstream.into());
        self
    }

    pub fn default_branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.default_branch = Some(branch.into());
        self
    }
}

/// The checkout the card acts on (Pink `GitActionPreview`).
#[derive(Clone, Debug, PartialEq)]
pub struct GitPreview {
    /// The branch, or `HEAD` when detached.
    pub branch: SharedString,
    /// The head commit, or `UNBORN` before the first commit.
    pub head: SharedString,
    pub remote: Option<SharedString>,
    pub files: Vec<GitFile>,
    pub publish: Option<GitPublish>,
}

impl GitPreview {
    pub fn new(branch: impl Into<SharedString>, head: impl Into<SharedString>) -> Self {
        Self {
            branch: branch.into(),
            head: head.into(),
            remote: None,
            files: Vec::new(),
            publish: None,
        }
    }

    pub fn remote(mut self, remote: impl Into<SharedString>) -> Self {
        self.remote = Some(remote.into());
        self
    }

    pub fn file(mut self, file: GitFile) -> Self {
        self.files.push(file);
        self
    }

    pub fn files(mut self, files: impl IntoIterator<Item = GitFile>) -> Self {
        self.files.extend(files);
        self
    }

    pub fn publish(mut self, publish: GitPublish) -> Self {
        self.publish = Some(publish);
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrState {
    Open,
    Merged,
    Closed,
}

impl PrState {
    fn label(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Merged => "Merged",
            Self::Closed => "Closed",
        }
    }

    /// Pink `ChangeRequestBadgeTone::color`.
    fn tone(self, t: &Pink) -> Hsla {
        match self {
            Self::Open => t.success,
            Self::Merged => t.code_text,
            Self::Closed => t.danger,
        }
    }
}

/// A pull request linked to the branch (Pink `ChangeRequestSummary`).
#[derive(Clone, Debug, PartialEq)]
pub struct PullRequest {
    pub number: u64,
    pub title: SharedString,
    pub state: PrState,
    pub url: SharedString,
    pub head_ref: SharedString,
    pub base_ref: SharedString,
}

impl PullRequest {
    /// An open pull request into `main`.
    pub fn new(number: u64, title: impl Into<SharedString>) -> Self {
        Self {
            number,
            title: title.into(),
            state: PrState::Open,
            url: SharedString::default(),
            head_ref: SharedString::default(),
            base_ref: "main".into(),
        }
    }

    pub fn state(mut self, state: PrState) -> Self {
        self.state = state;
        self
    }

    pub fn url(mut self, url: impl Into<SharedString>) -> Self {
        self.url = url.into();
        self
    }

    pub fn head_ref(mut self, head: impl Into<SharedString>) -> Self {
        self.head_ref = head.into();
        self
    }

    pub fn base_ref(mut self, base: impl Into<SharedString>) -> Self {
        self.base_ref = base.into();
        self
    }
}

/// What the PR lookup found (Pink `GitPrStatus`); the card shows "Checking
/// PR status…" while none is set.
#[derive(Clone, Debug, PartialEq)]
pub enum PrStatus {
    Linked(PullRequest),
    NoMatch,
    UnsupportedHost,
    Unavailable(SharedString),
}

/// One CI check on the PR; `status` is GitHub's word for it, such as
/// `SUCCESS`, `FAILURE` or `IN_PROGRESS` (Pink `GitPrCheck`).
#[derive(Clone, Debug, PartialEq)]
pub struct PrCheck {
    pub name: SharedString,
    pub status: SharedString,
    pub url: Option<SharedString>,
}

impl PrCheck {
    pub fn new(name: impl Into<SharedString>, status: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            status: status.into(),
            url: None,
        }
    }

    pub fn url(mut self, url: impl Into<SharedString>) -> Self {
        self.url = Some(url.into());
        self
    }
}

/// The linked PR as the panel shows it (Pink `GitPrDetails`).
#[derive(Clone, Debug, PartialEq)]
pub struct PrDetails {
    pub summary: PullRequest,
    pub is_draft: bool,
    pub body: SharedString,
    pub checks: Vec<PrCheck>,
}

impl PrDetails {
    pub fn new(summary: PullRequest) -> Self {
        Self {
            summary,
            is_draft: false,
            body: SharedString::default(),
            checks: Vec::new(),
        }
    }

    pub fn draft(mut self, is_draft: bool) -> Self {
        self.is_draft = is_draft;
        self
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = body.into();
        self
    }

    pub fn check(mut self, check: PrCheck) -> Self {
        self.checks.push(check);
        self
    }
}

/// What a new draft PR would be, prefilled for the form (Pink
/// `GitPrDraftPreview`).
#[derive(Clone, Debug, PartialEq)]
pub struct DraftPr {
    pub repository: SharedString,
    pub cwd: SharedString,
    pub head_commit: SharedString,
    pub title: SharedString,
    pub body: SharedString,
    pub base: SharedString,
    pub head: SharedString,
    /// A create may already have landed: the form only checks GitHub
    /// (Pink `reconcile_only`).
    pub reconcile_only: bool,
}

impl DraftPr {
    pub fn new(repository: impl Into<SharedString>, head: impl Into<SharedString>) -> Self {
        Self {
            repository: repository.into(),
            cwd: SharedString::default(),
            head_commit: SharedString::default(),
            title: SharedString::default(),
            body: SharedString::default(),
            base: "main".into(),
            head: head.into(),
            reconcile_only: false,
        }
    }

    pub fn checkout(
        mut self,
        cwd: impl Into<SharedString>,
        head_commit: impl Into<SharedString>,
    ) -> Self {
        self.cwd = cwd.into();
        self.head_commit = head_commit.into();
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = body.into();
        self
    }

    pub fn base(mut self, base: impl Into<SharedString>) -> Self {
        self.base = base.into();
        self
    }

    pub fn reconcile_only(mut self, reconcile_only: bool) -> Self {
        self.reconcile_only = reconcile_only;
        self
    }
}

/// What the PR panel under the status row shows.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum PrPanel {
    /// Folded to its buttons.
    #[default]
    Closed,
    /// Open while its read is under way, or after it failed.
    Empty,
    Details(PrDetails),
    Draft(DraftPr),
}

/// One completed turn's file snapshot (Pink `GitCheckpoint`).
#[derive(Clone, Debug, PartialEq)]
pub struct Checkpoint {
    pub turn_id: SharedString,
    /// When the turn started, as shown: `Oct 24 14:02`.
    pub started: SharedString,
    /// How the turn ended; `None` while it runs.
    pub outcome: Option<SharedString>,
    pub before_available: bool,
    pub after_available: bool,
    pub error: Option<SharedString>,
}

impl Checkpoint {
    pub fn new(turn_id: impl Into<SharedString>, started: impl Into<SharedString>) -> Self {
        Self {
            turn_id: turn_id.into(),
            started: started.into(),
            outcome: None,
            before_available: true,
            after_available: true,
            error: None,
        }
    }

    pub fn outcome(mut self, outcome: impl Into<SharedString>) -> Self {
        self.outcome = Some(outcome.into());
        self
    }

    pub fn snapshots(mut self, before: bool, after: bool) -> Self {
        self.before_available = before;
        self.after_available = after;
        self
    }

    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }
}

/// The checkout's turn checkpoints (Pink `GitCheckpointList`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Checkpoints {
    pub list: Vec<Checkpoint>,
    pub restore_available: bool,
    pub restore_unavailable_reason: Option<SharedString>,
}

impl Checkpoints {
    pub fn new(list: impl IntoIterator<Item = Checkpoint>) -> Self {
        Self {
            list: list.into_iter().collect(),
            restore_available: true,
            restore_unavailable_reason: None,
        }
    }

    /// Restoring is off, with the reason shown.
    pub fn restore_unavailable(mut self, reason: impl Into<SharedString>) -> Self {
        self.restore_available = false;
        self.restore_unavailable_reason = Some(reason.into());
        self
    }
}

/// A turn's observed changes under its checkpoint row.
#[derive(Clone, Debug, PartialEq)]
pub struct CheckpointDiff {
    pub paths: usize,
    pub patch: SharedString,
}

/// Everything the card asks its host to do.
#[derive(Clone, Debug, PartialEq)]
pub enum GitRequest {
    /// Read the checkout again.
    Refresh,
    Stage(Vec<SharedString>),
    Unstage(Vec<SharedString>),
    /// Commit the staged version of `paths`.
    Commit {
        paths: Vec<SharedString>,
        message: SharedString,
    },
    Push,
    CommitAndPush {
        paths: Vec<SharedString>,
        message: SharedString,
    },
    /// Draft a message for `paths`; hand it back through
    /// [`GitActions::message`].
    GenerateMessage {
        paths: Vec<SharedString>,
    },
    /// Open the panel on the linked PR's details.
    PrDetails(u64),
    /// Open the panel on a new draft PR.
    NewDraftPr,
    ClosePrPanel,
    CreateDraftPr {
        title: SharedString,
        body: SharedString,
        base: SharedString,
        head: SharedString,
    },
    RefreshDraftPreview,
    RefreshChecks(u64),
    OpenUrl(SharedString),
    CheckpointDiff(SharedString),
    PreviewRestore(SharedString),
    RestoreFiles,
}

impl GitRequest {
    /// Reads and writes of the checkout, which wait while one runs (Pink
    /// `git_action`'s busy guard).
    fn waits_on_git(&self) -> bool {
        matches!(
            self,
            Self::Refresh
                | Self::Stage(_)
                | Self::Unstage(_)
                | Self::Commit { .. }
                | Self::Push
                | Self::CommitAndPush { .. }
                | Self::GenerateMessage { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// The next step (Pink: git_actions/next_action.rs)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NextStep {
    AddFiles,
    StageChanges,
    SelectStaged,
    Commit,
    SwitchBranch,
    AddPushRemote,
    PublishUnknown,
    Push,
    UpdateBranch,
    UpToDate,
    CheckingPr,
    PrUnavailable,
    SetUpstream,
    NewDraftPr,
    ViewPr,
    FixChecks,
    WaitForChecks,
    ReviewOnGitHub,
    Merged,
    Closed,
}

impl NextStep {
    fn label(self) -> &'static str {
        match self {
            Self::AddFiles => "Add files",
            Self::StageChanges => "Stage changes",
            Self::SelectStaged => "Select staged files",
            Self::Commit => "Commit",
            Self::SwitchBranch => "Switch to a branch",
            Self::AddPushRemote => "Add a push remote",
            Self::PublishUnknown => "Push state unknown",
            Self::Push => "Push",
            Self::UpdateBranch => "Update the branch",
            Self::UpToDate => "Up to date",
            Self::CheckingPr => "Checking PR",
            Self::PrUnavailable => "PR status unavailable",
            Self::SetUpstream => "Set the upstream",
            Self::NewDraftPr => "New draft PR",
            Self::ViewPr => "Check the PR",
            Self::FixChecks => "Fix failing checks",
            Self::WaitForChecks => "Checks running",
            Self::ReviewOnGitHub => "Open on GitHub",
            Self::Merged => "Merged",
            Self::Closed => "PR closed",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct NextAction {
    step: NextStep,
    reason: String,
    /// The one file a `StageChanges` step names; its Stage button performs it.
    file: Option<SharedString>,
}

impl NextAction {
    /// Whether the card button with this (Pink) id performs the step and so
    /// carries the accent plate. Steps that need work outside the card, or
    /// with nothing left to do, light none; Commit & Push never: pushing
    /// stays an explicit choice (Pink `NextGitAction::accents`).
    fn accents(&self, button: &str) -> bool {
        match self.step {
            NextStep::StageChanges => self
                .file
                .as_deref()
                .is_some_and(|file| button.strip_prefix("git-stage-") == Some(file)),
            NextStep::Commit => button == "git-commit",
            NextStep::Push => button == "git-push",
            NextStep::PrUnavailable => button == "git-refresh",
            NextStep::NewDraftPr => button == "pr-draft-open",
            NextStep::ViewPr => matches!(button, "pr-details-open" | "pr-preview-retry"),
            NextStep::WaitForChecks => button == "pr-checks-refresh",
            NextStep::ReviewOnGitHub => button == "pr-open-github",
            _ => false,
        }
    }
}

fn next(step: NextStep, reason: impl Into<String>) -> NextAction {
    NextAction {
        step,
        reason: reason.into(),
        file: None,
    }
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// How a reported check reads (Pink `check_outcome`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CheckOutcome {
    Passed,
    Failed,
    Pending,
    Unknown,
}

fn check_outcome(status: &str) -> CheckOutcome {
    match status {
        "SUCCESS" | "NEUTRAL" | "SKIPPED" => CheckOutcome::Passed,
        "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE" => {
            CheckOutcome::Failed
        }
        "PENDING" | "QUEUED" | "IN_PROGRESS" | "WAITING" | "REQUESTED" | "EXPECTED" => {
            CheckOutcome::Pending
        }
        _ => CheckOutcome::Unknown,
    }
}

/// Local work first, then branch and remote prerequisites, then a merged or
/// closed PR, then the pushed branch, then the forge (Pink `resolve`).
fn resolve(
    preview: &GitPreview,
    selected: &HashSet<SharedString>,
    message_empty: bool,
    pr: Option<&PrStatus>,
    details: Option<&PrDetails>,
) -> NextAction {
    if let Some(local) = local_work(preview, selected, message_empty) {
        return local;
    }
    let branch = preview.branch.as_ref();
    if preview.head.as_ref() == "UNBORN" {
        return next(
            NextStep::AddFiles,
            "This repository has no commits yet; add files to make the first commit",
        );
    }
    if branch == "HEAD" {
        return next(
            NextStep::SwitchBranch,
            "HEAD is detached; switch to a branch before pushing or opening a PR",
        );
    }
    let Some(remote) = preview.remote.as_deref() else {
        return next(
            NextStep::AddPushRemote,
            format!("{branch} has no push remote; add one to publish it"),
        );
    };
    let linked = match pr {
        Some(PrStatus::Linked(pr)) => Some(pr),
        _ => None,
    };
    let details = linked.and_then(|pr| details.filter(|d| d.summary.number == pr.number));
    if let Some(pr) = linked
        && let Some(finished) = finished_pr(pr, details, branch, remote, preview.publish.as_ref())
    {
        return finished;
    }
    let Some(publish) = preview.publish.as_ref() else {
        return next(
            NextStep::PublishUnknown,
            format!("Whether {branch} is on {remote} is not known from this checkout's refs"),
        );
    };
    if !publish.published {
        return next(
            NextStep::Push,
            format!("{branch} is not on {remote} as of the last fetch or push"),
        );
    }
    let tracking = publish.tracking_ref.as_ref();
    match (publish.ahead, publish.behind) {
        (0, 0) => {}
        (ahead, 0) => {
            let commits = count(ahead as usize, "commit is", "commits are");
            return next(
                NextStep::Push,
                match linked {
                    Some(pr) => {
                        format!("{commits} not on {tracking}; push to update #{}", pr.number)
                    }
                    None => format!("{commits} not on {tracking} yet"),
                },
            );
        }
        (0, behind) => {
            return next(
                NextStep::UpdateBranch,
                format!(
                    "{tracking} has {} this checkout lacks; pull in a terminal, then refresh",
                    count(behind as usize, "commit", "commits")
                ),
            );
        }
        (ahead, behind) => {
            return next(
                NextStep::UpdateBranch,
                format!(
                    "{branch} and {tracking} have diverged ({ahead} ahead, {behind} behind); rebase or merge in a terminal before pushing"
                ),
            );
        }
    }
    let on_default_branch = match publish.default_branch.as_deref() {
        Some(default) => default == branch,
        None => matches!(branch, "main" | "master"),
    };
    match pr {
        None => next(
            NextStep::CheckingPr,
            format!("{branch} matches {tracking}; checking for a pull request"),
        ),
        Some(PrStatus::UnsupportedHost) => next(
            NextStep::UpToDate,
            format!("{branch} matches {tracking}; PR status is not supported for this remote"),
        ),
        Some(PrStatus::Unavailable(_)) => next(
            NextStep::PrUnavailable,
            format!("{branch} matches {tracking}; refresh to check for a pull request again"),
        ),
        Some(PrStatus::NoMatch) if on_default_branch => {
            next(NextStep::UpToDate, format!("{branch} matches {tracking}"))
        }
        Some(PrStatus::NoMatch) => draft_pr(branch, remote, publish),
        Some(PrStatus::Linked(pr)) => open_pr(pr, details),
    }
}

/// The step for uncommitted work, which always comes before publishing
/// (Pink `local_work`).
fn local_work(
    preview: &GitPreview,
    selected: &HashSet<SharedString>,
    message_empty: bool,
) -> Option<NextAction> {
    let files = &preview.files;
    if files.is_empty() {
        return None;
    }
    let picked: Vec<&GitFile> = files
        .iter()
        .filter(|file| file.staged && selected.contains(&file.path))
        .collect();
    // Only the staged version of each picked file is committed.
    if let Some(changed) = picked.iter().find(|file| file.unstaged) {
        return Some(NextAction {
            file: Some(changed.path.clone()),
            ..next(
                NextStep::StageChanges,
                format!(
                    "{} changed after staging; stage its current version before committing",
                    changed.path
                ),
            )
        });
    }
    if !picked.is_empty() {
        let what = count(picked.len(), "selected file", "selected files");
        return Some(next(
            NextStep::Commit,
            if message_empty {
                format!("Write or generate a message, then commit {what}")
            } else {
                format!("Commit {what}")
            },
        ));
    }
    let staged = files.iter().filter(|file| file.staged).count();
    if staged > 0 {
        return Some(next(
            NextStep::SelectStaged,
            format!(
                "{} staged; select what to commit",
                count(staged, "file is", "files are")
            ),
        ));
    }
    // Nothing is staged, so every file has unstaged changes. With one, its
    // Stage button is the step; with several there is no Stage all.
    let only = match files.as_slice() {
        [file] => Some(file.path.clone()),
        _ => None,
    };
    Some(NextAction {
        file: only,
        ..next(
            NextStep::StageChanges,
            format!(
                "{} not staged yet",
                count(files.len(), "changed file is", "changed files are")
            ),
        )
    })
}

/// A merged or closed PR ends the walk, whatever the pushed branch says
/// (Pink `finished_pr`).
fn finished_pr(
    pr: &PullRequest,
    details: Option<&PrDetails>,
    branch: &str,
    remote: &str,
    publish: Option<&GitPublish>,
) -> Option<NextAction> {
    let summary = details.map_or(pr, |details| &details.summary);
    let number = summary.number;
    let (step, done) = match summary.state {
        PrState::Open => return None,
        PrState::Merged => (
            NextStep::Merged,
            format!("#{number} was merged into {}", summary.base_ref),
        ),
        PrState::Closed => (
            NextStep::Closed,
            format!("#{number} was closed without merging"),
        ),
    };
    let gone = publish.is_some_and(|publish| !publish.published);
    let newer = publish
        .filter(|publish| publish.published)
        .map_or(0, |publish| publish.ahead as usize);
    Some(next(
        step,
        if gone {
            format!(
                "{done}, and {branch} is not on {remote} as of the last fetch; start new work on a new branch"
            )
        } else if newer > 0 {
            format!(
                "{done}; {} not in it. Move them to a new branch for another PR",
                count(newer, "newer commit is", "newer commits are")
            )
        } else if step == NextStep::Closed {
            format!("{done}; reopen it on GitHub to continue it")
        } else {
            done
        },
    ))
}

/// A draft PR needs the branch to track `<remote>/<branch>` (Pink
/// `draft_pr`).
fn draft_pr(branch: &str, remote: &str, publish: &GitPublish) -> NextAction {
    let expected = format!("{remote}/{branch}");
    match publish.upstream.as_deref() {
        Some(upstream) if upstream == expected.as_str() => next(
            NextStep::NewDraftPr,
            format!("{branch} is pushed to {remote} and has no pull request yet"),
        ),
        Some(other) => next(
            NextStep::SetUpstream,
            format!(
                "{branch} tracks {other}; a draft PR needs it to track {expected} (git branch -u {expected})"
            ),
        ),
        None => next(
            NextStep::SetUpstream,
            format!(
                "{branch} is pushed to {remote} without an upstream; a draft PR needs git branch -u {expected}"
            ),
        ),
    }
}

/// An open PR: its checks first, then review (Pink `open_pr`).
fn open_pr(pr: &PullRequest, details: Option<&PrDetails>) -> NextAction {
    let number = pr.number;
    let Some(details) = details else {
        return next(
            NextStep::ViewPr,
            format!("#{number} is open; check its CI before review"),
        );
    };
    let total = details.checks.len();
    let mut failed = Vec::new();
    let (mut pending, mut unknown) = (0_usize, 0_usize);
    for check in &details.checks {
        match check_outcome(&check.status) {
            CheckOutcome::Failed => failed.push(check.name.as_ref()),
            CheckOutcome::Pending => pending += 1,
            CheckOutcome::Unknown => unknown += 1,
            CheckOutcome::Passed => {}
        }
    }
    if !failed.is_empty() {
        let mut names = failed
            .iter()
            .take(2)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        if failed.len() > 2 {
            names.push_str(", …");
        }
        return next(
            NextStep::FixChecks,
            format!(
                "{} of {total} {} failed on #{number}: {names}",
                failed.len(),
                if total == 1 { "check" } else { "checks" }
            ),
        );
    }
    if pending > 0 {
        return next(
            NextStep::WaitForChecks,
            format!(
                "{pending} of {total} {} still running on #{number}",
                if total == 1 { "check is" } else { "checks are" }
            ),
        );
    }
    let checks = if total == 0 {
        "No checks reported".to_string()
    } else if unknown > 0 {
        format!(
            "{} not recognized",
            count(unknown, "check state is", "check states are")
        )
    } else {
        "Checks passed".to_string()
    };
    next(
        NextStep::ReviewOnGitHub,
        if details.is_draft {
            format!("{checks}; #{number} is a draft. Mark it ready for review on GitHub")
        } else {
            format!("{checks}; review and merge #{number} on GitHub")
        },
    )
}

/// A key for the checkout a preview describes, so a new read resets the
/// picks (Pink: the selection follows every preview it receives).
fn preview_key(preview: &GitPreview) -> u64 {
    let mut bytes = Vec::new();
    for part in [&preview.branch, &preview.head] {
        bytes.extend_from_slice(part.as_bytes());
        bytes.push(0);
    }
    for file in &preview.files {
        bytes.extend_from_slice(file.path.as_bytes());
        bytes.push(u8::from(file.staged) | u8::from(file.unstaged) << 1);
    }
    pink::loaders::fnv1a(&bytes)
}

fn draft_key(draft: &DraftPr) -> u64 {
    let mut bytes = Vec::new();
    for part in [
        &draft.repository,
        &draft.head_commit,
        &draft.title,
        &draft.base,
        &draft.head,
    ] {
        bytes.extend_from_slice(part.as_bytes());
        bytes.push(0);
    }
    pink::loaders::fnv1a(&bytes)
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

type RequestHandler = Rc<dyn Fn(&GitRequest, &mut Window, &mut App)>;
type MessageHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct GitActions {
    id: ElementId,
    checkout: bool,
    preview: Option<GitPreview>,
    pr: Option<PrStatus>,
    panel: PrPanel,
    pr_busy: bool,
    pr_notice: Option<SharedString>,
    checkpoints: Option<Checkpoints>,
    checkpoint_diff: Option<CheckpointDiff>,
    restore: Option<Vec<SharedString>>,
    checkpoint_notice: Option<SharedString>,
    checkpoint_receipt: Option<SharedString>,
    busy: bool,
    notice: Option<SharedString>,
    message: Option<SharedString>,
    width: f32,
    on_request: Option<RequestHandler>,
    on_message: Option<MessageHandler>,
}

impl GitActions {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            checkout: true,
            preview: None,
            pr: None,
            panel: PrPanel::Closed,
            pr_busy: false,
            pr_notice: None,
            checkpoints: None,
            checkpoint_diff: None,
            restore: None,
            checkpoint_notice: None,
            checkpoint_receipt: None,
            busy: false,
            notice: None,
            message: None,
            width: 560.,
            on_request: None,
            on_message: None,
        }
    }

    /// Whether a Git checkout is selected at all (default true).
    pub fn checkout(mut self, checkout: bool) -> Self {
        self.checkout = checkout;
        self
    }

    /// The checkout the card acts on; without one it says it is loading or
    /// unavailable.
    pub fn preview(mut self, preview: impl Into<Option<GitPreview>>) -> Self {
        self.preview = preview.into();
        self
    }

    /// What the PR lookup found; leave unset while it runs.
    pub fn pr_status(mut self, status: impl Into<Option<PrStatus>>) -> Self {
        self.pr = status.into();
        self
    }

    pub fn pr_panel(mut self, panel: PrPanel) -> Self {
        self.panel = panel;
        self
    }

    /// A GitHub read or the draft create is under way.
    pub fn pr_busy(mut self, busy: bool) -> Self {
        self.pr_busy = busy;
        self
    }

    pub fn pr_notice(mut self, notice: impl Into<SharedString>) -> Self {
        self.pr_notice = Some(notice.into());
        self
    }

    /// The turn checkpoints; leave unset while they load.
    pub fn checkpoints(mut self, checkpoints: Checkpoints) -> Self {
        self.checkpoints = Some(checkpoints);
        self
    }

    pub fn checkpoint_diff(mut self, diff: CheckpointDiff) -> Self {
        self.checkpoint_diff = Some(diff);
        self
    }

    /// The paths a restore would change, shown with Restore files.
    pub fn restore_preview<S: Into<SharedString>>(
        mut self,
        paths: impl IntoIterator<Item = S>,
    ) -> Self {
        self.restore = Some(paths.into_iter().map(Into::into).collect());
        self
    }

    pub fn checkpoint_notice(mut self, notice: impl Into<SharedString>) -> Self {
        self.checkpoint_notice = Some(notice.into());
        self
    }

    pub fn checkpoint_receipt(mut self, receipt: impl Into<SharedString>) -> Self {
        self.checkpoint_receipt = Some(receipt.into());
        self
    }

    /// A Git read or write is running: the card says "Working…" and waits.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// The last result: what landed ("Committed 3f9c2ab", "Push completed")
    /// or what failed.
    pub fn notice(mut self, notice: impl Into<Option<SharedString>>) -> Self {
        self.notice = notice.into();
        self
    }

    /// The commit message. The field takes it whenever it changes here – a
    /// generated draft, or empty after a commit – and reports typing through
    /// [`on_message`](Self::on_message).
    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Card width in px (default 560).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Called with everything the card asks for.
    pub fn on_request(
        mut self,
        handler: impl Fn(&GitRequest, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_request = Some(Rc::new(handler));
        self
    }

    /// Called as the commit message is typed.
    pub fn on_message(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_message = Some(Rc::new(handler));
        self
    }
}

/// The draft form's four fields (Pink `PullRequestPanel` inputs).
struct DraftFields {
    title: Entity<TextInput>,
    body: Entity<TextInput>,
    base: Entity<TextInput>,
    head: Entity<TextInput>,
}

struct CardState {
    /// The staged files picked for the commit (Pink `git_selected`).
    selected: HashSet<SharedString>,
    preview_key: Option<u64>,
    message: Entity<TextInput>,
    _message_events: Subscription,
    /// The last message the host handed over.
    host_message: Option<SharedString>,
    /// The step last shown, kept while a re-read only re-checks the PR
    /// (Pink `ShownGitStep`).
    shown: ShownStep,
    draft_key: Option<u64>,
    draft: Option<DraftFields>,
    on_message: Option<MessageHandler>,
}

impl CardState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let message = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Commit message…")
                .multiline()
        });
        let events = cx.subscribe_in(&message, window, |s: &mut Self, _, event, window, cx| {
            if let InputEvent::Change(text) = event {
                if let Some(f) = s.on_message.clone() {
                    let text = text.clone();
                    window.defer(cx, move |window, cx| f(&text, window, cx));
                }
                cx.notify();
            }
        });
        Self {
            selected: HashSet::new(),
            preview_key: None,
            message,
            _message_events: events,
            host_message: None,
            shown: None,
            draft_key: None,
            draft: None,
            on_message: None,
        }
    }

    /// Every re-read re-checks the PR; for an unchanged checkout the card
    /// keeps the step it showed instead of flashing "Checking PR" (Pink
    /// `shown_git_action`).
    fn shown_action(
        &mut self,
        fresh: NextAction,
        key: u64,
        publish: Option<&GitPublish>,
    ) -> NextAction {
        keep_shown(&mut self.shown, fresh, key, publish)
    }
}

/// The step last shown, for a preview `key` and its pushed-branch state.
type ShownStep = Option<(u64, Option<GitPublish>, NextAction)>;

/// Keeps the shown step while a re-read only re-checks the PR of an
/// unchanged checkout (Pink `shown_git_action`).
fn keep_shown(
    shown: &mut ShownStep,
    fresh: NextAction,
    key: u64,
    publish: Option<&GitPublish>,
) -> NextAction {
    if fresh.step == NextStep::CheckingPr
        && let Some((shown_key, shown_publish, step)) = shown.as_ref()
        && *shown_key == key
        && shown_publish.as_ref() == publish
    {
        return step.clone();
    }
    *shown = Some((key, publish.cloned(), fresh.clone()));
    fresh
}

fn field(
    window: &mut Window,
    cx: &mut Context<CardState>,
    placeholder: &'static str,
    text: &SharedString,
    multiline: bool,
) -> Entity<TextInput> {
    let text = text.clone();
    cx.new(|cx| {
        let input = TextInput::new(window, cx)
            .placeholder(placeholder)
            .default_text(text);
        if multiline { input.multiline() } else { input }
    })
}

/// What each click hands the host.
#[derive(Clone)]
struct Sender {
    on_request: Option<RequestHandler>,
    busy: bool,
    pr_busy: bool,
}

impl Sender {
    fn send(&self, request: &GitRequest, window: &mut Window, cx: &mut App) {
        if self.busy && request.waits_on_git() {
            return;
        }
        if self.pr_busy
            && matches!(
                request,
                GitRequest::CreateDraftPr { .. }
                    | GitRequest::RefreshChecks(_)
                    | GitRequest::RefreshDraftPreview
            )
        {
            return;
        }
        if let Some(f) = &self.on_request {
            f(request, window, cx);
        }
    }

    fn on(
        &self,
        request: GitRequest,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
        let sender = self.clone();
        move |_, window, cx| sender.send(&request, window, cx)
    }
}

impl RenderOnce for GitActions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            CardState::new,
        );

        // Mirror the host's facts into the card state.
        let checkout_key = self.preview.as_ref().map(preview_key);
        state.update(cx, |s, cx| {
            s.on_message = self.on_message.clone();
            if checkout_key != s.preview_key {
                s.preview_key = checkout_key;
                s.selected = self
                    .preview
                    .iter()
                    .flat_map(|p| &p.files)
                    .filter(|f| f.staged)
                    .map(|f| f.path.clone())
                    .collect();
            }
            if let Some(message) = &self.message
                && s.host_message.as_ref() != Some(message)
            {
                s.host_message = Some(message.clone());
                if s.message.read(cx).text() != *message {
                    let message = message.clone();
                    s.message
                        .update(cx, |input, cx| input.set_text(message, cx));
                }
            }
            match &self.panel {
                PrPanel::Draft(draft) => {
                    let key = draft_key(draft);
                    if s.draft_key != Some(key) {
                        s.draft_key = Some(key);
                        s.draft = Some(DraftFields {
                            title: field(window, cx, "Pull request title…", &draft.title, false),
                            body: field(window, cx, "Describe your changes…", &draft.body, true),
                            base: field(window, cx, "Base branch", &draft.base, false),
                            head: field(window, cx, "Head branch", &draft.head, false),
                        });
                    }
                }
                _ => {
                    s.draft_key = None;
                    s.draft = None;
                }
            }
        });

        let (message_input, draft_inputs, selected) = {
            let s = state.read(cx);
            (
                s.message.clone(),
                s.draft.as_ref().map(|d| {
                    [
                        d.title.clone(),
                        d.body.clone(),
                        d.base.clone(),
                        d.head.clone(),
                    ]
                }),
                s.selected.clone(),
            )
        };
        for input in std::iter::once(&message_input).chain(draft_inputs.iter().flatten()) {
            input.update(cx, |input, _| {
                input.set_colors(Some(t.text_faint), Some(t.caret), Some(t.selection))
            });
        }
        let message_empty = message_input.read(cx).text().trim().is_empty();

        let details = match &self.panel {
            PrPanel::Details(details) => Some(details),
            _ => None,
        };
        let next = self.preview.as_ref().map(|preview| {
            let fresh = resolve(preview, &selected, message_empty, self.pr.as_ref(), details);
            let key = preview_key(preview);
            state.update(cx, |s, _| {
                s.shown_action(fresh, key, preview.publish.as_ref())
            })
        });

        let sender = Sender {
            on_request: self.on_request.clone(),
            busy: self.busy,
            pr_busy: self.pr_busy,
        };
        let c = Card {
            key: &key,
            t: &t,
            next: next.as_ref(),
            sender: &sender,
        };

        // Pink `render_git_card`.
        let mut card = div()
            .id(self.id.clone())
            .flex_none()
            .w(px(self.width))
            .max_h(px(CARD_MAX_HEIGHT))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .px(px(pink::space::S12))
            .py(px(pink::space::S8))
            .border_b_1()
            .border_color(t.border)
            .bg(t.card_glass_bg())
            .font_family(pink::SANS)
            .text_color(t.text);
        if let Some(preview) = &self.preview {
            let remote = preview
                .remote
                .clone()
                .unwrap_or_else(|| "No push remote".into());
            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(11.))
                    .text_color(t.text_dim)
                    .child(format!("Branch {} on {remote}", preview.branch))
                    .child(
                        c.step_button("git-refresh", "Refresh")
                            .on_click(sender.on(GitRequest::Refresh)),
                    ),
            );
            if let Some(next) = &next {
                card = card.child(next_step_line(&key, next, &t));
            }
            card = card
                .child(pr_status_row(&c, self.pr.as_ref()))
                .child(pr_panel(
                    &c,
                    &self.panel,
                    self.pr.as_ref(),
                    self.pr_busy,
                    self.pr_notice.clone(),
                    draft_inputs.clone(),
                ))
                .child(checkpoint_section(
                    &c,
                    self.checkpoints.as_ref(),
                    self.checkpoint_diff.as_ref(),
                    self.restore.as_deref(),
                    self.checkpoint_notice.clone(),
                    self.checkpoint_receipt.clone(),
                ));
            let rows = preview
                .files
                .iter()
                .map(|file| file_row(&c, &state, file, selected.contains(&file.path)));
            card = card.child(
                div()
                    .id(SharedString::from(format!("{key}-file-list")))
                    .max_h(px(FILE_LIST_MAX_HEIGHT))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .children(rows),
            );
            // The commit row reads the picks and the message at click time.
            let commit = |with_push: bool| {
                let state = state.clone();
                let sender = sender.clone();
                let files: Vec<SharedString> =
                    preview.files.iter().map(|f| f.path.clone()).collect();
                move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                    let (paths, message) = {
                        let s = state.read(cx);
                        let paths = files
                            .iter()
                            .filter(|p| s.selected.contains(*p))
                            .cloned()
                            .collect();
                        (paths, s.message.read(cx).text())
                    };
                    let request = if with_push {
                        GitRequest::CommitAndPush { paths, message }
                    } else {
                        GitRequest::Commit { paths, message }
                    };
                    sender.send(&request, window, cx);
                }
            };
            let generate = {
                let state = state.clone();
                let sender = sender.clone();
                move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                    let paths = state.read(cx).selected.iter().cloned().collect();
                    sender.send(&GitRequest::GenerateMessage { paths }, window, cx);
                }
            };
            card = card
                .child(
                    div()
                        .h(px(MESSAGE_HEIGHT))
                        .flex_none()
                        .overflow_hidden()
                        .text_size(px(INPUT_TEXT_SIZE))
                        .line_height(px(INPUT_LINE_HEIGHT))
                        .text_color(t.text)
                        .child(message_input.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            git_button(&key, "git-generate", "Generate message", &t)
                                .on_click(generate),
                        )
                        .child(
                            c.step_button("git-commit", "Commit")
                                .on_click(commit(false)),
                        )
                        .child(
                            c.step_button("git-push", "Push")
                                .on_click(sender.on(GitRequest::Push)),
                        )
                        .child(
                            git_button(&key, "git-commit-push", "Commit & Push", &t)
                                .on_click(commit(true)),
                        ),
                );
        } else {
            card = card.child(if !self.checkout {
                "Select a task with a Git checkout"
            } else if self.busy {
                "Loading checkout status…"
            } else {
                "Git status unavailable; refresh to retry"
            });
            if self.checkout {
                card = card.child(
                    git_button(&key, "git-refresh", "Refresh", &t)
                        .on_click(sender.on(GitRequest::Refresh)),
                );
            }
        }
        if let Some(notice) = self.notice.clone() {
            card = card.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_dim)
                    .child(notice),
            );
        }
        if self.busy {
            card = card.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_faint)
                    .child("Working…"),
            );
        }
        card
    }
}

/// What the card's pieces need.
struct Card<'a> {
    key: &'a str,
    t: &'a Pink,
    next: Option<&'a NextAction>,
    sender: &'a Sender,
}

impl Card<'_> {
    /// A button that wears the accent plate exactly when it performs the
    /// shown step (Pink `git_step_button`).
    fn step_button(&self, id: &str, label: impl Into<SharedString>) -> Stateful<gpui::Div> {
        if self.next.is_some_and(|next| next.accents(id)) {
            git_primary_button(self.key, id, label, self.t)
        } else {
            git_button(self.key, id, label, self.t)
        }
    }
}

/// The shared button body (Pink `git_button_base`): 26 tall, `px 10`, r6, a
/// 1 px `border`, 12 in `text`, a tab stop with the inset focus ring.
fn git_button_base(
    key: &str,
    id: &str,
    label: impl Into<SharedString>,
    t: &Pink,
) -> Stateful<gpui::Div> {
    let label = label.into();
    let ring = t.focus_ring();
    div()
        .id(SharedString::from(format!("{key}-{id}")))
        .role(Role::Button)
        .aria_label(label.clone())
        .tab_index(0)
        .flex_none()
        .h(px(BUTTON_HEIGHT))
        .px(px(BUTTON_PAD_X))
        .flex()
        .items_center()
        .rounded(px(CONTROL_RADIUS))
        .border_1()
        .border_color(t.border)
        .text_size(px(12.))
        .text_color(t.text)
        .cursor_pointer()
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .child(label)
}

/// A secondary card button on the raised plate (Pink `git_button`).
fn git_button(
    key: &str,
    id: &str,
    label: impl Into<SharedString>,
    t: &Pink,
) -> Stateful<gpui::Div> {
    git_button_base(key, id, label, t).hover_fill(
        format!("{key}-{id}-fill"),
        t.surface_raised,
        t.surface_raised_hover,
    )
}

/// The card's default action (Pink `git_primary_button`, audit SS-13): an
/// accent plate, medium label in `on_accent`, easing to 88%.
fn git_primary_button(
    key: &str,
    id: &str,
    label: impl Into<SharedString>,
    t: &Pink,
) -> Stateful<gpui::Div> {
    let fill = t.accent_strong;
    git_button_base(key, id, label, t)
        .border_color(gpui::transparent_black())
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.on_accent)
        .hover_fill(format!("{key}-{id}-fill"), fill, fill.opacity(0.88))
}

/// "Commit · Commit 2 selected files" (Pink `render_next_git_action`): the
/// step in medium `text`, its reason in `text_dim`, 11 on a shared baseline.
fn next_step_line(key: &str, next: &NextAction, t: &Pink) -> impl IntoElement {
    let label = next.step.label();
    div()
        .id(SharedString::from(format!("{key}-git-next-action")))
        .role(Role::Group)
        .aria_label(SharedString::from(format!(
            "Next Git step: {label}. {}",
            next.reason
        )))
        .flex()
        .items_baseline()
        .gap(px(6.))
        .min_w_0()
        .text_size(px(11.))
        .child(
            div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(label),
        )
        .child(
            div()
                .min_w_0()
                .text_color(t.text_dim)
                .child(next.reason.clone()),
        )
}

/// The PR status line (Pink `render_pr_status`): the badge, its state word
/// in the state's tone and the title, or what the lookup found.
fn pr_status_row(c: &Card, pr: Option<&PrStatus>) -> impl IntoElement {
    let t = c.t;
    let row = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(11.))
        .text_color(t.text_dim);
    match pr {
        Some(PrStatus::Linked(pr)) => row
            .child(pr_badge(c, pr))
            .child(
                div()
                    .flex_none()
                    .text_color(pr.state.tone(t))
                    .child(pr.state.label()),
            )
            .child(div().min_w_0().truncate().child(pr.title.clone())),
        Some(PrStatus::NoMatch) => row.child("No PR found in the selected remote"),
        Some(PrStatus::UnsupportedHost) => row.child("PR status unsupported for this remote"),
        Some(PrStatus::Unavailable(reason)) => row.child(reason.clone()),
        None => row.child("Checking PR status…"),
    }
}

/// The PR badge on its composer surface (Pink: change_requests.rs
/// `pull_request_badge`), drawn by the `pink-pr-badge` package: 20 tall,
/// `px 7` gap 5 r6, 11 medium; a state-toned wash easing from 8% to 16%
/// with the label brightening, the PR glyph and the mono number. It opens
/// the PR, with a hover card naming it.
fn pr_badge(c: &Card, pr: &PullRequest) -> impl IntoElement {
    use crate::components::pr_badge as badge;
    let t = c.t;
    let tone = pr.state.tone(t);
    let tooltip = PrTooltip {
        number: pr.number,
        state: pr.state,
        title: pr.title.clone(),
    };
    let key = format!("{}-pr-badge", c.key);
    badge::badge(badge::Surface::Composer, &badge::hover_style(&key, tone, t))
        .id(SharedString::from(key.clone()))
        .role(Role::Link)
        .aria_label(SharedString::from(format!(
            "Pull request #{}, {}",
            pr.number,
            pr.state.label()
        )))
        .cursor_pointer()
        .on_hover(motion::hover_listener(key))
        .on_click(c.sender.on(GitRequest::OpenUrl(pr.url.clone())))
        .tooltip(move |_, cx| {
            let tooltip = tooltip.clone();
            cx.new(|_| tooltip).into()
        })
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(badge::glyph(ICON_PULL_REQUEST, t.readable_tone(tone, 0.85)))
        .child(badge::number(pink::MONO, format!("#{}", pr.number)))
}

/// The PR badge's hover card (Pink `ChangeRequestTooltip`): "PR #482 ·
/// Open" in the state's tone over the title in `text_muted`.
#[derive(Clone)]
struct PrTooltip {
    number: u64,
    state: PrState,
    title: SharedString,
}

impl gpui::Render for PrTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        pink::tooltip_card(&t)
            .max_w(px(320.))
            .px(px(9.))
            .py(px(7.))
            .flex()
            .flex_col()
            .gap(px(3.))
            .child(
                div()
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(self.state.tone(&t))
                    .child(format!(
                        "Pull request #{}, {}",
                        self.number,
                        self.state.label()
                    )),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .whitespace_nowrap()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child(self.title.clone()),
            )
    }
}

/// The PR panel (Pink: pull_requests.rs `render_pr_panel`): folded to its
/// buttons, or a bordered `card_glass_bg` box with the PR's details and
/// checks, or the draft form.
fn pr_panel(
    c: &Card,
    panel: &PrPanel,
    pr: Option<&PrStatus>,
    busy: bool,
    notice: Option<SharedString>,
    fields: Option<[Entity<TextInput>; 4]>,
) -> AnyElement {
    let t = c.t;
    let linked = match pr {
        Some(PrStatus::Linked(pr)) => Some(pr.number),
        _ => None,
    };
    if *panel == PrPanel::Closed {
        return div()
            .flex()
            .items_center()
            .gap(px(6.))
            .when_some(linked, |row, number| {
                row.child(
                    c.step_button("pr-details-open", "PR details & checks")
                        .on_click(c.sender.on(GitRequest::PrDetails(number))),
                )
            })
            .child(
                c.step_button("pr-draft-open", "New draft PR…")
                    .on_click(c.sender.on(GitRequest::NewDraftPr)),
            )
            .into_any_element();
    }
    let details = match panel {
        PrPanel::Details(details) => Some(details),
        _ => None,
    };
    let small = |copy: SharedString| div().text_size(px(11.)).text_color(t.text_dim).child(copy);
    let mut body =
        div()
            .id(SharedString::from(format!("{}-pr-panel", c.key)))
            .flex()
            .flex_col()
            .gap(px(CARD_GAP))
            .p(px(pink::space::S8))
            .border_1()
            .border_color(t.border)
            .rounded(px(CONTROL_RADIUS))
            .bg(t.card_glass_bg())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(12.)).text_color(t.text).child(
                        if details.is_some() {
                            "Pull request"
                        } else {
                            "New draft pull request"
                        },
                    ))
                    .child(
                        c.step_button("pr-panel-close", "Close")
                            .on_click(c.sender.on(GitRequest::ClosePrPanel)),
                    ),
            );
    match panel {
        PrPanel::Details(details) => {
            let summary = &details.summary;
            body = body
                .child(div().text_size(px(12.)).text_color(t.text).child(format!(
                    "#{}: {}{}",
                    summary.number,
                    if details.is_draft { "Draft, " } else { "" },
                    summary.title
                )))
                .child(small(
                    format!(
                        "{} into {}, {}",
                        summary.head_ref,
                        summary.base_ref,
                        summary.state.label()
                    )
                    .into(),
                ))
                .child(
                    div()
                        .id(SharedString::from(format!("{}-pr-description", c.key)))
                        .max_h(px(PR_DESCRIPTION_MAX_HEIGHT))
                        .overflow_y_scroll()
                        .text_size(px(11.))
                        .text_color(t.text)
                        .child(if details.body.is_empty() {
                            SharedString::from("No description")
                        } else {
                            details.body.clone()
                        }),
                )
                .child(small(if details.checks.is_empty() {
                    "No checks reported".into()
                } else {
                    format!("{} checks reported", details.checks.len()).into()
                }));
            for (ix, check) in details.checks.iter().enumerate() {
                let tone = match check_outcome(&check.status) {
                    CheckOutcome::Passed => t.success,
                    CheckOutcome::Failed => t.danger,
                    CheckOutcome::Pending => t.warning,
                    CheckOutcome::Unknown => t.text_dim,
                };
                let status = check.status.replace('_', " ").to_ascii_lowercase();
                body = body.child(
                    div()
                        .id(SharedString::from(format!("{}-pr-check-{ix}", c.key)))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .text_size(px(11.))
                        .text_color(t.text)
                        .child(div().min_w_0().truncate().child(check.name.clone()))
                        .child(div().flex_none().text_color(tone).child(status))
                        .when_some(check.url.clone(), |row, url| {
                            row.cursor_pointer()
                                .on_click(c.sender.on(GitRequest::OpenUrl(url)))
                        }),
                );
            }
            body = body.child(
                div()
                    .flex()
                    .gap(px(6.))
                    .child(
                        c.step_button("pr-open-github", "Open on GitHub")
                            .on_click(c.sender.on(GitRequest::OpenUrl(summary.url.clone()))),
                    )
                    .child(
                        c.step_button("pr-checks-refresh", "Refresh checks")
                            .on_click(c.sender.on(GitRequest::RefreshChecks(summary.number))),
                    ),
            );
        }
        PrPanel::Draft(draft) => {
            let head_commit: String = draft.head_commit.chars().take(8).collect();
            body = body
                .child(small(draft.repository.clone()))
                .child(small(format!("{} at {head_commit}", draft.cwd).into()));
            if let Some(fields) = &fields {
                for ((label, height), input) in [
                    ("Title", 32.0),
                    ("Description", 96.0),
                    ("Base branch", 30.0),
                    ("Head branch (current checkout)", 30.0),
                ]
                .into_iter()
                .zip(fields.iter())
                {
                    body = body.child(small(label.into())).child(
                        div()
                            .h(px(height))
                            .flex_none()
                            .overflow_hidden()
                            .border_1()
                            .border_color(t.border)
                            .rounded(px(PR_FIELD_RADIUS))
                            .text_size(px(INPUT_TEXT_SIZE))
                            .line_height(px(INPUT_LINE_HEIGHT))
                            .text_color(t.text)
                            .child(input.clone()),
                    );
                }
            }
            let label = if busy {
                "Working…"
            } else if draft.reconcile_only {
                "Check GitHub"
            } else {
                "Create draft PR"
            };
            let create = {
                let sender = c.sender.clone();
                let fields = fields.clone();
                move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                    let Some(fields) = &fields else {
                        return;
                    };
                    let [title, body, base, head] =
                        fields.clone().map(|input| input.read(cx).text());
                    sender.send(
                        &GitRequest::CreateDraftPr {
                            title,
                            body,
                            base,
                            head,
                        },
                        window,
                        cx,
                    );
                }
            };
            body = body
                .child(small(
                    "Creates a draft in this repository from the published head branch.".into(),
                ))
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(
                            git_button(c.key, "pr-create-draft", label, t)
                                .when(busy, |b| b.opacity(0.5))
                                .on_click(create),
                        )
                        .child(
                            git_button(c.key, "pr-preview-refresh", "Refresh preview", t)
                                .on_click(c.sender.on(GitRequest::RefreshDraftPreview)),
                        ),
                );
        }
        _ if !busy => {
            let retry = match linked {
                Some(number) => GitRequest::PrDetails(number),
                None => GitRequest::NewDraftPr,
            };
            body = body.child(
                c.step_button("pr-preview-retry", "Retry")
                    .on_click(c.sender.on(retry)),
            );
        }
        _ => {}
    }
    if busy {
        body = body.child(small("Checking GitHub…".into()));
    }
    if let Some(notice) = notice {
        body = body.child(small(notice));
    }
    body.into_any_element()
}

/// File checkpoints (Pink `render_checkpoint_section`): a `border` rule
/// below, the latest five turns with Diff and Preview restore, and what a
/// diff or a restore would show.
fn checkpoint_section(
    c: &Card,
    checkpoints: Option<&Checkpoints>,
    diff: Option<&CheckpointDiff>,
    restore: Option<&[SharedString]>,
    notice: Option<SharedString>,
    receipt: Option<SharedString>,
) -> impl IntoElement {
    let t = c.t;
    let dim = |copy: SharedString| div().text_size(px(11.)).text_color(t.text_dim).child(copy);
    let mut section = div()
        .flex()
        .flex_col()
        .gap(px(5.))
        .py(px(pink::space::S8))
        .border_b_1()
        .border_color(t.border)
        .child(
            div()
                .text_size(px(11.))
                .text_color(t.text)
                .child("File checkpoints"),
        );
    match checkpoints {
        Some(list) => {
            if let Some(reason) = &list.restore_unavailable_reason {
                section = section.child(dim(reason.clone()));
            }
            if list.list.is_empty() {
                section = section.child(dim("No completed turn checkpoints yet".into()));
            }
            for row in list.list.iter().take(CHECKPOINT_ROWS) {
                let outcome = row.outcome.as_deref().unwrap_or("in progress");
                let turn = row.turn_id.clone();
                let line = div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.))
                    .text_color(t.text_dim)
                    .child(format!("{}, {outcome}", row.started))
                    .when(row.before_available && row.after_available, |line| {
                        line.child(
                            git_button(c.key, &format!("checkpoint-diff-{turn}"), "Diff", t)
                                .on_click(c.sender.on(GitRequest::CheckpointDiff(turn.clone()))),
                        )
                    })
                    .when(row.before_available && list.restore_available, |line| {
                        line.child(
                            git_button(
                                c.key,
                                &format!("checkpoint-preview-{turn}"),
                                "Preview restore",
                                t,
                            )
                            .on_click(c.sender.on(GitRequest::PreviewRestore(turn.clone()))),
                        )
                    });
                section = section.child(line);
                if let Some(error) = &row.error {
                    section = section.child(dim(format!("Capture failed: {error}").into()));
                }
            }
            if list.list.len() > CHECKPOINT_ROWS {
                section = section.child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.text_faint)
                        .child(format!("Showing 5 of {} recent turns", list.list.len())),
                );
            }
        }
        None => section = section.child(dim("Checkpoints unavailable or loading…".into())),
    }
    if let Some(diff) = diff {
        let snippet: String = diff.patch.chars().take(8000).collect();
        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(dim(format!(
                    "{} changed paths, observed checkout changes",
                    diff.paths
                )
                .into()))
                .child(
                    div()
                        .id(SharedString::from(format!("{}-checkpoint-diff", c.key)))
                        .max_h(px(CHECKPOINT_DIFF_MAX_HEIGHT))
                        .overflow_y_scroll()
                        .text_size(px(11.))
                        .font_family(pink::MONO)
                        .text_color(t.text)
                        .child(snippet),
                ),
        );
    }
    if let Some(paths) = restore {
        let path_text = if paths.is_empty() {
            "No file changes".to_string()
        } else {
            paths
                .iter()
                .take(12)
                .map(|p| p.as_ref())
                .collect::<Vec<_>>()
                .join(", ")
        };
        section = section.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(dim(format!("Restore will change {} paths: {path_text}", paths.len()).into()))
                .child(dim(
                    "This changes files only. Conversation history and external actions remain as they are."
                        .into(),
                ))
                .child(
                    git_button(c.key, "checkpoint-restore", "Restore files", t)
                        .on_click(c.sender.on(GitRequest::RestoreFiles)),
                ),
        );
    }
    if let Some(notice) = notice {
        section = section.child(dim(notice));
    }
    if let Some(receipt) = receipt {
        section = section.child(dim(receipt));
    }
    section
}

/// One changed file (Pink `render_git_card` rows): its pick box, the mono
/// path, then Stage when it has unstaged changes and Unstage when staged.
/// Only a staged file can be picked for the commit.
fn file_row(c: &Card, state: &Entity<CardState>, file: &GitFile, picked: bool) -> impl IntoElement {
    let t = c.t;
    let path = file.path.clone();
    let staged = file.staged;
    let toggle = state.clone();
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .min_w_0()
        .child(
            div()
                .id(SharedString::from(format!("{}-git-select-{path}", c.key)))
                .role(Role::CheckBox)
                .aria_label(path.clone())
                .flex_none()
                .cursor_pointer()
                .text_size(px(11.))
                .text_color(t.text)
                .child(if picked { "☑" } else { "☐" })
                .on_click({
                    let path = path.clone();
                    move |_, _, cx| {
                        if staged {
                            toggle.update(cx, |s, cx| {
                                if !s.selected.insert(path.clone()) {
                                    s.selected.remove(&path);
                                }
                                cx.notify();
                            });
                        }
                    }
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(pink::MONO)
                .text_size(px(11.))
                .text_color(t.text)
                .child(path.clone()),
        )
        .when(file.unstaged, |row| {
            row.child(
                c.step_button(&format!("git-stage-{path}"), "Stage")
                    .on_click(c.sender.on(GitRequest::Stage(vec![path.clone()]))),
            )
        })
        .when(file.staged, |row| {
            row.child(
                git_button(c.key, &format!("git-unstage-{path}"), "Unstage", t)
                    .on_click(c.sender.on(GitRequest::Unstage(vec![path.clone()]))),
            )
        })
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

const BRANCH: &str = "sync/backoff-jitter";
const TRACKING: &str = "origin/sync/backoff-jitter";
const GENERATED: &str = "Back off sync retries exponentially with jitter";

fn pr_482() -> PullRequest {
    PullRequest::new(482, "Back off sync retries with jitter")
        .head_ref(BRANCH)
        .url("https://github.com/fieldnote/sync-service/pull/482")
}

/// A stand-in for the engine behind the card: it reads and writes a checkout
/// on timers, so the card shows each step running and landing.
struct Demo {
    preview: Option<GitPreview>,
    pr: Option<PrStatus>,
    busy: bool,
    notice: Option<SharedString>,
    message: SharedString,
    panel: PrPanel,
    pr_busy: bool,
    pr_notice: Option<SharedString>,
    checkpoints: Option<Checkpoints>,
    diff: Option<CheckpointDiff>,
    restore: Option<(SharedString, Vec<SharedString>)>,
    receipt: Option<SharedString>,
    checks_settled: bool,
    commits: u32,
    _work: Option<Task<()>>,
    _pr_work: Option<Task<()>>,
    _lookup: Option<Task<()>>,
    _script: Option<Task<()>>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut demo = Self {
            preview: None,
            pr: None,
            busy: true,
            notice: None,
            message: SharedString::default(),
            panel: PrPanel::Closed,
            pr_busy: false,
            pr_notice: None,
            checkpoints: None,
            diff: None,
            restore: None,
            receipt: None,
            checks_settled: false,
            commits: 0,
            _work: None,
            _pr_work: None,
            _lookup: None,
            _script: None,
        };
        demo._script = Some(Self::script(cx));
        demo
    }

    /// The checkout loads, the agent's doc change is staged and a message
    /// is drafted; committing and pushing are left to the viewer.
    fn script(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            let pause = |ms: u64| Duration::from_millis(ms);
            cx.background_executor().timer(pause(800)).await;
            this.update(cx, |d, cx| d.loaded(cx)).ok();
            cx.background_executor().timer(pause(1200)).await;
            this.update(cx, |d, cx| {
                d.request(&GitRequest::Stage(vec!["docs/sync.md".into()]), cx)
            })
            .ok();
            cx.background_executor().timer(pause(1400)).await;
            this.update(cx, |d, cx| {
                d.request(&GitRequest::GenerateMessage { paths: Vec::new() }, cx)
            })
            .ok();
        })
    }

    fn loaded(&mut self, cx: &mut Context<Self>) {
        self.busy = false;
        self.preview = Some(
            GitPreview::new(BRANCH, "9b7e0d2a41c6")
                .remote("origin")
                .files([
                    GitFile::new("crates/sync/src/backoff.rs")
                        .staged(true)
                        .unstaged(false),
                    GitFile::new("crates/sync/src/retry_policy.rs")
                        .staged(true)
                        .unstaged(false),
                    GitFile::new("docs/sync.md"),
                ])
                .publish(
                    GitPublish::new(TRACKING)
                        .upstream(TRACKING)
                        .default_branch("main"),
                ),
        );
        self.checkpoints = Some(Checkpoints::new([
            Checkpoint::new("turn-14", "Oct 24 14:02").outcome("completed"),
            Checkpoint::new("turn-13", "Oct 24 13:41").outcome("completed"),
            Checkpoint::new("turn-12", "Oct 24 13:20")
                .outcome("interrupted")
                .snapshots(true, false),
        ]));
        self.look_up_pr(cx);
        cx.notify();
    }

    /// Every read re-checks the PR (Pink `load_git_pr`).
    fn look_up_pr(&mut self, cx: &mut Context<Self>) {
        self.pr = None;
        self._lookup = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(600))
                .await;
            this.update(cx, |d, cx| {
                d.pr = Some(PrStatus::Linked(pr_482()));
                cx.notify();
            })
            .ok();
        }));
    }

    /// A Git read or write: busy for `ms`, then `done`.
    fn run(
        &mut self,
        ms: u64,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        self.busy = true;
        self._work = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            this.update(cx, |d, cx| {
                d.busy = false;
                done(d, cx);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// A GitHub read: the panel says "Checking GitHub…" for `ms`.
    fn run_pr(
        &mut self,
        ms: u64,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        self.pr_busy = true;
        self.pr_notice = None;
        self._pr_work = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            this.update(cx, |d, cx| {
                d.pr_busy = false;
                done(d, cx);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn details(&self) -> PrDetails {
        PrDetails::new(pr_482())
            .body("Retries now double from the policy's base delay up to its cap, with up to 20% jitter so clients that failed together spread out.")
            .check(PrCheck::new("test", "SUCCESS"))
            .check(PrCheck::new("lint", "SUCCESS"))
            .check(PrCheck::new("e2e sync-offline", if self.checks_settled { "SUCCESS" } else { "IN_PROGRESS" }))
    }

    fn request(&mut self, request: &GitRequest, cx: &mut Context<Self>) {
        match request.clone() {
            GitRequest::Refresh => self.run(600, cx, |d, cx| {
                d.notice = None;
                d.look_up_pr(cx);
            }),
            GitRequest::Stage(paths) | GitRequest::Unstage(paths) => {
                let stage = matches!(request, GitRequest::Stage(_));
                self.notice = None;
                self.run(600, cx, move |d, _| {
                    if let Some(preview) = d.preview.as_mut() {
                        for file in preview.files.iter_mut().filter(|f| paths.contains(&f.path)) {
                            file.staged = stage;
                            file.unstaged = !stage;
                        }
                    }
                    d.notice = Some("Git selection updated".into());
                });
            }
            GitRequest::GenerateMessage { .. } => self.run(800, cx, |d, _| {
                d.message = GENERATED.into();
                d.notice = Some("Generated draft; review and edit before committing".into());
            }),
            GitRequest::Commit { paths, message }
            | GitRequest::CommitAndPush { paths, message } => {
                let push = matches!(request, GitRequest::CommitAndPush { .. });
                self.notice = None;
                self.run(800, cx, move |d, cx| {
                    if message.trim().is_empty() {
                        d.notice = Some("Commit message is empty".into());
                        return;
                    }
                    if paths.is_empty() {
                        d.notice = Some("Nothing selected to commit".into());
                        return;
                    }
                    d.commits += 1;
                    let sha = ["3f9c2ab", "8d41e07", "c52a9f3"][(d.commits as usize - 1) % 3];
                    if let Some(preview) = d.preview.as_mut() {
                        preview
                            .files
                            .retain(|f| !(f.staged && paths.contains(&f.path)));
                        preview.head = format!("{sha}e5b19").into();
                        if let Some(publish) = preview.publish.as_mut() {
                            if push {
                                publish.ahead = 0;
                            } else {
                                publish.ahead += 1;
                            }
                        }
                    }
                    d.message = SharedString::default();
                    d.notice = Some(if push {
                        "Push completed".into()
                    } else {
                        format!("Committed {sha}").into()
                    });
                    d.look_up_pr(cx);
                });
            }
            GitRequest::Push => {
                self.notice = None;
                self.run(900, cx, |d, cx| {
                    if let Some(publish) = d.preview.as_mut().and_then(|p| p.publish.as_mut()) {
                        publish.ahead = 0;
                        publish.published = true;
                    }
                    d.notice = Some("Push completed".into());
                    d.look_up_pr(cx);
                });
            }
            GitRequest::PrDetails(_) => {
                self.panel = PrPanel::Empty;
                self.run_pr(800, cx, |d, _| d.panel = PrPanel::Details(d.details()));
            }
            GitRequest::RefreshChecks(_) => self.run_pr(800, cx, |d, _| {
                d.checks_settled = true;
                d.panel = PrPanel::Details(d.details());
            }),
            GitRequest::NewDraftPr | GitRequest::RefreshDraftPreview => {
                self.panel = PrPanel::Empty;
                self.run_pr(700, cx, |d, _| {
                    d.panel = PrPanel::Draft(
                        DraftPr::new("fieldnote/sync-service", BRANCH)
                            .checkout("~/code/sync-service", "9b7e0d2a41c6")
                            .title(GENERATED)
                            .body(
                                "Retries double from the policy's base delay, capped, with jitter.",
                            ),
                    );
                });
            }
            GitRequest::ClosePrPanel => {
                self.panel = PrPanel::Closed;
                self.pr_notice = None;
                cx.notify();
            }
            GitRequest::CreateDraftPr { .. } => self.run_pr(900, cx, |d, _| {
                d.pr_notice = Some("A pull request already exists for this branch: #482".into());
            }),
            GitRequest::CheckpointDiff(_) => {
                self.diff = Some(CheckpointDiff {
                    paths: 2,
                    patch:
                        "crates/sync/src/backoff.rs  +6 −4\ncrates/sync/src/retry_policy.rs  +18"
                            .into(),
                });
                cx.notify();
            }
            GitRequest::PreviewRestore(turn) => {
                self.restore = Some((
                    turn,
                    vec![
                        "crates/sync/src/backoff.rs".into(),
                        "crates/sync/src/retry_policy.rs".into(),
                    ],
                ));
                cx.notify();
            }
            GitRequest::RestoreFiles => {
                if let Some((turn, paths)) = self.restore.take() {
                    self.receipt =
                        Some(format!("Restored {} files from {turn}", paths.len()).into());
                }
                cx.notify();
            }
            GitRequest::OpenUrl(_) => {}
        }
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let surface = Pink::get(cx).chrome_surface();
        let demo =
            window.use_keyed_state(ElementId::Name("git-actions-demo".into()), cx, |_, cx| {
                Demo::new(cx)
            });
        let d = demo.read(cx);
        let mut card = GitActions::new("git-actions")
            .preview(d.preview.clone())
            .pr_status(d.pr.clone())
            .pr_panel(d.panel.clone())
            .pr_busy(d.pr_busy)
            .busy(d.busy)
            .notice(d.notice.clone())
            .message(d.message.clone())
            .width(560.);
        if let Some(checkpoints) = d.checkpoints.clone() {
            card = card.checkpoints(checkpoints);
        }
        if let Some(diff) = d.diff.clone() {
            card = card.checkpoint_diff(diff);
        }
        if let Some((_, paths)) = d.restore.clone() {
            card = card.restore_preview(paths);
        }
        if let Some(receipt) = d.receipt.clone() {
            card = card.checkpoint_receipt(receipt);
        }
        if let Some(notice) = d.pr_notice.clone() {
            card = card.pr_notice(notice);
        }
        let (on_request, on_message) = (demo.clone(), demo.clone());
        // The card sits at the top of the changes pane, on the chrome
        // surface of the right sidebar.
        div()
            .w(px(560.))
            .rounded(px(pink::radius::PANEL))
            .overflow_hidden()
            .bg(surface)
            .child(
                card.on_request(move |request, _, cx| {
                    on_request.update(cx, |d, cx| d.request(request, cx));
                })
                .on_message(move |text, _, cx| {
                    on_message.update(cx, |d, _| d.message = text.clone());
                }),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picked(paths: &[&str]) -> HashSet<SharedString> {
        paths.iter().map(|p| SharedString::from(*p)).collect()
    }

    fn feature(files: Vec<GitFile>, publish: Option<GitPublish>) -> GitPreview {
        let mut preview = GitPreview::new("feature/next", "a".repeat(40))
            .remote("origin")
            .files(files);
        preview.publish = publish;
        preview
    }

    fn tracking() -> GitPublish {
        GitPublish::new("origin/feature/next")
            .upstream("origin/feature/next")
            .default_branch("main")
    }

    #[test]
    fn local_work_comes_first() {
        let preview = feature(
            vec![
                GitFile::new("a.rs").staged(true).unstaged(false),
                GitFile::new("b.rs"),
            ],
            Some(tracking()),
        );
        let step = resolve(&preview, &picked(&["a.rs"]), true, None, None);
        assert_eq!(step.step, NextStep::Commit);
        assert_eq!(
            step.reason,
            "Write or generate a message, then commit 1 selected file"
        );
        assert!(step.accents("git-commit") && !step.accents("git-commit-push"));
        let nothing = resolve(&preview, &picked(&[]), false, None, None);
        assert_eq!(nothing.step, NextStep::SelectStaged);
        // A lone unstaged file names its Stage button.
        let one = feature(vec![GitFile::new("b.rs")], Some(tracking()));
        let stage = resolve(&one, &picked(&[]), true, None, None);
        assert_eq!(stage.step, NextStep::StageChanges);
        assert!(stage.accents("git-stage-b.rs"));
    }

    #[test]
    fn publishing_then_the_forge() {
        let ahead = feature(vec![], Some(tracking().ahead(2)));
        let push = resolve(
            &ahead,
            &picked(&[]),
            true,
            Some(&PrStatus::Linked(PullRequest::new(9, "x"))),
            None,
        );
        assert_eq!(push.step, NextStep::Push);
        assert_eq!(
            push.reason,
            "2 commits are not on origin/feature/next; push to update #9"
        );
        let level = feature(vec![], Some(tracking()));
        assert_eq!(
            resolve(&level, &picked(&[]), true, None, None).step,
            NextStep::CheckingPr
        );
        let draft = resolve(&level, &picked(&[]), true, Some(&PrStatus::NoMatch), None);
        assert_eq!(draft.step, NextStep::NewDraftPr);
        assert!(draft.accents("pr-draft-open"));
        let diverged = feature(vec![], Some(tracking().ahead(1).behind(3)));
        assert_eq!(
            resolve(&diverged, &picked(&[]), true, None, None).step,
            NextStep::UpdateBranch
        );
    }

    #[test]
    fn checks_decide_an_open_pr() {
        let level = feature(vec![], Some(tracking()));
        let pr = PullRequest::new(9, "x");
        let status = PrStatus::Linked(pr.clone());
        let running = PrDetails::new(pr.clone())
            .check(PrCheck::new("test", "SUCCESS"))
            .check(PrCheck::new("e2e", "IN_PROGRESS"));
        let step = resolve(&level, &picked(&[]), true, Some(&status), Some(&running));
        assert_eq!(step.step, NextStep::WaitForChecks);
        assert_eq!(step.reason, "1 of 2 checks are still running on #9");
        let failed = PrDetails::new(pr.clone()).check(PrCheck::new("lint", "FAILURE"));
        assert_eq!(
            resolve(&level, &picked(&[]), true, Some(&status), Some(&failed)).step,
            NextStep::FixChecks
        );
        let merged = PrStatus::Linked(pr.state(PrState::Merged));
        assert_eq!(
            resolve(&level, &picked(&[]), true, Some(&merged), None).step,
            NextStep::Merged
        );
    }

    #[test]
    fn a_rechecked_pr_keeps_the_shown_step() {
        let mut shown: ShownStep = None;
        let publish = tracking();
        let view = next(NextStep::ViewPr, "#9 is open");
        assert_eq!(
            keep_shown(&mut shown, view, 1, Some(&publish)).step,
            NextStep::ViewPr
        );
        let checking = next(NextStep::CheckingPr, "checking");
        assert_eq!(
            keep_shown(&mut shown, checking.clone(), 1, Some(&publish)).step,
            NextStep::ViewPr
        );
        // A moved checkout shows the fresh step.
        assert_eq!(
            keep_shown(&mut shown, checking, 2, Some(&publish)).step,
            NextStep::CheckingPr
        );
    }

    #[test]
    fn preview_keys_follow_the_files() {
        let a = feature(vec![GitFile::new("a.rs")], None);
        let b = feature(vec![GitFile::new("a.rs").staged(true)], None);
        assert_ne!(preview_key(&a), preview_key(&b));
        assert_eq!(preview_key(&a), preview_key(&a.clone()));
    }
}
