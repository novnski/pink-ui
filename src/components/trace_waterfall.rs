//! Trace Waterfall – where an agent run spends its time and money.
//!
//! Every step of the run (sub-agents, model calls and tool calls) is a row
//! with a bar on a shared timeline. While the run is live the bars grow, a
//! now-line sweeps right and the time axis rescales as the run gets longer.
//! Each bar is labelled with its duration and cost; the slowest finished
//! step is flagged and failures turn red. Sub-agents fold their children
//! away. Hover or select a step to read its timing, tokens and cost in the
//! strip at the bottom; arrows move the selection, ← / → fold and unfold.
//!
//! ```ignore
//! TraceWaterfall::new("run", "Nightly eval")
//!     .span(TraceSpan::agent("Evaluator", 0.0, 9.5))
//!     .span(TraceSpan::llm("Grade answers", 0.4, 6.1).parent(0).tokens(8_200, 640).cost(0.031))
//!     .playback(1.0)
//! ```

use std::{collections::HashSet, rc::Rc, time::Instant};

use gpui::{
    AnyElement, AnyView, App, ElementId, Entity, FocusHandle, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseButton, SharedString, Transformation, Window, div, font, prelude::*, px,
    radians, svg,
};

use crate::{
    HoverFill,
    luna::{self, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, AnimationExt, GLIDE, HOVER_FADE, MENU_IN, ROW_IN},
    primitives::ring,
    story,
};

pub const NAME: &str = "Trace Waterfall";
pub const DESCRIPTION: &str = "An agent run laid out on a live timeline – every sub-agent, model call and tool call as a bar with its duration and cost, the slowest step flagged, branches that fold and a readout for the step under the pointer";

const ICON_LLM: &str = "icons/trace-waterfall/llm.svg";
const ICON_TOOL: &str = "icons/trace-waterfall/tool.svg";
const ICON_AGENT: &str = "icons/trace-waterfall/agent.svg";
const ICON_CHEVRON: &str = "icons/trace-waterfall/chevron.svg";
const ICON_REPLAY: &str = "icons/trace-waterfall/replay.svg";
const ICON_TRACE: &str = "icons/trace-waterfall/trace.svg";

const ROW_H: f32 = 26.0;
const TREE_W: f32 = 268.0;
const AXIS_H: f32 = 28.0;
/// Inset of the time axis inside the timeline column.
const PAD_L: f32 = 12.0;
/// Room after the axis for the last bar's label.
const PAD_R: f32 = 96.0;
const LABEL_SIZE: f32 = 11.0;

/// What a step is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind {
    /// A (sub-)agent: its bar brackets the steps under it.
    Agent,
    /// A model call.
    Llm,
    /// A tool call.
    Tool,
}

impl SpanKind {
    fn icon(self) -> &'static str {
        match self {
            SpanKind::Agent => ICON_AGENT,
            SpanKind::Llm => ICON_LLM,
            SpanKind::Tool => ICON_TOOL,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SpanKind::Agent => "Sub-agent",
            SpanKind::Llm => "Model call",
            SpanKind::Tool => "Tool call",
        }
    }
}

/// One step of the run. Times are seconds from the start of the run.
#[derive(Clone, Debug)]
pub struct TraceSpan {
    pub kind: SpanKind,
    pub name: SharedString,
    /// The model or tool, shown beside the name.
    pub detail: Option<SharedString>,
    pub parent: Option<usize>,
    pub start: f32,
    pub end: f32,
    pub tokens_in: u32,
    pub tokens_out: u32,
    /// Cost in dollars.
    pub cost: f32,
    pub failed: bool,
}

impl TraceSpan {
    pub fn new(kind: SpanKind, name: impl Into<SharedString>, start: f32, end: f32) -> Self {
        Self {
            kind,
            name: name.into(),
            detail: None,
            parent: None,
            start,
            end: end.max(start),
            tokens_in: 0,
            tokens_out: 0,
            cost: 0.,
            failed: false,
        }
    }

    pub fn agent(name: impl Into<SharedString>, start: f32, end: f32) -> Self {
        Self::new(SpanKind::Agent, name, start, end)
    }

    pub fn llm(name: impl Into<SharedString>, start: f32, end: f32) -> Self {
        Self::new(SpanKind::Llm, name, start, end)
    }

    pub fn tool(name: impl Into<SharedString>, start: f32, end: f32) -> Self {
        Self::new(SpanKind::Tool, name, start, end)
    }

