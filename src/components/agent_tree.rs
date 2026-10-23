//! Agent Tree – a multi-agent run as a live outline.
//!
//! Every agent is one row: a monogram with a status badge, its name, model
//! and task, and its spend and runtime on the right. Sub-agents hang off a
//! rail under the agent that spawned them, branch rows show the cost of their
//! whole subtree, and the fold pill collapses a branch to that one row. The
//! inspector on the right explains the selected agent (own vs. branch spend,
//! its share of the run) and offers Stop / Retry.
//!
//! Arrow keys walk the outline: Up/Down move, Left folds or jumps to the
//! parent, Right unfolds or enters the first child, Enter toggles a branch.
//!
//! ```ignore
//! AgentTree::new("run", AgentNode::new("lead", "Lead").status(AgentStatus::Running)
//!     .child(AgentNode::new("audit", "Schema auditor").cost(0.12)))
//!     .title("Migrate billing to Postgres 16")
//!     .on_retry(|id, _, _| println!("retry {id}"))
//! ```

use std::{collections::HashSet, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyElement, AnyView, App, Div, ElementId, Entity, FocusHandle,
    FontWeight, Hsla, KeyDownEvent, SharedString, Task, Transformation, Window, div, percentage,
    prelude::*, px, relative, svg,
};

use crate::{
    HoverFill,
    motion::{self, GLIDE, MENU_IN, ROW_IN},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, Tone, oklch, radius, text},
    story,
};

pub const NAME: &str = "Agent Tree";
pub const DESCRIPTION: &str = "A multi-agent run as a live outline – status, spend and runtime on every agent, branch costs rolled up, folding branches and an inspector for the selected agent";

const ICON_TREE: &str = "icons/agent-tree/tree.svg";
const ICON_CHEVRON: &str = "icons/agent-tree/chevron.svg";
const ICON_STOP: &str = "icons/agent-tree/stop.svg";
const ICON_RETRY: &str = "icons/agent-tree/retry.svg";
const ICON_LOG: &str = "icons/agent-tree/log.svg";
const ICON_TICK: &str = "icons/agent-tree/tick.svg";
const ICON_CROSS: &str = "icons/agent-tree/cross.svg";
const ICON_RING: &str = "icons/agent-tree/ring.svg";

/// Row height of the outline.
const ROW_H: f32 = 48.;
/// Horizontal step per tree level.
const INDENT: f32 = 28.;
/// Inset of the first level from the outline's left edge.
const PAD_X: f32 = 10.;
const AVATAR: f32 = 26.;
const ELBOW_H: f32 = 10.;
const INSPECTOR_W: f32 = 268.;
const FOLD_W: f32 = 36.;
const FIGURES_W: f32 = 72.;
const TREE_W: f32 = 524.;

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

    fn tone(self) -> Tone {
        match self {
            AgentStatus::Queued | AgentStatus::Stopped => Tone::Neutral,
            AgentStatus::Running => Tone::Accent,
            AgentStatus::Done => Tone::Success,
            AgentStatus::Failed => Tone::Danger,
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
    /// Hue (OKLCH degrees) of the monogram.
    pub hue: f32,
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
            hue: 277.,
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

    pub fn hue(mut self, degrees: f32) -> Self {
        self.hue = degrees;
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

    fn initials(&self) -> String {
        let mut words = self.name.split_whitespace();
        let first = words.next().and_then(|w| w.chars().next());
        let second = words.next().and_then(|w| w.chars().next());
        match (first, second) {
            (Some(a), Some(b)) => format!("{}{}", a.to_ascii_uppercase(), b.to_ascii_uppercase()),
            (Some(a), None) => a.to_ascii_uppercase().to_string(),
            _ => "?".into(),
        }
    }
}

fn money(dollars: f32) -> String {
    format!("${:.2}", dollars.max(0.))
}

fn runtime(seconds: u32) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    }
}

fn token_count(tokens: u32) -> String {
    if tokens >= 1000 {
        format!("{:.1}K", tokens as f32 / 1000.)
    } else {
        tokens.to_string()
    }
}

