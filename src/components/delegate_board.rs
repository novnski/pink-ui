//! Delegate Board – hand work to agents by dropping it on them.
//!
//! Drawn as pink's Board page (`shell/board`): a page title with its count
//! and help, a toolbar, then borderless column wells side by side. The first
//! column is the Backlog; every other column is an agent and its queue (at
//! most [`QUEUE_LIMIT`] tasks, the first one running). An agent column also
//! carries a load meter on a shared 90-minute horizon and says when the agent
//! is free again.
//!
//! Dragging a task over a column previews the drop before you let go: the
//! column takes pink's accent wash, the meter grows an accent segment, the
//! free-at time moves and pink's 2 px insertion line marks where the card
//! lands. A full queue refuses with a danger wash. Queued tasks can be
//! dragged on to another agent or back to the Backlog, or moved with
//! Alt+Left / Alt+Right from the keyboard. With Auto-assign on, the host
//! hands each new task to whoever is free first ([`first_free`]); the column
//! that took it flashes the accent wash and a notice says why.
//!
//! ```ignore
//! DelegateBoard::new("board", agents, backlog, 14 * 60 + 5)
//!     .on_assign(|task, target, _, _| println!("{task} → {target:?}"))
//!     .on_auto(|on, _, _| println!("auto {on}"))
//! ```

use std::{rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyElement, AnyView, App, Context, Div, ElementId, Entity, FocusHandle,
    FontWeight, Hsla, IntoElement, KeyDownEvent, Render, Role, SharedString, Stateful, Task,
    Window, div, prelude::*, px, relative, svg,
};

use crate::{
    motion::{self, CURVE_STANDARD, EMPHASIZED, MotionSpec, QUICK, TOOL_ROW_REVEAL},
    pink::{
        self, ControlSize, FillTier, Pink, TextStyle, TypeStyleExt, layout, loaders, radius,
        tabular_figures,
    },
    story,
};

pub const NAME: &str = "Delegate Board";
pub const DESCRIPTION: &str = "Hand work to agents by dragging tasks from the Backlog onto their columns – each column previews the agent's new free-at time before you let go, and Auto-assign gives new work to whoever frees up first";

const ICON_TICKET: &str = "icons/delegate-board/ticket.svg";
const ICON_QUEUED: &str = "icons/delegate-board/clock-circle.svg";
const ICON_WORKING: &str = "icons/delegate-board/bolt-fast.svg";
const ICON_REFUSED: &str = "icons/delegate-board/danger-circle.svg";
const ICON_INFO: &str = "icons/delegate-board/info-circle.svg";
const ICON_AUTO: &str = "icons/delegate-board/stopwatch-play.svg";

/// A column's width (pink: board/columns.rs `COLUMN_MIN_WIDTH`).
const COLUMN_W: f32 = 220.;
/// Between columns (pink: `render_columns`).
const COLUMN_GAP: f32 = 12.;
/// Between the header, the notice and the columns (pink: the Board page's
/// scroll column).
const PAGE_GAP: f32 = 14.;
/// Tasks an agent holds at most, the running one included.
pub const QUEUE_LIMIT: usize = 3;
/// Minutes the load meters span.
const HORIZON: f32 = 90.;
/// The flash on a column Auto-assign just fed: the accent wash holds, then
/// eases back to the resting well.
const PULSE: MotionSpec = MotionSpec::new(600, CURVE_STANDARD).with_delay(400);

type AssignHandler = Rc<dyn Fn(&SharedString, &DropTarget, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// A unit of work.
#[derive(Clone, Debug)]
pub struct BoardTask {
    pub id: SharedString,
    pub title: SharedString,
    pub tag: SharedString,
    /// Estimated minutes.
    pub minutes: u32,
    /// Minutes already worked (the running task).
    pub done: u32,
}

impl BoardTask {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        tag: impl Into<SharedString>,
        minutes: u32,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            tag: tag.into(),
            minutes: minutes.max(1),
            done: 0,
        }
    }

    pub fn done(mut self, minutes: u32) -> Self {
        self.done = minutes.min(self.minutes);
        self
    }

    pub fn left(&self) -> u32 {
        self.minutes.saturating_sub(self.done)
    }
}

/// An agent and its queue; the first task is the one it is working on.
#[derive(Clone, Debug)]
pub struct BoardAgent {
    pub id: SharedString,
    pub name: SharedString,
    pub model: SharedString,
    pub queue: Vec<BoardTask>,
}

impl BoardAgent {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        model: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            model: model.into(),
            queue: Vec::new(),
        }
    }

    pub fn task(mut self, task: BoardTask) -> Self {
        self.queue.push(task);
        self
    }

    /// Minutes until the queue is empty.
    pub fn busy(&self) -> u32 {
        self.queue.iter().map(BoardTask::left).sum()
    }

    fn has_room(&self) -> bool {
        self.queue.len() < QUEUE_LIMIT
    }
}

