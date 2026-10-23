//! Inline Diff – an agent's edits to a file, shown word by word inside the
//! code instead of as a separate before/after view.
//!
//! Every edit is marked in place: the old words struck through on a red
//! wash, the new ones on a green wash. A floating rail follows the edit in
//! focus; keep it or drop it with the rail, `Y` / `N`, or the footer, and the
//! losing side folds away while the code settles back to plain text. `J` / `K`
//! (or the arrows) step between edits and `U` undoes the last decision.
//!
//! The source uses `git diff --word-diff` markers: `[-old-]{+new+}` is a
//! replacement, a lone `[-old-]` a deletion and a lone `{+new+}` an insertion.
//!
//! ```ignore
//! InlineDiff::new("retry", "src/sync/backoff.rs", "let tries = [-3-]{+5+};")
//!     .notes(["Retry five times before giving up"])
//!     .on_apply(|decisions, _, _| { /* write the kept edits */ })
//! ```

use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, AnyView, App, Div, ElementId, FocusHandle, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseButton, SharedString, Window, div, font, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt, EASE_OUT, GLIDE, HOVER_FADE, MENU_IN, MotionSpec},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, radius, space, text},
    primitives::ring,
    story,
};

pub const NAME: &str = "Inline Diff";
pub const DESCRIPTION: &str = "An agent's edits marked word by word inside the code – step through them and keep or drop each one with a floating rail, Y / N and undo";

const ICON_FILE: &str = "icons/inline-diff/file.svg";
const ICON_SPARK: &str = "icons/inline-diff/spark.svg";
const ICON_KEEP: &str = "icons/inline-diff/check.svg";
const ICON_DROP: &str = "icons/inline-diff/cross.svg";
const ICON_UNDO: &str = "icons/inline-diff/undo.svg";
const ICON_UP: &str = "icons/inline-diff/chevron-up.svg";
const ICON_DOWN: &str = "icons/inline-diff/chevron-down.svg";
const ICON_DONE: &str = "icons/inline-diff/done.svg";

/// Code type size and row height.
const CODE_SIZE: f32 = 12.5;
const LINE_H: f32 = 24.0;
/// Width of the line-number gutter.
const GUTTER_W: f32 = 44.0;
/// Rough width of the Keep / Drop rail, to keep it inside the card.
const RAIL_W: f32 = 160.0;
/// Horizontal padding inside a marked span.
const MARK_PAD: f32 = 2.0;
/// How long a kept or dropped edit keeps its tint while it settles.
const SETTLE: MotionSpec = MotionSpec::new(700, EASE_OUT);

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
    /// Why the agent made it, shown in the footer while the edit is in focus.
    pub note: Option<SharedString>,
    /// Index of the source line that holds the edit.
    pub line: usize,
}

#[derive(Clone, Debug)]
enum Piece {
    Code(SharedString),
    Edit(usize),
}

/// Splits word-diff markup into lines of plain code and edits.
fn parse(source: &str) -> (Vec<Vec<Piece>>, Vec<DiffEdit>) {
    let mut lines = Vec::new();
    let mut edits = Vec::new();
    for (line_ix, raw) in source.lines().enumerate() {
        let mut pieces = Vec::new();
        let mut rest = raw;
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Token {
    Plain,
    Keyword,
    Type,
    Literal,
    Comment,
}

const KEYWORDS: &[&str] = &[
    "as", "async", "await", "const", "else", "enum", "fn", "for", "if", "impl", "in", "let",
    "match", "mut", "pub", "return", "self", "struct", "use", "where", "while",
];

/// A small Rust-flavoured tokenizer: enough to tint keywords, types, literals
/// and comments.
fn tokenize(code: &str) -> Vec<(String, Token)> {
    let mut out: Vec<(String, Token)> = Vec::new();
    let chars: Vec<char> = code.chars().collect();
    let mut i = 0;
    let push = |out: &mut Vec<(String, Token)>, s: String, k: Token| match out.last_mut() {
        Some((prev, kind)) if *kind == k && k == Token::Plain => prev.push_str(&s),
        _ => out.push((s, k)),
    };
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            push(&mut out, chars[i..].iter().collect(), Token::Comment);
            break;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let kind = if KEYWORDS.contains(&word.as_str()) {
                Token::Keyword
            } else if word.starts_with(|c: char| c.is_ascii_uppercase())
                || matches!(word.as_str(), "u32" | "u64" | "f32" | "usize" | "bool")
            {
                Token::Type
            } else {
                Token::Plain
            };
            push(&mut out, word, kind);
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                i += 1;
            }
            push(&mut out, chars[start..i].iter().collect(), Token::Literal);
            continue;
        }
        if c == '"' {
            let start = i;
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            i = (i + 1).min(chars.len());
            push(&mut out, chars[start..i].iter().collect(), Token::Literal);
            continue;
        }
        push(&mut out, c.to_string(), Token::Plain);
        i += 1;
    }
    out
}

