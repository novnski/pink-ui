//! Inline Diff – an agent's edits to one file, reviewed where they sit.
//!
//! The diff is Pink's inline tool diff (Pink: transcript.rs `detail_body`,
//! `ToolDetail::Diff`): a framed code surface whose 28 px header names the
//! file with its `+N −N`, a hunk band, and unified rows from the changes
//! pane – the old line on a red wash above the new one on a green wash, the
//! changed words washed stronger, dual line-number gutters and a `+ − ·`
//! marker column.
//!
//! Pink records edits; this component also reviews them. A card in the
//! shape of Pink's inline review comment opens under the edit in focus with
//! the agent's note and Keep / Drop. `Y` / `N` decide, `J` / `K` (or the
//! arrows) move between open edits and `U` undoes the last decision. A
//! decided edit settles into a plain line: the losing row folds away and the
//! survivor's wash fades out. The footer keeps or drops everything at once
//! and applies the review.
//!
//! The source uses `git diff --word-diff` markers: `[-old-]{+new+}` is a
//! replacement, a lone `[-old-]` a deletion and a lone `{+new+}` an insertion.
//!
//! ```ignore
//! InlineDiff::new("retry", "src/sync/backoff.rs", "let tries = [-3-]{+5+};")
//!     .notes(["Retry five times before giving up"])
//!     .on_apply(|decisions, _, _| { /* write the kept edits */ })
//! ```

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, AnyView, App, ElementId, FocusHandle, Font, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseButton, Role, ScrollHandle, SharedString, StyledText, TextRun, Window, div,
    font, prelude::*, px, svg,
};

use crate::{
    components::diff_rows,
    motion::{self, DISCLOSURE, HOVER_FADE, TOOL_BODY_FADE},
    pink::{
        self, ButtonKind, ControlSize, Pink, Syntax, TEXT_CONTRAST_TARGET, layout, loaders, radius,
    },
    story,
};

pub const NAME: &str = "Inline Diff";
pub const DESCRIPTION: &str = "An agent's edits to a file as Pink's inline diff: old and new lines on red and green washes with the changed words marked, a review card under the edit in focus to keep or drop it with Y or N, and a footer that settles the rest and applies the review";

const ICON_NOTE: &str = "icons/inline-diff/chat-round-line.svg";
const ICON_UNDO: &str = "icons/inline-diff/undo-left.svg";

// Diff metrics (Pink: changes.rs layout numbers).
/// Code type size and row height (`DIFF_TEXT_SIZE`, `DIFF_LINE_HEIGHT`).
const DIFF_TEXT_SIZE: f32 = 12.0;
const DIFF_LINE_HEIGHT: f32 = 21.0;
// The row chrome (the hunk band, gutters, marker) is the `pink-diff-rows`
// package's.
const BODY_BOTTOM_PAD: f32 = 8.0;
/// One line-number column, never narrower than this (`GUTTER_WIDTH`).
const GUTTER_WIDTH: f32 = 36.0;
const GUTTER_PAD_RIGHT: f32 = diff_rows::GUTTER_PAD_RIGHT;
/// A review card's bar matches the rows' bar column.
const ACCENT_BAR_WIDTH: f32 = diff_rows::ACCENT_BAR_WIDTH;
const CODE_PADDING_LEFT: f32 = 12.0;

// Code surface (Pink: transcript.rs `code_surface_header`, `copy_action`).
const CODE_HEADER_HEIGHT: f32 = 28.0;
const CODE_HEADER_ACTION_PAD: f32 = 5.0;
const CODE_ACTION_SIZE: f32 = 22.0;
const CODE_ACTION_ICON: f32 = 13.0;
/// The inline diff's scroll viewport cap (`DIFF_VIEWPORT_MAX_HEIGHT`).
const DIFF_VIEWPORT_MAX_HEIGHT: f32 = 360.0;
/// The review footer: a header-like strip holding `ControlSize::Sm` buttons.
const FOOTER_HEIGHT: f32 = 36.0;

// Review card (Pink: comment_ui.rs `render_comment_card`, comments.rs).
const CARD_PAD_V: f32 = 20.0;
const CARD_GAP: f32 = 6.0;
const CARD_HEADER_HEIGHT: f32 = 22.0;
const CARD_LINE_HEIGHT: f32 = 18.0;
const CARD_WRAP_COLUMNS: usize = 64;
const CARD_MAX_LINES: usize = 8;
const CARD_BAR_REST: f32 = 0.35;
const CARD_BAR_ACTIVE: f32 = 0.7;

/// What happened to one edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Pending,
    Kept,
    Dropped,
}

/// One word-level edit: `before` is replaced by `after` (either may be empty).
#[derive(Clone, Debug)]
pub struct DiffEdit {
    pub before: SharedString,
    pub after: SharedString,
    /// Why the agent made it, shown in the review card while it is in focus.
    pub note: Option<SharedString>,
    /// Index of the source line that holds the edit.
    pub line: usize,
}

#[derive(Clone, Debug)]
enum Piece {
    Code(SharedString),
    Edit(usize),
}

/// Splits word-diff markup into lines of plain code and edits. Tabs expand
/// to four columns (Pink `DIFF_TAB_SIZE`).
fn parse(source: &str) -> (Vec<Vec<Piece>>, Vec<DiffEdit>) {
    let mut lines = Vec::new();
    let mut edits = Vec::new();
    for (line_ix, raw) in source.lines().enumerate() {
        let raw = raw.replace('\t', "    ");
        let mut pieces = Vec::new();
        let mut rest = raw.as_str();
        loop {
            let del = rest.find("[-");
            let ins = rest.find("{+");
            let start = match (del, ins) {
                (Some(d), Some(i)) => d.min(i),
                (Some(d), None) => d,
                (None, Some(i)) => i,
                (None, None) => break,
            };
            let (before_text, marked) = rest.split_at(start);
            let (before, after, tail) = if marked.starts_with("[-") {
                let Some(end) = marked.find("-]") else { break };
                let old = &marked[2..end];
                let tail = &marked[end + 2..];
                match tail
                    .strip_prefix("{+")
                    .and_then(|t| t.find("+}").map(|e| (t, e)))
                {
                    Some((t, e)) => (old, &t[..e], &t[e + 2..]),
                    None => (old, "", tail),
                }
            } else {
                let Some(end) = marked.find("+}") else { break };
                ("", &marked[2..end], &marked[end + 2..])
            };
            if !before_text.is_empty() {
                pieces.push(Piece::Code(before_text.to_string().into()));
            }
            pieces.push(Piece::Edit(edits.len()));
            edits.push(DiffEdit {
                before: before.to_string().into(),
                after: after.to_string().into(),
                note: None,
                line: line_ix,
            });
            rest = tail;
        }
        if !rest.is_empty() {
            pieces.push(Piece::Code(rest.to_string().into()));
        }
        lines.push(pieces);
    }
    (lines, edits)
}

