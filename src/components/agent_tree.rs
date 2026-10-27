//! Agent Tree – a Pink component.
//!
//! A multi-agent run as Pink's Agents list, nested. Every agent is Pink's
//! agent row: a status dot (the mini glyph spinner while it runs), the title
//! and a quiet status word, then one meta line of what it is doing, its
//! model, spend and runtime. Sub-agents hang under the agent that started
//! them on Pink's activity rail (a 1 px hairline ribbon with a 6 px elbow
//! that draws in as a row arrives), and a branch folds behind the
//! transcript's disclosure chevron. The header is the run's summary line,
//! and the inspector beside the list is Pink's child-agent card with usage
//! cells, the agent's share of the run and Stop / Retry / Open log.
//!
//! Arrow keys walk the outline: Up/Down move, Left folds or jumps to the
//! parent, Right unfolds or enters the first child, Enter / Space toggle a
//! branch.
//!
//! ```ignore
//! AgentTree::new("run", AgentNode::new("lead", "Lead").status(AgentStatus::Running)
//!     .child(AgentNode::new("audit", "Schema auditor").cost(0.12)))
//!     .title("Migrate billing to Postgres 16")
//!     .on_retry(|id, _, _| println!("retry {id}"))
//! ```

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use gpui::{
    AnyElement, AnyView, App, Div, ElementId, Entity, FocusHandle, FontWeight, Hsla, KeyDownEvent,
    PathBuilder, Pixels, Point, Role, SharedString, Task, Transformation, Window, canvas, div,
    point, prelude::*, px, radians, relative, svg,
};
use web_time::Instant;

use crate::{
    HoverFill,
    motion::{self, MotionSpec},
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, TextStyle, TypeStyleExt, loaders, radius,
        space, text,
    },
    story,
};

pub const NAME: &str = "Agent Tree";
pub const DESCRIPTION: &str = "A multi-agent run as a nested agents list: status, model, spend and runtime on every row, sub-agents on a rail that folds away, and an inspector with the selected agent's usage, Stop and Retry";

const ICON_BOT: &str = "icons/agent-tree/bot.svg";
const ICON_CHEVRON: &str = "icons/agent-tree/alt-arrow-right.svg";

/// GPUI's default line height (`phi()`), which Pink's agent rows inherit:
/// they set sizes but no line heights.
const PHI: f32 = 1.618_034;

// Agent row (Pink: `shell/agents.rs` `render_agents_section`).
const ROW_PAD_X: f32 = 10.0;
const ROW_PAD_Y: f32 = 7.0;
/// Between the title line and the meta line.
const ROW_LINE_GAP: f32 = 2.0;
/// Between rows (the list's `gap(2)`).
const ROW_GAP: f32 = 2.0;
const DOT: f32 = 7.0;
const LEAD_GAP: f32 = 8.0;
/// The meta line starts under the title (dot + gap).
const META_INDENT: f32 = DOT + LEAD_GAP;
const TITLE_SIZE: f32 = 13.0;
const STATUS_SIZE: f32 = 11.0;
const META_SIZE: f32 = 11.5;
const TITLE_LINE: f32 = TITLE_SIZE * PHI;
const META_LINE: f32 = META_SIZE * PHI;
const ROW_H: f32 = 2.0 * ROW_PAD_Y + TITLE_LINE + ROW_LINE_GAP + META_LINE;
/// A row and the gap under it: the rail runs through both.
const SLOT_H: f32 = ROW_H + ROW_GAP;
const DOT_CY: f32 = ROW_PAD_Y + TITLE_LINE / 2.0;
/// An arriving row's content settles this far up (Pink: `transcript.rs`
/// `tool_chip`, `top(4 * (1 - content_reveal))`).
const ROW_RISE: f32 = 4.0;

// Activity rail (Pink: `transcript.rs` `activity_rail`, trunk x 12.5, elbow
// radius 6, branch to x 28, glyph at x 32).
const RAIL_ALPHA: f32 = 0.12;
const BEND_RADIUS: f32 = 6.0;
/// Trunk to the child's status glyph.
const RAIL_SPAN: f32 = 32.0 - 12.5;
/// The break between the branch tip and the glyph.
const BRANCH_GAP: f32 = 32.0 - 28.0;
/// The trunk hangs from the parent's dot.
const TRUNK_X: f32 = ROW_PAD_X + DOT / 2.0;
/// Each level puts the child's dot at the rail's glyph position.
const INDENT: f32 = TRUNK_X + RAIL_SPAN - ROW_PAD_X;
/// The trunk leaves the parent's dot after the same break.
const RAIL_TOP: f32 = DOT_CY + DOT / 2.0 + BRANCH_GAP;

// Disclosure tile (Pink: `transcript.rs` `chip_header_row` trail).
const TILE: f32 = 18.0;
const CHEVRON: f32 = 12.0;
const ROW_GROUP: &str = "agent-row";

/// The list column: Pink's widest right pane.
const OUTLINE_W: f32 = pink::layout::SIDEBAR_MAX;
/// Usage cells (Pink: `shell/usage.rs` `metric_cell`, min 96 wide).
const CELL_MIN_W: f32 = 96.0;
const CELL_GAP: f32 = 8.0;
const INSPECTOR_PAD: f32 = 16.0;
/// Room for three usage cells side by side.
const INSPECTOR_W: f32 = 3.0 * CELL_MIN_W + 2.0 * CELL_GAP + 2.0 * INSPECTOR_PAD;

type IdHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// Where an agent is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStatus {
    Queued,
    Running,
    Done,
    Failed,
    Stopped,
}

impl AgentStatus {
    pub fn label(self) -> &'static str {
        match self {
            AgentStatus::Queued => "Queued",
            AgentStatus::Running => "Running",
            AgentStatus::Done => "Done",
            AgentStatus::Failed => "Failed",
            AgentStatus::Stopped => "Stopped",
        }
    }

    /// The status dot (Pink: `shell/agents.rs` agent row).
    fn dot(self, t: &Pink) -> Hsla {
        match self {
            AgentStatus::Done => t.readable_tone(t.success, 1.0),
            AgentStatus::Failed => t.readable_tone(t.danger, 1.0),
            AgentStatus::Stopped => t.readable_tone(t.warning, 1.0),
            AgentStatus::Running => t.accent,
            AgentStatus::Queued => t.text_faint,
        }
    }
}

/// One agent and the sub-agents it started.
#[derive(Clone, Debug)]
pub struct AgentNode {
    pub id: SharedString,
    pub name: SharedString,
    pub model: SharedString,
    pub task: SharedString,
    pub status: AgentStatus,
    /// Own spend in dollars, without the sub-agents.
    pub cost: f32,
    /// Runtime in seconds.
    pub elapsed: u32,
    pub tokens: u32,
    pub children: Vec<AgentNode>,
}