    /// Index of the enclosing span (spans are indexed in the order added).
    pub fn parent(mut self, parent: usize) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn tokens(mut self, input: u32, output: u32) -> Self {
        self.tokens_in = input;
        self.tokens_out = output;
        self
    }

    pub fn cost(mut self, dollars: f32) -> Self {
        self.cost = dollars;
        self
    }

    pub fn failed(mut self) -> Self {
        self.failed = true;
        self
    }

    fn duration(&self) -> f32 {
        self.end - self.start
    }

    /// Share of the span done at `now` (0 before it starts, 1 once it ends).
    fn progress(&self, now: f32) -> f32 {
        if self.duration() <= 0. {
            return if now >= self.start { 1. } else { 0. };
        }
        ((now - self.start) / self.duration()).clamp(0., 1.)
    }
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct TraceWaterfall {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    spans: Vec<TraceSpan>,
    playback: Option<f32>,
    width: f32,
    on_select: Option<SelectHandler>,
}

impl TraceWaterfall {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            spans: Vec::new(),
            playback: None,
            width: 800.,
            on_select: None,
        }
    }

    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Adds a step. A parent must be added before its children.
    pub fn span(mut self, span: TraceSpan) -> Self {
        self.spans.push(span);
        self
    }

    pub fn spans(mut self, spans: impl IntoIterator<Item = TraceSpan>) -> Self {
        self.spans.extend(spans);
        self
    }

    /// Plays the run back live at `speed`× real time (a live view of a
    /// recorded trace); leave unset to show the finished run.
    pub fn playback(mut self, speed: f32) -> Self {
        self.playback = Some(speed.max(0.01));
        self
    }

    /// Card width in px (default 800).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Called with the span index when a step is selected.
    pub fn on_select(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

struct TraceState {
    focus: FocusHandle,
    started: Instant,
    selected: Option<usize>,
    hovered: Option<usize>,
    collapsed: HashSet<usize>,
}

/// Spans in display order (depth first, siblings by start) with their depth.
fn display_order(spans: &[TraceSpan]) -> Vec<(usize, usize)> {
    fn visit(
        spans: &[TraceSpan],
        parent: Option<usize>,
        depth: usize,
        out: &mut Vec<(usize, usize)>,
    ) {
        let mut kids: Vec<usize> = (0..spans.len())
            .filter(|ix| spans[*ix].parent == parent && Some(*ix) != parent)
            .collect();
        kids.sort_by(|a, b| spans[*a].start.total_cmp(&spans[*b].start));
        for ix in kids {
            out.push((ix, depth));
            if depth < 16 {
                visit(spans, Some(ix), depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    visit(spans, None, 0, &mut out);
    out
}

fn descendants(spans: &[TraceSpan], ix: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![ix];
    while let Some(p) = stack.pop() {
        for (c, s) in spans.iter().enumerate() {
            if s.parent == Some(p) && c != p && !out.contains(&c) {
                out.push(c);
                stack.push(c);
            }
        }
    }
    out
}

/// A tick step that gives at most six ticks over `scale` seconds.
fn tick_step(scale: f32) -> f32 {
    [1., 2., 5., 10., 15., 20., 30., 60., 120., 300.]
        .into_iter()
        .find(|step| scale / step <= 6.)
        .unwrap_or(600.)
}

fn fmt_secs(s: f32) -> String {
    if s < 10. {
        format!("{s:.1}s")
    } else if s < 60. {
        format!("{s:.0}s")
    } else {
        format!("{}m{:02}s", (s / 60.) as u32, (s % 60.) as u32)
    }
}

fn fmt_cost(c: f32) -> String {
    if c >= 1. {
        format!("${c:.2}")
    } else {
        format!("${c:.3}")
    }
}

fn fmt_tokens(n: f32) -> String {
    if n >= 1000. {
        format!("{:.1}k", n / 1000.)
    } else {
        format!("{n:.0}")
    }
}

#[derive(Clone, Copy)]
enum Command {
    Select(usize),
    Toggle(usize),
    Step(isize),
    Fold(bool),
    Replay,
}

/// Live numbers for a span at `now`: tokens and cost accrue as it runs, and
/// an agent sums its descendants.
fn totals(spans: &[TraceSpan], ix: usize, now: f32) -> (f32, f32, f32) {
    let one = |s: &TraceSpan| {
        let p = s.progress(now);
        // The prompt is sent up front; output and cost stream in.
        let sent = if now >= s.start { 1. } else { 0. };
        (
            s.tokens_in as f32 * sent,
            s.tokens_out as f32 * p,
            s.cost * p,
        )
    };
    let mut acc = one(&spans[ix]);
    for d in descendants(spans, ix) {
        let (a, b, c) = one(&spans[d]);
        acc = (acc.0 + a, acc.1 + b, acc.2 + c);
    }
    acc
}

impl RenderOnce for TraceWaterfall {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| TraceState {
                focus: cx.focus_handle(),
                started: Instant::now(),
                selected: None,
                hovered: None,
                collapsed: HashSet::new(),
            },
        );
        let (started, selected, hovered, collapsed, focus) = {
            let s = state.read(cx);
            (
                s.started,
                s.selected,
                s.hovered,
                s.collapsed.clone(),
                s.focus.clone(),
            )
        };
        let spans = Rc::new(self.spans);
        let total = spans.iter().map(|s| s.end).fold(0., f32::max);
        let now = match self.playback {
            Some(speed) if !motion::reduced_motion(cx) => {
                (started.elapsed().as_secs_f32() * speed * motion::speed_scale().recip()).min(total)
            }
            _ => total,
        };
        let live = now < total;
        if live {
            window.request_animation_frame();
        }

        let order = Rc::new(display_order(&spans));
        let hidden = |ix: usize| {
            let mut p = spans[ix].parent;
            while let Some(parent) = p {
                if collapsed.contains(&parent) {
                    return true;
                }
                p = spans[parent].parent;
            }
            false
        };
        let visible_rows: Vec<usize> = order
            .iter()
            .map(|(ix, _)| *ix)
            .filter(|ix| spans[*ix].start <= now && !hidden(*ix))
            .collect();

        // The slowest finished leaf step.
        let slowest = spans
            .iter()
            .enumerate()
            .filter(|(_, s)| s.kind != SpanKind::Agent && s.end <= now)
            .max_by(|a, b| a.1.duration().total_cmp(&b.1.duration()))
            .map(|(ix, _)| ix);

        // Time axis: rescales in whole steps as the run grows, gliding.
        let target_scale = if live {
            (((now * 1.2).max(8.)) / 5.).ceil() * 5.
        } else {
            total.max(1.) * 1.02
        };
        let scale = motion::follow(&format!("{key}-scale"), target_scale, GLIDE);
        let col_w = self.width - TREE_W - 2.;
        let axis_w = col_w - PAD_L - PAD_R;
        let x_of = move |s: f32| PAD_L + (s / scale).clamp(0., 1.2) * axis_w;

        let font_id = window.text_system().resolve_font(&font(luna::MONO));
        let advance = window
            .text_system()
            .em_advance(font_id, px(LABEL_SIZE))
            .map(f32::from)
            .unwrap_or(LABEL_SIZE * 0.6);

        let on_select = self.on_select.clone();
        let act: Rc<dyn Fn(Command, &mut Window, &mut App)> = {
            let state = state.clone();
            let spans = spans.clone();
            let order = order.clone();
            Rc::new(move |command, window, cx| {
                run(&state, &spans, &order, command, &on_select, window, cx)
            })
        };

        // ---- header ---------------------------------------------------------
        let (run_tokens, run_cost) =
            order
                .iter()
                .filter(|(_, depth)| *depth == 0)
                .fold((0., 0.), |acc, (ix, _)| {
                    let (tin, tout, cost) = totals(&spans, *ix, now);
                    (acc.0 + tin + tout, acc.1 + cost)
                });
        let calls = spans
            .iter()
            .filter(|s| s.kind != SpanKind::Agent && s.start <= now)
            .count();
        let metric = |value: String, label: &'static str| {
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(
                    div()
                        .font_family(luna::MONO)
                        .text_size(px(text::BODY))
                        .line_height(px(18.))
                        .text_color(t.text)
                        .child(value),
                )
                .child(
                    div()
                        .text_size(px(text::CAPTION))
                        .line_height(px(14.))
                        .text_color(t.text_faint)
                        .child(label),
                )
        };
        let status = if live {
            let dot = div()
                .size(px(6.))
                .rounded_full()
                .bg(t.accent)
                .with_animation(
                    SharedString::from(format!("{key}-pulse")),
                    gpui::Animation::new(std::time::Duration::from_millis(1400)).repeat(),
                    |el, p| el.opacity(0.35 + 0.65 * (1. - (p * 2. - 1.).abs())),
                );
            div()
                .flex()
                .items_center()
                .gap(px(space::S6))
                .h(px(22.))
                .px(px(space::S8))
                .rounded(px(radius::SM))
                .bg(t.accent.opacity(if t.dark { 0.16 } else { 0.10 }))
                .text_size(px(text::CAPTION))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.accent)
                .child(dot)
                .child("Live")
                .into_any_element()
        } else {
            luna::badge("Finished", Tone::Success, &t).into_any_element()
        };
        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S12))
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
                    .child(
                        svg()
                            .path(ICON_TRACE)
                            .size(px(16.))
                            .text_color(t.text_muted),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(self.title.clone()),
                    )
                    .when_some(self.subtitle.clone(), |d, s| {
                        d.child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(s),
                        )
                    }),
            )
            .child(metric(fmt_secs(now), "elapsed"))
            .child(metric(fmt_cost(run_cost), "cost"))
            .child(metric(fmt_tokens(run_tokens), "tokens"))
            .child(metric(calls.to_string(), "calls"))
            .child(div().w(px(1.)).h(px(28.)).bg(t.border))
            .child(status)
            .child({
                let act = act.clone();
                luna::icon_button(format!("{key}-replay"), ICON_REPLAY, ControlSize::Md, &t)
                    .on_click(move |_, window, cx| act(Command::Replay, window, cx))
            });

        // ---- axis -------------------------------------------------------------
        let step = tick_step(scale);
        let mut ticks = Vec::new();
        let mut s = 0.;
        while s <= scale + 0.001 {
            ticks.push(s);
            s += step;
        }
        let axis = div()
            .flex()
            .h(px(AXIS_H))
            .border_t_1()
            .border_b_1()
            .border_color(t.border)
            .text_size(px(text::CAPTION))
            .text_color(t.text_faint)
            .child(
                div()
                    .w(px(TREE_W))
                    .flex_none()
                    .flex()
                    .items_center()
                    .px(px(space::S16))
                    .border_r_1()
                    .border_color(t.border)
                    .child("Step"),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .font_family(luna::MONO)
                    .children(ticks.iter().map(|s| {
                        div()
                            .absolute()
                            .top(px(7.))
                            .left(px(x_of(*s) + 4.))
                            .child(format!("{}s", *s as u32))
                    }))
                    .when(live, |d| {
                        d.child(
                            div()
                                .absolute()
                                .top(px(4.))
                                .left(px(x_of(now) - 22.))
                                .w(px(44.))
                                .h(px(18.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(radius::XS))
                                .bg(t.accent)
                                .text_color(t.on_accent)
                                .child(fmt_secs(now)),
                        )
                    }),
            );

        // ---- rows -------------------------------------------------------------
        let mut rows: Vec<AnyElement> = Vec::new();
        for (ix, depth) in order.iter().copied() {
            let span = &spans[ix];
            if span.start > now {
                continue;
            }
            let vis = motion::follow(
                &format!("{key}-row{ix}-vis"),
                if hidden(ix) { 0. } else { 1. },
                GLIDE,
            );
            let born = motion::follow_from(&format!("{key}-row{ix}-born"), 0., 1., ROW_IN);
            let h = ROW_H * vis * born;
            if h < 0.1 {
                continue;
            }
            rows.push(row(
                &key,
                ix,
                depth,
                &spans,
                now,
                RowFlags {
                    selected: selected == Some(ix),
                    slowest: slowest == Some(ix),
                    collapsed: collapsed.contains(&ix),
                    has_children: spans.iter().any(|s| s.parent == Some(ix)),
                },
                (h, vis.min(born), born),
                &x_of,
                col_w,
                advance,
                state.clone(),
                act.clone(),
                &t,
            ));
        }

        let grid = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(TREE_W + 1.))
            .right_0()
            .children(ticks.iter().skip(1).map(|s| {
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(x_of(*s)))
                    .w(px(1.))
                    .bg(t.border.opacity(0.7))
            }))
            .when(live, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px(x_of(now)))
                        .w(px(1.))
                        .bg(t.accent.opacity(0.7)),
                )
            });
        let body = div()
            .relative()
            .py(px(space::S4))
            .min_h(px(ROW_H * visible_rows.len().max(1) as f32 + 8.))
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(TREE_W))
                    .w(px(1.))
                    .bg(t.border),
            )
            .child(grid)
            .children(rows);

        // ---- readout ----------------------------------------------------------
        // The step under the pointer, else the selection, else the whole run.
        let focus_ix = hovered
            .or(selected)
            .or_else(|| order.first().map(|(ix, _)| *ix));
        let readout = readout(&key, focus_ix, &spans, now, total, &t);

        let on_key = {
            let act = act.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let k = &event.keystroke;
                if k.modifiers.platform || k.modifiers.control || k.modifiers.alt {
                    return;
                }
                let command = match k.key.as_str() {
                    "down" | "j" => Command::Step(1),
                    "up" | "k" => Command::Step(-1),
                    "left" => Command::Fold(true),
                    "right" => Command::Fold(false),
                    "r" => Command::Replay,
                    _ => return,
                };
                act(command, window, cx);
                cx.stop_propagation();
            }
        };

        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, {
                let focus = focus.clone();
                move |_, window, _| window.focus(&focus)
            })
            .w(px(self.width))
            .overflow_hidden()
            .font_family(luna::SANS)
            .text_color(t.text)
            .child(header)
            .child(axis)
            .child(body)
            .child(readout)
    }
}