// ---- syntax --------------------------------------------------------------

/// The highlight roles a small Rust-flavoured scanner can tell apart, named
/// after the captures Pink's tree-sitter query gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Plain,
    /// keyword, macro, operator, `self`.
    Keyword,
    /// type, primitive type, constructor.
    Type,
    Function,
    Property,
    /// number, constant.
    Number,
    String,
    Comment,
    Punctuation,
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "type", "use", "where", "while",
];

const PRIMITIVES: &[&str] = &[
    "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "isize", "str", "u8", "u16", "u32",
    "u64", "usize",
];

/// Byte ranges and roles for one line of code.
fn tokenize(code: &str) -> Vec<(Range<usize>, Token)> {
    let chars: Vec<(usize, char)> = code.char_indices().collect();
    let byte_at = |i: usize| chars.get(i).map_or(code.len(), |(b, _)| *b);
    let next_sig = |mut i: usize| {
        while i < chars.len() && chars[i].1.is_whitespace() {
            i += 1;
        }
        chars.get(i).map(|(_, c)| *c)
    };
    let mut out: Vec<(Range<usize>, Token)> = Vec::new();
    let mut push = |range: Range<usize>, kind: Token| match out.last_mut() {
        Some((last, k)) if *k == kind && last.end == range.start => last.end = range.end,
        _ => out.push((range, kind)),
    };
    let mut prev_sig: Option<char> = None;
    let mut prev_word = "";
    let mut i = 0;
    while i < chars.len() {
        let (start, c) = chars[i];
        if c == '/' && chars.get(i + 1).map(|(_, c)| *c) == Some('/') {
            push(start..code.len(), Token::Comment);
            break;
        }
        if c.is_whitespace() {
            push(start..byte_at(i + 1), Token::Plain);
            i += 1;
            continue;
        }
        if c == '"' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].1 != '"' {
                j += if chars[j].1 == '\\' { 2 } else { 1 };
            }
            j = (j + 1).min(chars.len());
            push(start..byte_at(j), Token::String);
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
            push(start..byte_at(j), Token::Number);
            prev_sig = Some('0');
            i = j;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].1.is_alphanumeric() || chars[j].1 == '_') {
                j += 1;
            }
            let word = &code[start..byte_at(j)];
            let after = next_sig(j);
            let upper = word.starts_with(|c: char| c.is_uppercase());
            let kind = if after == Some('!') && chars.get(j).map(|(_, c)| *c) == Some('!') {
                j += 1;
                Token::Keyword
            } else if KEYWORDS.contains(&word) {
                Token::Keyword
            } else if upper && word.len() > 1 && !word.chars().any(|c| c.is_lowercase()) {
                Token::Number
            } else if upper {
                Token::Type
            } else if prev_word == "fn" || after == Some('(') {
                Token::Function
            } else if PRIMITIVES.contains(&word) {
                Token::Type
            } else if prev_sig == Some('.') {
                Token::Property
            } else {
                Token::Plain
            };
            push(start..byte_at(j), kind);
            prev_word = word;
            prev_sig = Some('a');
            i = j;
            continue;
        }
        let kind = match c {
            '(' | ')' | '[' | ']' | '{' | '}' | ':' | '.' | ',' | ';' => Token::Punctuation,
            '*' | '&' => Token::Keyword,
            _ => Token::Plain,
        };
        push(start..byte_at(i + 1), kind);
        prev_sig = Some(c);
        i += 1;
    }
    out
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
    }
}