fn token_color(kind: Token, t: &Pink) -> Hsla {
    match kind {
        Token::Plain => t.text,
        Token::Keyword => t.accent,
        Token::Type => t.text_muted,
        Token::Literal => t.warning,
        Token::Comment => t.text_faint,
    }
}

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
            width: 700.,
            autofocus: false,
            on_decide: None,
            on_apply: None,
        }
    }

    /// The number of the first source line (for the gutter).
    pub fn first_line(mut self, line: usize) -> Self {
        self.first_line = line;
        self
    }

    /// What the agent was asked to do, shown under the file name.
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

    /// Card width in px (default 700).
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
    fn new(n: usize) -> Self {
        Self {
            decisions: vec![Decision::Pending; n],
            current: 0,
            history: Vec::new(),
            applied: false,
        }
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

    /// Moves to the next pending edit after the current one, wrapping.
    fn advance(&mut self) {
        let n = self.decisions.len();
        if n == 0 {
            return;
        }
        for step in 0..n {
            let ix = (self.current + step) % n;
            if self.decisions[ix] == Decision::Pending {
                self.current = ix;
                return;
            }
        }
    }

    fn undo(&mut self) -> Vec<(usize, Decision)> {
        let Some(batch) = self.history.pop() else {
            return Vec::new();
        };
        for &(ix, previous) in &batch {
            self.decisions[ix] = previous;
        }
        if let Some((first, _)) = batch.first() {
            self.current = *first;
        }
        batch
            .iter()
            .map(|(ix, _)| (*ix, self.decisions[*ix]))
            .collect()
    }

    fn step(&mut self, delta: isize) {
        let n = self.decisions.len() as isize;
        if n > 0 {
            self.current = (self.current as isize + delta).rem_euclid(n) as usize;
        }
    }
}

/// What a listener asks the state to do.
#[derive(Clone, Copy)]
enum Command {
    Decide(Decision),
    DecideAll(Decision),
    Focus(usize),
    Step(isize),
    Undo,
    Apply,
}

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
        match command {
            Command::Decide(d) if !s.applied => {
                let current = s.current;
                if s.decisions.get(current) == Some(&Decision::Pending) {
                    changed = s.decide(&[current], d);
                }
            }
            Command::DecideAll(d) if !s.applied => {
                let open: Vec<usize> = (0..s.decisions.len())
                    .filter(|ix| s.decisions[*ix] == Decision::Pending)
                    .collect();
                changed = s.decide(&open, d);
            }
            Command::Focus(ix) => s.current = ix.min(s.decisions.len().saturating_sub(1)),
            Command::Step(delta) => s.step(delta),
            Command::Undo if !s.applied => changed = s.undo(),
            Command::Apply if !s.applied && s.pending() == 0 => {
                s.applied = true;
                applied = Some(s.decisions.clone());
            }
            _ => {}
        }
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