/// The agent with room in its queue that frees up first (ties go to the
/// earlier column).
pub fn first_free(agents: &[BoardAgent]) -> Option<&BoardAgent> {
    agents
        .iter()
        .filter(|a| a.has_room())
        .min_by_key(|a| a.busy())
}

/// Where a task was dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DropTarget {
    Agent(SharedString),
    Backlog,
}

/// The drag payload.
#[derive(Clone, Debug)]
struct DraggedTask {
    task: BoardTask,
    from: DropTarget,
}

/// What follows the pointer while a task is dragged: its tag and title
/// (pink: board/card.rs `BoardCardGhost`).
struct Ghost {
    tag: SharedString,
    title: SharedString,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        div()
            .font_family(pink::SANS)
            .max_w(px(260.))
            .px(px(10.))
            .py(px(7.))
            .rounded(px(radius::LG))
            .border_1()
            .border_color(t.accent.opacity(0.6))
            .bg(t.surface_raised)
            .shadow(t.menu_shadows())
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.readable_muted(0.85))
                    .child(self.tag.clone()),
            )
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .truncate()
                    .child(self.title.clone()),
            )
    }
}

#[derive(Default)]
struct BoardState {
    over: Option<DropTarget>,
    dragging: Option<SharedString>,
    selected: Option<SharedString>,
}

/// A board for handing tasks to agents.
#[derive(IntoElement)]
pub struct DelegateBoard {
    id: ElementId,
    agents: Vec<BoardAgent>,
    backlog: Vec<BoardTask>,
    /// Minutes since midnight.
    clock: u32,
    auto: bool,
    note: Option<SharedString>,
    pulse: Option<(SharedString, Instant)>,
    on_assign: Option<AssignHandler>,
    on_auto: Option<ToggleHandler>,
}

impl DelegateBoard {
    pub fn new(
        id: impl Into<ElementId>,
        agents: Vec<BoardAgent>,
        backlog: Vec<BoardTask>,
        clock: u32,
    ) -> Self {
        Self {
            id: id.into(),
            agents,
            backlog,
            clock,
            auto: false,
            note: None,
            pulse: None,
            on_assign: None,
            on_auto: None,
        }
    }

    /// Whether Auto-assign is on.
    pub fn auto(mut self, on: bool) -> Self {
        self.auto = on;
        self
    }

    /// A notice under the header, e.g. why Auto-assign picked an agent.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Flashes an agent's column (after an automatic assignment).
    pub fn pulse(mut self, agent: impl Into<SharedString>, at: Instant) -> Self {
        self.pulse = Some((agent.into(), at));
        self
    }

    /// A task was dropped (or moved with Alt+arrows) on an agent or back on
    /// the Backlog.
    pub fn on_assign(
        mut self,
        f: impl Fn(&SharedString, &DropTarget, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_assign = Some(Rc::new(f));
        self
    }

    pub fn on_auto(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_auto = Some(Rc::new(f));
        self
    }
}

fn clock_label(minutes: u32) -> String {
    format!("{:02}:{:02}", (minutes / 60) % 24, minutes % 60)
}

// ---------------------------------------------------------------------------
// Pieces (pink: board/card.rs, board/columns.rs, board/page.rs)
// ---------------------------------------------------------------------------

/// How a badge is toned (pink: board/card.rs `Tone`). Tone is a secondary
/// cue only: the glyph and the word carry the meaning.
#[derive(Clone, Copy)]
enum BadgeTone {
    /// Nothing is running.
    Quiet,
    /// An agent is working on it.
    Running,
    Danger,
}

impl BadgeTone {
    fn color(self, t: &Pink) -> Hsla {
        match self {
            BadgeTone::Quiet => t.readable_muted(0.9),
            BadgeTone::Running => t.readable_tone(t.accent, 1.0),
            BadgeTone::Danger => t.readable_tone(t.danger, 0.95),
        }
    }
}

/// A small glyph + word badge; a ring, never a filled dot, when nothing has
/// started (pink: board/card.rs `badge`).
fn badge(
    glyph: Option<&'static str>,
    label: impl Into<SharedString>,
    tone: BadgeTone,
    t: &Pink,
) -> Div {
    let color = tone.color(t);
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .min_w_0()
        .type_style(TextStyle::Caption)
        .text_color(color)
        .child(match glyph {
            Some(path) => svg()
                .path(path)
                .size(px(12.))
                .flex_none()
                .text_color(color)
                .into_any_element(),
            None => div()
                .flex_none()
                .size(px(8.))
                .rounded_full()
                .border_1()
                .border_color(color)
                .into_any_element(),
        })
        .child(div().min_w_0().child(label.into()))
}

/// The status mark beside a column title: a muted ring while nothing runs
/// (pink's Backlog), a filled accent dot while the agent works (pink's
/// Ready). Pink: board/card.rs `column_mark`.
fn column_mark(running: bool, t: &Pink) -> Div {
    let mark = div()
        .flex_none()
        .size(px(8.))
        .rounded_full()
        .border(px(1.5));
    if running {
        mark.border_color(t.accent).bg(t.accent)
    } else {
        mark.border_color(t.readable_muted(0.8))
    }
}

/// A column's title, count and help (pink: board/columns.rs `column_header`).
fn column_header(
    running: bool,
    label: SharedString,
    count: String,
    help: SharedString,
    t: &Pink,
) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .px(px(2.))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(column_mark(running, t))
                .child(label)
                .child(
                    div()
                        .font_weight(FontWeight::NORMAL)
                        .text_color(t.readable_muted(0.7))
                        .child(count),
                ),
        )
        .child(
            div()
                .type_style(TextStyle::Caption)
                .text_color(t.readable_muted(0.8))
                .child(help),
        )
}