/// Paint-only syntax runs for one line (Pink: render.rs
/// `runs_for_syntax_line_with_plain`): one font everywhere, so recoloring
/// never changes layout.
fn syntax_runs(text: &str, plain: Hsla, syntax: &Syntax, mono: &Font) -> Vec<TextRun> {
    tokenize(text)
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

/// Splits `runs` so the given byte ranges carry `background` (Pink:
/// changes.rs `with_word_backgrounds`).
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

// ---- diff paint (Pink: changes.rs) ---------------------------------------

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

/// One line-number column, fitted to the largest number (`gutter_width`).
fn gutter_width(max_line: usize) -> f32 {
    let digits = max_line.max(1).ilog10() + 1;
    (digits as f32 * 6.6 + GUTTER_PAD_RIGHT + 6.0).max(GUTTER_WIDTH)
}

/// Review-card body lines, guessed rather than measured so the fold height
/// is analytic (Pink: comments.rs `card_body_lines`).
fn card_body_lines(body: &str) -> usize {
    body.lines()
        .map(|line| line.chars().count().div_ceil(CARD_WRAP_COLUMNS).max(1))
        .sum::<usize>()
        .clamp(1, CARD_MAX_LINES)
}

fn card_height(body: &str) -> f32 {
    CARD_PAD_V + CARD_GAP + CARD_HEADER_HEIGHT + card_body_lines(body) as f32 * CARD_LINE_HEIGHT
}

// ---- the component -------------------------------------------------------

type DecideHandler = Rc<dyn Fn(usize, Decision, &mut Window, &mut App)>;
type ApplyHandler = Rc<dyn Fn(&[Decision], &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct InlineDiff {
    id: ElementId,
    path: SharedString,
    lines: Vec<Vec<Piece>>,
    edits: Vec<DiffEdit>,
    first_line: usize,
    intent: Option<SharedString>,
    decided: Vec<Decision>,
    width: f32,
    autofocus: bool,
    on_decide: Option<DecideHandler>,
    on_apply: Option<ApplyHandler>,
}

impl InlineDiff {
    /// `source` is the edited code with `[-old-]{+new+}` word-diff markers.
    pub fn new(
        id: impl Into<ElementId>,
        path: impl Into<SharedString>,
        source: impl AsRef<str>,
    ) -> Self {
        let (lines, edits) = parse(source.as_ref());
        Self {
            id: id.into(),
            path: path.into(),
            lines,
            edits,
            first_line: 1,
            intent: None,
            decided: Vec::new(),
            width: 700.,
            autofocus: false,
            on_decide: None,
            on_apply: None,
        }
    }

    /// The number of the first source line (for the gutters and hunk band).
    pub fn first_line(mut self, line: usize) -> Self {
        self.first_line = line;
        self
    }

    /// What the agent was asked to do, shown after the file in the header.
    pub fn intent(mut self, intent: impl Into<SharedString>) -> Self {
        self.intent = Some(intent.into());
        self
    }

    /// One note per edit, in source order.
    pub fn notes<S: Into<SharedString>>(mut self, notes: impl IntoIterator<Item = S>) -> Self {
        for (edit, note) in self.edits.iter_mut().zip(notes) {
            edit.note = Some(note.into());
        }
        self
    }

    /// Decisions already made when the review opens, in edit order; the
    /// rest stay pending. They form one step that undo can take back.
    pub fn decisions(mut self, decisions: impl IntoIterator<Item = Decision>) -> Self {
        self.decided = decisions.into_iter().collect();
        self
    }

    /// Surface width in px (default 700).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Takes keyboard focus when first shown, so Y / N work right away.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Called for every decision, including the ones made by Keep all / Drop
    /// all and undo (which reports `Pending`).
    pub fn on_decide(
        mut self,
        handler: impl Fn(usize, Decision, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }

    /// Called with every edit's decision when the reviewer applies them.
    pub fn on_apply(
        mut self,
        handler: impl Fn(&[Decision], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_apply = Some(Rc::new(handler));
        self
    }
}

struct DiffState {
    focus: FocusHandle,
    review: Review,
    scroll: ScrollHandle,
    /// Keep the edit in focus scrolled into view until its card has opened.
    reveal: bool,
}

/// The reviewer's progress: one decision per edit, the edit in focus and
/// the undo history.
#[derive(Clone, Debug)]
struct Review {
    decisions: Vec<Decision>,
    current: usize,
    /// Batches of `(edit, previous decision)` for undo.
    history: Vec<Vec<(usize, Decision)>>,
    applied: bool,
}

impl Review {
    /// A review that opens with `decided` already made, as one undo step.
    fn seeded(n: usize, decided: &[Decision]) -> Self {
        let mut decisions = vec![Decision::Pending; n];
        let mut batch = Vec::new();
        for (ix, d) in decided.iter().take(n).enumerate() {
            if *d != Decision::Pending {
                decisions[ix] = *d;
                batch.push((ix, Decision::Pending));
            }
        }
        let mut review = Self {
            decisions,
            current: 0,
            history: Vec::new(),
            applied: false,
        };
        if !batch.is_empty() {
            review.history.push(batch);
        }
        review.advance();
        review
    }

    fn pending(&self) -> usize {
        self.decisions
            .iter()
            .filter(|d| **d == Decision::Pending)
            .count()
    }

    /// Records `decision` for the given edits; returns the ones that changed.
    fn decide(&mut self, edits: &[usize], decision: Decision) -> Vec<(usize, Decision)> {
        let mut batch = Vec::new();
        for &ix in edits {
            if let Some(slot) = self.decisions.get_mut(ix)
                && *slot != decision
            {
                batch.push((ix, *slot));
                *slot = decision;
            }
        }
        if batch.is_empty() {
            return Vec::new();
        }
        self.history.push(batch.clone());
        self.advance();
        batch.iter().map(|(ix, _)| (*ix, decision)).collect()
    }

    /// Moves to the first pending edit at or after the current one, wrapping.
    fn advance(&mut self) {
        let n = self.decisions.len();
        if let Some(ix) = (0..n)
            .map(|step| (self.current + step) % n)
            .find(|ix| self.decisions[*ix] == Decision::Pending)
        {
            self.current = ix;
        }
    }

    fn undo(&mut self) -> Vec<(usize, Decision)> {
        let Some(batch) = self.history.pop() else {
            return Vec::new();
        };
        for &(ix, previous) in &batch {
            self.decisions[ix] = previous;
        }
        if let Some((first, _)) = batch.iter().min_by_key(|(ix, _)| *ix) {
            self.current = *first;
        }
        batch
            .iter()
            .map(|(ix, _)| (*ix, self.decisions[*ix]))
            .collect()
    }

    /// Moves to the next (or previous) pending edit, wrapping.
    fn step(&mut self, delta: isize) {
        let n = self.decisions.len() as isize;
        let mut ix = self.current as isize;
        for _ in 0..n {
            ix = (ix + delta.signum()).rem_euclid(n);
            if self.decisions[ix as usize] == Decision::Pending {
                self.current = ix as usize;
                return;
            }
        }
    }
}

/// What a listener asks the state to do.
#[derive(Clone, Copy)]
enum Command {
    /// Decide the edit in focus.
    Decide(Decision),
    /// Decide one edit (from its card).
    DecideEdit(usize, Decision),
    DecideAll(Decision),
    Focus(usize),
    Step(isize),
    Undo,
    Apply,
}

type Act = Rc<dyn Fn(Command, &mut Window, &mut App)>;

/// Runs `command` against the state and reports changes to the callbacks.
fn run(
    state: &gpui::Entity<DiffState>,
    command: Command,
    on_decide: &Option<DecideHandler>,
    on_apply: &Option<ApplyHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut changed = Vec::new();
    let mut applied = None;
    state.update(cx, |state, cx| {
        let s = &mut state.review;
        let pending = |s: &Review, ix: usize| s.decisions.get(ix) == Some(&Decision::Pending);
        match command {
            Command::Decide(d) if !s.applied && pending(s, s.current) => {
                let current = s.current;
                changed = s.decide(&[current], d);
            }
            Command::DecideEdit(ix, d) if !s.applied && pending(s, ix) => {
                s.current = ix;
                changed = s.decide(&[ix], d);
            }
            Command::DecideAll(d) if !s.applied => {
                let open: Vec<usize> = (0..s.decisions.len())
                    .filter(|ix| pending(s, *ix))
                    .collect();
                changed = s.decide(&open, d);
            }
            Command::Focus(ix) if !s.applied && pending(s, ix) => s.current = ix,
            Command::Step(delta) if !s.applied => s.step(delta),
            Command::Undo if !s.applied => changed = s.undo(),
            Command::Apply if !s.applied && s.pending() == 0 => {
                s.applied = true;
                applied = Some(s.decisions.clone());
            }
            _ => {}
        }
        state.reveal = !matches!(command, Command::Apply);
        cx.notify();
    });
    if let Some(f) = on_decide {
        for (ix, d) in changed {
            f(ix, d, window, cx);
        }
    }
    if let (Some(f), Some(decisions)) = (on_apply, applied) {
        f(&decisions, window, cx);
    }
}

/// Which side of an edit a row shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Before,
    After,
}

/// What one row of the diff paints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RowKind {
    Context,
    Del,
    Add,
}

/// One planned row: its kind, its hue (a settled row keeps the hue it fades
/// out from), text, washed words, line numbers and whether it stays.
struct Row {
    kind: RowKind,
    added: bool,
    text: String,
    marks: Vec<Range<usize>>,
    old_no: Option<usize>,
    new_no: Option<usize>,
    shown: bool,
}

/// Assembles one line from its pieces, taking `side(ix)` of every edit, and
/// returns the byte ranges of the edits `marked(ix)` picks. Whitespace edges
/// earn no wash, and a line that is almost all new gets none (Pink:
/// changes.rs `word_changes`): the row tint already says it.
fn assemble(
    pieces: &[Piece],
    edits: &[DiffEdit],
    side: impl Fn(usize) -> Side,
    marked: impl Fn(usize) -> bool,
) -> (String, Vec<Range<usize>>) {
    let mut text = String::new();
    let mut marks = Vec::new();
    for piece in pieces {
        match piece {
            Piece::Code(code) => text.push_str(code),
            Piece::Edit(ix) => {
                let edit = &edits[*ix];
                let words = match side(*ix) {
                    Side::Before => &edit.before,
                    Side::After => &edit.after,
                };
                let start = text.len();
                text.push_str(words);
                let lead = words.len() - words.trim_start().len();
                let trail = words.len() - words.trim_end().len();
                if marked(*ix) && lead + trail < words.len() {
                    marks.push(start + lead..start + words.len() - trail);
                }
            }
        }
    }
    let covered: usize = marks.iter().map(|r| r.len()).sum();
    if covered * 10 >= text.len() * 9 {
        marks.clear();
    }
    (text, marks)
}

/// A row with no partner (a whole line added or removed) gets no word
/// layer (Pink: changes.rs `word_changes`).
fn unpaired(paired: bool, marks: [&mut Vec<Range<usize>>; 2]) {
    if !paired {
        marks.into_iter().for_each(Vec::clear);
    }
}

/// A one-line summary of an edit for a card with no note.
fn describe(edit: &DiffEdit) -> SharedString {
    let short = |s: &SharedString| loaders::ellipsize_chars(s.trim(), 24);
    match (edit.before.trim().is_empty(), edit.after.trim().is_empty()) {
        (false, false) => format!(
            "Replace {} with {}",
            short(&edit.before),
            short(&edit.after)
        ),
        (true, false) => format!("Insert {}", short(&edit.after)),
        _ => format!("Remove {}", short(&edit.before)),
    }
    .into()
}

/// The diff's rows for the current decisions, with its `+N −N` and hunk
/// counts.
struct Plan {
    lines: Vec<PlannedLine>,
    additions: usize,
    deletions: usize,
    old_count: usize,
    new_count: usize,
}

/// One source line: its row (or −/+ pair), its edits and the number its
/// review card cites.
struct PlannedLine {
    top: Row,
    bottom: Option<Row>,
    edits: Vec<usize>,
    cite: Option<usize>,
}

/// Plans every row. A line with an open edit shows the current file above
/// (decided edits already read as plain code) and the proposal below; a
/// decided line settles into one context row while the losing row folds.
fn plan(
    lines: &[Vec<Piece>],
    edits: &[DiffEdit],
    decisions: &[Decision],
    first_line: usize,
) -> Plan {
    let mut old_no = first_line;
    let mut new_no = first_line;
    let mut plan = Plan {
        lines: Vec::new(),
        additions: 0,
        deletions: 0,
        old_count: 0,
        new_count: 0,
    };
    for pieces in lines {
        let here: Vec<usize> = pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Edit(ix) => Some(*ix),
                Piece::Code(_) => None,
            })
            .collect();
        if here.is_empty() {
            let (text, _) = assemble(pieces, edits, |_| Side::Before, |_| false);
            plan.lines.push(PlannedLine {
                top: Row {
                    kind: RowKind::Context,
                    added: false,
                    text,
                    marks: Vec::new(),
                    old_no: Some(old_no),
                    new_no: Some(new_no),
                    shown: true,
                },
                bottom: None,
                edits: here,
                cite: None,
            });
            old_no += 1;
            new_no += 1;
            continue;
        }
        let result_side = |ix: usize| match decisions[ix] {
            Decision::Dropped => Side::Before,
            _ => Side::After,
        };
        let (original, _) = assemble(pieces, edits, |_| Side::Before, |_| false);
        let (result, _) = assemble(pieces, edits, result_side, |_| false);
        let in_old = !original.trim().is_empty();
        let in_new = !result.trim().is_empty();
        let o = in_old.then_some(old_no);
        let nn = in_new.then_some(new_no);
        old_no += usize::from(in_old);
        new_no += usize::from(in_new);
        if result != original {
            plan.deletions += usize::from(in_old);
            plan.additions += usize::from(in_new);
        }
        let open = |ix: usize| decisions[ix] == Decision::Pending;
        let (top, bottom) = if here.iter().any(|ix| open(*ix)) {
            // The current file above, the proposal below; decided edits
            // already read as plain code on both.
            let current_side = |ix: usize| match decisions[ix] {
                Decision::Kept => Side::After,
                _ => Side::Before,
            };
            let (old_text, mut old_marks) = assemble(pieces, edits, current_side, open);
            let (new_text, mut new_marks) = assemble(pieces, edits, result_side, open);
            let old_shown = !old_text.trim().is_empty();
            unpaired(old_shown && in_new, [&mut old_marks, &mut new_marks]);
            (
                Row {
                    kind: RowKind::Del,
                    added: false,
                    text: old_text,
                    marks: old_marks,
                    old_no: o,
                    new_no: None,
                    shown: old_shown,
                },
                Row {
                    kind: RowKind::Add,
                    added: true,
                    text: new_text,
                    marks: new_marks,
                    old_no: None,
                    new_no: nn,
                    shown: in_new,
                },
            )
        } else if here.iter().all(|ix| decisions[*ix] == Decision::Dropped) {
            // Dropped: the old row settles into context, the new folds.
            let (text, mut marks) = assemble(pieces, edits, |_| Side::Before, |_| true);
            let (gone, mut gone_marks) = assemble(pieces, edits, |_| Side::After, |_| true);
            let paired = in_old && !gone.trim().is_empty();
            unpaired(paired, [&mut marks, &mut gone_marks]);
            (
                Row {
                    kind: RowKind::Context,
                    added: false,
                    text,
                    marks,
                    old_no: o,
                    new_no: nn,
                    shown: in_new,
                },
                Row {
                    kind: RowKind::Add,
                    added: true,
                    text: gone,
                    marks: gone_marks,
                    old_no: None,
                    new_no: None,
                    shown: false,
                },
            )
        } else {
            // Kept: the new row settles into context, the old folds.
            let (text, mut marks) = assemble(pieces, edits, result_side, |_| true);
            let (gone, mut gone_marks) = assemble(pieces, edits, |_| Side::Before, |_| true);
            unpaired(in_old && in_new, [&mut marks, &mut gone_marks]);
            (
                Row {
                    kind: RowKind::Del,
                    added: false,
                    text: gone,
                    marks: gone_marks,
                    old_no: None,
                    new_no: None,
                    shown: false,
                },
                Row {
                    kind: RowKind::Context,
                    added: true,
                    text,
                    marks,
                    old_no: o,
                    new_no: nn,
                    shown: in_new,
                },
            )
        };
        plan.lines.push(PlannedLine {
            top,
            bottom: Some(bottom),
            edits: here,
            cite: nn.or(o),
        });
    }
    plan.old_count = old_no - first_line;
    plan.new_count = new_no - first_line;
    plan
}