impl RenderOnce for InlineDiff {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let n = self.edits.len();
        let autofocus = self.autofocus;
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
                    review: Review::new(n),
                }
            },
        );
        let (decisions, current, can_undo, applied, focus) = {
            let state = state.read(cx);
            let s = &state.review;
            (
                s.decisions.clone(),
                s.current.min(n.saturating_sub(1)),
                !s.history.is_empty() && !s.applied,
                s.applied,
                state.focus.clone(),
            )
        };
        let pending = decisions
            .iter()
            .filter(|d| **d == Decision::Pending)
            .count();
        let kept = decisions.iter().filter(|d| **d == Decision::Kept).count();
        let focused = focus.is_focused(window);

        // Geist Mono advance, for folding marked spans to zero width.
        let font_id = window.text_system().resolve_font(&font(pink::MONO));
        let advance = window
            .text_system()
            .em_advance(font_id, px(CODE_SIZE))
            .map(f32::from)
            .unwrap_or(CODE_SIZE * 0.6);

        let on_decide = self.on_decide.clone();
        let on_apply = self.on_apply.clone();
        let act = {
            let state = state.clone();
            Rc::new(move |command: Command, window: &mut Window, cx: &mut App| {
                run(&state, command, &on_decide, &on_apply, window, cx)
            })
        };

        // Per-edit visibility of the old and new words, and how much diff
        // tint remains. Followers make every change glide, undo included.
        let vis: Vec<(f32, f32, f32)> = decisions
            .iter()
            .enumerate()
            .map(|(ix, d)| {
                let old = motion::follow(
                    &format!("{key}-e{ix}-old"),
                    if *d == Decision::Kept { 0. } else { 1. },
                    GLIDE,
                );
                let new = motion::follow(
                    &format!("{key}-e{ix}-new"),
                    if *d == Decision::Dropped { 0. } else { 1. },
                    GLIDE,
                );
                let tint = motion::follow(
                    &format!("{key}-e{ix}-tint"),
                    if *d == Decision::Pending { 1. } else { 0. },
                    SETTLE,
                );
                (old, new, tint)
            })
            .collect();

        // ---- code ------------------------------------------------------
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut y = 0.0_f32;
        let mut current_y = 0.0_f32;
        let mut current_w = 0.0_f32;
        for (line_ix, pieces) in self.lines.iter().enumerate() {
            let has_code = pieces
                .iter()
                .any(|p| matches!(p, Piece::Code(s) if !s.trim().is_empty()));
            let edits_here: Vec<usize> = pieces
                .iter()
                .filter_map(|p| match p {
                    Piece::Edit(ix) => Some(*ix),
                    _ => None,
                })
                .collect();
            // A line made only of an insertion or a deletion folds away
            // with its edit.
            let (line_vis, target_vis) = if has_code || edits_here.is_empty() {
                (1., 1.)
            } else {
                edits_here.iter().fold((0.0_f32, 0.0_f32), |(v, tv), ix| {
                    let e = &self.edits[*ix];
                    let (old, new, _) = vis[*ix];
                    let d = decisions[*ix];
                    let mut v = v;
                    let mut tv = tv;
                    if !e.before.trim().is_empty() {
                        v = v.max(old);
                        tv = tv.max(if d == Decision::Kept { 0. } else { 1. });
                    }
                    if !e.after.trim().is_empty() {
                        v = v.max(new);
                        tv = tv.max(if d == Decision::Dropped { 0. } else { 1. });
                    }
                    (v, tv)
                })
            };
            if edits_here.contains(&current) {
                current_y = y;
                current_w = line_width(pieces, &self.edits, &decisions, advance);
            }
            y += LINE_H * target_vis;
            if line_vis <= 0.001 {
                continue;
            }

            let focus_t = motion::follow(
                &format!("{key}-l{line_ix}-focus"),
                if edits_here.contains(&current) && pending > 0 && !applied {
                    1.
                } else {
                    0.
                },
                HOVER_FADE,
            );
            let marker = if edits_here.is_empty() {
                None
            } else {
                let any_pending = edits_here
                    .iter()
                    .any(|ix| decisions[*ix] == Decision::Pending);
                let any_kept = edits_here.iter().any(|ix| decisions[*ix] == Decision::Kept);
                let rest = if any_pending {
                    t.border_strong
                } else if any_kept {
                    t.success.opacity(0.7)
                } else {
                    t.border
                };
                Some(motion::mix(rest, t.accent, focus_t))
            };

            let mut code = div()
                .flex()
                .items_center()
                .h_full()
                .pl(px(space::S12))
                .whitespace_nowrap();
            for piece in pieces {
                code = match piece {
                    Piece::Code(s) => code.children(code_tokens(s, 1., None, &t)),
                    Piece::Edit(ix) => code.child(edit_mark(
                        &key,
                        *ix,
                        &self.edits[*ix],
                        decisions[*ix],
                        vis[*ix],
                        *ix == current && !applied,
                        advance,
                        act.clone(),
                        &t,
                    )),
                };
            }

            rows.push(
                div()
                    .flex()
                    .items_center()
                    .h(px(LINE_H * line_vis))
                    .opacity(line_vis)
                    .overflow_hidden()
                    .child(
                        div()
                            .w(px(GUTTER_W))
                            .flex_none()
                            .flex()
                            .justify_end()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(motion::mix(t.text_faint, t.text_muted, focus_t))
                            .child((self.first_line + line_ix).to_string()),
                    )
                    .child(
                        div()
                            .ml(px(space::S10))
                            .w(px(2.))
                            .h(px(LINE_H - 8.))
                            .flex_none()
                            .rounded(px(1.))
                            .when_some(marker, |d, c| d.bg(c)),
                    )
                    .child(code)
                    .into_any_element(),
            );
        }

        let band_y = motion::follow(&format!("{key}-band"), current_y, GLIDE);
        // The rail sits just past the end of the line, inside the card.
        let rail_x = (GUTTER_W + space::S10 + 2. + space::S12 + current_w + space::S12)
            .min(self.width - RAIL_W - space::S8);
        let rail_x = motion::follow(&format!("{key}-rail-x"), rail_x, GLIDE);
        let band_on = motion::follow(
            &format!("{key}-band-on"),
            if pending > 0 && !applied { 1. } else { 0. },
            MENU_IN,
        );
        let wash = t.accent.opacity(if t.dark { 0.09 } else { 0.05 });

        let code_area = div()
            .relative()
            .py(px(space::S8))
            .bg(t.fill(FillTier::Quaternary))
            .border_y_1()
            .border_color(t.border)
            .font_family(pink::MONO)
            .text_size(px(CODE_SIZE))
            .line_height(px(LINE_H))
            .child(
                // The focus band, gliding from line to line.
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px(space::S8 + band_y))
                    .h(px(LINE_H))
                    .bg(wash.opacity(band_on)),
            )
            .children(rows)
            .when(band_on > 0.01, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(px(rail_x))
                        .top(px(space::S8 + band_y - 1.))
                        .opacity(band_on)
                        .child(rail(&key, act.clone(), &t)),
                )
            });

        // ---- header ----------------------------------------------------
        let (dir, file) = match self.path.rfind('/') {
            Some(i) => (
                SharedString::from(self.path[..=i].to_string()),
                SharedString::from(self.path[i + 1..].to_string()),
            ),
            None => (SharedString::default(), self.path.clone()),
        };
        let pips =
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .children(decisions.iter().enumerate().map(|(ix, d)| {
                    let color = match d {
                        Decision::Pending => t.fill(FillTier::Primary),
                        Decision::Kept => t.success,
                        Decision::Dropped => t.danger.opacity(0.55),
                    };
                    let on = motion::follow(
                        &format!("{key}-pip{ix}"),
                        if ix == current && pending > 0 && !applied {
                            1.
                        } else {
                            0.
                        },
                        GLIDE,
                    );
                    let act = act.clone();
                    div()
                        .id(("pip", ix))
                        .h(px(14.))
                        .px(px(1.))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .on_click(move |_, window, cx| act(Command::Focus(ix), window, cx))
                        .child(
                            div()
                                .h(px(4. + 2. * on))
                                .w(px(10. + 6. * on))
                                .rounded(px(3.))
                                .bg(if *d == Decision::Pending {
                                    motion::mix(color, t.accent, on)
                                } else {
                                    color
                                }),
                        )
                }));
        let undo = {
            let act = act.clone();
            pink::icon_button(format!("{key}-undo"), ICON_UNDO, ControlSize::Md, &t)
                .when(!can_undo, |b| b.opacity(0.35).cursor_default())
                .when(can_undo, |b| {
                    b.on_click(move |_, window, cx| act(Command::Undo, window, cx))
                })
        };
        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S10))
            .px(px(space::S16))
            .py(px(space::S12))
            .child(
                div()
                    .size(px(32.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::MD))
                    .bg(t.fill(FillTier::Tertiary))
                    .child(svg().path(ICON_FILE).size(px(16.)).text_color(t.text_muted)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(space::S6))
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text)
                                    .child(file),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_faint)
                                    .child(dir),
                            ),
                    )
                    .when_some(self.intent.clone(), |d, intent| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S4))
                                .mt(px(2.))
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_muted)
                                .child(
                                    svg()
                                        .path(ICON_SPARK)
                                        .size(px(12.))
                                        .flex_none()
                                        .text_color(t.accent),
                                )
                                .child(div().truncate().child(intent)),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(format!("{} of {n} reviewed", n - pending)),
                    )
                    .child(pips),
            )
            .child(undo);

        // ---- footer ----------------------------------------------------
        let footer = if pending == 0 {
            let summary = if applied {
                format!(
                    "Applied {kept} {} to {}",
                    if kept == 1 { "edit" } else { "edits" },
                    self.path
                )
            } else {
                format!("All {n} reviewed · {kept} kept, {} dropped", n - kept)
            };
            let apply = {
                let act = act.clone();
                pink::button(
                    format!("{key}-apply"),
                    ButtonKind::Primary,
                    ControlSize::Md,
                    &t,
                )
                .child(format!(
                    "Apply {kept} {}",
                    if kept == 1 { "edit" } else { "edits" }
                ))
                .on_click(move |_, window, cx| act(Command::Apply, window, cx))
            };
            div()
                .flex()
                .items_center()
                .gap(px(space::S8))
                .child(
                    svg()
                        .path(ICON_DONE)
                        .size(px(16.))
                        .flex_none()
                        .text_color(t.success),
                )
                .child(
                    div()
                        .flex_1()
                        .truncate()
                        .text_size(px(text::BODY))
                        .text_color(t.text)
                        .child(summary),
                )
                .when(!applied, |d| d.child(apply))
                .with_animation(
                    SharedString::from(format!("{key}-done-{applied}")),
                    MENU_IN.animation(),
                    |el, p| el.opacity(p),
                )
                .into_any_element()
        } else {
            let note = self
                .edits
                .get(current)
                .and_then(|e| e.note.clone())
                .unwrap_or_else(|| "Suggested edit".into());
            let nav = |id: &str, icon: &'static str, delta: isize| {
                let act = act.clone();
                pink::icon_button(format!("{key}-{id}"), icon, ControlSize::Md, &t)
                    .on_click(move |_, window, cx| act(Command::Step(delta), window, cx))
            };
            let all = |id: &str, kind: ButtonKind, label: &'static str, d: Decision| {
                let act = act.clone();
                pink::button(format!("{key}-{id}"), kind, ControlSize::Md, &t)
                    .child(label)
                    .on_click(move |_, window, cx| act(Command::DecideAll(d), window, cx))
            };
            div()
                .flex()
                .items_center()
                .gap(px(space::S8))
                .child(
                    div()
                        .flex_none()
                        .font_family(pink::MONO)
                        .text_size(px(text::CAPTION))
                        .text_color(t.text_faint)
                        .child(format!("{}/{n}", current + 1)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(text::BODY))
                        .text_color(t.text_muted)
                        .child(motion::rise_in(
                            SharedString::from(format!("{key}-note-{current}")),
                            MENU_IN,
                            3.,
                            div().truncate().child(note),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(space::S2))
                        .child(nav("prev", ICON_UP, -1))
                        .child(nav("next", ICON_DOWN, 1)),
                )
                .child(div().w(px(1.)).h(px(16.)).bg(t.border))
                .child(all(
                    "drop-all",
                    ButtonKind::Ghost,
                    "Drop all",
                    Decision::Dropped,
                ))
                .child(all(
                    "keep-all",
                    ButtonKind::Primary,
                    "Keep all",
                    Decision::Kept,
                ))
                .into_any_element()
        };

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
                    "tab" if !k.modifiers.shift => Command::Step(1),
                    "k" | "up" | "tab" => Command::Step(-1),
                    "u" => Command::Undo,
                    _ => return,
                };
                act(command, window, cx);
                cx.stop_propagation();
            }
        };

        motion::keep_alive(window);
        let focus_ring = motion::follow(
            &format!("{key}-focus-ring"),
            if focused { 1. } else { 0. },
            HOVER_FADE,
        );
        pink::card(&t)
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, {
                let focus = focus.clone();
                move |_, window, cx| window.focus(&focus, cx)
            })
            .w(px(self.width))
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .border_color(motion::mix(t.border, t.border_strong, focus_ring))
            .child(header)
            .child(code_area)
            .child(
                div()
                    .px(px(space::S16))
                    .py(px(space::S10))
                    .min_h(px(48.))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(footer),
            )
    }
}