/// Monogram wash and ink for a hue.
fn hue_colors(hue: f32, t: &Pink) -> (Hsla, Hsla) {
    if t.dark {
        (oklch(0.30, 0.055, hue), oklch(0.86, 0.10, hue))
    } else {
        (oklch(0.945, 0.035, hue), oklch(0.47, 0.13, hue))
    }
}

/// Rows the subtree under `node` currently shows (itself included).
fn visible_rows(node: &AgentNode, folded: &HashSet<SharedString>) -> usize {
    1 + if folded.contains(&node.id) {
        0
    } else {
        node.children
            .iter()
            .map(|c| visible_rows(c, folded))
            .sum::<usize>()
    }
}

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

struct TreeState {
    folded: HashSet<SharedString>,
    selected: Option<SharedString>,
    focus: FocusHandle,
}

/// A live outline of an orchestrator and the sub-agents it started.
#[derive(IntoElement)]
pub struct AgentTree {
    id: ElementId,
    root: AgentNode,
    title: SharedString,
    selected: Option<SharedString>,
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

    /// "Log" in the inspector.
    pub fn on_open_log(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_log = Some(Rc::new(f));
        self
    }
}

/// What every row needs to know about the tree around it.
struct Ctx<'a> {
    t: &'a Pink,
    key: &'a str,
    folded: &'a HashSet<SharedString>,
    selected: &'a SharedString,
    focused: bool,
    state: &'a Entity<TreeState>,
    reduced: bool,
}

impl RenderOnce for AgentTree {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial = self.selected.clone();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| TreeState {
                folded: HashSet::new(),
                selected: initial,
                focus: cx.focus_handle(),
            },
        );
        let (folded, selected, focus) = {
            let s = state.read(cx);
            (s.folded.clone(), s.selected.clone(), s.focus.clone())
        };
        let root = self.root;
        let selected = selected
            .filter(|id| root.find(id).is_some())
            .unwrap_or_else(|| root.id.clone());
        let focused = focus.is_focused(window);
        let reduced = motion::reduced_motion(cx);

        let ctx = Ctx {
            t: &t,
            key: &key,
            folded: &folded,
            selected: &selected,
            focused,
            state: &state,
            reduced,
        };
        let mut ix = 0;
        let outline = branch(&root, 0, &mut ix, &ctx);

        let mut visible = Vec::new();
        flatten(&root, None, &folded, &mut visible);
        let key_state = state.clone();
        let on_key = move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
            let current = key_state.read(cx).selected.clone();
            let Some(pos) = visible
                .iter()
                .position(|v| Some(&v.id) == current.as_ref())
                .or(Some(0))
            else {
                return;
            };
            let row = &visible[pos];
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
                        if fold {
                            s.folded.insert(id);
                        } else {
                            s.folded.remove(&id);
                        }
                    }
                }
                cx.notify();
            });
            cx.stop_propagation();
        };

        let tree = div()
            .id("outline")
            .track_focus(&focus)
            .on_key_down(on_key)
            .w(px(TREE_W))
            .flex_none()
            .p(px(6.))
            .flex()
            .flex_col()
            .child(outline);

        let inspector = self.inspector.then(|| {
            let node = root.find(&selected).unwrap_or(&root);
            let parent = root.parent_of(&selected).map(|p| p.name.clone());
            inspector(
                node,
                parent,
                root.branch_cost(),
                &key,
                &t,
                self.on_stop.clone(),
                self.on_retry.clone(),
                self.on_open_log.clone(),
            )
        });

        let el = pink::card(&t)
            .id(self.id.clone())
            .w(px(TREE_W + if self.inspector { INSPECTOR_W } else { 0. }))
            .font_family(pink::SANS)
            .text_size(px(text::BODY))
            .text_color(t.text)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(header(&root, &self.title, &key, &t))
            .child(pink::divider(&t))
            .child(div().flex().child(tree).children(inspector));
        motion::keep_alive(window);
        el
    }
}

