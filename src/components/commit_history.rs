//! Commit History – a repository's commits, with the lanes that tie them
//! together, as Pink's History pane.
//!
//! Pink: `history.rs` `GitHistory`, a Review surface in the right panel
//! (#176): Review's `History` action in the panel's band opens it
//! (`shell/right_rail.rs` `show_history_surface`), so the band reads
//! `Review` (shown), `Files` and `More`, with History and Expand on the right
//! ([`crate::pink::right_panel`]). Under the band sits the surface's own
//! toolbar (`changes.rs` `render_header_controls` inside
//! `surface_chrome::toolbar`). The toolbar names the checked-out branch in
//! mono, counts the commits with how far the branch is ahead of / behind its
//! upstream, and carries the search field that grows out of a magnifier,
//! Fetch all, the branch-tips toggle and Refresh. Under a 24 px column header
//! every commit is a 36 px row: a topological lane graph (one lane per open
//! path, Bézier hand-offs at forks and merges, a ring around HEAD), the
//! subject with its branch / remote / tag pills, then Author (an initial
//! disc or the name), Date and a 7-character SHA that copies on click.
//!
//! Pointing at a lane focuses its whole path: its strokes thicken, its nodes
//! grow and every other lane and row dims. A branch tip shows a small fold
//! control beside its node that collapses the branch's linear run (rows fold
//! away over 180 ms) and expands it again; the toolbar's toggle swaps the
//! list for branch tips alone. Columns resize by their dividers (double-click
//! resets), reorder by dragging their headers and toggle from the checklist
//! menu; right-clicking Author switches it to names. The graph narrows to
//! keep subjects readable and collapses into a single rail when it must.
//!
//! ```ignore
//! CommitHistory::new("history", "main")
//!     .head("3f9c2e1")
//!     .total(214)
//!     .commits(vec![HistoryCommit::new("3f9c2e1", "Tighten invoice rounding")
//!         .parents(["b41d0a7"])
//!         .author("Lena Hartmann")
//!         .date("2026-10-22")
//!         .reference(CommitRef::branch("main"))])
//!     .on_open(|commit, _, _| println!("open {}", commit.sha))
//! ```

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnimationExt, AnyElement, AnyView, App, Bounds, ClipboardItem, Context, DragMoveEvent,
    ElementId, Entity, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, Render, ScrollHandle, SharedString, Subscription, Task, Window,
    anchored, canvas, container_query, deferred, div, point, prelude::*, px, size, svg,
};

use crate::{
    HoverFill,
    components::{
        commit_row as cr,
        icon_button::{IconButtonStyle, icon_button},
        surface_toolbar as st,
    },
    interaction::HoverColors,
    motion::{self, COLLAPSE, HOVER_FADE, MENU_OUT, MENU_RISE_DOWN, RESIZE},
    pink::{self, FillTier, Pink, chrome, layout, loaders, radius, right_panel},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Commit History";
pub const DESCRIPTION: &str = "A repository's commits as Pink's History pane: a lane graph that traces every branch and merge, branch, remote and tag pills, author, date and SHA columns you can resize and reorder, and a toolbar to search, fetch and fold the list down to branch tips";

const ICON_BRANCH: &str = "icons/commit-history/git-branch.svg";
const ICON_CLOUD: &str = "icons/commit-history/cloud.svg";
const ICON_TAG: &str = "icons/commit-history/tag.svg";
const ICON_FOLD: &str = "icons/commit-history/fold-vertical.svg";
const ICON_EXPAND: &str = "icons/commit-history/maximize.svg";
const ICON_SEARCH: &str = "icons/commit-history/magnifer.svg";
const ICON_CLOSE: &str = "icons/commit-history/close.svg";
const ICON_REFRESH: &str = "icons/commit-history/refresh.svg";
const ICON_COLUMNS: &str = "icons/commit-history/checklist.svg";
const ICON_DOWN: &str = "icons/commit-history/alt-arrow-down.svg";
const ICON_CHECK: &str = "icons/commit-history/check.svg";

// Pink: history.rs constants.
const ROW_HEIGHT: f32 = cr::ROW_HEIGHT;
const LANE_SPACING: f32 = 12.0;
const NODE_RADIUS: f32 = cr::NODE_RADIUS;
const GRAPH_SATURATION: f32 = 0.72;
/// The first lane's center sits on the header's text gutter.
const GRAPH_SIDE_PADDING: f32 = chrome::EDGE_INSET * 2.0 - NODE_RADIUS;
const GRAPH_TRAILING_PADDING: f32 = 12.0;
const GRAPH_MIN_COMPACT_WIDTH: f32 = 48.0;
const GRAPH_MAX_WIDTH_RATIO: f32 = 0.34;
const GRAPH_RESIZE_STEP: f32 = 2.0;
const COMPACT_ENTER_SUBJECT_WIDTH: f32 = 160.0;
const COMPACT_EXIT_SUBJECT_WIDTH: f32 = 184.0;
const COMPACT_ENTER_LANE_SPACING: f32 = 4.0;
const COMPACT_EXIT_LANE_SPACING: f32 = 5.0;
const GRAPH_HIT_RADIUS: f32 = 5.5;
const GRAPH_UNFOCUSED_OPACITY: f32 = 0.24;
const ROW_UNFOCUSED_OPACITY: f32 = 0.6;
const SUBJECT_MIN_WIDTH: f32 = cr::SUBJECT_MIN_WIDTH;
const REF_AREA_RATIO: f32 = 0.45;
const REF_BADGE_MAX_WIDTH: f32 = cr::REF_BADGE_MAX_WIDTH;
const REF_GAP: f32 = cr::REF_GAP;
const SEARCH_WIDTH: f32 = 196.0;
const SEARCH_IDLE_DISMISS: Duration = Duration::from_millis(1_500);
/// The comparison is supplemental: it hides before it crowds the controls.
const COMPARISON_MIN_WIDTH: f32 = 260.0;
/// Pink: `copy_sha` shows "Copied" for 1.2 s.
const COPIED_FOR: Duration = Duration::from_millis(1_200);
/// Pink: the column and author menus (`render_column_menu` /
/// `render_author_menu`): radius 9 with rows at 9 − the card inset.
const COLUMN_MENU_RADIUS: f32 = 9.0;

// Pink: settings.rs `GitHistoryColumnWidths`.
const AUTHOR_MIN: f32 = 44.0;
const AUTHOR_MAX: f32 = 220.0;
const DATE_MIN: f32 = 68.0;
const DATE_MAX: f32 = 180.0;
const SHA_MIN: f32 = 58.0;
const SHA_MAX: f32 = 140.0;

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// What a ref names (Pink `GitHistoryRefKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RefKind {
    Branch,
    Remote,
    Tag,
}

/// A branch, remote branch or tag pointing at a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitRef {
    pub label: SharedString,
    pub kind: RefKind,
}

impl CommitRef {
    pub fn new(label: impl Into<SharedString>, kind: RefKind) -> Self {
        Self {
            label: label.into(),
            kind,
        }
    }

    pub fn branch(label: impl Into<SharedString>) -> Self {
        Self::new(label, RefKind::Branch)
    }

    pub fn remote(label: impl Into<SharedString>) -> Self {
        Self::new(label, RefKind::Remote)
    }

    pub fn tag(label: impl Into<SharedString>) -> Self {
        Self::new(label, RefKind::Tag)
    }
}

/// One commit, listed child before parent (`git log --topo-order`).
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryCommit {
    pub sha: SharedString,
    pub parents: Vec<SharedString>,
    pub subject: SharedString,
    pub author: SharedString,
    /// When it was authored: `YYYY-MM-DD` or RFC 3339, shown as "Oct 22, 2026".
    pub date: SharedString,
    pub refs: Vec<CommitRef>,
}

impl HistoryCommit {
    pub fn new(sha: impl Into<SharedString>, subject: impl Into<SharedString>) -> Self {
        Self {
            sha: sha.into(),
            parents: Vec::new(),
            subject: subject.into(),
            author: SharedString::default(),
            date: SharedString::default(),
            refs: Vec::new(),
        }
    }

    pub fn parents<S: Into<SharedString>>(mut self, parents: impl IntoIterator<Item = S>) -> Self {
        self.parents = parents.into_iter().map(Into::into).collect();
        self
    }

    pub fn author(mut self, author: impl Into<SharedString>) -> Self {
        self.author = author.into();
        self
    }

    pub fn date(mut self, date: impl Into<SharedString>) -> Self {
        self.date = date.into();
        self
    }

    pub fn reference(mut self, reference: CommitRef) -> Self {
        self.refs.push(reference);
        self
    }

    pub fn refs(mut self, refs: impl IntoIterator<Item = CommitRef>) -> Self {
        self.refs.extend(refs);
        self
    }
}

/// How the branch stands against its upstream (Pink `GitHistoryComparison`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub base: SharedString,
    pub ahead: usize,
    pub behind: usize,
}

impl Comparison {
    pub fn new(base: impl Into<SharedString>, ahead: usize, behind: usize) -> Self {
        Self {
            base: base.into(),
            ahead,
            behind,
        }
    }
}

/// The optional data columns (Pink `GitHistoryColumn`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryColumn {
    Author,
    Date,
    Sha,
}

/// How the Author column shows a person (Pink `GitHistoryAuthorDisplay`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthorDisplay {
    #[default]
    Avatar,
    Name,
}

type OpenHandler = Rc<dyn Fn(&HistoryCommit, &mut Window, &mut App)>;
type CopyHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// The History pane.
#[derive(IntoElement)]
pub struct CommitHistory {
    id: ElementId,
    branch: SharedString,
    head: Option<SharedString>,
    commits: Vec<HistoryCommit>,
    total: Option<usize>,
    comparison: Option<Comparison>,
    width: f32,
    height: f32,
    loading: bool,
    has_more: bool,
    loading_more: bool,
    fetching: bool,
    fetch_error: Option<SharedString>,
    error: Option<SharedString>,
    on_open: Option<OpenHandler>,
    on_copy: Option<CopyHandler>,
    on_fetch: Option<ActionHandler>,
    on_load_more: Option<ActionHandler>,
    on_refresh: Option<ActionHandler>,
}

impl CommitHistory {
    /// `branch` names the checkout in the toolbar's title slot.
    pub fn new(id: impl Into<ElementId>, branch: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            branch: branch.into(),
            head: None,
            commits: Vec::new(),
            total: None,
            comparison: None,
            width: 640.0,
            height: 600.0,
            loading: false,
            has_more: false,
            loading_more: false,
            fetching: false,
            fetch_error: None,
            error: None,
            on_open: None,
            on_copy: None,
            on_fetch: None,
            on_load_more: None,
            on_refresh: None,
        }
    }

    /// The checked-out commit, ringed in the graph.
    pub fn head(mut self, sha: impl Into<SharedString>) -> Self {
        self.head = Some(sha.into());
        self
    }

    pub fn commits(mut self, commits: Vec<HistoryCommit>) -> Self {
        self.commits = commits;
        self
    }

    /// Commits reachable from HEAD (`git rev-list --count HEAD`), counted in
    /// the toolbar.
    pub fn total(mut self, count: usize) -> Self {
        self.total = Some(count);
        self
    }

    pub fn comparison(mut self, comparison: Comparison) -> Self {
        self.comparison = Some(comparison);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The first page is still on its way.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Older commits exist past the loaded ones: the list ends in Load more.
    pub fn has_more(mut self, has_more: bool) -> Self {
        self.has_more = has_more;
        self
    }

    pub fn loading_more(mut self, loading: bool) -> Self {
        self.loading_more = loading;
        self
    }

    /// A fetch from every remote is running.
    pub fn fetching(mut self, fetching: bool) -> Self {
        self.fetching = fetching;
        self
    }

    pub fn fetch_error(mut self, error: impl Into<SharedString>) -> Self {
        self.fetch_error = Some(error.into());
        self
    }

    /// The last page failed to load; Load more turns into Retry.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// A row was clicked (Pink opens the commit as its own diff tab).
    pub fn on_open(mut self, f: impl Fn(&HistoryCommit, &mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(f));
        self
    }

    /// A SHA was copied to the clipboard.
    pub fn on_copy(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_copy = Some(Rc::new(f));
        self
    }

    pub fn on_fetch(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_fetch = Some(Rc::new(f));
        self
    }

    pub fn on_load_more(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_load_more = Some(Rc::new(f));
        self
    }

    pub fn on_refresh(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_refresh = Some(Rc::new(f));
        self
    }
}