impl RenderOnce for InlineDiff {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let n = self.edits.len();
        let autofocus = self.autofocus;
        let decided = self.decided.clone();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| {
                let focus = cx.focus_handle();
                if autofocus {
                    window.focus(&focus, cx);
                }
                DiffState {
                    focus,
                    review: Review::seeded(n, &decided),
                    scroll: ScrollHandle::new(),
                    reveal: false,
                }
            },
        );
        let (decisions, current, can_undo, applied, focus, scroll, reveal) = {
            let state = state.read(cx);
            let s = &state.review;
            (
                s.decisions.clone(),
                s.current.min(n.saturating_sub(1)),
                !s.history.is_empty() && !s.applied,
                s.applied,
                state.focus.clone(),
                state.scroll.clone(),
                state.reveal,
            )
        };
        let pending = decisions
            .iter()
            .filter(|d| **d == Decision::Pending)
            .count();
        let kept = decisions.iter().filter(|d| **d == Decision::Kept).count();
        let focused = focus.contains_focused(window, cx);
        let focus_t = motion::follow(
            &format!("{key}-focus"),
            if focused { 1. } else { 0. },
            HOVER_FADE,
        );

        let on_decide = self.on_decide.clone();
        let on_apply = self.on_apply.clone();
        let act: Act = {
            let state = state.clone();
            Rc::new(move |command: Command, window: &mut Window, cx: &mut App| {
                run(&state, command, &on_decide, &on_apply, window, cx)
            })
        };

        // ---- plan and paint the rows ------------------------------------
        let gutter_px = gutter_width(self.first_line + self.lines.len());
        let plan = plan(&self.lines, &self.edits, &decisions, self.first_line);
        let mono = font(pink::MONO);
        let syntax = t.syntax();
        let plain = diff_plain_text(&t);
        let paint = RowPaint {
            gutter_px,
            mono: &mono,
            syntax: &syntax,
            plain,
            t: &t,
        };
        let mut current_line = None;
        let mut current_open = 1.0_f32;
        let mut blocks: Vec<AnyElement> = Vec::new();
        for (li, line) in plan.lines.into_iter().enumerate() {
            let here = line.edits;
            let focus_ix = here
                .iter()
                .copied()
                .find(|ix| *ix == current && decisions[*ix] == Decision::Pending)
                .or_else(|| {
                    here.iter()
                        .copied()
                        .find(|ix| decisions[*ix] == Decision::Pending)
                });
            let mut block = div().flex().flex_col();
            for (slot, row) in [Some(line.top), line.bottom]
                .into_iter()
                .flatten()
                .enumerate()
            {
                let row_key = format!("{key}-l{li}-{slot}");
                block = block.children(paint.row(&row_key, &row, focus_ix, &act));
            }
            for &ix in &here {
                let is_current = ix == current && decisions[ix] == Decision::Pending && !applied;
                if is_current {
                    current_line = Some(li);
                }
                let open = motion::follow(
                    &format!("{key}-card-{ix}"),
                    if is_current { 1. } else { 0. },
                    DISCLOSURE,
                );
                if is_current {
                    current_open = open;
                }
                if open <= 0.001 && !is_current {
                    continue;
                }
                let location: SharedString = match line.cite {
                    Some(no) => format!("{}:{no}", self.path).into(),
                    None => self.path.clone(),
                };
                let edit = &self.edits[ix];
                let note = edit.note.clone().unwrap_or_else(|| describe(edit));
                block = block.child(review_card(
                    &key,
                    ix,
                    location,
                    note,
                    open,
                    is_current,
                    focus_t,
                    act.clone(),
                    &t,
                ));
            }
            blocks.push(block.into_any_element());
        }

        // Keep the edit in focus (and its opening card) in view.
        if reveal {
            if let Some(li) = current_line {
                scroll.scroll_to_item(li + 1);
            }
            if current_line.is_none() || current_open >= 0.999 {
                state.update(cx, |s, _| s.reveal = false);
            }
        }

        let hunk = diff_rows::hunk_header_row(
            format!(
                "@@ -{},{} +{},{} @@",
                self.first_line, plan.old_count, self.first_line, plan.new_count
            ),
            &diff_rows::diff_style(&t),
        );

        let body = div()
            .id(SharedString::from(format!("{key}-diff-scroll")))
            .max_h(px(DIFF_VIEWPORT_MAX_HEIGHT))
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .flex()
            .flex_col()
            .pb(px(BODY_BOTTOM_PAD))
            .child(hunk)
            .children(blocks);

        // ---- header (Pink: transcript.rs code_surface_header) ----------
        let name: SharedString = self
            .path
            .rsplit(['/', '\\'])
            .find(|part| !part.is_empty())
            .unwrap_or(&self.path)
            .to_string()
            .into();
        let undo = {
            let act = act.clone();
            let hover_key = format!("{key}-undo");
            div()
                .id(SharedString::from(hover_key.clone()))
                .role(Role::Button)
                .aria_label("Undo")
                .flex_none()
                .size(px(CODE_ACTION_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::SM))
                .map(|el| {
                    if can_undo {
                        let ring = t.focus_ring();
                        el.tab_index(0)
                            .cursor_pointer()
                            .focus_visible(move |s| pink::focus_ring(s, ring))
                            .on_key_down(pink::swallow_activation_key)
                            .bg(motion::hover_blend(
                                &hover_key,
                                gpui::transparent_black(),
                                t.ink(0.08),
                            ))
                            .on_hover(motion::hover_listener(hover_key))
                            .tooltip(pink::text_tooltip("Undo  U"))
                            .tooltip_show_delay(layout::TOOLTIP_DELAY)
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                act(Command::Undo, window, cx)
                            })
                    } else {
                        el.opacity(0.45)
                    }
                })
                .child(
                    svg()
                        .path(ICON_UNDO)
                        .size(px(CODE_ACTION_ICON))
                        .text_color(t.content_muted),
                )
        };
        let header = div()
            .h(px(CODE_HEADER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(pink::space::S12))
            .pl(px(pink::chrome::CONTENT_INSET))
            .pr(px(if applied {
                pink::chrome::CONTENT_INSET
            } else {
                CODE_HEADER_ACTION_PAD
            }))
            .border_b_1()
            .border_color(t.border)
            .bg(t.ink(0.02))
            .text_size(px(11.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.content_muted)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(pink::space::S6))
                    .child(div().min_w_0().truncate().child(name))
                    .child(
                        div()
                            .flex_none()
                            .text_color(t.readable_tone(t.diff_add, 1.0))
                            .child(format!("+{}", plan.additions)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(t.readable_tone(t.diff_del, 1.0))
                            .child(format!("−{}", plan.deletions)),
                    )
                    .when_some(self.intent.clone(), |row, intent| {
                        row.child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::NORMAL)
                                .text_color(t.content_faint)
                                .child(intent),
                        )
                    }),
            )
            .when(!applied, |header| {
                header.child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(pink::space::S6))
                        .child(
                            div()
                                .font_weight(FontWeight::NORMAL)
                                .font_features(pink::tabular_figures())
                                .text_color(t.content_faint)
                                .child(format!("{} of {n} reviewed", n - pending)),
                        )
                        .child(undo),
                )
            });

        // ---- footer -----------------------------------------------------
        let edits_word = |count: usize| if count == 1 { "edit" } else { "edits" };
        let (footer_left, footer_right): (AnyElement, Option<AnyElement>) = if applied {
            (
                footer_note(
                    format!("Applied {kept} {} to {}", edits_word(kept), self.path),
                    &t,
                ),
                None,
            )
        } else if pending == 0 {
            let apply = {
                let act = act.clone();
                pink::button(
                    format!("{key}-apply"),
                    ButtonKind::Primary,
                    ControlSize::Sm,
                    &t,
                )
                .child(if kept == 0 {
                    "Keep original".to_string()
                } else {
                    format!("Apply {kept} {}", edits_word(kept))
                })
                .on_click(move |_, window, cx| act(Command::Apply, window, cx))
            };
            (
                footer_note(format!("{kept} kept, {} dropped", n - kept), &t),
                Some(apply.into_any_element()),
            )
        } else {
            let all = |id: &str, kind: ButtonKind, label: &'static str, d: Decision| {
                let act = act.clone();
                pink::button(format!("{key}-{id}"), kind, ControlSize::Sm, &t)
                    .child(label)
                    .on_click(move |_, window, cx| act(Command::DecideAll(d), window, cx))
            };
            (
                div()
                    .flex()
                    .items_center()
                    .gap(px(pink::space::S12))
                    .overflow_hidden()
                    .child(key_legend("Y", "Keep", &t))
                    .child(key_legend("N", "Drop", &t))
                    .child(key_legend("J K", "Move", &t))
                    .child(key_legend("U", "Undo", &t))
                    .into_any_element(),
                Some(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(pink::space::S6))
                        .child(all(
                            "drop-all",
                            ButtonKind::Ghost,
                            "Drop all",
                            Decision::Dropped,
                        ))
                        .child(all(
                            "keep-all",
                            ButtonKind::Secondary,
                            "Keep all",
                            Decision::Kept,
                        ))
                        .into_any_element(),
                ),
            )
        };
        let phase = if applied {
            "applied"
        } else if pending == 0 {
            "decided"
        } else {
            "open"
        };
        let footer = div()
            .h(px(FOOTER_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(pink::space::S12))
            .pl(px(pink::chrome::CONTENT_INSET))
            .pr(px(pink::space::S6))
            .border_t_1()
            .border_color(t.border)
            .bg(t.ink(0.02))
            .child(motion::status_swap(
                SharedString::from(format!("{key}-footer-{phase}")),
                div().min_w_0().flex().items_center().child(footer_left),
            ))
            .children(footer_right.map(|right| {
                motion::status_swap(
                    SharedString::from(format!("{key}-actions-{phase}")),
                    div().flex_none().child(right),
                )
            }));

        let on_key = {
            let act = act.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let k = &event.keystroke;
                let command = match k.key.as_str() {
                    "z" if k.modifiers.platform || k.modifiers.control => Command::Undo,
                    _ if k.modifiers.platform || k.modifiers.control || k.modifiers.alt => return,
                    "y" | "enter" => Command::Decide(Decision::Kept),
                    "n" | "backspace" | "delete" => Command::Decide(Decision::Dropped),
                    "j" | "down" => Command::Step(1),
                    "k" | "up" => Command::Step(-1),
                    "u" => Command::Undo,
                    _ => return,
                };
                act(command, window, cx);
                cx.stop_propagation();
            }
        };

        motion::keep_alive(window);
        let ring = t.focus_ring();
        // The frame (Pink: transcript.rs code_surface_frame).
        pink::code_surface(&t)
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, {
                let focus = focus.clone();
                move |_, window, cx| window.focus(&focus, cx)
            })
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .w(px(self.width))
            .flex()
            .flex_col()
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(header)
            .child(body)
            .child(footer)
    }
}

