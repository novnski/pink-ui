//! Diff Viewer – a checkout's changes, file by file, the way Pink's changes
//! pane shows them (Pink: changes.rs `Changes`).
//!
//! The toolbar carries the scope dropdown (Working tree, Branch changes,
//! Latest turn), the `{branch} → {base}` ref selector in branch scope, the
//! Git pill and the unified ⇄ split, wrap and fold-all toggles (Pink:
//! changes.rs `render_header_controls`). Under a strip that counts the files
//! and lines (`render_header_strip`) every file is a section: a 38 px header
//! with its disclosure chevron, file-type artwork, mono path and `+N −N`
//! (`render_file_header`), then its notices, `@@` hunk bands and diff rows
//! (their chrome drawn by the `pink-diff-rows` package,
//! [`crate::components::diff_rows`]) – dual line-number gutters, a `+ − ·` marker column, red and green row
//! washes with the changed words washed stronger (`diff_line_row`,
//! `word_changes`). Sections fold on Pink's disclosure curve, the header of
//! the file under the top edge sticks (`render_sticky_file_header`), long
//! lines scroll sideways together per file or wrap, split mode pairs each
//! hunk's deletions against its additions (`split_pairs`), and the list
//! wears Pink's floating scrollbar (popover.rs `rail`).
//!
//! Hovering a line offers Pink's `+` adder. It opens an inline draft card
//! whose note stages as a review comment card under the line, with edit and
//! remove on hover (Pink: comment_ui.rs).
//!
//! Feed it a unified patch (`git diff` output) or build the files yourself:
//!
//! ```ignore
//! DiffViewer::new("changes")
//!     .patch(git_diff_output)
//!     .scope(DiffScope::WorkingTree)
//!     .on_comments(|comments, _, _| { /* hand them to the agent */ })
//! ```

use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnyElement, AnyView, App, Context, DispatchPhase, DragMoveEvent, ElementId, EmptyView,
    Entity, Font, Hsla, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Role,
    ScrollHandle, SharedString, Stateful, StyledText, Subscription, Task, TextRun, Window,
    anchored, canvas, deferred, div, font, img, point, prelude::*, px, svg,
};

use super::scroll_rail;
use crate::{
    HoverFill as _,
    components::{
        comment_card as cc, diff_header as dh, diff_rows,
        icon_button::{IconButtonStyle, icon_button},
        surface_toolbar,
    },
    motion::{self, DISCLOSURE, MENU_OUT, MENU_RISE_DOWN},
    pink::{self, Pink, Syntax, TEXT_CONTRAST_TARGET, layout, loaders, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Diff Viewer";
pub const DESCRIPTION: &str = "A checkout's changes as Pink's diff pane: one folding section per file with its icon, path and +N −N, hunk bands over unified or side-by-side rows with the changed words washed, a sticky file header, scope and base pickers, and a + on any line to leave a review comment";

const ICON_CHEVRON: &str = "icons/diff-viewer/alt-arrow-right.svg";
const ICON_DOWN: &str = "icons/diff-viewer/alt-arrow-down.svg";
const ICON_ARROW_RIGHT: &str = "icons/diff-viewer/arrow-right.svg";
const ICON_SPLIT: &str = "icons/diff-viewer/columns-2.svg";
const ICON_WRAP: &str = "icons/diff-viewer/wrap-text.svg";
const ICON_FOLD_ALL: &str = "icons/diff-viewer/fold-vertical.svg";
const ICON_PLUS: &str = "icons/diff-viewer/plus.svg";
const ICON_PEN: &str = "icons/diff-viewer/pen.svg";
const ICON_REMOVE: &str = "icons/diff-viewer/close-circle.svg";

// Diff metrics (Pink: changes.rs layout numbers).
/// A file header is the surface chrome's header height.
const FILE_HEADER_HEIGHT: f32 = dh::FILE_HEADER_HEIGHT;
// The row chrome's own numbers live in the `pink-diff-rows` package.
const HUNK_HEADER_HEIGHT: f32 = diff_rows::HUNK_HEADER_HEIGHT;
const DIFF_LINE_HEIGHT: f32 = diff_rows::LINE_HEIGHT;
const NOTICE_HEIGHT: f32 = diff_rows::NOTICE_HEIGHT;
const BODY_BOTTOM_PAD: f32 = 8.0;
/// One line-number column, never narrower than this (`GUTTER_WIDTH`).
const GUTTER_WIDTH: f32 = diff_rows::GUTTER_WIDTH;
const GUTTER_PAD_RIGHT: f32 = diff_rows::GUTTER_PAD_RIGHT;
// A comment card's accent bar matches the rows' bar column.
const _: () = assert!(cc::ACCENT_BAR_WIDTH == diff_rows::ACCENT_BAR_WIDTH);
const DIFF_TEXT_SIZE: f32 = diff_rows::TEXT_SIZE;
const DIFF_TAB_SIZE: usize = 4;
const UNIFIED_CODE_PADDING_LEFT: f32 = 12.0;
const SPLIT_CODE_PADDING_LEFT: f32 = 6.0;
/// Breathing room after the widest line when scrolled fully right.
const CODE_PADDING_RIGHT: f32 = 24.0;

// Review comments (Pink: comments.rs, comment_ui.rs), drawn by the
// `pink-comment-card` package.
const COMMENT_ADDER_SIZE: f32 = cc::ADDER;
const CARD_WRAP_COLUMNS: usize = 64;
const CARD_MAX_LINES: usize = 8;
const DRAFT_CARD_HEIGHT: f32 = cc::DRAFT_HEIGHT;
/// Pink's composer input type, which its draft and search fields keep
/// (composer.rs `INPUT_TEXT_SIZE`, `INPUT_LINE_HEIGHT`).
const INPUT_TEXT_SIZE: f32 = 14.0;
const INPUT_LINE_HEIGHT: f32 = 22.75;

/// The changes header hangs its menus 10 pt below the trigger
/// (`anchored_menu_below_gap(…, 10.0)`).
const MENU_DROP_GAP: f32 = 10.0;

// The floating list scrollbar (Pink: popover.rs `MENU_SCROLLBAR_*`).
const RAIL_TRACK_INSET: f32 = scroll_rail::TRACK_INSET;
const RAIL_MIN_THUMB: f32 = scroll_rail::MIN_THUMB;
const RAIL_LINGER: Duration = Duration::from_millis(1400);
const RAIL_FADE: Duration = Duration::from_millis(260);
const RAIL_FADE_FRAME: Duration = Duration::from_millis(16);

// ---------------------------------------------------------------------------
// Patch model (Pink: changes.rs)
// ---------------------------------------------------------------------------

/// What one diff line is (Pink `LineKind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Add,
    Del,
    /// `\ No newline at end of file` and friends.
    Meta,
}

/// One line of a hunk with its line numbers on either side (Pink `DiffLine`).
/// Tabs expand to four columns (Pink `DIFF_TAB_SIZE`).
#[derive(Clone, Debug, PartialEq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
    pub text: SharedString,
}

impl DiffLine {
    /// An unchanged line, present on both sides.
    pub fn context(old_no: u32, new_no: u32, text: impl AsRef<str>) -> Self {
        Self {
            kind: LineKind::Context,
            old_no: Some(old_no),
            new_no: Some(new_no),
            text: expand_tabs(text.as_ref()).into(),
        }
    }

    /// A line only the new file has.
    pub fn add(new_no: u32, text: impl AsRef<str>) -> Self {
        Self {
            kind: LineKind::Add,
            old_no: None,
            new_no: Some(new_no),
            text: expand_tabs(text.as_ref()).into(),
        }
    }

    /// A line only the old file had.
    pub fn del(old_no: u32, text: impl AsRef<str>) -> Self {
        Self {
            kind: LineKind::Del,
            old_no: Some(old_no),
            new_no: None,
            text: expand_tabs(text.as_ref()).into(),
        }
    }

    /// A note about the row, such as "No newline at end of file".
    pub fn meta(text: impl Into<SharedString>) -> Self {
        Self {
            kind: LineKind::Meta,
            old_no: None,
            new_no: None,
            text: text.into(),
        }
    }
}

/// One `@@ … @@` hunk and its lines (Pink `Hunk`).
#[derive(Clone, Debug, PartialEq)]
pub struct Hunk {
    pub header: SharedString,
    pub lines: Vec<DiffLine>,
}

impl Hunk {
    pub fn new(header: impl Into<SharedString>) -> Self {
        Self {
            header: header.into(),
            lines: Vec::new(),
        }
    }

    pub fn line(mut self, line: DiffLine) -> Self {
        self.lines.push(line);
        self
    }

    pub fn lines(mut self, lines: impl IntoIterator<Item = DiffLine>) -> Self {
        self.lines.extend(lines);
        self
    }
}

/// How a file changed (Pink `FileStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Deleted,
    Modified,
    Renamed,
}

/// One file section of the diff (Pink `FileDiff`).
#[derive(Clone, Debug, PartialEq)]
pub struct DiffFile {
    /// Display path (the post-change side).
    pub path: SharedString,
    /// Pre-rename path, when different.
    pub old_path: Option<SharedString>,
    pub status: FileStatus,
    pub binary: bool,
    /// Extra notice rows (mode changes etc.).
    pub notices: Vec<SharedString>,
    pub hunks: Vec<Hunk>,
    pub additions: u32,
    pub deletions: u32,
    /// Largest line number on either side; sizes the gutters.
    pub max_line: u32,
}

impl DiffFile {
    pub fn new(path: impl Into<SharedString>) -> Self {
        Self {
            path: path.into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            notices: Vec::new(),
            hunks: Vec::new(),
            additions: 0,
            deletions: 0,
            max_line: 0,
        }
    }

    pub fn status(mut self, status: FileStatus) -> Self {
        self.status = status;
        self
    }

    /// Marks the file renamed from `old_path`.
    pub fn renamed_from(mut self, old_path: impl Into<SharedString>) -> Self {
        self.status = FileStatus::Renamed;
        self.old_path = Some(old_path.into());
        self
    }

    pub fn binary(mut self, binary: bool) -> Self {
        self.binary = binary;
        self
    }

    pub fn notice(mut self, notice: impl Into<SharedString>) -> Self {
        self.notices.push(notice.into());
        self
    }

    /// Adds a hunk, counting its additions, deletions and line numbers.
    pub fn hunk(mut self, hunk: Hunk) -> Self {
        for line in &hunk.lines {
            match line.kind {
                LineKind::Add => self.additions += 1,
                LineKind::Del => self.deletions += 1,
                _ => {}
            }
            self.max_line = self
                .max_line
                .max(line.old_no.unwrap_or(0))
                .max(line.new_no.unwrap_or(0));
        }
        self.hunks.push(hunk);
        self
    }
}

fn expand_tabs(text: &str) -> String {
    if !text.contains('\t') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 8);
    let mut column = 0usize;
    for ch in text.chars() {
        if ch == '\t' {
            let pad = DIFF_TAB_SIZE - column % DIFF_TAB_SIZE;
            out.extend(std::iter::repeat_n(' ', pad));
            column += pad;
        } else {
            out.push(ch);
            column += 1;
        }
    }
    out
}

fn strip_git_prefix(path: &str) -> &str {
    path.strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path)
}

/// Splits the tail of a `diff --git a/… b/…` line into (old, new) paths
/// (Pink `parse_git_paths`): quoted paths are unquoted, and unquoted paths
/// with spaces split on the last ` b/`, git's own convention.
fn parse_git_paths(rest: &str) -> (String, String) {
    fn unquote(s: &str) -> String {
        let trimmed = s.trim();
        if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
            trimmed[1..trimmed.len() - 1]
                .replace("\\\"", "\"")
                .replace("\\\\", "\\")
        } else {
            trimmed.to_string()
        }
    }
    if let Some(pos) = rest.rfind(" b/").or_else(|| rest.rfind(" \"b/")) {
        let old = unquote(&rest[..pos]);
        let new = unquote(&rest[pos + 1..]);
        (
            strip_git_prefix(&old).to_string(),
            strip_git_prefix(&new).to_string(),
        )
    } else {
        let path = strip_git_prefix(&unquote(rest)).to_string();
        (path.clone(), path)
    }
}

/// The starting line numbers of a `@@ -a[,b] +c[,d] @@` header.
fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let rest = line.strip_prefix("@@")?;
    let start = |marker: char| -> Option<u32> {
        let at = rest.find(marker)?;
        rest[at + 1..]
            .split(|c: char| c == ',' || c.is_whitespace())
            .next()?
            .parse()
            .ok()
    };
    Some((start('-')?, start('+')?))
}

/// Parses a unified git patch into file sections (Pink `parse_patch`).
/// Tolerant: unknown header lines are skipped, and a truncated hunk keeps
/// what parsed so far.
pub fn parse_patch(patch: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut in_hunk = false;
    let mut old_no: u32 = 0;
    let mut new_no: u32 = 0;

    for raw in patch.lines() {
        if let Some(rest) = raw.strip_prefix("diff --git ") {
            let (old, new) = parse_git_paths(rest);
            let mut file = DiffFile::new(new.clone());
            if old != new {
                file.old_path = Some(old.into());
            }
            files.push(file);
            in_hunk = false;
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };

        if raw.starts_with("@@") {
            if let Some((old, new)) = parse_hunk_header(raw) {
                old_no = old;
                new_no = new;
                file.hunks.push(Hunk::new(raw.to_string()));
                in_hunk = true;
            }
            continue;
        }

        if in_hunk {
            let mut chars = raw.chars();
            let marker = chars.next();
            let body = chars.as_str();
            let line = match marker {
                Some('+') => {
                    file.additions += 1;
                    let line = DiffLine::add(new_no, body);
                    new_no += 1;
                    Some(line)
                }
                Some('-') => {
                    file.deletions += 1;
                    let line = DiffLine::del(old_no, body);
                    old_no += 1;
                    Some(line)
                }
                Some(' ') | None => {
                    let line = DiffLine::context(old_no, new_no, body);
                    old_no += 1;
                    new_no += 1;
                    Some(line)
                }
                Some('\\') => Some(DiffLine::meta(
                    raw.trim_start_matches('\\').trim().to_string(),
                )),
                // A non-hunk line ends the hunk; read it as a header.
                _ => {
                    in_hunk = false;
                    None
                }
            };
            if let Some(line) = line
                && let Some(hunk) = file.hunks.last_mut()
            {
                file.max_line = file
                    .max_line
                    .max(line.old_no.unwrap_or(0))
                    .max(line.new_no.unwrap_or(0));
                hunk.lines.push(line);
                continue;
            }
            if in_hunk {
                continue;
            }
        }

        // File header territory.
        if raw.starts_with("new file mode") {
            file.status = FileStatus::Added;
        } else if raw.starts_with("deleted file mode") {
            file.status = FileStatus::Deleted;
        } else if let Some(from) = raw.strip_prefix("rename from ") {
            file.status = FileStatus::Renamed;
            file.old_path = Some(from.trim().to_string().into());
        } else if let Some(to) = raw.strip_prefix("rename to ") {
            file.status = FileStatus::Renamed;
            file.path = to.trim().to_string().into();
        } else if raw.starts_with("Binary files") || raw.starts_with("GIT binary patch") {
            file.binary = true;
        } else if let Some(mode) = raw.strip_prefix("new mode ") {
            file.notices
                .push(format!("Mode changed to {}", mode.trim()).into());
        } else if let Some(new) = raw.strip_prefix("+++ ") {
            let new = new.trim();
            if new == "/dev/null" {
                file.status = FileStatus::Deleted;
            } else if file.old_path.is_none() {
                file.path = strip_git_prefix(new).to_string().into();
            }
        } else if let Some(old) = raw.strip_prefix("--- ")
            && old.trim() == "/dev/null"
        {
            file.status = FileStatus::Added;
        }
        // "index …", "similarity index …", "old mode …": skipped.
    }
    files
}