// ---------------------------------------------------------------------------
// Graph layout (Pink: history.rs `layout_graph` and friends)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentShape {
    Through,
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphSegment {
    from_lane: usize,
    to_lane: usize,
    color_id: usize,
    shape: SegmentShape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphRow {
    node_lane: usize,
    node_color_id: usize,
    segments: Vec<GraphSegment>,
    is_head: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct GraphLayout {
    rows: Vec<GraphRow>,
    max_lane_count: usize,
}

#[derive(Debug, Clone)]
struct ActiveLane {
    id: usize,
    color_id: usize,
    target: SharedString,
}

fn lane_index(lanes: &[ActiveLane], id: usize) -> usize {
    lanes.iter().position(|lane| lane.id == id).unwrap_or(0)
}

/// Commits arrive child before parent. Active lanes point at the parent that
/// will resolve each open path (Pink `layout_graph`, verbatim).
fn layout_graph(commits: &[HistoryCommit], head: Option<&str>) -> GraphLayout {
    let mut active: Vec<ActiveLane> = Vec::new();
    let mut next_lane_id = 0usize;
    let mut next_color_id = 0usize;
    let mut max_lane_count = 0usize;
    let mut rows = Vec::with_capacity(commits.len());

    for commit in commits {
        let before = active.clone();
        let incoming: Vec<usize> = before
            .iter()
            .enumerate()
            .filter_map(|(index, lane)| (lane.target == commit.sha).then_some(index))
            .collect();
        let primary_incoming = incoming.first().copied();
        let node_lane = primary_incoming.unwrap_or(before.len());
        let primary_lane = primary_incoming.and_then(|index| before.get(index));
        let node_color_id = primary_lane.map(|lane| lane.color_id).unwrap_or_else(|| {
            let color = next_color_id;
            next_color_id += 1;
            color
        });
        let resolved: HashSet<usize> = incoming.iter().map(|&index| before[index].id).collect();
        let mut next: Vec<ActiveLane> = before
            .iter()
            .filter(|lane| !resolved.contains(&lane.id))
            .cloned()
            .collect();
        let mut outgoing: Vec<(usize, usize)> = Vec::new();

        let mut primary_outgoing = None;
        if let Some(first_parent) = commit.parents.first() {
            let id = primary_lane.map(|lane| lane.id).unwrap_or_else(|| {
                let id = next_lane_id;
                next_lane_id += 1;
                id
            });
            let lane = ActiveLane {
                id,
                color_id: node_color_id,
                target: first_parent.clone(),
            };
            next.insert(node_lane.min(next.len()), lane.clone());
            primary_outgoing = Some(id);
            outgoing.push((id, lane.color_id));
        }

        let mut parent_offset = 1usize;
        for parent in commit.parents.iter().skip(1) {
            if let Some(existing) = next.iter().find(|lane| lane.target == *parent) {
                outgoing.push((existing.id, existing.color_id));
                continue;
            }
            let lane = ActiveLane {
                id: next_lane_id,
                color_id: next_color_id,
                target: parent.clone(),
            };
            next_lane_id += 1;
            next_color_id += 1;
            let primary_index = primary_outgoing
                .map(|id| lane_index(&next, id))
                .unwrap_or_else(|| node_lane.min(next.len()));
            next.insert(
                (primary_index + parent_offset).min(next.len()),
                lane.clone(),
            );
            parent_offset += 1;
            outgoing.push((lane.id, lane.color_id));
        }

        let mut segments: Vec<GraphSegment> = before
            .iter()
            .enumerate()
            .filter(|(_, lane)| !resolved.contains(&lane.id))
            .map(|(from_lane, lane)| GraphSegment {
                from_lane,
                to_lane: lane_index(&next, lane.id),
                color_id: lane.color_id,
                shape: SegmentShape::Through,
            })
            .collect();
        segments.extend(incoming.iter().map(|&from_lane| GraphSegment {
            from_lane,
            to_lane: node_lane,
            color_id: before[from_lane].color_id,
            shape: SegmentShape::Incoming,
        }));
        segments.extend(outgoing.into_iter().map(|(id, color_id)| GraphSegment {
            from_lane: node_lane,
            to_lane: lane_index(&next, id),
            color_id,
            shape: SegmentShape::Outgoing,
        }));

        max_lane_count = max_lane_count
            .max(before.len())
            .max(next.len())
            .max(node_lane + 1);
        rows.push(GraphRow {
            node_lane,
            node_color_id,
            segments,
            is_head: head == Some(commit.sha.as_ref()),
        });
        active = next;
    }

    GraphLayout {
        rows,
        max_lane_count,
    }
}

/// Pink `branch_ref_key`: local and remote branches fold; tags don't.
fn branch_ref_key(reference: &CommitRef) -> Option<String> {
    let prefix = match reference.kind {
        RefKind::Branch => "local",
        RefKind::Remote => "remote",
        RefKind::Tag => return None,
    };
    Some(format!("{prefix}:{}", reference.label))
}

/// Removes the linear stretches of the collapsed branches' lanes, keeping
/// refs, roots, merges and branch points; parents across a hidden run are
/// contracted to the nearest visible ancestors (Pink `collapse_branch_runs`).
fn collapse_branch_runs(
    commits: &[HistoryCommit],
    collapsed: &HashSet<String>,
    head: Option<&str>,
) -> (Vec<HistoryCommit>, HashMap<String, usize>) {
    if collapsed.is_empty() || commits.is_empty() {
        return (commits.to_vec(), HashMap::new());
    }
    let source = layout_graph(commits, head);
    let mut colors_by_ref = HashMap::new();
    for (commit, row) in commits.iter().zip(&source.rows) {
        for reference in &commit.refs {
            if let Some(key) = branch_ref_key(reference)
                && collapsed.contains(&key)
            {
                colors_by_ref.insert(key, row.node_color_id);
            }
        }
    }
    if colors_by_ref.is_empty() {
        return (commits.to_vec(), HashMap::new());
    }
    let collapsed_colors: HashSet<_> = colors_by_ref.values().copied().collect();
    let mut child_counts: HashMap<&str, usize> = HashMap::new();
    for commit in commits {
        for parent in &commit.parents {
            *child_counts.entry(parent.as_ref()).or_default() += 1;
        }
    }
    let mut visible = HashSet::new();
    let mut hidden_counts = HashMap::new();
    for (commit, row) in commits.iter().zip(&source.rows) {
        let is_selected_tip = commit
            .refs
            .iter()
            .any(|reference| branch_ref_key(reference).is_some_and(|key| collapsed.contains(&key)));
        let is_junction = commit.parents.len() != 1
            || child_counts
                .get(commit.sha.as_ref())
                .copied()
                .unwrap_or_default()
                > 1;
        let hide = collapsed_colors.contains(&row.node_color_id)
            && !is_selected_tip
            && commit.refs.is_empty()
            && !is_junction;
        if hide {
            for (key, color) in &colors_by_ref {
                if *color == row.node_color_id {
                    *hidden_counts.entry(key.clone()).or_default() += 1;
                }
            }
        } else {
            visible.insert(commit.sha.clone());
        }
    }
    (compact_commits_to_visible(commits, &visible), hidden_counts)
}

/// Keeps a subset in topological order and contracts parent edges across the
/// omitted commits (Pink `compact_commits_to_visible`).
fn compact_commits_to_visible(
    commits: &[HistoryCommit],
    visible: &HashSet<SharedString>,
) -> Vec<HistoryCommit> {
    let by_sha: HashMap<&str, &HistoryCommit> = commits
        .iter()
        .map(|commit| (commit.sha.as_ref(), commit))
        .collect();

    fn nearest(
        sha: &SharedString,
        visible: &HashSet<SharedString>,
        by_sha: &HashMap<&str, &HistoryCommit>,
        memo: &mut HashMap<SharedString, Vec<SharedString>>,
        visiting: &mut HashSet<SharedString>,
    ) -> Vec<SharedString> {
        if visible.contains(sha) || !by_sha.contains_key(sha.as_ref()) {
            return vec![sha.clone()];
        }
        if let Some(cached) = memo.get(sha) {
            return cached.clone();
        }
        if !visiting.insert(sha.clone()) {
            return Vec::new();
        }
        let mut resolved: Vec<SharedString> = Vec::new();
        for parent in &by_sha[sha.as_ref()].parents {
            for found in nearest(parent, visible, by_sha, memo, visiting) {
                if !resolved.contains(&found) {
                    resolved.push(found);
                }
            }
        }
        visiting.remove(sha);
        memo.insert(sha.clone(), resolved.clone());
        resolved
    }

    let mut memo = HashMap::new();
    commits
        .iter()
        .filter(|commit| visible.contains(&commit.sha))
        .cloned()
        .map(|mut commit| {
            let mut seen = HashSet::new();
            let mut visiting = HashSet::new();
            commit.parents = commit
                .parents
                .iter()
                .flat_map(|parent| nearest(parent, visible, &by_sha, &mut memo, &mut visiting))
                .filter(|parent| seen.insert(parent.clone()))
                .collect();
            commit
        })
        .collect()
}

/// Pink engine `fuzzy_score`: every term's characters in order.
fn fuzzy_score(query: &str, candidate: &str) -> Option<usize> {
    let candidate = candidate.to_lowercase();
    query
        .split_whitespace()
        .map(str::to_lowercase)
        .try_fold(0usize, |total, term| {
            let mut at = 0;
            let mut score = 0usize;
            let mut previous_end = None;
            for needle in term.chars() {
                let found = candidate[at..].find(needle)? + at;
                score += found;
                if previous_end == Some(found) {
                    score = score.saturating_sub(2);
                }
                at = found + needle.len_utf8();
                previous_end = Some(at);
            }
            Some(total.saturating_add(score))
        })
}

/// Pink engine `git_history_matches`: each term is a SHA prefix or fuzzily
/// in the subject.
fn history_matches(query: &str, commit: &HistoryCommit) -> bool {
    let sha = commit.sha.to_ascii_lowercase();
    query.split_whitespace().all(|term| {
        sha.starts_with(&term.to_ascii_lowercase()) || fuzzy_score(term, &commit.subject).is_some()
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ViewMode {
    #[default]
    AllCommits,
    BranchTips,
}

/// The rows the pane shows for a mode, fold set and search (Pink
/// `recompute_view`), with each collapsed branch's hidden count.
fn compute_view(
    commits: &[HistoryCommit],
    head: Option<&str>,
    mode: ViewMode,
    collapsed: &HashSet<String>,
    query: &str,
) -> (Vec<HistoryCommit>, HashMap<String, usize>) {
    if !query.trim().is_empty() {
        let visible: HashSet<SharedString> = commits
            .iter()
            .filter(|commit| history_matches(query, commit))
            .map(|commit| commit.sha.clone())
            .collect();
        return (
            compact_commits_to_visible(commits, &visible),
            HashMap::new(),
        );
    }
    match mode {
        ViewMode::AllCommits => collapse_branch_runs(commits, collapsed, head),
        // Branch tips are independent overview entries: their parents are
        // cleared rather than drawn as false connections (Pink).
        ViewMode::BranchTips => (
            commits
                .iter()
                .filter(|commit| {
                    head == Some(commit.sha.as_ref())
                        || commit.refs.iter().any(|r| r.kind != RefKind::Tag)
                })
                .cloned()
                .map(|mut commit| {
                    commit.parents.clear();
                    commit
                })
                .collect(),
            HashMap::new(),
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowTransition {
    Stable,
    Entering,
    Exiting,
}

/// The temporary list that keeps every old row while the target rows take
/// their final order; old-only rows sit beside their previous anchor so they
/// fold in place (Pink `history_transition_rows`).
fn transition_rows(
    old: &[HistoryCommit],
    target: &[HistoryCommit],
) -> (Vec<HistoryCommit>, Vec<RowTransition>) {
    let target_shas: HashSet<&str> = target.iter().map(|c| c.sha.as_ref()).collect();
    let old_shas: HashSet<&str> = old.iter().map(|c| c.sha.as_ref()).collect();
    let mut before_first = Vec::new();
    let mut after_anchor: HashMap<SharedString, Vec<HistoryCommit>> = HashMap::new();
    let mut anchor_order: Vec<SharedString> = Vec::new();
    let mut anchor: Option<SharedString> = None;
    for commit in old {
        if target_shas.contains(commit.sha.as_ref()) {
            anchor = Some(commit.sha.clone());
        } else if let Some(anchor) = anchor.as_ref() {
            if !after_anchor.contains_key(anchor) {
                anchor_order.push(anchor.clone());
            }
            after_anchor
                .entry(anchor.clone())
                .or_default()
                .push(commit.clone());
        } else {
            before_first.push(commit.clone());
        }
    }
    let mut commits = Vec::with_capacity(target.len() + old.len());
    let mut rows = Vec::with_capacity(target.len() + old.len());
    for commit in before_first {
        commits.push(commit);
        rows.push(RowTransition::Exiting);
    }
    for commit in target {
        commits.push(commit.clone());
        rows.push(if old_shas.contains(commit.sha.as_ref()) {
            RowTransition::Stable
        } else {
            RowTransition::Entering
        });
        if let Some(exiting) = after_anchor.remove(&commit.sha) {
            for commit in exiting {
                commits.push(commit);
                rows.push(RowTransition::Exiting);
            }
        }
    }
    for anchor in anchor_order {
        if let Some(exiting) = after_anchor.remove(&anchor) {
            for commit in exiting {
                commits.push(commit);
                rows.push(RowTransition::Exiting);
            }
        }
    }
    (commits, rows)
}

// ---------------------------------------------------------------------------
// Graph geometry (Pink: `GraphGeometry`, `responsive_graph_geometry`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
struct GraphGeometry {
    lane_count: usize,
    width: f32,
    lane_spacing: f32,
    device_scale: f32,
    compact: bool,
}

impl GraphGeometry {
    fn natural(lane_count: usize) -> Self {
        let count = lane_count.max(1);
        Self {
            lane_count: count,
            width: GRAPH_SIDE_PADDING
                + GRAPH_TRAILING_PADDING
                + NODE_RADIUS * 2.0
                + (count - 1) as f32 * LANE_SPACING,
            lane_spacing: LANE_SPACING,
            device_scale: 1.0,
            compact: false,
        }
    }

    fn fitted(lane_count: usize, width: f32) -> Self {
        let count = lane_count.max(1);
        let natural = Self::natural(count);
        if count == 1 || width >= natural.width {
            return natural;
        }
        let fixed = GRAPH_SIDE_PADDING + GRAPH_TRAILING_PADDING + NODE_RADIUS * 2.0;
        let width = width.clamp(fixed, natural.width);
        Self {
            lane_count: count,
            width,
            lane_spacing: (width - fixed) / (count - 1) as f32,
            device_scale: 1.0,
            compact: false,
        }
    }

    fn compact(lane_count: usize) -> Self {
        let mut geometry = Self::natural(1);
        geometry.lane_count = lane_count.max(1);
        geometry.lane_spacing = 0.0;
        geometry.compact = true;
        geometry
    }

    /// Lane centers snap to device pixels.
    fn lane_x(self, lane: usize) -> f32 {
        let x = GRAPH_SIDE_PADDING + NODE_RADIUS + lane as f32 * self.lane_spacing;
        (x * self.device_scale).round() / self.device_scale
    }
}

fn interpolate_geometry(from: GraphGeometry, to: GraphGeometry, progress: f32) -> GraphGeometry {
    let progress = progress.clamp(0.0, 1.0);
    let lerp = |a: f32, b: f32| a + (b - a) * progress;
    GraphGeometry {
        lane_count: to.lane_count,
        width: lerp(from.width, to.width),
        lane_spacing: lerp(from.lane_spacing, to.lane_spacing),
        device_scale: to.device_scale,
        // Lanes converge with the full topology; the rail takes over only
        // once every path shares its x, and expanding does the inverse.
        compact: if from.compact {
            progress <= 0.001
        } else {
            to.compact && progress >= 0.999
        },
    }
}

/// Subjects stay the primary content: the graph takes at most a third of the
/// pane and never the subject's minimum or the data columns.
fn responsive_geometry(lane_count: usize, width: f32, columns_width: f32) -> GraphGeometry {
    let natural = GraphGeometry::natural(lane_count);
    if natural.width <= GRAPH_MIN_COMPACT_WIDTH {
        return natural;
    }
    let content_budget = (width - columns_width - SUBJECT_MIN_WIDTH).max(GRAPH_MIN_COMPACT_WIDTH);
    let share_budget = (width * GRAPH_MAX_WIDTH_RATIO).max(GRAPH_MIN_COMPACT_WIDTH);
    GraphGeometry::fitted(
        lane_count,
        natural.width.min(content_budget.min(share_budget)),
    )
}

fn should_use_compact(
    target: GraphGeometry,
    previous: GraphGeometry,
    width: f32,
    columns_width: f32,
) -> bool {
    if target.lane_count <= 1 {
        return false;
    }
    let subject_width = width - columns_width - target.width;
    if previous.compact {
        subject_width < COMPACT_EXIT_SUBJECT_WIDTH
            || target.lane_spacing < COMPACT_EXIT_LANE_SPACING
    } else {
        subject_width < COMPACT_ENTER_SUBJECT_WIDTH
            || target.lane_spacing < COMPACT_ENTER_LANE_SPACING
    }
}

fn stabilized_geometry(
    target: GraphGeometry,
    previous: GraphGeometry,
    device_scale: f32,
    compact: bool,
) -> GraphGeometry {
    if compact {
        let mut geometry = GraphGeometry::compact(target.lane_count);
        geometry.device_scale = device_scale.max(1.0);
        return geometry;
    }
    let natural = GraphGeometry::natural(target.lane_count);
    let is_natural = (target.width - natural.width).abs() < f32::EPSILON;
    let snapped = if is_natural {
        natural.width
    } else {
        (target.width / GRAPH_RESIZE_STEP).floor() * GRAPH_RESIZE_STEP
    };
    let mut next = GraphGeometry::fitted(target.lane_count, snapped);
    next.device_scale = device_scale.max(1.0);
    if !is_natural
        && previous.lane_count == next.lane_count
        && (previous.width - next.width).abs() < GRAPH_RESIZE_STEP
    {
        // Small width oscillations keep the last geometry until a whole step
        // has accumulated.
        let mut stable = previous;
        stable.device_scale = next.device_scale;
        stable
    } else {
        next
    }
}

fn cubic(start: f32, c1: f32, c2: f32, end: f32, t: f32) -> f32 {
    let inv = 1.0 - t;
    inv.powi(3) * start + 3.0 * inv.powi(2) * t * c1 + 3.0 * inv * t.powi(2) * c2 + t.powi(3) * end
}

fn point_to_segment(px_: f32, py: f32, sx: f32, sy: f32, ex: f32, ey: f32) -> f32 {
    let dx = ex - sx;
    let dy = ey - sy;
    let len2 = dx * dx + dy * dy;
    if len2 <= f32::EPSILON {
        return ((px_ - sx).powi(2) + (py - sy).powi(2)).sqrt();
    }
    let t = (((px_ - sx) * dx + (py - sy) * dy) / len2).clamp(0.0, 1.0);
    ((px_ - (sx + t * dx)).powi(2) + (py - (sy + t * dy)).powi(2)).sqrt()
}

/// The package's stroke shape for a laid-out segment (Pink `lane_shape`).
fn lane_shape(segment: &GraphSegment) -> cr::LaneShape {
    match segment.shape {
        SegmentShape::Through if segment.from_lane == segment.to_lane => cr::LaneShape::Straight,
        SegmentShape::Through => cr::LaneShape::Through,
        SegmentShape::Incoming => cr::LaneShape::Incoming,
        SegmentShape::Outgoing => cr::LaneShape::Outgoing,
    }
}

/// Pointer distance to a painted segment, sampled along the package's
/// `lane_curve`, the curve it paints (Pink `segment_distance`).
fn segment_distance(segment: &GraphSegment, x: f32, y: f32, geometry: GraphGeometry) -> f32 {
    let shape = lane_shape(segment);
    let curve = cr::lane_curve(
        shape,
        geometry.lane_x(segment.from_lane),
        geometry.lane_x(segment.to_lane),
        ROW_HEIGHT,
    );
    let (start, c1, c2, end) = (curve.start, curve.control_1, curve.control_2, curve.end);
    if shape == cr::LaneShape::Straight {
        return point_to_segment(x, y, start.0, start.1, end.0, end.1);
    }
    const SAMPLES: usize = 10;
    let mut closest = f32::MAX;
    let (mut prev_x, mut prev_y) = start;
    for sample in 1..=SAMPLES {
        let t = sample as f32 / SAMPLES as f32;
        let cx_ = cubic(start.0, c1.0, c2.0, end.0, t);
        let cy = cubic(start.1, c1.1, c2.1, end.1, t);
        closest = closest.min(point_to_segment(x, y, prev_x, prev_y, cx_, cy));
        prev_x = cx_;
        prev_y = cy;
    }
    closest
}

/// The path (color id) under the pointer: the node first, then the nearest
/// segment within the hit radius (Pink `hovered_graph_path`).
fn hovered_graph_path(row: &GraphRow, x: f32, y: f32, geometry: GraphGeometry) -> Option<usize> {
    let node_x = geometry.lane_x(row.node_lane);
    let node_y = ROW_HEIGHT / 2.0;
    if ((x - node_x).powi(2) + (y - node_y).powi(2)).sqrt() <= GRAPH_HIT_RADIUS + NODE_RADIUS {
        return Some(row.node_color_id);
    }
    if geometry.compact && (x - geometry.lane_x(0)).abs() <= GRAPH_HIT_RADIUS {
        return Some(row.node_color_id);
    }
    row.segments
        .iter()
        .map(|segment| (segment.color_id, segment_distance(segment, x, y, geometry)))
        .filter(|(_, distance)| *distance <= GRAPH_HIT_RADIUS)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(color, _)| color)
}

/// Graph lanes run at 72% saturation (Pink `graph_color`).
fn graph_color(mut color: Hsla) -> Hsla {
    color.s *= GRAPH_SATURATION;
    color
}

/// Pink `graph_paths`' palette, cycled by color id.
fn graph_palette(t: &Pink) -> [Hsla; 6] {
    [
        graph_color(t.accent),
        graph_color(t.busy),
        graph_color(t.success),
        graph_color(t.warning),
        graph_color(t.danger),
        graph_color(t.text_muted),
    ]
}

// ---------------------------------------------------------------------------
// Refs, authors, dates (Pink: history.rs helpers)
// ---------------------------------------------------------------------------

fn estimated_ref_badge_width(reference: &CommitRef) -> f32 {
    // 10 px icon + 2 px gap + 10 px padding + the 11 px label.
    (22.0 + reference.label.chars().count() as f32 * 5.7).min(REF_BADGE_MAX_WIDTH)
}

fn estimated_ref_overflow_width(hidden: usize) -> f32 {
    format!("+{hidden}").chars().count() as f32 * 5.7
}

fn ref_area_width(commit_column_width: f32) -> f32 {
    let inner = (commit_column_width - 8.0).max(0.0);
    (inner * REF_AREA_RATIO).min((inner - SUBJECT_MIN_WIDTH - REF_GAP).max(0.0))
}

fn visible_ref_count(refs: &[CommitRef], available: f32) -> usize {
    let mut visible = 0;
    for count in 1..=refs.len() {
        let hidden = refs.len() - count;
        let items = count + usize::from(hidden > 0);
        let badges: f32 = refs[..count].iter().map(estimated_ref_badge_width).sum();
        let overflow = if hidden > 0 {
            estimated_ref_overflow_width(hidden)
        } else {
            0.0
        };
        let gaps = items.saturating_sub(1) as f32 * REF_GAP;
        if badges + overflow + gaps <= available {
            visible = count;
        }
    }
    visible
}

fn ref_color(reference: &CommitRef, t: &Pink) -> Hsla {
    match reference.kind {
        RefKind::Branch => t.accent,
        RefKind::Remote => t.busy,
        RefKind::Tag => t.warning,
    }
}

fn ref_icon(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Branch => ICON_BRANCH,
        RefKind::Remote => ICON_CLOUD,
        RefKind::Tag => ICON_TAG,
    }
}

fn ref_description(reference: &CommitRef) -> SharedString {
    let kind = match reference.kind {
        RefKind::Branch => "Branch",
        RefKind::Remote => "Remote branch",
        RefKind::Tag => "Tag",
    };
    format!("{kind}: {}", reference.label).into()
}

fn author_name(name: &str) -> SharedString {
    if name.trim().is_empty() {
        "Unknown".into()
    } else {
        name.to_string().into()
    }
}

/// Pink `history_author_initial`: the first visible character, uppercased.
fn author_initial(name: &str) -> SharedString {
    name.chars()
        .find(|c| !c.is_whitespace())
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".to_string())
        .into()
}

/// Pink `format_date` (`%b %-d, %Y`), from the date's `YYYY-MM-DD` prefix.
fn format_date(value: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let parts: Vec<&str> = value.get(..10).unwrap_or("").split('-').collect();
    let parsed = match parts.as_slice() {
        [y, m, d] if y.len() == 4 => {
            match (y.parse::<u32>(), m.parse::<usize>(), d.parse::<u32>()) {
                (Ok(y), Ok(m), Ok(d)) if (1..=12).contains(&m) && (1..=31).contains(&d) => {
                    Some(format!("{} {d}, {y}", MONTHS[m - 1]))
                }
                _ => None,
            }
        }
        _ => None,
    };
    parsed.unwrap_or_else(|| "—".to_string()) // joiners: allow a lone dash for an unknown date
}

// ---------------------------------------------------------------------------
// Columns (Pink: `GitHistoryColumns`, widths, order, resize)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
struct Widths {
    author: f32,
    date: f32,
    sha: f32,
}

impl Default for Widths {
    fn default() -> Self {
        Self {
            author: 88.0,
            date: 88.0,
            sha: 74.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Columns {
    author: bool,
    date: bool,
    sha: bool,
}

impl Default for Columns {
    fn default() -> Self {
        Self {
            author: true,
            date: true,
            sha: true,
        }
    }
}

impl Columns {
    fn shows(self, column: HistoryColumn) -> bool {
        match column {
            HistoryColumn::Author => self.author,
            HistoryColumn::Date => self.date,
            HistoryColumn::Sha => self.sha,
        }
    }
}

const DEFAULT_ORDER: [HistoryColumn; 3] = [
    HistoryColumn::Author,
    HistoryColumn::Date,
    HistoryColumn::Sha,
];

/// The commit column plus the three data columns, for resize anchors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DataColumn {
    Commit,
    Data(HistoryColumn),
}

fn column_label(column: HistoryColumn) -> &'static str {
    match column {
        HistoryColumn::Author => "Author",
        HistoryColumn::Date => "Date",
        HistoryColumn::Sha => "SHA",
    }
}

fn column_width(column: DataColumn, widths: Widths) -> f32 {
    match column {
        DataColumn::Commit => SUBJECT_MIN_WIDTH,
        DataColumn::Data(HistoryColumn::Author) => widths.author,
        DataColumn::Data(HistoryColumn::Date) => widths.date,
        DataColumn::Data(HistoryColumn::Sha) => widths.sha,
    }
}

fn column_limits(column: DataColumn) -> (f32, f32) {
    match column {
        DataColumn::Commit => (SUBJECT_MIN_WIDTH, f32::MAX),
        DataColumn::Data(HistoryColumn::Author) => (AUTHOR_MIN, AUTHOR_MAX),
        DataColumn::Data(HistoryColumn::Date) => (DATE_MIN, DATE_MAX),
        DataColumn::Data(HistoryColumn::Sha) => (SHA_MIN, SHA_MAX),
    }
}

fn set_column_width(widths: &mut Widths, column: DataColumn, width: f32) {
    match column {
        DataColumn::Commit => {}
        DataColumn::Data(HistoryColumn::Author) => widths.author = width,
        DataColumn::Data(HistoryColumn::Date) => widths.date = width,
        DataColumn::Data(HistoryColumn::Sha) => widths.sha = width,
    }
}

#[derive(Clone, Copy, Debug)]
struct ResizeAnchor {
    start_x: f32,
    left: DataColumn,
    right: DataColumn,
    left_width: f32,
    right_width: f32,
}

/// The commit divider resizes its right neighbour against the elastic commit
/// column; interior dividers keep their pair's total (Pink
/// `resized_history_column_widths`).
fn resized_widths(mut widths: Widths, anchor: ResizeAnchor, delta: f32) -> Widths {
    if anchor.left == DataColumn::Commit {
        let (min, max) = column_limits(anchor.right);
        set_column_width(
            &mut widths,
            anchor.right,
            (anchor.right_width - delta).clamp(min, max),
        );
    } else {
        let (left_min, left_max) = column_limits(anchor.left);
        let (right_min, right_max) = column_limits(anchor.right);
        let min_delta = (left_min - anchor.left_width).max(anchor.right_width - right_max);
        let max_delta = (left_max - anchor.left_width).min(anchor.right_width - right_min);
        let delta = delta.clamp(min_delta, max_delta);
        set_column_width(&mut widths, anchor.left, anchor.left_width + delta);
        set_column_width(&mut widths, anchor.right, anchor.right_width - delta);
    }
    widths
}

/// Where a dragged header lands among the visible columns (Pink
/// `history_column_drop_index`).
fn column_drop_index(x: f32, rendered: f32, columns: &[HistoryColumn], widths: Widths) -> usize {
    if columns.is_empty() || rendered <= 0.0 {
        return 0;
    }
    let desired: f32 = columns
        .iter()
        .map(|c| column_width(DataColumn::Data(*c), widths))
        .sum();
    let x = x.clamp(0.0, rendered) * desired / rendered;
    let mut cursor = 0.0;
    for (index, column) in columns.iter().enumerate() {
        let width = column_width(DataColumn::Data(*column), widths);
        if x < cursor + width / 2.0 {
            return index;
        }
        cursor += width;
    }
    columns.len() - 1
}

/// Pink `reordered_history_columns`: hidden columns keep their slots.
fn reordered_columns(
    order: &[HistoryColumn],
    dragged: HistoryColumn,
    target: HistoryColumn,
) -> Vec<HistoryColumn> {
    if dragged == target {
        return order.to_vec();
    }
    let (Some(from), Some(over)) = (
        order.iter().position(|c| *c == dragged),
        order.iter().position(|c| *c == target),
    ) else {
        return order.to_vec();
    };
    let mut columns = order.to_vec();
    columns.remove(from);
    let after_removal = columns
        .iter()
        .position(|c| *c == target)
        .unwrap_or(columns.len());
    let insertion = if from < over {
        after_removal + 1
    } else {
        after_removal
    };
    columns.insert(insertion.min(columns.len()), dragged);
    columns
}

/// The drag payload of a column divider; its ghost is empty.
struct ColumnResize;

struct ResizeGhost;

impl Render for ResizeGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// The drag payload of a column header.
#[derive(Clone)]
struct ColumnDrag {
    column: HistoryColumn,
    label: SharedString,
}

/// The header chip that follows the pointer (Pink `HistoryColumnGhost`).
struct ColumnGhost {
    label: SharedString,
}

impl Render for ColumnGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        div()
            .h(px(24.0))
            .min_w(px(64.0))
            .px(px(9.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.0))
            .border_1()
            .border_color(t.border_strong)
            .bg(t.surface_raised)
            .shadow(pink::shadow_md())
            .font_family(pink::SANS)
            .text_size(px(11.0))
            .text_color(t.text_muted)
            .opacity(0.9)
            .child(self.label.clone())
    }
}

#[derive(Clone, Copy, Debug)]
struct DragOver {
    from: usize,
    over: usize,
}

/// The history pane's own tooltip (Pink `HistoryRefTooltip`): radius 5,
/// `border_strong`, the tooltip plate with `shadow_md`, one mono 11 px line
/// per description in `text_muted`.
struct RefTooltip {
    lines: Vec<SharedString>,
}

impl Render for RefTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let longest = self
            .lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or_default();
        let width = (longest as f32 * 6.4 + 16.0).clamp(72.0, 360.0);
        div()
            .w(px(width))
            .px(px(8.0))
            .py(px(6.0))
            .flex()
            .flex_col()
            .gap(px(3.0))
            .rounded(px(5.0))
            .border_1()
            .border_color(t.border_strong)
            .bg(t.tooltip_surface())
            .shadow(pink::shadow_md())
            .text_size(px(11.0))
            .text_color(t.text_muted)
            .children(self.lines.iter().cloned().map(|line| {
                div()
                    .min_w_0()
                    .truncate()
                    .whitespace_nowrap()
                    .font_family(pink::MONO)
                    .child(line)
            }))
    }
}

fn ref_tooltip(lines: Vec<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_, cx| {
        cx.new(|_| RefTooltip {
            lines: lines.clone(),
        })
        .into()
    }
}

/// The author disc's tooltip (Pink `HistoryAuthorTooltip`).
struct AuthorTooltip {
    name: SharedString,
}

impl Render for AuthorTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let width = (self.name.chars().count() as f32 * 6.2 + 16.0).clamp(72.0, 260.0);
        pink::tooltip_card(&t)
            .w(px(width))
            .px(px(8.0))
            .py(px(5.0))
            .truncate()
            .whitespace_nowrap()
            .text_size(px(11.0))
            .text_color(t.text_muted)
            .child(self.name.clone())
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Collapsed,
    Expanded,
    Collapsing,
}

struct Transition {
    rows: Vec<RowTransition>,
    commits: Vec<HistoryCommit>,
    final_commits: Vec<HistoryCommit>,
    epoch: usize,
}

#[derive(Clone, Copy, Debug)]
struct GraphFocus {
    color_id: usize,
    amount: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Morph {
    from: GraphGeometry,
    to: GraphGeometry,
    epoch: usize,
}

/// A popup opened at a point, with Pink's exit fade (`Popup`).
#[derive(Default)]
struct Popup {
    at: Option<Point<Pixels>>,
    closing: Option<(Point<Pixels>, Instant)>,
    reap: Option<Task<()>>,
}

impl Popup {
    fn open(&mut self, at: Point<Pixels>) {
        self.at = Some(at);
        self.closing = None;
        self.reap = None;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Columns,
    Author,
}

struct HistoryState {
    focus_key: String,
    // Refreshed every render, so handlers act on the current props.
    commits: Rc<Vec<HistoryCommit>>,
    head: Option<SharedString>,

    mode: ViewMode,
    collapsed: HashSet<String>,
    query: String,
    view_epoch: usize,
    transition: Option<Transition>,
    transition_task: Option<Task<()>>,

    input: Entity<TextInput>,
    search: SearchMode,
    search_epoch: usize,
    idle_epoch: usize,
    idle_task: Option<Task<()>>,
    search_task: Option<Task<()>>,
    _input_events: Subscription,

    hovered_path: Option<usize>,
    hovered_graph_path: Option<usize>,
    hovered_row_path: Option<usize>,
    hover_active: bool,
    hover_clear_task: Option<Task<()>>,
    hover_suppressed: Rc<Cell<bool>>,

    columns: Columns,
    widths: Widths,
    order: Vec<HistoryColumn>,
    author_display: AuthorDisplay,
    resize: Option<ResizeAnchor>,
    column_drag: Option<DragOver>,
    column_menu: Popup,
    author_menu: Popup,

    copied: Option<SharedString>,
    copy_task: Option<Task<()>>,

    lane_capacity: usize,
    geometry: Rc<Cell<GraphGeometry>>,
    target: Rc<Cell<GraphGeometry>>,
    morph: Rc<Cell<Option<Morph>>>,
    scroll: ScrollHandle,
    /// The row list's origin, recorded by the graph canvas each frame, so
    /// the graph cells can hit-test the pointer against the painted lanes.
    origin: Rc<Cell<Point<Pixels>>>,
}

impl HistoryState {
    fn new(focus_key: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Search"));
        let input_events = cx.subscribe_in(&input, window, |s: &mut Self, _, event, _, cx| {
            s.on_input(event, cx)
        });
        Self {
            focus_key,
            commits: Rc::new(Vec::new()),
            head: None,
            mode: ViewMode::AllCommits,
            collapsed: HashSet::new(),
            query: String::new(),
            view_epoch: 0,
            transition: None,
            transition_task: None,
            input,
            search: SearchMode::Collapsed,
            search_epoch: 0,
            idle_epoch: 0,
            idle_task: None,
            search_task: None,
            _input_events: input_events,
            hovered_path: None,
            hovered_graph_path: None,
            hovered_row_path: None,
            hover_active: false,
            hover_clear_task: None,
            hover_suppressed: Rc::new(Cell::new(false)),
            columns: Columns::default(),
            widths: Widths::default(),
            order: DEFAULT_ORDER.to_vec(),
            author_display: AuthorDisplay::Avatar,
            resize: None,
            column_drag: None,
            column_menu: Popup::default(),
            author_menu: Popup::default(),
            copied: None,
            copy_task: None,
            lane_capacity: 1,
            geometry: Rc::new(Cell::new(GraphGeometry::natural(1))),
            target: Rc::new(Cell::new(GraphGeometry::natural(1))),
            morph: Rc::new(Cell::new(None)),
            scroll: ScrollHandle::new(),
            origin: Rc::new(Cell::new(point(px(0.0), px(0.0)))),
        }
    }

    fn search_active(&self) -> bool {
        !self.query.trim().is_empty()
    }

    fn final_view(&self) -> (Vec<HistoryCommit>, HashMap<String, usize>) {
        compute_view(
            &self.commits,
            self.head.as_deref(),
            self.mode,
            &self.collapsed,
            &self.query,
        )
    }

    /// The rows as they settle: a running fold's destination, else the view.
    fn settled_view(&self) -> Vec<HistoryCommit> {
        match &self.transition {
            Some(transition) => transition.final_commits.clone(),
            None => self.final_view().0,
        }
    }

    fn visible_columns(&self) -> Vec<HistoryColumn> {
        self.order
            .iter()
            .copied()
            .filter(|c| self.columns.shows(*c))
            .collect()
    }

    fn clear_hover(&mut self) {
        self.hovered_path = None;
        self.hovered_graph_path = None;
        self.hovered_row_path = None;
        self.hover_active = false;
        self.hover_clear_task = None;
    }

    /// Pink `apply_view_change`: a fold plays its rows in or out over
    /// `COLLAPSE`; mode switches and searches swap at once.
    fn apply_view_change(
        &mut self,
        old: Vec<HistoryCommit>,
        animate: bool,
        cx: &mut Context<Self>,
    ) {
        self.transition = None;
        self.transition_task = None;
        let (target, _) = self.final_view();
        let unchanged =
            old.len() == target.len() && old.iter().zip(&target).all(|(a, b)| a.sha == b.sha);
        if !(unchanged || !animate || motion::reduced_motion(cx)) {
            let (commits, rows) = transition_rows(&old, &target);
            let epoch = self.view_epoch;
            self.transition = Some(Transition {
                rows,
                commits,
                final_commits: target,
                epoch,
            });
            let duration = COLLAPSE.total().mul_f32(motion::speed_scale());
            self.transition_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;
                this.update(cx, |s, cx| {
                    if s.transition.as_ref().is_some_and(|t| t.epoch == epoch) {
                        s.transition = None;
                        cx.notify();
                    }
                })
                .ok();
            }));
        }
        self.clear_hover();
        cx.notify();
    }

    /// Pink `set_view_mode`: back to every commit also drops the folds.
    fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        let cleared = mode == ViewMode::AllCommits && !self.collapsed.is_empty();
        if self.mode == mode && !cleared {
            return;
        }
        let old = self.settled_view();
        self.collapsed.clear();
        self.mode = mode;
        self.view_epoch = self.view_epoch.wrapping_add(1);
        self.apply_view_change(old, false, cx);
    }

    fn toggle_branch_ref(&mut self, reference: &CommitRef, cx: &mut Context<Self>) {
        let Some(key) = branch_ref_key(reference) else {
            return;
        };
        let old = self.settled_view();
        self.mode = ViewMode::AllCommits;
        self.view_epoch = self.view_epoch.wrapping_add(1);
        if !self.collapsed.remove(&key) {
            self.collapsed.insert(key);
        }
        self.apply_view_change(old, true, cx);
    }

    /// Pink `set_search_query`: the loaded page filters on every keystroke
    /// and the list returns to the top.
    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        let query = query.trim_start().to_string();
        if self.query == query {
            return;
        }
        self.query = query;
        self.transition = None;
        self.transition_task = None;
        self.scroll.set_offset(point(px(0.0), px(0.0)));
        self.clear_hover();
        cx.notify();
    }

    fn on_input(&mut self, event: &InputEvent, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                let empty = text.is_empty();
                self.set_query(text.to_string(), cx);
                if empty {
                    self.schedule_idle_dismiss(cx);
                } else {
                    self.cancel_idle_dismiss();
                }
            }
            // Pink clears the field when it loses focus; Esc does the same.
            InputEvent::Escape | InputEvent::Blur => self.clear_search(cx),
            _ => {}
        }
    }

    fn cancel_idle_dismiss(&mut self) {
        self.idle_epoch = self.idle_epoch.wrapping_add(1);
        self.idle_task = None;
    }

    /// An empty, idle field folds back into its magnifier after 1.5 s.
    fn schedule_idle_dismiss(&mut self, cx: &mut Context<Self>) {
        self.cancel_idle_dismiss();
        if self.search != SearchMode::Expanded || !self.query.is_empty() {
            return;
        }
        let epoch = self.idle_epoch;
        self.idle_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_IDLE_DISMISS).await;
            this.update(cx, |s, cx| {
                if s.idle_epoch == epoch && s.search == SearchMode::Expanded && s.query.is_empty() {
                    s.begin_collapse(cx);
                }
            })
            .ok();
        }));
    }

    fn begin_collapse(&mut self, cx: &mut Context<Self>) {
        if self.search != SearchMode::Expanded || !self.query.is_empty() {
            return;
        }
        self.cancel_idle_dismiss();
        self.search = SearchMode::Collapsing;
        self.search_epoch = self.search_epoch.wrapping_add(1);
        let epoch = self.search_epoch;
        let duration = RESIZE.total().mul_f32(motion::speed_scale());
        self.search_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(duration).await;
            this.update(cx, |s, cx| {
                if s.search_epoch == epoch && s.search == SearchMode::Collapsing {
                    s.search = SearchMode::Collapsed;
                    s.search_task = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn expand_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_idle_dismiss();
        self.search_epoch = self.search_epoch.wrapping_add(1);
        self.search_task = None;
        self.search = SearchMode::Expanded;
        let input = self.input.clone();
        let this = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            TextInput::focus(&input, window, cx);
            this.update(cx, |s, cx| s.schedule_idle_dismiss(cx)).ok();
        });
        cx.notify();
    }

    fn clear_search(&mut self, cx: &mut Context<Self>) {
        if !self.input.read(cx).is_empty() {
            self.input.update(cx, |input, cx| input.set_text("", cx));
        }
        self.set_query(String::new(), cx);
        self.schedule_idle_dismiss(cx);
        cx.notify();
    }

    /// Pink `set_history_hover`: focusing a path eases in; releasing it
    /// eases out before the focus is dropped.
    fn set_history_hover(&mut self, path: Option<usize>, cx: &mut Context<Self>) {
        let reduced = motion::reduced_motion(cx);
        if let Some(path) = path {
            if self.hover_active && self.hovered_path == Some(path) {
                return;
            }
            self.hover_clear_task = None;
            self.hover_active = true;
            self.hovered_path = Some(path);
            motion::set_hover(&self.focus_key, true, reduced);
            cx.notify();
            return;
        }
        if !self.hover_active {
            return;
        }
        self.hover_active = false;
        motion::set_hover(&self.focus_key, false, reduced);
        if reduced {
            self.hovered_path = None;
            self.hover_clear_task = None;
        } else {
            let fading = self.hovered_path;
            let duration = HOVER_FADE.total().mul_f32(motion::speed_scale());
            self.hover_clear_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(duration).await;
                this.update(cx, |s, cx| {
                    if !s.hover_active && s.hovered_path == fading {
                        s.hovered_path = None;
                        s.hover_clear_task = None;
                        cx.notify();
                    }
                })
                .ok();
            }));
        }
        cx.notify();
    }

    fn set_graph_hover(&mut self, path: Option<usize>, cx: &mut Context<Self>) {
        self.hovered_graph_path = path;
        self.set_history_hover(path.or(self.hovered_row_path), cx);
    }

    fn set_row_hover(&mut self, path: Option<usize>, cx: &mut Context<Self>) {
        if path.is_some() && self.hover_suppressed.replace(false) {
            cx.notify();
        }
        self.hovered_row_path = path;
        self.set_history_hover(self.hovered_graph_path.or(path), cx);
    }

    fn graph_focus(&self) -> Option<GraphFocus> {
        if self.hover_suppressed.get() {
            return None;
        }
        self.hovered_path.map(|color_id| GraphFocus {
            color_id,
            amount: motion::hover_t(&self.focus_key),
        })
    }

    fn copy_sha(&mut self, sha: SharedString, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(sha.to_string()));
        self.copied = Some(sha);
        self.copy_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            this.update(cx, |s, cx| {
                s.copied = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn popup(&mut self, menu: Menu) -> &mut Popup {
        match menu {
            Menu::Columns => &mut self.column_menu,
            Menu::Author => &mut self.author_menu,
        }
    }

    fn open_menu(&mut self, menu: Menu, at: Point<Pixels>, cx: &mut Context<Self>) {
        let other = match menu {
            Menu::Columns => Menu::Author,
            Menu::Author => Menu::Columns,
        };
        self.close_menu(other, cx);
        self.popup(menu).open(at);
        cx.notify();
    }

    /// Starts the 120 ms exit; the card stays mounted until it ends.
    fn close_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        let Some(at) = self.popup(menu).at.take() else {
            return;
        };
        let since = Instant::now();
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        let reap = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                let popup = s.popup(menu);
                if popup.closing.is_some_and(|(_, s)| s == since) {
                    popup.closing = None;
                    cx.notify();
                }
            })
            .ok();
        });
        let popup = self.popup(menu);
        popup.closing = Some((at, since));
        popup.reap = Some(reap);
        cx.notify();
    }

    fn toggle_column(&mut self, column: HistoryColumn, cx: &mut Context<Self>) {
        match column {
            HistoryColumn::Author => self.columns.author = !self.columns.author,
            HistoryColumn::Date => self.columns.date = !self.columns.date,
            HistoryColumn::Sha => self.columns.sha = !self.columns.sha,
        }
        cx.notify();
    }

    fn reset_columns(&mut self, cx: &mut Context<Self>) {
        self.columns = Columns::default();
        self.widths = Widths::default();
        self.order = DEFAULT_ORDER.to_vec();
        cx.notify();
    }

    fn can_reset(&self) -> bool {
        self.columns != Columns::default()
            || self.widths != Widths::default()
            || self.order != DEFAULT_ORDER
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

/// Everything a row or header needs, shared by reference.
struct BodyCx {
    key: String,
    t: Pink,
    state: Entity<HistoryState>,
    visible: Vec<HistoryCommit>,
    transitions: Option<Vec<RowTransition>>,
    fold_progress: f32,
    graph: GraphLayout,
    lane_capacity: usize,
    mode: ViewMode,
    collapsed: HashSet<String>,
    collapsed_counts: HashMap<String, usize>,
    columns: Vec<HistoryColumn>,
    widths: Widths,
    author_display: AuthorDisplay,
    column_drag: Option<DragOver>,
    columns_menu_open: bool,
    copied: Option<SharedString>,
    focus: Option<GraphFocus>,
    has_load_more: bool,
    loading_more: bool,
    load_error: bool,
    empty_message: Option<(SharedString, bool)>,
    loading: bool,
    on_open: Option<OpenHandler>,
    on_copy: Option<CopyHandler>,
    on_load_more: Option<ActionHandler>,
}

impl RenderOnce for CommitHistory {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let focus_key = format!("{key}-graph-focus");
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| HistoryState::new(focus_key, window, cx),
        );

        let commits = Rc::new(self.commits);
        let head = self.head.clone();
        let dragging = cx.has_active_drag();
        state.update(cx, |s, _| {
            s.commits = commits.clone();
            s.head = head.clone();
            if !dragging {
                s.column_drag = None;
            }
        });
        // The search field's palette (Pink `ComposerInput` on the toolbar).
        let input = state.read(cx).input.clone();
        input.update(cx, |input, _| {
            input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
        });

        let s = state.read(cx);
        let (final_commits, counts) = s.final_view();
        let (visible, transitions) = match &s.transition {
            Some(transition) => (transition.commits.clone(), Some(transition.rows.clone())),
            None => (final_commits, None),
        };
        let fold_progress = s
            .transition
            .as_ref()
            .map(|transition| {
                motion::follow_from(
                    &format!("{key}-fold-{}", transition.epoch),
                    0.0,
                    1.0,
                    COLLAPSE,
                )
            })
            .unwrap_or(1.0);
        let searching = s.search_active();
        let graph = layout_graph(&visible, head.as_deref());
        // The graph column keeps the widest lane count seen, so a fold never
        // slides every subject sideways (Pink `update_graph_layout`).
        let loaded_lanes = layout_graph(&commits, head.as_deref()).max_lane_count;
        let lane_capacity = s.lane_capacity.max(graph.max_lane_count).max(loaded_lanes);
        let has_load_more = !searching && s.mode == ViewMode::AllCommits && self.has_more;
        let empty_message = visible.is_empty().then(|| {
            let message: SharedString = if let Some(error) = &self.error {
                error.clone()
            } else if searching {
                "No matching commits".into()
            } else if self.total == Some(0) {
                "No commits yet".into()
            } else if s.mode == ViewMode::BranchTips {
                "No branch tips found".into()
            } else {
                "No commits found".into()
            };
            (message, self.error.is_some())
        });
        let count = if searching {
            Some(visible.len())
        } else {
            self.total
        };
        let body = BodyCx {
            key: key.clone(),
            t: t.clone(),
            state: state.clone(),
            transitions,
            fold_progress,
            graph,
            lane_capacity,
            mode: s.mode,
            collapsed: s.collapsed.clone(),
            collapsed_counts: counts,
            columns: s.visible_columns(),
            widths: s.widths,
            author_display: s.author_display,
            column_drag: s.column_drag,
            columns_menu_open: s.column_menu.at.is_some(),
            copied: s.copied.clone(),
            focus: s.graph_focus(),
            has_load_more,
            loading_more: self.loading_more,
            load_error: self.error.is_some(),
            empty_message,
            loading: self.loading && commits.is_empty(),
            visible,
            on_open: self.on_open.clone(),
            on_copy: self.on_copy.clone(),
            on_load_more: self.on_load_more.clone(),
        };
        let column_menu = s
            .column_menu
            .at
            .map(|at| (at, None))
            .or(s.column_menu.closing.map(|(at, since)| (at, Some(since))));
        let author_menu = s
            .author_menu
            .at
            .map(|at| (at, None))
            .or(s.author_menu.closing.map(|(at, since)| (at, Some(since))));
        state.update(cx, |s, _| s.lane_capacity = lane_capacity);

        let toolbar = toolbar(
            &key,
            &t,
            &state,
            &self.branch,
            count,
            self.comparison.clone(),
            self.fetching,
            self.on_fetch.clone(),
            self.on_refresh.clone(),
            cx,
        );

        let reduced = motion::reduced_motion(cx);
        let menus = [
            column_menu.map(|(at, closing)| {
                popup_at(
                    format!("{key}-columns-menu"),
                    at,
                    closing,
                    reduced,
                    column_menu_card(&key, &t, &state, cx).into_any_element(),
                )
            }),
            author_menu.map(|(at, closing)| {
                popup_at(
                    format!("{key}-author-menu"),
                    at,
                    closing,
                    reduced,
                    author_menu_card(&key, &t, &state, cx).into_any_element(),
                )
            }),
        ];

        let error_strip = |message: SharedString, t: &Pink| {
            // Pink: the fetch / page error band above the list.
            let wash = t.danger.opacity(0.05);
            div()
                .h(px(28.0))
                .flex_none()
                .flex()
                .items_center()
                .px(px(8.0))
                .border_b_1()
                .border_color(t.danger.opacity(0.16))
                .bg(wash)
                .truncate()
                .text_size(px(11.0))
                .text_color(t.readable_on(t.danger_muted, wash, pink::TEXT_CONTRAST_TARGET))
                .child(message)
        };
        let has_rows = !body.visible.is_empty();

        let on_resize = {
            let state = state.clone();
            move |ev: &DragMoveEvent<ColumnResize>, _: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| {
                    let Some(anchor) = s.resize else { return };
                    let delta = f32::from(ev.event.position.x) - anchor.start_x;
                    let widths = resized_widths(s.widths, anchor, delta);
                    if widths != s.widths {
                        s.widths = widths;
                        cx.notify();
                    }
                })
            }
        };

        motion::keep_alive(window);
        // Pink: the right pane shares the shell's chrome surface; the pane is
        // shown here as a panel of its own.
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .overflow_hidden()
            .bg(t.chrome_surface())
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_drag_move(on_resize)
            .child(toolbar)
            .when_some(self.fetch_error.clone(), |el, error| {
                el.child(error_strip(format!("Fetch failed: {error}").into(), &t))
            })
            .when_some(self.error.clone().filter(|_| has_rows), |el, error| {
                el.child(error_strip(error, &t))
            })
            .child(
                container_query(move |size, window, cx| {
                    history_body(body, f32::from(size.width), window, cx)
                })
                .w_full()
                .flex_1()
                .min_h_0(),
            )
            .children(menus.into_iter().flatten())
    }
}