/// Paints planned rows with one set of diff colours.
struct RowPaint<'a> {
    gutter_px: f32,
    mono: &'a Font,
    syntax: &'a Syntax,
    plain: Hsla,
    t: &'a Pink,
}

impl RowPaint<'_> {
    /// One diff line (Pink: changes.rs `diff_line_row`): a 3 px spacer, dual
    /// line-number gutters, the marker column and paint-only syntax runs on
    /// the row's wash. The wash, the marker's tone and the word washes fade
    /// with `tint` as a decided row settles; the height folds when it goes.
    fn row(&self, key: &str, row: &Row, focus_ix: Option<usize>, act: &Act) -> Option<AnyElement> {
        let t = self.t;
        let fold = motion::follow(
            &format!("{key}-h"),
            if row.shown { 1. } else { 0. },
            DISCLOSURE,
        );
        if fold <= 0.001 && !row.shown {
            return None;
        }
        let alpha = motion::follow(
            &format!("{key}-a"),
            if row.shown { 1. } else { 0. },
            TOOL_BODY_FADE,
        );
        let tint = motion::follow(
            &format!("{key}-tint"),
            if row.kind == RowKind::Context { 0. } else { 1. },
            DISCLOSURE,
        );
        let tone = if row.added { t.diff_add } else { t.diff_del };
        let wash = diff_row_wash(tone, t);
        let faint = t.readable_faint(0.8);
        let changed_number = motion::mix(
            faint,
            t.readable_on(faint, wash, TEXT_CONTRAST_TARGET),
            tint,
        );
        let marker_color = match row.kind {
            RowKind::Context => motion::mix(
                t.readable_faint(0.5),
                t.readable_on(tone, wash, TEXT_CONTRAST_TARGET),
                tint,
            ),
            _ => t.readable_on(tone, wash, TEXT_CONTRAST_TARGET),
        };
        let marker = match row.kind {
            // joiners: allow the context line's gutter marker, not a joiner
            RowKind::Context => "·",
            RowKind::Del => "−",
            RowKind::Add => "+",
        };
        let word = diff_word_wash(tone, t);
        let runs = with_word_backgrounds(
            syntax_runs(&row.text, self.plain, self.syntax, self.mono),
            &row.marks,
            Hsla {
                a: word.a * tint,
                ..word
            },
        );
        let style = diff_rows::diff_style(t);
        let gutter = |no: Option<usize>, color: Hsla| {
            diff_rows::gutter(no.map(|n| n as u32), color, self.gutter_px, &style)
        };
        let (old_color, new_color) = if row.added {
            (faint, changed_number)
        } else {
            (changed_number, faint)
        };
        let clickable = row.kind != RowKind::Context && focus_ix.is_some();
        Some(
            div()
                .id(SharedString::from(key.to_string()))
                .h(px(DIFF_LINE_HEIGHT * fold))
                .w_full()
                .flex_none()
                .flex()
                .flex_row()
                .items_start()
                .overflow_hidden()
                .opacity(alpha)
                .bg(Hsla {
                    a: wash.a * tint,
                    ..wash
                })
                .when_some(focus_ix.filter(|_| clickable), |el, ix| {
                    let act = act.clone();
                    el.cursor_pointer()
                        .on_click(move |_, window, cx| act(Command::Focus(ix), window, cx))
                })
                .child(diff_rows::accent_bar(None))
                .child(gutter(row.old_no, old_color))
                .child(gutter(row.new_no, new_color))
                .child(diff_rows::marker(
                    marker,
                    marker_color,
                    diff_rows::MARKER_WIDTH,
                    &style,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .h(px(DIFF_LINE_HEIGHT))
                        .overflow_hidden()
                        .child(
                            div()
                                .pl(px(CODE_PADDING_LEFT))
                                .font_family(pink::MONO)
                                .text_size(px(DIFF_TEXT_SIZE))
                                .line_height(px(DIFF_LINE_HEIGHT))
                                .whitespace_nowrap()
                                .child(StyledText::new(row.text.clone()).with_runs(runs)),
                        ),
                )
                .into_any_element(),
        )
    }
}