/// A file's notice rows: new / deleted / renamed / binary, then its own
/// (Pink `file_notices`).
pub fn file_notices(file: &DiffFile) -> Vec<SharedString> {
    let mut notices: Vec<SharedString> = Vec::new();
    match file.status {
        FileStatus::Added => notices.push("New file".into()),
        FileStatus::Deleted => notices.push("Deleted file".into()),
        FileStatus::Renamed => {
            let from = file.old_path.as_deref().unwrap_or("?");
            notices.push(format!("Renamed from {from}").into());
        }
        FileStatus::Modified => {}
    }
    if file.binary {
        notices.push("Binary file, contents not shown".into());
    }
    notices.extend(file.notices.iter().cloned());
    notices
}

// ---------------------------------------------------------------------------
// Scopes and review comments
// ---------------------------------------------------------------------------

/// What the pane diffs against (Pink `DiffScope`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DiffScope {
    /// Uncommitted changes vs HEAD.
    #[default]
    WorkingTree,
    /// Everything the branch adds over its base, working tree included.
    Branch,
    /// Changes since the chat's last turn started.
    LatestTurn,
}

impl DiffScope {
    pub const ALL: [DiffScope; 3] = [Self::WorkingTree, Self::Branch, Self::LatestTurn];

    pub fn label(self) -> &'static str {
        match self {
            Self::WorkingTree => "Working tree",
            Self::Branch => "Branch changes",
            Self::LatestTurn => "Latest turn",
        }
    }
}

/// The header strip's label (Pink `scope_label`, `uncommitted_label`).
fn scope_label(scope: DiffScope, commit: bool, count: usize, base: Option<&str>) -> String {
    let files = if count == 1 { "file" } else { "files" };
    if commit {
        return format!("{count} Changed {files} in this commit");
    }
    match scope {
        DiffScope::WorkingTree if count == 1 => "1 Uncommitted change".to_string(),
        DiffScope::WorkingTree => format!("{count} Uncommitted changes"),
        DiffScope::Branch => match base {
            Some(base) => format!("{count} Changed {files} vs {base}"),
            None => format!("{count} Changed {files}"),
        },
        DiffScope::LatestTurn => format!("{count} Changed {files} this turn"),
    }
}

/// Empty-state copy per scope (Pink `clean_message`).
fn clean_message(scope: DiffScope, commit: bool, base: Option<&str>) -> String {
    if commit {
        return "Empty commit".to_string();
    }
    match scope {
        DiffScope::WorkingTree => "No uncommitted changes".to_string(),
        DiffScope::Branch => match base {
            Some(base) => format!("No changes vs {base}"),
            None => "No branch changes".to_string(),
        },
        DiffScope::LatestTurn => "No changes this turn".to_string(),
    }
}

/// Which file a comment cites: a deletion only exists in the old file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CommentSide {
    Old,
    New,
}

/// A review note pinned to one diff line (Pink: comments.rs `ReviewComment`).
#[derive(Clone, Debug, PartialEq)]
pub struct DiffComment {
    pub path: SharedString,
    /// The pre-rename path, cited by notes on deleted lines.
    pub old_path: Option<SharedString>,
    pub side: CommentSide,
    pub line: u32,
    pub body: SharedString,
}

impl DiffComment {
    pub fn new(
        path: impl Into<SharedString>,
        side: CommentSide,
        line: u32,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            path: path.into(),
            old_path: None,
            side,
            line,
            body: body.into(),
        }
    }

    pub fn renamed_from(mut self, old_path: impl Into<SharedString>) -> Self {
        self.old_path = Some(old_path.into());
        self
    }

    /// The cited `path:line` (Pink `ReviewComment::location`).
    pub fn location(&self) -> SharedString {
        let path = match self.side {
            CommentSide::Old => self.old_path.as_ref().unwrap_or(&self.path),
            CommentSide::New => &self.path,
        };
        format!("{path}:{}", self.line).into()
    }

    fn anchor(&self) -> (CommentSide, u32) {
        (self.side, self.line)
    }
}

// ---------------------------------------------------------------------------
// Pure layout (Pink: changes.rs)
// ---------------------------------------------------------------------------

/// One line-number column, fitted to the file's largest number: 11 px mono
/// ≈ 6.6 px a digit, the 8 px right pad and a 6 px left gap
/// (Pink `gutter_width`).
fn gutter_width(file: &DiffFile) -> f32 {
    let digits = file.max_line.max(1).ilog10() + 1;
    (digits as f32 * 6.6 + GUTTER_PAD_RIGHT + 6.0).max(GUTTER_WIDTH)
}

/// The line a comment on this row cites (Pink `line_anchor`).
fn line_anchor(line: &DiffLine) -> Option<(CommentSide, u32)> {
    match line.kind {
        LineKind::Meta => None,
        LineKind::Del => line.old_no.map(|no| (CommentSide::Old, no)),
        _ => line.new_no.map(|no| (CommentSide::New, no)),
    }
}

/// One split row: indices into the hunk's lines for the old and new column.
type LinePair = (Option<usize>, Option<usize>);

/// Pairs a hunk's lines into split rows (Pink `split_pairs`): context sits on
/// both sides, and each run of deletions followed by additions lines up
/// index for index, the longer side's leftovers getting one-sided rows.
fn split_pairs(lines: &[DiffLine]) -> Vec<LinePair> {
    #[derive(Default)]
    struct Block {
        dels: Vec<usize>,
        adds: Vec<usize>,
        del_meta: Vec<usize>,
        add_meta: Vec<usize>,
    }
    fn drain(pairs: &mut Vec<LinePair>, left: &mut Vec<usize>, right: &mut Vec<usize>) {
        for ix in 0..left.len().max(right.len()) {
            pairs.push((left.get(ix).copied(), right.get(ix).copied()));
        }
        left.clear();
        right.clear();
    }
    fn flush(pairs: &mut Vec<LinePair>, block: &mut Block) {
        drain(pairs, &mut block.dels, &mut block.adds);
        // Markers trail the code they annotate and pair with each other.
        drain(pairs, &mut block.del_meta, &mut block.add_meta);
    }

    let mut pairs = Vec::with_capacity(lines.len());
    let mut block = Block::default();
    let mut pending: Option<LineKind> = None;
    for (ix, line) in lines.iter().enumerate() {
        match line.kind {
            LineKind::Del => {
                if !block.adds.is_empty()
                    || !block.del_meta.is_empty()
                    || !block.add_meta.is_empty()
                {
                    flush(&mut pairs, &mut block);
                }
                block.dels.push(ix);
                pending = Some(LineKind::Del);
            }
            LineKind::Add => {
                if !block.add_meta.is_empty() {
                    flush(&mut pairs, &mut block);
                }
                block.adds.push(ix);
                pending = Some(LineKind::Add);
            }
            LineKind::Meta => match pending {
                Some(LineKind::Del) => block.del_meta.push(ix),
                Some(LineKind::Add) => block.add_meta.push(ix),
                _ => {
                    block.del_meta.push(ix);
                    block.add_meta.push(ix);
                }
            },
            LineKind::Context => {
                flush(&mut pairs, &mut block);
                pairs.push((Some(ix), Some(ix)));
                pending = Some(LineKind::Context);
            }
        }
    }
    flush(&mut pairs, &mut block);
    pairs
}

/// Every anchor a split row shows cards for, old column first; a context
/// row names one anchor once (Pink `pair_anchors`).
fn pair_anchors(lines: &[DiffLine], pair: LinePair) -> [Option<(CommentSide, u32)>; 2] {
    let anchor = |ix: Option<usize>| ix.and_then(|ix| lines.get(ix)).and_then(line_anchor);
    let (left, right) = (anchor(pair.0), anchor(pair.1));
    if left == right {
        [left, None]
    } else {
        [left, right]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Change {
    Equal,
    Delete,
    Insert,
}

/// Words and the whitespace runs between them, as separate tokens
/// (the `similar` crate's `TextDiff::from_words` tokens).
fn word_tokens(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut space: Option<bool> = None;
    for (at, ch) in text.char_indices() {
        let ws = ch.is_whitespace();
        if space.is_some_and(|prev| prev != ws) {
            out.push(&text[start..at]);
            start = at;
        }
        space = Some(ws);
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// A word-level diff of two lines: the longest common subsequence of their
/// tokens, deletions first where both sides change.
fn word_diff<'a>(old: &'a str, new: &'a str) -> Vec<(Change, &'a str)> {
    let a = word_tokens(old);
    let b = word_tokens(new);
    let (n, m) = (a.len(), b.len());
    let at = |i: usize, j: usize| i * (m + 1) + j;
    let mut lcs = vec![0u16; (n + 1) * (m + 1)];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[at(i, j)] = if a[i] == b[j] {
                lcs[at(i + 1, j + 1)] + 1
            } else {
                lcs[at(i + 1, j)].max(lcs[at(i, j + 1)])
            };
        }
    }
    let mut out = Vec::with_capacity(n + m);
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push((Change::Equal, a[i]));
            i += 1;
            j += 1;
        } else if j >= m || (i < n && lcs[at(i + 1, j)] >= lcs[at(i, j + 1)]) {
            out.push((Change::Delete, a[i]));
            i += 1;
        } else {
            out.push((Change::Insert, b[j]));
            j += 1;
        }
    }
    out
}

/// Byte ranges of `hunk.lines[line_ix]` that changed against its partner in
/// the same change block, the k-th deletion pairing with the k-th addition
/// (Pink `word_changes`). Unpaired, context and very long lines get none, nor
/// does a line that is almost all new: the row tint already says it.
fn word_changes(hunk: &Hunk, line_ix: usize) -> Vec<Range<usize>> {
    const MAX_LINE: usize = 400;
    let Some(line) = hunk.lines.get(line_ix) else {
        return Vec::new();
    };
    let changed = |kind: LineKind| matches!(kind, LineKind::Add | LineKind::Del);
    if !changed(line.kind) || line.text.len() > MAX_LINE {
        return Vec::new();
    }
    let mut start = line_ix;
    while start > 0 && changed(hunk.lines[start - 1].kind) {
        start -= 1;
    }
    let mut end = line_ix + 1;
    while end < hunk.lines.len() && changed(hunk.lines[end].kind) {
        end += 1;
    }
    let block = &hunk.lines[start..end];
    let of_kind = |kind: LineKind| {
        block
            .iter()
            .enumerate()
            .filter(move |(_, l)| l.kind == kind)
            .map(|(ix, _)| ix)
            .collect::<Vec<_>>()
    };
    let (dels, adds) = (of_kind(LineKind::Del), of_kind(LineKind::Add));
    let here = line_ix - start;
    let (old, new, mine_is_new) = if line.kind == LineKind::Del {
        let Some(k) = dels.iter().position(|&ix| ix == here) else {
            return Vec::new();
        };
        let Some(&partner) = adds.get(k) else {
            return Vec::new();
        };
        (&line.text, &block[partner].text, false)
    } else {
        let Some(k) = adds.iter().position(|&ix| ix == here) else {
            return Vec::new();
        };
        let Some(&partner) = dels.get(k) else {
            return Vec::new();
        };
        (&block[partner].text, &line.text, true)
    };
    if old.len() > MAX_LINE || new.len() > MAX_LINE {
        return Vec::new();
    }
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut offset = 0usize;
    for (tag, value) in word_diff(old, new) {
        let len = value.len();
        let mine = match tag {
            Change::Equal => {
                offset += len;
                continue;
            }
            Change::Insert => mine_is_new,
            Change::Delete => !mine_is_new,
        };
        if !mine {
            continue;
        }
        // Whitespace-only changes don't earn a highlight of their own.
        if !value.trim().is_empty() {
            match ranges.last_mut() {
                Some(last) if last.end == offset => last.end = offset + len,
                _ => ranges.push(offset..offset + len),
            }
        }
        offset += len;
    }
    let covered: usize = ranges.iter().map(|r| r.len()).sum();
    let own_len = if mine_is_new { new.len() } else { old.len() };
    if covered * 10 >= own_len * 9 {
        return Vec::new();
    }
    ranges
}

/// Splits `runs` so the given byte ranges carry `background` (Pink
/// `with_word_backgrounds`).
fn with_word_backgrounds(
    runs: Vec<TextRun>,
    ranges: &[Range<usize>],
    background: Hsla,
) -> Vec<TextRun> {
    if ranges.is_empty() {
        return runs;
    }
    let mut out = Vec::with_capacity(runs.len() + ranges.len() * 2);
    let mut pos = 0usize;
    for run in runs {
        let run_end = pos + run.len;
        let mut cursor = pos;
        while cursor < run_end {
            let inside = ranges.iter().find(|r| r.start <= cursor && cursor < r.end);
            let next = match inside {
                Some(r) => r.end.min(run_end),
                None => ranges
                    .iter()
                    .filter(|r| r.start > cursor)
                    .map(|r| r.start)
                    .min()
                    .unwrap_or(run_end)
                    .min(run_end),
            };
            let mut piece = run.clone();
            piece.len = next - cursor;
            if inside.is_some() {
                piece.background_color = Some(background);
            }
            out.push(piece);
            cursor = next;
        }
        pos = run_end;
    }
    out
}

/// Display columns of a line: the floor of its painted width.
fn visual_columns(text: &str) -> usize {
    text.chars().count()
}

/// Review-card body lines, guessed rather than measured so fold heights stay
/// analytic (Pink: comments.rs `card_body_lines`).
fn card_body_lines(body: &str) -> usize {
    body.lines()
        .map(|line| line.chars().count().div_ceil(CARD_WRAP_COLUMNS).max(1))
        .sum::<usize>()
        .clamp(1, CARD_MAX_LINES)
}

/// A staged comment card's height (Pink: comments.rs `card_height_for`).
fn card_height(body: &str) -> f32 {
    cc::card_height(card_body_lines(body), 16.0)
}

/// The expanded body's height, worked out rather than measured so a fold
/// can tween it (Pink `body_height_for_theme`).
fn body_height(
    file: &DiffFile,
    split: bool,
    comments: &[DiffComment],
    draft: Option<(CommentSide, u32)>,
) -> f32 {
    let cards = |anchor: Option<(CommentSide, u32)>| -> f32 {
        let Some(anchor) = anchor else {
            return 0.0;
        };
        let staged: f32 = comments
            .iter()
            .filter(|c| c.path == file.path && c.anchor() == anchor)
            .map(|c| card_height(&c.body))
            .sum();
        staged
            + if draft == Some(anchor) {
                DRAFT_CARD_HEIGHT
            } else {
                0.0
            }
    };
    let mut height = file_notices(file).len() as f32 * NOTICE_HEIGHT;
    for hunk in &file.hunks {
        height += HUNK_HEADER_HEIGHT;
        if split {
            for pair in split_pairs(&hunk.lines) {
                height += DIFF_LINE_HEIGHT;
                for anchor in pair_anchors(&hunk.lines, pair) {
                    height += cards(anchor);
                }
            }
        } else {
            for line in &hunk.lines {
                height += DIFF_LINE_HEIGHT + cards(line_anchor(line));
            }
        }
    }
    height + BODY_BOTTOM_PAD
}

/// A content key for the files on screen, to tell a new diff from the
/// same one rendered again (Pink `parse_key`).
fn diff_key(files: &[DiffFile]) -> u64 {
    let mut bytes = Vec::new();
    for file in files {
        bytes.extend_from_slice(file.path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&file.additions.to_le_bytes());
        bytes.extend_from_slice(&file.deletions.to_le_bytes());
        for hunk in &file.hunks {
            bytes.extend_from_slice(hunk.header.as_bytes());
            bytes.extend_from_slice(&(hunk.lines.len() as u32).to_le_bytes());
        }
    }
    loaders::fnv1a(&bytes)
}

/// Ranked branch filter: prefix matches first, then substrings, stable in
/// each rank (Pink: popover.rs `filter_indices`, `match_rank`).
fn filter_refs(query: &str, refs: &[SharedString]) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut ranked: Vec<(usize, usize)> = refs
        .iter()
        .enumerate()
        .filter_map(|(ix, name)| {
            if query.is_empty() {
                return Some((1, ix));
            }
            let name = name.to_lowercase();
            if name.starts_with(&query) {
                Some((0, ix))
            } else if name.contains(&query) {
                Some((1, ix))
            } else {
                None
            }
        })
        .collect();
    ranked.sort();
    ranked.into_iter().map(|(_, ix)| ix).collect()
}