/// The toolbar: branch title, count and comparison, then search, Fetch all,
/// the branch-tips toggle and Refresh (Pink: `changes.rs`
/// `render_header_controls` for `DiffScope::History`).
#[allow(clippy::too_many_arguments)]
fn toolbar(
    key: &str,
    t: &Pink,
    state: &Entity<HistoryState>,
    branch: &SharedString,
    count: Option<usize>,
    comparison: Option<Comparison>,
    fetching: bool,
    on_fetch: Option<ActionHandler>,
    on_refresh: Option<ActionHandler>,
    cx: &App,
) -> gpui::Div {
    let s = state.read(cx);
    let mode = s.mode;
    let search = s.search;
    let search_epoch = s.search_epoch;
    let input = s.input.clone();

    // Pink: the History scope's title slot shows the branch in mono.
    let title = div()
        .min_w_0()
        .max_w(px(160.0))
        .truncate()
        .h(px(chrome::CONTROL_M))
        .px(px(8.0))
        .flex_shrink(1.0)
        .flex()
        .items_center()
        .font_family(pink::MONO)
        .text_size(px(11.5))
        .line_height(px(14.0))
        .text_color(t.text_dim)
        .child(branch.clone());

    // Pink `GitHistoryCount`: "N commits" and the ahead / behind pair.
    let comparison = comparison.filter(|c| c.ahead > 0 || c.behind > 0);
    let count_t = t.clone();
    let count_key = key.to_string();
    let count_view = div()
        .flex_1()
        .min_w_0()
        .overflow_hidden()
        .h(px(chrome::CONTROL_M))
        .flex()
        .items_center()
        .child(container_query(move |size, _, _| {
            let t = count_t;
            let fits = f32::from(size.width) >= COMPARISON_MIN_WIDTH;
            div()
                .size_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(10.0))
                .when_some(count, |el, count| {
                    el.child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .text_size(px(11.0))
                            .line_height(px(14.0))
                            .text_color(t.text_muted)
                            .child(format!(
                                "{count} commit{}",
                                if count == 1 { "" } else { "s" }
                            )),
                    )
                })
                .when_some(comparison.filter(|_| fits), |el, c| {
                    let tip: SharedString = format!(
                        "Compared with {}: {} ahead, {} behind",
                        c.base, c.ahead, c.behind
                    )
                    .into();
                    let word = |copy: String, color: Hsla| {
                        div()
                            .whitespace_nowrap()
                            .text_size(px(11.0))
                            .line_height(px(13.0))
                            .text_color(color)
                            .child(copy)
                    };
                    el.child(
                        div()
                            .id(SharedString::from(format!("{count_key}-comparison")))
                            .relative()
                            .top(px(1.0))
                            .flex_none()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .when(c.ahead > 0, |el| {
                                el.child(word(
                                    format!("{} ahead", c.ahead),
                                    t.readable_tone(t.accent, 0.88),
                                ))
                            })
                            .when(c.behind > 0, |el| {
                                el.child(word(
                                    format!("{} behind", c.behind),
                                    t.readable_tone(t.warning, 0.82),
                                ))
                            })
                            .tooltip(ref_tooltip(vec![tip]))
                            .tooltip_show_delay(layout::TOOLTIP_DELAY),
                    )
                })
        }));

    // Pink `GitHistorySearchControl`.
    let search_el: AnyElement = if search == SearchMode::Collapsed {
        let state = state.clone();
        let id = format!("{key}-search-trigger");
        icon_button(
            id.clone(),
            st::control_glyph(svg().path(ICON_SEARCH).flex_none(), t.text_muted).into_any_element(),
            &IconButtonStyle {
                size: st::CONTROL_SIZE,
                radius: st::CONTROL_RADIUS,
                wash: gpui::transparent_black(),
                enabled: true,
            },
        )
        .hover_wash(id, t.wash(0.14))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            state.update(cx, |s, cx| s.expand_search(window, cx));
        })
        .tooltip(ref_tooltip(vec!["Search commits".into()]))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .into_any_element()
    } else {
        let closing = search == SearchMode::Collapsing;
        let clear_state = state.clone();
        let close_id = format!("{key}-search-close");
        st::compact_field(
            SharedString::from(format!("{key}-search")),
            SEARCH_WIDTH,
            t.ink(0.035),
        )
        .child(st::compact_field_status(st::compact_field_glyph(
            svg().path(ICON_SEARCH),
            t.text_faint,
        )))
        .child(st::compact_field_input(
            div()
                .w_full()
                .text_size(px(11.0))
                .line_height(px(14.0))
                .text_color(t.text)
                .child(input),
        ))
        .child(
            st::compact_close(
                SharedString::from(close_id.clone()),
                gpui::transparent_black(),
            )
            .hover_wash(close_id, t.fill(FillTier::Tertiary))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                clear_state.update(cx, |s, cx| s.clear_search(cx));
            })
            .child(st::compact_close_glyph(
                svg().path(ICON_CLOSE),
                t.text_faint,
            )),
        )
        .with_animation(
            SharedString::from(format!(
                "{key}-search-morph-{search_epoch}-{}",
                if closing { "out" } else { "in" }
            )),
            RESIZE.animation(),
            move |el, progress| {
                let amount = if closing { 1.0 - progress } else { progress };
                el.w(px(24.0 + (SEARCH_WIDTH - 24.0) * amount))
                    .opacity(0.45 + 0.55 * amount)
            },
        )
        .into_any_element()
    };

    // Pink `GitHistoryFetchButton`.
    let fetch_id = format!("{key}-fetch-all");
    let fetch = st::labeled_control(
        SharedString::from(fetch_id.clone()),
        &st::LabeledControlStyle {
            wash: if fetching {
                t.wash(0.05)
            } else {
                gpui::transparent_black()
            },
            enabled: !fetching,
        },
    )
    .map(|el| {
        if fetching {
            el
        } else {
            let on_fetch = on_fetch.clone();
            el.hover_wash(fetch_id.clone(), t.wash(0.14))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    if let Some(f) = &on_fetch {
                        f(window, cx);
                    }
                })
        }
    })
    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
    .child(if fetching {
        loaders::mini_glyph_spinner(format!("{key}-fetch-spin"), 1.75, t.glyph).into_any_element()
    } else {
        st::control_glyph(svg().path(ICON_CLOUD).flex_none(), t.readable_muted(0.75))
            .into_any_element()
    })
    .child(st::control_label(
        if fetching { "Fetching…" } else { "Fetch all" },
        if fetching { t.text_faint } else { t.text_muted },
    ));

    // Pink `GitHistoryViewButton`: latched accent wash while showing tips.
    let tips = mode == ViewMode::BranchTips;
    let view_id = format!("{key}-view-toggle");
    let view_state = state.clone();
    let view = icon_button(
        view_id.clone(),
        st::control_glyph(
            svg().path(ICON_FOLD),
            if tips { t.accent } else { t.text_muted },
        )
        .into_any_element(),
        &IconButtonStyle {
            size: st::CONTROL_SIZE,
            radius: st::CONTROL_RADIUS,
            wash: if tips {
                t.accent.opacity(0.12)
            } else {
                gpui::transparent_black()
            },
            enabled: true,
        },
    )
    .when(!tips, |el| el.hover_wash(view_id, t.wash(0.14)))
    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
    .on_click(move |_, _, cx| {
        cx.stop_propagation();
        view_state.update(cx, |s, cx| {
            let next = if s.mode == ViewMode::BranchTips {
                ViewMode::AllCommits
            } else {
                ViewMode::BranchTips
            };
            s.set_view_mode(next, cx);
        });
    })
    .tooltip(ref_tooltip(vec![
        if tips {
            "Show all commits"
        } else {
            "Show branch tips"
        }
        .into(),
    ]))
    .tooltip_show_delay(layout::TOOLTIP_DELAY);

    // Pink `Changes::header_button`: a focusable 28 pt icon button.
    let ring = t.focus_ring();
    let refresh_id = format!("{key}-refresh");
    let refresh = div()
        .id(SharedString::from(refresh_id.clone()))
        .size(px(chrome::CONTROL_M))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(chrome::RADIUS_M))
        .cursor_pointer()
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .hover_wash(refresh_id, t.wash(0.14))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            if let Some(f) = &on_refresh {
                f(window, cx);
            }
        })
        .child(
            svg()
                .path(ICON_REFRESH)
                .size(px(chrome::ICON_M))
                .text_color(t.readable_muted(0.7)),
        )
        .tooltip(pink::text_tooltip("Refresh"))
        .tooltip_show_delay(layout::TOOLTIP_DELAY);

    // Pink `surface_chrome::toolbar`: 38 tall, 8 in, controls 4 apart.
    st::toolbar().child(title).child(count_view).child(
        div()
            .min_w_0()
            .flex_shrink(1.0)
            .flex()
            .items_center()
            .gap(px(chrome::CONTROL_GAP))
            .child(search_el)
            .child(fetch)
            .child(view)
            .child(refresh),
    )
}

