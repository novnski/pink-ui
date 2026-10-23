//! Agent Lanes – what every agent is doing, as columns of time.
//!
//! One lane per agent, drawn as Pink's board columns (`shell/board`): a quiet
//! well headed by the agent's name, role and status. Time runs down each lane
//! into a **now** line near the bottom, so running work grows into the present
//! while the past rises and fades out under the lane heads. Each block of work
//! is a board card sized by its time; hand-offs are Pink's activity-rail
//! ribbon (1 px, rounded elbows) from one agent's finished card to the next
//! agent's, and a card that waits on a person carries its Approve / Deny. The
//! window switch (5, 15 or 30 minutes) is a segmented control and rescales the
//! whole picture over `DISCLOSURE`.
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
    AnimationExt, AnyElement, AnyView, App, Bounds, ElementId, FillOptions, FillRule, FontWeight,
    Hsla, IntoElement, PathBuilder, PathStyle, Pixels, Point, Role, SharedString, Task, Window,
    canvas, div, fill, linear_color_stop, linear_gradient, point, prelude::*, px, size, svg,
};

use crate::{
    HoverFill,
    motion::{self, DISCLOSURE, TOOL_ROW_REVEAL},
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, Segment, loaders, radius, tabular_figures,
    },
    story,
};

pub const NAME: &str = "Agent Lanes";
pub const DESCRIPTION: &str = "Every agent's work as a board column of time – cards grow down into a now line, hand-offs step across lanes on a hairline rail, a 5, 15 or 30 minute switch rescales it all, and an approval waits on its card with Approve and Deny";

const ICON_SHIELD: &str = "icons/agent-lanes/shield-warning.svg";
const ICON_CHECK: &str = "icons/agent-lanes/check.svg";
const ICON_CLOSE: &str = "icons/agent-lanes/close.svg";

const WIDTH: f32 = 780.0;
/// Side and bottom inset of the lanes area.
const INSET: f32 = 12.0;
/// The time labels' column, left of the first lane.
const GUTTER: f32 = 48.0;
/// Between two lane wells (Pink board columns: `gap 12`).
const LANE_GAP: f32 = 12.0;
/// Around the cards inside a well (Pink board column: `p 8`).
const WELL_PAD: f32 = 8.0;
/// Card padding (Pink board card: `px 10`).
const CARD_PAD_X: f32 = 10.0;
const BODY_H: f32 = 320.0;
/// The now line sits a little above the bottom, over a sliver of future.
const NOW_Y: f32 = BODY_H - 24.0;
/// The past fades out under the lane heads (Pink's results fade band).
const FADE_BAND: f32 = 18.0;
/// Hand-off elbow radius (Pink `ACTIVITY_BEND_RADIUS`).
const BEND: f32 = 6.0;
/// A waiting card is always tall enough for its title, wait and buttons.
const WAITING_MIN_H: f32 = 80.0;
const WINDOWS: [(f32, &str, f32); 3] = [
    (300.0, "5m", 60.0),
    (900.0, "15m", 300.0),
    (1800.0, "30m", 600.0),
];
/// Seconds of wall time at "now" when the component mounts (10:42:20).
const WALL_START: f32 = 10.0 * 3600.0 + 42.0 * 60.0 + 20.0;

/// An agent, one lane.
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

    /// What a lane's agent is doing now: waiting beats working beats idle.
    fn lane_state(&self, lane: usize) -> Status {
        let mut state = Status::Idle;
        for live in self.blocks.iter().filter(|b| b.block.lane == lane) {
            match live.phase(self.clock) {
                Phase::Waiting => return Status::Waiting,
                Phase::Running => state = Status::Working,
                _ => {}
            }
        }
        state
    }
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------

fn wall(seconds: f32) -> String {
    let s = (WALL_START + seconds).rem_euclid(24.0 * 3600.0) as u32;
    format!("{}:{:02}", s / 3600, (s % 3600) / 60)
}

/// A status as a badge shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Working,
    Waiting,
    Approved,
    Denied,
    Done,
    Idle,
    Scheduled,
}

