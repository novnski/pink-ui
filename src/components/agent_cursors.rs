//! Agent Cursors – agents editing a document next to you, like teammates in a
//! multiplayer editor.
//!
//! Each agent has a named pointer that glides to where it works. It drags
//! across a phrase to select it and types its suggestion letter by letter,
//! grabs a figure's corner to resize it, fixes a value in a code block, or
//! highlights a phrase and writes a note. Every change lands as a card in the
//! margin, lined up with its spot in the text, to accept or dismiss. Click an
//! agent in the header to follow it: the others fade back.
//!
//! ```ignore
//! AgentCursors::new("doc", "Incidents", "INC-2291")
//!     .agent(CursorAgent::new("Scribe"))
//!     .block(DocBlock::Paragraph(vec![Run::text("Slow for "), Run::Edit(0), Run::text(".")]))
//!     .edit(DocEdit::replace(0, "about 40 minutes", "47 minutes").at(0.5))
//!     .on_review(|edit, accepted, _, _| println!("{edit}: {accepted}"))
//! ```
//!
//! Pink has no multiplayer pointers, so the parts come from its nearest
//! pieces: the page is its markdown preview (14/22 prose, the H1/H3 scale,
//! the code surface), the margin cards are its review-note rows (Pink:
//! badges.rs `BadgeCard::row`), agents wear the project tints and monogram
//! tiles (Pink: sidebar_projects.rs `project_tile`), name tags are tooltip
//! cards and the header is a surface toolbar with the titlebar's crumb.
//!
//! Pointers find their targets by measuring the laid-out text (each target
//! records its bounds when painted) and move on a critically damped spring,
//! so they keep up with targets that shift while the page reflows.

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Bounds, Div, ElementId, FontWeight, Hsla, IntoElement, PathBuilder,
    Pixels, Point, Role, SharedString, Stateful, Task, Window, canvas, div, point, prelude::*, px,
    relative,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt, EASE_IN_OUT},
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, Syntax, TextStyle, TypeStyleExt, layout,
        loaders, radius, space, text,
    },
    story,
};

pub const NAME: &str = "Agent Cursors";
pub const DESCRIPTION: &str = "Agents edit a shared document like teammates – tinted pointers glide to their spot, type suggestions letter by letter, resize a figure and fix a config, and every change waits in a margin note to accept or dismiss";

const ICON_CLOSE: &str = "icons/agent-cursors/close.svg";
const ICON_FOLLOW: &str = "icons/agent-cursors/follow.svg";

const WIDTH: f32 = 800.0;
const PAGE_W: f32 = 528.0;
const PAGE_PAD: f32 = space::S32;
const FIGURE_RATIO: f32 = 0.36;
/// Edit timeline: glide in, select, then work.
const APPROACH: f32 = 1.15;
const SELECT: f32 = 0.55;
const LINGER: f32 = 0.8;
const TYPE_EVERY: f32 = 0.085;

/// Pink's markdown prose, 14/22 (Pink: markdown/render.rs `MD_TEXT_SIZE`).
const PROSE_SIZE: f32 = 14.0;
const PROSE_LINE: f32 = 22.0;
/// Pink's gap between markdown blocks (`MD_BLOCK_GAP`).
const BLOCK_GAP: f32 = 12.0;
/// Markdown H1 and H3, and the extra air above an H3
/// (Pink: markdown/render.rs `heading_metrics`, `heading_top_extra`).
const H1: (f32, f32) = (19.0, 27.0);
const H3: (f32, f32) = (15.0, 22.0);
const H3_TOP_EXTRA: f32 = 8.0;
/// Code surface metrics (Pink: markdown/render.rs `CODE_*`).
const CODE_LINE: f32 = 18.0;
const CODE_PAD_Y: f32 = 10.0;
const CODE_HEADER: f32 = 28.0;
/// The advance of a space in 14 px Geist: words are laid out one box each.
const WORD_GAP: f32 = 3.8;
/// The advance of a space in Geist Mono (0.6 em) at the code size.
const MONO_GAP: f32 = text::CODE * 0.6;
/// Margin cards (Pink: badges.rs `BadgeCard::row`): `p 8`, radius 6, gap 8
/// to a 2 px bar, a column with gap 4.
const NOTE_PAD: f32 = 8.0;
const NOTE_RADIUS: f32 = 6.0;
const NOTE_BAR: f32 = 2.0;
const NOTE_GAP: f32 = 4.0;
/// The review row under a note: the column gap, 2 px of air and a small
/// button. The column spaces its rows with top margins, not a gap, so this
/// row can fold open from nothing (a negative margin would make GPUI measure
/// the column as empty).
const NOTE_ACTIONS: f32 = NOTE_GAP + space::S2 + ControlSize::Sm.height();
/// The note header row holds an 18 px tile (Pink's sidebar tile size).
const NOTE_TILE: f32 = 18.0;
/// Header agent buttons hold a 20 px tile in a 28 px chrome control.
const AGENT_TILE: f32 = 20.0;

/// One agent in the document. Agents take Pink's project tints in the order
/// they are added.
#[derive(Clone, Debug)]
pub struct CursorAgent {
    pub name: SharedString,
}

impl CursorAgent {
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self { name: name.into() }
    }
}

/// What an agent does to the document.
#[derive(Clone, Debug)]
pub enum EditKind {
    /// Selects `old` and types `new` over it, as a suggestion.
    Replace {
        old: SharedString,
        new: SharedString,
    },
    /// Highlights `text` and leaves `note` in the margin.
    Comment {
        text: SharedString,
        note: SharedString,
    },
    /// Drags the figure's corner from one width to another.
    Resize { from: f32, to: f32 },
}

#[derive(Clone, Debug)]
pub struct DocEdit {
    pub agent: usize,
    pub kind: EditKind,
    /// Seconds after mount when the agent starts on it.
    pub at: f32,
    /// One line for the margin card (resizes).
    pub summary: SharedString,
}

impl DocEdit {
    pub fn replace(
        agent: usize,
        old: impl Into<SharedString>,
        new: impl Into<SharedString>,
    ) -> Self {
        Self {
            agent,
            kind: EditKind::Replace {
                old: old.into(),
                new: new.into(),
            },
            at: 0.0,
            summary: SharedString::default(),
        }
    }

    pub fn comment(
        agent: usize,
        text: impl Into<SharedString>,
        note: impl Into<SharedString>,
    ) -> Self {
        Self {
            agent,
            kind: EditKind::Comment {
                text: text.into(),
                note: note.into(),
            },
            at: 0.0,
            summary: SharedString::default(),
        }
    }

    pub fn resize(agent: usize, from: f32, to: f32) -> Self {
        Self {
            agent,
            kind: EditKind::Resize { from, to },
            at: 0.0,
            summary: SharedString::default(),
        }
    }

