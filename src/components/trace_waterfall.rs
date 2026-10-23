//! Trace Waterfall – a pink component.
//!
//! An agent run laid out against time. The step column is pink's tool
//! activity group (pink: `transcript.rs` `render_tool_group`): the run is the
//! group's header plate, its model and tool calls hang off the activity rail
//! (a 1 px hairline ribbon with a 6 px elbow), and sub-agents are rows with
//! pink's agent tile that fold their own steps away behind the disclosure
//! chevron. Beside every step its duration sits on a shared time axis as a
//! 6 px capsule (pink's usage meter), labelled with its time and cost in
//! pink's timer type. While the run is live, steps arrive on pink's reveal
//! (the rail draws in, the row rises), the axis rescales, a now-line sweeps
//! right and the header carries pink's working line: breathing orb,
//! shimmering verb and "· 14s". Run totals sit in pink's usage metric cells;
//! the strip at the bottom reads out the step under the pointer (or the
//! selection). ↑ / ↓ move the selection, ← / → fold and unfold, R replays.
//!
//! ```ignore
//! TraceWaterfall::new("run", "Nightly eval")
//!     .span(TraceSpan::agent("Evaluator", 0.0, 9.5))
//!     .span(TraceSpan::llm("Grade answers", 0.4, 6.1).parent(0).tokens(8_200, 640).cost(0.031))
//!     .playback(1.0)
//! ```

use std::{
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, ClickEvent, Div, ElementId, Entity, FocusHandle, FontWeight,
    IntoElement, KeyDownEvent, MouseButton, PathBuilder, Pixels, Point, SharedString, Stateful,
    TextRun, Transformation, Window, canvas, div, font, point, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    motion::{
        self, DISCLOSURE, EMPHASIZED, HOVER_FADE, TOOL_BODY_FADE, TOOL_BODY_FADE_OUT_FRACTION,
        TOOL_BODY_RISE, TOOL_CONNECTOR_REVEAL, TOOL_FOLD, TOOL_ROW_REVEAL,
    },
    pink::{
        self, ControlSize, FillTier, Pink, TextStyle, TypeStyleExt, layout,
        loaders::{self, TOOL_LABEL_LINE_HEIGHT, TOOL_LABEL_SIZE},
        radius,
    },
    story,
};

pub const NAME: &str = "Trace Waterfall";
pub const DESCRIPTION: &str = "An agent run against time – its sub-agents, model calls and tool calls on an activity rail that folds, each with a duration bar and its time and cost on a shared axis, a live working line while it runs and a readout of the step under the pointer";

const ICON_BOT: &str = "icons/trace-waterfall/bot.svg";
const ICON_BRAIN: &str = "icons/trace-waterfall/brain.svg";
const ICON_TOOLBOX: &str = "icons/trace-waterfall/toolbox.svg";
const ICON_DANGER: &str = "icons/trace-waterfall/danger-circle.svg";
const ICON_CHEVRON: &str = "icons/trace-waterfall/alt-arrow-right.svg";
const ICON_RESTART: &str = "icons/trace-waterfall/restart.svg";

// Step column (pink: `transcript.rs` tool-group and activity-rail constants).
/// The group header row; its plate is 2 px shorter
/// (pink `TOOL_GROUP_HEADER_HEIGHT`).
const HEADER_H: f32 = 26.0;
/// A step on the rail (pink `TOOL_TREE_ROW_HEIGHT`).
const ROW_H: f32 = 32.0;
/// Between the header and its first step (pink `CHIPS_TOP_PAD`).
const BODY_TOP_PAD: f32 = 2.0;
/// The rail's gutter: trunk, elbow, branch and glyph
/// (pink `ACTIVITY_GUTTER_WIDTH`, `ACTIVITY_TEXT_GAP`).
const GUTTER_W: f32 = 48.0;
const TEXT_GAP: f32 = 8.0;
const TRUNK_X: f32 = 12.5;
const BEND_RADIUS: f32 = 6.0;
const BRANCH_END_X: f32 = 28.0;
const ICON_LEFT: f32 = 32.0;
const ICON_SIZE: f32 = 14.0;
const RAIL_ALPHA: f32 = 0.12;
/// The sub-agent tile (pink `chip_header_row`: 18, radius 6, `ink(0.08)`,
/// a 12 icon).
const TILE: f32 = 18.0;
const TILE_ICON: f32 = 12.0;
/// Disclosure chevron (pink `DISCLOSURE_CHEVRON_SIZE`).
const CHEVRON: f32 = 12.0;
/// A sub-agent's steps hang from its tile: their trunk runs under the tile's
/// centre, on a half pixel so the 1 px ribbon stays crisp.
const INDENT: f32 = 27.0;
const TILE_CX: f32 = TRUNK_X + INDENT;
/// The header plate's left edge in the step column.
const TREE_X0: f32 = 10.0;
const TREE_W: f32 = 340.0;
const CELL_PAD_R: f32 = 10.0;
/// Deepest nesting drawn.
const MAX_DEPTH: usize = 8;
/// Steps starting this close together arrive as one staggered batch.
const SAME_ARRIVAL: f32 = 0.05;