impl Status {
    fn of(phase: Phase) -> Self {
        match phase {
            Phase::Running => Status::Working,
            Phase::Waiting => Status::Waiting,
            Phase::Approved => Status::Approved,
            Phase::Denied => Status::Denied,
            Phase::Done => Status::Done,
            Phase::Hidden => Status::Scheduled,
        }
    }
}

/// One status badge, shape before color: running work gets Pink's mini
/// spinner and a medium `content_muted` word, finished work a 6 px dot (Pink:
/// transcript.rs subagent status badge); a decision gets its glyph and word
/// in the tone (Pink: board/card.rs `badge`); waiting on a person is Pink's
/// sidebar status pill with the approval shield (Pink: shell.rs status pill).
fn status_badge(status: Status, key: &str, t: &Pink) -> AnyElement {
    let row = |color: Hsla, gap: f32| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(gap))
            .text_size(px(11.))
            .line_height(px(14.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(color)
    };
    let dot = |color: Hsla| div().size(px(6.)).flex_none().rounded_full().bg(color);
    let glyph = |path: &'static str, color: Hsla| {
        svg().path(path).size(px(12.)).flex_none().text_color(color)
    };
    match status {
        Status::Working => row(t.content_muted, 6.)
            .child(loaders::mini_glyph_spinner(
                format!("{key}-spin"),
                2.0,
                t.glyph,
            ))
            .child("Working")
            .into_any_element(),
        Status::Waiting => {
            pink::status_pill("Waiting", t.warning, Some(ICON_SHIELD.into()), t).into_any_element()
        }
        Status::Approved => {
            let tone = t.readable_tone(t.success, 0.95);
            row(tone, 4.)
                .child(glyph(ICON_CHECK, tone))
                .child("Approved")
                .into_any_element()
        }
        Status::Denied => {
            let tone = t.readable_tone(t.danger, 0.95);
            row(tone, 4.)
                .child(glyph(ICON_CLOSE, tone))
                .child("Denied")
                .into_any_element()
        }
        Status::Done => row(t.content_muted, 6.)
            .child(dot(t.readable_tone(t.success, 1.0)))
            .child("Done")
            .into_any_element(),
        Status::Idle => row(t.content_muted, 6.)
            .child(dot(t.content_faint))
            .child("Idle")
            .into_any_element(),
        // A ring, never a filled dot: nothing has run yet (Pink board).
        Status::Scheduled => row(t.readable_muted(0.9), 6.)
            .child(
                div()
                    .size(px(8.))
                    .flex_none()
                    .rounded_full()
                    .border(px(1.5))
                    .border_color(t.readable_muted(0.8)),
            )
            .child("Scheduled")
            .into_any_element(),
    }
}

/// A lane head (Pink: board/columns.rs `column_header`): the name 12/16
/// semibold `text` with the agent's status on the right, the role 11/14 in
/// `readable_muted(0.8)` under it, gap 2.
fn lane_head(agent: &LaneAgent, status: Status, width: f32, key: &str, t: &Pink) -> gpui::Div {
    div()
        .w(px(width))
        .flex_none()
        .px(px(WELL_PAD + 2.))
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .h(px(16.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(agent.name.clone()),
                )
                .child(status_badge(status, key, t)),
        )
        .child(
            div().flex().child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .text_color(t.readable_muted(0.8))
                    .child(agent.role.clone()),
            ),
        )
}

/// A clockwise ribbon contour, 1 px wide (Pink: transcript.rs
/// `activity_ribbon`). Nonzero fill unions crossing ribbons so a crossing
/// never paints twice.
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

