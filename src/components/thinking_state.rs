//! Thinking State – a Pink component.
//!
//! What an assistant is doing while it thinks, drawn the way Pink's
//! transcript draws it (Pink: `transcript.rs` `render_tool_group`,
//! `activity_rail`, `chip_header_row`, `ThoughtExpand`,
//! `render_working_trailer`). There is no card. Reasoning and the tool calls
//! between thoughts form one tool group on the 28 pt activity pitch, its
//! sentence in 14/20 drawn by the `pink-tool-activity` package with a light
//! sweeping its verb while it runs: what the calls did, verb first and
//! counted (`Read 3 files and searched once`, `Running psql`), `Thinking` or
//! `Reasoning` for a group of thoughts alone, and a red `1 failed` when the
//! sentence does not say it (UI rethink D63; the sentences are Tool Call
//! Card's, Pink `transcript/activity.rs`). Rows arrive one by one on a drawn
//! rail: a "Thought process" row opens onto its text while it streams, a
//! running tool shows the mini spinner in its glyph slot, and a failed one
//! reads `Failed to run` over its failure line (what it reported, then Copy
//! command). A thought past 24 lines shows its tail while it streams and its
//! head once settled, with `Show N more lines` to read the rest (#177), and
//! past eight settled calls the rest wait behind `Show N more` (D64). Under
//! the group the working line names the current step beside a breathing orb
//! and a timer, with a Stop that shows on hover. When the work is done the
//! line goes and the group folds shut; click the plate (or press Enter or
//! Space on it) to open it again. Pitch and text scale with the Interface
//! size.
//!
//! ```ignore
//! ThinkingState::new("plan", vec![
//!     ThinkingStep::thought("I need every writer of `orders.total` first.").done(),
//!     ThinkingStep::search("orders.total").done(),
//!     ThinkingStep::read("prisma/schema.prisma").running(),
//!     ThinkingStep::run("psql -c 'select count(*) from orders'"),
//! ])
//! .started_at(turn_started)
//! .on_stop(|_, cx| stop_turn(cx))
//! ```

use std::{
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, BoxShadow, ClipboardItem, Context, Div, ElementId, Entity,
    FillOptions, FillRule, Font, Hsla, IntoElement, PathBuilder, PathStyle, Pixels, Point, Role,
    SharedString, StyledText, Task, TextRun, Transformation, Window, canvas, div, img, point,
    prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    components::{
        file_badge as badge,
        tool_activity::{self as activity, ActivityState, ActivitySummary, Segment},
        tool_call_card::{self as calls, ToolCallCard, ToolCallStatus, Verb},
    },
    motion::{self, TOOL_CONNECTOR_REVEAL, TOOL_FOLD, TOOL_ROW_REVEAL},
    pink::{
        self, FillTier, Pink, icon_size,
        loaders::{
            self, LIVE_STATUS_OBJECT_CHARS, WORKING_STATUS_LINE_HEIGHT, WORKING_TIMER_SIZE,
            ellipsize_chars, mini_glyph_spinner, orb_slot, working_status, working_timer,
        },
        radius, space,
    },
    story,
};

pub const NAME: &str = "Thinking State";
pub const DESCRIPTION: &str = "An assistant's reasoning as it works: thoughts and the tool calls between them arriving one by one on a drawn rail under one verb-first sentence that shimmers while live, a streaming thought open to its text with a long one capped behind Show N more lines, a failed call's own words, the current step and a timer on the working line, folding to one quiet line when the work is done";

const ICON_BRAIN: &str = "icons/thinking-state/brain.svg";
const ICON_LAYERS: &str = "icons/thinking-state/layers-minimalistic.svg";
const ICON_CHEVRON: &str = "icons/thinking-state/alt-arrow-right.svg";
const ICON_MORE: &str = "icons/thinking-state/alt-arrow-down.svg";
const ICON_DANGER: &str = "icons/thinking-state/danger-circle.svg";
const ICON_READ: &str = "icons/thinking-state/file-text.svg";
const ICON_SEARCH: &str = "icons/thinking-state/file-search.svg";
const ICON_RUN: &str = "icons/thinking-state/terminal.svg";
const ICON_EDIT: &str = "icons/thinking-state/file-pen.svg";
const ICON_FETCH: &str = "icons/thinking-state/global.svg";
/// Pink: `file_icons` – the polychrome file-type artwork in a file badge's
/// well (vscode-symbols; dark variants lifted like Pink's `dark_icon_svg`).
const FILE_TS: &str = "icons/thinking-state/file-ts.svg";
const FILE_TS_DARK: &str = "icons/thinking-state/file-ts-dark.svg";
const FILE_PRISMA: &str = "icons/thinking-state/file-prisma.svg";
const FILE_DATABASE: &str = "icons/thinking-state/file-database.svg";
const FILE_DOCUMENT: &str = "icons/thinking-state/file-document.svg";
const FILE_DOCUMENT_DARK: &str = "icons/thinking-state/file-document-dark.svg";

/// Pink: `TOOL_GROUP_HEADER_HEIGHT`, `TOOL_TREE_ROW_HEIGHT` – the header and
/// every row share the 28 pt activity pitch (D63), scaled with the Interface
/// size; the header's plate is 1 pt in on each side. `CHIPS_TOP_PAD`.
const TOOL_GROUP_HEADER_HEIGHT: f32 = 28.;
const TOOL_TREE_ROW_HEIGHT: f32 = 28.;
const CHIPS_TOP_PAD: f32 = 2.;
/// Pink: `TOOL_LABEL_SIZE`, `TOOL_LABEL_LINE_HEIGHT` – the sentences' 14/20.
const TOOL_LABEL_SIZE: f32 = activity::TEXT_SIZE;
const TOOL_LABEL_LINE_HEIGHT: f32 = activity::LINE_HEIGHT;
/// Pink: `TOOL_TEXT_SIZE` – a thought's text stays 12 on its 18 pt rows.
const THOUGHT_TEXT_SIZE: f32 = loaders::TOOL_LABEL_SIZE;
/// Pink: `TOOL_FAILURE_LINE_HEIGHT`, `TOOL_FAILURE_TEXT_SIZE`.
const TOOL_FAILURE_LINE_HEIGHT: f32 = 24.;
const TOOL_FAILURE_TEXT_SIZE: f32 = 13.;
/// How long Copy command reads "Copied" (Pink `copy_ui_for`).
const COPIED_FOR: Duration = Duration::from_millis(1200);
/// Pink: the activity rail – a 48px gutter, the trunk at x 12.5, a 6px elbow
/// to x 28, the glyph at x 32, then 8px to the row text.
const ACTIVITY_GUTTER_WIDTH: f32 = 48.;
const ACTIVITY_TEXT_GAP: f32 = 8.;
const ACTIVITY_TRUNK_X: f32 = 12.5;
const ACTIVITY_BEND_RADIUS: f32 = 6.;
const ACTIVITY_BRANCH_END_X: f32 = 28.;
const ACTIVITY_ICON_LEFT: f32 = 32.;
const ACTIVITY_ICON_SIZE: f32 = icon_size::SM;
/// Pink: `DISCLOSURE_CHEVRON_SIZE` and `chip_header_row`'s 18px trailing
/// tile.
const DISCLOSURE_CHEVRON_SIZE: f32 = icon_size::XS;
const TRAIL_TILE: f32 = 18.;
/// Pink: `OUTPUT_LINE_HEIGHT`, `OUTPUT_BODY_PAD` (`py 6` × 2),
/// `DETAIL_SEPARATOR`, `OUTPUT_DETAIL_MAX_LINES` – a thought's text rows.
const OUTPUT_LINE_HEIGHT: f32 = 18.;
const OUTPUT_BODY_PAD: f32 = 12.;
const DETAIL_SEPARATOR: f32 = 1.;
const OUTPUT_DETAIL_MAX_LINES: usize = 24;
/// Pink: the open body settles 6px as it fades in.
const TOOL_BODY_RISE: f32 = 6.;
/// Pink: arriving row content rises 4px.
const ROW_CONTENT_RISE: f32 = 4.;