/// Width of a line's code once its decided edits have folded away.
fn line_width(pieces: &[Piece], edits: &[DiffEdit], decisions: &[Decision], advance: f32) -> f32 {
    let chars = |s: &str| s.chars().count() as f32 * advance;
    pieces
        .iter()
        .map(|piece| match piece {
            Piece::Code(s) => chars(s),
            Piece::Edit(ix) => {
                let (e, d) = (&edits[*ix], decisions[*ix]);
                let mut w = 0.;
                let mut sides = 0;
                for (content, shown) in [
                    (&e.before, d != Decision::Kept),
                    (&e.after, d != Decision::Dropped),
                ] {
                    if shown && !content.is_empty() {
                        let trimmed = content.trim_start();
                        w += chars(&content[..content.len() - trimmed.len()]);
                        w += chars(trimmed) + MARK_PAD * 2.;
                        sides += 1;
                    }
                }
                w + if sides == 2 { 2. } else { 0. }
            }
        })
        .sum()
}

/// Plain code as syntax-tinted spans. `tint` pulls every token toward
/// `diff` (a marked span's color) by that amount.
fn code_tokens(code: &str, opacity: f32, tint: Option<(Hsla, f32)>, t: &Pink) -> Vec<Div> {
    tokenize(code)
        .into_iter()
        .map(|(s, kind)| {
            let mut color = token_color(kind, t);
            if let Some((diff, amount)) = tint {
                color = motion::mix(color, diff, amount);
            }
            div()
                .flex_none()
                .text_color(color.opacity(opacity))
                .child(s)
        })
        .collect()
}