/// A column well: borderless, `fill(Quaternary)` at half strength, radius
/// 10, `p 8`, its parts 8 apart (pink: board/columns.rs `render_column`).
fn column_well(id: ElementId, aria: String, wash: Hsla) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Group)
        .aria_label(aria)
        .flex_1()
        .min_w(px(COLUMN_W))
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(8.))
        .rounded(px(radius::LG))
        .bg(wash)
}

/// The resting well and the washes it takes under a drag: accent where the
/// drop lands, danger at 6% where it is refused.
fn well_washes(t: &Pink) -> (Hsla, Hsla, Hsla) {
    (
        t.fill(FillTier::Quaternary).opacity(0.5),
        t.accent_wash,
        t.danger.opacity(0.06),
    )
}

/// An empty column's hint (pink: board/columns.rs `render_column`).
fn empty_hint(copy: &'static str, t: &Pink) -> Div {
    div()
        .px(px(4.))
        .py(px(14.))
        .text_size(px(11.))
        .line_height(px(15.))
        .text_color(t.readable_muted(0.7))
        .child(copy)
}

/// The insertion line where a dragged card would land (pink: board/card.rs
/// `drop_indicator`).
fn drop_indicator(t: &Pink) -> Div {
    div().w_full().h(px(2.)).rounded_full().bg(t.accent)
}

/// A status strip under the header (pink: board/page.rs `notice_strip`,
/// Info tone).
fn notice_strip(key: &str, note: SharedString, t: &Pink) -> impl IntoElement {
    let color = t.readable_muted(0.95);
    motion::status_swap(
        loaders::label_id(&format!("{key}-notice"), &note),
        div()
            .id(eid(format!("{key}-notice")))
            .role(Role::Status)
            .aria_label(note.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(6.))
            .rounded(px(radius::MD))
            .bg(t.fill(FillTier::Quaternary))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(color)
            .child(
                svg()
                    .path(ICON_INFO)
                    .size(px(14.))
                    .flex_none()
                    .text_color(color),
            )
            .child(div().flex_1().min_w_0().child(note)),
    )
}

/// An agent's load on the shared horizon: pink's usage meter (a 6 px
/// capsule, `fill(Secondary)` track, neutral `text_muted` fill; shell/usage.rs
/// `usage_meter`), with the incoming task as an accent segment while a drag
/// previews it. `label` trails the bar.
fn load_meter(busy: f32, extra: f32, label: impl IntoElement, t: &Pink) -> Div {
    let used = (busy / HORIZON).clamp(0., 1.);
    let with_extra = ((busy + extra) / HORIZON).clamp(0., 1.);
    let segment = |w: f32, color: Hsla| {
        div()
            .absolute()
            .top_0()
            .left_0()
            .h_full()
            .w(relative(w))
            .rounded_full()
            .bg(color)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .px(px(2.))
        .child(
            div()
                .relative()
                .h(px(6.))
                .flex_1()
                .min_w_0()
                .rounded_full()
                .bg(t.fill(FillTier::Secondary))
                // Nothing drawn below 0.5% (pink: no false stub).
                .when(with_extra - used >= 0.005, |bar| {
                    bar.child(segment(with_extra, t.accent))
                })
                .when(used >= 0.005, |bar| bar.child(segment(used, t.text_muted))),
        )
        .child(div().flex_none().child(label))
}

/// Where a card sits, which decides its badge and footer.
enum Phase {
    /// In the Backlog.
    NotStarted,
    /// In an agent's queue behind other work.
    Queued { agent: SharedString, starts: u32 },
    /// The agent's current task.
    Working { agent: SharedString, left: u32 },
}

struct CardArgs<'a> {
    key: &'a str,
    task: &'a BoardTask,
    phase: Phase,
    column: DropTarget,
    lifted: bool,
    selected: bool,
    focus: FocusHandle,
    /// Where Alt+Left / Alt+Right send the card.
    moves: [Option<DropTarget>; 2],
    state: &'a Entity<BoardState>,
    on_assign: Option<AssignHandler>,
}