/// The review card under the edit in focus, in the shape of Pink's inline
/// review comment (Pink: comment_ui.rs `render_comment_card`): an
/// `ink(0.05)` band with a 3 px `solid` bar, a 22 px header with the note
/// glyph and the mono `path:line`, the note at 12/18, and Keep / Drop. The
/// bar firms up from 0.35 to 0.7 (Pink's draft card) while the diff holds
/// keyboard focus. It folds open and shut over `DISCLOSURE`.
#[allow(clippy::too_many_arguments)]
fn review_card(
    key: &str,
    ix: usize,
    location: SharedString,
    note: SharedString,
    open: f32,
    current: bool,
    focus_t: f32,
    act: Act,
    t: &Pink,
) -> impl IntoElement {
    let full = card_height(&note);
    let alpha = motion::follow(
        &format!("{key}-card-{ix}-a"),
        if current { 1. } else { 0. },
        TOOL_BODY_FADE,
    );
    let action = |id: &str, kind: ButtonKind, label: &'static str, d: Decision| {
        let act = act.clone();
        pink::button(format!("{key}-card-{ix}-{id}"), kind, ControlSize::Sm, t)
            .child(label)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                act(Command::DecideEdit(ix, d), window, cx)
            })
    };
    // Centre the actions on the header and first note line.
    let actions_top =
        CARD_PAD_V / 2. + (CARD_HEADER_HEIGHT + CARD_LINE_HEIGHT - ControlSize::Sm.height()) / 2.;
    div()
        .h(px(full * open))
        .w_full()
        .flex_none()
        .overflow_hidden()
        .child(
            div()
                .h(px(full))
                .w_full()
                .flex()
                .flex_row()
                .bg(t.ink(0.05))
                .opacity(alpha)
                .child(
                    div().w(px(ACCENT_BAR_WIDTH)).h_full().flex_none().bg(t
                        .solid
                        .opacity(motion::lerp(CARD_BAR_REST, CARD_BAR_ACTIVE, focus_t))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .px(px(pink::space::S16))
                        .py(px(CARD_PAD_V / 2.))
                        .child(
                            div()
                                .h(px(CARD_HEADER_HEIGHT))
                                .flex_none()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(pink::space::S6))
                                .child(
                                    svg()
                                        .path(ICON_NOTE)
                                        .size(px(pink::icon_size::XS))
                                        .flex_none()
                                        .text_color(t.text_faint),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_family(pink::MONO)
                                        .text_size(px(11.))
                                        .text_color(t.text_faint)
                                        .child(location),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                .text_size(px(12.))
                                .line_height(px(CARD_LINE_HEIGHT))
                                .text_color(t.text_dim)
                                .child(note),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_start()
                        .gap(px(pink::space::S6))
                        .pt(px(actions_top))
                        .pr(px(pink::chrome::CONTENT_INSET))
                        .child(action("drop", ButtonKind::Ghost, "Drop", Decision::Dropped))
                        .child(action(
                            "keep",
                            ButtonKind::Secondary,
                            "Keep",
                            Decision::Kept,
                        )),
                ),
        )
}

/// A footer key legend (Pink: command_palette.rs `command_key_hint`): a
/// `kbd_hint` and its 11 px verb, gap 5.
fn key_legend(keys: &'static str, label: &'static str, t: &Pink) -> impl IntoElement {
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(5.))
        .child(pink::kbd_hint(keys, t))
        .child(
            div()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child(label),
        )
}