fn run(
    state: &Entity<TraceState>,
    spans: &[TraceSpan],
    order: &[(usize, usize)],
    command: Command,
    on_select: &Option<SelectHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut picked = None;
    state.update(cx, |s, cx| {
        let hidden = |s: &TraceState, ix: usize| {
            let mut p = spans[ix].parent;
            while let Some(parent) = p {
                if s.collapsed.contains(&parent) {
                    return true;
                }
                p = spans[parent].parent;
            }
            false
        };
        match command {
            Command::Select(ix) => {
                s.selected = Some(ix);
                picked = Some(ix);
            }
            Command::Toggle(ix) => {
                if !s.collapsed.remove(&ix) {
                    s.collapsed.insert(ix);
                }
            }
            Command::Step(delta) => {
                let rows: Vec<usize> = order
                    .iter()
                    .map(|(ix, _)| *ix)
                    .filter(|ix| !hidden(s, *ix))
                    .collect();
                if !rows.is_empty() {
                    let pos = s
                        .selected
                        .and_then(|sel| rows.iter().position(|r| *r == sel))
                        .map(|p| (p as isize + delta).clamp(0, rows.len() as isize - 1))
                        .unwrap_or(0) as usize;
                    s.selected = Some(rows[pos]);
                    picked = s.selected;
                }
            }
            Command::Fold(fold) => {
                if let Some(sel) = s.selected {
                    let has_children = spans.iter().any(|sp| sp.parent == Some(sel));
                    if fold && has_children {
                        s.collapsed.insert(sel);
                    } else if fold {
                        s.selected = spans[sel].parent.or(Some(sel));
                        picked = s.selected;
                    } else {
                        s.collapsed.remove(&sel);
                    }
                }
            }
            Command::Replay => {
                s.started = Instant::now();
                s.collapsed.clear();
                s.hovered = None;
            }
        }
        cx.notify();
    });
    if let (Some(f), Some(ix)) = (on_select, picked) {
        f(ix, window, cx);
    }
}