/// One task card (pink: board/card.rs `render_card`): `px 10 py 9`, gap 8,
/// radius 10, a `separator(true)` edge on `fill(Quaternary)` (accent edge and
/// wash when selected, an accent edge under the pointer), the top line with
/// the ticket glyph, a 13 pt medium title clamped to three lines, the phase
/// badge and a hairline footer. 45% while it is being dragged.
fn task_card(a: CardArgs, t: &Pink) -> AnyElement {
    let task = a.task;
    let id = task.id.clone();
    let ring = t.focus_ring();
    let muted = t.readable_muted(0.85);
    let (phase_badge, description, location, when) = match &a.phase {
        Phase::NotStarted => (
            badge(None, "Not started", BadgeTone::Quiet, t),
            "Not handed to an agent yet",
            "No agent yet".to_string(),
            None,
        ),
        Phase::Queued { agent, starts } => (
            badge(Some(ICON_QUEUED), "Queued", BadgeTone::Quiet, t),
            "Waiting for the agent to finish the work ahead of it",
            format!("With {agent}"),
            Some(format!("Starts {}", clock_label(*starts))),
        ),
        Phase::Working { agent, left } => (
            badge(Some(ICON_WORKING), "Working", BadgeTone::Running, t),
            "The agent is working on it",
            format!("With {agent}"),
            Some(format!("{left}m left")),
        ),
    };
    let draggable = !matches!(a.phase, Phase::Working { .. });
    let label = format!("{}, {}, {}", task.tag, task.title, location);

    let top = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(5.))
        .type_style(TextStyle::Caption)
        .text_color(muted)
        .child(
            svg()
                .path(ICON_TICKET)
                .size(px(12.))
                .flex_none()
                .text_color(t.readable_muted(0.8)),
        )
        .child(div().flex_none().child(task.tag.clone()))
        .child(div().flex_1())
        .child(
            div()
                .flex_none()
                .font_features(tabular_figures())
                .child(format!("Est. {}m", task.minutes)),
        );

    let title = div()
        .min_w_0()
        .text_size(px(13.))
        .line_height(px(18.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text)
        .line_clamp(3)
        .child(task.title.clone());

    let status = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(px(10.))
        .gap_y(px(4.))
        .child(
            phase_badge
                .id(eid(format!("{}-phase-{id}", a.key)))
                .tooltip(pink::text_tooltip(description))
                .tooltip_show_delay(layout::TOOLTIP_DELAY),
        );

    let footer = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .pt(px(6.))
        .border_t_1()
        .border_color(t.separator(false))
        .type_style(TextStyle::Caption)
        .text_color(t.readable_muted(0.8))
        .child(div().flex_1().min_w_0().child(location))
        .when_some(when, |row, when| {
            row.child(
                div()
                    .flex_none()
                    .font_features(tabular_figures())
                    .child(when),
            )
        });

    let click_state = a.state.clone();
    let click_id = id.clone();
    let key_state = a.state.clone();
    let key_id = id.clone();
    let [left, right] = a.moves;
    let on_assign = a.on_assign.clone();
    let hover_edge = t.accent.opacity(0.45);
    let card = div()
        .id(eid(format!("{}-card-{id}", a.key)))
        .role(Role::Button)
        .aria_label(label)
        .aria_description(description)
        .aria_selected(a.selected)
        .when(draggable, |card| {
            card.aria_keyshortcuts("Alt+Right moves to the next column; Alt+Left moves back")
        })
        .track_focus(&a.focus)
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.))
        .px(px(10.))
        .py(px(9.))
        .rounded(px(radius::LG))
        .border_1()
        .border_color(if a.selected {
            t.accent.opacity(0.7)
        } else {
            t.separator(true)
        })
        .bg(if a.selected {
            t.accent_wash
        } else {
            t.fill(FillTier::Quaternary)
        })
        .cursor_pointer()
        .hover(move |style| style.border_color(hover_edge))
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_click(move |_, _, cx| {
            click_state.update(cx, |s, cx| {
                s.selected = Some(click_id.clone());
                cx.notify();
            });
        })
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let keystroke = &event.keystroke;
            if keystroke.key == "escape" {
                key_state.update(cx, |s, cx| {
                    if s.selected.take().is_some() {
                        cx.notify();
                    }
                });
                return;
            }
            if !keystroke.modifiers.alt {
                return;
            }
            let target = match keystroke.key.as_str() {
                "left" => left.clone(),
                "right" => right.clone(),
                _ => None,
            };
            let Some(target) = target else { return };
            cx.stop_propagation();
            key_state.update(cx, |s, cx| {
                s.selected = Some(key_id.clone());
                cx.notify();
            });
            if let Some(f) = on_assign.as_ref() {
                f(&key_id, &target, window, cx);
            }
        })
        .when(draggable, |card| {
            let drag_state = a.state.clone();
            let payload = DraggedTask {
                task: task.clone(),
                from: a.column.clone(),
            };
            card.on_drag(payload, move |dragged: &DraggedTask, _, _, cx| {
                cx.stop_propagation();
                drag_state.update(cx, |s, _| {
                    s.dragging = Some(dragged.task.id.clone());
                    s.over = None;
                });
                let (tag, title) = (dragged.task.tag.clone(), dragged.task.title.clone());
                cx.new(|_| Ghost { tag, title })
            })
        })
        .child(top)
        .child(title)
        .child(status)
        .child(footer);

    // A card arriving in a column (first mount, a drop, Auto-assign) reveals
    // like pink's new rows: TOOL_ROW_REVEAL with a 4 px rise.
    let column = match &a.column {
        DropTarget::Backlog => SharedString::from("backlog"),
        DropTarget::Agent(agent) => agent.clone(),
    };
    let rest_opacity = if a.lifted { 0.45 } else { 1. };
    card.with_animation(
        eid(format!("{}-in-{column}-{id}", a.key)),
        TOOL_ROW_REVEAL.animation(),
        move |el, p| {
            el.relative()
                .opacity(rest_opacity * p)
                .top(px(4. * (1. - p)))
        },
    )
    .into_any_element()
}