impl AgentNode {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            model: SharedString::default(),
            task: SharedString::default(),
            status: AgentStatus::Queued,
            cost: 0.,
            elapsed: 0,
            tokens: 0,
            children: Vec::new(),
        }
    }

    pub fn model(mut self, model: impl Into<SharedString>) -> Self {
        self.model = model.into();
        self
    }

    pub fn task(mut self, task: impl Into<SharedString>) -> Self {
        self.task = task.into();
        self
    }

    pub fn status(mut self, status: AgentStatus) -> Self {
        self.status = status;
        self
    }

    pub fn cost(mut self, dollars: f32) -> Self {
        self.cost = dollars;
        self
    }

    pub fn elapsed(mut self, seconds: u32) -> Self {
        self.elapsed = seconds;
        self
    }

    pub fn tokens(mut self, tokens: u32) -> Self {
        self.tokens = tokens;
        self
    }

    pub fn child(mut self, child: AgentNode) -> Self {
        self.children.push(child);
        self
    }

    /// Own spend plus every descendant's.
    pub fn branch_cost(&self) -> f32 {
        self.cost + self.children.iter().map(|c| c.branch_cost()).sum::<f32>()
    }

    /// Own tokens plus every descendant's.
    pub fn branch_tokens(&self) -> u64 {
        u64::from(self.tokens) + self.children.iter().map(|c| c.branch_tokens()).sum::<u64>()
    }

    /// Number of agents below this one.
    pub fn descendants(&self) -> usize {
        self.children.iter().map(|c| 1 + c.descendants()).sum()
    }

    /// Agents in this subtree (this one included) with `status`.
    pub fn count(&self, status: AgentStatus) -> usize {
        usize::from(self.status == status)
            + self.children.iter().map(|c| c.count(status)).sum::<usize>()
    }

    pub fn find(&self, id: &str) -> Option<&AgentNode> {
        if self.id.as_ref() == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut AgentNode> {
        if self.id.as_ref() == id {
            return Some(self);
        }
        self.children.iter_mut().find_map(|c| c.find_mut(id))
    }

    fn parent_of(&self, id: &str) -> Option<&AgentNode> {
        if self.children.iter().any(|c| c.id.as_ref() == id) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.parent_of(id))
    }

    fn descendant_ids(&self, out: &mut Vec<SharedString>) {
        for child in &self.children {
            out.push(child.id.clone());
            child.descendant_ids(out);
        }
    }
}

// ---------------------------------------------------------------------------
// Figures
// ---------------------------------------------------------------------------

/// `1234` → `1,234` (Pink: `shell/usage.rs` `count`).
fn grouped(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// `$1,234.56` (Pink: `shell/usage.rs` `money`).
fn money(dollars: f32) -> String {
    let cents = (dollars.max(0.) * 100.).round() as u64;
    format!("${}.{:02}", grouped(cents / 100), cents % 100)
}

/// `<1s`, `51s`, `1m 35s`, `1h 05m` (Pink: `native_agents.rs` `duration_label`).
fn duration_label(seconds: u32) -> String {
    match seconds {
        0 => "<1s".into(),
        1..=59 => format!("{seconds}s"),
        _ if seconds < 3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60),
    }
}

/// `30.4k tokens` for meta lines (Pink: `claude_advanced.rs` `compact_tokens`).
fn tokens_label(tokens: u64) -> String {
    match tokens {
        0..=999 => format!("{tokens} tokens"),
        1_000..=999_999 => format!("{:.1}k tokens", tokens as f64 / 1_000.0),
        _ => format!("{:.2}M tokens", tokens as f64 / 1_000_000.0),
    }
}