/// Title, run counts and the total spend.
fn header(root: &AgentNode, title: &SharedString, key: &str, t: &Pink) -> impl IntoElement {
    let total = motion::follow(&format!("{key}-total"), root.branch_cost(), GLIDE);
    let agents = 1 + root.descendants();
    let running = root.count(AgentStatus::Running);
    let failed = root.count(AgentStatus::Failed);
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .h(px(56.))
        .child(
            div()
                .flex_none()
                .size(px(30.))
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Tertiary))
                .border_1()
                .border_color(t.border)
                .flex()
                .items_center()
                .justify_center()
                .child(svg().path(ICON_TREE).size(px(16.)).text_color(t.text_muted)),
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
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(20.))
                        .truncate()
                        .child(title.clone()),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .text_color(t.text_faint)
                        .child(format!(
                            "{agents} agents · {} elapsed",
                            runtime(root.elapsed)
                        )),
                ),
        )
        .when(running > 0, |d| {
            d.child(pink::badge(format!("{running} running"), Tone::Accent, t))
        })
        .when(failed > 0, |d| {
            d.child(pink::badge(format!("{failed} failed"), Tone::Danger, t))
        })
        .child(
            div()
                .flex_none()
                .min_w(px(64.))
                .flex()
                .justify_end()
                .font_family(pink::MONO)
                .text_size(px(text::CALLOUT))
                .font_weight(FontWeight::MEDIUM)
                .child(money(total)),
        )
}

/// A node's row and, under it, its sub-agents on a rail that folds away.
fn branch(node: &AgentNode, depth: usize, ix: &mut usize, c: &Ctx) -> AnyElement {
    let row_ix = *ix;
    *ix += 1;
    let row = row(node, depth, row_ix, c);
    if node.children.is_empty() {
        return row.into_any_element();
    }
    let open = !c.folded.contains(&node.id);
    let inner: usize = node
        .children
        .iter()
        .map(|ch| visible_rows(ch, c.folded))
        .sum();
    let height = motion::follow_from(
        &format!("{}-kids-{}", c.key, node.id),
        0.,
        if open { inner as f32 * ROW_H } else { 0. },
        GLIDE,
    );
    // The rail runs from under the monogram to the last child's elbow.
    let before: usize = node.children[..node.children.len() - 1]
        .iter()
        .map(|ch| visible_rows(ch, c.folded))
        .sum();
    let rail_top = ROW_H / 2. + AVATAR / 2. + 3.;
    let rail_end = ROW_H + before as f32 * ROW_H + ROW_H / 2. - ELBOW_H;
    let rail = motion::follow_from(
        &format!("{}-rail-{}", c.key, node.id),
        0.,
        if open { rail_end - rail_top } else { 0. },
        GLIDE,
    );
    let kids: Vec<AnyElement> = node
        .children
        .iter()
        .map(|ch| branch(ch, depth + 1, ix, c))
        .collect();
    let rail_color = if node.status == AgentStatus::Running {
        motion::mix(c.t.border_strong, c.t.accent, 0.45)
    } else {
        c.t.border_strong
    };
    div()
        .relative()
        .flex_none()
        .flex()
        .flex_col()
        .child(row)
        .child(
            div()
                .flex_none()
                .h(px(height.max(0.)))
                .overflow_hidden()
                .flex()
                .flex_col()
                .children(kids),
        )
        .child(
            div()
                .absolute()
                .left(px(rail_x(depth)))
                .top(px(rail_top))
                .w(px(1.))
                .h(px(rail.max(0.)))
                .bg(rail_color),
        )
        .into_any_element()
}

/// X of the rail under a node at `depth` (its monogram's center line).
fn rail_x(depth: usize) -> f32 {
    PAD_X + depth as f32 * INDENT + (AVATAR / 2.).floor()
}