// ---------------------------------------------------------------------------
// The board
// ---------------------------------------------------------------------------

/// Drag-move tracking shared by every column: `target` becomes the column
/// under the pointer, or stops being it.
fn track_over(
    state: &Entity<BoardState>,
    target: DropTarget,
) -> impl Fn(&gpui::DragMoveEvent<DraggedTask>, &mut Window, &mut App) + 'static {
    let state = state.clone();
    move |ev, _, cx| {
        let inside = ev.bounds.contains(&ev.event.position);
        state.update(cx, |s, cx| {
            if inside && s.over.as_ref() != Some(&target) {
                s.over = Some(target.clone());
                cx.notify();
            } else if !inside && s.over.as_ref() == Some(&target) {
                s.over = None;
                cx.notify();
            }
        });
    }
}

/// The drop on `target`: clears the drag, selects the card (pink selects
/// what was just moved) and reports the move unless it changes nothing or
/// is refused.
fn drop_on(
    state: &Entity<BoardState>,
    target: DropTarget,
    accepts: bool,
    on_assign: Option<AssignHandler>,
) -> impl Fn(&DraggedTask, &mut Window, &mut App) + 'static {
    let state = state.clone();
    move |dragged, window, cx| {
        let moves = dragged.from != target && accepts;
        state.update(cx, |s, cx| {
            s.over = None;
            s.dragging = None;
            if moves {
                s.selected = Some(dragged.task.id.clone());
            }
            cx.notify();
        });
        if moves && let Some(f) = on_assign.as_ref() {
            f(&dragged.task.id, &target, window, cx);
        }
    }
}

