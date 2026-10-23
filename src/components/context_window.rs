//! Context Window – what fills a model's context, one cell per slice.
//!
//! The window is a field of cells filled column by column: system prompt,
//! tool definitions, attached files, the conversation and memory, then free
//! space, with the reply reserve as dashed cells at the very end and a
//! dashed marker where auto-compaction kicks in. Hovering a cell or a legend
//! row lights up that part and dims the rest; files can be dropped from the
//! context, and compacting the conversation ripples its cells while it is
//! summarized, then sweeps the freed cells clear.
//!
//! ```ignore
//! ContextWindow::new("ctx", "lumen-4", 200_000)
//!     .system(3_100)
//!     .tools(14_600)
//!     .file("rules", "billing_rules.md", 18_400)
//!     .conversation(96_300, 46)
//!     .on_compact(|_, _| {})
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyView, App, ElementId, Entity, FontWeight, Hsla, SharedString, Task,
    Transformation, Window, div, percentage, prelude::*, px, svg,
};

use crate::{
    motion::{self, EASE_OUT_QUINT, GLIDE, MotionSpec},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, Tone, oklch, radius, text},
    story,
};

pub const NAME: &str = "Context Window";
pub const DESCRIPTION: &str = "A model's context as a field of cells, one per slice of tokens – hover a part to light up its cells, detach files, and summarize the chat history while its cells ripple and clear";

const ICON_REMOVE: &str = "icons/context-window/remove.svg";
const ICON_COMPACT: &str = "icons/context-window/compact.svg";
const ICON_SPINNER: &str = "icons/context-window/spinner.svg";
const ICON_FILE: &str = "icons/context-window/file.svg";

const COLS: usize = 20;
const ROWS: usize = 10;
const CELL: f32 = 13.;
const PITCH: f32 = 16.;
const FIELD_W: f32 = COLS as f32 * PITCH - (PITCH - CELL);
const FIELD_H: f32 = ROWS as f32 * PITCH - (PITCH - CELL);
const LEGEND_W: f32 = 300.;
const ROW_H: f32 = 30.;
const FILE_H: f32 = 28.;
/// Share of the window at which the conversation compacts on its own.
const AUTO_COMPACT: f32 = 0.9;
const COMPACT_TIME: Duration = Duration::from_millis(1700);

type IdHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// A part of the context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    System,
    Tools,
    Files,
    Conversation,
    Memory,
}

impl Part {
    fn label(self) -> &'static str {
        match self {
            Part::System => "Instructions",
            Part::Tools => "Tool definitions",
            Part::Files => "Attached files",
            Part::Conversation => "Chat history",
            Part::Memory => "Saved memory",
        }
    }

    fn color(self, t: &Pink) -> Hsla {
        let l = if t.dark { 0.70 } else { 0.64 };
        match self {
            Part::System => pink::neutral(if t.dark { 0.6 } else { 0.66 }),
            Part::Tools => oklch(l, 0.10, 232.),
            Part::Files => oklch(l, 0.11, 185.),
            Part::Conversation => t.accent,
            Part::Memory => oklch(l, 0.13, 335.),
        }
    }
}

/// A file attached to the context.
#[derive(Clone, Debug)]
pub struct ContextFile {
    pub id: SharedString,
    pub name: SharedString,
    pub tokens: u32,
}

/// What one cell holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Free,
    Reserved,
    /// A part, and for files the file's index.
    Part(Part, Option<usize>),
}

/// What the pointer is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spot {
    Part(Part),
    File(usize),
    Free,
    Reserved,
}

impl Spot {
    fn of(cell: Cell) -> Spot {
        match cell {
            Cell::Free => Spot::Free,
            Cell::Reserved => Spot::Reserved,
            Cell::Part(Part::Files, Some(i)) => Spot::File(i),
            Cell::Part(p, _) => Spot::Part(p),
        }
    }

    fn lights(self, cell: Cell) -> bool {
        match (self, cell) {
            (Spot::Part(p), Cell::Part(q, _)) => p == q,
            (Spot::File(i), Cell::Part(Part::Files, Some(j))) => i == j,
            (Spot::Free, Cell::Free) | (Spot::Reserved, Cell::Reserved) => true,
            _ => false,
        }
    }
}