/// Column header, graph and rows, laid out for the pane's width (Pink: the
/// `container_query` in `GitHistory::render`).
fn history_body(c: BodyCx, width: f32, window: &mut Window, cx: &mut App) -> AnyElement {
    let t = &c.t;
    let key = c.key.as_str();

    // Geometry: natural lanes when they fit, fitted when they don't, the
    // compact rail when even that crowds the subjects; a switch to or from
    // the rail morphs over `COLLAPSE`.
    let columns_width: f32 = c
        .columns
        .iter()
        .map(|col| column_width(DataColumn::Data(*col), c.widths))
        .sum();
    let (geometry, show_rows) = {
        let s = c.state.read(cx);
        let geometry_cell = s.geometry.clone();
        let target_cell = s.target.clone();
        let morph_cell = s.morph.clone();
        let suppressed = s.hover_suppressed.clone();
        let responsive = responsive_geometry(c.lane_capacity, width, columns_width);
        let previous = target_cell.get();
        let compact = should_use_compact(responsive, previous, width, columns_width);
        let target = stabilized_geometry(responsive, previous, window.scale_factor(), compact);
        let reduced = motion::reduced_motion(cx);
        let morph = match morph_cell.get() {
            Some(active) if active.to == target => Some(active),
            Some(active) if active.to.compact == target.compact => Some(Morph {
                to: target,
                ..active
            }),
            _ if target.compact != previous.compact && !reduced => {
                let epoch = morph_cell.get().map_or(0, |m| m.epoch.wrapping_add(1));
                let morph = Morph {
                    from: geometry_cell.get(),
                    to: target,
                    epoch,
                };
                suppressed.set(true);
                Some(morph)
            }
            _ => {
                if geometry_cell.get() != target {
                    suppressed.set(true);
                }
                None
            }
        };
        target_cell.set(target);
        let geometry = match morph {
            Some(morph) => {
                let progress = motion::follow_from(
                    &format!("{key}-morph-{}", morph.epoch),
                    0.0,
                    1.0,
                    COLLAPSE,
                );
                if progress >= 1.0 {
                    morph_cell.set(None);
                    morph.to
                } else {
                    morph_cell.set(Some(morph));
                    interpolate_geometry(morph.from, morph.to, progress)
                }
            }
            None => {
                morph_cell.set(None);
                target
            }
        };
        geometry_cell.set(geometry);
        (geometry, !c.visible.is_empty())
    };
    let focus = if c.state.read(cx).hover_suppressed.get() {
        None
    } else {
        c.focus
    };

    if c.loading {
        // Pink: the first page's loading state.
        return div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .child(loaders::gradient_spinner(3.0, t.glyph))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(t.text_faint)
                    .child("Loading history…"),
            )
            .into_any_element();
    }
    if !show_rows {
        let (message, is_error) = c
            .empty_message
            .clone()
            .unwrap_or_else(|| ("No commits found".into(), false));
        return div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .px(px(20.0))
            .text_size(px(12.0))
            .text_color(if is_error { t.warning } else { t.text_faint })
            .child(message)
            .into_any_element();
    }

    let header = column_header(&c, geometry);

    // Row offsets: folding rows shrink and grow with the fold's progress.
    let mut tops = Vec::with_capacity(c.visible.len());
    let mut y = 0.0;
    for ix in 0..c.visible.len() {
        let amount = match c.transitions.as_ref().and_then(|rows| rows.get(ix)) {
            Some(RowTransition::Entering) => c.fold_progress,
            Some(RowTransition::Exiting) => 1.0 - c.fold_progress,
            _ => 1.0,
        };
        let height = ROW_HEIGHT * amount;
        tops.push((y, height, amount));
        y += height;
    }

    let palette = graph_palette(t);
    let origin = c.state.read(cx).origin.clone();
    let rows: Vec<AnyElement> = c
        .visible
        .iter()
        .enumerate()
        .map(|(ix, commit)| {
            let (top, _, amount) = tops[ix];
            let transition = c
                .transitions
                .as_ref()
                .and_then(|rows| rows.get(ix).copied())
                .unwrap_or(RowTransition::Stable);
            history_row(
                &c, ix, commit, geometry, focus, &palette, top, transition, amount, &origin, cx,
            )
        })
        .collect();

    let paths = graph_paths(
        Rc::new(c.graph.rows.clone()),
        Rc::new(tops.iter().map(|(top, h, _)| (*top, *h)).collect()),
        geometry,
        focus,
        palette,
        t.bg,
        c.mode == ViewMode::AllCommits,
        origin,
    );

    let load_more = c.has_load_more.then(|| load_more_row(&c));
    let list = div()
        .id(SharedString::from(format!("{key}-list")))
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&c.state.read(cx).scroll)
        .child(
            div()
                .relative()
                .w_full()
                .flex()
                .flex_col()
                .child(paths)
                .children(rows)
                .children(load_more),
        );

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(header)
        .child(list)
        .into_any_element()
}