struct RowFlags {
    selected: bool,
    slowest: bool,
    collapsed: bool,
    has_children: bool,
}

#[allow(clippy::too_many_arguments)]
fn row(
    key: &str,
    ix: usize,
    depth: usize,
    spans: &[TraceSpan],
    now: f32,
    flags: RowFlags,
    (height, opacity, born): (f32, f32, f32),
    x_of: &dyn Fn(f32) -> f32,
    col_w: f32,
    advance: f32,
    state: Entity<TraceState>,
    act: Rc<dyn Fn(Command, &mut Window, &mut App)>,
    t: &Luna,
) -> AnyElement {
    let span = &spans[ix];
    let running = now < span.end;
    let row_key = format!("{key}-row-{ix}");
    let hover = motion::hover_t(&row_key);
    let sel = motion::follow(
        &format!("{row_key}-sel"),
        if flags.selected { 1. } else { 0. },
        HOVER_FADE,
    );
    let bg = motion::mix(
        t.fill(FillTier::Quaternary).opacity(0.),
        t.fill(FillTier::Quaternary),
        hover,
    );
    let bg = motion::mix(bg, t.accent.opacity(if t.dark { 0.12 } else { 0.07 }), sel);

    // Tree cell.
    let caret_turn = motion::follow(
        &format!("{row_key}-caret"),
        if flags.collapsed { 0. } else { 1. },
        GLIDE,
    );
    let caret = if flags.has_children {
        let act = act.clone();
        div()
            .id(SharedString::from(format!("{row_key}-caret")))
            .size(px(16.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::XS))
            .cursor_pointer()
            .hover_fill(
                format!("{row_key}-caret-h"),
                t.fill(FillTier::Secondary).opacity(0.),
                t.fill(FillTier::Secondary),
            )
            .on_click(move |_, window, cx| {
                act(Command::Toggle(ix), window, cx);
                cx.stop_propagation();
            })
            .child(
                svg()
                    .path(ICON_CHEVRON)
                    .size(px(12.))
                    .text_color(t.text_faint)
                    .with_transformation(Transformation::rotate(radians(
                        caret_turn * std::f32::consts::FRAC_PI_2,
                    ))),
            )
            .into_any_element()
    } else {
        div().w(px(16.)).flex_none().into_any_element()
    };
    let hidden_count = if flags.collapsed {
        descendants(spans, ix)
            .iter()
            .filter(|d| spans[**d].start <= now)
            .count()
    } else {
        0
    };
    let (tile_bg, tile_fg) = match span.kind {
        SpanKind::Llm => (t.accent.opacity(if t.dark { 0.18 } else { 0.10 }), t.accent),
        SpanKind::Tool => (t.fill(FillTier::Tertiary), t.text_muted),
        SpanKind::Agent => (t.fill(FillTier::Secondary), t.text),
    };
    let tree = div()
        .w(px(TREE_W))
        .flex_none()
        .h_full()
        .flex()
        .items_center()
        .gap(px(space::S6))
        .pl(px(space::S8 + depth as f32 * 16.))
        .pr(px(space::S12))
        .child(caret)
        .child(
            div()
                .size(px(18.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::XS + 1.))
                .bg(if span.failed {
                    t.danger.opacity(0.14)
                } else {
                    tile_bg
                })
                .child(
                    svg()
                        .path(span.kind.icon())
                        .size(px(12.))
                        .text_color(if span.failed { t.danger } else { tile_fg }),
                ),
        )
        .child(
            div()
                .flex_none()
                .max_w(px(150.))
                .truncate()
                .text_size(px(text::BODY))
                .when(span.kind == SpanKind::Agent, |d| {
                    d.font_weight(FontWeight::MEDIUM)
                })
                .text_color(if span.failed { t.danger } else { t.text })
                .child(span.name.clone()),
        )
        .when(hidden_count > 0, |d| {
            d.child(
                div()
                    .flex_none()
                    .h(px(16.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::XS))
                    .bg(t.fill(FillTier::Tertiary))
                    .text_size(px(text::CAPTION))
                    .text_color(t.text_muted)
                    .child(format!("+{hidden_count}")),
            )
        })
        .when_some(span.detail.clone(), |d, detail| {
            d.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(detail),
            )
        });

    // Bar cell.
    let end = span.end.min(now);
    let x0 = x_of(span.start);
    let x1 = x_of(end).max(x0 + 3.);
    let bar_w = (x1 - x0) * born.max(0.001);
    let agent = span.kind == SpanKind::Agent;
    let color = if span.failed {
        t.danger
    } else {
        match span.kind {
            SpanKind::Llm => t.accent,
            SpanKind::Tool => {
                if t.dark {
                    t.text_faint.opacity(0.85)
                } else {
                    t.text_faint.opacity(0.6)
                }
            }
            SpanKind::Agent => t.text_muted,
        }
    };
    let (bar_h, fill) = if agent {
        (6., color.opacity(if t.dark { 0.45 } else { 0.35 }))
    } else {
        (12., color.opacity(if running { 0.55 } else { 0.9 }))
    };
    let mut label = fmt_secs(end - span.start);
    let (_, _, cost) = totals(spans, ix, now);
    if cost > 0. {
        label.push_str(" · ");
        label.push_str(&fmt_cost(cost));
    }
    let tag: Option<(&'static str, Hsla)> = if span.failed && !running {
        Some(("failed", t.danger))
    } else if flags.slowest {
        Some(("slowest", t.warning))
    } else {
        None
    };
    let label_w = label.chars().count() as f32 * advance
        + tag.map_or(0., |(s, _)| s.len() as f32 * advance + 16.);
    let label_left = x1 + 6. + label_w < col_w - 4.;
    let label_x = if label_left {
        x1 + 6.
    } else {
        (x0 - 6. - label_w).max(4.)
    };
    let ring_color = motion::mix(
        if flags.slowest {
            t.warning.opacity(0.8)
        } else {
            t.accent.opacity(0.)
        },
        t.accent,
        sel.max(hover * 0.6),
    );
    let bar = div()
        .absolute()
        .left(px(x0))
        .top(px((ROW_H - bar_h) / 2.))
        .w(px(bar_w))
        .h(px(bar_h))
        .rounded(px(if agent { 3. } else { radius::XS }))
        .bg(fill)
        .when(ring_color.a > 0.02 && !agent, |d| {
            d.shadow(vec![ring(1., ring_color)])
        })
        .when(running && !agent, |d| {
            // The growing edge of a running call.
            d.child(
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(2.))
                    .rounded(px(1.))
                    .bg(color)
                    .with_animation(
                        SharedString::from(format!("{row_key}-edge")),
                        gpui::Animation::new(std::time::Duration::from_millis(900)).repeat(),
                        |el, p| el.opacity(0.4 + 0.6 * (1. - (p * 2. - 1.).abs())),
                    ),
            )
        });
    let label_el = div()
        .absolute()
        .left(px(label_x))
        .top_0()
        .h_full()
        .flex()
        .items_center()
        .gap(px(space::S6))
        .font_family(luna::MONO)
        .text_size(px(LABEL_SIZE))
        .text_color(if running { t.text_muted } else { t.text_faint })
        .child(label)
        .when_some(tag, |d, (tag, c)| {
            d.child(
                div()
                    .h(px(16.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .rounded(px(radius::XS))
                    .bg(c.opacity(if t.dark { 0.16 } else { 0.12 }))
                    .font_family(luna::SANS)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c)
                    .child(tag),
            )
        });
    let timeline = div()
        .relative()
        .flex_1()
        .h_full()
        .child(bar)
        .child(label_el);

    let hover_key = row_key.clone();
    div()
        .id(SharedString::from(row_key))
        .relative()
        .flex()
        .h(px(height))
        .overflow_hidden()
        .opacity(opacity)
        .bg(bg)
        .cursor_pointer()
        .on_hover(move |hovered: &bool, window, cx| {
            motion::set_hover(&hover_key, *hovered, motion::reduced_motion(cx));
            state.update(cx, |s, cx| {
                if *hovered {
                    s.hovered = Some(ix);
                } else if s.hovered == Some(ix) {
                    s.hovered = None;
                }
                cx.notify();
            });
            window.refresh();
        })
        .on_click(move |_, window, cx| act(Command::Select(ix), window, cx))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(2.))
                .bg(t.accent.opacity(sel)),
        )
        .child(tree)
        .child(div().w(px(1.)).flex_none())
        .child(timeline)
        .into_any_element()
}