    pub fn at(mut self, seconds: f32) -> Self {
        self.at = seconds;
        self
    }

    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = summary.into();
        self
    }

    fn work(&self) -> f32 {
        match &self.kind {
            EditKind::Replace { new, .. } => new.chars().count() as f32 * TYPE_EVERY + 0.2,
            EditKind::Resize { .. } => 1.35,
            EditKind::Comment { note, .. } => note.chars().count() as f32 * 0.028 + 0.2,
        }
    }

    fn busy_until(&self) -> f32 {
        self.at + APPROACH + SELECT + self.work() + LINGER
    }
}

/// Inline content of a paragraph or code line.
#[derive(Clone, Debug)]
pub enum Run {
    Text(SharedString),
    /// The span of the edit at this index (a replace or a comment).
    Edit(usize),
}

impl Run {
    pub fn text(text: impl Into<SharedString>) -> Self {
        Run::Text(text.into())
    }
}

#[derive(Clone, Debug)]
pub enum DocBlock {
    Title(SharedString),
    Meta(SharedString),
    Heading(SharedString),
    Paragraph(Vec<Run>),
    /// A latency chart; `edit` is the index of a resize edit.
    Figure {
        caption: SharedString,
        edit: Option<usize>,
        width: f32,
    },
    Code {
        file: SharedString,
        lines: Vec<Vec<Run>>,
    },
}

type ReviewHandler = Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct AgentCursors {
    id: ElementId,
    folder: SharedString,
    title: SharedString,
    agents: Vec<CursorAgent>,
    blocks: Vec<DocBlock>,
    edits: Vec<DocEdit>,
    loop_after: Option<f32>,
    on_review: Option<ReviewHandler>,
}