/// The 24 px column header: a graph-width spacer, Commit, the data columns
/// with their dividers, and the checklist button that shows on hover (Pink
/// `render_column_header_cell`, `column_resize_handle`).
fn column_header(c: &BodyCx, geometry: GraphGeometry) -> gpui::Stateful<gpui::Div> {
    let t = &c.t;
    let key = c.key.as_str();
    let state = &c.state;
    let mut previous = DataColumn::Commit;
    let mut cells = Vec::with_capacity(c.columns.len());
    for (index, column) in c.columns.iter().copied().enumerate() {
        let data = DataColumn::Data(column);
        let width = column_width(data, c.widths);
        let (min_width, _) = column_limits(data);
        let label = column_label(column);

        // The divider on the cell's left edge.
        let handle_key = format!("{key}-resize-{previous:?}-{column:?}");
        let left = previous;
        let down_state = state.clone();
        let up_state = state.clone();
        let hover = t.border_strong;
        let handle = cr::resize_handle(SharedString::from(handle_key.clone()), hover.opacity(0.0))
            .hover_fill(handle_key, hover.opacity(0.0), hover.opacity(0.7))
            .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                down_state.update(cx, |s, _| {
                    s.resize = Some(ResizeAnchor {
                        start_x: f32::from(ev.position.x),
                        left,
                        right: data,
                        left_width: column_width(left, s.widths),
                        right_width: column_width(data, s.widths),
                    });
                })
            })
            .on_drag(ColumnResize, |_, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| ResizeGhost)
            })
            .on_mouse_up(MouseButton::Left, move |ev: &MouseUpEvent, _, cx| {
                up_state.update(cx, |s, cx| {
                    if ev.click_count == 2 {
                        s.widths = Widths::default();
                        cx.notify();
                    }
                    s.resize = None;
                })
            });

        let indicator = c
            .column_drag
            .filter(|d| d.over == index && d.from != d.over)
            .map(|d| cr::drop_line(d.from < d.over, t.accent));
        let menu_state = state.clone();
        cells.push(
            cr::header_cell(
                SharedString::from(format!("{key}-header-{column:?}")),
                width,
                min_width,
                column == HistoryColumn::Author,
            )
            .when(column == HistoryColumn::Author, |el| {
                el.on_mouse_down(
                    MouseButton::Right,
                    move |ev: &MouseDownEvent, window, cx| {
                        window.prevent_default();
                        cx.stop_propagation();
                        let at = ev.position;
                        menu_state.update(cx, |s, cx| s.open_menu(Menu::Author, at, cx));
                    },
                )
            })
            .on_drag(
                ColumnDrag {
                    column,
                    label: label.into(),
                },
                |payload, _, _, cx| {
                    cx.stop_propagation();
                    let label = payload.label.clone();
                    cx.new(|_| ColumnGhost { label })
                },
            )
            .children(indicator)
            .child(label)
            // Painted last so the narrow divider wins over the drag.
            .child(handle),
        );
        previous = data;
    }

    let move_state = state.clone();
    let drop_state = state.clone();
    let optional = cr::cells()
        .id(SharedString::from(format!("{key}-optional-headers")))
        .on_drag_move::<ColumnDrag>(move |ev, _, cx| {
            let dragged = ev.drag(cx).column;
            let position = ev.event.position;
            let bounds = ev.bounds;
            move_state.update(cx, |s, cx| {
                let visible = s.visible_columns();
                let Some(from) = visible.iter().position(|c| *c == dragged) else {
                    return;
                };
                let over = column_drop_index(
                    f32::from(position.x - bounds.left()),
                    f32::from(bounds.size.width),
                    &visible,
                    s.widths,
                );
                if s.column_drag
                    .is_some_and(|d| d.from == from && d.over == over)
                {
                    return;
                }
                s.column_drag = Some(DragOver { from, over });
                cx.notify();
            })
        })
        .on_drop::<ColumnDrag>(move |payload, _, cx| {
            let dragged = payload.column;
            drop_state.update(cx, |s, cx| {
                let visible = s.visible_columns();
                let target = s
                    .column_drag
                    .and_then(|d| visible.get(d.over).copied())
                    .unwrap_or(dragged);
                s.order = reordered_columns(&s.order, dragged, target);
                s.column_drag = None;
                cx.notify();
            })
        })
        .children(cells);

    let button_id = format!("{key}-columns-button");
    let open_state = state.clone();
    let column_button = cr::columns_button(
        SharedString::from(button_id.clone()),
        format!("{key}-column-header"),
        c.columns_menu_open,
        gpui::transparent_black(),
    )
    .hover_wash(button_id, t.fill(FillTier::Tertiary))
    .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
        window.prevent_default();
        cx.stop_propagation();
        let at = ev.position;
        open_state.update(cx, |s, cx| s.open_menu(Menu::Columns, at, cx));
    })
    .child(cr::columns_glyph(svg().path(ICON_COLUMNS), t.text_muted));

    cr::column_header(
        SharedString::from(format!("{key}-column-header")),
        format!("{key}-column-header"),
        &cr::ColumnHeaderStyle {
            rule: t.hairline(0.06),
            text: t.text_faint,
        },
    )
    .child(div().w(px(geometry.width)).flex_none())
    .child(cr::commit_header("Commit"))
    .child(optional)
    .child(column_button)
}