/// The strip under the waterfall: the hovered or selected step in numbers.
fn readout(
    key: &str,
    focus: Option<usize>,
    spans: &[TraceSpan],
    now: f32,
    total: f32,
    t: &Luna,
) -> impl IntoElement {
    let base = div()
        .h(px(64.))
        .flex()
        .items_center()
        .gap(px(space::S16))
        .px(px(space::S16))
        .border_t_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary));
    let Some(ix) = focus.filter(|ix| spans[*ix].start <= now) else {
        return base
            .text_size(px(text::FOOTNOTE))
            .text_color(t.text_faint)
            .child("Hover or select a step to see its timing, tokens and cost.")
            .into_any_element();
    };
    let span = &spans[ix];
    let running = now < span.end;
    let (tin, tout, cost) = totals(spans, ix, now);
    let steps = descendants(spans, ix).len();
    let field = |label: &'static str, value: String| {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(text::CAPTION))
                    .text_color(t.text_faint)
                    .child(label),
            )
            .child(
                div()
                    .font_family(luna::MONO)
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text)
                    .child(value),
            )
    };
    let end = span.end.min(now);
    let share_track = 120.;
    let run_len = total.max(0.001);
    let seg_x = span.start / run_len * share_track;
    let seg_w = ((end - span.start) / run_len * share_track).max(2.);
    let kind = if span.failed {
        format!("{} · failed", span.kind.label())
    } else if running {
        format!("{} · running", span.kind.label())
    } else if span.kind == SpanKind::Agent {
        format!("{} · {steps} steps", span.kind.label())
    } else {
        span.kind.label().to_string()
    };
    base.child(
        div()
            .w(px(220.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .truncate()
                    .text_size(px(text::BODY))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if span.failed { t.danger } else { t.text })
                    .child(span.name.clone()),
            )
            .child(
                div()
                    .truncate()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(match &span.detail {
                        Some(d) => format!("{kind} · {d}"),
                        None => kind,
                    }),
            ),
    )
    .child(field("Start", fmt_secs(span.start)))
    .child(field(
        "Duration",
        format!(
            "{}{}",
            fmt_secs(end - span.start),
            if running { "…" } else { "" }
        ),
    ))
    .child(field(
        "Tokens in → out",
        if tin + tout > 0. {
            format!("{} → {}", fmt_tokens(tin), fmt_tokens(tout))
        } else {
            "–".into()
        },
    ))
    .child(field(
        "Cost",
        if cost > 0. {
            fmt_cost(cost)
        } else {
            "–".into()
        },
    ))
    .child(div().flex_1())
    .child(
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .text_size(px(text::CAPTION))
                    .text_color(t.text_faint)
                    .child("Share of run")
                    .child(
                        div()
                            .font_family(luna::MONO)
                            .text_color(t.text_muted)
                            .child(format!("{:.0}%", (end - span.start) / run_len * 100.)),
                    ),
            )
            .child(
                div()
                    .relative()
                    .w(px(share_track))
                    .h(px(6.))
                    .rounded(px(3.))
                    .bg(t.fill(FillTier::Secondary))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(motion::follow(&format!("{key}-share-x"), seg_x, GLIDE)))
                            .w(px(motion::follow(&format!("{key}-share-w"), seg_w, GLIDE)))
                            .h_full()
                            .rounded(px(3.))
                            .bg(if span.failed { t.danger } else { t.accent }),
                    ),
            ),
    )
    .with_animation(
        SharedString::from(format!("{key}-readout-{ix}")),
        MENU_IN.animation(),
        |el, p| el.opacity(0.4 + 0.6 * p),
    )
    .into_any_element()
}