/// `30.4K` for a usage cell (Pink: `shell/usage.rs` `compact_tokens`).
fn tokens_figure(tokens: u64) -> String {
    for (threshold, suffix) in [(1_000_000_000u64, "B"), (1_000_000, "M"), (1_000, "K")] {
        if tokens >= threshold {
            let scaled = tokens as f64 / threshold as f64;
            let digits = if scaled >= 100.0 {
                0
            } else if scaled >= 10.0 {
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
    grouped(tokens)
}

/// `48.0%`, `<0.1%` (Pink: `shell/usage.rs` `usage_percent`).
fn share_label(share: f32) -> String {
    let percent = share * 100.0;
    if percent > 0.0 && percent < 0.1 {
        "<0.1%".into()
    } else {
        format!("{percent:.1}%")
    }
}

fn agents_label(n: usize) -> String {
    format!("{n} agent{}", if n == 1 { "" } else { "s" })
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// One visible row, for keyboard navigation.
struct Visible {
    id: SharedString,
    parent: Option<SharedString>,
    first_child: Option<SharedString>,
}

fn flatten(
    node: &AgentNode,
    parent: Option<&SharedString>,
    folded: &HashSet<SharedString>,
    out: &mut Vec<Visible>,
) {
    out.push(Visible {
        id: node.id.clone(),
        parent: parent.cloned(),
        first_child: node.children.first().map(|c| c.id.clone()),
    });
    if !folded.contains(&node.id) {
        for child in &node.children {
            flatten(child, Some(&node.id), folded, out);
        }
    }
}

/// Gives every agent seen for the first time its reveal start: Pink's
/// arrival stagger, with the first-row beat on the first render.
fn stamp_arrivals(
    node: &AgentNode,
    arrived: &mut HashMap<SharedString, Instant>,
    first_mount: bool,
    new_rows: &mut usize,
    now: Instant,
) {
    if !arrived.contains_key(&node.id) {
        arrived.insert(
            node.id.clone(),
            now + motion::tool_row_arrival_delay(*new_rows, first_mount),
        );
        *new_rows += 1;
    }
    for child in &node.children {
        stamp_arrivals(child, arrived, first_mount, new_rows, now);
    }
}

struct TreeState {
    folded: HashSet<SharedString>,
    /// When each branch last folded or unfolded: one clock for its chevron,
    /// height and body fade (Pink tool fold).
    toggled: HashMap<SharedString, Instant>,
    /// When each agent's row starts revealing.
    arrived: HashMap<SharedString, Instant>,
    selected: Option<SharedString>,
    focus: FocusHandle,
}

impl TreeState {
    /// Folds or unfolds `id`; a fold that hides the selection selects the
    /// branch instead.
    fn set_folded(&mut self, id: &SharedString, fold: bool, hidden: &[SharedString]) {
        if fold {
            self.folded.insert(id.clone());
            if self.selected.as_ref().is_some_and(|s| hidden.contains(s)) {
                self.selected = Some(id.clone());
            }
        } else {
            self.folded.remove(id);
        }
        self.toggled.insert(id.clone(), Instant::now());
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// A live outline of an orchestrator and the sub-agents it started.
#[derive(IntoElement)]
pub struct AgentTree {
    id: ElementId,
    root: AgentNode,
    title: SharedString,
    selected: Option<SharedString>,
    folded: Vec<SharedString>,
    inspector: bool,
    on_stop: Option<IdHandler>,
    on_retry: Option<IdHandler>,
    on_open_log: Option<IdHandler>,
}

impl AgentTree {
    pub fn new(id: impl Into<ElementId>, root: AgentNode) -> Self {
        Self {
            id: id.into(),
            root,
            title: "Agent run".into(),
            selected: None,
            folded: Vec::new(),
            inspector: true,
            on_stop: None,
            on_retry: None,
            on_open_log: None,
        }
    }

    /// The run's title in the header.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// The agent selected on first render (defaults to the root).
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// A branch that starts folded.
    pub fn folded(mut self, id: impl Into<SharedString>) -> Self {
        self.folded.push(id.into());
        self
    }

    /// Hides the inspector, leaving just the outline.
    pub fn inspector(mut self, show: bool) -> Self {
        self.inspector = show;
        self
    }

    /// "Stop" on a running agent.
    pub fn on_stop(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(f));
        self
    }

    /// "Retry" on a failed or stopped agent.
    pub fn on_retry(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(f));
        self
    }

    /// "Open log" in the inspector.
    pub fn on_open_log(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_log = Some(Rc::new(f));
        self
    }
}

/// What every row needs to know about the tree around it, plus the frame's
/// clock.
struct Ctx<'a> {
    t: &'a Pink,
    key: &'a str,
    state: &'a Entity<TreeState>,
    folded: &'a HashSet<SharedString>,
    toggled: &'a HashMap<SharedString, Instant>,
    arrived: &'a HashMap<SharedString, Instant>,
    selected: &'a SharedString,
    /// The outline holds keyboard focus: the selected row wears the ring.
    ring: bool,
    now: Instant,
    reduced: bool,
    /// Something is still moving this frame.
    animating: Cell<bool>,
}

impl Ctx<'_> {
    fn reveal(&self, spec: MotionSpec, id: &SharedString) -> f32 {
        let progress =
            motion::reveal_progress(spec, self.arrived.get(id).copied(), self.now, self.reduced);
        if progress < 1.0 {
            self.animating.set(true);
        }
        progress
    }

    /// The row's own reveal: height and content (Pink `TOOL_ROW_REVEAL`).
    fn row_reveal(&self, id: &SharedString) -> f32 {
        self.reveal(motion::TOOL_ROW_REVEAL, id)
    }

    /// The row's connector draw (Pink `TOOL_CONNECTOR_REVEAL`).
    fn connector(&self, id: &SharedString) -> f32 {
        self.reveal(motion::TOOL_CONNECTOR_REVEAL, id)
    }

    /// `(progress, body_alpha)` of a branch's fold.
    fn fold(&self, id: &SharedString) -> (f32, f32) {
        let open = !self.folded.contains(id);
        let elapsed = self
            .toggled
            .get(id)
            .map(|at| self.now.saturating_duration_since(*at))
            .filter(|elapsed| !self.reduced && *elapsed < motion::FOLD_TWEEN_WINDOW);
        if elapsed.is_some() {
            self.animating.set(true);
        }
        motion::fold_progress(open, elapsed)
    }

    /// Height of a row and everything showing under it this frame.
    fn subtree_h(&self, node: &AgentNode) -> f32 {
        SLOT_H * self.row_reveal(&node.id) + self.kids_h(node)
    }

    fn kids_h(&self, node: &AgentNode) -> f32 {
        if node.children.is_empty() {
            return 0.;
        }
        let (progress, _) = self.fold(&node.id);
        progress * node.children.iter().map(|c| self.subtree_h(c)).sum::<f32>()
    }
}

impl RenderOnce for AgentTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);
        let now = Instant::now();
        let (initial, initial_folds) = (self.selected.clone(), self.folded.clone());
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| TreeState {
                folded: initial_folds.into_iter().collect(),
                toggled: HashMap::new(),
                arrived: HashMap::new(),
                selected: initial,
                focus: cx.focus_handle().tab_stop(true),
            },
        );
        let root = self.root;
        state.update(cx, |s, _| {
            let first_mount = s.arrived.is_empty();
            let mut new_rows = 0;
            stamp_arrivals(&root, &mut s.arrived, first_mount, &mut new_rows, now);
        });
        let (folded, toggled, arrived, selected, focus) = {
            let s = state.read(cx);
            (
                s.folded.clone(),
                s.toggled.clone(),
                s.arrived.clone(),
                s.selected.clone(),
                s.focus.clone(),
            )
        };
        let selected = selected
            .filter(|id| root.find(id).is_some())
            .unwrap_or_else(|| root.id.clone());
        let ring = focus.is_focused(window) && window.last_input_was_keyboard();

        let ctx = Ctx {
            t: &t,
            key: &key,
            state: &state,
            folded: &folded,
            toggled: &toggled,
            arrived: &arrived,
            selected: &selected,
            ring,
            now,
            reduced,
            animating: Cell::new(false),
        };
        let outline_rows = branch(&root, 0, &[], &ctx);
        if ctx.animating.get() {
            window.request_animation_frame();
        }

        let mut visible = Vec::new();
        flatten(&root, None, &folded, &mut visible);
        let hidden: HashMap<SharedString, Vec<SharedString>> = visible
            .iter()
            .filter_map(|v| root.find(&v.id))
            .map(|node| {
                let mut ids = Vec::new();
                node.descendant_ids(&mut ids);
                (node.id.clone(), ids)
            })
            .collect();
        let key_state = state.clone();
        let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let current = key_state.read(cx).selected.clone();
            let pos = visible
                .iter()
                .position(|v| Some(&v.id) == current.as_ref())
                .unwrap_or(0);
            let Some(row) = visible.get(pos) else { return };
            let is_folded = key_state.read(cx).folded.contains(&row.id);
            let has_kids = row.first_child.is_some();
            enum Move {
                Select(SharedString),
                Fold(SharedString, bool),
            }
            let action = match event.keystroke.key.as_str() {
                "down" => visible.get(pos + 1).map(|v| Move::Select(v.id.clone())),
                "up" => pos
                    .checked_sub(1)
                    .and_then(|p| visible.get(p))
                    .map(|v| Move::Select(v.id.clone())),
                "home" => visible.first().map(|v| Move::Select(v.id.clone())),
                "end" => visible.last().map(|v| Move::Select(v.id.clone())),
                "left" if has_kids && !is_folded => Some(Move::Fold(row.id.clone(), true)),
                "left" => row.parent.clone().map(Move::Select),
                "right" if has_kids && is_folded => Some(Move::Fold(row.id.clone(), false)),
                "right" => row.first_child.clone().map(Move::Select),
                "enter" | "space" if has_kids => Some(Move::Fold(row.id.clone(), !is_folded)),
                _ => None,
            };
            let Some(action) = action else { return };
            key_state.update(cx, |s, cx| {
                match action {
                    Move::Select(id) => s.selected = Some(id),
                    Move::Fold(id, fold) => {
                        let ids = hidden.get(&id).cloned().unwrap_or_default();
                        s.set_folded(&id, fold, &ids);
                    }
                }
                cx.notify();
            });
            cx.stop_propagation();
        };

        let outline = div()
            .id(eid(format!("{key}-outline")))
            .role(Role::Tree)
            .aria_label(self.title.clone())
            .track_focus(&focus)
            .on_key_down(on_key)
            .w(px(OUTLINE_W))
            .flex_none()
            .p(px(space::SM))
            .flex()
            .flex_col()
            .child(outline_rows);

        let inspector = self.inspector.then(|| {
            let node = root.find(&selected).unwrap_or(&root);
            let parent = root.parent_of(&selected).map(|p| p.name.clone());
            inspector(
                node,
                parent,
                root.branch_cost(),
                &key,
                &t,
                Handlers {
                    stop: self.on_stop.clone(),
                    retry: self.on_retry.clone(),
                    open_log: self.on_open_log.clone(),
                },
            )
        });

        let width = OUTLINE_W + if self.inspector { INSPECTOR_W } else { 0. } + 2.;
        let el = pink::card(&t)
            .id(self.id.clone())
            .w(px(width))
            .font_family(pink::SANS)
            // Pink's root text size; the rows set their own.
            .text_size(px(text::PROSE))
            .text_color(t.text)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(header(&root, &self.title, &key, &t))
            .child(pink::divider(&t))
            .child(div().flex().child(outline).children(inspector));
        motion::keep_alive(window);
        el
    }
}