/// Every lane, painted by the package in one stroked path per color under
/// the rows; with a focused path, a second pass draws it thicker over the
/// dimmed rest (Pink `graph_paths`).
#[allow(clippy::too_many_arguments)]
fn graph_paths(
    rows: Rc<Vec<GraphRow>>,
    tops: Rc<Vec<(f32, f32)>>,
    geometry: GraphGeometry,
    focus: Option<GraphFocus>,
    palette: [Hsla; 6],
    graph_bg: Hsla,
    show_compact_rail: bool,
    origin: Rc<Cell<Point<Pixels>>>,
) -> impl IntoElement {
    canvas(
        move |bounds, _, _| origin.set(bounds.origin),
        move |bounds, _, window, _| {
            // Dimmed strokes stay opaque: overlapping segments would
            // otherwise stack alpha into dark dots.
            let dimmed = focus.map(|f| {
                palette.map(|color| {
                    motion::mix(color, graph_bg, (1.0 - GRAPH_UNFOCUSED_OPACITY) * f.amount)
                })
            });
            let style = cr::LaneStyle {
                colours: &palette,
                dimmed: dimmed.as_ref().map_or(&[], |dimmed| dimmed.as_slice()),
                focus: focus.map(|f| cr::LaneFocus {
                    colour: f.color_id,
                    amount: f.amount,
                }),
            };
            let row_bounds = |index: usize| {
                tops.get(index).map(|&(top, height)| {
                    Bounds::new(
                        point(bounds.origin.x, bounds.origin.y + px(top)),
                        size(bounds.size.width, px(height)),
                    )
                })
            };
            if geometry.compact && show_compact_rail {
                let rail = rows.iter().enumerate().filter_map(|(index, row)| {
                    Some(cr::RailRow {
                        bounds: row_bounds(index)?,
                        next: row_bounds(index + 1),
                        colour: row.node_color_id,
                    })
                });
                cr::paint_rail(window, bounds, geometry.lane_x(0), rail, &style);
                return;
            }
            let lanes = rows.iter().enumerate().filter_map(|(index, row)| {
                let segments = row.segments.iter().map(|segment| cr::LaneSegment {
                    from_x: geometry.lane_x(segment.from_lane),
                    to_x: geometry.lane_x(segment.to_lane),
                    shape: lane_shape(segment),
                    colour: segment.color_id,
                });
                Some((row_bounds(index)?, segments))
            });
            cr::paint_lanes(window, bounds, lanes, &style);
        },
    )
    .absolute()
    .inset_0()
}

/// One 36 px commit row (Pink `render_row` + `graph_cell`).
#[allow(clippy::too_many_arguments)]
fn history_row(
    c: &BodyCx,
    ix: usize,
    commit: &HistoryCommit,
    geometry: GraphGeometry,
    focus: Option<GraphFocus>,
    palette: &[Hsla; 6],
    top: f32,
    transition: RowTransition,
    amount: f32,
    origin: &Rc<Cell<Point<Pixels>>>,
    cx: &mut App,
) -> AnyElement {
    let t = &c.t;
    let key = c.key.as_str();
    let state = &c.state;
    let Some(graph_row) = c.graph.rows.get(ix).cloned() else {
        return gpui::Empty.into_any_element();
    };
    let sha = commit.sha.clone();
    let row_key = format!("{key}-row-{sha}");

    // Path focus: the focused lane's rows keep full strength over a faint
    // wash; everything else dims to 60%.
    let row_focused = focus.is_some_and(|f| f.color_id == graph_row.node_color_id);
    let content_opacity = focus
        .filter(|_| !row_focused)
        .map(|f| 1.0 - (1.0 - ROW_UNFOCUSED_OPACITY) * f.amount)
        .unwrap_or(1.0);
    let focused_wash = focus
        .filter(|_| row_focused)
        .map(|f| t.ink(0.018 * f.amount));

    // -- Graph cell -------------------------------------------------------
    let mut color = palette[graph_row.node_color_id % palette.len()];
    let node_selected = row_focused;
    if let Some(f) = focus
        && !node_selected
    {
        color.a *= 1.0 - (1.0 - GRAPH_UNFOCUSED_OPACITY) * f.amount;
    }
    let node_radius = NODE_RADIUS
        + focus
            .filter(|_| node_selected)
            .map(|f| f.amount * 0.75)
            .unwrap_or_default();
    let node_x = geometry.lane_x(graph_row.node_lane);
    let group = format!("{key}-graph-tip-{ix}");

    // A branch tip's fold control (Pink: `graph_cell` `fold_control`).
    let fold_reference = (c.mode == ViewMode::AllCommits)
        .then(|| {
            commit
                .refs
                .iter()
                .find(|r| branch_ref_key(r).is_some())
                .cloned()
        })
        .flatten();
    let fold_control = fold_reference.map(|reference| {
        let ref_key = branch_ref_key(&reference).unwrap_or_default();
        let collapsed = c.collapsed.contains(&ref_key);
        let hidden = c
            .collapsed_counts
            .get(&ref_key)
            .copied()
            .unwrap_or_default();
        let tip: SharedString = if collapsed {
            format!(
                "Expand {}{}",
                reference.label,
                if hidden == 0 {
                    String::new()
                } else {
                    format!(" ({hidden} hidden)")
                }
            )
            .into()
        } else {
            format!("Collapse {}", reference.label).into()
        };
        let fold_state = state.clone();
        cr::fold_control(
            SharedString::from(format!("{key}-fold-{sha}")),
            group.clone(),
            node_x,
            collapsed,
            &cr::FoldStyle {
                edge: color.opacity(0.32),
                fill: t.bg.opacity(0.96),
            },
        )
        .on_mouse_down(MouseButton::Left, |_, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
        })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            fold_state.update(cx, |s, cx| s.toggle_branch_ref(&reference, cx));
        })
        .child(cr::fold_glyph(
            svg().path(if collapsed { ICON_EXPAND } else { ICON_FOLD }),
            t.readable_tone(color, 0.9),
        ))
        .tooltip(ref_tooltip(vec![tip]))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
    });
    let move_state = state.clone();
    let leave_state = state.clone();
    let hit_row = graph_row.clone();
    let hit_origin = origin.clone();
    let graph_cell = cr::graph_cell(
        SharedString::from(format!("{key}-graph-{sha}")),
        group,
        geometry.width,
    )
    .on_mouse_move(move |ev: &MouseMoveEvent, _, cx| {
        let o = hit_origin.get();
        let x = f32::from(ev.position.x - o.x);
        let y = f32::from(ev.position.y - o.y) - top;
        let path = hovered_graph_path(&hit_row, x, y, geometry);
        move_state.update(cx, |s, cx| {
            if s.hover_suppressed.replace(false) {
                cx.notify();
            }
            s.set_graph_hover(path, cx)
        });
    })
    .on_hover(move |hovered, _, cx| {
        if !*hovered {
            leave_state.update(cx, |s, cx| s.set_graph_hover(None, cx));
        }
    })
    .when(graph_row.is_head, |el| {
        // HEAD wears a ring 2 px outside its node.
        el.child(cr::head_ring(node_x, node_radius, color, t.bg))
    })
    .child(cr::graph_node(node_x, node_radius, color))
    .children(fold_control);

    // -- Commit column ----------------------------------------------------
    let subject: SharedString = if commit.subject.is_empty() {
        "(no subject)".into()
    } else {
        commit.subject.clone()
    };
    let refs = commit.refs.clone();
    let commit_t = t.clone();
    let refs_key = format!("{key}-{sha}");
    let commit_column = container_query(move |size, _, _| {
        let refs_width = ref_area_width(f32::from(size.width));
        let t = commit_t;
        cr::subject_lane(content_opacity)
            .child(cr::subject(subject, t.text))
            .when(!refs.is_empty(), |el| {
                el.child(ref_area(&refs_key, refs, refs_width, &t))
            })
    })
    .flex_1()
    .min_w(px(SUBJECT_MIN_WIDTH))
    .overflow_hidden();

    // -- Data columns -----------------------------------------------------
    let name = author_name(&commit.author);
    let cells: Vec<AnyElement> = c
        .columns
        .iter()
        .map(|column| match column {
            HistoryColumn::Author => author_cell(
                key,
                &sha,
                c.widths.author,
                c.author_display,
                &name,
                content_opacity,
                t,
            ),
            HistoryColumn::Date => cr::date_cell(
                c.widths.date,
                DATE_MIN,
                content_opacity,
                format_date(&commit.date),
                t.text_muted,
            )
            .into_any_element(),
            HistoryColumn::Sha => sha_cell(c, &sha, content_opacity),
        })
        .collect();

    let hover_state = state.clone();
    let hover_key = row_key.clone();
    let path = graph_row.node_color_id;
    let open = c.on_open.clone();
    let open_commit = commit.clone();
    let row = cr::row(
        SharedString::from(row_key.clone()),
        focused_wash.unwrap_or_else(|| {
            motion::hover_blend(
                &row_key,
                gpui::transparent_black(),
                t.fill(FillTier::Quaternary),
            )
        }),
    )
    .on_hover(move |hovered, window, cx| {
        motion::set_hover(&hover_key, *hovered, motion::reduced_motion(cx));
        window.refresh();
        hover_state.update(cx, |s, cx| s.set_row_hover(hovered.then_some(path), cx));
    })
    .on_click(move |_, window, cx| {
        if let Some(f) = &open {
            f(&open_commit, window, cx);
        }
    })
    .child(graph_cell)
    .child(commit_column)
    .child(cr::cells().children(cells));

    let _ = cx;
    match transition {
        // Folding rows shrink to nothing as they fade to 35% (Pink:
        // `COLLAPSE`, opacity 0.35 → 1).
        RowTransition::Entering | RowTransition::Exiting => div()
            .w_full()
            .flex_none()
            .overflow_hidden()
            .h(px(ROW_HEIGHT * amount))
            .opacity(0.35 + 0.65 * amount)
            .child(row)
            .into_any_element(),
        RowTransition::Stable => row.into_any_element(),
    }
}

