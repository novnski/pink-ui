//! Agent Lanes – what every agent is doing, as columns of time.
//!
//! One column per agent; time runs down the page into a **now** line at the
//! bottom, so running work grows downward into the present while the past
//! scrolls up and away. Dashed connectors show hand-offs from one agent's
//! finished block to the next agent's work, and a block that waits on a
//! person carries its Approve / Deny right on it. The window switch (5, 15 or
//! 30 minutes) rescales the whole picture with a glide.
//!
//! ```ignore
//! AgentLanes::new("lanes")
//!     .agent(LaneAgent::new("Planner", "Breaks work down"))
//!     .agent(LaneAgent::new("Builder", "Writes the code"))
//!     .block(LaneBlock::new(0, "Scope the refund fix").span(-800.0, -560.0))
//!     .block(LaneBlock::new(1, "Patch partial refunds").span(-530.0, 40.0).handoff(0, "spec.md"))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnimationExt, AnyView, App, ElementId, FontWeight, Hsla, IntoElement, PathBuilder,
    Pixels, Point, SharedString, Task, Window, canvas, div, point, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, EASE_IN_OUT, GLIDE, MotionSpec},
    primitives::ring,
    story,
};

pub const NAME: &str = "Agent Lanes";
pub const DESCRIPTION: &str = "A live column of time for each agent – work grows down into now, hand-offs link one agent's output to the next, and approvals wait right on the block";

const ICON_LANES: &str = "icons/agent-lanes/lanes.svg";
const ICON_CHECK: &str = "icons/agent-lanes/check.svg";
const ICON_CLOSE: &str = "icons/agent-lanes/close.svg";
const ICON_ATTENTION: &str = "icons/agent-lanes/attention.svg";
const ICON_HANDOFF: &str = "icons/agent-lanes/handoff.svg";

const WIDTH: f32 = 780.0;
const GUTTER: f32 = 58.0;
const PAD_R: f32 = 10.0;
const BODY_H: f32 = 340.0;
/// The now line sits a little above the bottom, over a sliver of future.
const NOW_Y: f32 = BODY_H - 22.0;
const WINDOWS: [(f32, &str, f32); 3] = [
    (300.0, "5m", 60.0),
    (900.0, "15m", 300.0),
    (1800.0, "30m", 600.0),
];
/// Seconds of wall time at "now" when the component mounts (10:42:20).
const WALL_START: f32 = 10.0 * 3600.0 + 42.0 * 60.0 + 20.0;

/// An agent, one column.
#[derive(Clone, Debug)]
pub struct LaneAgent {
    pub name: SharedString,
    pub role: SharedString,
}

impl LaneAgent {
    pub fn new(name: impl Into<SharedString>, role: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            role: role.into(),
        }
    }
}

/// A block of work in a lane. Times are seconds relative to the moment the
/// component mounts (negative is the past).
#[derive(Clone, Debug)]
pub struct LaneBlock {
    pub lane: usize,
    pub title: SharedString,
    pub tool: Option<SharedString>,
    pub start: f32,
    /// `None` runs until something ends it.
    pub end: Option<f32>,
    /// Waits for a person to approve or deny.
    pub approval: bool,
    /// Demo: approves itself at this time unless someone answers first.
    pub auto_approve: Option<f32>,
    /// A block (by index) that starts when this approval is granted, and how long it runs.
    pub then: Option<(usize, f32)>,
    /// Starts only when another approval is granted.
    pub gated: bool,
    /// Picks up the output of another block (by index), with a label for what was handed over.
    pub handoff: Option<(usize, SharedString)>,
}

impl LaneBlock {
    pub fn new(lane: usize, title: impl Into<SharedString>) -> Self {
        Self {
            lane,
            title: title.into(),
            tool: None,
            start: 0.0,
            end: None,
            approval: false,
            auto_approve: None,
            then: None,
            gated: false,
            handoff: None,
        }
    }