/// The footer's status line once every edit is decided.
fn footer_note(copy: String, t: &Pink) -> AnyElement {
    div()
        .min_w_0()
        .truncate()
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(t.readable_muted(0.85))
        .child(copy)
        .into_any_element()
}

const DEMO: &str = "/// Wait before retry `attempt`, growing with each failure.
pub fn next_delay(attempt: u32, policy: &Policy) -> Duration {
    let base = Duration::from_millis([-500-]{+policy.base_ms+});
    let step = base * [-attempt-]{+2u32.pow(attempt.min(6))+};
{+    let jitter = step.mul_f32(fastrand::f32() * 0.2);+}
    let cap = Duration::from_secs([-60-]{+30+});
    [-step-]{+(step + jitter)+}.min(cap)
}";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        InlineDiff::new("inline-diff", "src/sync/backoff.rs", DEMO)
            .first_line(41)
            .intent("Cap the retry backoff at 30 s and add jitter")
            .notes([
                "Read the base delay from the retry policy",
                "Grow exponentially, capped at 2⁶",
                "Spread retries by up to 20% so clients don't stampede",
                "Never wait longer than the 30 s upstream timeout",
                "Include the jitter in the final wait",
            ])
            .decisions([Decision::Kept])
            .autofocus(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_word_diff_markers() {
        let (lines, edits) = parse("let a = [-1-]{+2+};\n{+new line+}\nkeep [-gone-] x");
        assert_eq!(lines.len(), 3);
        assert_eq!(edits.len(), 3);
        assert_eq!(edits[0].before.as_ref(), "1");
        assert_eq!(edits[0].after.as_ref(), "2");
        assert_eq!(edits[1].before.as_ref(), "");
        assert_eq!(edits[1].after.as_ref(), "new line");
        assert_eq!(edits[1].line, 1);
        assert_eq!(edits[2].after.as_ref(), "");
        assert!(matches!(&lines[2][2], Piece::Code(s) if s.as_ref() == " x"));
    }

    #[test]
    fn word_marks_skip_whole_line_changes() {
        let (lines, edits) = parse("let a = [-1-]{+2+};\n  [-a-]{+let total = sum(&parts);+}");
        let (text, marks) = assemble(&lines[0], &edits, |_| Side::After, |_| true);
        assert_eq!(text, "let a = 2;");
        assert_eq!(marks, vec![8..9]);
        let (text, marks) = assemble(&lines[1], &edits, |_| Side::After, |_| true);
        assert_eq!(text, "  let total = sum(&parts);");
        assert!(marks.is_empty(), "almost the whole line changed");
    }

    #[test]
    fn plan_follows_decisions() {
        let (lines, edits) = parse(DEMO);
        let open = plan(&lines, &edits, &[Decision::Pending; 5], 41);
        assert_eq!((open.additions, open.deletions), (5, 4));
        assert_eq!((open.old_count, open.new_count), (7, 8));
        let dropped = plan(&lines, &edits, &[Decision::Dropped; 5], 41);
        assert_eq!((dropped.additions, dropped.deletions), (0, 0));
        assert_eq!(dropped.new_count, 7);
        // Every line settles into one row; the dropped insertion folds away.
        let shown = dropped
            .lines
            .iter()
            .flat_map(|line| [Some(&line.top), line.bottom.as_ref()])
            .flatten()
            .filter(|row| row.shown)
            .count();
        assert_eq!(shown, 7);
    }

    #[test]
    fn undo_restores_batches() {
        let mut s = Review::seeded(3, &[]);
        s.decide(&[0], Decision::Kept);
        assert_eq!(s.current, 1);
        s.decide(&[1, 2], Decision::Dropped);
        assert_eq!(s.pending(), 0);
        s.undo();
        assert_eq!(s.decisions[1], Decision::Pending);
        assert_eq!(s.decisions[0], Decision::Kept);
    }

    #[test]
    fn seeded_decisions_undo_as_one_step() {
        let mut s = Review::seeded(3, &[Decision::Kept, Decision::Pending, Decision::Dropped]);
        assert_eq!(s.current, 1);
        s.step(1);
        assert_eq!(s.current, 1, "only one edit is open");
        s.undo();
        assert_eq!(s.pending(), 3);
    }

    /// Hosts the demo diff and records what it reports.
    #[derive(Default)]
    struct Host {
        decided: Vec<(usize, Decision)>,
    }

    impl gpui::Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            let host = cx.entity().downgrade();
            div().size_full().child(
                InlineDiff::new("diff", "src/sync/backoff.rs", DEMO)
                    .decisions([Decision::Kept])
                    .autofocus(true)
                    .on_decide(move |ix, d, _, cx| {
                        host.update(cx, |h, _| h.decided.push((ix, d))).ok();
                    }),
            )
        }
    }

    #[gpui::test]
    fn keys_review_the_edits(cx: &mut gpui::TestAppContext) {
        cx.update(crate::init);
        let (host, cx) = cx.add_window_view(|_, _| Host::default());
        let settle = |cx: &mut gpui::VisualTestContext| {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(400));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        };
        settle(cx);
        // Keep the edit in focus (1), drop the next (2), skip 3 for 4.
        cx.simulate_keystrokes("y");
        settle(cx);
        cx.simulate_keystrokes("n");
        settle(cx);
        cx.simulate_keystrokes("j n");
        settle(cx);
        host.read_with(cx, |h, _| {
            assert_eq!(
                h.decided,
                vec![
                    (1, Decision::Kept),
                    (2, Decision::Dropped),
                    (4, Decision::Dropped)
                ]
            );
        });
        // Undo reopens 4, and the seeded decision is one more step back.
        cx.simulate_keystrokes("u u");
        settle(cx);
        cx.simulate_keystrokes("u u u");
        settle(cx);
        host.read_with(cx, |h, _| {
            assert_eq!(h.decided.len(), 7);
            assert_eq!(h.decided[6], (0, Decision::Pending));
        });
    }

    #[test]
    fn tokens_follow_rust_captures() {
        let kinds: Vec<(String, Token)> =
            tokenize("let d = Duration::from_millis(policy.base_ms);")
                .into_iter()
                .map(|(r, k)| {
                    (
                        "let d = Duration::from_millis(policy.base_ms);"[r].to_string(),
                        k,
                    )
                })
                .filter(|(s, _)| !s.trim().is_empty())
                .collect();
        assert!(kinds.contains(&("let".into(), Token::Keyword)));
        assert!(kinds.contains(&("Duration".into(), Token::Type)));
        assert!(kinds.contains(&("from_millis".into(), Token::Function)));
        assert!(kinds.contains(&("base_ms".into(), Token::Property)));
    }
}