/// The subject's ref pills, as many as fit, then "+N" (Pink
/// `render_ref_area`, `render_ref`).
fn ref_area(key: &str, refs: Vec<CommitRef>, available: f32, t: &Pink) -> gpui::Div {
    let shown = visible_ref_count(&refs, available);
    let hidden: Vec<SharedString> = refs.iter().skip(shown).map(ref_description).collect();
    let hidden_count = hidden.len();
    cr::ref_area(available)
        .children(
            refs.into_iter()
                .take(shown)
                .enumerate()
                .map(|(ix, reference)| {
                    let color = ref_color(&reference, t);
                    let description = ref_description(&reference);
                    cr::ref_badge(
                        SharedString::from(format!("{key}-ref-{ix}")),
                        reference.label,
                        svg().path(ref_icon(reference.kind)).flex_none(),
                        &cr::RefBadgeStyle {
                            fill: color.opacity(0.07),
                            text: t.readable_tone(color, 0.9),
                            glyph: t.readable_tone(color, 0.78),
                        },
                    )
                    .tooltip(ref_tooltip(vec![description]))
                    .tooltip_show_delay(layout::TOOLTIP_DELAY)
                }),
        )
        .when(hidden_count > 0, |el| {
            el.child(
                cr::ref_overflow(
                    SharedString::from(format!("{key}-ref-overflow")),
                    format!("+{hidden_count}"),
                    t.text_faint,
                )
                .tooltip(ref_tooltip(hidden))
                .tooltip_show_delay(layout::TOOLTIP_DELAY),
            )
        })
}

/// Author as a 20 px initial disc (or the name), Pink `render_author_cell`.
fn author_cell(
    key: &str,
    sha: &SharedString,
    width: f32,
    display: AuthorDisplay,
    name: &SharedString,
    opacity: f32,
    t: &Pink,
) -> AnyElement {
    match display {
        AuthorDisplay::Avatar => {
            let tooltip_name = name.clone();
            cr::author_avatar_cell(width, AUTHOR_MIN, opacity)
                .child(
                    cr::avatar(
                        SharedString::from(format!("{key}-author-{sha}")),
                        &cr::AvatarStyle {
                            edge: t.hairline(0.12),
                            fill: t.wash(0.08),
                        },
                    )
                    .child(cr::avatar_initial(
                        author_initial(name),
                        pink::SANS.into(),
                        t.text_faint,
                    ))
                    .tooltip(move |_, cx| {
                        let name = tooltip_name.clone();
                        cx.new(|_| AuthorTooltip { name }).into()
                    })
                    .tooltip_show_delay(layout::TOOLTIP_DELAY),
                )
                .into_any_element()
        }
        AuthorDisplay::Name => {
            cr::author_name_cell(width, AUTHOR_MIN, opacity, name.clone(), t.text_muted)
                .into_any_element()
        }
    }
}

/// The short SHA: copies on click and says "Copied" in the accent for 1.2 s
/// (Pink `render_sha_cell`).
fn sha_cell(c: &BodyCx, sha: &SharedString, opacity: f32) -> AnyElement {
    let t = &c.t;
    let copied = c.copied.as_ref() == Some(sha);
    let label: SharedString = if copied {
        "Copied".into()
    } else {
        sha.chars().take(7).collect::<String>().into()
    };
    let id = format!("{}-sha-{sha}", c.key);
    let state = c.state.clone();
    let on_copy = c.on_copy.clone();
    let full = sha.clone();
    cr::sha_cell(c.widths.sha, SHA_MIN, opacity)
        .child(
            cr::sha_pill(
                SharedString::from(id.clone()),
                label.clone(),
                pink::MONO.into(),
                gpui::transparent_black(),
                if copied { t.accent } else { t.text_muted },
            )
            .hover_wash(id, t.fill(FillTier::Tertiary))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                state.update(cx, |s, cx| s.copy_sha(full.clone(), cx));
                if let Some(f) = &on_copy {
                    f(&full, window, cx);
                }
            }),
        )
        .into_any_element()
}

/// The footer row: Load more, Loading… or Retry (Pink `render_row` past the
/// last commit).
fn load_more_row(c: &BodyCx) -> gpui::Div {
    let t = &c.t;
    let pending = c.loading_more;
    let label = if pending {
        "Loading…"
    } else if c.load_error {
        "Retry"
    } else {
        "Load more"
    };
    let id = format!("{}-load-more", c.key);
    let rest = HoverColors {
        bg: t.surface_raised.opacity(0.72),
        border: t.border.opacity(0.85),
        fg: if pending { t.text_faint } else { t.text_muted },
    };
    let hover = if pending {
        rest
    } else {
        HoverColors {
            bg: t.glass_hover(),
            border: t.border_strong.opacity(0.75),
            fg: t.text,
        }
    };
    let on_load_more = c.on_load_more.clone();
    let t_hover = motion::hover_t(&id);
    let button = cr::load_more(
        SharedString::from(id.clone()),
        label,
        svg().path(if c.load_error {
            ICON_REFRESH
        } else {
            ICON_DOWN
        }),
        &cr::LoadMoreStyle {
            fill: motion::mix(rest.bg, hover.bg, t_hover),
            border: motion::mix(rest.border, hover.border, t_hover),
            text: motion::mix(rest.fg, hover.fg, t_hover),
            glyph: t.text_faint,
            enabled: !pending,
        },
    )
    .on_hover(motion::hover_listener(id))
    .when(!pending, |el| {
        el.on_click(move |_, window, cx| {
            if let Some(f) = &on_load_more {
                f(window, cx);
            }
        })
    });
    cr::load_more_row().child(button)
}

/// A popup anchored at a point, dropping 6 pt in on `MENU_IN` and fading on
/// `MENU_OUT` (Pink `popover::menu_at`).
fn popup_at(
    id: String,
    at: Point<Pixels>,
    closing: Option<Instant>,
    reduced: bool,
    card: AnyElement,
) -> impl IntoElement {
    let inner = div().occlude().child(card);
    let layer = match closing {
        Some(since) => {
            let t = if reduced {
                1.0
            } else {
                motion::menu_exit_progress(since)
            };
            motion::menu_out(
                SharedString::from(format!("{id}-out")),
                t,
                inner.relative().child(div().absolute().inset_0().occlude()),
            )
            .into_any_element()
        }
        None => {
            motion::menu_in_from(SharedString::from(id), MENU_RISE_DOWN, inner).into_any_element()
        }
    };
    deferred(
        anchored()
            .position(at)
            .anchor(Anchor::TopLeft)
            .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
            .child(layer),
    )
    .priority(1)
}

/// One compact menu row with a 12 px check slot (Pink: the column menu's
/// `option`).
fn compact_menu_row(
    id: String,
    label: &'static str,
    checked: bool,
    t: &Pink,
) -> gpui::Stateful<gpui::Div> {
    pink::menu_row(id, false, t)
        .gap(px(0.0))
        .px(px(7.0))
        .py(px(4.0))
        .rounded(px(COLUMN_MENU_RADIUS - layout::CARD_INSET))
        .text_size(px(11.5))
        .child(div().flex_1().child(label))
        .child(
            div()
                .w(px(12.0))
                .flex_none()
                .flex()
                .justify_end()
                .when(checked, |el| {
                    el.child(
                        svg()
                            .path(ICON_CHECK)
                            .size(px(10.0))
                            .text_color(t.text_muted),
                    )
                }),
        )
}

/// Show or hide Author, Date and SHA; Reset once anything moved (Pink
/// `render_column_menu`).
fn column_menu_card(key: &str, t: &Pink, state: &Entity<HistoryState>, cx: &App) -> gpui::Div {
    let s = state.read(cx);
    let columns = s.columns;
    let can_reset = s.can_reset();
    let close_state = state.clone();
    let option = |label: &'static str, column: HistoryColumn, ix: usize| {
        let state = state.clone();
        compact_menu_row(
            format!("{key}-column-option-{ix}"),
            label,
            columns.shows(column),
            t,
        )
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            state.update(cx, |s, cx| s.toggle_column(column, cx));
        })
    };
    let reset_state = state.clone();
    pink::popover_card(t)
        .w(px(132.0))
        .rounded(px(COLUMN_MENU_RADIUS))
        .on_mouse_down_out(move |_, _, cx| {
            close_state.update(cx, |s, cx| s.close_menu(Menu::Columns, cx))
        })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(layout::MENU_GAP))
                .child(option("Author", HistoryColumn::Author, 0))
                .child(option("Date", HistoryColumn::Date, 1))
                .child(option("SHA", HistoryColumn::Sha, 2))
                .when(can_reset, |el| {
                    el.child(
                        div()
                            .h(px(1.0))
                            .mx(px(5.0))
                            .my(px(2.0))
                            .bg(t.hairline(0.08)),
                    )
                    .child(
                        pink::menu_row(format!("{key}-columns-reset"), false, t)
                            .px(px(7.0))
                            .py(px(4.0))
                            .rounded(px(COLUMN_MENU_RADIUS - layout::CARD_INSET))
                            .text_size(px(11.5))
                            .text_color(t.text_muted)
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                reset_state.update(cx, |s, cx| {
                                    s.reset_columns(cx);
                                    s.close_menu(Menu::Columns, cx);
                                });
                            })
                            .child("Reset"),
                    )
                }),
        )
}

/// Author as names instead of initials (Pink `render_author_menu`).
fn author_menu_card(key: &str, t: &Pink, state: &Entity<HistoryState>, cx: &App) -> gpui::Div {
    let show_name = state.read(cx).author_display == AuthorDisplay::Name;
    let close_state = state.clone();
    let toggle_state = state.clone();
    pink::popover_card(t)
        .w(px(116.0))
        .rounded(px(COLUMN_MENU_RADIUS))
        .on_mouse_down_out(move |_, _, cx| {
            close_state.update(cx, |s, cx| s.close_menu(Menu::Author, cx))
        })
        .child(
            compact_menu_row(format!("{key}-author-name"), "Name", show_name, t).on_click(
                move |_, _, cx| {
                    cx.stop_propagation();
                    toggle_state.update(cx, |s, cx| {
                        s.author_display = match s.author_display {
                            AuthorDisplay::Avatar => AuthorDisplay::Name,
                            AuthorDisplay::Name => AuthorDisplay::Avatar,
                        };
                        s.close_menu(Menu::Author, cx);
                    });
                },
            ),
        )
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// A 40-character hex id from a seed.
fn demo_sha(seed: &str) -> String {
    // splitmix64 over the seed's FNV-1a, so near-identical seeds diverge.
    let mix = |mut z: u64| {
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    };
    let base = loaders::fnv1a(seed.as_bytes());
    let (a, b, c) = (
        mix(base),
        mix(base.wrapping_add(0x9e37_79b9_7f4a_7c15)),
        mix(base.wrapping_add(0x3c6e_f372_fe94_f82a)),
    );
    format!("{a:016x}{b:016x}{c:016x}")[..40].to_string()
}

struct DemoCommit {
    id: &'static str,
    parents: &'static [&'static str],
    subject: &'static str,
    author: &'static str,
    date: &'static str,
    refs: &'static [(&'static str, RefKind)],
}

const fn dc(
    id: &'static str,
    parents: &'static [&'static str],
    subject: &'static str,
    author: &'static str,
    date: &'static str,
    refs: &'static [(&'static str, RefKind)],
) -> DemoCommit {
    DemoCommit {
        id,
        parents,
        subject,
        author,
        date,
        refs,
    }
}

const LENA: &str = "Lena Hartmann";
const SOFIA: &str = "Sofia Marquez";
const DEV: &str = "Dev Patel";
const MATEO: &str = "Mateo Ruiz";
const AIKO: &str = "Aiko Tanaka";

/// What a fetch brings in: someone else's commit on `origin/main`.
const FETCHED: DemoCommit = dc(
    "upstream",
    &["merge-482"],
    "Bump stripe-go to 76.4",
    AIKO,
    "2026-10-23",
    &[("origin/main", RefKind::Remote)],
);