/// The run's summary: its title, the counts, tokens, spend and runtime on
/// one quiet line (Pink: `claude_advanced.rs` `task_card` metrics line, in
/// the settings card header's type), and the run's status badge.
fn header(root: &AgentNode, title: &SharedString, key: &str, t: &Pink) -> Div {
    let agents = 1 + root.descendants();
    let done = root.count(AgentStatus::Done);
    let failed = root.count(AgentStatus::Failed);
    let mut counts = format!("{}, {done} done", agents_label(agents));
    if failed > 0 {
        counts.push_str(&format!(", {failed} failed"));
    }
    let mut parts = vec![
        counts,
        tokens_label(root.branch_tokens()),
        money(root.branch_cost()),
    ];
    if root.elapsed > 0 {
        parts.push(duration_label(root.elapsed));
    }
    pink::section_header(title.clone(), Some(parts.join(", ").into()), t)
        // Text starts on the rows' dot column.
        .px(px(space::SM + ROW_PAD_X))
        .child(pink::row_controls().child(run_status(root, key, t)))
}

/// Pink's one status badge (`transcript.rs` `chip_header_row`): a 6 px dot
/// and an 11 px medium word, red only on failure; while agents run, the
/// word counts them and the mini glyph spinner trails it.
fn run_status(root: &AgentNode, key: &str, t: &Pink) -> Div {
    let running = root.count(AgentStatus::Running);
    let (label, dot) = if running > 0 {
        (format!("{running} running"), None)
    } else {
        (root.status.label().to_string(), Some(root.status.dot(t)))
    };
    let failed = running == 0 && root.status == AgentStatus::Failed;
    div()
        .flex_none()
        .h(px(TextStyle::Title3.line_height()))
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(STATUS_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if failed {
                    t.readable_tone(t.danger, 1.0)
                } else {
                    t.content_muted
                })
                .when_some(dot, |badge, dot| {
                    badge.child(div().size(px(6.)).flex_none().rounded_full().bg(dot))
                })
                .child(motion::status_swap(
                    loaders::label_id(&format!("{key}-run-status"), &label),
                    div().child(label),
                )),
        )
        .when(running > 0, |row| {
            row.child(loaders::mini_glyph_spinner(
                format!("{key}-run-spin"),
                2.0,
                t.glyph,
            ))
        })
}

/// A node's slot and, under it, its sub-agents in a body that folds on
/// Pink's disclosure spring. `lines[k]` is the next sibling at rail level
/// `k` along this node's path (the last entry is this node's own).
fn branch(node: &AgentNode, depth: usize, lines: &[Option<SharedString>], c: &Ctx) -> AnyElement {
    let slot = slot(node, depth, lines, c);
    if node.children.is_empty() {
        return slot;
    }
    let (progress, alpha) = c.fold(&node.id);
    let body = (progress > 0.).then(|| {
        let height = c.kids_h(node);
        let kids = node.children.iter().enumerate().map(|(i, child)| {
            let mut child_lines = lines.to_vec();
            child_lines.push(node.children.get(i + 1).map(|n| n.id.clone()));
            branch(child, depth + 1, &child_lines, c)
        });
        div().flex_none().h(px(height)).overflow_hidden().child(
            div()
                .relative()
                .top(px(motion::TOOL_BODY_RISE * (1. - alpha)))
                .opacity(alpha)
                .flex()
                .flex_col()
                .children(kids),
        )
    });
    div()
        .flex_none()
        .flex()
        .flex_col()
        .child(slot)
        .children(body)
        .into_any_element()
}

/// One row and the gap under it, growing in on arrival with the rail
/// drawn over both.
fn slot(node: &AgentNode, depth: usize, lines: &[Option<SharedString>], c: &Ctx) -> AnyElement {
    let reveal = c.row_reveal(&node.id);
    let rail = rail_for(node, depth, lines, c);
    let color = c.t.hairline(RAIL_ALPHA);
    div()
        .relative()
        .flex_none()
        .w_full()
        .h(px(SLOT_H * reveal))
        .overflow_hidden()
        .child(row(node, depth, reveal, c))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| paint_rail(&rail, bounds.origin, color, window),
            )
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h(px(SLOT_H)),
        )
        .into_any_element()
}

