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
//!     .agent(CursorAgent::new("Scribe", 290.0))
//!     .block(DocBlock::Paragraph(vec![Run::text("Slow for "), Run::Edit(0), Run::text(".")]))
//!     .edit(DocEdit::replace(0, "about 40 minutes", "47 minutes").at(0.5))
//!     .on_review(|edit, accepted, _, _| println!("{edit}: {accepted}"))
//! ```
//!
//! Pointers find their targets by measuring the laid-out text (each target
//! records its bounds when painted) and move on a critically damped spring,
//! so they keep up with targets that shift while the page reflows.

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, App, Bounds, Div, ElementId, FontWeight, Hsla,
    IntoElement, PathBuilder, Pixels, Point, SharedString, Task, Window, canvas, div, point,
    prelude::*, px, relative, svg,
};

use crate::{
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, radius, space, text},
    motion::{self, EASE_IN_OUT, GLIDE, MotionSpec},
    primitives::ring,
    story,
};

pub const NAME: &str = "Agent Cursors";
pub const DESCRIPTION: &str = "Agents edit a shared document like teammates – named pointers glide to their spot, type suggestions letter by letter, resize a figure, fix a config and leave margin notes to accept or dismiss";

const ICON_DOC: &str = "icons/agent-cursors/doc.svg";
const ICON_CHECK: &str = "icons/agent-cursors/check.svg";
const ICON_CLOSE: &str = "icons/agent-cursors/close.svg";
const ICON_COMMENT: &str = "icons/agent-cursors/comment.svg";
const ICON_RESIZE: &str = "icons/agent-cursors/resize.svg";
const ICON_FOLLOW: &str = "icons/agent-cursors/follow.svg";

const WIDTH: f32 = 800.0;
const PAGE_W: f32 = 528.0;
const PAGE_PAD: f32 = 36.0;
const FIGURE_RATIO: f32 = 0.36;
/// Edit timeline: glide in, select, then work.
const APPROACH: f32 = 1.15;
const SELECT: f32 = 0.55;
const LINGER: f32 = 0.8;
const TYPE_EVERY: f32 = 0.085;
const CARD_GAP: f32 = 8.0;

/// One agent in the document.
#[derive(Clone, Debug)]
pub struct CursorAgent {
    pub name: SharedString,
    /// OKLCH hue in degrees for its pointer, marks and cards.
    pub hue: f32,
}

impl CursorAgent {
    pub fn new(name: impl Into<SharedString>, hue: f32) -> Self {
        Self {
            name: name.into(),
            hue,
        }
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

    fn working_or_later(self) -> bool {
        matches!(self, Stage::Work(_) | Stage::Review | Stage::Accepted)
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn agent_color(hue: f32, t: &Luna) -> Hsla {
    if t.dark {
        luna::oklch(0.72, 0.15, hue)
    } else {
        luna::oklch(0.57, 0.19, hue)
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

    fn render(self, gap: f32) -> Div {
        div().flex().flex_wrap().gap_x(px(gap)).children(
            self.groups
                .into_iter()
                .map(|group| div().flex().flex_none().children(group)),
        )
    }
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
            let mut stroke = PathBuilder::stroke(px(1.75));
            stroke.add_polygon(&points, false);
            if let Ok(path) = stroke.build() {
                window.paint_path(path, line);
            }
        },
    )
    .size_full()
}

fn blink(id: impl Into<ElementId>, color: Hsla, height: f32) -> impl IntoElement {
    div()
        .w(px(2.))
        .h(px(height))
        .flex_none()
        .rounded_full()
        .bg(color)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1000)).repeat(),
            |el, t| el.opacity(if t < 0.55 { 1.0 } else { 0.15 }),
        )
}