/// A hand-off's course: down from the source card, across with a rounded
/// elbow (Pink's activity-rail bend, a quadratic sampled 12 times), and down
/// into the target card.
fn elbow(from: (f32, f32), to: (f32, f32)) -> Vec<(f32, f32)> {
    let ((sx, sy), (tx, ty)) = (from, to);
    let dx = tx - sx;
    if dx.abs() < 0.5 {
        return vec![from, to];
    }
    let my = (sy + ty) / 2.0;
    let r = BEND.min((ty - sy) / 2.0).min(dx.abs() / 2.0).max(0.0);
    let dir = dx.signum();
    let quad = |a: (f32, f32), c: (f32, f32), b: (f32, f32), t: f32| {
        let u = 1.0 - t;
        (
            u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
            u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
        )
    };
    let mut points = vec![from];
    for step in 0..=12 {
        let t = step as f32 / 12.0;
        points.push(quad((sx, my - r), (sx, my), (sx + dir * r, my), t));
    }
    for step in 0..=12 {
        let t = step as f32 / 12.0;
        points.push(quad((tx - dir * r, my), (tx, my), (tx, my + r), t));
    }
    points.push(to);
    points
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl RenderOnce for AgentLanes {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
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
        let area_w = WIDTH - 2.0 - 2.0 * INSET - GUTTER;
        let lane_w = (area_w - LANE_GAP * (lanes - 1) as f32) / lanes as f32;
        let lanes_right = GUTTER + area_w;
        let (window_secs, _, grid_step) = WINDOWS[s.window];
        let pps = motion::follow(&format!("{key}-pps"), NOW_Y / window_secs, DISCLOSURE);
        let y_of = |time: f32| NOW_Y - (clock - time) * pps;
        let lane_x = |lane: usize| GUTTER + lane as f32 * (lane_w + LANE_GAP);
        let card_bg = t.card_glass_bg();
        let well = t.fill(FillTier::Quaternary).opacity(0.5);
        // The well as painted: cards and the fade band sit on it opaquely.
        let plate = pink::flatten(well, card_bg);

        // -- Header: Pink's section header with the window switch -------------------
        let statuses: Vec<Status> = (0..s.agents.len()).map(|l| s.lane_state(l)).collect();
        let working = statuses.iter().filter(|s| **s == Status::Working).count();
        let waiting = statuses.iter().filter(|s| **s == Status::Waiting).count();
        let summary = match (working, waiting) {
            (0, 0) => "Every agent is idle".to_string(),
            (w, 0) => format!("{w} working"),
            (0, n) => format!("{n} waiting on you"),
            (w, n) => format!("{w} working · {n} waiting on you"),
        };
        let switch = pink::segmented(
            format!("{key}-window"),
            WINDOWS
                .iter()
                .map(|(_, label, _)| Segment::new(*label, *label))
                .collect(),
            s.window,
            132.0,
            ControlSize::Sm,
            &t,
            {
                let state = state.clone();
                move |ix, _, cx| {
                    let ix = *ix;
                    state.update(cx, |s, cx| {
                        s.window = ix;
                        cx.notify();
                    })
                }
            },
        );
        let header = pink::section_header("Agent lanes", Some(summary.into()), &t)
            .child(pink::row_controls().child(switch));

        // -- Lane wells and heads (Pink board columns) -------------------------------
        let wells = (0..lanes).map(|lane| {
            div()
                .absolute()
                .left(px(INSET + lane_x(lane)))
                .top_0()
                .bottom(px(INSET))
                .w(px(lane_w))
                .rounded(px(radius::LG))
                .bg(well)
        });
        let heads = s.agents.iter().enumerate().map(|(lane, agent)| {
            lane_head(
                agent,
                statuses[lane],
                lane_w,
                &format!("{key}-lane-{lane}"),
                &t,
            )
        });
        let head_row = div()
            .relative()
            .flex()
            .pl(px(GUTTER))
            .pt(px(WELL_PAD))
            .gap(px(LANE_GAP))
            .children(heads);

        // -- Body: grid, cards, hand-offs, now ---------------------------------------
        let wall_now = WALL_START + clock;
        let first_tick = ((wall_now - window_secs * 1.2) / grid_step).ceil() * grid_step;
        let mut grid = Vec::new();
        let mut tick_time = first_tick;
        while tick_time <= wall_now {
            let y = y_of(tick_time - WALL_START);
            if y > FADE_BAND && y < NOW_Y - 16.0 {
                grid.push((y, wall(tick_time - WALL_START)));
            }
            tick_time += grid_step;
        }
        // Wall times in the gutter: 11 px tabular `content_faint`, as Pink sets
        // its timers.
        let grid_labels = grid.iter().map(|(y, label)| {
            div()
                .absolute()
                .left_0()
                .w(px(GUTTER - 8.))
                .top(px(y - 7.))
                .h(px(14.))
                .flex()
                .items_center()
                .justify_end()
                .text_size(px(11.))
                .line_height(px(14.))
                .font_features(tabular_figures())
                .text_color(t.content_faint)
                .child(label.clone())
        });

        let mut links = Vec::new();
        let mut link_labels = Vec::new();
        let mut cards = Vec::new();
        for (ix, live) in s.blocks.iter().enumerate() {
            let phase = live.phase(clock);
            if phase == Phase::Hidden {
                continue;
            }
            let block = &live.block;
            let x = lane_x(block.lane) + WELL_PAD;
            let w = lane_w - 2.0 * WELL_PAD;
            let bottom = y_of(live.shown_end(clock));
            let min_h = if phase == Phase::Waiting {
                WAITING_MIN_H
            } else {
                4.0
            };
            let top = y_of(live.start).min(bottom - min_h);
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
                if top - sy >= 1.0 {
                    links.push(elbow((sx, sy), (tx, top)));
                    let mid_y = (sy + top) / 2.0;
                    if top - sy >= 28.0 && mid_y > FADE_BAND + 8.0 {
                        // What was handed over, on the rail's crossing: a
                        // neutral status pill on an opaque plate so the rail
                        // stops at its edges.
                        link_labels.push(
                            div()
                                .absolute()
                                .left(px((sx + tx) / 2.0 - 60.0))
                                .w(px(120.))
                                .top(px(mid_y - 8.))
                                .flex()
                                .justify_center()
                                .child(div().rounded(px(radius::XS)).bg(card_bg).child(
                                    pink::status_pill(label.clone(), t.text_muted, None, &t),
                                )),
                        );
                    }
                }
            }
            cards.push(block_card(
                &state,
                ix,
                live,
                phase,
                s.selected == Some(ix),
                Bounds::new(point(x, top), size(w, h)),
                plate,
                clock,
                &key,
                s.generation,
                &t,
            ));
        }

        let grid_ys: Vec<f32> = grid.iter().map(|(y, _)| *y).collect();
        let (grid_color, rail_color) = (t.hairline(0.05), t.hairline(0.16));
        let lines = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let o = bounds.origin;
                for y in &grid_ys {
                    window.paint_quad(fill(
                        Bounds::new(
                            point(o.x + px(GUTTER), o.y + px(y.round())),
                            size(px(lanes_right - GUTTER), px(1.)),
                        ),
                        grid_color,
                    ));
                }
                if links.is_empty() {
                    return;
                }
                // Pink: transcript.rs activity_rail – all ribbons in one
                // nonzero path, painted once in `hairline`.
                let mut tree = PathBuilder::fill().with_style(PathStyle::Fill(
                    FillOptions::default().with_fill_rule(FillRule::NonZero),
                ));
                for link in &links {
                    let points: Vec<Point<Pixels>> = link
                        .iter()
                        .map(|(x, y)| point(o.x + px(*x), o.y + px(*y)))
                        .collect();
                    ribbon(&mut tree, &points);
                }
                if let Ok(path) = tree.build() {
                    window.paint_path(path, rail_color);
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        // The past fades out under the lane heads: a band of the well's own
        // (flattened) color at the top of each lane.
        let fades = (0..lanes).map(|lane| {
            div()
                .absolute()
                .left(px(lane_x(lane)))
                .top_0()
                .w(px(lane_w))
                .h(px(FADE_BAND))
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(plate, 0.),
                    linear_color_stop(plate.opacity(0.), 1.),
                ))
        });
        // Now: a 1 px accent line (time's caret) and the wall time on an
        // accent status pill in the gutter.
        let now_line = div()
            .absolute()
            .left(px(GUTTER - 4.))
            .top(px(NOW_Y))
            .w(px(lanes_right - GUTTER + 4.))
            .h(px(1.))
            .bg(t.accent);
        let now_label = div()
            .absolute()
            .left_0()
            .w(px(GUTTER - 4.))
            .top(px(NOW_Y - 7.5))
            .flex()
            .justify_end()
            .child(
                pink::status_pill(wall(clock), t.accent, None, &t).font_features(tabular_figures()),
            );

        let body = div()
            .relative()
            .mt(px(WELL_PAD))
            .h(px(BODY_H))
            .overflow_hidden()
            .child(lines)
            .children(grid_labels)
            .children(cards)
            .children(link_labels)
            .children(fades)
            .child(now_line)
            .child(now_label);

        let lanes_area = div()
            .relative()
            .px(px(INSET))
            .pb(px(INSET))
            .children(wells)
            .child(head_row)
            .child(body);

        // -- Footer: the selected card, as a settings row -----------------------------
        let footer = s
            .selected
            .and_then(|ix| s.blocks.get(ix).map(|l| (ix, l)))
            .map(|(ix, live)| {
                let phase = live.phase(clock);
                let agent = s
                    .agents
                    .get(live.block.lane)
                    .map(|a| a.name.to_string())
                    .unwrap_or_default();
                let mut meta = vec![agent];
                if let Some(tool) = &live.block.tool {
                    meta.push(tool.to_string());
                }
                if let Some((from, label)) = &live.block.handoff
                    && let Some(src) = s.blocks.get(*from)
                    && let Some(source) = s.agents.get(src.block.lane)
                {
                    meta.push(format!("{label} from {}", source.name));
                }
                if phase != Phase::Hidden {
                    meta.push(format!(
                        "{} – {}",
                        wall(live.start),
                        if matches!(phase, Phase::Running | Phase::Waiting) {
                            "now".to_string()
                        } else {
                            wall(live.shown_end(clock))
                        }
                    ));
                }
                pink::card_row(false, &t)
                    .child(motion::status_swap(
                        ElementId::Name(format!("{key}-detail-{ix}-{}", s.generation).into()),
                        pink::row_text(live.block.title.clone(), Some(meta.join(" · ").into()), &t),
                    ))
                    .child(pink::row_controls().child(status_badge(
                        Status::of(phase),
                        &format!("{key}-detail"),
                        &t,
                    )))
            });

        motion::keep_alive(window);
        pink::card(&t)
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label("Agent lanes")
            .w(px(WIDTH))
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(header)
            .child(lanes_area)
            .children(footer)
    }
}