// Time axis (pink-ui; pink's usage meter, table header and timer type).
/// Header row of the table (pink `usage_table_row` header, `min_h 30`).
const AXIS_H: f32 = 30.0;
/// A step's bar: pink's 6 pt usage-meter capsule.
const BAR_H: f32 = 6.0;
const PAD_L: f32 = 12.0;
/// Room after the axis for the longest bar's label.
const PAD_R: f32 = 88.0;
const LABEL_GAP: f32 = 6.0;
/// Pink's meta / timer size and line.
const META_SIZE: f32 = 11.0;
const META_LINE: f32 = 14.0;
const READOUT_H: f32 = 60.0;
const SHARE_W: f32 = 120.0;

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
    /// Pink's glyph for the kind (pink `tool_icon_path`: bot for agents,
    /// brain for model thinking, toolbox for tools).
    fn icon(self) -> &'static str {
        match self {
            SpanKind::Agent => ICON_BOT,
            SpanKind::Llm => ICON_BRAIN,
            SpanKind::Tool => ICON_TOOLBOX,
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
    /// The model or tool, shown in the row's trailing slot.
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

    fn status(&self, now: f32) -> Status {
        if now < self.end {
            Status::Running
        } else if self.failed {
            Status::Failed
        } else {
            Status::Done
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Running,
    Done,
    Failed,
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type Act = Rc<dyn Fn(Command, &mut Window, &mut App)>;

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
            width: 880.,
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

    /// Card width in px (default 880).
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
    /// When each fold last turned, and how open it was then.
    toggled: HashMap<usize, (Instant, f32)>,
}

/// Every span's children (the roots at `spans.len()`), siblings by start.
fn children(spans: &[TraceSpan]) -> Vec<Vec<usize>> {
    let n = spans.len();
    let mut kids = vec![Vec::new(); n + 1];
    for (ix, span) in spans.iter().enumerate() {
        kids[slot_of(span, ix, n)].push(ix);
    }
    for list in &mut kids {
        list.sort_by(|a, b| spans[*a].start.total_cmp(&spans[*b].start));
    }
    kids
}

/// The child list a span belongs to: its parent's, or the roots'.
fn slot_of(span: &TraceSpan, ix: usize, n: usize) -> usize {
    match span.parent {
        Some(parent) if parent < n && parent != ix => parent,
        _ => n,
    }
}

/// Spans in display order (depth first, siblings by start) with their depth.
fn display_order(spans: &[TraceSpan]) -> Vec<(usize, usize)> {
    fn visit(kids: &[Vec<usize>], slot: usize, depth: usize, out: &mut Vec<(usize, usize)>) {
        for &ix in &kids[slot] {
            out.push((ix, depth));
            if depth < MAX_DEPTH {
                visit(kids, ix, depth + 1, out);
            }
        }
    }
    let kids = children(spans);
    let mut out = Vec::new();
    visit(&kids, spans.len(), 0, &mut out);
    out
}

fn descendants(spans: &[TraceSpan], ix: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![ix];
    while let Some(p) = stack.pop() {
        for (c, s) in spans.iter().enumerate() {
            if s.parent == Some(p) && c != p && c != ix && !out.contains(&c) {
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

/// A step's duration: tenths under 10 s, whole seconds to a minute, then
/// pink's `1m 05s` (pink `native_agents::duration_label`).
fn fmt_secs(s: f32) -> String {
    let s = s.max(0.);
    if s < 10. {
        format!("{s:.1}s")
    } else if s < 60. {
        format!("{s:.0}s")
    } else {
        let whole = s as u32;
        format!("{}m {:02}s", whole / 60, whole % 60)
    }
}

fn fmt_tick(s: f32) -> String {
    let whole = s.round() as u32;
    match (whole / 60, whole % 60) {
        (0, secs) => format!("{secs}s"),
        (mins, 0) => format!("{mins}m"),
        (mins, secs) => format!("{mins}m {secs}s"),
    }
}

/// Live spend (pink `usage_live::live_money`): mills under a dollar.
fn fmt_cost(c: f32) -> String {
    if c < 1. {
        format!("${:.3}", c.max(0.))
    } else {
        let cents = (c * 100.).round() as u64;
        format!("${}.{:02}", cents / 100, cents % 100)
    }
}

/// Token counts (pink `usage::compact_tokens`): `640`, `2.14K`, `32.1K`.
fn fmt_tokens(n: f32) -> String {
    let value = n.max(0.).round() as u64;
    for (threshold, suffix) in [(1_000_000_000_u64, "B"), (1_000_000, "M"), (1_000, "K")] {
        if value >= threshold {
            let scaled = value as f64 / threshold as f64;
            let digits = if scaled >= 100. {
                0
            } else if scaled >= 10. {
                1
            } else {
                2
            };
            let formatted = format!("{scaled:.digits$}");
            let trimmed = if formatted.contains('.') {
                formatted.trim_end_matches('0').trim_end_matches('.')
            } else {
                &formatted
            };
            return format!("{trimmed}{suffix}");
        }
    }
    value.to_string()
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

/// A fold on one clock (pink: `transcript.rs` tool fold): the body height,
/// chevron turn and body fade all read it, so they agree mid-fold.
#[derive(Clone, Copy)]
struct Fold {
    /// 0 closed, 1 open.
    progress: f32,
    /// The opening body fades in over [`TOOL_BODY_FADE`]; a closing one is
    /// clear by [`TOOL_BODY_FADE_OUT_FRACTION`] of the fold.
    alpha: f32,
    moving: bool,
}

/// Where a fold is at `clock`, turning on [`TOOL_FOLD`] from how open it
/// was when last toggled (pink lerps from the rendered height the same way).
fn fold_at(open: bool, toggled: Option<(Instant, f32)>, clock: Instant, reduced: bool) -> Fold {
    let target = if open { 1. } else { 0. };
    let settled = Fold {
        progress: target,
        alpha: 1.,
        moving: false,
    };
    let Some((at, from)) = toggled.filter(|_| !reduced) else {
        return settled;
    };
    let elapsed_ms =
        clock.saturating_duration_since(at).as_secs_f32() * 1000. / motion::speed_scale();
    let raw = elapsed_ms / TOOL_FOLD.duration_ms as f32;
    if raw >= 1. {
        return settled;
    }
    let alpha = if open {
        TOOL_BODY_FADE
            .curve
            .eval((elapsed_ms / TOOL_BODY_FADE.duration_ms as f32).clamp(0., 1.))
    } else {
        1. - (raw / TOOL_BODY_FADE_OUT_FRACTION).clamp(0., 1.)
    };
    Fold {
        progress: motion::lerp(from, target, TOOL_FOLD.curve.eval(raw)),
        alpha,
        moving: true,
    }
}

/// The time axis: seconds to x inside the timeline column.
#[derive(Clone, Copy)]
struct Axis {
    scale: f32,
    width: f32,
}

impl Axis {
    fn x(&self, s: f32) -> f32 {
        PAD_L + (s / self.scale).clamp(0., 1.2) * self.width
    }
}

/// A bar's label and its shaped width.
struct BarLabel {
    text: String,
    width: f32,
}

impl RenderOnce for TraceWaterfall {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| TraceState {
                focus: cx.focus_handle(),
                started: Instant::now(),
                selected: None,
                hovered: None,
                collapsed: HashSet::new(),
                toggled: HashMap::new(),
            },
        );
        let (started, selected, hovered, collapsed, toggled, focus) = {
            let s = state.read(cx);
            (
                s.started,
                s.selected,
                s.hovered,
                s.collapsed.clone(),
                s.toggled.clone(),
                s.focus.clone(),
            )
        };
        let clock = Instant::now();
        let spans = Rc::new(self.spans);
        let n = spans.len();
        let total = spans.iter().map(|s| s.end).fold(0., f32::max);
        let now = match self.playback {
            Some(speed) if !reduced => (clock.saturating_duration_since(started).as_secs_f32()
                * speed
                / motion::speed_scale())
            .min(total),
            _ => total,
        };
        let live = now < total;

        let all_kids = children(&spans);
        let order = Rc::new(display_order(&spans));
        let kids: Vec<Vec<usize>> = all_kids
            .iter()
            .map(|list| {
                list.iter()
                    .copied()
                    .filter(|c| spans[*c].start <= now)
                    .collect()
            })
            .collect();

        // Arrivals (pink: `transcript.rs` row reveal starts): a step reveals
        // when the playback clock reaches it; a group's first step waits
        // 90 ms and steps arriving together stagger by 65 ms.
        let scale_motion = motion::speed_scale();
        let arrival: Vec<Option<Instant>> = (0..n)
            .map(|ix| {
                let speed = self.playback?;
                let span = &spans[ix];
                let slot = slot_of(span, ix, n);
                let delay = if slot == n {
                    Duration::ZERO
                } else {
                    let siblings = &all_kids[slot];
                    let pos = siblings.iter().position(|c| *c == ix).unwrap_or(0);
                    let rank = siblings[..pos]
                        .iter()
                        .rev()
                        .take_while(|c| (spans[**c].start - span.start).abs() < SAME_ARRIVAL)
                        .count();
                    motion::tool_row_arrival_delay(rank, pos == rank)
                };
                Some(
                    started
                        + Duration::from_secs_f32(span.start.max(0.) / speed * scale_motion)
                        + delay.mul_f32(scale_motion),
                )
            })
            .collect();
        let reveal: Vec<f32> = arrival
            .iter()
            .map(|at| motion::reveal_progress(TOOL_ROW_REVEAL, *at, clock, reduced))
            .collect();
        let connector: Vec<f32> = arrival
            .iter()
            .map(|at| motion::reveal_progress(TOOL_CONNECTOR_REVEAL, *at, clock, reduced))
            .collect();
        let folds: Vec<Fold> = (0..n)
            .map(|ix| {
                fold_at(
                    !collapsed.contains(&ix),
                    toggled.get(&ix).copied(),
                    clock,
                    reduced,
                )
            })
            .collect();
        let moving = live
            || folds.iter().any(|f| f.moving)
            || (0..n).any(|ix| spans[ix].start <= now && (reveal[ix] < 1. || connector[ix] < 1.));
        if moving {
            window.request_animation_frame();
        }

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
        let scale = motion::follow(&format!("{key}-scale"), target_scale, EMPHASIZED);
        let col_w = self.width - 2. - TREE_W;
        let axis = Axis {
            scale,
            width: col_w - PAD_L - PAD_R,
        };

        // Bar labels, shaped so they can flip to the bar's left near the edge.
        let mut label_font = font(pink::SANS);
        label_font.features = pink::tabular_figures();
        let measure = |text: &str, weight: FontWeight, window: &mut Window| -> f32 {
            let mut font = label_font.clone();
            font.weight = weight;
            let run = TextRun {
                len: text.len(),
                font,
                color: t.text,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            f32::from(
                window
                    .text_system()
                    .shape_line(
                        SharedString::from(text.to_string()),
                        px(META_SIZE),
                        &[run],
                        None,
                    )
                    .width(),
            )
        };
        let slowest_w = measure("Slowest", FontWeight::MEDIUM, window) + LABEL_GAP;
        let labels: Vec<Option<BarLabel>> = (0..n)
            .map(|ix| {
                let span = &spans[ix];
                if span.start > now {
                    return None;
                }
                let mut text = fmt_secs(span.end.min(now) - span.start);
                let (_, _, cost) = totals(&spans, ix, now);
                if cost > 0. {
                    text.push_str(" · ");
                    text.push_str(&fmt_cost(cost));
                }
                let mut width = measure(&text, FontWeight::NORMAL, window);
                if slowest == Some(ix) {
                    width += slowest_w;
                }
                Some(BarLabel { text, width })
            })
            .collect();

        let on_select = self.on_select.clone();
        let act: Act = {
            let state = state.clone();
            let spans = spans.clone();
            let order = order.clone();
            Rc::new(move |command, window, cx| {
                run(&state, &spans, &order, now, command, &on_select, window, cx)
            })
        };

        let frame = Frame {
            key: &key,
            t: &t,
            spans: &spans,
            kids: &kids,
            now,
            arrival: &arrival,
            reveal: &reveal,
            connector: &connector,
            folds: &folds,
            labels: &labels,
            collapsed: &collapsed,
            selected,
            slowest,
            axis,
            col_w,
            state: &state,
            act: &act,
        };

        // ---- header ---------------------------------------------------------
        let run_status = if live {
            // Pink's working line (pink: `transcript.rs` `render_working_trailer`).
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(loaders::orb_slot(t.accent))
                .child(loaders::working_status("Running", t.content_muted, t.text))
                .child(loaders::working_timer(now as i64, t.content_faint))
                .into_any_element()
        } else {
            // Pink's status read (pink: `transcript/agent_cards.rs` meta line):
            // a dot and a quiet word, then the settled duration.
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(TOOL_LABEL_SIZE))
                .line_height(px(16.))
                .text_color(t.content_muted)
                .child(
                    div()
                        .flex_none()
                        .size(px(6.))
                        .rounded_full()
                        .bg(t.readable_tone(t.success, 1.)),
                )
                .child("Done")
                .child(div().text_color(t.content_faint).child("·"))
                .child(
                    div()
                        .font_features(pink::tabular_figures())
                        .child(loaders::format_elapsed(total.round() as i64)),
                )
                .into_any_element()
        };
        // Pink's section header (pink: `settings/widgets.rs` `section_header`)
        // with the run's status and replay in its trailing controls, centred
        // on the title's line.
        let header = pink::section_header(self.title.clone(), self.subtitle.clone(), &t)
            .flex_none()
            .child(
                pink::row_controls()
                    .h(px(ControlSize::Sm.height()))
                    .mt(px((TextStyle::Title3.line_height()
                        - ControlSize::Sm.height())
                        / 2.))
                    .gap(px(10.))
                    .child(run_status)
                    .when(self.playback.is_some(), |row| {
                        let act = act.clone();
                        row.child(
                            pink::icon_button(
                                format!("{key}-replay"),
                                ICON_RESTART,
                                ControlSize::Sm,
                                &t,
                            )
                            .tooltip(pink::text_tooltip("Replay"))
                            .tooltip_show_delay(layout::TOOLTIP_DELAY)
                            .on_click(move |_, window, cx| act(Command::Replay, window, cx)),
                        )
                    }),
            );

        // ---- totals (pink: `shell/usage.rs` Totals grid of metric cells) ----
        let roots = &kids[n];
        let (run_tokens, run_cost) = roots.iter().fold((0., 0.), |acc, ix| {
            let (tin, tout, cost) = totals(&spans, *ix, now);
            (acc.0 + tin + tout, acc.1 + cost)
        });
        let count = |kind: SpanKind| {
            spans
                .iter()
                .filter(|s| s.kind == kind && s.start <= now)
                .count()
        };
        let metrics = div()
            .w_full()
            .flex_none()
            .px(px(layout::ROW_PAD_X))
            .py(px(layout::ROW_PAD_Y))
            .border_t_1()
            .border_color(t.border)
            .flex()
            .items_stretch()
            .gap(px(12.))
            .child(metric_cell(
                "Cost",
                if run_cost > 0. {
                    fmt_cost(run_cost)
                } else {
                    "—".into()
                },
                &t,
            ))
            .child(metric_cell(
                "Tokens",
                if run_tokens > 0. {
                    fmt_tokens(run_tokens)
                } else {
                    "—".into()
                },
                &t,
            ))
            .child(metric_cell(
                "Model calls",
                count(SpanKind::Llm).to_string(),
                &t,
            ))
            .child(metric_cell(
                "Tool calls",
                count(SpanKind::Tool).to_string(),
                &t,
            ));

        // ---- axis -------------------------------------------------------------
        let step = tick_step(scale);
        let mut ticks = Vec::new();
        let mut s = 0.;
        while s <= scale + 0.001 {
            ticks.push(s);
            s += step;
        }
        let axis_row = div()
            .w_full()
            .h(px(AXIS_H))
            .flex_none()
            .flex()
            .items_center()
            .border_t_1()
            .border_color(t.border)
            .text_size(px(META_SIZE))
            .line_height(px(META_LINE))
            .text_color(t.readable_muted(0.85))
            .child(
                div()
                    .w(px(TREE_W))
                    .flex_none()
                    .pl(px(layout::ROW_PAD_X))
                    .child("Step"),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .h_full()
                    .font_features(pink::tabular_figures())
                    .children(ticks.iter().map(|s| {
                        div()
                            .absolute()
                            .top(px((AXIS_H - META_LINE) / 2.))
                            .left(px(axis.x(*s).round() + 4.))
                            .whitespace_nowrap()
                            .child(fmt_tick(*s))
                    })),
            );

        // ---- steps ------------------------------------------------------------
        let grid = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(TREE_W))
            .right_0()
            .children(ticks.iter().skip(1).map(|s| {
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(axis.x(*s).round()))
                    .w(px(1.))
                    .bg(t.hairline(0.06))
            }));
        let body = div()
            .relative()
            .w_full()
            .flex_none()
            .flex()
            .flex_col()
            .pt(px(4.))
            .pb(px(8.))
            .border_t_1()
            .border_color(t.border)
            .child(grid)
            .children(
                roots
                    .iter()
                    .enumerate()
                    .map(|(k, ix)| frame.block(*ix, 0, k > 0, roots.get(k + 1).copied())),
            )
            .when(live, |body| {
                // The now-line: where the run has got to, in pink's Working
                // status tone (pink: `shell/spaces.rs` `busy@0.55`).
                body.child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(px((TREE_W + axis.x(now)).round()))
                        .w(px(1.))
                        .bg(t.busy.opacity(0.55)),
                )
            });

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
        let ring = t.focus_ring();
        pink::card(&t)
            .id(self.id.clone())
            .track_focus(&focus)
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, {
                let focus = focus.clone();
                move |_, window, cx| window.focus(&focus, cx)
            })
            .w(px(self.width))
            .overflow_hidden()
            .flex()
            .flex_col()
            .text_color(t.text)
            .child(header)
            .child(metrics)
            .child(axis_row)
            .child(body)
            .child(readout)
    }
}

/// One figure of the run's totals (pink: `shell/usage.rs` `metric_cell`):
/// a caption over a Title3 value on the resting plate.
fn metric_cell(label: &'static str, value: String, t: &Pink) -> Div {
    div()
        .min_w(px(96.))
        .flex_1()
        .rounded(px(radius::MD))
        .border_1()
        .border_color(t.separator(false))
        .bg(t.fill(FillTier::Quaternary))
        .px(px(12.))
        .py(px(10.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(META_SIZE))
                .line_height(px(META_LINE))
                .text_color(t.readable_muted(0.85))
                .child(label),
        )
        .child(
            div()
                .type_style(TextStyle::Title3)
                .font_features(pink::tabular_figures())
                .text_color(t.text)
                .child(value),
        )
}

/// Everything one frame of the step list reads.
struct Frame<'a> {
    key: &'a str,
    t: &'a Pink,
    spans: &'a [TraceSpan],
    /// Started children of every span; the roots at `spans.len()`.
    kids: &'a [Vec<usize>],
    now: f32,
    arrival: &'a [Option<Instant>],
    reveal: &'a [f32],
    connector: &'a [f32],
    folds: &'a [Fold],
    labels: &'a [Option<BarLabel>],
    collapsed: &'a HashSet<usize>,
    selected: Option<usize>,
    slowest: Option<usize>,
    axis: Axis,
    col_w: f32,
    state: &'a Entity<TraceState>,
    act: &'a Act,
}