/// The polychrome file-type artwork for a path (Pink `file_icons`, Symbols;
/// dark variants lifted like Pink's `dark_icon_svg`).
fn file_icon(path: &str, dark: bool) -> &'static str {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let ext = name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match (ext.as_str(), dark) {
        ("rs", false) => "icons/diff-viewer/file-rust.svg",
        ("rs", true) => "icons/diff-viewer/file-rust-dark.svg",
        ("ts" | "mts" | "cts", false) => "icons/diff-viewer/file-ts.svg",
        ("ts" | "mts" | "cts", true) => "icons/diff-viewer/file-ts-dark.svg",
        ("md" | "markdown", _) => "icons/diff-viewer/file-markdown.svg",
        ("toml", false) => "icons/diff-viewer/file-gear.svg",
        ("toml", true) => "icons/diff-viewer/file-gear-dark.svg",
        ("png" | "jpg" | "jpeg" | "gif" | "webp", _) => "icons/diff-viewer/file-image.svg",
        (_, false) => "icons/diff-viewer/file-document.svg",
        (_, true) => "icons/diff-viewer/file-document-dark.svg",
    }
}

// ---------------------------------------------------------------------------
// Syntax (paint-only runs, Pink: markdown/render.rs
// `runs_for_syntax_line_with_plain`)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lang {
    Rust,
    Script,
    Toml,
    Markdown,
    Plain,
}

impl Lang {
    fn of(path: &str) -> Self {
        let ext = path
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        match ext.as_str() {
            "rs" => Self::Rust,
            "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" => Self::Script,
            "toml" => Self::Toml,
            "md" | "markdown" => Self::Markdown,
            _ => Self::Plain,
        }
    }
}

/// The highlight roles a small scanner can tell apart, named after the
/// captures Pink's tree-sitter queries give them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Plain,
    Keyword,
    Type,
    Function,
    Property,
    Number,
    String,
    Comment,
    Punctuation,
    Heading,
    Raw,
    Link,
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "type", "use", "where", "while",
];

const SCRIPT_KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "default",
    "else",
    "export",
    "extends",
    "finally",
    "for",
    "from",
    "function",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "interface",
    "let",
    "new",
    "of",
    "private",
    "public",
    "readonly",
    "return",
    "static",
    "super",
    "switch",
    "this",
    "throw",
    "try",
    "type",
    "typeof",
    "var",
    "void",
    "while",
    "yield",
];

const PRIMITIVES: &[&str] = &[
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "isize", "str", "u8", "u16", "u32",
    "u64", "usize", "string", "number", "boolean", "unknown", "never",
];

const CONSTANTS: &[&str] = &["true", "false", "null", "undefined", "None", "Some"];

fn push_token(out: &mut Vec<(Range<usize>, Token)>, range: Range<usize>, kind: Token) {
    if range.is_empty() {
        return;
    }
    match out.last_mut() {
        Some((last, k)) if *k == kind && last.end == range.start => last.end = range.end,
        _ => out.push((range, kind)),
    }
}

/// Rust and TypeScript lines: comments, strings, numbers, keywords, types,
/// calls and fields.
fn tokenize_code(code: &str, lang: Lang) -> Vec<(Range<usize>, Token)> {
    let keywords = if lang == Lang::Rust {
        RUST_KEYWORDS
    } else {
        SCRIPT_KEYWORDS
    };
    let chars: Vec<(usize, char)> = code.char_indices().collect();
    let byte_at = |i: usize| chars.get(i).map_or(code.len(), |(b, _)| *b);
    let next_sig = |mut i: usize| {
        while i < chars.len() && chars[i].1.is_whitespace() {
            i += 1;
        }
        chars.get(i).map(|(_, c)| *c)
    };
    let mut out = Vec::new();
    let mut prev_sig: Option<char> = None;
    let mut prev_word = "";
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        if c == '/' && chars.get(i + 1).map(|(_, c)| *c) == Some('/') {
            push_token(&mut out, start..code.len(), Token::Comment);
            break;
        }
        if c.is_whitespace() {
            push_token(&mut out, start..byte_at(i + 1), Token::Plain);
            i += 1;
            continue;
        }
        let quote = c == '"' || (lang == Lang::Script && (c == '\'' || c == '`'));
        if quote {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1 != c {
                j += if chars[j].1 == '\\' { 2 } else { 1 };
            }
            j = (j + 1).min(chars.len());
            push_token(&mut out, start..byte_at(j), Token::String);
            prev_sig = Some('"');
            i = j;
            continue;
        }
        if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < chars.len() {
                let d = chars[j].1;
                let decimal = d == '.' && chars.get(j + 1).is_some_and(|(_, n)| n.is_ascii_digit());
                if d.is_ascii_alphanumeric() || d == '_' || decimal {
                    j += 1;
                } else {
                    break;
                }
            }
            push_token(&mut out, start..byte_at(j), Token::Number);
            prev_sig = Some('0');
            i = j;
            continue;
        }
        if c.is_alphabetic() || c == '_' || c == '$' {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].1.is_alphanumeric() || chars[j].1 == '_') {
                j += 1;
            }
            let word = &code[start..byte_at(j)];
            let after = next_sig(j);
            let upper = word.starts_with(|c: char| c.is_uppercase());
            let kind = if lang == Lang::Rust && chars.get(j).map(|(_, c)| *c) == Some('!') {
                j += 1;
                Token::Keyword
            } else if keywords.contains(&word) {
                Token::Keyword
            } else if CONSTANTS.contains(&word) {
                Token::Number
            } else if upper && word.len() > 1 && !word.chars().any(|c| c.is_lowercase()) {
                Token::Number
            } else if upper {
                Token::Type
            } else if prev_word == "fn" || prev_word == "function" || after == Some('(') {
                Token::Function
            } else if PRIMITIVES.contains(&word) {
                Token::Type
            } else if prev_sig == Some('.') {
                Token::Property
            } else {
                Token::Plain
            };
            push_token(&mut out, start..byte_at(j), kind);
            prev_word = word;
            prev_sig = Some('a');
            i = j;
            continue;
        }
        let kind = match c {
            '(' | ')' | '[' | ']' | '{' | '}' | ':' | '.' | ',' | ';' => Token::Punctuation,
            '*' | '&' | '=' | '!' | '?' | '<' | '>' | '+' | '-' | '|' => Token::Keyword,
            _ => Token::Plain,
        };
        push_token(&mut out, start..byte_at(i + 1), kind);
        prev_sig = Some(c);
        i += 1;
    }
    out
}

/// TOML: comments, `[sections]`, keys, strings, numbers and booleans.
fn tokenize_toml(code: &str) -> Vec<(Range<usize>, Token)> {
    let trimmed = code.trim_start();
    let indent = code.len() - trimmed.len();
    let mut out = Vec::new();
    push_token(&mut out, 0..indent, Token::Plain);
    if trimmed.starts_with('#') {
        push_token(&mut out, indent..code.len(), Token::Comment);
        return out;
    }
    if trimmed.starts_with('[') {
        push_token(&mut out, indent..code.len(), Token::Type);
        return out;
    }
    let value_start = match trimmed.find('=') {
        Some(eq) => {
            push_token(&mut out, indent..indent + eq, Token::Property);
            push_token(&mut out, indent + eq..indent + eq + 1, Token::Punctuation);
            indent + eq + 1
        }
        None => indent,
    };
    let value = &code[value_start..];
    let chars: Vec<(usize, char)> = value.char_indices().collect();
    let byte_at = |i: usize| value_start + chars.get(i).map_or(value.len(), |(b, _)| *b);
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        let start = value_start + start;
        if c == '"' || c == '\'' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1 != c {
                j += 1;
            }
            j = (j + 1).min(chars.len());
            push_token(&mut out, start..byte_at(j), Token::String);
            i = j;
        } else if c == '#' {
            push_token(&mut out, start..code.len(), Token::Comment);
            break;
        } else if c.is_ascii_digit() {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].1.is_ascii_alphanumeric() || chars[j].1 == '.') {
                j += 1;
            }
            push_token(&mut out, start..byte_at(j), Token::Number);
            i = j;
        } else if c.is_alphabetic() {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].1.is_alphanumeric() || chars[j].1 == '_') {
                j += 1;
            }
            let word = &code[start..byte_at(j)];
            let kind = if word == "true" || word == "false" {
                Token::Number
            } else {
                Token::Property
            };
            push_token(&mut out, start..byte_at(j), kind);
            i = j;
        } else {
            let kind = if "[]{},=".contains(c) {
                Token::Punctuation
            } else {
                Token::Plain
            };
            push_token(&mut out, start..byte_at(i + 1), kind);
            i += 1;
        }
    }
    out
}

/// Markdown: headings, code spans and links.
fn tokenize_markdown(code: &str) -> Vec<(Range<usize>, Token)> {
    let mut out = Vec::new();
    if code.trim_start().starts_with('#') {
        push_token(&mut out, 0..code.len(), Token::Heading);
        return out;
    }
    let bytes = code.as_bytes();
    let mut i = 0;
    let mut plain_from = 0;
    while i < bytes.len() {
        let span = match bytes[i] {
            b'`' => code[i + 1..]
                .find('`')
                .map(|end| (i, i + end + 2, Token::Raw)),
            b'[' | b'!' => {
                let open = if bytes[i] == b'!' { i + 1 } else { i };
                (bytes.get(open) == Some(&b'['))
                    .then(|| code[open..].find("](").map(|mid| open + mid))
                    .flatten()
                    .and_then(|mid| {
                        code[mid..]
                            .find(')')
                            .map(|end| (i, mid + end + 1, Token::Link))
                    })
            }
            _ => None,
        };
        if let Some((from, to, kind)) = span {
            push_token(&mut out, plain_from..from, Token::Plain);
            push_token(&mut out, from..to, kind);
            i = to;
            plain_from = to;
        } else {
            i += 1;
        }
    }
    push_token(&mut out, plain_from..code.len(), Token::Plain);
    out
}

fn tokenize(code: &str, lang: Lang) -> Vec<(Range<usize>, Token)> {
    match lang {
        Lang::Rust | Lang::Script => tokenize_code(code, lang),
        Lang::Toml => tokenize_toml(code),
        Lang::Markdown => tokenize_markdown(code),
        Lang::Plain => {
            let mut out = Vec::new();
            push_token(&mut out, 0..code.len(), Token::Plain);
            out
        }
    }
}

fn token_color(kind: Token, plain: Hsla, syntax: &Syntax) -> Hsla {
    match kind {
        Token::Plain => plain,
        Token::Keyword => syntax.keyword,
        Token::Type => syntax.type_name,
        Token::Function => syntax.function,
        Token::Property => syntax.property,
        Token::Number => syntax.number,
        Token::String => syntax.string,
        Token::Comment => syntax.comment,
        Token::Punctuation => syntax.punctuation,
        Token::Heading => syntax.markup_heading,
        Token::Raw => syntax.markup_raw,
        Token::Link => syntax.markup_link,
    }
}

/// Paint-only syntax runs for one line: one font everywhere, so recoloring
/// never changes layout.
fn syntax_runs(text: &str, lang: Lang, plain: Hsla, syntax: &Syntax, mono: &Font) -> Vec<TextRun> {
    tokenize(text, lang)
        .into_iter()
        .map(|(range, kind)| TextRun {
            len: range.len(),
            font: mono.clone(),
            color: token_color(kind, plain, syntax),
            background_color: None,
            underline: None,
            strikethrough: None,
        })
        .filter(|run| run.len > 0)
        .collect()
}

// ---------------------------------------------------------------------------
// Diff paint (Pink: changes.rs)
// ---------------------------------------------------------------------------

/// A changed row's tint: the hue at 10% dark / 8% light (`diff_row_wash`).
fn diff_row_wash(tone: Hsla, t: &Pink) -> Hsla {
    Hsla {
        a: if t.dark { 0.10 } else { 0.08 },
        ..tone
    }
}

/// The changed words' wash: 28% dark / 20% light (`diff_word_wash`).
fn diff_word_wash(tone: Hsla, t: &Pink) -> Hsla {
    Hsla {
        a: if t.dark { 0.28 } else { 0.20 },
        ..tone
    }
}

/// Unhighlighted code on context, added and deleted rows alike
/// (`diff_plain_text`).
fn diff_plain_text(t: &Pink) -> Hsla {
    let on_add = t.readable_on(
        t.text.opacity(0.92),
        diff_row_wash(t.diff_add, t),
        TEXT_CONTRAST_TARGET,
    );
    t.readable_on(on_add, diff_row_wash(t.diff_del, t), TEXT_CONTRAST_TARGET)
}

/// A row's marker, its color, the row wash and the line-number color
/// (`diff_row_paint`): two signals per changed row, the tint and the marker.
struct RowPaint {
    marker: &'static str,
    marker_color: Hsla,
    wash: Option<Hsla>,
    number: Hsla,
}

fn row_paint(kind: LineKind, t: &Pink) -> RowPaint {
    let tinted = |marker: &'static str, tone: Hsla| {
        let wash = diff_row_wash(tone, t);
        RowPaint {
            marker,
            marker_color: t.readable_on(tone, wash, TEXT_CONTRAST_TARGET),
            wash: Some(wash),
            number: t.readable_on(t.readable_faint(0.8), wash, TEXT_CONTRAST_TARGET),
        }
    };
    match kind {
        LineKind::Add => tinted("+", t.diff_add),
        LineKind::Del => tinted("−", t.diff_del),
        _ => RowPaint {
            // joiners: allow the context line's gutter marker, not a joiner
            marker: "·",
            marker_color: t.readable_faint(0.5),
            wash: None,
            number: t.readable_faint(0.8),
        },
    }
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

type ScopeHandler = Rc<dyn Fn(DiffScope, &mut Window, &mut App)>;
type RefHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type FlagHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type FoldHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type CommentsHandler = Rc<dyn Fn(&[DiffComment], &mut Window, &mut App)>;
type GitHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct DiffViewer {
    id: ElementId,
    files: Vec<DiffFile>,
    scope: DiffScope,
    branch: Option<SharedString>,
    base_ref: Option<SharedString>,
    branches: Vec<SharedString>,
    commit: Option<(SharedString, SharedString)>,
    preparing: bool,
    truncated: bool,
    error: Option<SharedString>,
    notice: Option<(SharedString, bool)>,
    split: bool,
    wrap: bool,
    collapsed: Vec<SharedString>,
    comments: Vec<DiffComment>,
    width: f32,
    height: f32,
    on_scope: Option<ScopeHandler>,
    on_base_ref: Option<RefHandler>,
    on_git: Option<GitHandler>,
    on_split: Option<FlagHandler>,
    on_wrap: Option<FlagHandler>,
    on_fold: Option<FoldHandler>,
    on_comments: Option<CommentsHandler>,
}