/// The agent row (Pink: `shell/agents.rs` `render_agents_section`): px 10,
/// py 7, radius 8, two lines 2 apart, no outlined box. Line one is the 7 px
/// status dot, the 13 px title (red when failed) and the 11 px medium
/// status word; line two is the 11.5 px meta line under the title. A
/// branch adds the transcript's disclosure tile. Hover is Pink's quiet
/// wash; the selected row takes the chrome selection fill, and the
/// keyboard ring while the outline has focus.
fn row(node: &AgentNode, depth: usize, reveal: f32, c: &Ctx) -> impl IntoElement {
    let t = c.t;
    let id = node.id.clone();
    let is_selected = *c.selected == node.id;
    let has_kids = !node.children.is_empty();
    let folded = has_kids && c.folded.contains(&node.id);
    let failed = node.status == AgentStatus::Failed;
    let label = node.status.label();

    // The running row's mark is the sidebar working spinner.
    let lead = div().flex_none().w(px(DOT)).flex().justify_center().child(
        if node.status == AgentStatus::Running {
            loaders::mini_glyph_spinner(format!("{}-spin-{id}", c.key), 2.0, t.glyph)
                .into_any_element()
        } else {
            div()
                .size(px(DOT))
                .flex_none()
                .rounded_full()
                .bg(node.status.dot(t))
                .into_any_element()
        },
    );

    // Leaves keep the tile's slot, so every status word ends on one column.
    let fold_tile = if !has_kids {
        div().size(px(TILE)).flex_none().into_any_element()
    } else {
        let (progress, _) = c.fold(&node.id);
        let state = c.state.clone();
        let fold_id = id.clone();
        let mut hidden = Vec::new();
        node.descendant_ids(&mut hidden);
        // Pink's trailing tile: half strength at rest, full under the
        // pointer; the chevron turns 0 → 90° with the fold.
        div()
            .id(eid(format!("{}-fold-{id}", c.key)))
            .size(px(TILE))
            .flex_none()
            .opacity(0.5)
            .group_hover(ROW_GROUP, |s| s.opacity(1.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(
                svg()
                    .path(ICON_CHEVRON)
                    .size(px(CHEVRON))
                    .with_transformation(Transformation::rotate(radians(FRAC_PI_2 * progress)))
                    .text_color(t.content_faint)
                    .group_hover(ROW_GROUP, |s| s.text_color(t.text)),
            )
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                state.update(cx, |s, cx| {
                    let fold = !s.folded.contains(&fold_id);
                    s.set_folded(&fold_id, fold, &hidden);
                    cx.notify();
                });
            })
            .into_any_element()
    };

    let title_line = div()
        .flex()
        .items_center()
        .gap(px(LEAD_GAP))
        .child(lead)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .truncate()
                .text_size(px(TITLE_SIZE))
                .line_height(px(TITLE_LINE))
                .text_color(if failed {
                    t.readable_tone(t.danger, 1.0)
                } else {
                    t.text
                })
                .child(node.name.clone()),
        )
        .child(motion::status_swap(
            loaders::label_id(&format!("{}-status-{id}", c.key), label),
            div()
                .flex_none()
                .text_size(px(STATUS_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.readable_muted(0.9))
                .child(label),
        ))
        .child(fold_tile);

    // The activity, then "model, spend, elapsed" in the trailing column
    // Pink's chip rows share (one right-aligned edge, not trailing the text
    // at a varying x): the activity alone gives way when the line runs out.
    let mut activity = if node.task.is_empty() {
        label.to_string()
    } else {
        node.task.to_string()
    };
    if folded {
        activity = format!("+{}, {activity}", agents_label(node.descendants()));
    }
    let mut figures: Vec<String> = Vec::new();
    if !node.model.is_empty() {
        figures.push(node.model.to_string());
    }
    if node.cost > 0. || node.elapsed > 0 {
        figures.push(money(node.cost));
        figures.push(duration_label(node.elapsed));
    }
    let meta_line = div()
        .pl(px(META_INDENT))
        .flex()
        .gap(px(LEAD_GAP))
        .min_w_0()
        .text_size(px(META_SIZE))
        .line_height(px(META_LINE))
        .text_color(t.readable_muted(0.85))
        .child(div().flex_1().min_w_0().truncate().child(activity))
        .when(!figures.is_empty(), |line| {
            line.child(
                div()
                    .flex_none()
                    .font_features(pink::tabular_figures())
                    .child(figures.join(", ")),
            )
        });

    let state = c.state.clone();
    let select_id = id.clone();
    let hover_key = format!("{}-row-{id}", c.key);
    div()
        .id(eid(hover_key.clone()))
        .group(ROW_GROUP)
        .role(Role::TreeItem)
        .aria_label(format!("{}, {label}", node.name))
        .aria_level(depth + 1)
        .aria_selected(is_selected)
        .when(has_kids, |el| el.aria_expanded(!folded))
        .relative()
        .top(px(ROW_RISE * (1. - reveal)))
        .opacity(reveal)
        .flex_none()
        .w_full()
        .h(px(ROW_H))
        .pl(px(ROW_PAD_X + depth as f32 * INDENT))
        .pr(px(ROW_PAD_X))
        .py(px(ROW_PAD_Y))
        .rounded(px(radius::MD))
        .flex()
        .flex_col()
        .gap(px(ROW_LINE_GAP))
        .cursor_pointer()
        .map(|el| {
            if is_selected {
                el.bg(t.glass_selected_bg())
            } else {
                el.hover_wash(hover_key, t.fill(FillTier::Tertiary))
            }
        })
        .when(is_selected && c.ring, |el| {
            el.shadow(t.focus_ring_shadows())
        })
        .on_click(move |_, window, cx| {
            state.update(cx, |s, cx| {
                s.selected = Some(select_id.clone());
                window.focus(&s.focus, cx);
                cx.notify();
            });
        })
        .child(title_line)
        .child(meta_line)
}

// ---------------------------------------------------------------------------
// Activity rail
// ---------------------------------------------------------------------------

/// What one slot's rail draws (Pink: `transcript.rs` `activity_rail`).
#[derive(Clone, Default)]
struct Rail {
    /// Ancestor trunks passing this slot: x and opacity.
    guides: Vec<(f32, f32)>,
    /// The elbow off the parent's trunk.
    elbow: Option<Elbow>,
    /// The trunk this row starts for its sub-agents: x, drawn share,
    /// opacity.
    start: Option<(f32, f32, f32)>,
}

#[derive(Clone, Copy)]
struct Elbow {
    x: f32,
    /// Trunk from the slot top to the bend.
    incoming: f32,
    /// The bend and the branch to the dot.
    branch: f32,
    /// Trunk from the bend on to the slot bottom, for a later sibling.
    continuation: f32,
}

fn trunk_x(level: usize) -> f32 {
    level as f32 * INDENT + TRUNK_X
}