/// Where a hover came from, so leaving one cell doesn't clear the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HoverSource {
    Cell(usize),
    Row(Spot),
}

#[derive(Clone, Debug)]
struct FileEntry {
    file: ContextFile,
    removed: Option<Instant>,
}

#[derive(Clone, Debug)]
struct Usage {
    capacity: u32,
    reserve: u32,
    system: u32,
    tools: u32,
    files: Vec<FileEntry>,
    conversation: u32,
    turns: u32,
    summarized: bool,
    memory: u32,
}

impl Usage {
    fn live_files(&self) -> impl Iterator<Item = (usize, &FileEntry)> {
        self.files
            .iter()
            .enumerate()
            .filter(|(_, f)| f.removed.is_none())
    }

    fn files_total(&self) -> u32 {
        self.live_files().map(|(_, f)| f.file.tokens).sum()
    }

    fn used(&self) -> u32 {
        self.system + self.tools + self.files_total() + self.conversation + self.memory
    }

    fn tokens(&self, spot: Spot) -> u32 {
        match spot {
            Spot::Part(Part::System) => self.system,
            Spot::Part(Part::Tools) => self.tools,
            Spot::Part(Part::Files) => self.files_total(),
            Spot::Part(Part::Conversation) => self.conversation,
            Spot::Part(Part::Memory) => self.memory,
            Spot::File(i) => self.files.get(i).map_or(0, |f| f.file.tokens),
            Spot::Free => self
                .capacity
                .saturating_sub(self.used())
                .saturating_sub(self.reserve),
            Spot::Reserved => self.reserve,
        }
    }

    /// The field, filled column by column.
    fn cells(&self) -> Vec<Cell> {
        let total = COLS * ROWS;
        let per = (self.capacity as f32 / total as f32).max(1.);
        let count = |tokens: u32| {
            if tokens == 0 {
                0
            } else {
                ((tokens as f32 / per).round() as usize).max(1)
            }
        };
        let reserved = count(self.reserve).min(total);
        let room = total - reserved;
        let mut cells = Vec::with_capacity(total);
        let push = |cell: Cell, n: usize, cells: &mut Vec<Cell>| {
            for _ in 0..n {
                if cells.len() < room {
                    cells.push(cell);
                }
            }
        };
        push(
            Cell::Part(Part::System, None),
            count(self.system),
            &mut cells,
        );
        push(Cell::Part(Part::Tools, None), count(self.tools), &mut cells);
        for (i, f) in self.live_files() {
            push(
                Cell::Part(Part::Files, Some(i)),
                count(f.file.tokens),
                &mut cells,
            );
        }
        push(
            Cell::Part(Part::Conversation, None),
            count(self.conversation),
            &mut cells,
        );
        push(
            Cell::Part(Part::Memory, None),
            count(self.memory),
            &mut cells,
        );
        cells.resize(room, Cell::Free);
        cells.resize(total, Cell::Reserved);
        cells
    }
}

fn k(tokens: u32) -> String {
    if tokens < 1000 {
        return tokens.to_string();
    }
    let s = format!("{:.1}", tokens as f32 / 1000.);
    format!("{}K", s.strip_suffix(".0").unwrap_or(&s))
}

struct WindowState {
    usage: Usage,
    hover: Option<(Spot, HoverSource)>,
    compacting: Option<Instant>,
    /// The field before the last change, for the sweep.
    prev: Vec<Cell>,
    changed: Option<Instant>,
    _task: Option<Task<()>>,
}

impl WindowState {
    fn hover(&mut self, spot: Spot, source: HoverSource, hovered: bool) {
        if hovered {
            self.hover = Some((spot, source));
        } else if self.hover.is_some_and(|(_, s)| s == source) {
            self.hover = None;
        }
    }