impl DiffViewer {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            files: Vec::new(),
            scope: DiffScope::WorkingTree,
            branch: None,
            base_ref: None,
            branches: Vec::new(),
            commit: None,
            preparing: false,
            truncated: false,
            error: None,
            notice: None,
            split: false,
            wrap: false,
            collapsed: Vec::new(),
            comments: Vec::new(),
            width: 760.,
            height: 820.,
            on_scope: None,
            on_base_ref: None,
            on_git: None,
            on_split: None,
            on_wrap: None,
            on_fold: None,
            on_comments: None,
        }
    }

    /// Shows a unified patch, as `git diff` prints it.
    pub fn patch(mut self, patch: impl AsRef<str>) -> Self {
        self.files = parse_patch(patch.as_ref());
        self
    }

    pub fn files(mut self, files: impl IntoIterator<Item = DiffFile>) -> Self {
        self.files.extend(files);
        self
    }

    pub fn file(mut self, file: DiffFile) -> Self {
        self.files.push(file);
        self
    }

    /// What the files are diffed against; report picks with
    /// [`on_scope`](Self::on_scope).
    pub fn scope(mut self, scope: DiffScope) -> Self {
        self.scope = scope;
        self
    }

    /// The checked-out branch, named before the base in branch scope.
    pub fn branch(mut self, branch: impl Into<SharedString>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// The ref branch scope compares against.
    pub fn base_ref(mut self, base: impl Into<SharedString>) -> Self {
        self.base_ref = Some(base.into());
        self
    }

    /// The refs the base picker lists, default branch first.
    pub fn branches<S: Into<SharedString>>(
        mut self,
        branches: impl IntoIterator<Item = S>,
    ) -> Self {
        self.branches = branches.into_iter().map(Into::into).collect();
        self
    }

    /// Pins the pane to one commit: a sha chip and its subject replace the
    /// scope dropdown.
    pub fn commit(
        mut self,
        sha: impl Into<SharedString>,
        subject: impl Into<SharedString>,
    ) -> Self {
        self.commit = Some((sha.into(), subject.into()));
        self
    }

    /// The diff hasn't arrived yet: a loader stands in for the files.
    pub fn preparing(mut self, preparing: bool) -> Self {
        self.preparing = preparing;
        self
    }

    /// Marks the diff a partial snapshot.
    pub fn truncated(mut self, truncated: bool) -> Self {
        self.truncated = truncated;
        self
    }

    /// A warning banner over the content, which stays.
    pub fn error(mut self, message: impl Into<SharedString>) -> Self {
        self.error = Some(message.into());
        self
    }

    /// A message in place of the files, such as a scope that can't load;
    /// `warn` tints it.
    pub fn scope_notice(mut self, message: impl Into<SharedString>, warn: bool) -> Self {
        self.notice = Some((message.into(), warn));
        self
    }

    /// Starts in side-by-side layout.
    pub fn split(mut self, split: bool) -> Self {
        self.split = split;
        self
    }

    /// Starts with long lines wrapped.
    pub fn wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    /// Paths folded shut when a diff arrives. Like Pink, every new diff
    /// starts from these folds again.
    pub fn collapsed<S: Into<SharedString>>(mut self, paths: impl IntoIterator<Item = S>) -> Self {
        self.collapsed = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Review comments already staged when the pane opens.
    pub fn comments(mut self, comments: impl IntoIterator<Item = DiffComment>) -> Self {
        self.comments.extend(comments);
        self
    }

    pub fn comment(mut self, comment: DiffComment) -> Self {
        self.comments.push(comment);
        self
    }

    /// Panel width in px (default 760).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Panel height in px (default 820).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn on_scope(
        mut self,
        handler: impl Fn(DiffScope, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_scope = Some(Rc::new(handler));
        self
    }

    pub fn on_base_ref(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_base_ref = Some(Rc::new(handler));
        self
    }

    /// Shows the Git pill in working-tree scope.
    pub fn on_git(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_git = Some(Rc::new(handler));
        self
    }

    /// Called with the new layout when the split toggle flips.
    pub fn on_split(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_split = Some(Rc::new(handler));
        self
    }

    /// Called with the new setting when the wrap toggle flips.
    pub fn on_wrap(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_wrap = Some(Rc::new(handler));
        self
    }

    /// Called with a path and whether it is now folded shut.
    pub fn on_fold(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_fold = Some(Rc::new(handler));
        self
    }

    /// Called with every staged comment whenever one is added, edited or
    /// removed.
    pub fn on_comments(
        mut self,
        handler: impl Fn(&[DiffComment], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_comments = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The line under the pointer, which shows the comment adder.
#[derive(Clone, Debug, PartialEq)]
struct Hover {
    path: SharedString,
    side: CommentSide,
    line: u32,
}

/// Where a comment goes.
#[derive(Clone)]
struct Target {
    path: SharedString,
    old_path: Option<SharedString>,
    side: CommentSide,
    line: u32,
}

/// An open comment draft (Pink `CommentDraft`).
struct Draft {
    target: Target,
    /// The staged comment being edited.
    editing: Option<usize>,
    input: Entity<TextInput>,
    _events: Subscription,
}

/// The open base-ref picker's search (Pink `RefMenu`).
struct RefSearch {
    input: Entity<TextInput>,
    _events: Subscription,
    query: SharedString,
    active: usize,
    list: ScrollHandle,
}

/// A menu's lifecycle (Pink `Popup`): open, or fading out from `closing`.
#[derive(Default)]
struct Popup {
    open: bool,
    closing: Option<Instant>,
    /// The trigger press found the menu mounted, so its click leaves it
    /// closed (Pink `note_trigger_press`).
    pressed_while_open: bool,
    reap: Option<Task<()>>,
}

impl Popup {
    fn mounted(&self) -> bool {
        self.open || self.closing.is_some()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Menu {
    Scope,
    Ref,
}

/// The floating scrollbar's hover, drag and linger state (Pink
/// `MenuScrollbarState`).
#[derive(Default)]
struct Rail {
    list_hovered: bool,
    bar_hovered: bool,
    grab: Option<f32>,
    last_y: Option<f32>,
    last_at: Option<Instant>,
    wake_at: Option<Instant>,
}

impl Rail {
    /// A changed offset marks fresh scrolling; the first one is a baseline.
    fn note(&mut self, y: f32, now: Instant) {
        match self.last_y {
            None => self.last_y = Some(y),
            Some(previous) if previous != y => {
                self.last_y = Some(y);
                self.last_at = Some(now);
            }
            Some(_) => {}
        }
    }

    fn countdown(&self, now: Instant) -> bool {
        self.last_at
            .is_some_and(|at| now.saturating_duration_since(at) < RAIL_LINGER + RAIL_FADE)
    }

    fn held(&self) -> bool {
        self.grab.is_some() || self.bar_hovered
    }

    fn visible(&self, now: Instant) -> bool {
        self.held() || self.countdown(now)
    }

    /// 1 → 0 across the fade once scrolling stops; full while held.
    fn fade(&self, now: Instant) -> f32 {
        if self.held() {
            return 1.0;
        }
        let Some(at) = self.last_at else {
            return 0.0;
        };
        let elapsed = now.saturating_duration_since(at).as_secs_f32();
        (1.0 - (elapsed - RAIL_LINGER.as_secs_f32()) / RAIL_FADE.as_secs_f32()).clamp(0.0, 1.0)
    }

    fn next_wake(&self, now: Instant) -> Option<Instant> {
        if self.held() || !self.countdown(now) {
            return None;
        }
        let at = self.last_at?;
        let elapsed = now.saturating_duration_since(at);
        if elapsed < RAIL_LINGER {
            Some(at + RAIL_LINGER)
        } else if elapsed < RAIL_LINGER + RAIL_FADE {
            Some(now + RAIL_FADE_FRAME)
        } else {
            None
        }
    }

    fn set_bar_hovered(&mut self, hovered: bool, now: Instant) -> bool {
        let active = hovered || self.grab.is_some();
        if self.bar_hovered == active {
            return false;
        }
        self.bar_hovered = active;
        if !active {
            self.last_at = Some(now);
        }
        true
    }

    fn set_list_hovered(&mut self, hovered: bool, now: Instant) -> bool {
        if self.list_hovered == hovered {
            return false;
        }
        self.list_hovered = hovered;
        if !hovered && self.grab.is_none() && self.bar_hovered {
            self.bar_hovered = false;
            self.last_at = Some(now);
        }
        true
    }

    fn end_press(&mut self, now: Instant) {
        self.grab = None;
        self.last_at = Some(now);
        if !self.list_hovered && self.bar_hovered {
            self.bar_hovered = false;
        }
    }
}

/// The thumb's geometry for one viewport (Pink `MenuScrollbarMetrics`).
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
        let content = viewport + max_scroll;
        let thumb = (track * viewport / content).max(RAIL_MIN_THUMB).min(track);
        let travel = (track - thumb).max(0.0);
        Some(Self {
            track,
            thumb_top: travel * scroll.clamp(0.0, max_scroll) / max_scroll,
            thumb,
            max_scroll,
        })
    }

    fn of(scroll: &ScrollHandle) -> Option<Self> {
        let max = f32::from(scroll.max_offset().y).max(0.0);
        Self::from_viewport(
            f32::from(scroll.bounds().size.height),
            max,
            -f32::from(scroll.offset().y),
        )
    }

    fn travel(&self) -> f32 {
        (self.track - self.thumb).max(0.0)
    }
}

struct RailDrag;

struct ViewerState {
    /// The content of the diff on screen; a new one resets the folds.
    diff_key: Option<u64>,
    collapsed: HashSet<SharedString>,
    /// Bumped when every section snaps (fold all, wrapped toggles), so the
    /// fold followers restart at their targets instead of gliding.
    fold_epoch: usize,
    split: bool,
    wrap: bool,
    hover: Option<Hover>,
    comments: Vec<DiffComment>,
    draft: Option<Draft>,
    scope_menu: Popup,
    ref_menu: Popup,
    ref_search: Option<RefSearch>,
    list: ScrollHandle,
    /// One horizontal scroll per file, shared by its rows (Pink
    /// `FileHorizontalState::scroll`).
    hscroll: HashMap<SharedString, ScrollHandle>,
    rail: Rail,
    rail_wake: Option<Task<()>>,
    // Mirrored from the builder every render, for input events.
    branches: Rc<Vec<SharedString>>,
    base_ref: Option<SharedString>,
    on_base_ref: Option<RefHandler>,
    on_comments: Option<CommentsHandler>,
}

impl ViewerState {
    fn popup(&mut self, menu: Menu) -> &mut Popup {
        match menu {
            Menu::Scope => &mut self.scope_menu,
            Menu::Ref => &mut self.ref_menu,
        }
    }

    /// Starts a menu's exit fade and unmounts it once the fade has run
    /// (Pink `begin_close` + `reap_popup`).
    fn close_menu(&mut self, menu: Menu, cx: &mut Context<Self>) {
        let popup = self.popup(menu);
        if !popup.open {
            return;
        }
        popup.open = false;
        let since = Instant::now();
        popup.closing = Some(since);
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        popup.reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                let popup = s.popup(menu);
                if popup.closing == Some(since) {
                    popup.closing = None;
                    if menu == Menu::Ref {
                        s.ref_search = None;
                    }
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Opens the base picker with its search focused and the highlight on
    /// the current base (Pink `open_ref_menu`).
    fn open_ref_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Search branches…"));
        let events = cx.subscribe_in(&input, window, |s: &mut Self, _, event, window, cx| {
            s.on_ref_input(event, window, cx)
        });
        let active = self
            .base_ref
            .as_ref()
            .and_then(|base| self.branches.iter().position(|b| b == base))
            .unwrap_or(0);
        TextInput::focus(&input, window, cx);
        self.ref_search = Some(RefSearch {
            input,
            _events: events,
            query: SharedString::default(),
            active,
            list: ScrollHandle::new(),
        });
        let popup = &mut self.ref_menu;
        popup.open = true;
        popup.closing = None;
        popup.reap = None;
        cx.notify();
    }

    /// ↑↓ move the highlight, ⏎ picks it, Esc closes (Pink `ref_menu_key`).
    fn on_ref_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ref_menu.open {
            return;
        }
        let query = self
            .ref_search
            .as_ref()
            .map(|r| r.query.clone())
            .unwrap_or_default();
        match event {
            InputEvent::Change(text) => {
                if let Some(search) = self.ref_search.as_mut() {
                    search.query = text.clone();
                    search.active = 0;
                }
                cx.notify();
            }
            InputEvent::MoveUp | InputEvent::MoveDown => {
                let count = filter_refs(&query, &self.branches).len();
                if let Some(search) = self.ref_search.as_mut()
                    && count > 0
                {
                    let delta: isize = if *event == InputEvent::MoveUp { -1 } else { 1 };
                    search.active =
                        (search.active as isize + delta).rem_euclid(count as isize) as usize;
                    search.list.scroll_to_item(search.active);
                    cx.notify();
                }
            }
            InputEvent::Submit(_) => {
                let active = self.ref_search.as_ref().map_or(0, |r| r.active);
                if let Some(&ix) = filter_refs(&query, &self.branches).get(active) {
                    self.pick_ref(ix, window, cx);
                }
            }
            InputEvent::Escape => self.close_menu(Menu::Ref, cx),
            _ => {}
        }
    }

    /// Picks a base ref and reports it (Pink `set_base_ref`).
    fn pick_ref(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menu(Menu::Ref, cx);
        let Some(name) = self.branches.get(ix).cloned() else {
            return;
        };
        if self.base_ref.as_ref() == Some(&name) {
            return;
        }
        if let Some(f) = self.on_base_ref.clone() {
            window.defer(cx, move |window, cx| f(&name, window, cx));
        }
    }

    /// Opens the inline draft under a line, or over a staged comment to edit
    /// it (Pink `open_draft`, `edit_comment`).
    fn open_draft(
        &mut self,
        target: Target,
        editing: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let body = editing
            .and_then(|ix| self.comments.get(ix))
            .map(|c| c.body.clone());
        let input = cx.new(|cx| {
            let input = TextInput::new(window, cx)
                .placeholder("Request a change…")
                .multiline()
                .submit_on_enter();
            match body {
                Some(body) => input.default_text(body),
                None => input,
            }
        });
        let events =
            cx.subscribe_in(
                &input,
                window,
                |s: &mut Self, _, event, window, cx| match event {
                    InputEvent::Submit(_) => s.commit_draft(window, cx),
                    InputEvent::Escape => {
                        s.draft = None;
                        cx.notify();
                    }
                    _ => {}
                },
            );
        TextInput::focus(&input, window, cx);
        self.draft = Some(Draft {
            target,
            editing,
            input,
            _events: events,
        });
        cx.notify();
    }

    /// Stages the draft's note, or saves the edit (Pink `commit_draft`).
    fn commit_draft(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.draft.take() else {
            return;
        };
        let body = draft.input.read(cx).text().trim().to_string();
        if !body.is_empty() {
            match draft.editing {
                Some(ix) => {
                    if let Some(comment) = self.comments.get_mut(ix) {
                        comment.body = body.into();
                    }
                }
                None => {
                    let target = draft.target;
                    self.comments.push(DiffComment {
                        path: target.path,
                        old_path: target.old_path,
                        side: target.side,
                        line: target.line,
                        body: body.into(),
                    });
                }
            }
            self.report_comments(window, cx);
        }
        cx.notify();
    }

    fn remove_comment(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        if ix < self.comments.len() {
            self.comments.remove(ix);
            if let Some(draft) = self.draft.as_mut()
                && let Some(editing) = draft.editing
            {
                if editing == ix {
                    self.draft = None;
                } else if editing > ix {
                    draft.editing = Some(editing - 1);
                }
            }
            self.report_comments(window, cx);
            cx.notify();
        }
    }

    fn report_comments(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(f) = self.on_comments.clone() {
            let comments = self.comments.clone();
            window.defer(cx, move |window, cx| f(&comments, window, cx));
        }
    }

    /// Keeps a repaint scheduled while the scrollbar lingers and fades
    /// (Pink `schedule_scrollbar_hide`).
    fn arm_rail_wake(&mut self, now: Instant, cx: &mut Context<Self>) {
        let Some(wake) = self.rail.next_wake(now) else {
            self.rail.wake_at = None;
            return;
        };
        if self.rail.wake_at.is_some_and(|pending| pending > now) {
            return;
        }
        self.rail.wake_at = Some(wake);
        let delay = wake.saturating_duration_since(now);
        self.rail_wake = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |s, cx| {
                s.rail.wake_at = None;
                cx.notify();
            })
            .ok();
        }));
    }

    /// A press on the rail: grab the thumb where it was pressed, or center
    /// it under the pointer, then scroll there (Pink `begin_press`).
    fn rail_press(&mut self, pointer_y: f32) -> bool {
        let Some(metrics) = RailMetrics::of(&self.list) else {
            return false;
        };
        let in_track = pointer_y - f32::from(self.list.bounds().top()) - RAIL_TRACK_INSET;
        let on_thumb = (metrics.thumb_top..=metrics.thumb_top + metrics.thumb).contains(&in_track);
        self.rail.grab = Some(if on_thumb {
            in_track - metrics.thumb_top
        } else {
            metrics.thumb / 2.0
        });
        self.rail_drag(pointer_y)
    }

    fn rail_drag(&mut self, pointer_y: f32) -> bool {
        let (Some(grab), Some(metrics)) = (self.rail.grab, RailMetrics::of(&self.list)) else {
            return false;
        };
        let in_track = pointer_y - f32::from(self.list.bounds().top()) - RAIL_TRACK_INSET;
        let thumb_top = (in_track - grab).clamp(0.0, metrics.travel());
        let fraction = if metrics.travel() <= 0.0 {
            0.0
        } else {
            thumb_top / metrics.travel()
        };
        let offset = self.list.offset();
        self.list
            .set_offset(point(offset.x, px(-fraction * metrics.max_scroll)));
        true
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

fn toggle_fold(
    state: &Entity<ViewerState>,
    path: &SharedString,
    on_fold: &Option<FoldHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let collapsed = state.update(cx, |s, cx| {
        let collapsed = if s.collapsed.remove(path) {
            false
        } else {
            s.collapsed.insert(path.clone());
            true
        };
        // Wrapped sections are variable-height: they settle at once.
        if s.wrap {
            s.fold_epoch += 1;
        }
        cx.notify();
        collapsed
    });
    if let Some(f) = on_fold {
        f(path, collapsed, window, cx);
    }
}

/// Folds every file shut, or opens them all when everything already is
/// (Pink `toggle_collapse_all`): a snap, not a tween.
fn toggle_all(
    state: &Entity<ViewerState>,
    paths: &[SharedString],
    on_fold: &Option<FoldHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let changed = state.update(cx, |s, cx| {
        let collapse = !paths.iter().all(|p| s.collapsed.contains(p));
        let mut changed = Vec::new();
        for path in paths {
            let now = if collapse {
                s.collapsed.insert(path.clone())
            } else {
                s.collapsed.remove(path)
            };
            if now {
                changed.push((path.clone(), collapse));
            }
        }
        s.fold_epoch += 1;
        cx.notify();
        changed
    });
    if let Some(f) = on_fold {
        for (path, collapsed) in changed {
            f(&path, collapsed, window, cx);
        }
    }
}

fn set_hover(state: &Entity<ViewerState>, next: Hover, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.hover.as_ref() != Some(&next) {
            s.hover = Some(next);
            cx.notify();
        }
    });
}