/// Left edge of the rail gutter at `depth` (1 = the header's steps).
fn gutter_x(depth: usize) -> f32 {
    TREE_X0 + depth.saturating_sub(1) as f32 * INDENT
}

fn line_h(depth: usize) -> f32 {
    if depth == 0 { HEADER_H } else { ROW_H }
}

impl Frame<'_> {
    /// The height of `ix`'s steps when open.
    fn body_h(&self, ix: usize, depth: usize) -> f32 {
        let kids = &self.kids[ix];
        if kids.is_empty() || depth >= MAX_DEPTH {
            return 0.;
        }
        let pad = if depth == 0 { BODY_TOP_PAD } else { 0. };
        pad + kids
            .iter()
            .map(|c| self.block_h(*c, depth + 1) * self.reveal[*c])
            .sum::<f32>()
    }

    /// A step's line and its body as far as it is open.
    fn block_h(&self, ix: usize, depth: usize) -> f32 {
        line_h(depth) + self.body_h(ix, depth) * self.folds[ix].progress
    }

    /// One step with its folding body (pink: `transcript.rs`
    /// `render_tool_group` and `reveal_tool_row`): the line, the steps under
    /// it clipped to the fold, and for steps on a rail their connector. An
    /// arriving step is clipped to its reveal.
    fn block(&self, ix: usize, depth: usize, has_pred: bool, next: Option<usize>) -> AnyElement {
        let fold = self.folds[ix];
        let body_h = self.body_h(ix, depth);
        let natural = line_h(depth) + body_h * fold.progress;
        let mut inner = div()
            .relative()
            .w_full()
            .h(px(natural))
            .flex_none()
            .flex()
            .flex_col()
            .child(self.line(ix, depth, has_pred));
        if body_h > 0. && fold.progress > 0. {
            let kids = &self.kids[ix];
            let mut content = div()
                .w_full()
                .flex_none()
                .flex()
                .flex_col()
                .when(depth == 0, |content| content.pt(px(BODY_TOP_PAD)))
                .children(
                    kids.iter()
                        .enumerate()
                        .map(|(k, c)| self.block(*c, depth + 1, k > 0, kids.get(k + 1).copied())),
                );
            // A sub-agent's steps fade and settle like pink's row bodies; the
            // group under the header only clips.
            if depth > 0 && fold.alpha < 1. {
                content = content
                    .relative()
                    .top(px(TOOL_BODY_RISE * (1. - fold.alpha)))
                    .opacity(fold.alpha);
            }
            inner = inner.child(
                div()
                    .w_full()
                    .h(px(body_h * fold.progress))
                    .flex_none()
                    .overflow_hidden()
                    .child(content),
            );
        }
        if depth > 0 {
            inner = inner.child(self.rail(ix, depth, has_pred, next));
        }
        let reveal = self.reveal[ix];
        if reveal >= 1. {
            inner.into_any_element()
        } else {
            div()
                .w_full()
                .h(px(natural * reveal))
                .flex_none()
                .overflow_hidden()
                .child(inner)
                .into_any_element()
        }
    }

    /// The row a step reads on: the step cell and its bar. Hover is a
    /// Tertiary wash, the selection the Primary fill (pink's chrome rows).
    fn line(&self, ix: usize, depth: usize, has_pred: bool) -> Stateful<Div> {
        let t = self.t;
        let row_key = format!("{}-row-{ix}", self.key);
        let hover = motion::hover_t(&row_key);
        let sel = motion::follow(
            &format!("{row_key}-sel"),
            if self.selected == Some(ix) { 1. } else { 0. },
            HOVER_FADE,
        );
        let wash = t.fill(FillTier::Tertiary);
        let band = motion::mix(
            motion::mix(wash.opacity(0.), wash, hover),
            t.glass_selected_bg(),
            sel,
        );
        let height = line_h(depth);
        // Text rises in with the branch (pink: `tool_connector_parts(..).1`).
        let content = if depth == 0 {
            1.
        } else {
            motion::tool_connector_parts(self.connector[ix], has_pred).1
        };
        let cell = if depth == 0 {
            self.plate_cell(ix)
        } else {
            self.step_cell(ix, depth, &row_key, hover.max(sel), content)
        };
        let state = self.state.clone();
        let act = self.act.clone();
        let hover_key = row_key.clone();
        div()
            .id(SharedString::from(row_key))
            .w_full()
            .h(px(height))
            .flex_none()
            .flex()
            .bg(band)
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
            .child(cell)
            .child(self.bar_cell(ix, height, content))
    }

    fn toggle(&self, ix: usize) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let act = self.act.clone();
        move |_, window, cx| {
            cx.stop_propagation();
            act(Command::Toggle(ix), window, cx);
        }
    }

    /// A root step as the group's header plate (pink: `transcript.rs`
    /// `render_tool_group` header): kind glyph on the rail's trunk, the
    /// summary (shimmering while live) and a chevron that turns as the group
    /// opens, on a resting fill that brightens under the pointer.
    fn plate_cell(&self, ix: usize) -> Div {
        let t = self.t;
        let span = &self.spans[ix];
        let status = span.status(self.now);
        let has_kids = !self.kids[ix].is_empty();
        let plate_key = format!("{}-plate-{ix}", self.key);
        let fg = motion::hover_blend(&plate_key, t.content_muted, t.text);
        let steps: Vec<usize> = descendants(self.spans, ix)
            .into_iter()
            .filter(|d| self.spans[*d].start <= self.now)
            .collect();
        let failed = steps
            .iter()
            .filter(|d| self.spans[**d].status(self.now) == Status::Failed)
            .count();
        // Pink's summary line: "Thought · Read 3 files · 1 failed".
        let mut summary = span.name.to_string();
        if !steps.is_empty() {
            let plural = if steps.len() == 1 { "" } else { "s" };
            summary.push_str(&format!(" · {} step{plural}", steps.len()));
        }
        if failed > 0 {
            summary.push_str(&format!(" · {failed} failed"));
        }
        let title =
            loaders::shimmer_text(summary, TOOL_LABEL_SIZE, TOOL_LABEL_LINE_HEIGHT, fg, t.text)
                .live(status == Status::Running)
                .flex_1()
                .min_w_0()
                .overflow_hidden();
        let title = match self.arrival[ix] {
            Some(at) => title.started_at(at),
            None => title,
        };
        let plate = div()
            .id(SharedString::from(plate_key.clone()))
            .min_w_0()
            .h(px(HEADER_H - 2.))
            .flex()
            .items_center()
            .gap(px(6.))
            .pl(px(TRUNK_X - ICON_SIZE / 2.))
            .pr(px(8.))
            .rounded(px(radius::CONTROL))
            .cursor_pointer()
            .hover_fill_fg(
                plate_key,
                t.fill(FillTier::Quaternary),
                t.fill(FillTier::Tertiary),
                t.content_muted,
                t.text,
            )
            .when(has_kids, |plate| plate.on_click(self.toggle(ix)))
            .child(
                svg()
                    .path(span.kind.icon())
                    .flex_none()
                    .size(px(ICON_SIZE))
                    .text_color(fg),
            )
            .child(
                div()
                    .min_w_0()
                    .h(px(TOOL_LABEL_LINE_HEIGHT))
                    .flex()
                    .items_center()
                    .truncate()
                    .child(title),
            )
            .when(has_kids, |plate| {
                plate.child(
                    svg()
                        .path(ICON_CHEVRON)
                        .flex_none()
                        .size(px(CHEVRON))
                        .with_transformation(Transformation::rotate(radians(
                            FRAC_PI_2 * self.folds[ix].progress,
                        )))
                        .text_color(t.content_faint),
                )
            });
        div()
            .w(px(TREE_W))
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .gap(px(TEXT_GAP))
            .pl(px(TREE_X0))
            .pr(px(CELL_PAD_R))
            .text_size(px(TOOL_LABEL_SIZE))
            .line_height(px(TOOL_LABEL_LINE_HEIGHT))
            .child(plate)
            .child(div().flex_1().min_w(px(4.)))
            .when(status == Status::Failed, |cell| cell.child(failed_word(t)))
            .when_some(span.detail.clone(), |cell, detail| {
                cell.child(trailing_meta(detail, t))
            })
            // Lines the trailing column up with the steps' disclosure slot.
            .child(div().size(px(TILE)).flex_none())
    }

    /// A step on the rail (pink: `transcript.rs` `chip_header_row`): the
    /// label in `content_muted` brightening under the pointer (medium for a
    /// sub-agent), then the trailing slot – "Failed" as one red word, the
    /// model or tool in faint 11 and the disclosure tile, half strength until
    /// the row is hovered.
    fn step_cell(&self, ix: usize, depth: usize, row_key: &str, lit: f32, reveal: f32) -> Div {
        let t = self.t;
        let span = &self.spans[ix];
        let status = span.status(self.now);
        let agent = span.kind == SpanKind::Agent;
        let has_kids = !self.kids[ix].is_empty();
        let hover = motion::hover_t(row_key);
        let folded = if self.collapsed.contains(&ix) {
            descendants(self.spans, ix)
                .iter()
                .filter(|d| self.spans[**d].start <= self.now)
                .count()
        } else {
            0
        };
        // A folded sub-agent says what it holds, like pink's group summary.
        let detail: Option<SharedString> = match (folded, span.detail.clone()) {
            (0, detail) => detail,
            (n, Some(detail)) => Some(format!("{n} steps · {detail}").into()),
            (n, None) => Some(format!("{n} steps").into()),
        };
        let cell = div()
            .w(px(TREE_W))
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .gap(px(TEXT_GAP))
            .pl(px(gutter_x(depth) + GUTTER_W + TEXT_GAP))
            .pr(px(CELL_PAD_R))
            .text_size(px(TOOL_LABEL_SIZE))
            .line_height(px(TOOL_LABEL_LINE_HEIGHT))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .when(agent, |label| label.font_weight(FontWeight::MEDIUM))
                    .text_color(motion::mix(t.content_muted, t.text, lit))
                    .child(span.name.clone()),
            )
            .child(div().flex_1().min_w(px(4.)))
            .when(status == Status::Failed, |cell| cell.child(failed_word(t)))
            .when_some(detail, |cell, detail| cell.child(trailing_meta(detail, t)))
            // A running sub-agent shows pink's working-row spinner in the
            // trailing slot (pink: `chip_header_row` for a running child).
            .when(agent && status == Status::Running, |cell| {
                cell.child(div().flex_none().child(loaders::mini_glyph_spinner(
                    format!("{}-spin-{ix}", self.key),
                    2.,
                    t.glyph,
                )))
            })
            // The disclosure tile's slot is kept on every row so the
            // trailing column lines up down the list.
            .child(if has_kids {
                div()
                    .id(SharedString::from(format!("{row_key}-fold")))
                    .size(px(TILE))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(motion::lerp(0.5, 1., hover))
                    .cursor_pointer()
                    .on_click(self.toggle(ix))
                    .child(
                        svg()
                            .path(ICON_CHEVRON)
                            .size(px(CHEVRON))
                            .with_transformation(Transformation::rotate(radians(
                                FRAC_PI_2 * self.folds[ix].progress,
                            )))
                            .text_color(motion::mix(t.content_faint, t.text, hover)),
                    )
                    .into_any_element()
            } else {
                div().size(px(TILE)).flex_none().into_any_element()
            });
        if reveal < 1. {
            cell.relative().top(px(4. * (1. - reveal))).opacity(reveal)
        } else {
            cell
        }
    }

    /// The step's connector and glyph (pink: `transcript.rs` `activity_rail`):
    /// the trunk comes down from the row above, bends on a 6 px radius into
    /// the branch and reveals the glyph at its tip; a later sibling's arrival
    /// first carries the trunk on past this row. One nonzero path, so the
    /// joints never double their alpha. A sub-agent's glyph is pink's agent
    /// tile; a running step shows pink's item spinner in its glyph slot, a
    /// failure the danger circle.
    fn rail(&self, ix: usize, depth: usize, has_pred: bool, next: Option<usize>) -> Div {
        let t = self.t;
        let span = &self.spans[ix];
        let status = span.status(self.now);
        let color = t.hairline(RAIL_ALPHA);
        let (incoming, branch) = motion::tool_connector_parts(self.connector[ix], has_pred);
        let continues = next.is_some();
        let continuation = motion::tool_connector_continuation(next.map(|n| self.connector[n]));
        let glyph_top = ROW_H / 2. - ICON_SIZE / 2.;
        let glyph = if span.kind == SpanKind::Agent {
            div()
                .absolute()
                .left(px(TILE_CX - TILE / 2.))
                .top(px(ROW_H / 2. - TILE / 2.))
                .size(px(TILE))
                .rounded(px(radius::SM))
                .bg(t.ink(0.08))
                .flex()
                .items_center()
                .justify_center()
                .opacity(branch)
                .child(
                    svg()
                        .path(ICON_BOT)
                        .size(px(TILE_ICON))
                        .text_color(t.content_muted),
                )
                .into_any_element()
        } else if status == Status::Running {
            div()
                .absolute()
                .left(px(ICON_LEFT))
                .top(px(glyph_top))
                .size(px(ICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .opacity(branch)
                .child(loaders::mini_glyph_spinner(
                    format!("{}-spin-{ix}", self.key),
                    2.,
                    t.glyph,
                ))
                .into_any_element()
        } else {
            let (path, tint) = if status == Status::Failed {
                (ICON_DANGER, t.danger)
            } else {
                (span.kind.icon(), t.content_muted)
            };
            svg()
                .path(path)
                .absolute()
                .left(px(ICON_LEFT))
                .top(px(glyph_top))
                .size(px(ICON_SIZE))
                .opacity(branch)
                .text_color(tint)
                .into_any_element()
        };
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(gutter_x(depth)))
            .w(px(GUTTER_W))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let x = bounds.origin.x + px(TRUNK_X);
                        let bend_y = bounds.origin.y + px(ROW_H / 2. - BEND_RADIUS);
                        let mut tree = PathBuilder::fill().with_style(gpui::PathStyle::Fill(
                            gpui::FillOptions::default().with_fill_rule(gpui::FillRule::NonZero),
                        ));
                        if incoming > 0. {
                            let mut bottom = point(
                                x,
                                bounds.origin.y + px((ROW_H / 2. - BEND_RADIUS) * incoming),
                            );
                            if incoming >= 1. && continues && continuation > 0. {
                                let rest = (f32::from(bounds.size.height)
                                    - (ROW_H / 2. - BEND_RADIUS))
                                    .max(0.);
                                bottom = point(x, bend_y + px(rest * continuation));
                            }
                            ribbon(&mut tree, &[point(x, bounds.origin.y), bottom]);
                        }
                        if branch > 0. {
                            let points: Vec<_> = branch_points(branch)
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
            .child(glyph)
    }

    /// The step's bar on the time axis: pink's 6 pt usage-meter capsule,
    /// neutral when settled (pink's meter fill), `busy` at 55% while it runs
    /// (pink's Working status tone), danger when it failed and warning for the
    /// slowest step; a sub-agent's bar is the meter's track it brackets.
    /// The label is pink's timer type: 11 tabular `content_faint`.
    fn bar_cell(&self, ix: usize, height: f32, reveal: f32) -> Div {
        let t = self.t;
        let span = &self.spans[ix];
        let status = span.status(self.now);
        let slowest = self.slowest == Some(ix);
        let x0 = self.axis.x(span.start);
        let x1 = self.axis.x(span.end.min(self.now)).max(x0 + BAR_H);
        let color = match (span.kind, status) {
            (_, Status::Failed) => t.readable_tone(t.danger, 1.),
            (SpanKind::Agent, _) => t.fill(FillTier::Secondary),
            // pink's Working status tone (`shell/spaces.rs`: `busy@0.55`).
            (_, Status::Running) => t.busy.opacity(0.55),
            _ if slowest => t.readable_tone(t.warning, 1.),
            _ => t.text_muted,
        };
        let bar = div()
            .absolute()
            .left(px(x0))
            .top(px((height - BAR_H) / 2.))
            .w(px(x1 - x0))
            .h(px(BAR_H))
            .rounded_full()
            .bg(color);
        let label = self.labels[ix].as_ref().map(|label| {
            let right = x1 + LABEL_GAP + label.width <= self.col_w - CELL_PAD_R;
            let x = if right {
                x1 + LABEL_GAP
            } else {
                (x0 - LABEL_GAP - label.width).max(4.)
            };
            div()
                .absolute()
                .left(px(x))
                .top_0()
                .h_full()
                .flex()
                .items_center()
                .gap(px(LABEL_GAP))
                .whitespace_nowrap()
                .text_size(px(META_SIZE))
                .line_height(px(META_LINE))
                .font_features(pink::tabular_figures())
                .text_color(t.content_faint)
                .child(label.text.clone())
                .when(slowest, |label| {
                    label.child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.readable_tone(t.warning, 1.))
                            .child("Slowest"),
                    )
                })
        });
        div()
            .relative()
            .flex_1()
            .h_full()
            .when(reveal < 1., |cell| cell.opacity(reveal))
            .child(bar)
            .children(label)
    }
}