    fn remove_file(&mut self, ix: usize) -> Option<SharedString> {
        let entry = self.usage.files.get(ix)?;
        if entry.removed.is_some() {
            return None;
        }
        self.prev = self.usage.cells();
        self.usage.files[ix].removed = Some(Instant::now());
        self.changed = Some(Instant::now());
        if matches!(self.hover, Some((Spot::File(i), _)) if i == ix) {
            self.hover = None;
        }
        Some(self.usage.files[ix].file.id.clone())
    }

    fn finish_compaction(&mut self) {
        self.prev = self.usage.cells();
        let summary = ((self.usage.conversation as f32 * 0.08) as u32).max(1_800);
        self.usage.conversation = summary.min(self.usage.conversation);
        self.usage.summarized = true;
        self.compacting = None;
        self.changed = Some(Instant::now());
    }
}

/// A model's context window, cell by cell.
#[derive(IntoElement)]
pub struct ContextWindow {
    id: ElementId,
    model: SharedString,
    usage: Usage,
    on_remove_file: Option<IdHandler>,
    on_compact: Option<Handler>,
}

impl ContextWindow {
    pub fn new(id: impl Into<ElementId>, model: impl Into<SharedString>, capacity: u32) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            usage: Usage {
                capacity: capacity.max(1),
                reserve: 0,
                system: 0,
                tools: 0,
                files: Vec::new(),
                conversation: 0,
                turns: 0,
                summarized: false,
                memory: 0,
            },
            on_remove_file: None,
            on_compact: None,
        }
    }

    pub fn system(mut self, tokens: u32) -> Self {
        self.usage.system = tokens;
        self
    }

    pub fn tools(mut self, tokens: u32) -> Self {
        self.usage.tools = tokens;
        self
    }

    pub fn file(
        mut self,
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        tokens: u32,
    ) -> Self {
        self.usage.files.push(FileEntry {
            file: ContextFile {
                id: id.into(),
                name: name.into(),
                tokens,
            },
            removed: None,
        });
        self
    }

    /// The conversation so far and how many turns it holds.
    pub fn conversation(mut self, tokens: u32, turns: u32) -> Self {
        self.usage.conversation = tokens;
        self.usage.turns = turns;
        self
    }

    pub fn memory(mut self, tokens: u32) -> Self {
        self.usage.memory = tokens;
        self
    }

    /// Tokens kept free for the model's reply.
    pub fn reserve(mut self, tokens: u32) -> Self {
        self.usage.reserve = tokens;
        self
    }

    /// A file was dropped from the context (it leaves the field right away).
    pub fn on_remove_file(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_remove_file = Some(Rc::new(f));
        self
    }

    /// Compaction started.
    pub fn on_compact(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_compact = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ContextWindow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial = self.usage.clone();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| WindowState {
                prev: initial.cells(),
                usage: initial,
                hover: None,
                compacting: None,
                changed: None,
                _task: None,
            },
        );
        let s = state.read(cx);
        let usage = s.usage.clone();
        let hover = s.hover.map(|(spot, _)| spot);
        let compacting = s.compacting;
        let used = usage.used();
        let shown_used = motion::follow(&format!("{key}-used"), used as f32, GLIDE);
        let share = used as f32 / usage.capacity as f32;
        let tone = if share >= AUTO_COMPACT {
            Tone::Danger
        } else if share >= 0.75 {
            Tone::Warning
        } else {
            Tone::Neutral
        };

        let header = div()
            .px(px(18.))
            .pt(px(16.))
            .pb(px(14.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(text::CALLOUT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Context"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .h(px(22.))
                            .pl(px(4.))
                            .pr(px(8.))
                            .rounded(px(radius::SM))
                            .border_1()
                            .border_color(t.border)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_muted)
                            .child(
                                div()
                                    .size(px(14.))
                                    .rounded(px(radius::XS))
                                    .bg(t.accent_wash)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(8.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.accent)
                                    .child(
                                        self.model
                                            .chars()
                                            .next()
                                            .map(|c| c.to_ascii_uppercase().to_string())
                                            .unwrap_or_default(),
                                    ),
                            )
                            .child(format!("{} · {} window", self.model, k(usage.capacity))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(28.))
                            .line_height(px(32.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(k(shown_used.round() as u32)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(text::BODY))
                            .text_color(t.text_faint)
                            .child(format!("of {} tokens", k(usage.capacity))),
                    )
                    .child(pink::badge(
                        format!("{:.0}%", (shown_used / usage.capacity as f32) * 100.),
                        tone,
                        &t,
                    )),
            );

        let field = field_view(&key, &usage, &state, hover, compacting, window, &t, cx);
        let legend = legend_view(&key, &usage, &state, hover, self.on_remove_file.clone(), &t);

        let free = usage.tokens(Spot::Free);
        let compact_state = state.clone();
        let on_compact = self.on_compact.clone();
        let can_compact = compacting.is_none() && !usage.summarized && usage.conversation > 4_000;
        let footer = div()
            .h(px(52.))
            .px(px(18.))
            .border_t_1()
            .border_color(t.border)
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .when_some(compacting, |d, _| {
                        let spinner = svg().path(ICON_SPINNER).size(px(14.)).text_color(t.accent);
                        d.child(spinner.with_animation(
                            eid(format!("{key}-spin")),
                            Animation::new(Duration::from_millis(800)).repeat(),
                            |s, p| s.with_transformation(Transformation::rotate(percentage(p))),
                        ))
                        .child(format!("Summarizing {} turns…", usage.turns))
                    })
                    .when(compacting.is_none(), |d| {
                        d.child(
                            div()
                                .font_family(pink::MONO)
                                .text_color(t.text)
                                .child(k(free)),
                        )
                        .child(format!(
                            "free · summarizes itself at {:.0}%",
                            AUTO_COMPACT * 100.
                        ))
                    }),
            )
            .child(
                pink::button(
                    format!("{key}-compact"),
                    ButtonKind::Secondary,
                    ControlSize::Md,
                    &t,
                )
                .when(!can_compact, |b| b.opacity(0.5).cursor_default())
                .child(pink::glyph(ICON_COMPACT, ControlSize::Md).text_color(t.text_muted))
                .child(if usage.summarized {
                    "Summarized"
                } else {
                    "Summarize history"
                })
                .when(can_compact, |b| {
                    b.on_click(move |_, window, cx| {
                        compact_state.update(cx, |s, cx| {
                            if s.compacting.is_some() || s.usage.summarized {
                                return;
                            }
                            s.compacting = Some(Instant::now());
                            s.hover = None;
                            s._task = Some(cx.spawn(async move |this, cx| {
                                cx.background_executor()
                                    .timer(COMPACT_TIME.mul_f32(motion::speed_scale()))
                                    .await;
                                this.update(cx, |s, cx| {
                                    s.finish_compaction();
                                    cx.notify();
                                })
                                .ok();
                            }));
                            cx.notify();
                        });
                        if let Some(f) = on_compact.as_ref() {
                            f(window, cx);
                        }
                    })
                }),
            );

        let el = pink::card(&t)
            .id(self.id.clone())
            .font_family(pink::SANS)
            .text_size(px(text::BODY))
            .text_color(t.text)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .px(px(18.))
                    .pb(px(16.))
                    .flex()
                    .gap(px(26.))
                    .child(field)
                    .child(legend),
            )
            .child(footer);
        motion::keep_alive(window);
        el
    }
}