impl RenderOnce for DelegateBoard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| BoardState::default(),
        );
        let dragging_now = cx.has_active_drag();
        let reduced = motion::reduced_motion(cx);
        let (over, dragging, selected) = {
            let s = state.read(cx);
            if dragging_now {
                (s.over.clone(), s.dragging.clone(), s.selected.clone())
            } else {
                (None, None, s.selected.clone())
            }
        };
        // The dragged task, wherever it currently sits.
        let dragged: Option<(BoardTask, DropTarget)> = dragging.as_ref().and_then(|id| {
            self.backlog
                .iter()
                .find(|task| &task.id == id)
                .map(|task| (task.clone(), DropTarget::Backlog))
                .or_else(|| {
                    self.agents.iter().find_map(|a| {
                        a.queue
                            .iter()
                            .find(|task| &task.id == id)
                            .map(|task| (task.clone(), DropTarget::Agent(a.id.clone())))
                    })
                })
        });

        // Column order for Alt+arrow moves, and whether each takes a card.
        let targets: Vec<(DropTarget, bool)> = std::iter::once((DropTarget::Backlog, true))
            .chain(
                self.agents
                    .iter()
                    .map(|a| (DropTarget::Agent(a.id.clone()), a.has_room())),
            )
            .collect();
        let moves_from = |column: usize| -> [Option<DropTarget>; 2] {
            let open = |ix: Option<usize>| {
                ix.and_then(|ix| targets.get(ix))
                    .filter(|(_, room)| *room)
                    .map(|(target, _)| target.clone())
            };
            [open(column.checked_sub(1)), open(Some(column + 1))]
        };
        // One focus handle per task, keyed by the task so focus follows a
        // card that moves to another column.
        let focus_for = |task: &BoardTask, window: &mut Window, cx: &mut App| {
            window
                .use_keyed_state(
                    ElementId::NamedChild(
                        Arc::new(self.id.clone()),
                        format!("focus-{}", task.id).into(),
                    ),
                    cx,
                    |_, cx| cx.focus_handle().tab_stop(true),
                )
                .read(cx)
                .clone()
        };

        let (rest, accept_wash, refuse_wash) = well_washes(&t);

        // Backlog column.
        let backlog_over = over == Some(DropTarget::Backlog)
            && dragged
                .as_ref()
                .is_some_and(|(_, from)| from != &DropTarget::Backlog);
        let backlog_cards = self
            .backlog
            .iter()
            .map(|task| {
                let focus = focus_for(task, window, cx);
                task_card(
                    CardArgs {
                        key: &key,
                        task,
                        phase: Phase::NotStarted,
                        column: DropTarget::Backlog,
                        lifted: dragging.as_ref() == Some(&task.id),
                        selected: selected.as_ref() == Some(&task.id),
                        focus,
                        moves: moves_from(0),
                        state: &state,
                        on_assign: self.on_assign.clone(),
                    },
                    &t,
                )
            })
            .collect::<Vec<_>>();
        let backlog_column = column_well(
            eid(format!("{key}-col-backlog")),
            format!("Backlog, {} tasks waiting for an agent", self.backlog.len()),
            if backlog_over { accept_wash } else { rest },
        )
        .on_drag_move::<DraggedTask>(track_over(&state, DropTarget::Backlog))
        .on_drop::<DraggedTask>(drop_on(
            &state,
            DropTarget::Backlog,
            true,
            self.on_assign.clone(),
        ))
        .child(column_header(
            false,
            "Backlog".into(),
            self.backlog.len().to_string(),
            "Waiting for an agent".into(),
            &t,
        ))
        .when(!backlog_cards.is_empty(), |col| {
            col.child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(backlog_cards),
            )
        })
        .when(backlog_over, |col| col.child(drop_indicator(&t)))
        .when(self.backlog.is_empty(), |col| {
            col.child(empty_hint("Everything is assigned", &t))
        })
        .into_any_element();

        // Agent columns.
        let mut columns = vec![backlog_column];
        for (ix, agent) in self.agents.iter().enumerate() {
            let target = DropTarget::Agent(agent.id.clone());
            let incoming = dragged.as_ref().filter(|(_, from)| from != &target);
            let full = !agent.has_room();
            let hovered = over.as_ref() == Some(&target) && incoming.is_some();
            let previewing = hovered && !full;
            let refusing = hovered && full;
            let extra = if previewing {
                incoming.map_or(0, |(task, _)| task.left())
            } else {
                0
            };
            let busy = agent.busy();
            let busy_v =
                motion::follow(&format!("{key}-busy-{}", agent.id), busy as f32, EMPHASIZED);
            let extra_v = motion::follow(&format!("{key}-extra-{}", agent.id), extra as f32, QUICK);
            let free_at = self.clock + busy + extra;

            // Auto-assign flash: the accent wash holds, then eases back.
            let pulse = self
                .pulse
                .as_ref()
                .filter(|(id, _)| id == &agent.id && !reduced)
                .map(|(_, at)| at.elapsed())
                .filter(|elapsed| *elapsed < PULSE.total().mul_f32(motion::speed_scale()))
                .map_or(0., |elapsed| {
                    window.request_animation_frame();
                    1. - PULSE.progress_at(elapsed)
                });
            let wash = if previewing {
                accept_wash
            } else if refusing {
                refuse_wash
            } else {
                motion::mix(rest, accept_wash, pulse)
            };

            let meter_label = if refusing {
                badge(Some(ICON_REFUSED), "Queue full", BadgeTone::Danger, &t).into_any_element()
            } else {
                let copy = if busy + extra == 0 {
                    "Free now".to_string()
                } else {
                    format!("Free at {}", clock_label(free_at))
                };
                div()
                    .type_style(TextStyle::Caption)
                    .font_features(tabular_figures())
                    .text_color(if previewing {
                        BadgeTone::Running.color(&t)
                    } else {
                        t.readable_muted(0.8)
                    })
                    .child(copy)
                    .into_any_element()
            };

            let mut starts = self.clock;
            let cards = agent
                .queue
                .iter()
                .enumerate()
                .map(|(i, task)| {
                    let phase = if i == 0 {
                        Phase::Working {
                            agent: agent.name.clone(),
                            left: task.left(),
                        }
                    } else {
                        Phase::Queued {
                            agent: agent.name.clone(),
                            starts,
                        }
                    };
                    starts += task.left();
                    let focus = focus_for(task, window, cx);
                    task_card(
                        CardArgs {
                            key: &key,
                            task,
                            moves: if i == 0 {
                                [None, None]
                            } else {
                                moves_from(ix + 1)
                            },
                            phase,
                            column: target.clone(),
                            lifted: dragging.as_ref() == Some(&task.id),
                            selected: selected.as_ref() == Some(&task.id),
                            focus,
                            state: &state,
                            on_assign: self.on_assign.clone(),
                        },
                        &t,
                    )
                })
                .collect::<Vec<_>>();

            let count = format!("{} of {QUEUE_LIMIT}", agent.queue.len());
            let aria = format!(
                "{}, {}, {} tasks, {}",
                agent.name,
                agent.model,
                agent.queue.len(),
                if busy == 0 {
                    "free now".to_string()
                } else {
                    format!("free at {}", clock_label(self.clock + busy))
                }
            );
            columns.push(
                column_well(eid(format!("{key}-col-{}", agent.id)), aria, wash)
                    .on_drag_move::<DraggedTask>(track_over(&state, target.clone()))
                    .on_drop::<DraggedTask>(drop_on(
                        &state,
                        target.clone(),
                        !full,
                        self.on_assign.clone(),
                    ))
                    .child(column_header(
                        !agent.queue.is_empty(),
                        agent.name.clone(),
                        count,
                        agent.model.clone(),
                        &t,
                    ))
                    .child(load_meter(busy_v, extra_v, meter_label, &t))
                    .when(!cards.is_empty(), |col| {
                        col.child(div().w_full().flex().flex_col().gap(px(8.)).children(cards))
                    })
                    .when(previewing, |col| col.child(drop_indicator(&t)))
                    .when(agent.queue.is_empty(), |col| {
                        col.child(empty_hint("Drop a task here to start it", &t))
                    })
                    .into_any_element(),
            );
        }

        let total = self.backlog.len() + self.agents.iter().map(|a| a.queue.len()).sum::<usize>();
        let header = page_header(&key, total, self.clock, self.auto, self.on_auto.clone(), &t);
        let notice = self.note.clone().map(|note| notice_strip(&key, note, &t));
        let n = columns.len() as f32;
        let el = div()
            .id(self.id.clone())
            .role(Role::Group)
            .aria_label("Delegate board")
            .w(px(n * COLUMN_W + (n - 1.) * COLUMN_GAP))
            .font_family(pink::SANS)
            .type_style(TextStyle::Body)
            .text_color(t.text)
            .flex()
            .flex_col()
            .gap(px(PAGE_GAP))
            .child(header)
            .children(notice)
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(COLUMN_GAP))
                    .children(columns),
            );
        motion::keep_alive(window);
        el
    }
}