/// Pink's failure word (pink: `chip_header_row`): 11 medium, danger.
fn failed_word(t: &Pink) -> Div {
    div()
        .flex_none()
        .text_size(px(META_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.readable_tone(t.danger, 1.))
        .child("Failed")
}

/// The trailing model / tool slot (pink: `chip_header_row` model): bare
/// faint 11 text, not a pill.
fn trailing_meta(text: SharedString, t: &Pink) -> Div {
    div()
        .flex_none()
        .h(px(TOOL_LABEL_LINE_HEIGHT))
        .flex()
        .items_center()
        .text_size(px(META_SIZE))
        .text_color(t.content_faint)
        .child(text)
}

/// The elbow and branch, revealed by distance so the bend and the straight
/// leg draw at one speed (pink: `transcript.rs` `activity_branch_points`).
fn branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<_> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0;
            point(BEND_RADIUS * t * t, BEND_RADIUS * (2.0 * t - t * t))
        })
        .collect();
    path.push(point(BRANCH_END_X - TRUNK_X, BEND_RADIUS));
    if progress >= 1.0 {
        return path;
    }
    let lengths: Vec<_> = path
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .collect();
    let mut remaining = lengths.iter().sum::<f32>() * progress.clamp(0.0, 1.0);
    let mut visible = vec![path[0]];
    for (pair, length) in path.windows(2).zip(lengths) {
        if remaining <= 0.0 {
            break;
        }
        let t = (remaining / length).min(1.0);
        visible.push(point(
            motion::lerp(pair[0].x, pair[1].x, t),
            motion::lerp(pair[0].y, pair[1].y, t),
        ));
        remaining -= length;
    }
    visible
}