type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type StopHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// What a step is: a stretch of reasoning, or a tool call between thoughts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    /// Reasoning – a "Thought process" row that opens onto its text.
    Thought,
    /// Read a file (the detail is its path).
    Read,
    /// Searched the workspace (the detail is the pattern).
    Search,
    /// Ran a command.
    Run,
    /// Edited a file (the detail is its path).
    Edit,
    /// Fetched a page (the detail is its URL).
    Fetch,
}

impl StepKind {
    /// The row label (Pink: `tool_chip_content`; a thought reads "Thought
    /// process").
    pub fn label(self) -> &'static str {
        match self {
            Self::Thought => "Thought process",
            Self::Read => "Read",
            Self::Search => "Search",
            Self::Run => "Run",
            Self::Edit => "Edit",
            Self::Fetch => "Fetch",
        }
    }

    /// Pink: `tool_icon_path` / `activity_glyph`.
    fn glyph(self) -> &'static str {
        match self {
            Self::Thought => ICON_BRAIN,
            Self::Read => ICON_READ,
            Self::Search => ICON_SEARCH,
            Self::Run => ICON_RUN,
            Self::Edit => ICON_EDIT,
            Self::Fetch => ICON_FETCH,
        }
    }

    /// Pink: `tool_file_path` – the detail is a path, shown as a file badge.
    fn names_file(self) -> bool {
        matches!(self, Self::Read | Self::Edit)
    }
}

/// Where a step stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepStatus {
    /// Not started – hidden until it runs.
    Pending,
    Running,
    Done,
    /// Finished with an error: one red glyph and one red word.
    Failed,
}

/// One row of the trace.
#[derive(Clone)]
pub struct ThinkingStep {
    pub kind: StepKind,
    /// What a tool acted on: a path, pattern, command or URL. Empty for a
    /// thought.
    pub detail: SharedString,
    /// A thought's text, shown when its row is open. Paragraphs split on a
    /// blank line; `backticks` mark code.
    pub text: SharedString,
    pub status: StepStatus,
}

impl ThinkingStep {
    pub fn new(kind: StepKind, detail: impl Into<SharedString>) -> Self {
        Self {
            kind,
            detail: detail.into(),
            text: SharedString::default(),
            status: StepStatus::Pending,
        }
    }

    /// A stretch of reasoning with its text so far.
    pub fn thought(text: impl Into<SharedString>) -> Self {
        Self::new(StepKind::Thought, "").text(text)
    }

    pub fn read(path: impl Into<SharedString>) -> Self {
        Self::new(StepKind::Read, path)
    }

    pub fn search(pattern: impl Into<SharedString>) -> Self {
        Self::new(StepKind::Search, pattern)
    }

    pub fn run(command: impl Into<SharedString>) -> Self {
        Self::new(StepKind::Run, command)
    }

    pub fn edit(path: impl Into<SharedString>) -> Self {
        Self::new(StepKind::Edit, path)
    }

    pub fn fetch(url: impl Into<SharedString>) -> Self {
        Self::new(StepKind::Fetch, url)
    }

    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.text = text.into();
        self
    }

    pub fn status(mut self, status: StepStatus) -> Self {
        self.status = status;
        self
    }

    pub fn running(self) -> Self {
        self.status(StepStatus::Running)
    }

    pub fn done(self) -> Self {
        self.status(StepStatus::Done)
    }

    pub fn failed(self) -> Self {
        self.status(StepStatus::Failed)
    }

    fn settled(&self) -> bool {
        matches!(self.status, StepStatus::Done | StepStatus::Failed)
    }
}

/// One fold (the group's, or a thought row's), Pink: `FoldState` plus the
/// group's `rendered_open` / `rendered_height`.
#[derive(Clone, Copy, Default)]
struct Fold {
    /// The reader's pin; `None` follows the live rule.
    open: Option<bool>,
    /// The height when the fold last turned: the tween's start.
    from: f32,
    toggled_at: Option<Instant>,
    rendered_open: Option<bool>,
    rendered_height: f32,
}

impl Fold {
    /// Starts the tween when the open state turned since the last frame,
    /// whether the reader toggled it or the live rule did (Pink:
    /// `render_tool_group` `rendered_open`).
    fn settle(&mut self, open: bool, now: Instant) {
        if self.rendered_open.is_some_and(|previous| previous != open) {
            self.from = self.rendered_height;
            self.toggled_at = Some(now);
        }
        self.rendered_open = Some(open);
    }

    /// Time since the last turn, while motion is on.
    fn elapsed(&self, now: Instant, reduced: bool) -> Option<Duration> {
        self.toggled_at
            .filter(|_| !reduced)
            .map(|at| now.saturating_duration_since(at))
    }
}

struct State {
    /// When each step's row began arriving, by step index (Pink:
    /// `ToolGroupReveal::starts`). Present = seen; `None` = already there
    /// when the component mounted, so it does not animate.
    starts: HashMap<usize, Option<Instant>>,
    header_started_at: Option<Instant>,
    shimmer_started_at: Option<Instant>,
    mounted: bool,
    group: Fold,
    details: HashMap<usize, Fold>,
    /// Thoughts the reader opened past the 24-line cap (Pink
    /// `expanded_thoughts`).
    expanded: HashSet<usize>,
    /// `Show N more` was pressed.
    more_open: bool,
    /// Which failed step's Copy command reads "Copied", and since when.
    copied: Option<(usize, Instant)>,
    /// The working line's clock when the caller gives none.
    started: Instant,
    live: bool,
    _ticker: Task<()>,
}