fn row(node: &AgentNode, depth: usize, row_ix: usize, c: &Ctx) -> impl IntoElement {
    let t = c.t;
    let id = node.id.clone();
    let is_selected = *c.selected == node.id;
    let hover_key = format!("{}-row-{}", c.key, id);
    let has_kids = !node.children.is_empty();
    let open = has_kids && !c.folded.contains(&node.id);
    let shown_cost = if has_kids {
        node.branch_cost()
    } else {
        node.cost
    };
    let cost = motion::follow(&format!("{}-cost-{}", c.key, id), shown_cost, GLIDE);
    let (rest, hover) = if is_selected {
        (t.fill(FillTier::Tertiary), t.fill(FillTier::Secondary))
    } else {
        (
            t.fill(FillTier::Tertiary).opacity(0.),
            t.fill(FillTier::Tertiary),
        )
    };
    let state = c.state.clone();
    let select_id = id.clone();

    let elbow = (depth > 0).then(|| {
        let left = rail_x(depth - 1);
        let right = PAD_X + depth as f32 * INDENT - 3.;
        div()
            .absolute()
            .left(px(left))
            .top(px(ROW_H / 2. - ELBOW_H))
            .w(px(right - left))
            .h(px(ELBOW_H))
            .border_l_1()
            .border_b_1()
            .rounded_bl(px(7.))
            .border_color(t.border_strong)
    });

    let fold = has_kids.then(|| {
        let fold_key = format!("{}-fold-{}", c.key, id);
        let angle = motion::follow(
            &format!("{fold_key}-angle"),
            if open { 0. } else { -0.25 },
            MENU_IN,
        );
        let state = c.state.clone();
        let fold_id = id.clone();
        let count = node.descendants();
        div()
            .id(eid(format!("fold-{id}")))
            .flex_none()
            .w(px(FOLD_W))
            .h(px(20.))
            .pl(px(3.))
            .pr(px(6.))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(radius::SM))
            .text_size(px(text::CAPTION))
            .font_family(pink::MONO)
            .cursor_pointer()
            .hover_fill_fg(
                fold_key.clone(),
                t.fill(FillTier::Quaternary),
                t.fill(FillTier::Secondary),
                t.text_faint,
                t.text,
            )
            .child(
                svg()
                    .path(ICON_CHEVRON)
                    .size(px(13.))
                    .text_color(motion::hover_blend(&fold_key, t.text_faint, t.text))
                    .with_transformation(Transformation::rotate(percentage(angle.rem_euclid(1.)))),
            )
            .child(if open {
                count.to_string()
            } else {
                format!("+{count}")
            })
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                state.update(cx, |s, cx| {
                    if !s.folded.remove(&fold_id) {
                        s.folded.insert(fold_id.clone());
                    }
                    cx.notify();
                });
            })
    });

    let name_line = div()
        .flex()
        .items_center()
        .gap(px(6.))
        .min_w_0()
        .child(
            div()
                .flex_none()
                .text_size(px(text::BODY))
                .line_height(px(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(node.name.clone()),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_size(px(text::CAPTION))
                .line_height(px(18.))
                .text_color(t.text_faint)
                .child(node.model.clone()),
        );
    // Text columns get an explicit width: GPUI truncates nowrap text at
    // the first width it is measured with, which inside nested flex
    // columns is not the final one.
    let text_w = (TREE_W - 12.)
        - 2.
        - (PAD_X + depth as f32 * INDENT)
        - 10.
        - AVATAR
        - FIGURES_W
        - 10. * 2.
        - if has_kids { FOLD_W + 10. } else { 0. };
    let task_line = div()
        .w(px(text_w))
        .truncate()
        .text_size(px(text::FOOTNOTE))
        .line_height(px(16.))
        .text_color(t.text_muted)
        .child(node.task.clone());

    let figures = div()
        .flex_none()
        .w(px(FIGURES_W))
        .flex()
        .flex_col()
        .items_end()
        .font_family(pink::MONO)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(3.))
                .text_size(px(text::FOOTNOTE))
                .line_height(px(18.))
                .text_color(t.text)
                .when(has_kids, |d| {
                    d.child(div().text_color(t.text_faint).child("Σ"))
                })
                .child(money(cost)),
        )
        .child(
            div()
                .text_size(px(text::CAPTION))
                .line_height(px(16.))
                .text_color(if node.status == AgentStatus::Running {
                    t.text_muted
                } else {
                    t.text_faint
                })
                .child(runtime(node.elapsed)),
        );

    let el = div()
        .id(eid(format!("row-{id}")))
        .relative()
        .flex_none()
        .h(px(ROW_H))
        .w(px(TREE_W - 12.))
        .pl(px(PAD_X + depth as f32 * INDENT))
        .pr(px(10.))
        .flex()
        .items_center()
        .gap(px(10.))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(if is_selected && c.focused {
            t.focus_ring()
        } else {
            gpui::transparent_black()
        })
        .cursor_pointer()
        .hover_fill(hover_key, rest, hover)
        .on_click(move |_, window, cx| {
            state.update(cx, |s, cx| {
                s.selected = Some(select_id.clone());
                window.focus(&s.focus, cx);
                cx.notify();
            });
        })
        .children(elbow)
        .child(avatar(node, AVATAR, c.key, t, c.reduced))
        .child(
            div()
                .flex_none()
                .w(px(text_w))
                .flex()
                .flex_col()
                .child(name_line)
                .child(task_line),
        )
        .children(fold)
        .child(figures);

    let delay = (row_ix as u64 * 45).min(400);
    motion::rise_in(
        eid(format!("{}-in-{}", c.key, id)),
        ROW_IN.with_delay(delay),
        6.,
        el,
    )
}