/// A 1 px ribbon along `points` as a clockwise contour; a nonzero fill
/// unions the contours (pink: `transcript.rs` `activity_ribbon`).
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

/// Opens or closes `ix`, turning from wherever its fold is now.
fn set_open(s: &mut TraceState, ix: usize, open: bool) {
    let was_open = !s.collapsed.contains(&ix);
    if was_open == open {
        return;
    }
    let clock = Instant::now();
    let from = fold_at(was_open, s.toggled.get(&ix).copied(), clock, false).progress;
    if open {
        s.collapsed.remove(&ix);
    } else {
        s.collapsed.insert(ix);
    }
    s.toggled.insert(ix, (clock, from));
}

#[allow(clippy::too_many_arguments)]
fn run(
    state: &Entity<TraceState>,
    spans: &[TraceSpan],
    order: &[(usize, usize)],
    now: f32,
    command: Command,
    on_select: &Option<SelectHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut picked = None;
    let has_kids = |ix: usize| {
        spans
            .iter()
            .enumerate()
            .any(|(c, s)| s.parent == Some(ix) && c != ix)
    };
    state.update(cx, |s, cx| {
        let hidden = |s: &TraceState, ix: usize| {
            let mut p = spans[ix].parent;
            let mut guard = 0;
            while let Some(parent) = p.filter(|p| *p < spans.len()) {
                if s.collapsed.contains(&parent) || guard > MAX_DEPTH {
                    return true;
                }
                p = spans[parent].parent;
                guard += 1;
            }
            false
        };
        match command {
            Command::Select(ix) => {
                s.selected = Some(ix);
                picked = Some(ix);
            }
            Command::Toggle(ix) => {
                let open = s.collapsed.contains(&ix);
                set_open(s, ix, open);
            }
            Command::Step(delta) => {
                let rows: Vec<usize> = order
                    .iter()
                    .map(|(ix, _)| *ix)
                    .filter(|ix| spans[*ix].start <= now && !hidden(s, *ix))
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
                    let open = !s.collapsed.contains(&sel);
                    if fold && has_kids(sel) && open {
                        set_open(s, sel, false);
                    } else if fold {
                        // Already folded, or a leaf: up to the parent.
                        if let Some(parent) = spans[sel].parent.filter(|p| *p < spans.len()) {
                            s.selected = Some(parent);
                            picked = s.selected;
                        }
                    } else {
                        set_open(s, sel, true);
                    }
                }
            }
            Command::Replay => {
                s.started = Instant::now();
                s.collapsed.clear();
                s.toggled.clear();
                s.hovered = None;
            }
        }
        cx.notify();
    });
    if let (Some(f), Some(ix)) = (on_select, picked) {
        f(ix, window, cx);
    }
}