/// The page title with its count, the help line and the toolbar (pink:
/// board/filters.rs `render_header`: settings `page_title_bar` – Title1 over
/// a 13 pt count on its baseline – then `page_subtitle`, then a toolbar of
/// chips 8 apart). Auto-assign is a toolbar chip like pink's "Needs
/// attention".
fn page_header(
    key: &str,
    total: usize,
    clock: u32,
    auto: bool,
    on_auto: Option<ToggleHandler>,
    t: &Pink,
) -> impl IntoElement {
    let auto_chip = pink::chip(format!("{key}-auto"), auto, ControlSize::Md, t)
        .aria_label("Auto-assign")
        .gap(px(6.))
        .child(pink::glyph(ICON_AUTO, ControlSize::Md))
        .child("Auto-assign")
        .on_click(move |_, window, cx| {
            if let Some(f) = on_auto.as_ref() {
                f(!auto, window, cx);
            }
        });
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap(px(10.))
                        .child(
                            div()
                                .type_style(TextStyle::Title1)
                                .text_color(t.text)
                                .child("Delegate"),
                        )
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(t.readable_muted(0.7))
                                .child(total.to_string()),
                        ),
                )
                .child(
                    div()
                        .mt(px(6.))
                        .max_w(px(600.))
                        .text_size(px(13.))
                        .line_height(px(19.))
                        .text_color(t.readable_muted(0.9))
                        .child(
                            "Drag a task onto an agent to queue it, three at most; drag it \
                             back to the Backlog to unassign it.",
                        ),
                ),
        )
        .child(
            div()
                .w_full()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .child(auto_chip)
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .font_features(tabular_figures())
                        .text_color(t.readable_muted(0.8))
                        .child(format!(
                            "Now {} · meters show the next {} minutes",
                            clock_label(clock),
                            HORIZON as u32
                        )),
                ),
        )
}

// ---------------------------------------------------------------------------
// Story: three agents, a backlog and a clock that runs a minute a tick
// ---------------------------------------------------------------------------

struct Sim {
    agents: Vec<BoardAgent>,
    backlog: Vec<BoardTask>,
    /// Work that arrives in the Backlog later.
    incoming: Vec<BoardTask>,
    clock: u32,
    auto: bool,
    note: Option<SharedString>,
    pulse: Option<(SharedString, Instant)>,
    tick: u32,
    _task: Task<()>,
}