impl State {
    fn new(cx: &mut Context<Self>) -> Self {
        // The timer ticks once a second even while the pulse clock is parked
        // (reduced motion).
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let alive = this.update(cx, |s, cx| {
                    if s.live {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Self {
            starts: HashMap::new(),
            header_started_at: None,
            shimmer_started_at: None,
            mounted: false,
            group: Fold::default(),
            details: HashMap::new(),
            expanded: HashSet::new(),
            more_open: false,
            copied: None,
            started: Instant::now(),
            live: true,
            _ticker: ticker,
        }
    }

    /// Records arrivals (Pink: the row sync in `transcript.rs`): a new group
    /// reveals its header and then its rows 90ms later, 65ms apart; rows
    /// joining a group already on screen start at once, 65ms apart. A trace
    /// that starts over begins fresh.
    fn sync(&mut self, visible: &[usize], live: bool, now: Instant) {
        if visible.len() < self.starts.len() {
            self.starts.clear();
            self.header_started_at = None;
            self.shimmer_started_at = None;
            self.group = Fold::default();
            self.details.clear();
            self.expanded.clear();
            self.more_open = false;
            self.copied = None;
            self.started = now;
        }
        let historical = !self.mounted;
        let new_group = self.starts.is_empty();
        if !visible.is_empty() {
            self.shimmer_started_at.get_or_insert(now);
            if new_group && !historical {
                self.header_started_at.get_or_insert(now);
            }
        }
        let mut arrival = 0;
        for &ix in visible {
            if self.starts.contains_key(&ix) {
                continue;
            }
            let start = (!historical).then(|| {
                let start = now + motion::tool_row_arrival_delay(arrival, new_group);
                arrival += 1;
                start
            });
            self.starts.insert(ix, start);
        }
        self.mounted = true;
        self.live = live;
    }
}

#[derive(IntoElement)]
pub struct ThinkingState {
    id: ElementId,
    steps: Vec<ThinkingStep>,
    started_at: Option<Instant>,
    auto_fold: bool,
    width: f32,
    on_toggle: Option<ToggleHandler>,
    on_stop: Option<StopHandler>,
}

impl ThinkingState {
    pub fn new(id: impl Into<ElementId>, steps: Vec<ThinkingStep>) -> Self {
        Self {
            id: id.into(),
            steps,
            started_at: None,
            auto_fold: true,
            width: 580.,
            on_toggle: None,
            on_stop: None,
        }
    }

    /// When the work began, for the working line's timer; defaults to the
    /// first render (and restarts when the trace starts over).
    pub fn started_at(mut self, at: Instant) -> Self {
        self.started_at = Some(at);
        self
    }

    /// Keep the group open after the work finishes.
    pub fn keep_open(mut self) -> Self {
        self.auto_fold = false;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Called with the new open state when the reader folds or unfolds the
    /// group.
    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Shows the working line's hover-only Stop while the work runs.
    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Copy: summary, working status, thought lines
// ---------------------------------------------------------------------------

/// A tool step as the call Tool Call Card names, so both stories read the
/// same sentences (Pink `activity::call_sentence`). Thoughts are not calls.
fn step_call(ix: usize, step: &ThinkingStep) -> Option<ToolCallCard> {
    let status = match step.status {
        StepStatus::Pending => return None,
        StepStatus::Running => ToolCallStatus::Running,
        StepStatus::Done => ToolCallStatus::Success,
        StepStatus::Failed => ToolCallStatus::Error,
    };
    let call = |verb| ToolCallCard::new(("step", ix), verb, status);
    let detail = step.detail.clone();
    Some(match step.kind {
        StepKind::Thought => return None,
        StepKind::Read => call(Verb::READ).file(detail),
        StepKind::Edit => call(Verb::EDIT).file(detail),
        StepKind::Search => call(Verb::SEARCH).words("for").code(detail),
        StepKind::Run => call(Verb::RUN).code(detail),
        StepKind::Fetch => call(Verb::FETCH).object(page(&detail).to_owned()),
    })
}

/// The group's sentence and header facts (Pink `tool_activity_summary`):
/// the calls' sentence, or `Thinking` / `Reasoning` for thoughts alone;
/// running while a call runs, thinking while only a thought streams.
fn group_sentence(steps: &[&ThinkingStep], live: bool) -> (calls::Sentence, ActivitySummary) {
    let cards: Vec<ToolCallCard> = steps
        .iter()
        .enumerate()
        .filter_map(|(ix, step)| step_call(ix, step))
        .collect();
    let running = steps
        .iter()
        .any(|step| step.kind != StepKind::Thought && step.status == StepStatus::Running);
    let state = if running {
        ActivityState::Running
    } else if live {
        ActivityState::Thinking
    } else {
        ActivityState::Recorded
    };
    if cards.is_empty() {
        let words = if live { "Thinking" } else { "Reasoning" };
        return (
            calls::Sentence::plain(words),
            ActivitySummary { state, failures: 0 },
        );
    }
    let group = calls::group_sentence(&cards);
    (
        group.sentence,
        ActivitySummary {
            state,
            failures: group.failures,
        },
    )
}

/// Pink: `tool_group_glyph` – the brain for a thought-only group, the tools'
/// own glyph when they share a kind, else layers.
fn group_glyph(steps: &[&ThinkingStep]) -> &'static str {
    let mut kinds = steps
        .iter()
        .filter(|s| s.kind != StepKind::Thought)
        .map(|s| s.kind);
    match kinds.next() {
        None => ICON_BRAIN,
        Some(first) if kinds.all(|kind| kind == first) => first.glyph(),
        Some(_) => ICON_LAYERS,
    }
}

/// Pink: `file_badge_name` – the last path component.
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|part| !part.is_empty())
        .unwrap_or(path)
}

/// Pink: `url_host`.
fn url_host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// Pink: `activity::page` – a page's readable address, host and path, no
/// scheme or trailing slash.
fn page(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.strip_suffix('/').unwrap_or(rest)
}

/// Pink: `live_status` / `tool_status` – the working line's verb and object:
/// the newest running step, else "Thinking" before anything shows and
/// "Working" between steps.
fn live_status(steps: &[ThinkingStep]) -> (&'static str, Option<String>) {
    let Some(step) = steps.iter().rev().find(|s| s.status == StepStatus::Running) else {
        let started = steps.iter().any(|s| s.status != StepStatus::Pending);
        return (if started { "Working" } else { "Thinking" }, None);
    };
    let detail = step.detail.as_ref();
    let (verb, object) = match step.kind {
        StepKind::Thought => ("Thinking", None),
        StepKind::Read => ("Reading", Some(file_name(detail))),
        StepKind::Search => ("Searching for", Some(detail)),
        StepKind::Run => ("Running", Some(detail)),
        StepKind::Edit => ("Editing", Some(file_name(detail))),
        StepKind::Fetch => ("Fetching", Some(url_host(detail))),
    };
    let object = object
        .map(|object| object.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|object| !object.is_empty())
        .map(|object| ellipsize_chars(&object, LIVE_STATUS_OBJECT_CHARS));
    (verb, object)
}

/// A stretch of one thought line: prose, or a `code` span.
struct ThoughtRun {
    text: String,
    code: bool,
}

type ThoughtLine = Vec<ThoughtRun>;

fn push_run(line: &mut ThoughtLine, text: &str, code: bool) {
    match line.last_mut() {
        Some(last) if last.code == code => last.text.push_str(text),
        _ => line.push(ThoughtRun {
            text: text.to_string(),
            code,
        }),
    }
}

/// Pink: `thought_cols_for_width` – columns of 12px text (Geist's average
/// advance ≈ 0.56em) that fit the text column.
fn thought_cols_for_width(text_width: f32, scale: f32) -> usize {
    let advance = THOUGHT_TEXT_SIZE * scale.max(0.5) * 0.56;
    ((text_width / advance).floor() as usize).clamp(24, 200)
}

/// Pink: `thought_lines` – the thought flattened into lines wrapped at `cols`
/// columns (one fixed-height row each, so heights stay analytic), with a
/// blank row between paragraphs.
fn thought_lines(text: &str, cols: usize) -> Vec<ThoughtLine> {
    let mut out: Vec<ThoughtLine> = Vec::new();
    for paragraph in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push(Vec::new());
        }
        let mut code = false;
        let mut line: ThoughtLine = Vec::new();
        let mut width = 0;
        for word in paragraph.split_whitespace() {
            let space_code = code;
            let mut parts: Vec<(String, bool)> = Vec::new();
            let mut current = String::new();
            for ch in word.chars() {
                if ch == '`' {
                    if !current.is_empty() {
                        parts.push((std::mem::take(&mut current), code));
                    }
                    code = !code;
                } else {
                    current.push(ch);
                }
            }
            if !current.is_empty() {
                parts.push((current, code));
            }
            let len: usize = parts.iter().map(|(p, _)| p.chars().count()).sum();
            if len == 0 {
                continue;
            }
            if width > 0 && width + 1 + len > cols {
                out.push(std::mem::take(&mut line));
                width = 0;
            }
            if width > 0 {
                push_run(&mut line, " ", space_code);
                width += 1;
            }
            for (part, part_code) in &parts {
                push_run(&mut line, part, *part_code);
            }
            width += len;
        }
        if !line.is_empty() {
            out.push(line);
        }
    }
    out
}

/// Pink: `cap_thought_lines` – at most 24 rows: the tail while the thought
/// streams, the head once it settles; returns the cut count.
fn cap_thought_lines(mut lines: Vec<ThoughtLine>, live: bool) -> (Vec<ThoughtLine>, usize) {
    let truncated_by = lines.len().saturating_sub(OUTPUT_DETAIL_MAX_LINES);
    if truncated_by > 0 {
        if live {
            lines.drain(..truncated_by);
            while lines.first().is_some_and(|line| line.is_empty()) {
                lines.remove(0);
            }
        } else {
            lines.truncate(OUTPUT_DETAIL_MAX_LINES);
        }
    }
    (lines, truncated_by)
}