#[allow(clippy::too_many_arguments)]
fn field_view(
    key: &str,
    usage: &Usage,
    state: &Entity<WindowState>,
    hover: Option<Spot>,
    compacting: Option<Instant>,
    window: &mut Window,
    t: &Pink,
    cx: &App,
) -> impl IntoElement {
    let cells = usage.cells();
    let s = state.read(cx);
    let (prev, changed) = (s.prev.clone(), s.changed);
    let reduced = motion::reduced_motion(cx);
    let free = t.fill(FillTier::Secondary);
    let color_of = |cell: Cell| match cell {
        Cell::Free | Cell::Reserved => free,
        Cell::Part(p, _) => p.color(t),
    };

    // The sweep: changed cells re-color one after another in fill order.
    let elapsed = changed.map(|c| c.elapsed().as_secs_f32() / motion::speed_scale());
    let mut rank = 0usize;
    let mut animating = false;
    let compact_t = compacting.map(|c| c.elapsed().as_secs_f32() / motion::speed_scale());
    if compact_t.is_some() && !reduced {
        animating = true;
    }

    let highlight = gpui::white();
    let children: Vec<_> = cells
        .iter()
        .enumerate()
        .map(|(i, &cell)| {
            let (col, row) = (i / ROWS, i % ROWS);
            let mut color = color_of(cell);
            let before = prev.get(i).copied().unwrap_or(cell);
            if before != cell
                && let Some(e) = elapsed
            {
                let delay = rank as f32 * 0.006;
                rank += 1;
                let p = ((e - delay) / 0.32).clamp(0., 1.);
                if p < 1. && !reduced {
                    animating = true;
                    color = motion::mix(color_of(before), color, EASE_OUT_QUINT.eval(p));
                }
            }
            if let (Some(ct), Cell::Part(Part::Conversation, _)) = (compact_t, cell)
                && !reduced
            {
                let phase = ct * 7.0 - (col as f32 * 0.55 + row as f32 * 0.25);
                let wave = (phase.sin() * 0.5 + 0.5).powf(2.);
                color = motion::mix(color, highlight, wave * if t.dark { 0.45 } else { 0.55 });
            }
            let lit = hover.is_none_or(|spot| spot.lights(cell));
            let opacity = motion::follow(
                &format!("{key}-c{i}"),
                if lit { 1. } else { 0.22 },
                MotionSpec::new(180, motion::EASE),
            );
            let spot = Spot::of(cell);
            let hover_state = state.clone();
            div()
                .id(("ctx-cell", i))
                .absolute()
                .left(px(col as f32 * PITCH))
                .top(px(row as f32 * PITCH))
                .size(px(CELL))
                .rounded(px(3.))
                .opacity(opacity)
                .map(|d| {
                    if cell == Cell::Reserved {
                        d.border_1().border_dashed().border_color(t.border_strong)
                    } else {
                        d.bg(color)
                    }
                })
                .on_hover(move |hovered, _, cx| {
                    hover_state.update(cx, |s, cx| {
                        s.hover(spot, HoverSource::Cell(i), *hovered);
                        cx.notify();
                    })
                })
        })
        .collect();
    if animating {
        window.request_animation_frame();
    }

    // The auto-compaction marker sits on a column boundary.
    let marker_cell = ((COLS * ROWS) as f32 * AUTO_COMPACT).round() as usize;
    let marker_x = (marker_cell / ROWS) as f32 * PITCH - (PITCH - CELL) / 2. - 0.5;
    let marker = div()
        .absolute()
        .left(px(marker_x))
        .top(px(-6.))
        .w(px(1.))
        .h(px(FIELD_H + 12.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .children((0..((FIELD_H + 12.) / 5.) as usize).map(|_| {
            div()
                .flex_none()
                .w(px(1.))
                .h(px(3.))
                .bg(t.warning.opacity(0.8))
        }));
    let marker_label = div()
        .absolute()
        .right(px(FIELD_W - marker_x + 4.))
        .top(px(-20.))
        .text_size(px(10.5))
        .text_color(t.text_faint)
        .whitespace_nowrap()
        .child(format!("auto-summary {:.0}%", AUTO_COMPACT * 100.));

    let readout = match hover {
        Some(spot) => {
            let tokens = usage.tokens(spot);
            let (label, swatch): (SharedString, Option<Hsla>) = match spot {
                Spot::Part(p) => (p.label().into(), Some(p.color(t))),
                Spot::File(i) => (
                    usage
                        .files
                        .get(i)
                        .map(|f| f.file.name.clone())
                        .unwrap_or_default(),
                    Some(Part::Files.color(t)),
                ),
                Spot::Free => ("Free".into(), Some(free)),
                Spot::Reserved => ("Reserved for the reply".into(), None),
            };
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(text::FOOTNOTE))
                .child(div().size(px(9.)).rounded(px(2.5)).map(|d| match swatch {
                    Some(c) => d.bg(c),
                    None => d.border_1().border_dashed().border_color(t.border_strong),
                }))
                .child(div().text_color(t.text).child(label))
                .child(
                    div()
                        .font_family(pink::MONO)
                        .text_color(t.text_muted)
                        .child(k(tokens)),
                )
                .child(div().text_color(t.text_faint).child(format!(
                    "{:.0}% of the window",
                    tokens as f32 / usage.capacity as f32 * 100.
                )))
        }
        None => div()
            .text_size(px(text::FOOTNOTE))
            .text_color(t.text_faint)
            .child(format!(
                "Each cell ≈ {} tokens · hover to inspect",
                k(usage.capacity / (COLS * ROWS) as u32)
            )),
    };

    div()
        .flex_none()
        .w(px(FIELD_W))
        .pt(px(18.))
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .relative()
                .w(px(FIELD_W))
                .h(px(FIELD_H))
                .children(children)
                .child(marker)
                .child(marker_label),
        )
        .child(div().h(px(18.)).child(readout))
}