fn clear_hover(state: &Entity<ViewerState>, at: &Hover, cx: &mut App) {
    state.update(cx, |s, cx| {
        if s.hover.as_ref() == Some(at) {
            s.hover = None;
            cx.notify();
        }
    });
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

/// What every row needs to paint.
struct Ctx<'a> {
    key: &'a str,
    t: &'a Pink,
    state: &'a Entity<ViewerState>,
    split: bool,
    wrap: bool,
    epoch: usize,
    collapsed: &'a HashSet<SharedString>,
    hover: Option<&'a Hover>,
    comments: &'a [DiffComment],
    draft: Option<&'a DraftView>,
    mono: Font,
    syntax: Syntax,
    plain: Hsla,
    /// One mono column at the code size.
    advance: f32,
    hscroll: &'a HashMap<SharedString, ScrollHandle>,
    on_fold: Option<FoldHandler>,
}

struct DraftView {
    target: Target,
    editing: bool,
    input: Entity<TextInput>,
}

impl Ctx<'_> {
    fn draft_anchor(&self, file: &DiffFile) -> Option<(CommentSide, u32)> {
        self.draft
            .filter(|d| d.target.path == file.path)
            .map(|d| (d.target.side, d.target.line))
    }

    fn fold_key(&self, ix: usize, file: &DiffFile) -> String {
        format!("{}-fold-{}-{ix}-{}", self.key, self.epoch, file.path)
    }
}

/// One file's geometry and language.
struct FileCx<'a> {
    c: &'a Ctx<'a>,
    ix: usize,
    file: &'a DiffFile,
    gutter: f32,
    lang: Lang,
    text_width: f32,
    scroll: Option<ScrollHandle>,
}

impl FileCx<'_> {
    fn slot(&self, suffix: &str) -> Option<(SharedString, ScrollHandle)> {
        self.scroll.clone().map(|handle| {
            (
                SharedString::from(format!("{}-code-{}-{suffix}", self.c.key, self.ix)),
                handle,
            )
        })
    }
}

impl RenderOnce for DiffViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();

        let state = {
            let collapsed: HashSet<SharedString> = self.collapsed.iter().cloned().collect();
            let comments = self.comments.clone();
            let (split, wrap) = (self.split, self.wrap);
            window.use_keyed_state(
                ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
                cx,
                move |_, _| ViewerState {
                    diff_key: None,
                    collapsed,
                    fold_epoch: 0,
                    split,
                    wrap,
                    hover: None,
                    comments,
                    draft: None,
                    scope_menu: Popup::default(),
                    ref_menu: Popup::default(),
                    ref_search: None,
                    list: ScrollHandle::new(),
                    hscroll: HashMap::new(),
                    rail: Rail::default(),
                    rail_wake: None,
                    branches: Rc::default(),
                    base_ref: None,
                    on_base_ref: None,
                    on_comments: None,
                },
            )
        };

        let files = Rc::new(self.files);
        let now = Instant::now();
        let content = diff_key(&files);
        let (draft_input, ref_input) = state.update(cx, |s, cx| {
            s.branches = Rc::new(self.branches.clone());
            s.base_ref = self.base_ref.clone();
            s.on_base_ref = self.on_base_ref.clone();
            s.on_comments = self.on_comments.clone();
            // A new diff starts from fresh folds and horizontal scrolls
            // (Pink `sync`: `folds.clear()` and a new `FileHorizontalState`
            // per file on every parse).
            if s.diff_key != Some(content) {
                if s.diff_key.is_some() {
                    s.collapsed = self.collapsed.iter().cloned().collect();
                    s.fold_epoch += 1;
                    s.hscroll.clear();
                    s.hover = None;
                }
                s.diff_key = Some(content);
            }
            for file in files.iter() {
                s.hscroll.entry(file.path.clone()).or_default();
            }
            let y = -f32::from(s.list.offset().y);
            s.rail.note(y, now);
            s.arm_rail_wake(now, cx);
            (
                s.draft.as_ref().map(|d| d.input.clone()),
                s.ref_search.as_ref().map(|r| r.input.clone()),
            )
        });
        for input in [draft_input, ref_input].into_iter().flatten() {
            input.update(cx, |input, _| {
                // Pink's composer inputs outside the message composer use the faint
                // tone for their placeholder.
                input.set_colors(Some(t.text_faint), Some(t.caret), Some(t.selection))
            });
        }

        // Snapshot the state for painting.
        let s = state.read(cx);
        let collapsed = s.collapsed.clone();
        let hover = s.hover.clone();
        let comments = s.comments.clone();
        let draft = s.draft.as_ref().map(|d| DraftView {
            target: d.target.clone(),
            editing: d.editing.is_some(),
            input: d.input.clone(),
        });
        let ref_view = s.ref_search.as_ref().map(|r| RefView {
            input: r.input.clone(),
            query: r.query.clone(),
            active: r.active,
            list: r.list.clone(),
        });
        let hscroll = s.hscroll.clone();
        let list_scroll = s.list.clone();
        let (split, wrap, epoch) = (s.split, s.wrap, s.fold_epoch);
        let rail_view = RailMetrics::of(&s.list)
            .filter(|_| s.rail.visible(now))
            .map(|metrics| (metrics, s.rail.fade(now), s.rail.held()));
        let reduced = motion::reduced_motion(cx);
        let exit = |popup: &Popup| -> Option<Option<f32>> {
            if popup.open {
                Some(None)
            } else {
                popup.closing.map(|since| {
                    Some(if reduced {
                        1.0
                    } else {
                        motion::menu_exit_progress(since)
                    })
                })
            }
        };
        let scope_menu = exit(&s.scope_menu);
        let ref_menu = exit(&s.ref_menu);

        let c = Ctx {
            key: &key,
            t: &t,
            state: &state,
            split,
            wrap,
            epoch,
            collapsed: &collapsed,
            hover: hover.as_ref(),
            comments: &comments,
            draft: draft.as_ref(),
            mono: font(pink::MONO),
            syntax: t.syntax(),
            plain: diff_plain_text(&t),
            advance: mono_advance(window),
            hscroll: &hscroll,
            on_fold: self.on_fold.clone(),
        };

        // ---- toolbar (Pink: changes.rs `render_header_controls`) ---------
        let paths: Rc<Vec<SharedString>> = Rc::new(files.iter().map(|f| f.path.clone()).collect());
        let split_toggle = {
            let state = state.clone();
            let on_split = self.on_split.clone();
            header_toggle(&c, "split", ICON_SPLIT, "Split view", split).on_click(
                move |_, window, cx| {
                    cx.stop_propagation();
                    let split = state.update(cx, |s, cx| {
                        s.split = !s.split;
                        s.hover = None;
                        for handle in s.hscroll.values() {
                            handle.set_offset(point(px(0.), px(0.)));
                        }
                        cx.notify();
                        s.split
                    });
                    if let Some(f) = &on_split {
                        f(split, window, cx);
                    }
                },
            )
        };
        let wrap_toggle = {
            let state = state.clone();
            let on_wrap = self.on_wrap.clone();
            header_toggle(&c, "wrap", ICON_WRAP, "Wrap long lines", wrap)
                .tooltip(header_tooltip("Wrap long lines"))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let wrap = state.update(cx, |s, cx| {
                        s.wrap = !s.wrap;
                        s.fold_epoch += 1;
                        for handle in s.hscroll.values() {
                            handle.set_offset(point(px(0.), px(0.)));
                        }
                        cx.notify();
                        s.wrap
                    });
                    if let Some(f) = &on_wrap {
                        f(wrap, window, cx);
                    }
                })
        };
        let fold_all = {
            let state = state.clone();
            let on_fold = self.on_fold.clone();
            let paths = paths.clone();
            header_toggle(&c, "fold-all", ICON_FOLD_ALL, "Fold all files", false).on_click(
                move |_, window, cx| {
                    cx.stop_propagation();
                    toggle_all(&state, &paths, &on_fold, window, cx);
                },
            )
        };

        let controls = dh::header_row();
        let controls = if let Some((sha, subject)) = self.commit.clone() {
            // A commit-pinned pane: a fixed identity chip replaces the scope
            // dropdown.
            let short: String = sha.chars().take(7).collect();
            controls
                .child(dh::commit_sha(
                    short,
                    pink::MONO.into(),
                    t.ink(0.05),
                    t.text_muted,
                ))
                .child(dh::commit_subject(subject, t.text))
                .child(split_toggle)
                .child(wrap_toggle)
                .child(fold_all)
        } else {
            let scope_card = scope_menu.map(|exit| {
                anchored_menu(
                    format!("{key}-scope-menu").into(),
                    exit,
                    scope_menu_card(&c, self.scope, self.on_scope.clone()),
                )
            });
            let ref_selector = (self.scope == DiffScope::Branch).then(|| {
                let card = ref_menu.zip(ref_view.as_ref()).map(|(exit, view)| {
                    anchored_menu(
                        format!("{key}-ref-menu").into(),
                        exit,
                        ref_menu_card(&c, view, &self.branches, self.base_ref.as_ref()),
                    )
                });
                ref_selector(
                    &c,
                    self.branch.clone().unwrap_or_else(|| "HEAD".into()),
                    self.base_ref.clone().unwrap_or_else(|| "…".into()),
                    card,
                )
            });
            let git = self
                .on_git
                .clone()
                .filter(|_| self.scope == DiffScope::WorkingTree)
                .map(|on_git| {
                    // Pink's Git pill: a quiet `wash(0.05)` plate, no hover.
                    dh::git_chip(
                        SharedString::from(format!("{key}-git")),
                        "Git",
                        t.wash(0.05),
                        t.text,
                    )
                    .role(Role::Button)
                    .on_click(move |_, window, cx| on_git(window, cx))
                });
            controls
                .child(scope_trigger(&c, self.scope, scope_card))
                .children(ref_selector)
                .child(dh::spacer())
                .child(
                    dh::trailing()
                        .children(git)
                        .child(split_toggle)
                        .child(wrap_toggle)
                        .child(fold_all),
                )
        };
        // Pink: surface_chrome.rs `toolbar` – no band and no border.
        let toolbar = surface_toolbar::toolbar().child(controls);

        // ---- content (Pink: changes.rs `render`) --------------------------
        let commit = self.commit.is_some();
        let content: AnyElement = if let Some((message, warn)) = self.notice.clone() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .px(px(pink::space::S16))
                .text_size(px(12.))
                .text_color(if warn {
                    t.warning.opacity(0.85)
                } else {
                    t.text_faint
                })
                .child(message)
                .into_any_element()
        } else if self.preparing {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(pink::space::S8))
                .child(loaders::gradient_spinner(3.0, t.glyph))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(t.text_faint)
                        .child("Preparing diff…"),
                )
                .into_any_element()
        } else if files.is_empty() {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .text_color(t.text_faint)
                .child(clean_message(self.scope, commit, self.base_ref.as_deref()))
                .into_any_element()
        } else {
            let additions: u32 = files.iter().map(|f| f.additions).sum();
            let deletions: u32 = files.iter().map(|f| f.deletions).sum();
            let label = scope_label(self.scope, commit, files.len(), self.base_ref.as_deref());
            let sections: Vec<AnyElement> = files
                .iter()
                .enumerate()
                .map(|(ix, file)| file_section(&c, ix, file))
                .collect();
            let sticky = sticky_header(&c, &files, &list_scroll);
            let rail = rail_view.map(|(metrics, fade, held)| rail(&c, metrics, fade, held));
            let list = div()
                .id(SharedString::from(format!("{key}-list")))
                .size_full()
                .overflow_y_scroll()
                .track_scroll(&list_scroll)
                .flex()
                .flex_col()
                .children(sections);
            let host = div()
                .id(SharedString::from(format!("{key}-list-host")))
                .relative()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .on_hover({
                    let state = state.clone();
                    move |hovered: &bool, _, cx| {
                        let hovered = *hovered;
                        state.update(cx, |s, cx| {
                            if s.rail.set_list_hovered(hovered, Instant::now()) {
                                cx.notify();
                            }
                        });
                    }
                })
                .child(list)
                .children(sticky)
                .children(rail);
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(header_strip(
                    &t,
                    label,
                    additions,
                    deletions,
                    self.truncated,
                ))
                .child(host)
                .into_any_element()
        };

        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .h(px(self.height))
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .overflow_hidden()
            // The right sidebar sits on the shell's chrome surface.
            .bg(t.chrome_surface())
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(toolbar)
            .when_some(self.error.clone(), |el, message| {
                el.child(
                    div()
                        .flex_none()
                        .px(px(pink::space::S12))
                        .py(px(4.))
                        .border_b_1()
                        .border_color(t.border)
                        .text_size(px(11.))
                        .text_color(t.readable_tone(t.warning, 1.0))
                        .child(message),
                )
            })
            .child(content)
    }
}

struct RefView {
    input: Entity<TextInput>,
    query: SharedString,
    active: usize,
    list: ScrollHandle,
}

/// One mono column at the code size, for the shared horizontal extent.
fn mono_advance(window: &Window) -> f32 {
    let text = window.text_system();
    let id = text.resolve_font(&font(pink::MONO));
    text.ch_advance(id, px(DIFF_TEXT_SIZE))
        .map(f32::from)
        .unwrap_or(DIFF_TEXT_SIZE * 0.6)
}

// ---- toolbar pieces -------------------------------------------------------

/// A pane-header icon button (Pink `header_toggle`): 28 square, r6, a
/// `wash(0.14)` hover; an active toggle holds the wash and the full text
/// tone, so the pane says which layout it is in without a label.
fn header_toggle(
    c: &Ctx,
    name: &str,
    icon: &'static str,
    label: &'static str,
    active: bool,
) -> Stateful<gpui::Div> {
    let t = c.t;
    let id = format!("{}-{name}", c.key);
    let ring = t.focus_ring();
    icon_button(
        id.clone(),
        surface_toolbar::control_glyph(
            svg().path(icon),
            if active {
                t.text
            } else {
                t.readable_muted(0.7)
            },
        )
        .into_any_element(),
        &IconButtonStyle {
            size: surface_toolbar::CONTROL_SIZE,
            radius: surface_toolbar::CONTROL_RADIUS,
            wash: t.wash(if active { 0.14 } else { 0.0 }),
            enabled: true,
        },
    )
    .role(Role::Button)
    .aria_label(label)
    .tab_index(0)
    .focus_visible(move |s| pink::focus_ring(s, ring))
    .when(!active, |el| el.hover_fill(id, t.wash(0.0), t.wash(0.14)))
}