fn pulse(id: impl Into<ElementId>, color: Hsla) -> impl IntoElement {
    div()
        .size(px(5.))
        .flex_none()
        .rounded_full()
        .bg(color)
        .with_animation(
            id,
            Animation::new(Duration::from_millis(1200))
                .repeat()
                .with_easing(EASE_IN_OUT.easing()),
            |el, t| el.opacity(0.3 + 0.7 * (1.0 - (t * 2.0 - 1.0).abs())),
        )
}

/// A round monogram in the agent's color.
fn face(name: &str, color: Hsla, size: f32, t: &Luna) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(color)
        .text_size(px(size * 0.46))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if t.dark { t.on_solid } else { t.on_accent })
        .child(name.chars().next().unwrap_or('?').to_string())
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl RenderOnce for AgentCursors {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "cursors".into()),
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
        let colors: Vec<Hsla> = self.agents.iter().map(|a| agent_color(a.hue, &t)).collect();
        let color_of = |edit: &DocEdit| colors.get(edit.agent).copied().unwrap_or(t.accent);
        let dim_of = |agent: usize| match following {
            Some(f) if f != agent => 0.28,
            _ => 1.0,
        };

        // -- Document ----------------------------------------------------------
        let body_size = 14.0;
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
            let color = color_of(edit);
            let wash = color.opacity(if t.dark { 0.22 } else { 0.14 });
            let span_key = format!("{key}-span-{ix}-{generation}");
            let base = div()
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .whitespace_nowrap()
                .child(anchor(&anchors, format!("edit-{ix}")));
            let selection = |p: f32| {
                div()
                    .absolute()
                    .top(px(1.))
                    .bottom(px(1.))
                    .left_0()
                    .w(relative(p))
                    .rounded(px(2.))
                    .bg(wash)
            };
            match &edit.kind {
                EditKind::Replace { old, new } => {
                    let typed = match stage {
                        Stage::Work(p) => {
                            let n = new.chars().count();
                            let shown = ((p * n as f32) as usize + 1).min(n);
                            Some(new.chars().take(shown).collect::<String>())
                        }
                        Stage::Review => Some(new.to_string()),
                        _ => None,
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
                        _ => base
                            .child(
                                div()
                                    .text_color(t.text_faint)
                                    .line_through()
                                    .text_decoration_color(color.opacity(0.7))
                                    .child(old.clone()),
                            )
                            .child(div().w(px(if mono { 0.0 } else { 3.5 })))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px(px(1.))
                                    .rounded(px(2.))
                                    .bg(wash)
                                    .text_color(t.text)
                                    .border_b_1()
                                    .border_color(color)
                                    .child(typed.unwrap_or_default())
                                    .when(matches!(stage, Stage::Work(_)), |el| {
                                        el.child(blink(
                                            ElementId::Name(format!("{span_key}-caret").into()),
                                            color,
                                            if mono { 15.0 } else { 17.0 },
                                        ))
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
                                .rounded(px(2.))
                                .bg(wash)
                                .border_b_2()
                                .border_color(color)
                                .child(quoted.clone()),
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
                    .text_size(px(text::LARGE))
                    .line_height(px(28.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(anchor(&anchors, "title".into()))
                    .relative()
                    .child(title.clone())
                    .into_any_element(),
                DocBlock::Meta(meta) => div()
                    .mt(px(2.))
                    .mb(px(space::S8))
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(meta.clone())
                    .into_any_element(),
                DocBlock::Heading(heading) => div()
                    .mt(px(space::S16))
                    .mb(px(space::S4))
                    .text_size(px(15.))
                    .line_height(px(22.))
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
                    flow.render(3.8)
                        .relative()
                        .mt(px(space::S6))
                        .text_size(px(body_size))
                        .line_height(px(23.))
                        .text_color(t.text_muted)
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
                            EditKind::Resize { from, to } => Some((ix, from, to, color_of(e))),
                            _ => None,
                        })
                    });
                    let (target, active, color) = match resize {
                        Some((ix, from, to, color)) => {
                            let stage = stages[ix];
                            let target = if stage.working_or_later() { to } else { from };
                            let active = matches!(stage, Stage::Select(_) | Stage::Work(_));
                            (target, active, color)
                        }
                        None => (*width, false, t.accent),
                    };
                    let spec = MotionSpec::new(1350, EASE_IN_OUT);
                    // A figure exactly as wide as the column sends GPUI's layout
                    // into a runaway height: keep it a hair narrower.
                    let w = motion::follow(&format!("{key}-fig-w"), target, spec)
                        .min(PAGE_W - PAGE_PAD * 2.0 - 2.0);
                    let h = w * FIGURE_RATIO;
                    figure_box = Some((w, h));
                    let handle = |left: bool, top: bool| {
                        div()
                            .absolute()
                            .size(px(8.))
                            .when(left, |el| el.left(px(-4.)))
                            .when(!left, |el| el.right(px(-4.)))
                            .when(top, |el| el.top(px(-4.)))
                            .when(!top, |el| el.bottom(px(-4.)))
                            .rounded(px(2.))
                            .bg(t.surface_card)
                            .border_1()
                            .border_color(color)
                    };
                    div()
                        .mt(px(space::S12))
                        .flex()
                        .flex_col()
                        .gap(px(space::S6))
                        .child(
                            div()
                                .relative()
                                .w(px(w))
                                .h(px(h))
                                .rounded(px(radius::MD))
                                .bg(t.fill(FillTier::Quaternary))
                                .border_1()
                                .border_color(t.border)
                                .when(active, |el| el.shadow(vec![ring(1.5, color)]))
                                .child(anchor(&anchors, "figure".into()))
                                .child(chart(
                                    t.accent,
                                    t.accent.opacity(if t.dark { 0.16 } else { 0.09 }),
                                    t.warning.opacity(0.8),
                                    t.border,
                                ))
                                .when(active, |el| {
                                    el.child(handle(true, true))
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
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_faint)
                                // A bare string in a flex row can be measured at zero
                                // width and wrap per letter: keep it in its own box.
                                .child(div().flex_none().whitespace_nowrap().child(caption.clone()))
                                .when(active, |el| {
                                    el.child(
                                        div()
                                            .font_family(luna::MONO)
                                            .text_color(color)
                                            .child(format!("{:.0} px", w)),
                                    )
                                }),
                        )
                        .into_any_element()
                }
                DocBlock::Code { file, lines } => {
                    let rows = lines.iter().map(|line| {
                        let mut row = div().flex().items_center().h(px(20.)).whitespace_nowrap();
                        for run in line {
                            row = match run {
                                Run::Text(text) => row.child(text.clone()),
                                Run::Edit(ix) => row.child(span_for(*ix, true)),
                            };
                        }
                        row
                    });
                    div()
                        .relative()
                        .mt(px(space::S10))
                        .rounded(px(radius::MD))
                        .bg(t.fill(FillTier::Quaternary))
                        .border_1()
                        .border_color(t.border)
                        .child(anchor(&anchors, format!("block-{bix}")))
                        .child(
                            div()
                                .px(px(space::S12))
                                .h(px(28.))
                                .flex()
                                .items_center()
                                .border_b_1()
                                .border_color(t.border)
                                .font_family(luna::MONO)
                                .text_size(px(text::CAPTION))
                                .text_color(t.text_faint)
                                .child(file.clone()),
                        )
                        .child(
                            div()
                                .px(px(space::S12))
                                .py(px(space::S8))
                                .font_family(luna::MONO)
                                .text_size(px(12.5))
                                .text_color(t.text_muted)
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
                let color = colors[ix];
                let edge = if t.dark { t.bg } else { t.surface_card };
                let scale = motion::follow(
                    &format!("{key}-press-{ix}"),
                    if pressed { 0.86 } else { 1.0 },
                    MotionSpec::new(140, motion::EASE),
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
                                paint_arrow(window, bounds.origin, scale, color, edge)
                            },
                        )
                        .w(px(14.))
                        .h(px(19.)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(11.))
                            .top(px(17.))
                            .h(px(20.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .rounded(px(radius::SM))
                            .bg(color)
                            .shadow(t.tooltip_shadow())
                            .text_size(px(text::CAPTION))
                            .font_weight(FontWeight::SEMIBOLD)
                            .whitespace_nowrap()
                            .text_color(if t.dark { t.on_solid } else { t.on_accent })
                            .child(agent.name.clone()),
                    )
            })
            .collect::<Vec<_>>();

        // -- Margin cards -----------------------------------------------------
        let mut cards: Vec<(f32, usize, f32)> = Vec::new();
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
            let height = match edit.kind {
                EditKind::Comment { .. } => 86.0,
                _ => 68.0,
            };
            cards.push((f32::from(b.origin.y) - 10.0, ix, height));
        }
        cards.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut floor = 12.0f32;
        let mut card_els = Vec::new();
        let mut to_review = 0;
        for (want, ix, height) in cards {
            let edit = &self.edits[ix];
            let stage = stages[ix];
            let y = want.max(floor);
            floor = y + height + CARD_GAP;
            let y = motion::follow(&format!("{key}-card-y-{ix}"), y, GLIDE);
            let color = color_of(edit);
            let agent = self
                .agents
                .get(edit.agent)
                .map(|a| a.name.clone())
                .unwrap_or_default();
            let card_key = format!("{key}-card-{ix}");
            let busy = matches!(stage, Stage::Select(_) | Stage::Work(_));
            if stage == Stage::Review {
                to_review += 1;
            }
            let is_comment = matches!(edit.kind, EditKind::Comment { .. });
            let review_buttons = (stage == Stage::Review).then(|| {
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
                div()
                    .flex()
                    .gap(px(2.))
                    .child(
                        luna::icon_button(
                            format!("{card_key}-ok"),
                            ICON_CHECK,
                            ControlSize::Sm,
                            &t,
                        )
                        .on_click(act(true)),
                    )
                    .when(!is_comment, |el| {
                        el.child(
                            luna::icon_button(
                                format!("{card_key}-no"),
                                ICON_CLOSE,
                                ControlSize::Sm,
                                &t,
                            )
                            .on_click(act(false)),
                        )
                    })
            });
            let status = busy.then(|| {
                let label = match edit.kind {
                    EditKind::Replace { .. } => "typing",
                    EditKind::Resize { .. } => "resizing",
                    EditKind::Comment { .. } => "writing",
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .text_size(px(text::CAPTION))
                    .text_color(t.text_faint)
                    .child(pulse(
                        ElementId::Name(format!("{card_key}-busy").into()),
                        color,
                    ))
                    .child(label)
            });
            let content: AnyElement = match &edit.kind {
                EditKind::Replace { old, new } => div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_x(px(5.))
                    .child(div().text_color(t.text_faint).child("Replace"))
                    .child(
                        div()
                            .text_color(t.text_muted)
                            .line_through()
                            .child(old.clone()),
                    )
                    .child(div().text_color(t.text_faint).child("with"))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(color)
                            .child(new.clone()),
                    )
                    .into_any_element(),
                EditKind::Resize { .. } => div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(svg().path(ICON_RESIZE).size(px(13.)).text_color(color))
                    .child(
                        div()
                            .text_color(t.text_muted)
                            .child(if edit.summary.is_empty() {
                                SharedString::from("Resized the figure")
                            } else {
                                edit.summary.clone()
                            }),
                    )
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
                    div()
                        .text_color(t.text)
                        .line_height(px(18.))
                        .child(shown)
                        .into_any_element()
                }
            };
            card_els.push(
                div()
                    .absolute()
                    .left(px(space::S12))
                    .right(px(space::S12))
                    .top(px(y))
                    .h(px(height))
                    .opacity(dim_of(edit.agent).max(0.5))
                    .child(
                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .gap(px(space::S6))
                            .pl(px(space::S10))
                            .pr(px(space::S6))
                            .py(px(space::S8))
                            .rounded(px(radius::LG))
                            .bg(t.surface_card)
                            .border_1()
                            .border_color(if busy { color.opacity(0.55) } else { t.border })
                            .shadow(t.card_shadow())
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(17.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(space::S6))
                                    .h(px(20.))
                                    .child(face(&agent, color, 18.0, &t))
                                    .child(
                                        div()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(t.text)
                                            .child(agent.clone()),
                                    )
                                    .when(is_comment, |el| {
                                        el.child(
                                            svg()
                                                .path(ICON_COMMENT)
                                                .size(px(12.))
                                                .text_color(t.text_faint),
                                        )
                                    })
                                    .child(div().flex_1())
                                    .children(status)
                                    .children(review_buttons),
                            )
                            .child(content),
                    )
                    .with_animation(
                        ElementId::Name(format!("{card_key}-in-{generation}").into()),
                        motion::MENU_IN.animation(),
                        |el, p| el.opacity(p),
                    ),
            );
        }

        // -- Header --------------------------------------------------------------
        let faces = self.agents.iter().enumerate().map(|(ix, agent)| {
            let color = colors[ix];
            let face_key = format!("{key}-face-{ix}");
            let on = following == Some(ix);
            let lift = motion::hover_t(&face_key);
            div()
                .id(ElementId::Name(face_key.clone().into()))
                .relative()
                .ml(px(if ix == 0 { 0.0 } else { -6.0 }))
                .top(px(-2.0 * lift))
                .cursor_pointer()
                .on_hover(motion::hover_listener(face_key))
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
                .child(
                    div()
                        .rounded_full()
                        .border_2()
                        .border_color(if on { color } else { t.surface_card })
                        .child(face(&agent.name, color, 24.0, &t).opacity(dim_of(ix).max(0.5))),
                )
        });
        let follow_chip = following.and_then(|ix| self.agents.get(ix)).map(|agent| {
            luna::button(
                format!("{key}-unfollow"),
                ButtonKind::Ghost,
                ControlSize::Sm,
                &t,
            )
            .child(luna::glyph(ICON_FOLLOW, ControlSize::Sm).text_color(t.text_muted))
            .child(format!("Following {}", agent.name))
            .child(luna::glyph(ICON_CLOSE, ControlSize::Sm).text_color(t.text_faint))
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
        let accept_all = luna::button(
            format!("{key}-accept-all"),
            ButtonKind::Primary,
            ControlSize::Md,
            &t,
        )
        .when(to_review == 0, |el| el.opacity(0.4))
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
        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S10))
            .h(px(52.))
            .px(px(space::S16))
            .border_b_1()
            .border_color(t.border)
            .child(svg().path(ICON_DOC).size(px(16.)).text_color(t.text_muted))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .text_size(px(text::BODY))
                    .child(div().text_color(t.text_muted).child(self.folder.clone()))
                    .child(div().text_color(t.text_faint).child("/"))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(self.title.clone()),
                    ),
            )
            .child(div().flex_1())
            .children(follow_chip)
            .child(div().flex().items_center().pl(px(4.)).children(faces))
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
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
                motion::VIEW_IN.animation(),
                |el, p| el.opacity(if p >= 1.0 { 1.0 } else { 0.4 + 0.6 * p }),
            );
        let margin = div()
            .relative()
            .flex_1()
            .bg(t.surface)
            .border_l_1()
            .border_color(t.border)
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
        luna::card(&t)
            .w(px(WIDTH))
            .overflow_hidden()
            .font_family(luna::SANS)
            .text_color(t.text)
            .child(header)
            .child(body)
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentCursors::new("agent-cursors", "Incidents", "INC-2291 postmortem")
            .agent(CursorAgent::new("Scribe", 292.0))
            .agent(CursorAgent::new("Analyst", 222.0))
            .agent(CursorAgent::new("Fixer", 150.0))
            .agent(CursorAgent::new("Reviewer", 28.0))
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