/// One block of work: Pink's board card (board/card.rs `render_card`) sized
/// by its time – radius 10 (less when thin), a `separator` edge over
/// `fill(Quaternary)`, the edge easing to `accent @ 0.45` on hover; selected
/// is `accent_wash` inside an `accent @ 0.7` edge. Finished work recedes to
/// the quieter `separator(false)` edge with its title in `content_muted`
/// until hovered. A card waiting on a person is Pink's notice wash
/// (`warning @ 0.08` inside `warning @ 0.24`, transcript.rs notice) with the
/// shield, its wait and Approve / Deny. Fills are flattened onto the well
/// (`under`) so the grid and the hand-off rail pass beneath a card.
#[allow(clippy::too_many_arguments)]
fn block_card(
    state: &gpui::Entity<LanesState>,
    ix: usize,
    live: &Live,
    phase: Phase,
    selected: bool,
    frame: Bounds<f32>,
    under: Hsla,
    clock: f32,
    key: &str,
    generation: usize,
    t: &Pink,
) -> AnyElement {
    let block = &live.block;
    let (x, top, w, h) = (
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
    );
    let block_key = format!("{key}-block-{ix}");
    let waiting = phase == Phase::Waiting;
    let finished = matches!(phase, Phase::Done | Phase::Approved | Phase::Denied);
    let (rest_fill, rest_edge) = if waiting {
        (t.warning.opacity(0.08), t.warning.opacity(0.24))
    } else if finished {
        (t.fill(FillTier::Quaternary), t.separator(false))
    } else {
        (t.fill(FillTier::Quaternary), t.separator(true))
    };
    let (bg, edge, hover_edge) = if selected {
        let edge = t.accent.opacity(0.7);
        (if waiting { rest_fill } else { t.accent_wash }, edge, edge)
    } else {
        (rest_fill, rest_edge, t.accent.opacity(0.45))
    };
    let title_color = if finished && !selected {
        motion::hover_blend(&block_key, t.content_muted, t.text)
    } else {
        t.text
    };
    // Keep the title in view while the card's top has scrolled away, just
    // below the fade band.
    let pad_top = (FADE_BAND - 6.0 - top).max(6.0);
    let visible_h = h - (pad_top - 6.0);
    let title_row = (visible_h >= 22.0).then(|| {
        div()
            .h(px(16.))
            .flex()
            .items_center()
            .gap(px(6.))
            .when(waiting, |el| {
                el.child(
                    svg()
                        .path(ICON_SHIELD)
                        .size(px(12.))
                        .flex_none()
                        .text_color(t.readable_tone(t.warning, 0.95)),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(title_color)
                    .child(block.title.clone()),
            )
            .when(phase == Phase::Running, |el| {
                el.child(loaders::mini_glyph_spinner(
                    format!("{block_key}-spin"),
                    2.0,
                    t.glyph,
                ))
            })
    });
    let sub = match phase {
        Phase::Approved => Some(status_badge(Status::Approved, &block_key, t)),
        Phase::Denied => Some(status_badge(Status::Denied, &block_key, t)),
        Phase::Waiting => Some(
            div()
                .text_size(px(11.))
                .line_height(px(14.))
                .font_features(tabular_figures())
                .text_color(t.readable_tone(t.warning, 0.95))
                .child(format!(
                    "Waiting {}",
                    loaders::format_elapsed((clock - live.start) as i64)
                ))
                .into_any_element(),
        ),
        // Tool ids are machine text: Geist Mono 11, faint.
        _ => block.tool.as_ref().map(|tool| {
            div()
                .flex()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_family(pink::MONO)
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .text_color(t.content_faint)
                        .child(tool.clone()),
                )
                .into_any_element()
        }),
    };
    let sub_row = (visible_h >= 40.0)
        .then_some(sub)
        .flatten()
        .map(|sub| div().mt(px(2.)).flex().child(sub));
    let actions = (waiting && visible_h >= 72.0).then(|| {
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
        // One Primary per group; the refusal is a ghost (Pink's question
        // wizard footer).
        div()
            .mt(px(8.))
            .flex()
            .gap(px(6.))
            .child(
                pink::button(
                    format!("{block_key}-approve"),
                    ButtonKind::Primary,
                    ControlSize::Sm,
                    t,
                )
                .child("Approve")
                .on_click(decide(true)),
            )
            .child(
                pink::button(
                    format!("{block_key}-deny"),
                    ButtonKind::Ghost,
                    ControlSize::Sm,
                    t,
                )
                .child("Deny")
                .on_click(decide(false)),
            )
    });
    let ring = t.focus_ring();
    div()
        .id(ElementId::Name(block_key.clone().into()))
        .role(Role::Button)
        .aria_label(block.title.clone())
        .aria_selected(selected)
        .tab_index(0)
        .absolute()
        .left(px(x))
        .top(px(top))
        .w(px(w))
        .h(px(h))
        .overflow_hidden()
        .rounded(px(radius::LG.min(h / 2.0)))
        .bg(pink::flatten(bg, under))
        .border_1()
        .hover_border(block_key.clone(), edge, hover_edge)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .cursor_pointer()
        .px(px(CARD_PAD_X))
        .pt(px(pad_top))
        .flex()
        .flex_col()
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
        .with_animation(
            ElementId::Name(format!("{block_key}-in-{generation}").into()),
            TOOL_ROW_REVEAL.animation(),
            |el, p| el.opacity(p),
        )
        .into_any_element()
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