/// Pink: `detail_height_for` (`ToolDetail::Thought`) – separator, rows (on
/// the Interface size's 18 pt rhythm) and the body's padding; a capped
/// thought adds its `Show N more lines` row.
fn thought_detail_height(lines: usize, truncated_by: usize, scale: f32) -> f32 {
    let rows = lines + usize::from(truncated_by > 0);
    DETAIL_SEPARATOR + rows as f32 * OUTPUT_LINE_HEIGHT * scale + OUTPUT_BODY_PAD
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------

/// Pink: `reveal_tool_row` – an arriving row grows to its height.
fn reveal_tool_row(row: AnyElement, height: f32, progress: f32) -> AnyElement {
    if progress >= 1. {
        return row;
    }
    div()
        .w_full()
        .h(px(height * progress))
        .flex_none()
        .overflow_hidden()
        .child(row)
        .into_any_element()
}

/// Pink: `activity_branch_points` – the elbow from the trunk to the branch
/// tip, cut to `progress` of its length.
fn branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<Point<f32>> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.;
            point(
                ACTIVITY_BEND_RADIUS * t * t,
                ACTIVITY_BEND_RADIUS * (2. * t - t * t),
            )
        })
        .collect();
    path.push(point(
        ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X,
        ACTIVITY_BEND_RADIUS,
    ));
    if progress >= 1. {
        return path;
    }
    let lengths: Vec<f32> = path
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .collect();
    let mut remaining = lengths.iter().sum::<f32>() * progress.clamp(0., 1.);
    let mut visible = vec![path[0]];
    for (pair, length) in path.windows(2).zip(lengths) {
        if remaining <= 0. {
            break;
        }
        let t = (remaining / length).min(1.);
        visible.push(point(
            motion::lerp(pair[0].x, pair[1].x, t),
            motion::lerp(pair[0].y, pair[1].y, t),
        ));
        remaining -= length;
    }
    visible
}

/// Pink: `activity_ribbon` – a 1px ribbon contour; nonzero fill unions the
/// trunk and the elbow so the fork paints one alpha.
fn ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
    if points.len() < 2 {
        return;
    }
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for (ix, p) in points.iter().enumerate() {
        let a = points[ix.saturating_sub(1)];
        let b = points[(ix + 1).min(points.len() - 1)];
        let dx = f32::from(b.x - a.x);
        let dy = f32::from(b.y - a.y);
        let length = dx.hypot(dy).max(0.0001);
        let normal = point(px(-dy / length * 0.5), px(dx / length * 0.5));
        left.push(*p + normal);
        right.push(*p - normal);
    }
    path.move_to(left[0]);
    for p in left.iter().skip(1).chain(right.iter().rev()) {
        path.line_to(*p);
    }
    path.close();
}

/// Pink: `activity_rail` – one row's 48px gutter: the trunk drawing down
/// from the row above (and on past an open row's text), the elbow to the
/// branch tip, and the row's glyph at x 32 fading in as the branch lands.
/// The ribbons are `hairline(0.12)`; `slot` is the glyph or the running
/// row's spinner.
fn activity_rail(
    slot: AnyElement,
    has_predecessor: bool,
    continues: bool,
    reveal: f32,
    continuation: f32,
    row_height: f32,
    t: &Pink,
) -> Div {
    let color = t.hairline(0.12);
    let (incoming, branch) = motion::tool_connector_parts(reveal, has_predecessor);
    div()
        .relative()
        .w(px(ACTIVITY_GUTTER_WIDTH))
        .flex_none()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let x = bounds.origin.x + px(ACTIVITY_TRUNK_X);
                    let bend_y = bounds.origin.y + px(row_height / 2. - ACTIVITY_BEND_RADIUS);
                    let mut tree = PathBuilder::fill().with_style(PathStyle::Fill(
                        FillOptions::default().with_fill_rule(FillRule::NonZero),
                    ));
                    if incoming > 0. {
                        let mut bottom = point(
                            x,
                            bounds.origin.y
                                + px((row_height / 2. - ACTIVITY_BEND_RADIUS) * incoming),
                        );
                        if incoming >= 1. && continues && continuation > 0. {
                            let below = (f32::from(bounds.size.height)
                                - (row_height / 2. - ACTIVITY_BEND_RADIUS))
                                .max(0.);
                            bottom = point(x, bend_y + px(below * continuation));
                        }
                        ribbon(&mut tree, &[point(x, bounds.origin.y), bottom]);
                    }
                    if branch > 0. {
                        let points: Vec<Point<Pixels>> = branch_points(branch)
                            .into_iter()
                            .map(|p| point(x + px(p.x), bend_y + px(p.y)))
                            .collect();
                        ribbon(&mut tree, &points);
                    }
                    if let Ok(path) = tree.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .absolute()
                .left(px(ACTIVITY_ICON_LEFT))
                .top(px(row_height / 2. - ACTIVITY_ICON_SIZE / 2.))
                .size(px(ACTIVITY_ICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .opacity(branch)
                .child(slot),
        )
}

/// Pink: `file_icons::asset_path` for the kinds this component names.
fn file_icon(path: &str, dark: bool) -> &'static str {
    let name = file_name(path).to_ascii_lowercase();
    let ext = name.rsplit_once('.').map_or("", |(_, ext)| ext);
    match ext {
        "ts" | "mts" | "cts" if dark => FILE_TS_DARK,
        "ts" | "mts" | "cts" => FILE_TS,
        "prisma" => FILE_PRISMA,
        "sql" => FILE_DATABASE,
        _ if dark => FILE_DOCUMENT_DARK,
        _ => FILE_DOCUMENT,
    }
}

/// Pink: `file_badges::render_file_badge` (no open route), drawn by the
/// `pink-file-badge` package as Pink does: a 22px chip on `ink(0.06)`, the
/// file-type artwork in a 20px well, then the file name at `text` 85%.
fn file_badge(path: &str, t: &Pink) -> AnyElement {
    // Pink: `file_icons::well_bg` – a counter-shade seat for the artwork.
    let well = pink::grey(if t.dark { 0 } else { 255 }).opacity(0.16);
    badge::badge(&badge::BadgeStyle {
        background: t.ink(0.06),
        text: t.text.opacity(0.85),
    })
    .child(badge::well(
        well,
        img(file_icon(path, t.dark))
            .flex_none()
            .size(px(badge::GLYPH)),
    ))
    .child(badge::name(file_name(path).to_owned()))
    .into_any_element()
}

const HEADER_GROUP: &str = "tool-header";

/// Pink: `chip_header_row` for a row on the rail – the call's verb-first
/// sentence (a thought reads "Thought process") drawn by the
/// `pink-tool-activity` package in `content_muted`, a file ending it as its
/// badge, and for an expandable row the trailing tile: a chevron at half
/// strength turning with `trail` (0 shut, 1 open). An expandable row's text
/// brightens to `text` under the pointer. A failure is the sentence's own
/// words ("Failed to run"), never a red word.
fn chip_header_row(
    step: &ThinkingStep,
    ix: usize,
    trail: Option<f32>,
    pitch: f32,
    t: &Pink,
) -> Div {
    let scale = pitch / TOOL_TREE_ROW_HEIGHT;
    let hover_text = trail.is_some();
    let text = t.text;
    let (mut segments, names_file) = match step_call(ix, step) {
        Some(call) => (call.sentence().segments, step.kind.names_file()),
        None => (vec![Segment::Text(step.kind.label().into())], false),
    };
    let badge = names_file.then(|| {
        segments.pop();
        file_badge(&step.detail, t)
    });
    let line = activity::sentence(&segments, &activity::sentence_style(t, scale))
        .text_color(t.content_muted);
    let line = if hover_text {
        line.id("tool-label")
            .group_hover(HEADER_GROUP, move |s| s.text_color(text))
            .into_any_element()
    } else {
        line.into_any_element()
    };
    div()
        .group(HEADER_GROUP)
        .h(px(pitch))
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::SM))
        .text_size(px(TOOL_LABEL_SIZE * scale))
        .line_height(px(TOOL_LABEL_LINE_HEIGHT * scale))
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(activity::SEGMENT_GAP * scale))
                .child(line)
                .children(badge),
        )
        .child(div().flex_1().min_w(px(space::XS)))
        .when_some(trail, |row, progress| {
            row.child(
                div()
                    .size(px(TRAIL_TILE))
                    .flex_none()
                    .opacity(0.5)
                    .group_hover(HEADER_GROUP, |s| s.opacity(1.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(ICON_CHEVRON)
                            .size(px(DISCLOSURE_CHEVRON_SIZE * scale))
                            .with_transformation(Transformation::rotate(radians(
                                FRAC_PI_2 * progress,
                            )))
                            .text_color(t.content_faint)
                            .group_hover(HEADER_GROUP, move |s| s.text_color(text)),
                    ),
            )
        })
}