    pub fn tool(mut self, tool: impl Into<SharedString>) -> Self {
        self.tool = Some(tool.into());
        self
    }

    /// Runs from `start` to `end`.
    pub fn span(mut self, start: f32, end: f32) -> Self {
        self.start = start;
        self.end = Some(end);
        self
    }

    /// Starts at `start` and keeps running.
    pub fn since(mut self, start: f32) -> Self {
        self.start = start;
        self.end = None;
        self
    }

    pub fn approval(mut self) -> Self {
        self.approval = true;
        self
    }

    pub fn auto_approve(mut self, at: f32) -> Self {
        self.auto_approve = Some(at);
        self
    }

    /// When approved, block `index` starts and runs for `seconds`.
    pub fn then(mut self, index: usize, seconds: f32) -> Self {
        self.then = Some((index, seconds));
        self
    }

    /// Hidden until an approval starts it (see [`LaneBlock::then`]).
    pub fn gated(mut self) -> Self {
        self.gated = true;
        self
    }

    pub fn handoff(mut self, from: usize, label: impl Into<SharedString>) -> Self {
        self.handoff = Some((from, label.into()));
        self
    }
}

type DecisionHandler = Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct AgentLanes {
    id: ElementId,
    agents: Vec<LaneAgent>,
    blocks: Vec<LaneBlock>,
    speed: f32,
    autoplay: Option<f32>,
    on_decide: Option<DecisionHandler>,
}