/// The rail for `node`'s slot. The connector draws on Pink's sequence: a
/// later sibling's arrival first extends the trunk past the rows above it,
/// then draws its own trunk, bend and branch.
fn rail_for(node: &AgentNode, depth: usize, lines: &[Option<SharedString>], c: &Ctx) -> Rail {
    let continuation = |next: &Option<SharedString>| {
        next.as_ref()
            .map(|id| motion::tool_connector_continuation(Some(c.connector(id))))
            .unwrap_or(0.)
    };
    let mut rail = Rail::default();
    if depth > 0 {
        for (level, next) in lines.iter().enumerate().take(depth - 1) {
            let alpha = continuation(next);
            if alpha > 0. {
                rail.guides.push((trunk_x(level), alpha));
            }
        }
        let (incoming, branch) = motion::tool_connector_parts(c.connector(&node.id), true);
        rail.elbow = Some(Elbow {
            x: trunk_x(depth - 1),
            incoming,
            branch,
            continuation: continuation(&lines[depth - 1]),
        });
    }
    if let Some(first) = node.children.first() {
        let (progress, _) = c.fold(&node.id);
        let drawn = motion::tool_connector_continuation(Some(c.connector(&first.id)));
        if progress > 0. && drawn > 0. {
            rail.start = Some((trunk_x(depth), drawn, progress));
        }
    }
    rail
}

fn union_path() -> PathBuilder {
    PathBuilder::fill().with_style(gpui::PathStyle::Fill(
        gpui::FillOptions::default().with_fill_rule(gpui::FillRule::NonZero),
    ))
}

/// Paints a slot's rail. Fully drawn segments share one nonzero path so the
/// joints never double their alpha; fading ones paint on their own.
fn paint_rail(rail: &Rail, origin: Point<Pixels>, color: Hsla, window: &mut Window) {
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));
    let mut solid = union_path();
    let mut faded: Vec<(PathBuilder, f32)> = Vec::new();
    let mut add = |points: &[Point<Pixels>], alpha: f32| {
        if alpha >= 1. {
            ribbon(&mut solid, points);
        } else if alpha > 0. {
            let mut path = union_path();
            ribbon(&mut path, points);
            faded.push((path, alpha));
        }
    };
    for &(x, alpha) in &rail.guides {
        add(&[at(x, 0.), at(x, SLOT_H)], alpha);
    }
    if let Some(elbow) = rail.elbow {
        let bend_y = DOT_CY - BEND_RADIUS;
        if elbow.incoming > 0. {
            let mut bottom = bend_y * elbow.incoming;
            if elbow.incoming >= 1. && elbow.continuation > 0. {
                bottom = bend_y + (SLOT_H - bend_y) * elbow.continuation;
            }
            add(&[at(elbow.x, 0.), at(elbow.x, bottom)], 1.);
        }
        if elbow.branch > 0. {
            let points: Vec<_> = branch_points(elbow.branch)
                .into_iter()
                .map(|p| at(elbow.x + p.x, bend_y + p.y))
                .collect();
            add(&points, 1.);
        }
    }
    if let Some((x, drawn, alpha)) = rail.start {
        let bottom = RAIL_TOP + (SLOT_H - RAIL_TOP) * drawn;
        add(&[at(x, RAIL_TOP), at(x, bottom)], alpha);
    }
    if let Ok(path) = solid.build() {
        window.paint_path(path, color);
    }
    for (path, alpha) in faded {
        if let Ok(path) = path.build() {
            window.paint_path(path, color.opacity(alpha));
        }
    }
}

/// The elbow and branch, revealed by distance so the bend and the straight
/// leg draw at one speed (Pink: `transcript.rs` `activity_branch_points`).
fn branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<_> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0;
            point(BEND_RADIUS * t * t, BEND_RADIUS * (2.0 * t - t * t))
        })
        .collect();
    path.push(point(RAIL_SPAN - BRANCH_GAP, BEND_RADIUS));
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

/// A 1 px ribbon along `points`, as a clockwise contour (Pink:
/// `transcript.rs` `activity_ribbon`).
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

// ---------------------------------------------------------------------------
// Inspector
// ---------------------------------------------------------------------------

struct Handlers {
    stop: Option<IdHandler>,
    retry: Option<IdHandler>,
    open_log: Option<IdHandler>,
}

/// The agent's identity tile (Pink: `transcript/agent_cards.rs`
/// `AgentCard::render`): 28 square, radius 7, `hairline(0.07)` edge on the
/// resting fill, the 16 px bot glyph.
fn agent_tile(t: &Pink) -> Div {
    div()
        .flex_none()
        .size(px(28.))
        .rounded(px(radius::TILE))
        .border_1()
        .border_color(t.hairline(0.07))
        .bg(t.fill(FillTier::Quaternary))
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(ICON_BOT)
                .size(px(16.))
                .flex_none()
                .text_color(t.content_muted),
        )
}

/// A quiet type label (Pink: `transcript/agent_cards.rs` `type_badge`).
fn type_badge(label: &'static str, t: &Pink) -> Div {
    div()
        .flex_none()
        .h(px(16.))
        .px(px(5.))
        .flex()
        .items_center()
        .rounded(px(radius::XS))
        .bg(t.ink(0.06))
        .text_size(px(STATUS_SIZE))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.content_muted)
        .child(label)
}

/// A label over a value (Pink: `composer.rs` approval context rows).
fn field(label: &'static str, value: SharedString, t: &Pink) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(px(11.))
                .text_color(t.readable_muted(0.65))
                .child(label),
        )
        .child(div().text_size(px(12.)).text_color(t.text).child(value))
}