/// The line under a failed call (D63, Pink `tool_failure_line`): what it
/// reported, then Copy command for a command, so it can be rerun by hand.
/// The rows here carry no detail body, so there is no Show details; nothing
/// retries or claims the failure resolved.
fn failure_line(
    state: &Entity<State>,
    ix: usize,
    step: &ThinkingStep,
    copied: bool,
    pitch: f32,
    t: &Pink,
) -> AnyElement {
    let scale = pitch / TOOL_TREE_ROW_HEIGHT;
    let spoken = step_call(ix, step)
        .map(|call| call.sentence().text)
        .unwrap_or_default();
    let message = step
        .text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map_or_else(
            || "The tool reported a failure without output".to_owned(),
            str::to_owned,
        );
    let mut actions = Vec::new();
    if step.kind == StepKind::Run {
        let ring = t.focus_ring();
        let id: SharedString = format!("thinking-failure-copy-{ix}").into();
        let command = step.detail.clone();
        let state = state.clone();
        actions.push(
            activity::recovery_action(
                id.clone(),
                if copied { "Copied" } else { "Copy command" }.into(),
                &activity::RecoveryStyle {
                    scale,
                    height: TOOL_LABEL_LINE_HEIGHT * scale,
                    radius: radius::CONTROL,
                    text: t.content_muted,
                },
            )
            .role(Role::Button)
            .aria_label(SharedString::from(format!("Copy command, {command}")))
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .hover_fill_fg(
                format!("tl-{id}"),
                t.fill(FillTier::Quaternary),
                t.fill(FillTier::Tertiary),
                t.content_muted,
                t.text,
            )
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                cx.write_to_clipboard(ClipboardItem::new_string(command.to_string()));
                state.update(cx, |s, cx| {
                    s.copied = Some((ix, Instant::now()));
                    cx.notify();
                });
            })
            .into_any_element(),
        );
    }
    let label = format!("{spoken}, {message}");
    activity::failure_line(
        message.into(),
        actions,
        &activity::FailureStyle {
            scale,
            height: TOOL_FAILURE_LINE_HEIGHT * scale,
            text_size: TOOL_FAILURE_TEXT_SIZE * scale,
            line_height: TOOL_LABEL_LINE_HEIGHT * scale,
            message: t.readable_tone(t.danger, 1.),
        },
    )
    .id(("thinking-failure", ix))
    .role(Role::Group)
    .aria_label(SharedString::from(label))
    .into_any_element()
}

/// Pink: `thought_line_text` – a line as shaped runs: faint prose, code in
/// the mono face.
fn thought_line_text(line: &ThoughtLine, color: Hsla) -> Option<(SharedString, Vec<TextRun>)> {
    let mut text = String::new();
    let mut runs = Vec::new();
    for run in line.iter().filter(|run| !run.text.is_empty()) {
        let font: Font = gpui::font(if run.code { pink::MONO } else { pink::SANS });
        runs.push(TextRun {
            len: run.text.len(),
            font,
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        });
        text.push_str(&run.text);
    }
    if text.trim().is_empty() {
        return None;
    }
    Some((text.into(), runs))
}

/// Pink: `detail_body` (`ToolDetail::Thought`) – 18 pt rows of 12 pt
/// `content_faint` text inside 6px of padding, scaled with the Interface
/// size. Past the 24-line cap comes Pink's `ThoughtExpand`: `Show N more
/// lines`, a focusable text action that shows the whole thought.
fn thought_body(
    lines: &[ThoughtLine],
    truncated_by: usize,
    expand: Option<(SharedString, Rc<dyn Fn(&mut App)>)>,
    scale: f32,
    t: &Pink,
) -> Div {
    let color = t.content_faint;
    let line_height = OUTPUT_LINE_HEIGHT * scale;
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .py(px(OUTPUT_BODY_PAD / 2.))
        .text_size(px(THOUGHT_TEXT_SIZE * scale))
        .children(lines.iter().map(|line| {
            let row = div()
                .h(px(line_height))
                .w_full()
                .min_w_0()
                .flex()
                .items_center();
            match thought_line_text(line, color) {
                Some((text, runs)) => row.child(
                    div()
                        .w_full()
                        .min_w_0()
                        .truncate()
                        .child(StyledText::new(text).with_runs(runs)),
                ),
                None => row,
            }
        }))
        .when(truncated_by > 0, |body| {
            let label: SharedString = match truncated_by {
                1 => "Show 1 more line".into(),
                n => format!("Show {n} more lines").into(),
            };
            let row = div().h(px(line_height)).flex().items_center();
            body.child(match expand {
                Some((id, on_expand)) => {
                    let ring = t.focus_ring();
                    row.child(
                        div()
                            .id(id.clone())
                            .role(Role::Button)
                            .aria_label(label.clone())
                            .tab_index(0)
                            .focus_visible(move |s| pink::focus_ring(s, ring))
                            .h_full()
                            .flex()
                            .items_center()
                            .px(px(4. * scale))
                            .ml(px(-4. * scale))
                            .rounded(px(radius::CONTROL))
                            .cursor_pointer()
                            .hover_fill_fg(
                                format!("tl-{id}"),
                                t.fill(FillTier::Tertiary).opacity(0.),
                                t.fill(FillTier::Tertiary),
                                t.content_muted,
                                t.text,
                            )
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                on_expand(cx);
                            })
                            .child(label),
                    )
                }
                // Pink: `more_lines_row`.
                None => row.text_color(color).child(match truncated_by {
                    1 => "… 1 more line".to_string(),
                    n => format!("… {n} more lines"),
                }),
            })
        })
}

/// A visible row's frame: its thought detail and fold, resolved before
/// painting so the group's height is analytic.
struct RowFrame {
    ix: usize,
    detail: Option<(Vec<ThoughtLine>, usize)>,
    open: bool,
    /// The failure line under a failed call, else zero.
    failure: f32,
    height: f32,
    chevron: f32,
    body_alpha: f32,
    body_mounted: bool,
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl RenderOnce for ThinkingState {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let reduced = motion::reduced_motion(cx);
        let key = self.id.to_string();
        let now = Instant::now();
        let ring = t.focus_ring();
        let scale = motion::speed_scale();
        let fold_span = TOOL_FOLD.total().mul_f32(scale);
        let fold_window = motion::FOLD_TWEEN_WINDOW.mul_f32(scale);
        // Pitch, sentences and thought rows follow the Interface size.
        let ui = pink::ui_scale(cx);
        let pitch = TOOL_TREE_ROW_HEIGHT * ui;
        let header_height = TOOL_GROUP_HEADER_HEIGHT * ui;
        let failure_height = TOOL_FAILURE_LINE_HEIGHT * ui;

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx: &mut Context<State>| State::new(cx),
        );