impl AgentLanes {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            agents: Vec::new(),
            blocks: Vec::new(),
            speed: 1.0,
            autoplay: None,
            on_decide: None,
        }
    }

    pub fn agent(mut self, agent: LaneAgent) -> Self {
        self.agents.push(agent);
        self
    }

    pub fn block(mut self, block: LaneBlock) -> Self {
        self.blocks.push(block);
        self
    }

    /// Simulated seconds per real second (1 is real time).
    pub fn speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.0);
        self
    }

    /// Demo: start over after `seconds` of simulated time.
    pub fn loop_after(mut self, seconds: f32) -> Self {
        self.autoplay = Some(seconds);
        self
    }

    /// Called with the block index and `true` for approve, `false` for deny.
    pub fn on_decide(
        mut self,
        handler: impl Fn(usize, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Hidden,
    Running,
    Done,
    Waiting,
    Approved,
    Denied,
}

#[derive(Clone)]
struct Live {
    block: LaneBlock,
    start: f32,
    end: Option<f32>,
    /// Some(true) approved, Some(false) denied.
    decision: Option<bool>,
    visible: bool,
}

impl Live {
    fn phase(&self, clock: f32) -> Phase {
        if !self.visible || self.start > clock {
            return Phase::Hidden;
        }
        if self.block.approval {
            return match self.decision {
                Some(true) => Phase::Approved,
                Some(false) => Phase::Denied,
                None => Phase::Waiting,
            };
        }
        match self.end {
            Some(end) if end <= clock => Phase::Done,
            _ => Phase::Running,
        }
    }

    /// Where the block ends on screen right now.
    fn shown_end(&self, clock: f32) -> f32 {
        self.end.map_or(clock, |e| e.min(clock))
    }
}

struct LanesState {
    agents: Vec<LaneAgent>,
    initial: Vec<LaneBlock>,
    blocks: Vec<Live>,
    clock: f32,
    speed: f32,
    loop_after: Option<f32>,
    generation: usize,
    window: usize,
    selected: Option<usize>,
    last_tick: Instant,
    on_decide: Option<DecisionHandler>,
    _ticker: Task<()>,
}

impl LanesState {
    fn new(config: &AgentLanes, cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                if this.update(cx, |state, cx| state.tick(cx)).is_err() {
                    break;
                }
            }
        });
        let mut state = Self {
            agents: config.agents.clone(),
            initial: config.blocks.clone(),
            blocks: Vec::new(),
            clock: 0.0,
            speed: config.speed,
            loop_after: config.autoplay,
            generation: 0,
            window: 1,
            selected: None,
            last_tick: Instant::now(),
            on_decide: None,
            _ticker: ticker,
        };
        state.load();
        state
    }

    fn load(&mut self) {
        self.clock = 0.0;
        self.blocks = self
            .initial
            .iter()
            .map(|block| Live {
                block: block.clone(),
                start: block.start,
                end: block.end,
                decision: None,
                visible: !block.gated,
            })
            .collect();
        self.selected = self.blocks.iter().position(|b| b.block.approval);
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let dt = now
            .saturating_duration_since(self.last_tick)
            .as_secs_f32()
            .min(0.25);
        self.last_tick = now;
        self.clock += dt * self.speed;
        let pending: Vec<usize> = self
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.decision.is_none() && b.block.auto_approve.is_some_and(|at| at <= self.clock)
            })
            .map(|(ix, _)| ix)
            .collect();
        for ix in pending {
            self.decide(ix, true);
        }
        if let Some(limit) = self.loop_after
            && self.clock >= limit
        {
            self.generation += 1;
            self.load();
        }
        cx.notify();
    }

    fn decide(&mut self, ix: usize, approve: bool) {
        let clock = self.clock;
        let Some(live) = self.blocks.get_mut(ix) else {
            return;
        };
        if live.decision.is_some() {
            return;
        }
        live.decision = Some(approve);
        live.end = Some(clock);
        if approve
            && let Some((next, seconds)) = live.block.then
            && let Some(follow) = self.blocks.get_mut(next)
        {
            follow.visible = true;
            follow.start = clock;
            follow.end = Some(clock + seconds);
        }
    }

    fn lane_state(&self, lane: usize) -> Phase {
        let mut state = Phase::Hidden;
        for live in self.blocks.iter().filter(|b| b.block.lane == lane) {
            match live.phase(self.clock) {
                Phase::Waiting => return Phase::Waiting,
                Phase::Running => state = Phase::Running,
                _ => {}
            }
        }
        state
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn wall(seconds: f32) -> String {
    let s = (WALL_START + seconds).rem_euclid(24.0 * 3600.0) as u32;
    format!("{}:{:02}", s / 3600, (s % 3600) / 60)
}

fn span_label(seconds: f32) -> String {
    let s = seconds.max(0.0) as u32;
    if s < 60 {
        format!("{s}s")
    } else {
        format!("{}m {:02}s", s / 60, s % 60)
    }
}

/// A stable hue per agent name, from luna's palette family.
fn agent_hue(name: &str, t: &Luna) -> Hsla {
    const HUES: [f32; 5] = [277.0, 200.0, 155.0, 60.0, 330.0];
    let ix = name
        .bytes()
        .fold(0usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize))
        % HUES.len();
    let (l, c) = if t.dark { (0.74, 0.13) } else { (0.56, 0.17) };
    luna::oklch(l, c, HUES[ix])
}