/// The header buttons' tooltip (Pink `DiffHeaderTooltip`): `px 8 / py 6`,
/// 11 in `text`.
struct HeaderTooltip(&'static str);

impl gpui::Render for HeaderTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        pink::tooltip_card(&t)
            .px(px(8.))
            .py(px(6.))
            .text_size(px(11.))
            .text_color(t.text)
            .child(self.0)
    }
}

fn header_tooltip(label: &'static str) -> impl Fn(&mut Window, &mut App) -> AnyView {
    move |_, cx| cx.new(|_| HeaderTooltip(label)).into()
}

/// The scope dropdown's trigger (Pink `changes-scope-trigger`): h 28 px 8
/// gap 6 r6 on `wash(0.05)` easing to `wash(0.14)`, 12/14 label and a 12 px
/// chevron.
fn scope_trigger(c: &Ctx, scope: DiffScope, menu: Option<AnyElement>) -> Stateful<gpui::Div> {
    let t = c.t;
    let id = format!("{}-scope-trigger", c.key);
    let (press, click) = (c.state.clone(), c.state.clone());
    dh::scope_trigger(
        SharedString::from(id.clone()),
        scope.label(),
        svg().path(ICON_DOWN).flex_none(),
        &dh::ScopeTriggerStyle {
            fill: t.wash(0.05),
            text: t.text,
            chevron: t.readable_muted(0.7),
        },
    )
    .role(Role::Button)
    .relative()
    .hover_fill(id, t.wash(0.05), t.wash(0.14))
    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
        press.update(cx, |s, _| {
            s.scope_menu.pressed_while_open = s.scope_menu.mounted()
        });
    })
    .on_click(move |_, _, cx| {
        cx.stop_propagation();
        click.update(cx, |s, cx| {
            if std::mem::take(&mut s.scope_menu.pressed_while_open) {
                return;
            }
            s.scope_menu.open = true;
            s.scope_menu.closing = None;
            s.scope_menu.reap = None;
            cx.notify();
        });
    })
    .children(menu)
}

/// The scope menu (Pink `render_scope_menu`): a 180 popover card of menu
/// rows, the current scope selected.
fn scope_menu_card(
    c: &Ctx,
    current: DiffScope,
    on_scope: Option<ScopeHandler>,
) -> impl IntoElement {
    let t = c.t;
    let close = c.state.clone();
    pink::popover_card(t)
        .w(dh::ui_rems(dh::SCOPE_MENU_WIDTH))
        .flex()
        .flex_col()
        .on_mouse_down_out(move |_, _, cx| close.update(cx, |s, cx| s.close_menu(Menu::Scope, cx)))
        .child(
            dh::scope_list().children(DiffScope::ALL.into_iter().enumerate().map(|(ix, scope)| {
                let state = c.state.clone();
                let on_scope = on_scope.clone();
                pink::menu_row(format!("{}-scope-row-{ix}", c.key), scope == current, t)
                    .on_click(move |_, window, cx| {
                        state.update(cx, |s, cx| s.close_menu(Menu::Scope, cx));
                        if scope != current
                            && let Some(f) = &on_scope
                        {
                            f(scope, window, cx);
                        }
                    })
                    .child(dh::scope_row_label(scope.label()))
            })),
        )
}

/// `{branch} → {base ⌄}` (Pink `render_ref_selector`): both names truncate,
/// the longer one losing more.
fn ref_selector(
    c: &Ctx,
    branch: SharedString,
    base: SharedString,
    menu: Option<AnyElement>,
) -> gpui::Div {
    let t = c.t;
    let id = format!("{}-ref-trigger", c.key);
    let (press, click) = (c.state.clone(), c.state.clone());
    let trigger = dh::ref_trigger(
        SharedString::from(id.clone()),
        base,
        svg().path(ICON_DOWN),
        &dh::RefTriggerStyle {
            mono: pink::MONO.into(),
            fill: t.wash(0.0),
            text: t.text,
            chevron: t.readable_muted(0.7),
        },
    )
    .role(Role::Button)
    .relative()
    .hover_fill(id, t.wash(0.0), t.wash(0.12))
    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
        press.update(cx, |s, _| {
            s.ref_menu.pressed_while_open = s.ref_menu.mounted()
        });
    })
    .on_click(move |_, window, cx| {
        cx.stop_propagation();
        click.update(cx, |s, cx| {
            if std::mem::take(&mut s.ref_menu.pressed_while_open) {
                return;
            }
            s.open_ref_menu(window, cx);
        });
    })
    .children(menu);
    dh::ref_selector(
        branch,
        svg().path(ICON_ARROW_RIGHT),
        trigger,
        &dh::RefSelectorStyle {
            mono: pink::MONO.into(),
            branch: t.text_dim,
            arrow: t.text_faint,
        },
    )
}

/// The base picker (Pink `render_ref_menu`): a 240 popover card with a
/// search field over keyboard-navigable mono rows, the current base checked.
fn ref_menu_card(
    c: &Ctx,
    view: &RefView,
    branches: &[SharedString],
    base: Option<&SharedString>,
) -> impl IntoElement {
    let t = c.t;
    let rows = filter_refs(&view.query, branches);
    let list: AnyElement = if rows.is_empty() {
        dh::ref_list_empty(
            if branches.is_empty() {
                "No branches"
            } else {
                "No matching branches"
            },
            t.text_faint,
        )
        .into_any_element()
    } else {
        dh::ref_list(SharedString::from(format!("{}-ref-list", c.key)))
            .overflow_y_scroll()
            .track_scroll(&view.list)
            .children(rows.into_iter().enumerate().map(|(row, ix)| {
                let name = branches[ix].clone();
                let selected = base == Some(&name);
                let (hover, pick) = (c.state.clone(), c.state.clone());
                pink::menu_row_nav(selected, row == view.active, t)
                    .id(SharedString::from(format!("{}-ref-row-{row}", c.key)))
                    .on_mouse_move(move |_: &MouseMoveEvent, _, cx| {
                        hover.update(cx, |s, cx| {
                            if let Some(search) = s.ref_search.as_mut()
                                && search.active != row
                            {
                                search.active = row;
                                cx.notify();
                            }
                        });
                    })
                    .on_click(move |_, window, cx| {
                        pick.update(cx, |s, cx| s.pick_ref(ix, window, cx));
                    })
                    .child(dh::ref_row_label(name, pink::MONO.into()))
            }))
            .into_any_element()
    };
    let close = c.state.clone();
    pink::popover_card(t)
        .w(dh::ui_rems(dh::REF_MENU_WIDTH))
        .flex()
        .flex_col()
        .on_mouse_down_out(move |_, _, cx| close.update(cx, |s, cx| s.close_menu(Menu::Ref, cx)))
        .child(
            pink::search_input_frame(t).child(
                div()
                    .text_size(px(INPUT_TEXT_SIZE))
                    .line_height(px(INPUT_LINE_HEIGHT))
                    .child(view.input.clone()),
            ),
        )
        .child(list)
}

/// A menu hung from the trigger's bottom-left corner, 10 pt below it and
/// kept 8 pt inside the window (Pink `popover::anchored_menu_below_gap`). It
/// drops 6 pt into place on `MENU_IN` and fades on `MENU_OUT` under an
/// occluder so the dying rows take no clicks.
fn anchored_menu(id: SharedString, exit: Option<f32>, card: impl IntoElement) -> AnyElement {
    let inner = div()
        .occlude()
        .pt(px(MENU_DROP_GAP))
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
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .size_0()
        .child(
            deferred(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                    .child(layer),
            )
            .priority(1),
        )
        .into_any_element()
}

/// Swallows the press that dismisses a menu, so nothing behind it (the
/// trigger included) acts on it too (Pink `frosted_menu_rounded`'s guard).
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

/// The count strip over the list (Pink `render_header_strip`): 38 tall,
/// `px 16`, gap 10, a `hairline(0.06)` rule below; the scope label in 12
/// `text_muted`, then mono `+N −N`.
fn header_strip(
    t: &Pink,
    label: String,
    additions: u32,
    deletions: u32,
    truncated: bool,
) -> gpui::Div {
    dh::summary_strip(t.hairline(0.06))
        .child(dh::summary_label(label, t.text_muted))
        .child(dh::summary_count(
            format!("+{additions}"),
            pink::MONO.into(),
            t.readable_tone(t.diff_add, 1.0),
        ))
        .child(dh::summary_count(
            format!("\u{2212}{deletions}"),
            pink::MONO.into(),
            t.readable_tone(t.diff_del, 1.0),
        ))
        .child(dh::spacer())
        .when(truncated, |el| {
            el.child(dh::partial_badge(
                "Partial snapshot",
                t.warning.opacity(0.08),
                t.readable_tone(t.warning, 0.75),
            ))
        })
}

// ---- file sections --------------------------------------------------------

/// One file: its header, then its body folding open or shut on
/// `DISCLOSURE` (Pink `toggle_fold`: a clipped stand-in tweening its
/// analytic height; wrapped sections settle at once).
fn file_section(c: &Ctx, ix: usize, file: &DiffFile) -> AnyElement {
    let collapsed = c.collapsed.contains(&file.path);
    let spec = if c.wrap {
        DISCLOSURE.with_duration(0)
    } else {
        DISCLOSURE
    };
    let open = motion::follow(&c.fold_key(ix, file), if collapsed { 0. } else { 1. }, spec);
    let section = div()
        .w_full()
        .flex()
        .flex_col()
        .child(file_header(c, ix, file, open, false));
    if open <= 0.001 {
        return section.into_any_element();
    }
    let body = file_body(c, ix, file);
    if open >= 0.999 {
        return section.child(body).into_any_element();
    }
    let full = body_height(file, c.split, c.comments, c.draft_anchor(file));
    section
        .child(
            div()
                .w_full()
                .flex_none()
                .h(px(full * open))
                .overflow_hidden()
                .child(body),
        )
        .into_any_element()
}

/// A file header (Pink `render_file_header`): 38 tall, `px 12` gap 8, an
/// `ink(0.025)` plate easing to `ink(0.05)`; the chevron turns through the
/// fold, then the file-type artwork, the mono path in `text_dim` and the
/// `+N −N` counts. Rows after the first carry a `hairline(0.04)` rule; the
/// sticky copy sits on the opaque content plane with a `border` rule below.
fn file_header(
    c: &Ctx,
    ix: usize,
    file: &DiffFile,
    open: f32,
    sticky: bool,
) -> Stateful<gpui::Div> {
    let t = c.t;
    let (rest, hover) = if sticky {
        (
            pink::flatten(t.ink(0.025), t.bg),
            pink::flatten(t.element_hover, t.bg),
        )
    } else {
        (t.ink(0.025), t.ink(0.05))
    };
    let id = format!(
        "{}-{}-{ix}",
        c.key,
        if sticky {
            "sticky-file-hdr"
        } else {
            "file-hdr"
        }
    );
    let state = c.state.clone();
    let path = file.path.clone();
    let on_fold = c.on_fold.clone();
    let (adds, dels) = (file.additions, file.deletions);
    let mono: SharedString = pink::MONO.into();
    dh::file_header(
        SharedString::from(id.clone()),
        if sticky {
            dh::FileHeaderPlacement::Sticky
        } else {
            dh::FileHeaderPlacement::Row { first: ix == 0 }
        },
        &dh::FileHeaderStyle {
            fill: rest,
            rule: if sticky { t.border } else { t.hairline(0.04) },
        },
    )
    .when(sticky, |el| el.block_mouse_except_scroll())
    .hover_fill(id, rest, hover)
    .on_click(move |_, window, cx| toggle_fold(&state, &path, &on_fold, window, cx))
    .child(dh::disclosure(
        svg().path(ICON_CHEVRON),
        open,
        t.readable_muted(0.7),
    ))
    .child(dh::artwork(img(file_icon(&file.path, t.dark))))
    .child(dh::path(file.path.clone(), mono.clone(), t.text_dim))
    .when(file.binary, |el| {
        el.child(dh::binary_tag("BIN", t.text_faint))
    })
    .when(adds > 0 || !file.binary, |el| {
        el.child(dh::count(
            format!("+{adds}"),
            mono.clone(),
            t.readable_tone(t.diff_add, 1.0),
        ))
    })
    .when(dels > 0 || !file.binary, |el| {
        el.child(dh::count(
            format!("\u{2212}{dels}"),
            mono.clone(),
            t.readable_tone(t.diff_del, 1.0),
        ))
    })
}

/// The header of the file under the list's top edge, held there and pushed
/// up by the next file's header (Pink `render_sticky_file_header`,
/// `sticky_file_header`, `sticky_header_push_offset`).
fn sticky_header(c: &Ctx, files: &[DiffFile], scroll: &ScrollHandle) -> Option<AnyElement> {
    let bounds = scroll.bounds();
    let top = -f32::from(scroll.offset().y);
    if top <= 0.0 {
        return None;
    }
    let count = scroll.children_count().min(files.len());
    let rel = |ix: usize| {
        scroll
            .bounds_for_item(ix)
            .map(|b| f32::from(b.top() - bounds.top()))
    };
    let mut current = None;
    for ix in 0..count {
        match rel(ix) {
            Some(y) if y <= top => current = Some((ix, y)),
            _ => break,
        }
    }
    let (ix, y) = current?;
    if top <= y {
        return None;
    }
    let push = rel(ix + 1)
        .filter(|_| ix + 1 < count)
        .map(|next| (next - top - FILE_HEADER_HEIGHT).min(0.0))
        .unwrap_or(0.0);
    let file = &files[ix];
    let collapsed = c.collapsed.contains(&file.path);
    let spec = if c.wrap {
        DISCLOSURE.with_duration(0)
    } else {
        DISCLOSURE
    };
    let open = motion::follow(&c.fold_key(ix, file), if collapsed { 0. } else { 1. }, spec);
    Some(
        div()
            .absolute()
            .top(px(push))
            .left_0()
            .w_full()
            .child(file_header(c, ix, file, open, true))
            .into_any_element(),
    )
}

/// A file's expanded body: notice rows, then each hunk's band and rows, and
/// an 8 pt pad (Pink `body_rows`).
fn file_body(c: &Ctx, ix: usize, file: &DiffFile) -> gpui::Div {
    let max_columns = file
        .hunks
        .iter()
        .flat_map(|h| &h.lines)
        .map(|l| visual_columns(&l.text))
        .max()
        .unwrap_or(0);
    let fc = FileCx {
        c,
        ix,
        file,
        gutter: gutter_width(file),
        lang: Lang::of(&file.path),
        text_width: max_columns as f32 * c.advance,
        scroll: c.hscroll.get(&file.path).cloned(),
    };
    let style = diff_rows::diff_style(c.t);
    let mut body = div().w_full().flex().flex_col();
    for notice in file_notices(file) {
        body = body.child(diff_rows::notice_row(notice, &style));
    }
    for (h, hunk) in file.hunks.iter().enumerate() {
        body = body.child(diff_rows::hunk_header_row(hunk.header.clone(), &style));
        if c.split {
            for (p, pair) in split_pairs(&hunk.lines).into_iter().enumerate() {
                body = body.child(split_line(&fc, h, p, &hunk.lines, pair));
                for anchor in pair_anchors(&hunk.lines, pair).into_iter().flatten() {
                    body = body.children(cards_at(&fc, anchor));
                }
            }
        } else {
            for (l, line) in hunk.lines.iter().enumerate() {
                body = body.child(unified_line(&fc, h, l, hunk, line));
                if let Some(anchor) = line_anchor(line) {
                    body = body.children(cards_at(&fc, anchor));
                }
            }
        }
    }
    body.child(div().w_full().flex_none().h(px(BODY_BOTTOM_PAD)))
}