/// Monogram with a status badge in its corner.
fn avatar(node: &AgentNode, size: f32, key: &str, t: &Pink, reduced: bool) -> Div {
    let (wash, ink) = hue_colors(node.hue, t);
    let badge_size = if size > 28. { 14. } else { 12. };
    let badge = status_badge(node, badge_size, key, t, reduced);
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .rounded(px(radius::SM + if size > 28. { 2. } else { 0. }))
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(if size > 28. { 12. } else { 10.5 }))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(node.initials())
        .child(div().absolute().right(px(-4.)).bottom(px(-4.)).child(badge))
}

fn status_badge(node: &AgentNode, size: f32, key: &str, t: &Pink, reduced: bool) -> AnyElement {
    let ring = div()
        .size(px(size + 4.))
        .rounded_full()
        .bg(t.surface_card)
        .flex()
        .items_center()
        .justify_center();
    let glyph =
        |path: &'static str, color: Hsla| svg().path(path).size(px(size - 3.)).text_color(color);
    match node.status {
        AgentStatus::Running => {
            let spinner = svg().path(ICON_RING).size(px(size)).text_color(t.accent);
            let spinner = if reduced {
                spinner.into_any_element()
            } else {
                spinner
                    .with_animation(
                        eid(format!("{key}-spin-{}", node.id)),
                        Animation::new(Duration::from_millis(900)).repeat(),
                        |s, d| s.with_transformation(Transformation::rotate(percentage(d))),
                    )
                    .into_any_element()
            };
            ring.child(spinner).into_any_element()
        }
        AgentStatus::Queued => ring
            .child(
                div()
                    .size(px(size - 2.))
                    .rounded_full()
                    .border_1()
                    .border_color(t.text_faint),
            )
            .into_any_element(),
        status => {
            let (fill, path) = match status {
                AgentStatus::Done => (t.success, ICON_TICK),
                AgentStatus::Failed => (t.danger, ICON_CROSS),
                _ => (t.text_faint, ICON_CROSS),
            };
            let dot = div()
                .size(px(size))
                .rounded_full()
                .bg(fill)
                .flex()
                .items_center()
                .justify_center()
                .child(glyph(path, if t.dark { t.on_solid } else { t.on_accent }));
            let dot = dot.with_animation(
                eid(format!("{key}-badge-{}-{}", node.id, status.label())),
                MENU_IN.with_duration(260).animation(),
                move |el, p| {
                    let s = 0.4 + 0.6 * motion::EASE_OUT_BACK.eval(p);
                    el.opacity(p).size(px(size * s))
                },
            );
            ring.child(dot).into_any_element()
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn inspector(
    node: &AgentNode,
    parent: Option<SharedString>,
    run_total: f32,
    key: &str,
    t: &Pink,
    on_stop: Option<IdHandler>,
    on_retry: Option<IdHandler>,
    on_open_log: Option<IdHandler>,
) -> impl IntoElement {
    let branch = node.branch_cost();
    let share = if run_total > 0. {
        (branch / run_total).clamp(0., 1.)
    } else {
        0.
    };
    let share_v = motion::follow(&format!("{key}-share"), share, GLIDE);
    let running_kids =
        node.count(AgentStatus::Running) - usize::from(node.status == AgentStatus::Running);
    let stat = |label: &'static str, value: String| {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(26.))
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(label),
            )
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    // Figures in mono, names in the interface face.
                    .when(
                        value.starts_with(|c: char| c.is_ascii_digit() || c == '$'),
                        |d| d.font_family(pink::MONO),
                    )
                    .text_color(t.text)
                    .child(value),
            )
    };
    let id = node.id.clone();
    let actions = {
        let mut row = div().flex().gap(px(6.)).mt(px(14.));
        match node.status {
            AgentStatus::Running | AgentStatus::Queued => {
                let id = id.clone();
                row = row.child(
                    pink::button(
                        format!("{key}-stop"),
                        ButtonKind::Secondary,
                        ControlSize::Md,
                        t,
                    )
                    .flex_1()
                    .child(pink::glyph(ICON_STOP, ControlSize::Md).text_color(t.text_muted))
                    .child("Stop")
                    .when_some(on_stop, |b, f| {
                        b.on_click(move |_, window, cx| f(&id, window, cx))
                    }),
                );
            }
            AgentStatus::Failed | AgentStatus::Stopped => {
                let id = id.clone();
                row = row.child(
                    pink::button(
                        format!("{key}-retry"),
                        ButtonKind::Primary,
                        ControlSize::Md,
                        t,
                    )
                    .flex_1()
                    .child(pink::glyph(ICON_RETRY, ControlSize::Md).text_color(t.on_solid))
                    .child("Retry")
                    .when_some(on_retry, |b, f| {
                        b.on_click(move |_, window, cx| f(&id, window, cx))
                    }),
                );
            }
            AgentStatus::Done => {}
        }
        row.child(
            pink::button(format!("{key}-log"), ButtonKind::Ghost, ControlSize::Md, t)
                .flex_1()
                .child(pink::glyph(ICON_LOG, ControlSize::Md).text_color(t.text_muted))
                .child("Open log")
                .when_some(on_open_log, |b, f| {
                    b.on_click(move |_, window, cx| f(&id, window, cx))
                }),
        )
    };

    let content = div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .child(avatar(node, 34., &format!("{key}-insp"), t, false))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(text::CALLOUT))
                                .font_weight(FontWeight::SEMIBOLD)
                                .line_height(px(20.))
                                .truncate()
                                .child(node.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .truncate()
                                .child(node.model.clone()),
                        ),
                )
                .child(pink::badge(node.status.label(), node.status.tone(), t)),
        )
        .child(
            div()
                .mt(px(14.))
                .text_size(px(text::CAPTION))
                .text_color(t.text_faint)
                .child("Task"),
        )
        .child(
            div()
                .mt(px(2.))
                .text_size(px(text::BODY))
                .line_height(px(19.))
                .text_color(t.text)
                .child(node.task.clone()),
        )
        .child(div().mt(px(12.)).mb(px(4.)).child(pink::divider(t)))
        .child(stat(
            "Spawned by",
            parent
                .map(|p| p.to_string())
                .unwrap_or_else(|| "you".into()),
        ))
        .child(stat("Runtime", runtime(node.elapsed)))
        .child(stat("Tokens", token_count(node.tokens)))
        .child(stat("Own spend", money(node.cost)))
        .when(!node.children.is_empty(), |d| {
            d.child(stat(
                "Sub-agents",
                if running_kids > 0 {
                    format!("{} · {running_kids} live", node.descendants())
                } else {
                    node.descendants().to_string()
                },
            ))
            .child(stat("Branch spend", money(branch)))
        })
        .child(
            div()
                .mt(px(8.))
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(text::CAPTION))
                .text_color(t.text_faint)
                .child("Share of run")
                .child(
                    div()
                        .font_family(pink::MONO)
                        .child(format!("{:.0}%", share_v * 100.)),
                ),
        )
        .child(
            div()
                .mt(px(6.))
                .h(px(4.))
                .w_full()
                .rounded(px(2.))
                .bg(t.fill(FillTier::Secondary))
                .child(
                    div()
                        .h_full()
                        .w(relative(share_v))
                        .rounded(px(2.))
                        .bg(t.accent),
                ),
        )
        .child(actions);

    div()
        .flex_none()
        .w(px(INSPECTOR_W))
        .border_l_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .p(px(16.))
        .child(motion::rise_in(
            eid(format!("{key}-insp-{}", node.id)),
            motion::VIEW_IN,
            4.,
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
            .hue(277.)
            .child(
                AgentNode::new("auditor", "Schema auditor")
                    .model(MINI)
                    .task("Diff all 42 tables against the v16 type changes")
                    .status(AgentStatus::Running)
                    .cost(0.04)
                    .elapsed(51)
                    .tokens(21_900)
                    .hue(200.),
            )
            .child(
                AgentNode::new("rewriter", "Query rewriter")
                    .model(LARGE)
                    .task("Rewrite the 118 raw SQL calls that rely on implicit casts")
                    .status(AgentStatus::Running)
                    .cost(0.12)
                    .elapsed(44)
                    .tokens(30_400)
                    .hue(32.),
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
                    .task("Flag indexes that v16 would rebuild during the upgrade")
                    .hue(150.),
            ),
            3 => self.spawn(
                "rewriter",
                AgentNode::new("tests", "Test runner")
                    .model(MINI)
                    .task("Run the integration suite against a v16 replica")
                    .hue(110.),
            ),
            5 => self.set("indexes", AgentStatus::Done),
            6 => self.spawn(
                "lead",
                AgentNode::new("docs", "Docs writer")
                    .model(MINI)
                    .task("Update the runbook and the rollback notes")
                    .hue(330.),
            ),
            7 => self.set("auditor", AgentStatus::Done),
            8 => self.spawn(
                "rewriter",
                AgentNode::new("profiler", "Perf profiler")
                    .model(MINI)
                    .task("Compare p95 latency of the 20 hottest queries")
                    .hue(250.),
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
        .task("Re-run the 6 failing invoice tests with fixed fixtures")
        .hue(node.hue);
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
            .status(AgentStatus::Running)
            .child(
                AgentNode::new("a", "A")
                    .cost(0.25)
                    .status(AgentStatus::Done)
                    .child(AgentNode::new("a1", "A one").cost(0.125)),
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
        assert_eq!(root.descendants(), 3);
        assert_eq!(root.count(AgentStatus::Failed), 1);
        assert_eq!(root.parent_of("a1").unwrap().id.as_ref(), "a");
    }

    #[test]
    fn folding_hides_descendant_rows() {
        let root = run();
        let mut folded = HashSet::new();
        assert_eq!(visible_rows(&root, &folded), 4);
        folded.insert(SharedString::from("a"));
        assert_eq!(visible_rows(&root, &folded), 3);
        let mut rows = Vec::new();
        flatten(&root, None, &folded, &mut rows);
        let ids: Vec<_> = rows.iter().map(|r| r.id.to_string()).collect();
        assert_eq!(ids, ["lead", "a", "b"]);
    }
}