/// One figure on the resting plate (Pink: `shell/usage.rs` `metric_cell`).
fn metric_cell(label: &'static str, value: String, t: &Pink) -> Div {
    div()
        .flex_1()
        .min_w_0()
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
                .type_style(TextStyle::Caption)
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

/// A 6 px capsule of what is used (Pink: `shell/usage.rs` `usage_meter`):
/// neutral fill on the Secondary track, nothing drawn below 0.5%. A share
/// of the run is no limit, so it never turns warning or danger.
fn usage_meter(share: f32, t: &Pink) -> Div {
    let share = share.clamp(0., 1.);
    div()
        .h(px(6.))
        .w_full()
        .rounded_full()
        .bg(t.fill(FillTier::Secondary))
        .when(share >= 0.005, |bar| {
            bar.child(
                div()
                    .h_full()
                    .w(relative(share))
                    .rounded_full()
                    .bg(t.text_muted),
            )
        })
}

/// The selected agent, as Pink's child-agent card: the tile, the title with
/// its type badge and the status line (dot, status, runtime, model), then
/// its task and parent, its usage cells, its share of the run's spend and
/// the actions.
fn inspector(
    node: &AgentNode,
    parent: Option<SharedString>,
    run_total: f32,
    key: &str,
    t: &Pink,
    handlers: Handlers,
) -> impl IntoElement {
    let failed = node.status == AgentStatus::Failed;
    // Facts sit apart on the row's gap, twice over (Pink `meta_line`'s 12):
    // no separator glyph between them.
    let separator = || div().flex_none();
    let mut status = div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(6.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(t.content_muted)
        .child(
            div()
                .flex_none()
                .size(px(6.))
                .rounded_full()
                .bg(node.status.dot(t)),
        )
        .child(
            div()
                .flex_none()
                .when(failed, |label| {
                    label.text_color(t.readable_tone(t.danger, 1.0))
                })
                .child(node.status.label()),
        );
    if node.elapsed > 0 {
        status = status
            .child(separator())
            .child(div().flex_none().child(duration_label(node.elapsed)));
    }
    if !node.model.is_empty() {
        status = status
            .child(separator())
            .child(div().min_w_0().truncate().child(node.model.clone()));
    }
    let identity = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(agent_tile(t))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .text_size(px(13.))
                                .line_height(px(18.))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(node.name.clone()),
                        )
                        .child(type_badge(
                            if parent.is_some() {
                                "Sub-agent"
                            } else {
                                "Lead"
                            },
                            t,
                        )),
                )
                .child(status),
        );

    let context = div()
        .mt(px(space::MD))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(field(
            "Task",
            if node.task.is_empty() {
                "No task reported".into()
            } else {
                node.task.clone()
            },
            t,
        ))
        .child(field(
            "Spawned by",
            parent.unwrap_or_else(|| "You".into()),
            t,
        ));

    let cells = div()
        .mt(px(space::MD))
        .flex()
        .gap(px(CELL_GAP))
        .child(metric_cell("Spend", money(node.cost), t))
        .child(metric_cell(
            "Tokens",
            tokens_figure(u64::from(node.tokens)),
            t,
        ))
        .child(metric_cell("Runtime", duration_label(node.elapsed), t));

    let branch = node.branch_cost();
    let share = if run_total > 0. {
        (branch / run_total).clamp(0., 1.)
    } else {
        0.
    };
    let subs = node.descendants();
    let running_subs =
        node.count(AgentStatus::Running) - usize::from(node.status == AgentStatus::Running);
    let share_block = div()
        .mt(px(space::MD))
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .type_style(TextStyle::Caption)
                .text_color(t.readable_muted(0.85))
                .child("Share of run")
                .child(
                    div()
                        .font_features(pink::tabular_figures())
                        .child(share_label(share)),
                ),
        )
        .child(usage_meter(share, t).mt(px(6.)))
        .when(subs > 0, |block| {
            let mut note = format!(
                "{} with its {} sub-agent{}",
                money(branch),
                subs,
                if subs == 1 { "" } else { "s" }
            );
            if running_subs > 0 {
                note.push_str(&format!(", {running_subs} running"));
            }
            block.child(
                div()
                    .mt(px(6.))
                    .text_size(px(12.))
                    .text_color(t.readable_muted(0.7))
                    .child(note),
            )
        });

    let id = node.id.clone();
    let mut actions = div().mt(px(space::LG)).flex().items_center().gap(px(8.));
    match node.status {
        AgentStatus::Running | AgentStatus::Queued => {
            let id = id.clone();
            actions = actions.child(
                pink::button(
                    format!("{key}-stop"),
                    ButtonKind::Secondary,
                    ControlSize::Md,
                    t,
                )
                .child("Stop")
                .when_some(handlers.stop, |b, f| {
                    b.on_click(move |_, window, cx| f(&id, window, cx))
                }),
            );
        }
        AgentStatus::Failed | AgentStatus::Stopped => {
            let id = id.clone();
            actions = actions.child(
                pink::button(
                    format!("{key}-retry"),
                    ButtonKind::Primary,
                    ControlSize::Md,
                    t,
                )
                .child("Retry")
                .when_some(handlers.retry, |b, f| {
                    b.on_click(move |_, window, cx| f(&id, window, cx))
                }),
            );
        }
        AgentStatus::Done => {}
    }
    let actions = actions.child(
        pink::button(format!("{key}-log"), ButtonKind::Ghost, ControlSize::Md, t)
            .child("Open log")
            .when_some(handlers.open_log, |b, f| {
                b.on_click(move |_, window, cx| f(&id, window, cx))
            }),
    );

    let content = div()
        .flex()
        .flex_col()
        .child(identity)
        .child(context)
        .child(cells)
        .child(share_block)
        .child(actions);

    div()
        .flex_none()
        .w(px(INSPECTOR_W))
        .border_l_1()
        .border_color(t.separator(false))
        .p(px(INSPECTOR_PAD))
        .child(motion::rise_in(
            eid(format!("{key}-insp-{}", node.id)),
            motion::TRANSCRIPT_IN,
            motion::TRANSCRIPT_IN_RISE,
            content,
        ))
}

// ---------------------------------------------------------------------------
// Story: a release agent fanning out over a database migration
// ---------------------------------------------------------------------------

const LARGE: &str = "lumen-4 large";
const MINI: &str = "lumen-4 mini";

struct Sim {
    root: AgentNode,
    tick: u32,
    _task: Task<()>,
}