/// One edit in place: the struck old words and the new ones, each folding
/// away when the other side wins.
#[allow(clippy::too_many_arguments)]
fn edit_mark(
    key: &str,
    ix: usize,
    edit: &DiffEdit,
    decision: Decision,
    (old_vis, new_vis, tint): (f32, f32, f32),
    current: bool,
    advance: f32,
    act: Rc<dyn Fn(Command, &mut Window, &mut App)>,
    t: &Pink,
) -> impl IntoElement {
    let hover_key = format!("{key}-mark-{ix}");
    let hover = motion::hover_t(&hover_key);
    let ring_on = motion::follow(
        &format!("{key}-mark-{ix}-ring"),
        if current && decision == Decision::Pending {
            1.
        } else {
            0.
        },
        HOVER_FADE,
    );
    let ring_color = motion::mix(
        t.border_strong.opacity(hover * tint),
        t.accent.opacity(0.7),
        ring_on,
    );
    let strength = if t.dark { 0.20 } else { 0.12 };
    let strength = strength + ring_on * 0.06;

    let side = |content: &SharedString, vis: f32, color: Hsla, struck: bool| -> Option<Div> {
        if content.is_empty() || vis <= 0.001 {
            return None;
        }
        // Leading indentation of a whole-line insertion stays unmarked.
        let trimmed = content.trim_start();
        let indent = &content[..content.len() - trimmed.len()];
        // The wash's padding settles away with the tint, so kept code
        // lines up with its neighbours again.
        let pad = MARK_PAD * tint;
        let full = trimmed.chars().count() as f32 * advance + pad * 2.;
        let span = div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(LINE_H - 6.))
            .px(px(pad))
            .rounded(px(radius::XS))
            .overflow_hidden()
            .bg(color.opacity(strength * tint))
            .when(vis < 0.999, |d| d.w(px(full * vis)))
            .when(struck && tint > 0.5, |d| {
                d.line_through().text_decoration_color(color.opacity(0.7))
            })
            .children(code_tokens(trimmed, vis.powf(1.5), Some((color, tint)), t));
        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .when(!indent.is_empty(), |d| d.child(indent.to_string()))
                .child(span),
        )
    };

    let old = side(&edit.before, old_vis, t.danger, true);
    let new = side(&edit.after, new_vis, t.success, false);
    let both = old.is_some() && new.is_some();
    div()
        .id(SharedString::from(hover_key.clone()))
        .flex()
        .flex_none()
        .items_center()
        .gap(px(if both {
            2. * old_vis.min(new_vis) * tint
        } else {
            0.
        }))
        .rounded(px(radius::XS + 1.))
        .when(ring_color.a > 0.01, |d| {
            d.shadow(vec![ring(1., ring_color)])
        })
        .cursor_pointer()
        .on_hover(motion::hover_listener(hover_key))
        .on_click(move |_, window, cx| act(Command::Focus(ix), window, cx))
        .children(old)
        .children(new)
}