/// The first page, newest first in topological order.
const PAGE: &[DemoCommit] = &[
    dc(
        "rounding",
        &["merge-482"],
        "Round invoice totals with banker's rounding",
        LENA,
        "2026-10-23",
        &[("main", RefKind::Branch)],
    ),
    dc(
        "tokio",
        &["time-apis"],
        "Upgrade tokio to 1.41",
        MATEO,
        "2026-10-22",
        &[("deps/tokio", RefKind::Branch)],
    ),
    dc(
        "time-apis",
        &["retry-jitter"],
        "Replace deprecated time APIs before the upgrade",
        MATEO,
        "2026-10-22",
        &[],
    ),
    dc(
        "merge-482",
        &["retry-jitter", "backfill"],
        "Merge pull request #482 from invoices/backfill",
        LENA,
        "2026-10-21",
        &[("origin/main", RefKind::Remote)],
    ),
    dc(
        "backfill",
        &["pdf-worker"],
        "Backfill invoice PDFs for archived accounts",
        SOFIA,
        "2026-10-21",
        &[
            ("invoices/backfill", RefKind::Branch),
            ("origin/invoices/backfill", RefKind::Remote),
        ],
    ),
    dc(
        "pdf-worker",
        &["merge-477"],
        "Render invoice PDFs off the request path",
        SOFIA,
        "2026-10-20",
        &[],
    ),
    dc(
        "retry-jitter",
        &["merge-477"],
        "Retry webhook deliveries with jittered backoff",
        DEV,
        "2026-10-20",
        &[],
    ),
    dc(
        "merge-477",
        &["coupon-table", "retry-cap"],
        "Merge pull request #477 from sync/backoff",
        LENA,
        "2026-10-18",
        &[("v4.2.0", RefKind::Tag)],
    ),
    dc(
        "retry-cap",
        &["latency-log"],
        "Cap sync retries at five attempts",
        DEV,
        "2026-10-17",
        &[("sync/backoff", RefKind::Branch)],
    ),
    dc(
        "latency-log",
        &["merge-471"],
        "Log provider latency for every sync attempt",
        DEV,
        "2026-10-16",
        &[],
    ),
    dc(
        "coupon-table",
        &["merge-471"],
        "Drop the legacy coupon table",
        AIKO,
        "2026-10-16",
        &[],
    ),
    dc(
        "merge-471",
        &["pg-pin", "expire-coupons"],
        "Merge pull request #471 from coupons/retire",
        LENA,
        "2026-10-14",
        &[],
    ),
    dc(
        "expire-coupons",
        &["coupon-index"],
        "Retire expired coupon codes nightly",
        AIKO,
        "2026-10-13",
        &[],
    ),
    dc(
        "coupon-index",
        &["pg-pin"],
        "Index coupons by expiry date",
        AIKO,
        "2026-10-12",
        &[],
    ),
    dc(
        "pg-pin",
        &["tax-config"],
        "Pin the Postgres driver to 1.14",
        MATEO,
        "2026-10-10",
        &[("v4.1.3", RefKind::Tag)],
    ),
    dc(
        "tax-config",
        &["invoice-crate"],
        "Move tax rates into config",
        SOFIA,
        "2026-10-09",
        &[],
    ),
    dc(
        "invoice-crate",
        &["webhook-sig"],
        "Split the invoice service into its own crate",
        LENA,
        "2026-10-07",
        &[],
    ),
    dc(
        "webhook-sig",
        &["refund-events"],
        "Verify Stripe webhook signatures",
        DEV,
        "2026-10-06",
        &[],
    ),
];

/// The older page Load more brings in.
const OLDER: &[DemoCommit] = &[
    dc(
        "refund-events",
        &["ledger-merge"],
        "Emit refund events to the ledger",
        SOFIA,
        "2026-10-03",
        &[],
    ),
    dc(
        "ledger-merge",
        &["currency", "ledger-views"],
        "Merge pull request #452 from ledger/views",
        LENA,
        "2026-10-01",
        &[("v4.1.0", RefKind::Tag)],
    ),
    dc(
        "ledger-views",
        &["currency"],
        "Add monthly ledger views",
        MATEO,
        "2026-09-29",
        &[],
    ),
    dc(
        "currency",
        &["ledger-schema"],
        "Store amounts in minor currency units",
        AIKO,
        "2026-09-26",
        &[],
    ),
    dc(
        "ledger-schema",
        &[],
        "Initial ledger schema",
        LENA,
        "2026-09-22",
        &[],
    ),
];

fn demo_commit(d: &DemoCommit) -> HistoryCommit {
    HistoryCommit::new(demo_sha(d.id), d.subject)
        .parents(d.parents.iter().map(|p| demo_sha(p)))
        .author(d.author)
        .date(d.date)
        .refs(
            d.refs
                .iter()
                .map(|(label, kind)| CommitRef::new(*label, *kind)),
        )
}

/// Commits reachable from `head` (what `git rev-list --count HEAD` counts).
fn reachable(commits: &[HistoryCommit], head: &str) -> usize {
    let by_sha: HashMap<&str, &HistoryCommit> =
        commits.iter().map(|c| (c.sha.as_ref(), c)).collect();
    let mut seen = HashSet::new();
    let mut stack = vec![head.to_string()];
    while let Some(sha) = stack.pop() {
        if !seen.insert(sha.clone()) {
            continue;
        }
        if let Some(commit) = by_sha.get(sha.as_str()) {
            stack.extend(commit.parents.iter().map(|p| p.to_string()));
        }
    }
    seen.len()
}

/// The demo repository: loads, fetches, pages in older commits.
struct Demo {
    loaded: bool,
    fetching: bool,
    fetched: bool,
    older: bool,
    loading_more: bool,
    _script: Task<()>,
    fetch: Option<Task<()>>,
    page: Option<Task<()>>,
}

impl Demo {
    fn start_fetch(&mut self, cx: &mut Context<Self>) {
        if self.fetching {
            return;
        }
        self.fetching = true;
        self.fetch = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1_800))
                .await;
            this.update(cx, |d, cx| {
                d.fetching = false;
                d.fetched = true;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn load_older(&mut self, cx: &mut Context<Self>) {
        if self.loading_more || self.older {
            return;
        }
        self.loading_more = true;
        self.page = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(700))
                .await;
            this.update(cx, |d, cx| {
                d.loading_more = false;
                d.older = true;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo =
            window.use_keyed_state("commit-history-demo", cx, |_, cx: &mut Context<Demo>| {
                Demo {
                    loaded: false,
                    fetching: false,
                    fetched: false,
                    older: false,
                    loading_more: false,
                    // The first page lands, then a fetch from every remote runs.
                    _script: cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(900))
                            .await;
                        this.update(cx, |d, cx| {
                            d.loaded = true;
                            cx.notify();
                        })
                        .ok();
                        cx.background_executor()
                            .timer(Duration::from_millis(2_400))
                            .await;
                        this.update(cx, |d, cx| d.start_fetch(cx)).ok();
                    }),
                    fetch: None,
                    page: None,
                }
            });
        let (loaded, fetching, fetched, older, loading_more) = {
            let d = demo.read(cx);
            (d.loaded, d.fetching, d.fetched, d.older, d.loading_more)
        };

        let mut all: Vec<HistoryCommit> = Vec::new();
        all.push(demo_commit(&FETCHED));
        all.extend(PAGE.iter().map(demo_commit));
        all.extend(OLDER.iter().map(demo_commit));
        let head = demo_sha("rounding");
        let total = reachable(&all, &head);

        let mut commits: Vec<HistoryCommit> = Vec::new();
        if loaded {
            // The fetched commit replaces `origin/main` on the merge.
            let mut page: Vec<HistoryCommit> = PAGE.iter().map(demo_commit).collect();
            if fetched {
                for commit in &mut page {
                    commit.refs.retain(|r| r.label != "origin/main");
                }
                commits.push(demo_commit(&FETCHED));
            }
            commits.extend(page);
            if older {
                commits.extend(OLDER.iter().map(demo_commit));
            }
        }

        let fetch_demo = demo.clone();
        let more_demo = demo.clone();
        // The count and the comparison arrive with the first page.
        let history = CommitHistory::new("commit-history", "main")
            .width(660.0)
            .height(620.0)
            .head(head);
        let history = if loaded {
            history
                .total(total)
                .comparison(Comparison::new("origin/main", 1, usize::from(fetched)))
        } else {
            history
        };
        let history = history
            .loading(!loaded)
            .commits(commits)
            .has_more(!older)
            .loading_more(loading_more)
            .fetching(fetching)
            .on_fetch(move |_, cx| fetch_demo.update(cx, |d, cx| d.start_fetch(cx)))
            .on_load_more(move |_, cx| more_demo.update(cx, |d, cx| d.load_older(cx)));
        // History lives under Review (#176): the right panel's band names
        // Review as the shown destination, with History and Expand.
        let t = Pink::get(cx);
        let band = right_panel::band(
            "commit-history",
            right_panel::Destination::Review,
            [
                right_panel::action(
                    "commit-history-history",
                    right_panel::ICON_HISTORY,
                    "History",
                    &t,
                )
                .into_any_element(),
                right_panel::action(
                    "commit-history-expand",
                    right_panel::ICON_EXPAND,
                    "Expand",
                    &t,
                )
                .into_any_element(),
            ],
            &t,
        );
        div()
            .w(px(660.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(band)
            .child(history)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(sha: &str, parents: &[&str]) -> HistoryCommit {
        HistoryCommit::new(sha.to_string(), sha.to_string())
            .parents(parents.iter().map(|p| p.to_string()))
    }

    #[test]
    fn graph_splits_and_rejoins_merge_lanes() {
        let commits = vec![
            commit("merge", &["main", "feature"]),
            commit("main", &["base"]),
            commit("feature", &["base"]),
            commit("base", &[]),
        ];
        let graph = layout_graph(&commits, Some("merge"));
        assert_eq!(graph.rows.len(), commits.len());
        assert!(graph.max_lane_count >= 2);
        assert!(graph.rows[0].is_head);
        assert_eq!(graph.rows[0].segments.len(), 2);
        assert_eq!(graph.rows[3].segments.len(), 2);
        assert_eq!(graph.rows[3].node_lane, 0);
    }

    #[test]
    fn appending_older_commits_preserves_the_loaded_prefix_layout() {
        let prefix = vec![commit("tip", &["parent"]), commit("parent", &["root"])];
        let before = layout_graph(&prefix, Some("tip"));
        let mut all = prefix.clone();
        all.push(commit("root", &[]));
        let after = layout_graph(&all, Some("tip"));
        assert_eq!(before.rows, after.rows[..before.rows.len()]);
    }

    #[test]
    fn collapsing_a_branch_contracts_linear_parents_and_keeps_junctions() {
        let commits = vec![
            commit("feature", &["middle"]).reference(CommitRef::branch("feature")),
            commit("middle", &["base"]),
            commit("main", &["base"]).reference(CommitRef::branch("main")),
            commit("base", &["root"]),
            commit("root", &[]),
        ];
        let collapsed = HashSet::from(["local:feature".to_string()]);
        let (visible, counts) = collapse_branch_runs(&commits, &collapsed, Some("main"));
        let shas: Vec<&str> = visible.iter().map(|c| c.sha.as_ref()).collect();
        assert_eq!(shas, vec!["feature", "main", "base", "root"]);
        assert_eq!(visible[0].parents, vec![SharedString::from("base")]);
        assert_eq!(counts.get("local:feature"), Some(&1));
    }

    #[test]
    fn transition_rows_fold_old_commits_beside_their_stable_anchor() {
        let old = vec![
            commit("tip", &["one"]),
            commit("one", &["two"]),
            commit("two", &["base"]),
            commit("base", &[]),
        ];
        let target = vec![commit("tip", &["base"]), commit("base", &[])];
        let (rows, transitions) = transition_rows(&old, &target);
        let shas: Vec<&str> = rows.iter().map(|c| c.sha.as_ref()).collect();
        assert_eq!(shas, vec!["tip", "one", "two", "base"]);
        assert_eq!(
            transitions,
            vec![
                RowTransition::Stable,
                RowTransition::Exiting,
                RowTransition::Exiting,
                RowTransition::Stable,
            ]
        );
    }

    #[test]
    fn search_matches_fuzzy_subject_terms_and_sha_prefixes() {
        let c = HistoryCommit::new("3f9c2e1abc", "Retry webhook deliveries with jitter");
        assert!(history_matches("retry jitter", &c));
        assert!(history_matches("rtwh", &c));
        assert!(history_matches("3f9c", &c));
        assert!(!history_matches("9c2e", &c));
        assert!(!history_matches("refund", &c));
    }

    #[test]
    fn responsive_graph_compresses_lanes_and_then_compacts() {
        assert_eq!(
            responsive_geometry(8, 900.0, 240.0),
            GraphGeometry::natural(8)
        );
        let fitted = responsive_geometry(20, 400.0, 240.0);
        assert_eq!(fitted.width, 80.0);
        assert!(fitted.lane_spacing < LANE_SPACING);
        let narrow = responsive_geometry(10, 500.0, 240.0);
        assert!(should_use_compact(
            narrow,
            GraphGeometry::natural(10),
            500.0,
            240.0
        ));
        let wide = responsive_geometry(10, 570.0, 240.0);
        assert!(!should_use_compact(
            wide,
            GraphGeometry::compact(10),
            570.0,
            240.0
        ));
    }

    #[test]
    fn ref_badges_fit_the_available_width() {
        let refs = vec![
            CommitRef::branch("main"),
            CommitRef::tag("tag"),
            CommitRef::remote("origin"),
        ];
        assert_eq!(visible_ref_count(&[], 100.0), 0);
        assert_eq!(visible_ref_count(&refs, 70.0), 1);
        assert_eq!(visible_ref_count(&refs, 115.0), 2);
        assert_eq!(visible_ref_count(&refs, 200.0), 3);
        assert_eq!(ref_area_width(80.0), 0.0);
        assert!((ref_area_width(200.0) - 86.4).abs() < 0.001);
    }

    #[test]
    fn interior_dividers_keep_their_pair_total() {
        let anchor = ResizeAnchor {
            start_x: 0.0,
            left: DataColumn::Data(HistoryColumn::Author),
            right: DataColumn::Data(HistoryColumn::Date),
            left_width: 88.0,
            right_width: 88.0,
        };
        let widths = resized_widths(Widths::default(), anchor, 60.0);
        assert_eq!(widths.author + widths.date, 176.0);
        assert_eq!(widths.date, DATE_MIN);
    }

    #[test]
    fn dates_and_initials_follow_pink() {
        assert_eq!(format_date("2026-10-22"), "Oct 22, 2026");
        assert_eq!(format_date("2026-03-05T09:12:00Z"), "Mar 5, 2026");
        assert_eq!(format_date("soon"), "—"); // joiners: allow the unknown-date dash
        assert_eq!(author_initial("  josé"), SharedString::from("J"));
        assert_eq!(author_initial("   "), SharedString::from("?"));
        assert_eq!(author_name(""), SharedString::from("Unknown"));
    }

    #[test]
    fn branch_tips_list_heads_and_branches_without_parents() {
        let commits = vec![
            commit("tip", &["mid"]).reference(CommitRef::branch("main")),
            commit("mid", &["tag"]),
            commit("tag", &[]).reference(CommitRef::tag("v1")),
        ];
        let (tips, _) = compute_view(
            &commits,
            Some("mid"),
            ViewMode::BranchTips,
            &HashSet::new(),
            "",
        );
        let shas: Vec<&str> = tips.iter().map(|c| c.sha.as_ref()).collect();
        assert_eq!(shas, vec!["tip", "mid"]);
        assert!(tips.iter().all(|c| c.parents.is_empty()));
    }
}