/// The strip under the steps: the hovered or selected step in numbers, laid
/// out like pink's settings row (title over a muted line) and usage figures
/// (11 caption over a 13 value), with the step's share of the run on pink's
/// usage meter.
fn readout(
    key: &str,
    focus: Option<usize>,
    spans: &[TraceSpan],
    now: f32,
    total: f32,
    t: &Pink,
) -> AnyElement {
    let base = div()
        .w_full()
        .h(px(READOUT_H))
        .flex_none()
        .flex()
        .items_center()
        .px(px(layout::ROW_PAD_X))
        .border_t_1()
        .border_color(t.border);
    let Some(ix) = focus.filter(|ix| spans[*ix].start <= now) else {
        return base
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(t.readable_muted(0.85))
            .child("Hover or select a step to read its timing, tokens and cost.")
            .into_any_element();
    };
    let span = &spans[ix];
    let status = span.status(now);
    let (tin, tout, cost) = totals(spans, ix, now);
    let steps = descendants(spans, ix)
        .iter()
        .filter(|d| spans[**d].start <= now)
        .count();
    let end = span.end.min(now);
    let separator = || div().flex_none().text_color(t.content_faint).child("·");
    let field = |label: &'static str, value: String| {
        div()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(META_SIZE))
                    .line_height(px(META_LINE))
                    .text_color(t.readable_muted(0.85))
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .font_features(pink::tabular_figures())
                    .text_color(t.text)
                    .child(value),
            )
    };
    let state_word = match status {
        Status::Running => Some(("Running", t.content_muted)),
        Status::Failed => Some(("Failed", t.readable_tone(t.danger, 1.))),
        Status::Done => None,
    };
    let meta = div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(4.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(t.readable_muted(0.85))
        .child(div().flex_none().child(span.kind.label()))
        .when_some(span.detail.clone(), |meta, detail| {
            meta.child(separator())
                .child(div().min_w_0().truncate().child(detail))
        })
        .when(span.kind == SpanKind::Agent, |meta| {
            let plural = if steps == 1 { "" } else { "s" };
            meta.child(separator())
                .child(div().flex_none().child(format!("{steps} step{plural}")))
        })
        .when_some(state_word, |meta, (word, color)| {
            meta.child(separator()).child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(color)
                    .child(word),
            )
        });
    let run_len = total.max(0.001);
    let share = (end - span.start) / run_len;
    let seg_x = span.start / run_len * SHARE_W;
    let seg_w = (share * SHARE_W).max(BAR_H);
    let content = div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .gap(px(layout::ROW_PAD_X))
        .child(
            div()
                .w(px(220.))
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .truncate()
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(span.name.clone()),
                )
                .child(meta),
        )
        .child(field("Start", fmt_secs(span.start)))
        .child(field("Duration", fmt_secs(end - span.start)))
        .child(field(
            "Tokens in → out",
            if tin + tout > 0. {
                format!("{} → {}", fmt_tokens(tin), fmt_tokens(tout))
            } else {
                "—".into()
            },
        ))
        .child(field(
            "Cost",
            if cost > 0. {
                fmt_cost(cost)
            } else {
                "—".into()
            },
        ))
        .child(div().flex_1())
        .child(
            div()
                .w(px(SHARE_W))
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_size(px(META_SIZE))
                        .line_height(px(META_LINE))
                        .text_color(t.readable_muted(0.85))
                        .child("Share of run")
                        .child(
                            div()
                                .font_features(pink::tabular_figures())
                                .text_color(t.text)
                                .child(format!("{:.0}%", share * 100.)),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .w_full()
                        .h(px(BAR_H))
                        .rounded_full()
                        .bg(t.fill(FillTier::Secondary))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left(px(motion::follow(
                                    &format!("{key}-share-x"),
                                    seg_x,
                                    DISCLOSURE,
                                )))
                                .w(px(motion::follow(
                                    &format!("{key}-share-w"),
                                    seg_w,
                                    DISCLOSURE,
                                )))
                                .h_full()
                                .rounded_full()
                                .bg(if status == Status::Failed {
                                    t.readable_tone(t.danger, 1.)
                                } else {
                                    t.text_muted
                                }),
                        ),
                ),
        );
    base.child(motion::status_swap(
        SharedString::from(format!("{key}-readout-{ix}")),
        content,
    ))
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

    #[test]
    fn figures_read_like_pink() {
        assert_eq!(fmt_tokens(640.), "640");
        assert_eq!(fmt_tokens(2_520.), "2.52K");
        assert_eq!(fmt_tokens(32_130.), "32.1K");
        assert_eq!(fmt_cost(0.0421), "$0.042");
        assert_eq!(fmt_secs(83.), "1m 23s");
    }

    #[test]
    fn a_fold_turns_from_where_it_was() {
        let at = Instant::now();
        let settled = fold_at(true, None, at, false);
        assert_eq!(settled.progress, 1.);
        let mid = fold_at(false, Some((at, 0.6)), at, false);
        assert!(mid.moving && (mid.progress - 0.6).abs() < 1e-3);
    }
}