impl Sim {
    fn new(cx: &mut Context<Self>) -> Self {
        let agents = vec![
            BoardAgent::new("kit", "Kit", "lumen-4 mini")
                .task(BoardTask::new("crash", "Triage overnight crash reports", "ops", 15).done(6)),
            BoardAgent::new("ivo", "Ivo", "lumen-4 large")
                .task(BoardTask::new("auth", "Refactor the auth middleware", "api", 30).done(8))
                .task(BoardTask::new("tls", "Bump the TLS library", "infra", 12)),
            BoardAgent::new("noor", "Noor", "lumen-4 large"),
        ];
        let backlog = vec![
            BoardTask::new("flaky", "Fix the flaky cart checkout test", "tests", 20),
            BoardTask::new(
                "tickets",
                "Summarize this week's support tickets",
                "research",
                15,
            ),
            BoardTask::new("paging", "Paginate /invoices", "api", 25),
        ];
        let incoming = vec![
            BoardTask::new("changelog", "Draft the Q3 changelog", "docs", 25),
            BoardTask::new("onboard", "Tighten the onboarding copy", "docs", 10),
            BoardTask::new("index", "Add an index for slow order lookups", "db", 18),
        ];
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(400))
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
            agents,
            backlog,
            incoming,
            clock: 14 * 60 + 5,
            auto: true,
            note: None,
            pulse: None,
            tick: 0,
            _task: task,
        }
    }

    /// Every 400ms: a sim minute passes every third tick, new work arrives
    /// every 25th, and Auto-assign hands out work every 10th.
    fn step(&mut self) {
        self.tick += 1;
        if self.tick.is_multiple_of(3) {
            self.clock += 1;
            for agent in &mut self.agents {
                if let Some(task) = agent.queue.first_mut() {
                    task.done += 1;
                    if task.done >= task.minutes {
                        // Finished work comes back later as a new ticket.
                        let mut again = agent.queue.remove(0);
                        again.id = format!("{}-{}", again.id, self.tick).into();
                        again.done = 0;
                        self.incoming.push(again);
                    }
                }
            }
        }
        if self.tick.is_multiple_of(25) && self.backlog.len() < 4 && !self.incoming.is_empty() {
            self.backlog.push(self.incoming.remove(0));
        }
        if self.auto && self.tick.is_multiple_of(10) && !self.backlog.is_empty() {
            let Some(agent_id) = first_free(&self.agents).map(|a| a.id.clone()) else {
                return;
            };
            let task = self.backlog.remove(0);
            if let Some(agent) = self.agents.iter_mut().find(|a| a.id == agent_id) {
                let busy = agent.busy();
                self.note = Some(
                    if busy == 0 {
                        format!(
                            "Auto-assign sent \u{201c}{}\u{201d} to {}, who was free.",
                            task.title, agent.name
                        )
                    } else {
                        format!(
                            "Auto-assign sent \u{201c}{}\u{201d} to {}, free first at {}.",
                            task.title,
                            agent.name,
                            clock_label(self.clock + busy)
                        )
                    }
                    .into(),
                );
                agent.queue.push(task);
                self.pulse = Some((agent_id, Instant::now()));
            }
        }
    }

    fn assign(&mut self, task_id: &SharedString, target: &DropTarget) {
        let task = if let Some(i) = self.backlog.iter().position(|t| &t.id == task_id) {
            Some(self.backlog.remove(i))
        } else {
            self.agents.iter_mut().find_map(|a| {
                let i = a.queue.iter().position(|t| &t.id == task_id)?;
                (i > 0).then(|| a.queue.remove(i))
            })
        };
        let Some(task) = task else { return };
        match target {
            DropTarget::Backlog => self.backlog.push(task),
            DropTarget::Agent(id) => {
                if let Some(agent) = self.agents.iter_mut().find(|a| &a.id == id) {
                    if agent.has_room() {
                        agent.queue.push(task);
                    } else {
                        self.backlog.push(task);
                    }
                }
            }
        }
        self.note = None;
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let sim = window.use_keyed_state("delegate-board-sim", cx, |_, cx| Sim::new(cx));
        let s = sim.read(cx);
        let (assign, toggle) = (sim.clone(), sim.clone());
        let mut board = DelegateBoard::new(
            "delegate-board",
            s.agents.clone(),
            s.backlog.clone(),
            s.clock,
        )
        .auto(s.auto)
        .on_assign(move |task, target, _, cx| {
            assign.update(cx, |s, cx| {
                s.assign(task, target);
                cx.notify();
            })
        })
        .on_auto(move |on, _, cx| {
            toggle.update(cx, |s, cx| {
                s.auto = on;
                if !on {
                    s.note = None;
                }
                cx.notify();
            })
        });
        if let Some(note) = s.note.clone() {
            board = board.note(note);
        }
        if let Some((agent, at)) = s.pulse.clone() {
            board = board.pulse(agent, at);
        }
        board
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_free_skips_full_queues() {
        let busy = BoardAgent::new("a", "A", "m").task(BoardTask::new("t", "T", "x", 30));
        let mut full = BoardAgent::new("b", "B", "m");
        for i in 0..QUEUE_LIMIT {
            full = full.task(BoardTask::new(format!("f{i}"), "F", "x", 1));
        }
        let agents = vec![busy.clone(), full];
        assert_eq!(first_free(&agents).unwrap().id.as_ref(), "a");
        let agents = vec![busy, BoardAgent::new("c", "C", "m")];
        assert_eq!(first_free(&agents).unwrap().id.as_ref(), "c");
    }

    #[test]
    fn busy_counts_what_is_left() {
        let agent = BoardAgent::new("a", "A", "m")
            .task(BoardTask::new("t", "T", "x", 30).done(10))
            .task(BoardTask::new("u", "U", "x", 5));
        assert_eq!(agent.busy(), 25);
        assert_eq!(clock_label(14 * 60 + 5 + agent.busy()), "14:30");
    }
}