fn monogram(name: &str, size: f32, t: &Luna) -> gpui::Div {
    let hue = agent_hue(name, t);
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .bg(hue.opacity(if t.dark { 0.2 } else { 0.13 }))
        .text_size(px(size * 0.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(hue)
        .child(name.chars().next().unwrap_or('?').to_string())
}

fn pulse_dot(id: impl Into<ElementId>, color: Hsla, live: bool) -> gpui::AnyElement {
    let dot = div().size(px(6.)).flex_none().rounded_full().bg(color);
    if live {
        dot.with_animation(
            id,
            Animation::new(Duration::from_millis(1400))
                .repeat()
                .with_easing(EASE_IN_OUT.easing()),
            |el, t| el.opacity(0.35 + 0.65 * (1.0 - (t * 2.0 - 1.0).abs())),
        )
        .into_any_element()
    } else {
        dot.into_any_element()
    }
}

/// A hand-off connector in body coordinates.
struct Link {
    from: Point<Pixels>,
    to: Point<Pixels>,
    color: Hsla,
}

impl RenderOnce for AgentLanes {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "lanes".into()),
            cx,
            |_, cx| LanesState::new(&self, cx),
        );
        let on_decide = self.on_decide.clone();
        state.update(cx, |s, _| s.on_decide = on_decide);
        let s = state.read(cx);
        let clock = s.clock;
        let lanes = s.agents.len().max(1);
        let lane_w = (WIDTH - 2.0 - GUTTER - PAD_R) / lanes as f32;
        let (window_secs, _, grid_step) = WINDOWS[s.window];
        let pps = motion::follow(&format!("{key}-pps"), NOW_Y / window_secs, GLIDE);
        let y_of = |time: f32| NOW_Y - (clock - time) * pps;
        let lane_x = |lane: usize| GUTTER + lane as f32 * lane_w;

        // -- Header ---------------------------------------------------------------
        let working = (0..s.agents.len())
            .filter(|lane| s.lane_state(*lane) == Phase::Running)
            .count();
        let waiting = (0..s.agents.len())
            .filter(|lane| s.lane_state(*lane) == Phase::Waiting)
            .count();
        let summary = |color: Hsla, label: String| {
            div()
                .flex()
                .items_center()
                .gap(px(space::S6))
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_muted)
                .child(div().size(px(6.)).rounded_full().bg(color))
                .child(label)
        };
        let window_chips = WINDOWS.iter().enumerate().map(|(ix, (_, label, _))| {
            luna::chip(
                format!("{key}-window-{ix}"),
                s.window == ix,
                ControlSize::Sm,
                &t,
            )
            .child(*label)
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.window = ix;
                        cx.notify();
                    })
                }
            })
        });
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
                    .child(svg().path(ICON_LANES).size(px(16.)).text_color(t.text)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(text::CALLOUT))
                            .line_height(px(20.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Agent lanes"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S12))
                            .child(summary(t.accent, format!("{working} working")))
                            .when(waiting > 0, |el| {
                                el.child(summary(t.warning, format!("{waiting} waiting on you")))
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S4))
                    .p(px(2.))
                    .rounded(px(radius::MD))
                    .bg(t.fill(FillTier::Quaternary))
                    .children(window_chips),
            );

        // -- Lane headers -----------------------------------------------------------
        let lane_heads = s.agents.iter().enumerate().map(|(lane, agent)| {
            let phase = s.lane_state(lane);
            let (label, color) = match phase {
                Phase::Running => ("Working", t.accent),
                Phase::Waiting => ("Waiting on you", t.warning),
                _ => ("Idle", t.text_faint),
            };
            div()
                .w(px(lane_w))
                .flex_none()
                .flex()
                .items_center()
                .gap(px(space::S8))
                .px(px(space::S8))
                .child(monogram(&agent.name, 26.0, &t))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div().flex().child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(text::BODY))
                                    .line_height(px(18.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(agent.name.clone()),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(5.))
                                .text_size(px(text::CAPTION))
                                .line_height(px(14.))
                                .text_color(color)
                                .child(pulse_dot(
                                    ElementId::Name(format!("{key}-lane-dot-{lane}").into()),
                                    color,
                                    phase == Phase::Running || phase == Phase::Waiting,
                                ))
                                .child(label),
                        ),
                )
        });
        let lane_header = div()
            .flex()
            .items_center()
            .h(px(52.))
            .pl(px(GUTTER))
            .border_t_1()
            .border_b_1()
            .border_color(t.border)
            .children(lane_heads);

        // -- Body: grid, blocks, hand-offs, now -------------------------------------
        let wall_now = WALL_START + clock;
        let first_tick = ((wall_now - window_secs * 1.2) / grid_step).ceil() * grid_step;
        let mut grid = Vec::new();
        let mut tick_time = first_tick;
        while tick_time <= wall_now {
            let y = y_of(tick_time - WALL_START);
            if y > 8.0 && y < NOW_Y - 14.0 {
                grid.push((y, wall(tick_time - WALL_START)));
            }
            tick_time += grid_step;
        }
        let grid_labels = grid.iter().map(|(y, label)| {
            div()
                .absolute()
                .left_0()
                .w(px(GUTTER - 10.))
                .top(px(y - 8.))
                .h(px(16.))
                .flex()
                .items_center()
                .justify_end()
                .font_family(luna::MONO)
                .text_size(px(text::CAPTION))
                .text_color(t.text_faint)
                .child(label.clone())
        });

        let mut links = Vec::new();
        let mut link_labels = Vec::new();
        let mut block_els = Vec::new();
        for (ix, live) in s.blocks.iter().enumerate() {
            let phase = live.phase(clock);
            if phase == Phase::Hidden {
                continue;
            }
            let block = &live.block;
            let x = lane_x(block.lane) + 6.0;
            let w = lane_w - 12.0;
            let top = y_of(live.start);
            let bottom = y_of(live.shown_end(clock));
            let min_h = if phase == Phase::Waiting { 62.0 } else { 4.0 };
            let top = top.min(bottom - min_h);
            let h = bottom - top;
            if bottom < -4.0 {
                continue;
            }

            if let Some((from, label)) = &block.handoff
                && let Some(src) = s.blocks.get(*from)
                && src.phase(clock) != Phase::Hidden
            {
                let sx = lane_x(src.block.lane) + lane_w / 2.0;
                let sy = y_of(src.shown_end(clock));
                let tx = x + w / 2.0;
                links.push(Link {
                    from: point(px(sx), px(sy)),
                    to: point(px(tx), px(top)),
                    color: t.text_faint,
                });
                let mid_y = (sy + top) / 2.0;
                if mid_y > 12.0 && (top - sy) > 34.0 {
                    link_labels.push(
                        div()
                            .absolute()
                            .left(px((sx + tx) / 2.0 - 52.0))
                            .w(px(104.))
                            .top(px(mid_y - 10.))
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .h(px(20.))
                                    .px(px(6.))
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .rounded(px(radius::SM))
                                    .bg(t.surface_card)
                                    .border_1()
                                    .border_color(t.border)
                                    .font_family(luna::MONO)
                                    .text_size(px(10.5))
                                    .text_color(t.text_muted)
                                    .child(
                                        svg()
                                            .path(ICON_HANDOFF)
                                            .size(px(11.))
                                            .text_color(t.text_faint),
                                    )
                                    .child(label.clone()),
                            ),
                    );
                }
            }

            let (bg, border, title_color) = match phase {
                Phase::Running => (
                    t.accent.opacity(if t.dark { 0.16 } else { 0.08 }),
                    t.accent.opacity(0.45),
                    t.text,
                ),
                Phase::Waiting => (
                    t.warning.opacity(if t.dark { 0.14 } else { 0.09 }),
                    t.warning.opacity(0.55),
                    t.text,
                ),
                Phase::Approved => (
                    t.success.opacity(if t.dark { 0.12 } else { 0.07 }),
                    t.success.opacity(0.35),
                    t.text_muted,
                ),
                Phase::Denied => (
                    t.danger.opacity(if t.dark { 0.12 } else { 0.07 }),
                    t.danger.opacity(0.35),
                    t.text_muted,
                ),
                _ => (t.fill(FillTier::Tertiary), t.border, t.text_muted),
            };
            let selected = s.selected == Some(ix);
            let block_key = format!("{key}-block-{ix}");
            let icon = match phase {
                Phase::Waiting => Some((ICON_ATTENTION, t.warning)),
                Phase::Approved => Some((ICON_CHECK, t.success)),
                Phase::Denied => Some((ICON_CLOSE, t.danger)),
                _ => None,
            };
            // Keep the label in view when the block starts above the window.
            let hidden_top = (-top).max(0.0);
            let visible_h = h - hidden_top;
            let title_row = (visible_h >= 20.0).then(|| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(5.))
                    .when_some(icon, |el, (path, color)| {
                        el.child(svg().path(path).size(px(12.)).flex_none().text_color(color))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(title_color)
                            .child(block.title.clone()),
                    )
            });
            let sub = match phase {
                Phase::Approved => Some("Approved".into()),
                Phase::Denied => Some("Denied".into()),
                Phase::Waiting => Some(format!("waiting {}", span_label(clock - live.start))),
                _ => block.tool.as_ref().map(|tool| tool.to_string()),
            };
            let sub_row = (visible_h >= 38.0).then_some(sub).flatten().map(|sub| {
                div().flex().child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(luna::MONO)
                        .text_size(px(10.5))
                        .line_height(px(14.))
                        .text_color(t.text_faint)
                        .child(sub),
                )
            });
            let actions = (phase == Phase::Waiting && visible_h >= 60.0).then(|| {
                let decide = |approve: bool| {
                    let state = state.clone();
                    move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut App| {
                        let handler = state.update(cx, |s, cx| {
                            s.loop_after = None;
                            s.decide(ix, approve);
                            s.selected = Some(ix);
                            cx.notify();
                            s.on_decide.clone()
                        });
                        if let Some(handler) = handler {
                            handler(ix, approve, window, cx);
                        }
                    }
                };
                div()
                    .mt(px(6.))
                    .flex()
                    .gap(px(4.))
                    .child(
                        luna::button(
                            format!("{block_key}-approve"),
                            ButtonKind::Accent,
                            ControlSize::Sm,
                            &t,
                        )
                        .h(px(22.))
                        .child("Approve")
                        .on_click(decide(true)),
                    )
                    .child(
                        luna::button(
                            format!("{block_key}-deny"),
                            ButtonKind::Secondary,
                            ControlSize::Sm,
                            &t,
                        )
                        .h(px(22.))
                        .child("Deny")
                        .on_click(decide(false)),
                    )
            });
            block_els.push(
                div()
                    .id(ElementId::Name(block_key.clone().into()))
                    .absolute()
                    .left(px(x))
                    .top(px(top))
                    .w(px(w))
                    .h(px(h))
                    .overflow_hidden()
                    .rounded(px(if h < 12.0 { 3.0 } else { radius::MD }))
                    .bg(bg)
                    .border_1()
                    .when(selected, |el| el.shadow(vec![ring(1.5, t.focus_ring())]))
                    .hover_border(block_key.clone(), border, title_color.opacity(0.6))
                    .cursor_pointer()
                    .px(px(space::S8))
                    .pt(px(5. + hidden_top))
                    .on_click({
                        let state = state.clone();
                        move |_, _, cx| {
                            state.update(cx, |s, cx| {
                                s.selected = Some(ix);
                                cx.notify();
                            })
                        }
                    })
                    .children(title_row)
                    .children(sub_row)
                    .children(actions)
                    // The growing edge of running work.
                    .when(phase == Phase::Running, |el| {
                        el.child(
                            div()
                                .absolute()
                                .left_0()
                                .right_0()
                                .bottom_0()
                                .h(px(2.))
                                .bg(t.accent),
                        )
                    })
                    .with_animation(
                        ElementId::Name(format!("{block_key}-in-{}", s.generation).into()),
                        MotionSpec::new(360, motion::EASE).animation(),
                        |el, p| el.opacity(p),
                    ),
            );
        }

        let lane_lines: Vec<f32> = (1..lanes).map(lane_x).collect();
        let grid_ys: Vec<f32> = grid.iter().map(|(y, _)| *y).collect();
        let (line_color, now_color) = (t.border, t.accent);
        let overlay = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let o = bounds.origin;
                let at = |x: f32, y: f32| point(o.x + px(x), o.y + px(y));
                for x in &lane_lines {
                    let mut line = PathBuilder::stroke(px(1.));
                    line.move_to(at(*x, 0.0));
                    line.line_to(at(*x, BODY_H));
                    if let Ok(path) = line.build() {
                        window.paint_path(path, line_color);
                    }
                }
                for y in &grid_ys {
                    let mut line = PathBuilder::stroke(px(1.)).dash_array(&[px(2.), px(4.)]);
                    line.move_to(at(GUTTER, *y));
                    line.line_to(at(WIDTH - 2.0 - PAD_R, *y));
                    if let Ok(path) = line.build() {
                        window.paint_path(path, line_color);
                    }
                }
                for link in &links {
                    let (from, to) = (link.from + o, link.to + o);
                    let dy = px(f32::from(to.y - from.y).max(24.0) * 0.5);
                    let mut curve = PathBuilder::stroke(px(1.25)).dash_array(&[px(3.), px(3.)]);
                    curve.move_to(from);
                    curve.cubic_bezier_to(
                        to - point(px(0.), px(4.)),
                        from + point(px(0.), dy),
                        to - point(px(0.), dy),
                    );
                    if let Ok(path) = curve.build() {
                        window.paint_path(path, link.color);
                    }
                    let mut head = PathBuilder::fill();
                    head.move_to(to);
                    head.line_to(to + point(px(-3.5), px(-5.)));
                    head.line_to(to + point(px(3.5), px(-5.)));
                    head.close();
                    if let Ok(path) = head.build() {
                        window.paint_path(path, link.color);
                    }
                    let mut dot = PathBuilder::fill();
                    let r = px(2.5);
                    dot.move_to(from + point(r, px(0.)));
                    dot.arc_to(point(r, r), px(0.), false, true, from - point(r, px(0.)));
                    dot.arc_to(point(r, r), px(0.), false, true, from + point(r, px(0.)));
                    if let Ok(path) = dot.build() {
                        window.paint_path(path, link.color);
                    }
                }
                let mut now = PathBuilder::stroke(px(1.));
                now.move_to(at(GUTTER - 4.0, NOW_Y));
                now.line_to(at(WIDTH - 2.0 - PAD_R, NOW_Y));
                if let Ok(path) = now.build() {
                    window.paint_path(path, now_color);
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let now_label = div()
            .absolute()
            .left(px(6.))
            .top(px(NOW_Y - 9.))
            .h(px(18.))
            .px(px(6.))
            .flex()
            .items_center()
            .rounded(px(radius::SM))
            .bg(t.accent_strong)
            .font_family(luna::MONO)
            .text_size(px(10.5))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.on_accent)
            .child(wall(clock));
        let future = div()
            .absolute()
            .left(px(GUTTER))
            .right(px(PAD_R))
            .top(px(NOW_Y))
            .bottom_0()
            .bg(t.fill(FillTier::Quaternary));

        let body = div()
            .relative()
            .h(px(BODY_H))
            .overflow_hidden()
            .child(future)
            .child(overlay)
            .children(grid_labels)
            .children(block_els)
            .children(link_labels)
            .child(now_label);

        // -- Footer: the selected block ----------------------------------------------
        let footer = s.selected.and_then(|ix| s.blocks.get(ix)).map(|live| {
            let phase = live.phase(clock);
            let agent = s
                .agents
                .get(live.block.lane)
                .map(|a| a.name.clone())
                .unwrap_or_default();
            let (label, tone) = match phase {
                Phase::Running => ("Running", Tone::Accent),
                Phase::Waiting => ("Waiting on you", Tone::Warning),
                Phase::Approved => ("Approved", Tone::Success),
                Phase::Denied => ("Denied", Tone::Danger),
                Phase::Done => ("Done", Tone::Neutral),
                Phase::Hidden => ("Scheduled", Tone::Neutral),
            };
            div()
                .flex()
                .items_center()
                .gap(px(space::S10))
                .px(px(space::S16))
                .h(px(48.))
                .border_t_1()
                .border_color(t.border)
                .text_size(px(text::BODY))
                .child(monogram(&agent, 22.0, &t))
                .child(div().text_color(t.text_muted).child(agent.clone()))
                .child(div().text_color(t.text_faint).child("·"))
                .child(
                    div().flex_1().min_w_0().flex().child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(live.block.title.clone()),
                    ),
                )
                .when_some(live.block.tool.clone(), |el, tool| {
                    el.child(
                        div()
                            .h(px(20.))
                            .px(px(6.))
                            .flex()
                            .items_center()
                            .rounded(px(radius::XS))
                            .bg(t.fill(FillTier::Tertiary))
                            .font_family(luna::MONO)
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_muted)
                            .child(tool),
                    )
                })
                .child(
                    div()
                        .font_family(luna::MONO)
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child(format!(
                            "{} – {}",
                            wall(live.start),
                            if matches!(phase, Phase::Running | Phase::Waiting) {
                                "now".to_string()
                            } else {
                                wall(live.shown_end(clock))
                            }
                        )),
                )
                .child(luna::badge(label, tone, &t))
        });

        motion::keep_alive(window);
        luna::card(&t)
            .w(px(WIDTH))
            .overflow_hidden()
            .font_family(luna::SANS)
            .text_color(t.text)
            .child(header)
            .child(lane_header)
            .child(body)
            .children(footer)
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        AgentLanes::new("agent-lanes")
            .agent(LaneAgent::new("Planner", "Breaks work down"))
            .agent(LaneAgent::new("Builder", "Writes the code"))
            .agent(LaneAgent::new("Checker", "Tests and reviews"))
            .agent(LaneAgent::new("Shipper", "Deploys"))
            // 0
            .block(
                LaneBlock::new(2, "Review PR #418")
                    .tool("git.diff")
                    .span(-860.0, -640.0),
            )
            // 1
            .block(
                LaneBlock::new(3, "Roll out search v7")
                    .tool("deploy.canary")
                    .span(-760.0, -470.0),
            )
            // 2
            .block(
                LaneBlock::new(0, "Scope the refund fix")
                    .tool("issues.read")
                    .span(-800.0, -560.0),
            )
            // 3
            .block(
                LaneBlock::new(1, "Patch partial refunds")
                    .tool("repo.edit")
                    .span(-530.0, -170.0)
                    .handoff(2, "spec.md"),
            )
            // 4
            .block(
                LaneBlock::new(2, "Run 1,284 tests")
                    .tool("ci.run")
                    .span(-150.0, 45.0)
                    .handoff(3, "PR #431"),
            )
            // 5
            .block(
                LaneBlock::new(0, "Triage six bug reports")
                    .tool("issues.read")
                    .span(-330.0, -80.0),
            )
            // 6
            .block(
                LaneBlock::new(1, "Fix CSV export encoding")
                    .tool("repo.edit")
                    .span(-60.0, 150.0)
                    .handoff(5, "csv-bug.md"),
            )
            // 7
            .block(
                LaneBlock::new(3, "Approve prod migration")
                    .since(-230.0)
                    .approval()
                    .auto_approve(70.0)
                    .then(8, 120.0),
            )
            // 8
            .block(
                LaneBlock::new(3, "Migrate prod database")
                    .tool("db.migrate")
                    .gated(),
            )
            // 9
            .block(
                LaneBlock::new(0, "Draft release notes")
                    .tool("docs.write")
                    .span(20.0, 240.0),
            )
            // 10
            .block(
                LaneBlock::new(2, "Review the CSV fix")
                    .tool("git.diff")
                    .span(160.0, 280.0)
                    .handoff(6, "PR #432"),
            )
            // 11
            .block(
                LaneBlock::new(3, "Ship refunds to staging")
                    .tool("deploy.staging")
                    .span(215.0, 330.0)
                    .handoff(4, "build 7f3c"),
            )
            // 12
            .block(
                LaneBlock::new(1, "Refactor the refund ledger")
                    .tool("repo.edit")
                    .span(170.0, 320.0),
            )
            .speed(8.0)
            .loop_after(360.0)
    })
}