/// The only part of a row allowed past its viewport (Pink
/// `code_text_viewport`): the code plane, sized to the file's widest line and
/// scrolled sideways by the file's shared handle, or wrapped.
fn code_viewport(
    c: &Ctx,
    text: &SharedString,
    runs: Vec<TextRun>,
    pad_left: f32,
    content_width: f32,
    slot: Option<(SharedString, ScrollHandle)>,
) -> AnyElement {
    let wrapped = c.wrap;
    let content = div()
        .map(|el| {
            if wrapped {
                el.w_full().min_w_0().whitespace_normal()
            } else {
                el.w(px(content_width))
                    .flex_none()
                    .overflow_hidden()
                    .whitespace_nowrap()
            }
        })
        .pl(px(pad_left))
        .font_family(pink::MONO)
        .text_size(px(DIFF_TEXT_SIZE))
        .line_height(px(DIFF_LINE_HEIGHT))
        .child(StyledText::new(text.clone()).with_runs(runs));
    let viewport = div()
        .flex_1()
        .min_w_0()
        .min_h(px(DIFF_LINE_HEIGHT))
        .overflow_hidden()
        .child(content);
    if wrapped {
        return viewport.into_any_element();
    }
    match slot {
        Some((id, handle)) => {
            let mut viewport = viewport.id(id).overflow_x_scroll().track_scroll(&handle);
            // A vertical wheel must reach the list, not scroll code sideways.
            viewport.style().restrict_scroll_to_axis = Some(true);
            viewport.into_any_element()
        }
        None => viewport.into_any_element(),
    }
}

/// One unified row (Pink `diff_line_row`): the 3 px spacer, old and new
/// gutters, the marker and the code on the row's wash, changed words washed
/// stronger. A row a comment can cite shows the `+` adder while hovered.
fn unified_line(fc: &FileCx, h: usize, l: usize, hunk: &Hunk, line: &DiffLine) -> AnyElement {
    let c = fc.c;
    let t = c.t;
    let style = diff_rows::diff_style(t);
    if line.kind == LineKind::Meta {
        return diff_rows::meta_line_row(
            line.text.clone(),
            diff_rows::unified_meta_pad(fc.gutter, UNIFIED_CODE_PADDING_LEFT),
            &style,
        )
        .into_any_element();
    }
    let paint = row_paint(line.kind, t);
    let runs = syntax_runs(&line.text, fc.lang, c.plain, &c.syntax, &c.mono);
    let runs = match line.kind {
        LineKind::Add => {
            with_word_backgrounds(runs, &word_changes(hunk, l), diff_word_wash(t.diff_add, t))
        }
        LineKind::Del => {
            with_word_backgrounds(runs, &word_changes(hunk, l), diff_word_wash(t.diff_del, t))
        }
        _ => runs,
    };
    let faint = t.readable_faint(0.8);
    let row = diff_rows::unified_row(paint.wash, c.wrap, &style)
        .child(diff_rows::accent_bar(None))
        .child(diff_rows::gutter(
            line.old_no,
            if line.kind == LineKind::Del {
                paint.number
            } else {
                faint
            },
            fc.gutter,
            &style,
        ))
        .child(diff_rows::gutter(
            line.new_no,
            if line.kind == LineKind::Add {
                paint.number
            } else {
                faint
            },
            fc.gutter,
            &style,
        ))
        .child(diff_rows::marker(
            paint.marker,
            paint.marker_color,
            diff_rows::MARKER_WIDTH,
            &style,
        ))
        .child(code_viewport(
            c,
            &line.text,
            runs,
            UNIFIED_CODE_PADDING_LEFT,
            fc.text_width + UNIFIED_CODE_PADDING_LEFT + CODE_PADDING_RIGHT,
            fc.slot(&format!("u{h}-{l}")),
        ));
    let Some((side, number)) = line_anchor(line) else {
        return row.into_any_element();
    };
    let at = Hover {
        path: fc.file.path.clone(),
        side,
        line: number,
    };
    let hovered = c.hover == Some(&at);
    let (enter, leave) = (c.state.clone(), c.state.clone());
    let leave_at = at.clone();
    div()
        .id(SharedString::from(format!(
            "{}-line-{}-{h}-{l}",
            c.key, fc.ix
        )))
        .w_full()
        .relative()
        .child(row)
        .when(hovered, |el| {
            let column = match side {
                CommentSide::Old => 0.0,
                CommentSide::New => fc.gutter,
            };
            el.child(positioned_adder(
                diff_rows::ACCENT_BAR_WIDTH + column + (fc.gutter - COMMENT_ADDER_SIZE) / 2.0,
                comment_adder(fc, side, number),
            ))
        })
        .on_mouse_move(move |_: &MouseMoveEvent, _, cx| set_hover(&enter, at.clone(), cx))
        .on_hover(move |hovered: &bool, _, cx| {
            if !*hovered {
                clear_hover(&leave, &leave_at, cx);
            }
        })
        .into_any_element()
}

/// One half of a split row (Pink `split_line_cell`): the unified columns
/// minus the second gutter, each half numbering its own side.
fn split_cell(fc: &FileCx, line: &DiffLine, number: Option<u32>, slot: &str) -> gpui::Div {
    let c = fc.c;
    let paint = row_paint(line.kind, c.t);
    let style = diff_rows::diff_style(c.t);
    let runs = syntax_runs(&line.text, fc.lang, c.plain, &c.syntax, &c.mono);
    diff_rows::split_half(paint.wash)
        .child(diff_rows::accent_bar(None))
        .child(diff_rows::gutter(number, paint.number, fc.gutter, &style))
        .child(diff_rows::marker(
            paint.marker,
            paint.marker_color,
            diff_rows::SPLIT_MARKER_WIDTH,
            &style,
        ))
        .child(code_viewport(
            c,
            &line.text,
            runs,
            SPLIT_CODE_PADDING_LEFT,
            fc.text_width + SPLIT_CODE_PADDING_LEFT + CODE_PADDING_RIGHT,
            fc.slot(slot),
        ))
}

/// One split row (Pink `SplitLine` + `split_row`): old on the left, new on
/// the right around a `hairline(0.06)` divider. Only the right column takes
/// a `+`: notes cite the file the agent edits.
fn split_line(fc: &FileCx, h: usize, p: usize, lines: &[DiffLine], pair: LinePair) -> AnyElement {
    let c = fc.c;
    let t = c.t;
    let style = diff_rows::diff_style(t);
    let left = pair.0.and_then(|ix| lines.get(ix));
    let right = pair.1.and_then(|ix| lines.get(ix));
    if let Some(meta) = [left, right]
        .into_iter()
        .flatten()
        .find(|line| line.kind == LineKind::Meta)
    {
        return diff_rows::meta_line_row(
            meta.text.clone(),
            diff_rows::split_meta_pad(fc.gutter),
            &style,
        )
        .into_any_element();
    }
    let left = match left {
        Some(line) => split_cell(fc, line, line.old_no, &format!("o{h}-{p}")).into_any_element(),
        None => diff_rows::split_filler(&style).into_any_element(),
    };
    let right = match right {
        Some(line) => {
            let cell = split_cell(fc, line, line.new_no, &format!("n{h}-{p}"));
            match line_anchor(line) {
                Some((side, number)) => {
                    let at = Hover {
                        path: fc.file.path.clone(),
                        side,
                        line: number,
                    };
                    let hovered = c.hover == Some(&at);
                    let (enter, leave) = (c.state.clone(), c.state.clone());
                    let leave_at = at.clone();
                    cell.id(SharedString::from(format!(
                        "{}-split-{}-{h}-{p}",
                        c.key, fc.ix
                    )))
                    .relative()
                    .when(hovered, |el| {
                        el.child(positioned_adder(
                            diff_rows::ACCENT_BAR_WIDTH + (fc.gutter - COMMENT_ADDER_SIZE) / 2.0,
                            comment_adder(fc, side, number),
                        ))
                    })
                    .on_mouse_move(move |_: &MouseMoveEvent, _, cx| {
                        set_hover(&enter, at.clone(), cx)
                    })
                    .on_hover(move |hovered: &bool, _, cx| {
                        if !*hovered {
                            clear_hover(&leave, &leave_at, cx);
                        }
                    })
                    .into_any_element()
                }
                None => cell.into_any_element(),
            }
        }
        None => diff_rows::split_filler(&style).into_any_element(),
    };
    diff_rows::split_row(left, right, c.wrap, &style).into_any_element()
}

// ---- review comments ------------------------------------------------------

fn positioned_adder(left: f32, adder: impl IntoElement) -> gpui::Div {
    div()
        .absolute()
        .left(px(left))
        .top_0()
        .h_full()
        .flex()
        .items_center()
        .child(adder)
}

/// Pink's comment adder (comment_ui.rs `render_comment_adder`): a 16 px r4
/// `solid` plate holding an 11 px plus in `on_solid`, centered in the gutter
/// of the side it cites.
fn comment_adder(fc: &FileCx, side: CommentSide, line: u32) -> Stateful<gpui::Div> {
    let c = fc.c;
    let t = c.t;
    let state = c.state.clone();
    let target = Target {
        path: fc.file.path.clone(),
        old_path: fc.file.old_path.clone(),
        side,
        line,
    };
    let tag = match side {
        CommentSide::Old => "L",
        CommentSide::New => "R",
    };
    cc::adder(t.solid, svg().path(ICON_PLUS), t.on_solid)
        .id(SharedString::from(format!(
            "{}-cmt-add-{}-{tag}-{line}",
            c.key, fc.ix
        )))
        .role(Role::Button)
        .aria_label("Add comment")
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            let target = target.clone();
            state.update(cx, |s, cx| s.open_draft(target, None, window, cx));
        })
}

/// The staged cards and the open draft anchored to one line.
fn cards_at(fc: &FileCx, anchor: (CommentSide, u32)) -> Vec<AnyElement> {
    let c = fc.c;
    let mut out: Vec<AnyElement> = c
        .comments
        .iter()
        .enumerate()
        .filter(|(_, comment)| comment.path == fc.file.path && comment.anchor() == anchor)
        .map(|(ix, comment)| comment_card(c, ix, comment))
        .collect();
    if let Some(draft) = c.draft
        && draft.target.path == fc.file.path
        && (draft.target.side, draft.target.line) == anchor
    {
        out.push(draft_card(c, draft));
    }
    out
}

/// A staged review comment (Pink: comment_ui.rs `render_comment_card`): an
/// `ink(0.05)` band with a `solid @ 0.35` bar, the heading, the note at
/// 12/18 in `text_dim`, and edit / remove that show on hover.
fn comment_card(c: &Ctx, ix: usize, comment: &DiffComment) -> AnyElement {
    let t = c.t;
    let group = SharedString::from(format!("{}-cmt-{ix}", c.key));
    let (edit, remove) = (c.state.clone(), c.state.clone());
    let target = Target {
        path: comment.path.clone(),
        old_path: comment.old_path.clone(),
        side: comment.side,
        line: comment.line,
    };
    let edit_button = cc::edit(svg().path(ICON_PEN), t.text_muted, group.clone())
        .id(SharedString::from(format!("{}-cmt-edit-{ix}", c.key)))
        .role(Role::Button)
        .aria_label("Edit comment")
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            let target = target.clone();
            edit.update(cx, |s, cx| s.open_draft(target, Some(ix), window, cx));
        });
    let remove_button = cc::remove(svg().path(ICON_REMOVE), t.text_muted, group.clone())
        .id(SharedString::from(format!("{}-cmt-remove-{ix}", c.key)))
        .role(Role::Button)
        .aria_label("Remove comment")
        .on_click(move |_, window, cx| {
            remove.update(cx, |s, cx| s.remove_comment(ix, window, cx));
        });
    cc::card(&cc::posted_style(t), card_height(&comment.body), None)
        .group(group)
        .child(
            cc::content(None)
                .child(
                    cc::heading(comment.location(), t)
                        .child(edit_button)
                        .child(remove_button),
                )
                .child(cc::body(comment.body.clone(), t.text_dim)),
        )
        .into_any_element()
}

/// The open draft (Pink: comment_ui.rs `render_comment_draft`): 116 tall on
/// `ink(0.08)` with a firmer `solid @ 0.7` bar, the heading, a 46 px field
/// and Cancel / Comment (Save when editing). ⏎ stages, Esc cancels.
fn draft_card(c: &Ctx, draft: &DraftView) -> AnyElement {
    let t = c.t;
    let (cancel, commit) = (c.state.clone(), c.state.clone());
    let location: SharedString = {
        let path = match draft.target.side {
            CommentSide::Old => draft.target.old_path.as_ref().unwrap_or(&draft.target.path),
            CommentSide::New => &draft.target.path,
        };
        format!("{path}:{}", draft.target.line).into()
    };
    cc::draft(&cc::draft_style(t), DRAFT_CARD_HEIGHT, None)
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(
            cc::content(None)
                .child(cc::heading(location, t))
                // Pink's composer input type, which its draft keeps.
                .child(cc::field(
                    div()
                        .text_size(px(INPUT_TEXT_SIZE))
                        .line_height(px(INPUT_LINE_HEIGHT))
                        .text_color(t.text)
                        .child(draft.input.clone()),
                ))
                .child(
                    cc::actions()
                        .child(comment_action(c, "cancel", "Cancel", false).on_click(
                            move |_, _, cx| {
                                cancel.update(cx, |s, cx| {
                                    s.draft = None;
                                    cx.notify();
                                })
                            },
                        ))
                        .child(
                            comment_action(
                                c,
                                "commit",
                                if draft.editing { "Save" } else { "Comment" },
                                true,
                            )
                            .on_click(move |_, window, cx| {
                                commit.update(cx, |s, cx| s.commit_draft(window, cx))
                            }),
                        ),
                ),
        )
        .into_any_element()
}

/// A draft action (Pink `comment_action`): the primary on the `solid`
/// plate, the other a quiet ghost brightening under the pointer.
fn comment_action(c: &Ctx, name: &str, label: &'static str, primary: bool) -> Stateful<gpui::Div> {
    let id = format!("{}-cmt-{name}", c.key);
    let el = cc::action(&cc::action_style(&id, primary, c.t), label)
        .id(SharedString::from(id.clone()))
        .role(Role::Button);
    if primary {
        el
    } else {
        el.on_hover(motion::hover_listener(id))
    }
}

// ---- floating scrollbar ---------------------------------------------------

/// Pink's floating list scrollbar (popover.rs `rail`, `render_rail`): a
/// 10 px hit strip on the right holding a 3 px thumb (5 px while hovered or
/// dragged) in `text_faint`, shown while scrolling and fading out after.
fn rail(c: &Ctx, metrics: RailMetrics, fade: f32, held: bool) -> AnyElement {
    let t = c.t;
    let (hover, press, drag, up, up_out) = (
        c.state.clone(),
        c.state.clone(),
        c.state.clone(),
        c.state.clone(),
        c.state.clone(),
    );
    scroll_rail::rail(t.text_faint, fade, held, metrics.thumb_top, metrics.thumb)
        .id(SharedString::from(format!("{}-rail", c.key)))
        .on_hover(move |hovered: &bool, _, cx| {
            let hovered = *hovered;
            hover.update(cx, |s, cx| {
                if s.rail.set_bar_hovered(hovered, Instant::now()) {
                    cx.notify();
                }
            });
        })
        .on_mouse_down(MouseButton::Left, move |event: &MouseDownEvent, _, cx| {
            let y = f32::from(event.position.y);
            if press.update(cx, |s, cx| {
                let pressed = s.rail_press(y);
                cx.notify();
                pressed
            }) {
                cx.stop_propagation();
            }
        })
        .on_drag(RailDrag, |_, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| EmptyView)
        })
        .on_drag_move::<RailDrag>(move |event: &DragMoveEvent<RailDrag>, _, cx| {
            let y = f32::from(event.event.position.y);
            drag.update(cx, |s, cx| {
                if s.rail_drag(y) {
                    cx.notify();
                }
            });
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            up.update(cx, |s, cx| {
                s.rail.end_press(Instant::now());
                cx.notify();
            });
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            up_out.update(cx, |s, cx| {
                s.rail.end_press(Instant::now());
                cx.notify();
            });
        })
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

const CARGO_TOML: &str = r#"diff --git a/crates/sync/Cargo.toml b/crates/sync/Cargo.toml
index 6c1d0a2..b84e3f1 100644
--- a/crates/sync/Cargo.toml
+++ b/crates/sync/Cargo.toml
@@ -8,5 +8,6 @@ publish = false
 [dependencies]
 anyhow = "1"
 serde = { version = "1", features = ["derive"] }
+fastrand = "2"
 tokio = { version = "1", features = ["time", "sync"] }
 tracing = "0.1""#;