fn legend_view(
    key: &str,
    usage: &Usage,
    state: &Entity<WindowState>,
    hover: Option<Spot>,
    on_remove: Option<IdHandler>,
    t: &Pink,
) -> impl IntoElement {
    let row = |spot: Spot, swatch: Option<Hsla>, label: SharedString, meta: Option<String>| {
        let row_key = format!("{key}-row-{spot:?}");
        let tokens = usage.tokens(spot);
        let shown = motion::follow(&format!("{row_key}-v"), tokens as f32, GLIDE);
        let hover_state = state.clone();
        let active = hover == Some(spot);
        div()
            .id(eid(format!("legend-{spot:?}")))
            .h(px(ROW_H))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .rounded(px(radius::MD))
            .bg(if active {
                t.fill(FillTier::Tertiary)
            } else {
                motion::hover_blend(
                    &row_key,
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                )
            })
            .on_hover({
                let fade = motion::hover_listener(row_key.clone());
                move |hovered, window, cx| {
                    fade(hovered, window, cx);
                    hover_state.update(cx, |s, cx| {
                        s.hover(spot, HoverSource::Row(spot), *hovered);
                        cx.notify();
                    })
                }
            })
            .child(
                div()
                    .flex_none()
                    .size(px(10.))
                    .rounded(px(3.))
                    .map(|d| match swatch {
                        Some(c) => d.bg(c),
                        None => d.border_1().border_dashed().border_color(t.border_strong),
                    }),
            )
            .child(div().flex_none().child(label))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(meta.unwrap_or_default()),
            )
            .child(
                div()
                    .flex_none()
                    .font_family(pink::MONO)
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .child(k(shown.round() as u32)),
            )
    };

    let live = usage.live_files().count();
    let files = usage.files.iter().enumerate().filter_map(|(i, entry)| {
        let file_key = format!("{key}-file-{}", entry.file.id);
        let height = match entry.removed {
            Some(at) if at.elapsed() > GLIDE.total().mul_f32(motion::speed_scale() * 1.5) => {
                return None;
            }
            Some(_) => motion::follow_from(&format!("{file_key}-h"), FILE_H, 0., GLIDE),
            None => FILE_H,
        };
        let spot = Spot::File(i);
        let hover_state = state.clone();
        let remove_state = state.clone();
        let on_remove = on_remove.clone();
        let active = hover == Some(spot);
        Some(
            div()
                .id(eid(format!("file-{}", entry.file.id)))
                .h(px(height))
                .opacity(height / FILE_H)
                .overflow_hidden()
                .flex_none()
                .pl(px(28.))
                .pr(px(2.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(px(radius::MD))
                .bg(if active {
                    t.fill(FillTier::Tertiary)
                } else {
                    motion::hover_blend(
                        &file_key,
                        t.fill(FillTier::Tertiary).opacity(0.),
                        t.fill(FillTier::Tertiary),
                    )
                })
                .when(entry.removed.is_none(), |d| {
                    let fade = motion::hover_listener(file_key.clone());
                    d.on_hover(move |hovered, window, cx| {
                        fade(hovered, window, cx);
                        hover_state.update(cx, |s, cx| {
                            s.hover(spot, HoverSource::Row(spot), *hovered);
                            cx.notify();
                        })
                    })
                })
                .child(
                    svg()
                        .path(ICON_FILE)
                        .size(px(13.))
                        .flex_none()
                        .text_color(t.text_faint),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(pink::MONO)
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child(entry.file.name.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .font_family(pink::MONO)
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_faint)
                        .child(k(entry.file.tokens)),
                )
                .child(
                    pink::icon_button(format!("{file_key}-x"), ICON_REMOVE, ControlSize::Sm, t)
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            let removed = remove_state.update(cx, |s, cx| {
                                let id = s.remove_file(i);
                                cx.notify();
                                id
                            });
                            if let (Some(id), Some(f)) = (removed, on_remove.as_ref()) {
                                f(&id, window, cx);
                            }
                        }),
                ),
        )
    });

    div()
        .flex_none()
        .w(px(LEGEND_W))
        .flex()
        .flex_col()
        .child(row(
            Spot::Part(Part::System),
            Some(Part::System.color(t)),
            Part::System.label().into(),
            None,
        ))
        .child(row(
            Spot::Part(Part::Tools),
            Some(Part::Tools.color(t)),
            Part::Tools.label().into(),
            None,
        ))
        .child(row(
            Spot::Part(Part::Files),
            Some(Part::Files.color(t)),
            Part::Files.label().into(),
            (live == 0).then(|| "none".to_string()),
        ))
        .children(files)
        .child(row(
            Spot::Part(Part::Conversation),
            Some(Part::Conversation.color(t)),
            Part::Conversation.label().into(),
            Some(if usage.summarized {
                format!("summary of {} turns", usage.turns)
            } else {
                format!("{} turns", usage.turns)
            }),
        ))
        .child(row(
            Spot::Part(Part::Memory),
            Some(Part::Memory.color(t)),
            Part::Memory.label().into(),
            None,
        ))
        .child(row(Spot::Reserved, None, "Reply reserve".into(), None))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        ContextWindow::new("context-window", "lumen-4", 200_000)
            .system(3_100)
            .tools(14_600)
            .file("rules", "billing_rules.md", 18_400)
            .file("schema", "invoice_schema.sql", 9_600)
            .file("tax", "q3_tax_changes.pdf", 13_200)
            .conversation(96_300, 46)
            .memory(2_400)
            .reserve(8_000)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage() -> Usage {
        ContextWindow::new("ctx", "model", 200_000)
            .system(3_000)
            .file("a", "a.md", 10_000)
            .file("b", "b.md", 4_000)
            .conversation(20_000, 4)
            .reserve(8_000)
            .usage
    }

    #[test]
    fn cells_fill_in_order_with_the_reserve_last() {
        let cells = usage().cells();
        assert_eq!(cells.len(), COLS * ROWS);
        assert_eq!(cells[0], Cell::Part(Part::System, None));
        assert_eq!(cells[3], Cell::Part(Part::Files, Some(0)));
        assert_eq!(cells[13], Cell::Part(Part::Files, Some(1)));
        assert_eq!(cells[17], Cell::Part(Part::Conversation, None));
        assert_eq!(cells[37], Cell::Free);
        assert!(cells[192..].iter().all(|c| *c == Cell::Reserved));
    }

    #[test]
    fn removing_a_file_frees_its_cells() {
        let mut u = usage();
        u.files[0].removed = Some(Instant::now());
        assert_eq!(u.files_total(), 4_000);
        assert_eq!(u.cells()[3], Cell::Part(Part::Files, Some(1)));
        assert_eq!(u.tokens(Spot::Free), 200_000 - 27_000 - 8_000);
    }

    #[test]
    fn token_labels() {
        assert_eq!(k(950), "950");
        assert_eq!(k(200_000), "200K");
        assert_eq!(k(18_400), "18.4K");
    }
}