        let visible: Vec<usize> = self
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| s.status != StepStatus::Pending)
            .map(|(ix, _)| ix)
            .collect();
        let live = self.steps.is_empty() || self.steps.iter().any(|s| !s.settled());
        state.update(cx, |s, _| s.sync(&visible, live, now));

        // ---- arrival clocks (Pink: `tool_row_reveal_progress`,
        // `tool_connector_reveal_progress`) -------------------------------
        let (starts, header_start, shimmer_start, clock, more_open, copied) = {
            let s = state.read(cx);
            (
                visible
                    .iter()
                    .map(|ix| s.starts.get(ix).copied().flatten())
                    .collect::<Vec<_>>(),
                s.header_started_at,
                s.shimmer_started_at,
                s.started,
                s.more_open,
                s.copied
                    .filter(|(_, at)| now.saturating_duration_since(*at) < COPIED_FOR)
                    .map(|(ix, _)| ix),
            )
        };
        let reveals: Vec<f32> = starts
            .iter()
            .map(|start| motion::reveal_progress(TOOL_ROW_REVEAL, *start, now, reduced))
            .collect();
        let connectors: Vec<f32> = starts
            .iter()
            .map(|start| motion::reveal_progress(TOOL_CONNECTOR_REVEAL, *start, now, reduced))
            .collect();
        let header_reveal = motion::reveal_progress(TOOL_ROW_REVEAL, header_start, now, reduced);
        let arrival_pending = connectors.iter().any(|p| *p < 1.);
        // "Copied" reverts on its own.
        let mut motion_active = header_reveal < 1.
            || arrival_pending
            || reveals.iter().any(|p| *p < 1.)
            || copied.is_some();

        // ---- rows: thought details and their folds -----------------------
        let cols = thought_cols_for_width(
            (self.width - ACTIVITY_GUTTER_WIDTH - ACTIVITY_TEXT_GAP).max(120.),
            ui,
        );
        let rows: Vec<RowFrame> = state.update(cx, |s, _| {
            visible
                .iter()
                .map(|&ix| {
                    let step = &self.steps[ix];
                    // A failed call adds its failure line under the sentence.
                    let failure =
                        if step.status == StepStatus::Failed && step.kind != StepKind::Thought {
                            failure_height
                        } else {
                            0.
                        };
                    let detail = (step.kind == StepKind::Thought)
                        .then(|| {
                            let lines = thought_lines(&step.text, cols);
                            // The reader asked for all of it: no cap.
                            if s.expanded.contains(&ix) {
                                (lines, 0)
                            } else {
                                cap_thought_lines(lines, step.status == StepStatus::Running)
                            }
                        })
                        .filter(|(lines, _)| !lines.is_empty());
                    let Some((lines, truncated_by)) = detail else {
                        return RowFrame {
                            ix,
                            detail: None,
                            open: false,
                            failure,
                            height: pitch + failure,
                            chevron: 0.,
                            body_alpha: 1.,
                            body_mounted: false,
                        };
                    };
                    // A streaming thought is open (the live thinking is the
                    // point); a settled one is shut. The reader's toggle wins.
                    let fold = s.details.entry(ix).or_default();
                    let open = fold.open.unwrap_or(step.status == StepStatus::Running);
                    fold.settle(open, now);
                    let target = if open {
                        pitch + thought_detail_height(lines.len(), truncated_by, ui)
                    } else {
                        pitch
                    };
                    let elapsed = fold.elapsed(now, reduced);
                    let height = match elapsed {
                        Some(e) => motion::lerp(fold.from, target, TOOL_FOLD.progress_at(e)),
                        None => target,
                    };
                    fold.rendered_height = height;
                    if elapsed.is_some_and(|e| e < fold_span) {
                        motion_active = true;
                    }
                    // One clock drives the chevron, the height and the body
                    // fade, so the three agree mid-fold.
                    let animating = elapsed.filter(|e| *e < fold_window);
                    let (chevron, body_alpha) = motion::fold_progress(open, animating);
                    RowFrame {
                        ix,
                        detail: Some((lines, truncated_by)),
                        open,
                        failure: 0.,
                        height,
                        chevron,
                        body_alpha,
                        body_mounted: open || animating.is_some(),
                    }
                })
                .collect()
        });

        // ---- the row budget (D64): settled calls past eight wait behind
        // `Show N more`; thoughts, running and failed calls never hide -----
        let settled: Vec<bool> = rows
            .iter()
            .map(|row| {
                let step = &self.steps[row.ix];
                step.kind != StepKind::Thought && step.status == StepStatus::Done
            })
            .collect();
        let inspected: Vec<bool> = rows.iter().map(|row| row.open).collect();
        let budget = calls::history_budget(&settled, &inspected, more_open);
        // Positions into `rows`; `None` is the `Show N more` row, which
        // draws in with the first call it stands for.
        let mut items: Vec<Option<usize>> = Vec::new();
        for pos in 0..rows.len() {
            if budget.more_at == Some(pos) {
                items.push(None);
            }
            if budget.visible[pos] {
                items.push(Some(pos));
            }
        }
        let item_pos = |item: &Option<usize>| item.unwrap_or(budget.more_at.unwrap_or(0));

        // ---- the group's fold (Pink: `render_tool_group`) ------------------
        let auto_open = live || !self.auto_fold;
        let effective_auto_open = auto_open || arrival_pending;
        let revealed_height = CHIPS_TOP_PAD
            + items
                .iter()
                .map(|item| {
                    let height = item.map_or(pitch, |pos| rows[pos].height);
                    height * reveals[item_pos(item)]
                })
                .sum::<f32>();
        let (open, body_height, disclosure) = state.update(cx, |s, _| {
            let open = s.group.open.unwrap_or(effective_auto_open);
            s.group.settle(open, now);
            let target = if open { revealed_height } else { 0. };
            let elapsed = s.group.elapsed(now, reduced);
            if elapsed.is_some_and(|e| e < fold_span) {
                motion_active = true;
            }
            let (height, disclosure) = match elapsed {
                Some(e) => {
                    let p = TOOL_FOLD.progress_at(e);
                    (
                        motion::lerp(s.group.from, target, p),
                        if open { p } else { 1. - p },
                    )
                }
                None => (target, if open { 1. } else { 0. }),
            };
            s.group.rendered_height = height;
            (open, height, disclosure)
        });
        if motion_active {
            window.request_animation_frame();
        }
        let shimmer_phase = shimmer_start.filter(|_| live && !reduced).map(|start| {
            motion::pulse_keep(window, cx);
            loaders::shimmer_phase_since(start, now)
        });

        let shown: Vec<&ThinkingStep> = visible.iter().map(|ix| &self.steps[*ix]).collect();
        let group = (!shown.is_empty()).then(|| {
            let (sentence, summary) = group_sentence(&shown, live);
            let hover_key = format!("tl-{key}-hdr");
            let glyph_size = ACTIVITY_ICON_SIZE * ui;

            // The header (Pink: `render_tool_group` on the
            // `pink-tool-activity` header): a glyph on the rail's trunk
            // naming what is inside, the sentence with a light sweeping its
            // verb while the work runs, a failure count only when the
            // sentence does not say it, and a chevron that turns as the
            // group opens.
            let toggle = {
                let state = state.clone();
                let on_toggle = self.on_toggle.clone();
                move |window: &mut Window, cx: &mut App| {
                    state.update(cx, |s, cx| {
                        s.group.open = Some(!open);
                        cx.notify();
                    });
                    if let Some(f) = &on_toggle {
                        f(!open, window, cx);
                    }
                }
            };
            let header = div()
                .h(px(header_height))
                .flex()
                .flex_row()
                .items_center()
                .child(
                    activity::activity_header(
                        "tool-group-header",
                        calls::group_title(&sentence, shimmer_phase, ui, &t),
                        svg()
                            .path(group_glyph(&shown))
                            .flex_none()
                            .size(px(glyph_size))
                            .text_color(motion::hover_blend(&hover_key, t.content_muted, t.text))
                            .into_any_element(),
                        svg()
                            .path(ICON_CHEVRON)
                            .flex_none()
                            .size(px(DISCLOSURE_CHEVRON_SIZE * ui))
                            .with_transformation(Transformation::rotate(radians(
                                FRAC_PI_2 * disclosure,
                            )))
                            .text_color(t.content_faint)
                            .into_any_element(),
                        summary,
                        activity::HeaderStyle {
                            scale: ui,
                            height: header_height - 2. * ui,
                            radius: radius::CONTROL,
                            left_inset: ACTIVITY_TRUNK_X - glyph_size / 2.,
                            text_size: TOOL_LABEL_SIZE * ui,
                            line_height: TOOL_LABEL_LINE_HEIGHT * ui,
                            foreground: t.content_muted,
                            muted: t.content_faint,
                            active: t.text,
                            danger: t.readable_tone(t.danger, 1.),
                            warning: t.warning,
                        },
                    )
                    .focus_visible(move |s| pink::focus_ring(s, ring))
                    .role(Role::Button)
                    .aria_label(SharedString::from(summary.description(&sentence.text)))
                    .aria_expanded(open)
                    .tab_index(0)
                    .hover_fill_fg(
                        hover_key.clone(),
                        t.fill(FillTier::Quaternary),
                        t.fill(FillTier::Tertiary),
                        t.content_muted,
                        t.text,
                    )
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        toggle(window, cx);
                    }),
                );

            // ---- rows on the rail --------------------------------------
            let count = items.len();
            let chips = div().pt(px(CHIPS_TOP_PAD)).flex().flex_col().children(
                items.iter().enumerate().map(|(place, item)| {
                    let pos = item_pos(item);
                    let reveal = reveals[pos];
                    let connector = connectors[pos];
                    let content_reveal = motion::tool_connector_parts(connector, place > 0).1;
                    let continuation = motion::tool_connector_continuation(
                        items.get(place + 1).map(|next| connectors[item_pos(next)]),
                    );
                    let rise = |el: Div| {
                        el.when(content_reveal < 1., |el| {
                            el.relative()
                                .top(px(ROW_CONTENT_RISE * (1. - content_reveal)))
                                .opacity(content_reveal)
                        })
                    };
                    let rail = |slot: AnyElement| {
                        activity_rail(
                            slot,
                            place > 0,
                            place + 1 < count,
                            connector,
                            continuation,
                            pitch,
                            &t,
                        )
                    };

                    let Some(pos) = *item else {
                        // Pink: `tool_history_more_row`.
                        let action = calls::more_action(
                            format!("{key}-more").into(),
                            budget.hidden,
                            pitch,
                            &t,
                        )
                        .on_click({
                            let state = state.clone();
                            move |_, _, cx| {
                                cx.stop_propagation();
                                state.update(cx, |s, cx| {
                                    s.more_open = true;
                                    s.group.open = Some(true);
                                    cx.notify();
                                });
                            }
                        });
                        let el = div()
                            .h(px(pitch))
                            .w_full()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .child(rail(
                                svg()
                                    .path(ICON_MORE)
                                    .size(px(ACTIVITY_ICON_SIZE))
                                    .text_color(t.content_faint)
                                    .into_any_element(),
                            ))
                            .child(rise(
                                // The label's text, not its padding, sits on
                                // the sentences' column.
                                div()
                                    .ml(px(ACTIVITY_TEXT_GAP - 6. * ui))
                                    .h(px(pitch))
                                    .min_w_0()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .text_size(px(TOOL_LABEL_SIZE * ui))
                                    .line_height(px(TOOL_LABEL_LINE_HEIGHT * ui))
                                    .child(action),
                            ))
                            .into_any_element();
                        return reveal_tool_row(el, pitch, reveal);
                    };
                    let row = &rows[pos];
                    let step = &self.steps[row.ix];
                    // A running tool shows the item spinner in its glyph
                    // slot; a thought keeps its brain while it streams.
                    let slot =
                        if step.status == StepStatus::Running && step.kind != StepKind::Thought {
                            mini_glyph_spinner(format!("{key}-running-{}", row.ix), 2., t.glyph)
                                .into_any_element()
                        } else {
                            let failed = step.status == StepStatus::Failed;
                            svg()
                                .path(if failed {
                                    ICON_DANGER
                                } else {
                                    step.kind.glyph()
                                })
                                .size(px(ACTIVITY_ICON_SIZE))
                                .text_color(if failed { t.danger } else { t.content_muted })
                                .into_any_element()
                        };

                    let Some((lines, truncated_by)) = &row.detail else {
                        // Pink: `tool_chip` – a plain row on the pitch, its
                        // failure line under it.
                        let card = div()
                            .ml(px(ACTIVITY_TEXT_GAP))
                            .h(px(row.height))
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .child(
                                div()
                                    .h(px(pitch))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .child(chip_header_row(step, row.ix, None, pitch, &t)),
                            )
                            .when(row.failure > 0., |card| {
                                card.child(failure_line(
                                    &state,
                                    row.ix,
                                    step,
                                    copied == Some(row.ix),
                                    pitch,
                                    &t,
                                ))
                            });
                        let el = div()
                            .h(px(row.height))
                            .w_full()
                            .flex_none()
                            .flex()
                            .flex_row()
                            .child(rail(slot))
                            .child(rise(card))
                            .into_any_element();
                        return reveal_tool_row(el, row.height, reveal);
                    };

                    // Pink: the expandable chip – a header that toggles its
                    // body, the thought's text along the same column.
                    let ix = row.ix;
                    let row_open = row.open;
                    let toggle_detail = {
                        let state = state.clone();
                        move |cx: &mut App| {
                            state.update(cx, |s, cx| {
                                s.details.entry(ix).or_default().open = Some(!row_open);
                                cx.notify();
                            });
                        }
                    };
                    let expand: Rc<dyn Fn(&mut App)> = {
                        let state = state.clone();
                        Rc::new(move |cx: &mut App| {
                            state.update(cx, |s, cx| {
                                s.expanded.insert(ix);
                                // Reading on is an explicit choice to keep
                                // the thought and its group open.
                                s.details.entry(ix).or_default().open = Some(true);
                                s.group.open = Some(true);
                                cx.notify();
                            });
                        })
                    };
                    let mut card = div()
                        .ml(px(ACTIVITY_TEXT_GAP))
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .id(("thought", ix))
                                .role(Role::Button)
                                .aria_label("Thought process")
                                .aria_expanded(row.open)
                                .tab_index(0)
                                .rounded(px(radius::CONTROL))
                                .focus_visible(move |s| pink::focus_ring(s, ring))
                                .h(px(pitch))
                                .flex_none()
                                .flex()
                                .items_center()
                                .cursor_pointer()
                                .on_click(move |_, _, cx| {
                                    cx.stop_propagation();
                                    toggle_detail(cx);
                                })
                                .child(chip_header_row(step, ix, Some(row.chevron), pitch, &t)),
                        );
                    if row.body_mounted {
                        let alpha = row.body_alpha;
                        card = card.child(
                            div()
                                .flex_none()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .when(alpha < 1., |panel| {
                                    panel
                                        .relative()
                                        .top(px(TOOL_BODY_RISE * (1. - alpha)))
                                        .opacity(alpha)
                                })
                                .child(div().h(px(DETAIL_SEPARATOR)).flex_none())
                                .child(thought_body(
                                    lines,
                                    *truncated_by,
                                    Some((format!("{key}-thought-more-{ix}").into(), expand)),
                                    ui,
                                    &t,
                                )),
                        );
                    }
                    let card = card.h(px(row.height));
                    let el = div()
                        .w_full()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .child(rail(slot))
                        .child(rise(div().min_w_0().flex_1().child(card)))
                        .into_any_element();
                    reveal_tool_row(el, row.height, reveal)
                }),
            );

            div()
                .relative()
                .flex()
                .flex_col()
                .child(reveal_tool_row(
                    header.into_any_element(),
                    header_height,
                    header_reveal,
                ))
                .child(div().overflow_hidden().h(px(body_height)).child(chips))
        });

        // ---- the working line (Pink: `render_working_trailer`) -------------
        // One line, three tones: the verb shimmering in `content_muted`, the
        // object in `text`, the timer in `content_faint` with tabular digits;
        // Stop appears only under the pointer.
        let trailer = live.then(|| {
            let (verb, object) = live_status(&self.steps);
            let mut status = working_status(verb, t.content_muted, t.text);
            if let Some(object) = object {
                status = status.object(object);
            }
            let elapsed = now
                .saturating_duration_since(self.started_at.unwrap_or(clock))
                .as_secs() as i64;
            let hover_group: SharedString = format!("{key}-working").into();
            div()
                .id("working-trailer")
                .group(hover_group.clone())
                .flex()
                .flex_row()
                .items_start()
                .gap(px(space::SM))
                .when(group.is_some(), |el| el.pt(px(space::LG)))
                .min_w_0()
                .child(orb_slot(t.accent))
                .child(status)
                .child(working_timer(elapsed, t.content_faint))
                .when_some(self.on_stop.clone(), |el, on_stop| {
                    el.child(
                        div()
                            .id("working-stop")
                            .role(Role::Button)
                            .aria_label("Stop the current turn")
                            .tab_index(0)
                            .flex_none()
                            .ml(px(space::XS))
                            .px(px(space::S6))
                            .rounded(px(radius::CONTROL))
                            .cursor_pointer()
                            .text_size(px(WORKING_TIMER_SIZE))
                            .line_height(px(WORKING_STATUS_LINE_HEIGHT))
                            .text_color(t.content_muted)
                            .opacity(0.)
                            .group_hover(hover_group, |s| s.opacity(1.))
                            .focus_visible(|s| s.opacity(1.))
                            .hover_wash(format!("tl-{key}-stop"), t.fill(FillTier::Tertiary))
                            .child("Stop")
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_stop(window, cx);
                            }),
                    )
                })
        });

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .children(group)
            .children(trailer)
    }
}