/// The floating Keep / Drop rail beside the edit in focus.
fn rail(key: &str, act: Rc<dyn Fn(Command, &mut Window, &mut App)>, t: &Pink) -> impl IntoElement {
    let action = |id: &str,
                  icon: &'static str,
                  label: &'static str,
                  hint: &'static str,
                  d: Decision,
                  tone: Hsla| {
        let act = act.clone();
        let hover_key = format!("{key}-rail-{id}");
        let icon_color = motion::hover_blend(&hover_key, t.text_muted, tone);
        div()
            .id(SharedString::from(hover_key.clone()))
            .flex()
            .items_center()
            .gap(px(space::S4))
            .h(px(20.))
            .pl(px(space::S6))
            .pr(px(space::S4))
            .rounded(px(radius::XS))
            .cursor_pointer()
            .font_family(pink::SANS)
            .text_size(px(text::FOOTNOTE))
            .font_weight(FontWeight::MEDIUM)
            .hover_fill(
                hover_key,
                t.fill(FillTier::Tertiary).opacity(0.),
                t.fill(FillTier::Tertiary),
            )
            .on_click(move |_, window, cx| act(Command::Decide(d), window, cx))
            .child(svg().path(icon).size(px(12.)).text_color(icon_color))
            .child(label)
            .child(
                div()
                    .ml(px(2.))
                    .px(px(4.))
                    .h(px(14.))
                    .flex()
                    .items_center()
                    .rounded(px(3.))
                    .bg(t.fill(FillTier::Tertiary))
                    .font_family(pink::MONO)
                    .text_size(px(10.))
                    .text_color(t.text_faint)
                    .child(hint),
            )
    };
    pink::overlay(t)
        .flex()
        .items_center()
        .gap(px(1.))
        .p(px(2.))
        .rounded(px(radius::SM))
        .text_color(t.text)
        .child(action(
            "keep",
            ICON_KEEP,
            "Keep",
            "Y",
            Decision::Kept,
            t.success,
        ))
        .child(div().w(px(1.)).h(px(12.)).bg(t.border))
        .child(action(
            "drop",
            ICON_DROP,
            "Drop",
            "N",
            Decision::Dropped,
            t.danger,
        ))
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
    fn undo_restores_batches() {
        let mut s = Review::new(3);
        s.decide(&[0], Decision::Kept);
        assert_eq!(s.current, 1);
        s.decide(&[1, 2], Decision::Dropped);
        assert_eq!(s.pending(), 0);
        s.undo();
        assert_eq!(s.decisions[1], Decision::Pending);
        assert_eq!(s.decisions[0], Decision::Kept);
    }
}