const BACKOFF_RS: &str = r#"diff --git a/crates/sync/src/backoff.rs b/crates/sync/src/backoff.rs
index 3f2a1c4..9b7e0d2 100644
--- a/crates/sync/src/backoff.rs
+++ b/crates/sync/src/backoff.rs
@@ -1,9 +1,10 @@
 use std::time::Duration;

 use crate::config::SyncConfig;
+use crate::retry_policy::RetryPolicy;

 /// How long to wait before the next sync attempt.
 pub struct Backoff {
-    base: Duration,
+    policy: RetryPolicy,
     attempt: u32,
 }
@@ -24,7 +25,8 @@ impl Backoff {
     /// Wait before retry `attempt`, growing with each failure.
     pub fn next_delay(&mut self) -> Duration {
-        let step = self.base * self.attempt;
-        let cap = Duration::from_secs(60);
+        let step = self.policy.base() * 2u32.pow(self.attempt.min(6));
+        let cap = Duration::from_secs(self.policy.cap_secs);
+        let jitter = step.mul_f32(fastrand::f32() * 0.2);
         self.attempt += 1;
-        step.min(cap)
+        (step + jitter).min(cap)
     }"#;

const BUDGET_RS: &str = r#"diff --git a/crates/sync/src/limits.rs b/crates/sync/src/budget.rs
similarity index 88%
rename from crates/sync/src/limits.rs
rename to crates/sync/src/budget.rs
index 41d2e7a..c03b9f5 100644
--- a/crates/sync/src/limits.rs
+++ b/crates/sync/src/budget.rs
@@ -3,6 +3,6 @@
 /// Caps how much work one sync pass may do.
 pub struct Budget {
     pub max_batch: usize,
-    pub max_retries: u32,
+    pub max_attempts: u32,
     pub deadline: Duration,
 }"#;

const CONFIG_RS: &str = r#"diff --git a/crates/sync/src/config.rs b/crates/sync/src/config.rs
index 9a8b7c6..1d2e3f4 100644
--- a/crates/sync/src/config.rs
+++ b/crates/sync/src/config.rs
@@ -12,4 +12,6 @@ pub struct SyncConfig {
     pub interval: Duration,
     pub workspace: PathBuf,
+    #[serde(default)]
+    pub retry: RetryPolicy,
     pub offline_queue: bool,
 }"#;

const RETRY_POLICY_RS: &str = r#"diff --git a/crates/sync/src/retry_policy.rs b/crates/sync/src/retry_policy.rs
new file mode 100644
index 0000000..5e8f2b1
--- /dev/null
+++ b/crates/sync/src/retry_policy.rs
@@ -0,0 +1,18 @@
+use std::time::Duration;
+
+use serde::Deserialize;
+
+/// Retry timing, read from `[sync.retry]` in the workspace config.
+#[derive(Clone, Debug, Deserialize)]
+pub struct RetryPolicy {
+    /// First delay, doubled on every failed attempt.
+    pub base_ms: u64,
+    /// Upper bound on any single wait.
+    pub cap_secs: u64,
+}
+
+impl RetryPolicy {
+    pub fn base(&self) -> Duration {
+        Duration::from_millis(self.base_ms)
+    }
+}"#;

const BACKOFF_TEST_RS: &str = r#"diff --git a/crates/sync/tests/backoff.rs b/crates/sync/tests/backoff.rs
new file mode 100644
index 0000000..d4e5f6a
--- /dev/null
+++ b/crates/sync/tests/backoff.rs
@@ -0,0 +1,12 @@
+use std::time::Duration;
+
+use sync::{backoff::Backoff, retry_policy::RetryPolicy};
+
+#[test]
+fn delays_double_until_the_cap() {
+    let policy = RetryPolicy { base_ms: 100, cap_secs: 1 };
+    let mut backoff = Backoff::new(policy);
+    assert!(backoff.next_delay() >= Duration::from_millis(100));
+    let late = (0..6).map(|_| backoff.next_delay()).last().unwrap();
+    assert!(late <= Duration::from_millis(1200));
+}"#;

const TIMELINE_PNG: &str = r#"diff --git a/docs/img/retry-timeline.png b/docs/img/retry-timeline.png
new file mode 100644
index 0000000..a1b2c3d
Binary files /dev/null and b/docs/img/retry-timeline.png differ"#;

const SYNC_MD: &str = r#"diff --git a/docs/sync.md b/docs/sync.md
index 77ad01e..e4c9a20 100644
--- a/docs/sync.md
+++ b/docs/sync.md
@@ -18,5 +18,9 @@ Each pass pulls remote changes, then pushes local ones.
 ## Retries

-A failed pass waits `attempt` seconds before trying again, up to a minute.
+A failed pass waits `base_ms` and doubles the wait on every failure, up to
+`cap_secs`. Each wait adds up to 20% random jitter so clients that failed
+together don't retry in lockstep.
+
+![Retry timeline](img/retry-timeline.png)

 Set `SYNC_RETRY=0` to disable retries while debugging."#;

const CLIENT_TS: &str = r#"diff --git a/web/src/api/client.ts b/web/src/api/client.ts
index 0b3f9de..7c21a44 100644
--- a/web/src/api/client.ts
+++ b/web/src/api/client.ts
@@ -41,7 +41,13 @@ export async function syncNow(workspace: string): Promise<SyncResult> {
   const res = await fetch(`/api/workspaces/${workspace}/sync`, { method: "POST" });
   if (!res.ok) {
-    throw new Error(`sync failed: ${res.status}`);
+    const retryAfter = Number(res.headers.get("Retry-After") ?? 0);
+    throw new SyncError(res.status, retryAfter * 1000);
   }
   return (await res.json()) as SyncResult;
 }

+export class SyncError extends Error {
+  constructor(readonly status: number, readonly retryInMs: number) {
+    super(`sync failed: ${status}`);
+  }
+}"#;

const CHANGELOG_MD: &str = r#"diff --git a/CHANGELOG.md b/CHANGELOG.md
index 1e2f3a4..5b6c7d8 100644
--- a/CHANGELOG.md
+++ b/CHANGELOG.md
@@ -1,5 +1,9 @@
 # Changelog

+## Unreleased
+
+- Sync retries back off exponentially with jitter instead of linearly.
+
 ## 2.4.0

 - Offline queue survives app restarts."#;

/// The demo's patch for a scope: the working tree (with the agent's next
/// edit once it lands), the branch over its base, or the last turn.
fn demo_patch(scope: DiffScope, landed: bool) -> String {
    let files: Vec<&str> = match scope {
        DiffScope::WorkingTree => {
            let mut files = vec![CARGO_TOML, BACKOFF_RS, BUDGET_RS, RETRY_POLICY_RS];
            if landed {
                files.push(BACKOFF_TEST_RS);
            }
            files.extend([TIMELINE_PNG, SYNC_MD, CLIENT_TS]);
            files
        }
        DiffScope::Branch => vec![
            CHANGELOG_MD,
            CARGO_TOML,
            BACKOFF_RS,
            BUDGET_RS,
            CONFIG_RS,
            RETRY_POLICY_RS,
            BACKOFF_TEST_RS,
            TIMELINE_PNG,
            SYNC_MD,
            CLIENT_TS,
        ],
        DiffScope::LatestTurn => vec![BACKOFF_RS, RETRY_POLICY_RS],
    };
    files.join("\n")
}

/// The demo host: the scope and base it shows, a fetch that takes a beat,
/// and the agent's next edit arriving on the live working tree.
struct Demo {
    scope: DiffScope,
    base: SharedString,
    ready: bool,
    landed: bool,
    _load: Option<Task<()>>,
    _live: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let live = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(5200))
                .await;
            this.update(cx, |demo, cx| {
                demo.landed = true;
                cx.notify();
            })
            .ok();
        });
        let mut demo = Self {
            scope: DiffScope::WorkingTree,
            base: "main".into(),
            ready: false,
            landed: false,
            _load: None,
            _live: live,
        };
        demo.load(cx);
        demo
    }

    /// The pane shows Preparing until the capture for the scope arrives.
    fn load(&mut self, cx: &mut Context<Self>) {
        self.ready = false;
        self._load = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1100))
                .await;
            this.update(cx, |demo, cx| {
                demo.ready = true;
                cx.notify();
            })
            .ok();
        }));
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo =
            window.use_keyed_state(ElementId::Name("diff-viewer-demo".into()), cx, |_, cx| {
                Demo::new(cx)
            });
        let (scope, base, ready, landed) = {
            let d = demo.read(cx);
            (d.scope, d.base.clone(), d.ready, d.landed)
        };
        let on_scope = demo.clone();
        let on_base = demo.clone();
        DiffViewer::new("diff-viewer")
            .patch(demo_patch(scope, landed))
            .scope(scope)
            .preparing(!ready)
            .branch("sync/backoff-jitter")
            .base_ref(base)
            .branches([
                "main",
                "release/2.4",
                "sync/backoff-jitter",
                "feat/offline-queue",
            ])
            .collapsed(["crates/sync/Cargo.toml", "docs/sync.md"])
            .comment(DiffComment::new(
                "crates/sync/src/backoff.rs",
                CommentSide::New,
                29,
                "Take the RNG from SyncConfig here so the retry tests stay deterministic.",
            ))
            .on_git(|_, _| {})
            .on_scope(move |scope, _, cx| {
                on_scope.update(cx, |demo, cx| {
                    demo.scope = scope;
                    demo.load(cx);
                    cx.notify();
                });
            })
            .on_base_ref(move |base, _, cx| {
                on_base.update(cx, |demo, cx| {
                    demo.base = base.clone();
                    demo.load(cx);
                    cx.notify();
                });
            })
            .width(780.)
            .height(980.)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every demo hunk's header counts match its lines.
    #[test]
    fn demo_hunks_are_well_formed() {
        for scope in DiffScope::ALL {
            for file in parse_patch(&demo_patch(scope, true)) {
                for hunk in &file.hunks {
                    let counts = |marker: char| -> u32 {
                        let at = hunk.header.find(marker).unwrap();
                        let spec = hunk.header[at + 1..].split_whitespace().next().unwrap();
                        spec.split(',').nth(1).map_or(1, |n| n.parse().unwrap())
                    };
                    let old = hunk
                        .lines
                        .iter()
                        .filter(|l| matches!(l.kind, LineKind::Context | LineKind::Del))
                        .count() as u32;
                    let new = hunk
                        .lines
                        .iter()
                        .filter(|l| matches!(l.kind, LineKind::Context | LineKind::Add))
                        .count() as u32;
                    assert_eq!(
                        (old, new),
                        (counts('-'), counts('+')),
                        "{} {}",
                        file.path,
                        hunk.header
                    );
                }
            }
        }
    }

    #[test]
    fn parses_statuses_paths_and_counts() {
        let files = parse_patch(&demo_patch(DiffScope::WorkingTree, true));
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_ref()).collect();
        assert_eq!(
            paths,
            [
                "crates/sync/Cargo.toml",
                "crates/sync/src/backoff.rs",
                "crates/sync/src/budget.rs",
                "crates/sync/src/retry_policy.rs",
                "crates/sync/tests/backoff.rs",
                "docs/img/retry-timeline.png",
                "docs/sync.md",
                "web/src/api/client.ts",
            ]
        );
        let backoff = &files[1];
        assert_eq!((backoff.additions, backoff.deletions), (6, 4));
        assert_eq!(backoff.hunks[1].lines[2].new_no, None);
        assert_eq!(backoff.hunks[1].lines[4].new_no, Some(27));
        let budget = &files[2];
        assert_eq!(budget.status, FileStatus::Renamed);
        assert_eq!(
            budget.old_path.as_deref(),
            Some("crates/sync/src/limits.rs")
        );
        assert_eq!(files[3].status, FileStatus::Added);
        assert!(files[5].binary);
        assert_eq!(
            file_notices(&files[5]),
            vec![
                SharedString::from("New file"),
                "Binary file, contents not shown".into()
            ]
        );
    }

    #[test]
    fn split_pairs_align_edits_and_strand_the_rest() {
        let lines = vec![
            DiffLine::context(1, 1, "a"),
            DiffLine::del(2, "b"),
            DiffLine::del(3, "c"),
            DiffLine::add(2, "B"),
            DiffLine::context(4, 3, "d"),
            DiffLine::add(4, "e"),
        ];
        assert_eq!(
            split_pairs(&lines),
            vec![
                (Some(0), Some(0)),
                (Some(1), Some(3)),
                (Some(2), None),
                (Some(4), Some(4)),
                (None, Some(5)),
            ]
        );
    }

    #[test]
    fn word_changes_mark_the_changed_words() {
        let hunk = Hunk::new("@@ -1 +1 @@").lines([
            DiffLine::del(1, "let cap = Duration::from_secs(60);"),
            DiffLine::add(1, "let cap = Duration::from_secs(self.policy.cap_secs);"),
        ]);
        let old = word_changes(&hunk, 0);
        assert_eq!(
            &hunk.lines[0].text[old[0].clone()],
            "Duration::from_secs(60);"
        );
        let new = word_changes(&hunk, 1);
        assert_eq!(
            &hunk.lines[1].text[new[0].clone()],
            "Duration::from_secs(self.policy.cap_secs);"
        );
        // A wholly rewritten line earns no word layer.
        let rewrite =
            Hunk::new("@@ -1 +1 @@").lines([DiffLine::del(1, "alpha"), DiffLine::add(1, "omega")]);
        assert!(word_changes(&rewrite, 1).is_empty());
    }

    #[test]
    fn body_height_counts_rows_cards_and_pad() {
        let file = DiffFile::new("a.rs").hunk(Hunk::new("@@ -1,2 +1,2 @@").lines([
            DiffLine::del(1, "x"),
            DiffLine::add(1, "y"),
            DiffLine::context(2, 2, "z"),
        ]));
        let unified = body_height(&file, false, &[], None);
        assert_eq!(
            unified,
            HUNK_HEADER_HEIGHT + 3.0 * DIFF_LINE_HEIGHT + BODY_BOTTOM_PAD
        );
        let split = body_height(&file, true, &[], None);
        assert_eq!(
            split,
            HUNK_HEADER_HEIGHT + 2.0 * DIFF_LINE_HEIGHT + BODY_BOTTOM_PAD
        );
        let note = DiffComment::new("a.rs", CommentSide::New, 1, "Rename y");
        let with_card = body_height(&file, false, &[note], Some((CommentSide::New, 2)));
        assert_eq!(
            with_card,
            unified + card_height("Rename y") + DRAFT_CARD_HEIGHT
        );
    }

    #[test]
    fn labels_follow_the_scope() {
        assert_eq!(
            scope_label(DiffScope::WorkingTree, false, 1, None),
            "1 Uncommitted change"
        );
        assert_eq!(
            scope_label(DiffScope::Branch, false, 3, Some("main")),
            "3 Changed files vs main"
        );
        assert_eq!(
            scope_label(DiffScope::LatestTurn, true, 2, None),
            "2 Changed files in this commit"
        );
        assert_eq!(
            clean_message(DiffScope::Branch, false, Some("main")),
            "No changes vs main"
        );
        assert_eq!(
            filter_refs("rel", &["main".into(), "release/2.4".into()]),
            vec![1]
        );
        assert_eq!(
            filter_refs("in", &["main".into(), "integration".into()]),
            vec![1, 0]
        );
    }

    #[test]
    fn rail_lingers_then_fades() {
        let start = Instant::now();
        let mut rail = Rail::default();
        rail.note(0.0, start);
        assert!(!rail.visible(start), "the first offset is a baseline");
        rail.note(40.0, start);
        assert!(rail.visible(start));
        assert_eq!(rail.fade(start + Duration::from_millis(1000)), 1.0);
        let mid = rail.fade(start + RAIL_LINGER + RAIL_FADE / 2);
        assert!(mid > 0.4 && mid < 0.6);
        assert!(!rail.visible(start + RAIL_LINGER + RAIL_FADE));
        let metrics = RailMetrics::from_viewport(400.0, 400.0, 200.0).unwrap();
        assert_eq!(metrics.track, 392.0);
        assert_eq!(metrics.thumb, 196.0);
        assert_eq!(metrics.thumb_top, 98.0);
    }
}