// ---------------------------------------------------------------------------
// Story: an agent planning a schema migration, looping.
// ---------------------------------------------------------------------------

struct DemoStep {
    kind: StepKind,
    detail: &'static str,
    text: &'static str,
    from: f32,
    to: f32,
    /// Ends with an error: the red glyph and its failure line.
    fails: bool,
}

const DEMO: &[DemoStep] = &[
    DemoStep {
        kind: StepKind::Thought,
        detail: "",
        text: "The ask is to split `orders.total` into subtotal, tax and shipping without downtime. Before proposing anything I need every writer of that column, the table's size, and whether any raw SQL bypasses Prisma.",
        from: 0.4,
        to: 2.8,
        fails: false,
    },
    DemoStep {
        kind: StepKind::Search,
        detail: "orders.total",
        text: "",
        from: 2.8,
        to: 3.6,
        fails: false,
    },
    DemoStep {
        kind: StepKind::Read,
        detail: "prisma/schema.prisma",
        text: "",
        from: 3.6,
        to: 4.2,
        fails: false,
    },
    DemoStep {
        kind: StepKind::Read,
        detail: "src/checkout/checkout.ts",
        text: "",
        from: 4.2,
        to: 4.9,
        fails: false,
    },
    DemoStep {
        kind: StepKind::Read,
        detail: "migrations/0041_orders.sql",
        text: "",
        from: 4.9,
        to: 5.5,
        fails: false,
    },
    DemoStep {
        kind: StepKind::Thought,
        detail: "",
        text: "Two raw SQL writers update `orders.total` directly, so a rename would break them mid-deploy. The safe path is expand, backfill, dual-write, then contract, each step shippable and reversible on its own.\n\nExpand: add nullable `subtotal`, `tax` and `shipping` columns. Nullable columns without defaults are a catalog-only change in Postgres, so the `ALTER TABLE` takes a brief lock and no rewrite, even at 2.3M rows.\n\nDual-write: `checkout.ts` already computes all three parts, so it can write them beside `total` in the same statement. The raw writers in `0041_orders.sql` and the refunds job need the same change, or rows written during the backfill come out half filled.\n\nBackfill: batches of 5,000 ordered by `id` keep each transaction short and the replica lag under a second. Old rows have no tax breakdown, so they get `subtotal = total` with zero tax and shipping, which keeps every sum equal to the stored total.\n\nVerify: a check that compares `subtotal + tax + shipping` with `total` across the table must return no rows before anything reads the new columns, and again after the deploy that switches the readers.\n\nContract: after a full release on the new readers, drop the old writes, make the three columns `NOT NULL`, then drop `total`. Until that drop, `total` stays correct for every row, so each step keeps its rollback.",
        from: 5.5,
        to: 9.6,
        fails: false,
    },
    DemoStep {
        kind: StepKind::Run,
        detail: "psql -c \"select count(*) from orders\"",
        // What the failed command reported: its failure line shows this.
        text: "psql: error: connection to server on socket \"/tmp/.s.PGSQL.5432\" failed: No such file or directory",
        from: 9.6,
        to: 10.4,
        fails: true,
    },
    DemoStep {
        kind: StepKind::Run,
        detail: "psql \"$DATABASE_URL\" -c \"select count(*) from orders\"",
        text: "",
        from: 10.4,
        to: 11.4,
        fails: false,
    },
];