fn demo_spans() -> Vec<TraceSpan> {
    vec![
        // 0
        TraceSpan::agent("Incident triage", 0.0, 23.8).detail("atlas-l"),
        // 1
        TraceSpan::llm("Plan the investigation", 0.2, 3.0)
            .parent(0)
            .detail("atlas-l")
            .tokens(2_140, 380)
            .cost(0.018),
        // 2
        TraceSpan::tool("Recent deploys", 3.2, 4.1)
            .parent(0)
            .detail("deploys.list"),
        // 3
        TraceSpan::tool("p99 by endpoint", 3.2, 5.3)
            .parent(0)
            .detail("metrics.query"),
        // 4
        TraceSpan::agent("Log analyst", 3.3, 12.9)
            .parent(0)
            .detail("atlas-s"),
        // 5
        TraceSpan::tool("Search error logs", 3.5, 6.2)
            .parent(4)
            .detail("logs.search"),
        // 6
        TraceSpan::tool("Search slow traces", 3.6, 8.8)
            .parent(4)
            .detail("logs.search"),
        // 7
        TraceSpan::llm("Cluster the errors", 9.0, 12.7)
            .parent(4)
            .detail("atlas-s")
            .tokens(9_420, 610)
            .cost(0.006),
        // 8
        TraceSpan::agent("Code reviewer", 4.4, 15.6)
            .parent(0)
            .detail("atlas-s"),
        // 9
        TraceSpan::tool("Diff deploy 7f3a2c", 4.6, 6.0)
            .parent(8)
            .detail("git.diff"),
        // 10
        TraceSpan::llm("Find risky changes", 6.2, 15.4)
            .parent(8)
            .detail("atlas-s")
            .tokens(14_200, 930)
            .cost(0.011),
        // 11
        TraceSpan::llm("Diagnose root cause", 15.8, 21.2)
            .parent(0)
            .detail("atlas-l")
            .tokens(18_650, 1_240)
            .cost(0.094),
        // 12
        TraceSpan::tool("Page on-call", 21.3, 21.9)
            .parent(0)
            .detail("pager.notify")
            .failed(),
        // 13
        TraceSpan::tool("Post status update", 22.0, 23.6)
            .parent(0)
            .detail("status.post"),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        TraceWaterfall::new("trace-waterfall", "Incident triage run")
            .subtitle("INC-4821 · checkout p99 latency spike")
            .spans(demo_spans())
            .playback(1.6)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_order_is_depth_first_by_start() {
        let order = display_order(&demo_spans());
        let ixs: Vec<usize> = order.iter().map(|(ix, _)| *ix).collect();
        assert_eq!(ixs[0], 0);
        assert_eq!(ixs[1], 1);
        // The log analyst's steps follow it directly.
        let pos = ixs.iter().position(|ix| *ix == 4).unwrap();
        assert_eq!(&ixs[pos + 1..pos + 4], &[5, 6, 7]);
        assert_eq!(order.iter().find(|(ix, _)| *ix == 6).unwrap().1, 2);
    }

    #[test]
    fn ticks_stay_readable() {
        assert_eq!(tick_step(8.), 2.);
        assert_eq!(tick_step(25.), 5.);
        assert_eq!(tick_step(45.), 10.);
    }
}