impl Sim {
    fn new(cx: &mut gpui::Context<Self>) -> Self {
        let root = AgentNode::new("lead", "Lead")
            .model(LARGE)
            .task("Migrate the billing service from Postgres 13 to 16")
            .status(AgentStatus::Running)
            .cost(0.21)
            .elapsed(96)
            .tokens(48_200)
            .child(
                AgentNode::new("auditor", "Schema auditor")
                    .model(MINI)
                    .task("Diff all 42 tables against the v16 type changes")
                    .status(AgentStatus::Running)
                    .cost(0.04)
                    .elapsed(51)
                    .tokens(21_900),
            )
            .child(
                AgentNode::new("rewriter", "Query rewriter")
                    .model(LARGE)
                    .task("Rewrite the 118 raw SQL calls that rely on implicit casts")
                    .status(AgentStatus::Running)
                    .cost(0.12)
                    .elapsed(44)
                    .tokens(30_400),
            );
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;
                if this
                    .update(cx, |sim, cx| {
                        sim.step();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Sim {
            root,
            tick: 0,
            _task: task,
        }
    }

    fn spawn(&mut self, parent: &str, node: AgentNode) {
        if let Some(p) = self.root.find_mut(parent)
            && p.find(&node.id).is_none()
        {
            p.children.push(node.status(AgentStatus::Running));
        }
    }

    fn set(&mut self, id: &str, status: AgentStatus) {
        if let Some(n) = self.root.find_mut(id)
            && n.status == AgentStatus::Running
        {
            n.status = status;
        }
    }

    fn step(&mut self) {
        self.tick += 1;
        match self.tick {
            2 => self.spawn(
                "auditor",
                AgentNode::new("indexes", "Index checker")
                    .model(MINI)
                    .task("Flag indexes that v16 would rebuild during the upgrade"),
            ),
            3 => self.spawn(
                "rewriter",
                AgentNode::new("tests", "Test runner")
                    .model(MINI)
                    .task("Run the integration suite against a v16 replica"),
            ),
            5 => self.set("indexes", AgentStatus::Done),
            6 => self.spawn(
                "lead",
                AgentNode::new("docs", "Docs writer")
                    .model(MINI)
                    .task("Update the runbook and the rollback notes"),
            ),
            7 => self.set("auditor", AgentStatus::Done),
            8 => self.spawn(
                "rewriter",
                AgentNode::new("profiler", "Perf profiler")
                    .model(MINI)
                    .task("Compare p95 latency of the 20 hottest queries"),
            ),
            10 => self.set("tests", AgentStatus::Failed),
            16 => self.retry("tests"),
            26 => self.set("profiler", AgentStatus::Done),
            34 => self.set("tests-2", AgentStatus::Done),
            37 => self.set("rewriter", AgentStatus::Done),
            42 => self.set("docs", AgentStatus::Done),
            46 => self.set("lead", AgentStatus::Done),
            _ => {}
        }
        let tick = self.tick;
        accrue(&mut self.root, tick);
    }

    fn stop(&mut self, id: &str) {
        if let Some(n) = self.root.find_mut(id) {
            stop_all(n);
        }
    }

    fn retry(&mut self, id: &str) {
        let Some(parent) = self.root.parent_of(id).map(|p| p.id.clone()) else {
            if let Some(n) = self.root.find_mut(id) {
                n.status = AgentStatus::Running;
            }
            return;
        };
        let Some(node) = self.root.find(id) else {
            return;
        };
        if !matches!(node.status, AgentStatus::Failed | AgentStatus::Stopped) {
            return;
        }
        let mut attempt = 2;
        while self.root.find(&format!("{id}-{attempt}")).is_some() {
            attempt += 1;
        }
        let retry = AgentNode::new(
            format!("{id}-{attempt}"),
            format!("{} #{attempt}", node.name),
        )
        .model(node.model.clone())
        .task("Re-run the 6 failing invoice tests with fixed fixtures");
        if let Some(p) = self.root.find_mut(&parent) {
            p.status = AgentStatus::Running;
        }
        self.spawn(&parent, retry);
    }
}

fn stop_all(node: &mut AgentNode) {
    if matches!(node.status, AgentStatus::Running | AgentStatus::Queued) {
        node.status = AgentStatus::Stopped;
    }
    for child in &mut node.children {
        stop_all(child);
    }
}

/// Running agents spend and age a little every tick.
fn accrue(node: &mut AgentNode, tick: u32) {
    if node.status == AgentStatus::Running {
        let large = node.model.as_ref() == LARGE;
        node.cost += if large { 0.011 } else { 0.0035 };
        node.tokens += if large { 640 } else { 410 };
        if tick.is_multiple_of(2) {
            node.elapsed += 1;
        }
    }
    for child in &mut node.children {
        accrue(child, tick);
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let sim = window.use_keyed_state("agent-tree-sim", cx, |_, cx| Sim::new(cx));
        let root = sim.read(cx).root.clone();
        let (stop, retry) = (sim.clone(), sim.clone());
        AgentTree::new("agent-tree", root)
            .title("Migrate billing to Postgres 16")
            .selected("rewriter")
            .folded("auditor")
            .on_stop(move |id, _, cx| {
                stop.update(cx, |s, cx| {
                    s.stop(id);
                    cx.notify();
                })
            })
            .on_retry(move |id, _, cx| {
                retry.update(cx, |s, cx| {
                    s.retry(id);
                    cx.notify();
                })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run() -> AgentNode {
        AgentNode::new("lead", "Lead")
            .cost(0.5)
            .tokens(1_000)
            .status(AgentStatus::Running)
            .child(
                AgentNode::new("a", "A")
                    .cost(0.25)
                    .status(AgentStatus::Done)
                    .child(AgentNode::new("a1", "A one").cost(0.125).tokens(500)),
            )
            .child(
                AgentNode::new("b", "B")
                    .cost(0.125)
                    .status(AgentStatus::Failed),
            )
    }

    #[test]
    fn costs_roll_up_each_branch() {
        let root = run();
        assert_eq!(root.branch_cost(), 1.0);
        assert_eq!(root.find("a").unwrap().branch_cost(), 0.375);
        assert_eq!(root.branch_tokens(), 1_500);
        assert_eq!(root.descendants(), 3);
        assert_eq!(root.count(AgentStatus::Failed), 1);
        assert_eq!(root.parent_of("a1").unwrap().id.as_ref(), "a");
    }

    #[test]
    fn folding_hides_descendant_rows() {
        let root = run();
        let mut folded = HashSet::new();
        let mut rows = Vec::new();
        flatten(&root, None, &folded, &mut rows);
        assert_eq!(rows.len(), 4);
        folded.insert(SharedString::from("a"));
        let mut rows = Vec::new();
        flatten(&root, None, &folded, &mut rows);
        let ids: Vec<_> = rows.iter().map(|r| r.id.to_string()).collect();
        assert_eq!(ids, ["lead", "a", "b"]);
    }

    #[test]
    fn figures_read_like_pink() {
        assert_eq!(money(1234.567), "$1,234.57");
        assert_eq!(duration_label(0), "<1s");
        assert_eq!(duration_label(95), "1m 35s");
        assert_eq!(duration_label(3_900), "1h 05m");
        assert_eq!(tokens_label(30_400), "30.4k tokens");
        assert_eq!(tokens_figure(30_400), "30.4K");
        assert_eq!(share_label(0.48), "48.0%");
    }

    #[test]
    fn a_child_dot_sits_where_the_rail_puts_its_glyph() {
        // Trunk to glyph is Pink's 32 − 12.5, one indent per level.
        assert_eq!(INDENT + ROW_PAD_X - TRUNK_X, RAIL_SPAN);
        assert_eq!(RAIL_SPAN - BRANCH_GAP, 28.0 - 12.5);
    }
}