impl AgentCursors {
    pub fn new(
        id: impl Into<ElementId>,
        folder: impl Into<SharedString>,
        title: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            folder: folder.into(),
            title: title.into(),
            agents: Vec::new(),
            blocks: Vec::new(),
            edits: Vec::new(),
            loop_after: None,
            on_review: None,
        }
    }

    pub fn agent(mut self, agent: CursorAgent) -> Self {
        self.agents.push(agent);
        self
    }

    pub fn block(mut self, block: DocBlock) -> Self {
        self.blocks.push(block);
        self
    }

    pub fn edit(mut self, edit: DocEdit) -> Self {
        self.edits.push(edit);
        self
    }

    /// Demo: replays everything after `seconds` unless someone has reviewed a card.
    pub fn loop_after(mut self, seconds: f32) -> Self {
        self.loop_after = Some(seconds);
        self
    }

    /// Called with the edit index and `true` when accepted, `false` when dismissed.
    pub fn on_review(
        mut self,
        handler: impl Fn(usize, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_review = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

type Anchors = Rc<RefCell<HashMap<String, Bounds<Pixels>>>>;

#[derive(Clone, Copy, Debug, Default)]
struct Pointer {
    pos: (f32, f32),
    vel: (f32, f32),
    placed: bool,
}

struct CursorsState {
    clock: f32,
    last_tick: Instant,
    last_frame: Instant,
    generation: usize,
    loop_after: Option<f32>,
    decisions: Vec<Option<bool>>,
    following: Option<usize>,
    pointers: Vec<Pointer>,
    anchors: Anchors,
    on_review: Option<ReviewHandler>,
    _ticker: Task<()>,
}

impl CursorsState {
    fn new(config: &AgentCursors, cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                if this.update(cx, |state, cx| state.tick(cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            clock: 0.0,
            last_tick: Instant::now(),
            last_frame: Instant::now(),
            generation: 0,
            loop_after: config.loop_after,
            decisions: vec![None; config.edits.len()],
            following: None,
            pointers: vec![Pointer::default(); config.agents.len()],
            anchors: Rc::default(),
            on_review: None,
            _ticker: ticker,
        }
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.clock += now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .min(0.25);
        self.last_tick = now;
        if let Some(limit) = self.loop_after
            && self.clock >= limit
        {
            self.clock = 0.0;
            self.generation += 1;
            self.decisions.iter_mut().for_each(|d| *d = None);
        }
        cx.notify();
    }

    fn review(&mut self, ix: usize, accept: bool) {
        self.loop_after = None;
        if let Some(decision) = self.decisions.get_mut(ix) {
            *decision = Some(accept);
        }
    }

    fn anchor(&self, key: &str) -> Option<Bounds<Pixels>> {
        let anchors = self.anchors.borrow();
        let body = anchors.get("body")?;
        anchors.get(key).map(|b| Bounds {
            origin: b.origin - body.origin,
            size: b.size,
        })
    }
}

/// Where an edit stands at a moment.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stage {
    Waiting,
    Approach,
    /// Drag-selecting the span, 0..1.
    Select(f32),
    /// Doing the work, 0..1.
    Work(f32),
    Review,
    Accepted,
    Dismissed,
}

impl Stage {
    fn of(edit: &DocEdit, clock: f32, decision: Option<bool>) -> Stage {
        match decision {
            Some(true) => return Stage::Accepted,
            Some(false) => return Stage::Dismissed,
            None => {}
        }
        let t = clock - edit.at;
        if t < 0.0 {
            Stage::Waiting
        } else if t < APPROACH {
            Stage::Approach
        } else if t < APPROACH + SELECT {
            Stage::Select((t - APPROACH) / SELECT)
        } else if t < APPROACH + SELECT + edit.work() {
            Stage::Work((t - APPROACH - SELECT) / edit.work())
        } else {
            Stage::Review
        }
    }

    fn started(self) -> bool {
        !matches!(self, Stage::Waiting | Stage::Approach)
    }

    fn busy(self) -> bool {
        matches!(self, Stage::Select(_) | Stage::Work(_))
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// An agent's colors: Pink's project tint in order, as authored for washes
/// and floored for glyphs (pointer, caret, bar) and for text.
#[derive(Clone, Copy)]
struct Tint {
    raw: Hsla,
    glyph: Hsla,
    text: Hsla,
}

impl Tint {
    fn of(ix: usize, t: &Pink) -> Self {
        let raw = pink::project_tint(ix);
        Tint {
            raw,
            glyph: t.readable_icon(raw),
            text: t.readable_tone(raw, 1.0),
        }
    }
}

/// An invisible layer that records its parent's bounds under `key` when painted.
fn anchor(anchors: &Anchors, key: String) -> impl IntoElement {
    let anchors = anchors.clone();
    canvas(
        move |bounds, _, _| {
            anchors.borrow_mut().insert(key, bounds);
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The pointer arrow, tip at `tip`.
fn paint_arrow(window: &mut Window, tip: Point<Pixels>, scale: f32, fill: Hsla, edge: Hsla) {
    const SHAPE: [(f32, f32); 7] = [
        (0.0, 0.0),
        (0.0, 15.5),
        (4.1, 11.9),
        (6.9, 18.1),
        (9.7, 16.9),
        (6.9, 10.9),
        (12.2, 10.9),
    ];
    let points: Vec<Point<Pixels>> = SHAPE
        .iter()
        .map(|(x, y)| tip + point(px(x * scale), px(y * scale)))
        .collect();
    let mut outline = PathBuilder::stroke(px(2.6));
    outline.add_polygon(&points, true);
    if let Ok(path) = outline.build() {
        window.paint_path(path, edge);
    }
    let mut body = PathBuilder::fill();
    body.add_polygon(&points, true);
    if let Ok(path) = body.build() {
        window.paint_path(path, fill);
    }
}

/// Words of running text, glued where there's no space (punctuation after a span).
struct Flow {
    groups: Vec<Vec<AnyElement>>,
    glue: bool,
}

impl Flow {
    fn new() -> Self {
        Flow {
            groups: Vec::new(),
            glue: false,
        }
    }

    fn push(&mut self, el: AnyElement, glue: bool) {
        match self.groups.last_mut() {
            Some(last) if glue => last.push(el),
            _ => self.groups.push(vec![el]),
        }
    }

    fn text(&mut self, text: &str, word: impl Fn(String) -> AnyElement) {
        let leading = text.starts_with(char::is_whitespace);
        let words: Vec<&str> = text.split_whitespace().collect();
        for (i, w) in words.iter().enumerate() {
            let glue = i == 0 && self.glue && !leading;
            self.push(word(w.to_string()), glue);
        }
        if !words.is_empty() {
            self.glue = !text.ends_with(char::is_whitespace);
        } else if !text.is_empty() {
            self.glue = false;
        }
    }

    fn span(&mut self, el: AnyElement) {
        let glue = self.glue;
        self.push(el, glue);
        self.glue = true;
    }

    fn render(self) -> Div {
        div().flex().flex_wrap().gap_x(px(WORD_GAP)).children(
            self.groups
                .into_iter()
                .map(|group| div().flex().flex_none().children(group)),
        )
    }
}

/// Pink's code colors for a TOML line: table headers as types, keys as
/// properties, `=` as punctuation, numbers and strings by kind.
fn code_tokens(text: &str, syntax: &Syntax, plain: Hsla) -> Vec<(String, Hsla)> {
    fn value(v: &str, syntax: &Syntax, plain: Hsla) -> Hsla {
        let v = v.trim();
        if !v.is_empty()
            && v.chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '_')
        {
            syntax.number
        } else if v.starts_with('"') {
            syntax.string
        } else {
            plain
        }
    }
    if text.trim_start().starts_with('[') {
        return vec![(text.to_string(), syntax.type_name)];
    }
    match text.split_once('=') {
        Some((key, rest)) => {
            let mut out = vec![
                (key.to_string(), syntax.property),
                ("=".to_string(), syntax.punctuation),
            ];
            if !rest.is_empty() {
                out.push((rest.to_string(), value(rest, syntax, plain)));
            }
            out
        }
        None => vec![(text.to_string(), value(text, syntax, plain))],
    }
}

/// Where each edit sits, for its card: "¶ 2", "Figure 1", "gateway.toml:2"
/// (Pink's review notes name their spot in mono, Pink: comment_ui.rs).
fn locations(blocks: &[DocBlock], count: usize) -> Vec<SharedString> {
    let mut out = vec![SharedString::default(); count];
    let mut set = |ix: usize, label: String| {
        if let Some(slot) = out.get_mut(ix) {
            *slot = label.into();
        }
    };
    let (mut paragraphs, mut figures) = (0, 0);
    for block in blocks {
        match block {
            DocBlock::Paragraph(runs) => {
                paragraphs += 1;
                for run in runs {
                    if let Run::Edit(ix) = run {
                        set(*ix, format!("¶ {paragraphs}"));
                    }
                }
            }
            DocBlock::Figure { edit, .. } => {
                figures += 1;
                if let Some(ix) = edit {
                    set(*ix, format!("Figure {figures}"));
                }
            }
            DocBlock::Code { file, lines } => {
                let name = file.rsplit('/').next().unwrap_or_default();
                for (line_ix, line) in lines.iter().enumerate() {
                    for run in line {
                        if let Run::Edit(ix) = run {
                            set(*ix, format!("{name}:{}", line_ix + 1));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// p95 latency, with the slowdown as a spike over the target line.
fn chart(line: Hsla, fill: Hsla, target: Hsla, grid: Hsla) -> impl IntoElement {
    const SERIES: [f32; 26] = [
        210., 220., 205., 230., 215., 260., 420., 980., 1620., 1840., 1910., 1760., 1880., 1690.,
        1540., 1720., 1210., 640., 330., 250., 230., 215., 220., 210., 225., 215.,
    ];
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
            let o = bounds.origin;
            let (pad_x, pad_top, pad_bottom) = (10.0, 12.0, 10.0);
            let max = 2100.0;
            let at = |ix: usize, v: f32| {
                let x = pad_x + (w - pad_x * 2.0) * ix as f32 / (SERIES.len() - 1) as f32;
                let y = pad_top + (h - pad_top - pad_bottom) * (1.0 - v / max);
                o + point(px(x), px(y))
            };
            for frac in [0.25, 0.5, 0.75] {
                let y = pad_top + (h - pad_top - pad_bottom) * frac;
                let mut g = PathBuilder::stroke(px(1.));
                g.move_to(o + point(px(pad_x), px(y)));
                g.line_to(o + point(px(w - pad_x), px(y)));
                if let Ok(path) = g.build() {
                    window.paint_path(path, grid);
                }
            }
            let points: Vec<Point<Pixels>> =
                SERIES.iter().enumerate().map(|(i, v)| at(i, *v)).collect();
            let mut area = PathBuilder::fill();
            area.move_to(o + point(px(pad_x), px(h - pad_bottom)));
            for p in &points {
                area.line_to(*p);
            }
            area.line_to(o + point(px(w - pad_x), px(h - pad_bottom)));
            area.close();
            if let Ok(path) = area.build() {
                window.paint_path(path, fill);
            }
            let mut slo = PathBuilder::stroke(px(1.)).dash_array(&[px(4.), px(3.)]);
            let y = at(0, 800.0).y;
            slo.move_to(point(o.x + px(pad_x), y));
            slo.line_to(point(o.x + px(w - pad_x), y));
            if let Ok(path) = slo.build() {
                window.paint_path(path, target);
            }
            let mut stroke = PathBuilder::stroke(px(1.5));
            stroke.add_polygon(&points, false);
            if let Ok(path) = stroke.build() {
                window.paint_path(path, line);
            }
        },
    )
    .size_full()
}

/// The typing caret: Pink's 2 px caret, solid while typing
/// (Pink: composer.rs `CARET_WIDTH`, `caret_visible`).
fn caret(color: Hsla, height: f32) -> Div {
    div()
        .w(px(layout::CARET_WIDTH))
        .h(px(height))
        .flex_none()
        .bg(color)
}

/// A header button for one agent: its monogram tile in a 28 px chrome
/// control, `hover_wash` at rest and `fill(Primary)` while followed
/// (Pink: chrome rows' hover and selected fills).
fn agent_button(
    key: String,
    agent: &CursorAgent,
    tint: Tint,
    following: bool,
    faded: bool,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    let hover = t.fill(FillTier::Tertiary);
    let (rest, hover) = if following {
        (t.fill(FillTier::Primary), t.fill(FillTier::Primary))
    } else {
        (hover.opacity(0.0), hover)
    };
    let label: SharedString = if following {
        format!("Stop following {}", agent.name).into()
    } else {
        format!("Follow {}", agent.name).into()
    };
    div()
        .id(ElementId::Name(key.clone().into()))
        .role(Role::Button)
        .aria_label(label.clone())
        .size(px(pink::chrome::CONTROL_M))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(pink::chrome::RADIUS_M))
        .tab_index(0)
        .cursor_pointer()
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .hover_fill(key, rest, hover)
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(
            pink::monogram_tile(tint.raw, &agent.name, AGENT_TILE, t).opacity(if faded {
                0.5
            } else {
                1.0
            }),
        )
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl RenderOnce for AgentCursors {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "cursors".into()),
            cx,
            |_, cx| CursorsState::new(&self, cx),
        );
        let on_review = self.on_review.clone();
        state.update(cx, |s, _| s.on_review = on_review);

        let (clock, generation, following, anchors) = {
            let s = state.read(cx);
            (s.clock, s.generation, s.following, s.anchors.clone())
        };
        let decisions = state.read(cx).decisions.clone();
        let stages: Vec<Stage> = self
            .edits
            .iter()
            .enumerate()
            .map(|(ix, e)| Stage::of(e, clock, decisions.get(ix).copied().flatten()))
            .collect();
        let tints: Vec<Tint> = (0..self.agents.len()).map(|ix| Tint::of(ix, &t)).collect();
        let tint_of = |agent: usize| tints.get(agent).copied().unwrap_or_else(|| Tint::of(0, &t));
        let dim_of = |agent: usize| match following {
            Some(f) if f != agent => 0.3,
            _ => 1.0,
        };
        let locations = locations(&self.blocks, self.edits.len());
        let syntax = t.syntax();
        // Agent washes share Pink's own alphas: a drag selection at the
        // selection token's, a suggestion's underlay at the inline-code
        // plate's (Pink: popover.rs match highlight).
        let select_alpha = t.selection.a;
        let underlay_alpha = t.code_wash.a;

        // -- Document ----------------------------------------------------------
        let word = |w: String| {
            div()
                .flex_none()
                .whitespace_nowrap()
                .child(w)
                .into_any_element()
        };
        let span_for = |ix: usize, mono: bool| -> AnyElement {
            let Some(edit) = self.edits.get(ix) else {
                return div().into_any_element();
            };
            let stage = stages[ix];
            let tint = tint_of(edit.agent);
            let underlay = tint.raw.opacity(underlay_alpha);
            let base = div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .gap_x(px(if mono { MONO_GAP } else { WORD_GAP }))
                .whitespace_nowrap()
                .child(anchor(&anchors, format!("edit-{ix}")));
            let selection = |p: f32| {
                div()
                    .absolute()
                    .top(px(1.))
                    .bottom(px(1.))
                    .left_0()
                    .w(relative(p))
                    .rounded(px(3.))
                    .bg(tint.raw.opacity(select_alpha))
            };
            match &edit.kind {
                EditKind::Replace { old, new } => {
                    let typed = match stage {
                        Stage::Work(p) => {
                            let n = new.chars().count();
                            let shown = ((p * n as f32) as usize + 1).min(n);
                            new.chars().take(shown).collect::<String>()
                        }
                        _ => new.to_string(),
                    };
                    match stage {
                        Stage::Accepted => base.child(new.clone()).into_any_element(),
                        Stage::Waiting | Stage::Approach | Stage::Dismissed => {
                            base.child(old.clone()).into_any_element()
                        }
                        Stage::Select(p) => base
                            .child(selection(p))
                            .child(div().relative().child(old.clone()))
                            .into_any_element(),
                        // The suggestion: the old words struck in the faint
                        // tone, the new ones in the agent's tint on its
                        // underlay, the caret riding the end while it types.
                        Stage::Work(_) | Stage::Review => base
                            .child(
                                div()
                                    .text_color(t.readable_faint(0.9))
                                    .line_through()
                                    .text_decoration_color(t.content_faint)
                                    .child(old.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px(px(1.))
                                    .rounded(px(3.))
                                    .bg(underlay)
                                    .text_color(tint.text)
                                    .child(typed)
                                    .when(matches!(stage, Stage::Work(_)), |el| {
                                        el.child(caret(tint.glyph, if mono { 14.0 } else { 17.0 }))
                                    }),
                            )
                            .into_any_element(),
                    }
                }
                EditKind::Comment { text: quoted, .. } => match stage {
                    Stage::Select(p) => base
                        .child(selection(p))
                        .child(div().relative().child(quoted.clone()))
                        .into_any_element(),
                    Stage::Work(_) | Stage::Review => base
                        .child(
                            div()
                                .relative()
                                .rounded(px(3.))
                                .bg(underlay)
                                .child(quoted.clone())
                                .child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .right_0()
                                        .bottom(px(1.))
                                        .h(px(NOTE_BAR))
                                        .bg(tint.glyph),
                                ),
                        )
                        .into_any_element(),
                    _ => base.child(quoted.clone()).into_any_element(),
                },
                EditKind::Resize { .. } => base.into_any_element(),
            }
        };

        let mut figure_box: Option<(f32, f32)> = None;
        let mut doc = Vec::new();
        for (bix, block) in self.blocks.iter().enumerate() {
            let el: AnyElement = match block {
                DocBlock::Title(title) => div()
                    .relative()
                    .text_size(px(H1.0))
                    .line_height(px(H1.1))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(anchor(&anchors, "title".into()))
                    .child(title.clone())
                    .into_any_element(),
                DocBlock::Meta(meta) => div()
                    .mt(px(space::S4))
                    .type_style(TextStyle::Footnote)
                    .text_color(t.content_faint)
                    .child(meta.clone())
                    .into_any_element(),
                DocBlock::Heading(heading) => div()
                    .mt(px(BLOCK_GAP + H3_TOP_EXTRA))
                    .text_size(px(H3.0))
                    .line_height(px(H3.1))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(heading.clone())
                    .into_any_element(),
                DocBlock::Paragraph(runs) => {
                    let mut flow = Flow::new();
                    for run in runs {
                        match run {
                            Run::Text(text) => flow.text(text, word),
                            Run::Edit(ix) => flow.span(span_for(*ix, false)),
                        }
                    }
                    flow.render()
                        .relative()
                        .mt(px(BLOCK_GAP))
                        .text_size(px(PROSE_SIZE))
                        .line_height(px(PROSE_LINE))
                        .text_color(t.text)
                        .child(anchor(&anchors, format!("block-{bix}")))
                        .into_any_element()
                }
                DocBlock::Figure {
                    caption,
                    edit,
                    width,
                } => {
                    let resize = edit.and_then(|ix| {
                        self.edits.get(ix).and_then(|e| match e.kind {
                            EditKind::Resize { from, to } => {
                                Some((stages[ix], from, to, tint_of(e.agent)))
                            }
                            _ => None,
                        })
                    });
                    let (target, active, tint) = match resize {
                        Some((stage, from, to, tint)) => {
                            // The corner follows the drag; a dismissed resize
                            // springs back, an accepted one stays.
                            let target = match stage {
                                Stage::Work(p) => from + (to - from) * EASE_IN_OUT.eval(p),
                                Stage::Review | Stage::Accepted => to,
                                _ => from,
                            };
                            (target, stage.busy(), Some(tint))
                        }
                        None => (*width, false, None),
                    };
                    // A figure exactly as wide as the column sends GPUI's layout
                    // into a runaway height: keep it a hair narrower.
                    let w = motion::follow(&format!("{key}-fig-w"), target, motion::RESIZE)
                        .min(PAGE_W - PAGE_PAD * 2.0 - 2.0);
                    let h = w * FIGURE_RATIO;
                    figure_box = Some((w, h));
                    let handle_color = tint.map(|c| c.glyph).unwrap_or(t.focus_ring());
                    let handle = |left: bool, top: bool| {
                        div()
                            .absolute()
                            .size(px(8.))
                            .when(left, |el| el.left(px(-4.)))
                            .when(!left, |el| el.right(px(-4.)))
                            .when(top, |el| el.top(px(-4.)))
                            .when(!top, |el| el.bottom(px(-4.)))
                            .rounded(px(2.))
                            .bg(t.bg)
                            .border_1()
                            .border_color(handle_color)
                    };
                    div()
                        .mt(px(BLOCK_GAP))
                        .flex()
                        .flex_col()
                        .gap(px(space::S8))
                        .child(
                            // The figure plate is Pink's code surface; the
                            // grab reads as Pink's selection ring, 2 px wide
                            // and 2 px outside, in the agent's tint.
                            pink::code_surface(&t)
                                .relative()
                                .w(px(w))
                                .h(px(h))
                                .child(anchor(&anchors, "figure".into()))
                                .child(chart(
                                    t.content_muted,
                                    t.ink(0.05),
                                    t.readable_icon(t.warning),
                                    t.separator(false),
                                ))
                                .when(active, |el| {
                                    el.child(
                                        div()
                                            .absolute()
                                            .top(px(-4.))
                                            .left(px(-4.))
                                            .right(px(-4.))
                                            .bottom(px(-4.))
                                            .rounded(px(radius::PANEL + 4.0))
                                            .border_2()
                                            .border_color(handle_color),
                                    )
                                    .child(handle(true, true))
                                    .child(handle(false, true))
                                    .child(handle(true, false))
                                    .child(handle(false, false))
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S6))
                                .type_style(TextStyle::Footnote)
                                .text_color(t.content_faint)
                                // A bare string in a flex row can be measured at zero
                                // width and wrap per letter: keep it in its own box.
                                .child(div().flex_none().whitespace_nowrap().child(caption.clone()))
                                .when_some(tint.filter(|_| active), |el, tint| {
                                    el.child(
                                        div()
                                            .flex_none()
                                            .font_family(pink::MONO)
                                            .text_size(px(text::CAPTION))
                                            .text_color(tint.text)
                                            .child(format!("{:.0} px", w)),
                                    )
                                }),
                        )
                        .into_any_element()
                }
                DocBlock::Code { file, lines } => {
                    let rows = lines.iter().map(|line| {
                        let mut row = div()
                            .flex()
                            .items_center()
                            .h(px(CODE_LINE))
                            .whitespace_nowrap();
                        for run in line {
                            row = match run {
                                Run::Text(text) => row.children(
                                    code_tokens(text, &syntax, t.text)
                                        .into_iter()
                                        .map(|(part, color)| div().text_color(color).child(part)),
                                ),
                                Run::Edit(ix) => row.child(span_for(*ix, true)),
                            };
                        }
                        row
                    });
                    // Pink: markdown/render.rs `code_block_frame` + `code_block_header`.
                    pink::code_surface(&t)
                        .relative()
                        .mt(px(BLOCK_GAP))
                        .overflow_hidden()
                        .child(anchor(&anchors, format!("block-{bix}")))
                        .child(
                            div()
                                .h(px(CODE_HEADER))
                                .pl(px(pink::chrome::CONTENT_INSET))
                                .pr(px(5.))
                                .flex()
                                .items_center()
                                .border_b_1()
                                .border_color(t.border)
                                .bg(t.ink(0.02))
                                .font_family(pink::MONO)
                                .text_size(px(text::CAPTION))
                                .text_color(t.content_muted)
                                .child(file.clone()),
                        )
                        .child(
                            div()
                                .px(px(pink::chrome::CONTENT_INSET))
                                .py(px(CODE_PAD_Y))
                                .font_family(pink::MONO)
                                .text_size(px(text::CODE))
                                .line_height(px(CODE_LINE))
                                .text_color(t.text)
                                .children(rows),
                        )
                        .into_any_element()
                }
            };
            doc.push(el);
        }

        // -- Pointers ----------------------------------------------------------
        let (body_w, body_h) = state
            .read(cx)
            .anchors
            .borrow()
            .get("body")
            .map(|b| (f32::from(b.size.width), f32::from(b.size.height)))
            .unwrap_or((WIDTH, 560.0));
        let mut targets = Vec::new();
        for (agent_ix, _) in self.agents.iter().enumerate() {
            let active = self.edits.iter().enumerate().rfind(|(ix, e)| {
                e.agent == agent_ix
                    && clock >= e.at
                    && clock < e.busy_until()
                    && !matches!(stages[*ix], Stage::Accepted | Stage::Dismissed)
            });
            let s = state.read(cx);
            let mut target = None;
            let mut stiffness = 6.0;
            let mut pressed = false;
            if let Some((ix, edit)) = active {
                let stage = stages[ix];
                match &edit.kind {
                    EditKind::Resize { .. } => {
                        if let (Some(fig), Some((w, h))) = (s.anchor("figure"), figure_box) {
                            let x = f32::from(fig.origin.x) + w;
                            let y = f32::from(fig.origin.y) + h;
                            target = Some((x - 1.0, y - 1.0));
                            match stage {
                                Stage::Work(_) => {
                                    stiffness = 40.0;
                                    pressed = true;
                                }
                                Stage::Select(_) => pressed = true,
                                Stage::Approach => stiffness = 7.0,
                                _ => {}
                            }
                        }
                    }
                    _ => {
                        if let Some(b) = s.anchor(&format!("edit-{ix}")) {
                            let left = f32::from(b.origin.x);
                            let right = left + f32::from(b.size.width);
                            let y = f32::from(b.origin.y) + f32::from(b.size.height) * 0.75;
                            target = Some(match stage {
                                Stage::Approach => {
                                    stiffness = 7.0;
                                    (left, y)
                                }
                                Stage::Select(p) => {
                                    stiffness = 40.0;
                                    pressed = true;
                                    (left + (right - left) * p, y)
                                }
                                _ => {
                                    stiffness = 16.0;
                                    (right + 4.0, y + 4.0)
                                }
                            });
                        }
                    }
                }
            }
            let target = target.unwrap_or_else(|| {
                // Idle: drift between a few resting spots on the page.
                // Each agent rests in its own region, so pointers never pile up.
                const SPOTS: [[(f32, f32); 3]; 4] = [
                    [(0.62, 0.07), (0.82, 0.19), (0.44, 0.27)],
                    [(0.72, 0.44), (0.30, 0.50), (0.56, 0.60)],
                    [(0.84, 0.80), (0.42, 0.88), (0.66, 0.93)],
                    [(0.88, 0.56), (0.22, 0.70), (0.86, 0.34)],
                ];
                let step = ((clock + agent_ix as f32 * 1.1) / 3.2) as usize;
                let (fx, fy) = SPOTS[agent_ix % SPOTS.len()][step % 3];
                stiffness = 4.5;
                (fx * PAGE_W.min(body_w), fy * (body_h - 40.0).max(0.0))
            });
            targets.push((target, stiffness, pressed));
        }

        // Integrate the springs (critically damped) towards their targets.
        let reduced = motion::reduced_motion(cx);
        let moving = state.update(cx, |s, _| {
            let now = Instant::now();
            let dt = now
                .saturating_duration_since(s.last_frame)
                .as_secs_f32()
                .min(0.05);
            s.last_frame = now;
            let mut moving = false;
            for (pointer, ((tx, ty), k, _)) in s.pointers.iter_mut().zip(&targets) {
                if !pointer.placed || reduced {
                    pointer.pos = (*tx, *ty);
                    pointer.vel = (0.0, 0.0);
                    pointer.placed = true;
                    continue;
                }
                let steps = (dt / 0.008).ceil().max(1.0) as usize;
                let h = dt / steps as f32;
                for _ in 0..steps {
                    let ax = k * k * (tx - pointer.pos.0) - 2.0 * k * pointer.vel.0;
                    let ay = k * k * (ty - pointer.pos.1) - 2.0 * k * pointer.vel.1;
                    pointer.vel.0 += ax * h;
                    pointer.vel.1 += ay * h;
                    pointer.pos.0 += pointer.vel.0 * h;
                    pointer.pos.1 += pointer.vel.1 * h;
                }
                let dist = (tx - pointer.pos.0).abs() + (ty - pointer.pos.1).abs();
                let speed = pointer.vel.0.abs() + pointer.vel.1.abs();
                if dist > 0.3 || speed > 0.5 {
                    moving = true;
                }
            }
            moving
        });
        if moving {
            window.request_animation_frame();
        }
        let placed = state.read(cx).anchors.borrow().contains_key("body");
        let pointers = state.read(cx).pointers.clone();
        let pointer_els = self
            .agents
            .iter()
            .enumerate()
            .filter(|_| placed)
            .map(|(ix, agent)| {
                let (x, y) = pointers[ix].pos;
                let pressed = targets.get(ix).is_some_and(|target| target.2);
                let tint = tints[ix];
                let edge = t.bg;
                // A press settles the arrow over Pink's micro step.
                let scale = motion::follow(
                    &format!("{key}-press-{ix}"),
                    if pressed { 0.86 } else { 1.0 },
                    motion::MICRO,
                );
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .opacity(dim_of(ix))
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |bounds, _, window, _| {
                                paint_arrow(window, bounds.origin, scale, tint.glyph, edge)
                            },
                        )
                        .w(px(14.))
                        .h(px(19.)),
                    )
                    .child(
                        // The name tag is Pink's text tooltip (Pink:
                        // popover.rs `TextTooltip`), led by a 6 px dot in the
                        // agent's tint.
                        pink::tooltip_card(&t)
                            .absolute()
                            .left(px(10.))
                            .top(px(18.))
                            .px(px(space::S8))
                            .py(px(5.))
                            .flex()
                            .items_center()
                            .gap(px(space::S6))
                            .whitespace_nowrap()
                            .type_style(TextStyle::Caption)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(div().size(px(6.)).flex_none().rounded_full().bg(tint.glyph))
                            .child(agent.name.clone()),
                    )
            })
            .collect::<Vec<_>>();

        // -- Margin cards -----------------------------------------------------
        // (wanted top, edit, height, how far its review row is open)
        let mut cards: Vec<(f32, usize, f32, f32)> = Vec::new();
        for (ix, edit) in self.edits.iter().enumerate() {
            let stage = stages[ix];
            if !stage.started() || matches!(stage, Stage::Accepted | Stage::Dismissed) {
                continue;
            }
            let anchor_key = match edit.kind {
                EditKind::Resize { .. } => "figure".to_string(),
                _ => format!("edit-{ix}"),
            };
            let s = state.read(cx);
            let Some(b) = s.anchor(&anchor_key) else {
                continue;
            };
            // The review row folds open over Pink's `DISCLOSURE`, so the
            // card grows as smoothly as the notes below it restack.
            let reveal = motion::follow(
                &format!("{key}-card-{ix}-actions-{generation}"),
                if stage == Stage::Review { 1.0 } else { 0.0 },
                motion::DISCLOSURE,
            );
            // A card's head (agent line and body) measures itself when
            // painted, before that a one-line guess; the folding review row
            // is added by arithmetic, so the stack never waits a frame on it.
            let head = s
                .anchors
                .borrow()
                .get(&format!("card-head-{ix}"))
                .map(|b| f32::from(b.size.height))
                .unwrap_or(NOTE_TILE + NOTE_GAP + 16.0);
            let height = head + NOTE_PAD * 2.0 + NOTE_ACTIONS * reveal;
            cards.push((f32::from(b.origin.y) - 2.0, ix, height, reveal));
        }
        cards.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Stack the notes: each as close to its line as it can sit, pushed
        // down past the one above, then lifted back off the column's bottom
        // edge, and finally kept below the top inset.
        let inset = pink::chrome::CONTENT_INSET;
        let mut floor = inset;
        for card in cards.iter_mut() {
            card.0 = card.0.max(floor);
            floor = card.0 + card.2 + space::S8;
        }
        let mut ceiling = body_h - inset;
        for card in cards.iter_mut().rev() {
            card.0 = card.0.min(ceiling - card.2);
            ceiling = card.0 - space::S8;
        }
        let mut floor = inset;
        for card in cards.iter_mut() {
            card.0 = card.0.max(floor);
            floor = card.0 + card.2 + space::S8;
        }
        let mut card_els = Vec::new();
        let mut to_review = 0;
        let mut shown_floor = 0.0f32;
        for (y, ix, height, reveal) in cards {
            let edit = &self.edits[ix];
            let stage = stages[ix];
            // Cards restack like a re-sorted list (Pink's `RESORT`), but a
            // card the one above grows into steps down at once: notes never
            // overlap mid-glide.
            let y =
                motion::follow(&format!("{key}-card-y-{ix}"), y, motion::RESORT).max(shown_floor);
            shown_floor = y + height + space::S8;
            let tint = tint_of(edit.agent);
            let agent = self
                .agents
                .get(edit.agent)
                .map(|a| a.name.clone())
                .unwrap_or_default();
            let card_key = format!("{key}-card-{ix}");
            let busy = stage.busy();
            if stage == Stage::Review {
                to_review += 1;
            }
            let is_comment = matches!(edit.kind, EditKind::Comment { .. });
            let actions = (reveal > 0.001).then(|| {
                let act = |accept: bool| {
                    let state = state.clone();
                    move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                        let handler = state.update(cx, |s, cx| {
                            s.review(ix, accept);
                            cx.notify();
                            s.on_review.clone()
                        });
                        if let Some(handler) = handler {
                            handler(ix, accept, window, cx);
                        }
                    }
                };
                let row = div()
                    .flex()
                    .justify_end()
                    .gap(px(space::S6))
                    .pt(px(NOTE_GAP + space::S2))
                    .when(!is_comment, |el| {
                        el.child(
                            pink::button(
                                format!("{card_key}-dismiss"),
                                ButtonKind::Ghost,
                                ControlSize::Sm,
                                &t,
                            )
                            .child("Dismiss")
                            .on_click(act(false)),
                        )
                    })
                    .child(
                        pink::button(
                            format!("{card_key}-accept"),
                            ButtonKind::Secondary,
                            ControlSize::Sm,
                            &t,
                        )
                        .child(if is_comment { "Resolve" } else { "Accept" })
                        .on_click(act(true)),
                    );
                div()
                    .flex_none()
                    .h(px(NOTE_ACTIONS * reveal))
                    .overflow_hidden()
                    .opacity(reveal)
                    .child(row)
            });
            // While the agent works, its verb shimmers like Pink's live
            // status line (Pink: transcript.rs working trailer).
            let status = busy.then(|| {
                let verb = match (&edit.kind, stage) {
                    (EditKind::Resize { .. }, _) => "Resizing",
                    (_, Stage::Select(_)) => "Selecting",
                    (EditKind::Replace { .. }, _) => "Typing",
                    (EditKind::Comment { .. }, _) => "Writing",
                };
                loaders::shimmer_text(verb, text::CAPTION, 14.0, t.content_muted, t.text)
                    .flex_none()
            });
            let content: AnyElement = match &edit.kind {
                EditKind::Replace { old, new } => div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_x(px(space::S4))
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_color(t.readable_faint(0.9))
                            .line_through()
                            .text_decoration_color(t.content_faint)
                            .child(old.clone()),
                    )
                    .child(div().text_color(t.decorative_muted(0.6)).child("→"))
                    .child(
                        div()
                            .whitespace_nowrap()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(new.clone()),
                    )
                    .into_any_element(),
                EditKind::Resize { .. } => div()
                    .text_color(t.text)
                    .child(if edit.summary.is_empty() {
                        SharedString::from("Resized the figure")
                    } else {
                        edit.summary.clone()
                    })
                    .into_any_element(),
                EditKind::Comment { note, .. } => {
                    let shown = match stage {
                        Stage::Work(p) => {
                            let n = note.chars().count();
                            note.chars()
                                .take(((p * n as f32) as usize + 1).min(n))
                                .collect::<String>()
                        }
                        Stage::Select(_) => String::new(),
                        _ => note.to_string(),
                    };
                    div().text_color(t.text).child(shown).into_any_element()
                }
            };
            let rise = motion::TRANSCRIPT_IN_RISE;
            // Notes from agents you aren't following fade back with them.
            let dim = dim_of(edit.agent).max(0.5);
            card_els.push(
                div()
                    .absolute()
                    .left(px(pink::chrome::CONTENT_INSET))
                    .right(px(pink::chrome::CONTENT_INSET))
                    .top(px(y))
                    .child(
                        // Pink: badges.rs `BadgeCard::row`, the bar in the
                        // agent's tint – strong while it works, as Pink's
                        // draft note is, quiet once it waits (a saved note).
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(space::S8))
                            .p(px(NOTE_PAD))
                            .rounded(px(NOTE_RADIUS))
                            .bg(t.ink(0.05))
                            .child(
                                div()
                                    .flex_none()
                                    .w(px(NOTE_BAR))
                                    .rounded(px(1.))
                                    .bg(tint.glyph.opacity(if busy { 0.7 } else { 0.35 })),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .relative()
                                            .flex()
                                            .flex_col()
                                            .child(anchor(&anchors, format!("card-head-{ix}")))
                                            .child(
                                                div()
                                                    .h(px(NOTE_TILE))
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(space::S6))
                                                    .child(pink::monogram_tile(
                                                        tint.raw, &agent, NOTE_TILE, &t,
                                                    ))
                                                    .child(
                                                        div()
                                                            .flex_none()
                                                            .type_style(TextStyle::Footnote)
                                                            .font_weight(FontWeight::MEDIUM)
                                                            .text_color(t.text)
                                                            .child(agent.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .min_w_0()
                                                            .truncate()
                                                            .font_family(pink::MONO)
                                                            .text_size(px(text::CAPTION))
                                                            .text_color(t.text_faint)
                                                            .child(locations[ix].clone()),
                                                    )
                                                    .children(status),
                                            )
                                            .child(
                                                div()
                                                    .mt(px(NOTE_GAP))
                                                    .type_style(TextStyle::Footnote)
                                                    .child(content),
                                            ),
                                    )
                                    .children(actions),
                            ),
                    )
                    // New notes arrive like Pink's tool rows: 360 ms expo, 4 px rise.
                    .with_animation(
                        ElementId::Name(format!("{card_key}-in-{generation}").into()),
                        motion::TOOL_ROW_REVEAL.animation(),
                        move |el, p| el.top(px(y + rise * (1.0 - p))).opacity(p * dim),
                    ),
            );
        }

        // -- Header --------------------------------------------------------------
        let agent_buttons = self.agents.iter().enumerate().map(|(ix, agent)| {
            let on = following == Some(ix);
            agent_button(
                format!("{key}-agent-{ix}"),
                agent,
                tints[ix],
                on,
                following.is_some() && !on,
                &t,
            )
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.following = if s.following == Some(ix) {
                            None
                        } else {
                            Some(ix)
                        };
                        cx.notify();
                    })
                }
            })
        });
        let follow_chip = following.and_then(|ix| self.agents.get(ix)).map(|agent| {
            pink::button(
                format!("{key}-unfollow"),
                ButtonKind::Ghost,
                ControlSize::Sm,
                &t,
            )
            .aria_label(SharedString::from(format!("Stop following {}", agent.name)))
            .child(pink::glyph(ICON_FOLLOW, ControlSize::Sm))
            .child(format!("Following {}", agent.name))
            .child(pink::glyph(ICON_CLOSE, ControlSize::Sm))
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.following = None;
                        cx.notify();
                    })
                }
            })
        });
        let accept_all = pink::button_enabled(
            format!("{key}-accept-all"),
            ButtonKind::Primary,
            ControlSize::Md,
            to_review > 0,
            &t,
        )
        .child("Accept all")
        .on_click({
            let state = state.clone();
            let reviewable: Vec<usize> = stages
                .iter()
                .enumerate()
                .filter(|(_, s)| **s == Stage::Review)
                .map(|(ix, _)| ix)
                .collect();
            move |_, window, cx| {
                if reviewable.is_empty() {
                    return;
                }
                let handler = state.update(cx, |s, cx| {
                    for ix in &reviewable {
                        s.review(*ix, true);
                    }
                    cx.notify();
                    s.on_review.clone()
                });
                if let Some(handler) = handler {
                    for ix in &reviewable {
                        handler(*ix, true, window, cx);
                    }
                }
            }
        });
        // A surface toolbar (Pink: surface_chrome.rs `toolbar`) with the
        // titlebar's crumb (Pink: shell/tabs.rs `render_title_crumb`).
        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S8))
            .h(px(layout::TITLEBAR_HEIGHT))
            .pl(px(pink::chrome::CONTENT_INSET))
            .pr(px(pink::chrome::EDGE_INSET))
            .border_b_1()
            .border_color(t.separator(false))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .text_size(px(text::BODY))
                    .child(
                        div()
                            .flex_none()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text.opacity(0.9))
                            .child(self.folder.clone()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_color(t.decorative_muted(0.4))
                            .child("›"),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text.opacity(0.85))
                            .child(self.title.clone()),
                    ),
            )
            .children(follow_chip)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(pink::chrome::CONTROL_GAP / 2.0))
                    .children(agent_buttons),
            )
            .child(
                div()
                    .flex_none()
                    .type_style(TextStyle::Footnote)
                    .font_features(pink::tabular_figures())
                    .text_color(t.readable_muted(0.7))
                    .child(match to_review {
                        0 => "Nothing to review".to_string(),
                        1 => "1 to review".to_string(),
                        n => format!("{n} to review"),
                    }),
            )
            .child(accept_all);

        // -- Body ------------------------------------------------------------------
        let page = div()
            .w(px(PAGE_W))
            .flex_none()
            .px(px(PAGE_PAD))
            .pt(px(space::S24))
            .pb(px(space::S32))
            .flex()
            .flex_col()
            .children(doc)
            .with_animation(
                ElementId::Name(format!("{key}-page-{generation}").into()),
                motion::TRANSCRIPT_IN.animation(),
                |el, p| el.opacity(if p >= 1.0 { 1.0 } else { 0.4 + 0.6 * p }),
            );
        // GPUI clips children to the card's rectangle, not its rounded
        // corner: the margin's fill rounds itself inside the 1 px edge.
        let margin = div()
            .relative()
            .flex_1()
            .rounded_br(px(radius::inner(radius::SECTION_CARD, 1.0)))
            .bg(t.surface)
            .border_l_1()
            .border_color(t.separator(false))
            .children(card_els);
        let body = div()
            .relative()
            .flex()
            .child(anchor(&anchors, "body".into()))
            .child(page)
            .child(margin)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .children(pointer_els),
            );

        if !placed {
            window.request_animation_frame();
        }
        motion::keep_alive(window);
        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .rounded(px(radius::SECTION_CARD))
            .border_1()
            .border_color(t.border)
            .bg(t.bg)
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(header)
            .child(body)
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentCursors::new("agent-cursors", "Incidents", "INC-2291 postmortem")
            .agent(CursorAgent::new("Scribe"))
            .agent(CursorAgent::new("Analyst"))
            .agent(CursorAgent::new("Fixer"))
            .agent(CursorAgent::new("Reviewer"))
            .block(DocBlock::Title("Checkout slowdown, October 14".into()))
            .block(DocBlock::Meta("Postmortem · Payments · Draft".into()))
            .block(DocBlock::Paragraph(vec![
                Run::text("From 14:05 to 14:52 card checkouts were slow for "),
                Run::Edit(0),
                Run::text(", and 3.1% of payments timed out before the gateway answered."),
            ]))
            .block(DocBlock::Figure {
                caption: "Figure 1 · p95 checkout latency, 13:30 to 15:30".into(),
                edit: Some(1),
                width: 260.0,
            })
            .block(DocBlock::Heading("What happened".into()))
            .block(DocBlock::Paragraph(vec![
                Run::text(
                    "A config push lowered the gateway timeout, so slow card checks were cut off and retried. The retries piled up into a ",
                ),
                Run::Edit(2),
                Run::text(" that filled the connection pool."),
            ]))
            .block(DocBlock::Code {
                file: "config/gateway.toml".into(),
                lines: vec![
                    vec![Run::text("[upstream.cards]")],
                    vec![Run::text("timeout_ms = "), Run::Edit(3)],
                    vec![Run::text("retries    = 3")],
                ],
            })
            .edit(DocEdit::replace(0, "about 40 minutes", "47 minutes").at(0.4))
            .edit(
                DocEdit::resize(1, 260.0, PAGE_W - PAGE_PAD * 2.0)
                    .summary("Widened Figure 1 to the full column")
                    .at(1.6),
            )
            .edit(
                DocEdit::comment(
                    3,
                    "retry storm",
                    "Can we link the panel that shows the retry spike here?",
                )
                .at(4.4),
            )
            .edit(DocEdit::replace(2, "800", "2500").at(3.0))
            .loop_after(21.0)
    })
}