/// Seconds from the end of a run to the next one.
const DEMO_REST: f32 = 5.0;
const DEMO_WIDTH: f32 = 580.;
/// Tall enough for the open group with a capped thought, so the stage never
/// jumps.
const DEMO_HEIGHT: f32 = 780.;

fn demo_end() -> f32 {
    DEMO.last().map_or(0., |s| s.to)
}

struct Demo {
    started: Instant,
    _ticker: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let alive = this.update(cx, |demo, cx| {
                    if demo.started.elapsed().as_secs_f32() > demo_end() + DEMO_REST {
                        demo.started = Instant::now();
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Demo {
            started: Instant::now(),
            _ticker: ticker,
        }
    }

    /// Stop: the turn wraps up where it is.
    fn stop(&mut self) {
        let end = Duration::from_secs_f32(demo_end());
        if self.started.elapsed() < end
            && let Some(started) = Instant::now().checked_sub(end)
        {
            self.started = started;
        }
    }

    fn steps(&self) -> Vec<ThinkingStep> {
        let at = self.started.elapsed().as_secs_f32();
        DEMO.iter()
            .map(|d| {
                let step = ThinkingStep::new(d.kind, d.detail);
                if at >= d.to {
                    let step = step.text(d.text);
                    if d.fails { step.failed() } else { step.done() }
                } else if at >= d.from {
                    // A thought streams in word by word.
                    let words: Vec<&str> = d.text.split(' ').collect();
                    let shown = ((at - d.from) / (d.to - d.from) * words.len() as f32).ceil();
                    let shown = (shown as usize).clamp(1, words.len());
                    step.text(words[..shown].join(" ")).running()
                } else {
                    step
                }
            })
            .collect()
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("thinking-state-demo", cx, |_, cx| Demo::new(cx));
        let (steps, started) = {
            let d = demo.read(cx);
            (d.steps(), d.started)
        };
        // Pink: the user bubble – right-aligned, `wash 0.08`, r16, 14/22.
        let bubble = div().flex().justify_end().child(
            div()
                .max_w(px(DEMO_WIDTH * 0.8))
                .px(px(space::LG))
                .py(px(space::S10))
                .rounded(px(radius::BUBBLE))
                .bg(t.user_bubble_bg())
                // Pink: an inset edge, so the bubble's height stays exact.
                .when_some(t.user_bubble_edge(), |el, edge| {
                    el.shadow(vec![BoxShadow {
                        color: edge,
                        offset: point(px(0.), px(0.)),
                        blur_radius: px(0.),
                        spread_radius: px(1.),
                        inset: true,
                    }])
                })
                .text_size(px(14.))
                .line_height(px(22.))
                .text_color(t.text)
                .child("Split orders.total into subtotal, tax and shipping without downtime."),
        );
        div()
            .w(px(DEMO_WIDTH))
            .h(px(DEMO_HEIGHT))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .child(bubble)
            .child(
                div().pt(px(space::LG)).child(
                    ThinkingState::new("thinking-state", steps)
                        .started_at(started)
                        .width(DEMO_WIDTH)
                        .on_stop(move |_, cx| {
                            demo.update(cx, |d, cx| {
                                d.stop();
                                cx.notify();
                            })
                        }),
                ),
            )
    })
}
