//! Delegate Board – hand work to agents by dropping it on them.
//!
//! The backlog runs along the top; each agent is a column with its queue
//! and a load bar on a shared 90-minute horizon that says when it is free
//! again. Dragging a task over a column previews the result before you let
//! go: the load bar grows a ghost segment, the free-at time moves, and a
//! dashed slot opens at the end of the queue. Queued tasks can be dragged on
//! to another agent or back to the backlog. With Auto on, the host hands each
//! new task to whoever is free first ([`first_free`]) and the board pulses
//! that column and says why.
//!
//! ```ignore
//! DelegateBoard::new("board", agents, backlog, 14 * 60 + 5)
//!     .on_assign(|task, target, _, _| println!("{task} → {target:?}"))
//!     .on_auto(|on, _, _| println!("auto {on}"))
//! ```

use std::sync::Arc;
use std::{
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    Animation, AnimationExt, AnyView, App, Context, ElementId, Entity, FontWeight, IntoElement,
    Render, SharedString, Task, Transformation, Window, div, percentage, prelude::*, px, relative,
    svg,
};

use crate::{
    HoverFill,
    luna::{self, FillTier, Luna, oklch, radius, text},
    motion::{self, GLIDE, MENU_IN},
    story,
};

pub const NAME: &str = "Delegate Board";
pub const DESCRIPTION: &str = "Assign work by dropping tasks on agent columns – each column previews its new free-at time before you let go, and Auto hands new work to whoever frees up first";

const ICON_BOARD: &str = "icons/delegate-board/board.svg";
const ICON_GRIP: &str = "icons/delegate-board/grip.svg";
const ICON_SPINNER: &str = "icons/delegate-board/spinner.svg";
const ICON_CLOCK: &str = "icons/delegate-board/clock.svg";
const ICON_AUTO: &str = "icons/delegate-board/auto.svg";

const BOARD_W: f32 = 784.;
const PAD: f32 = 16.;
const COL_GAP: f32 = 10.;
const CARD_W: f32 = 178.;
const CARD_H: f32 = 58.;
const CARD_GAP: f32 = 10.;
const CHIP_H: f32 = 44.;
const CHIP_GAP: f32 = 6.;
/// Tasks an agent holds at most, the running one included.
pub const QUEUE_LIMIT: usize = 3;
/// Minutes the load bars span.
const HORIZON: f32 = 90.;

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
    pub hue: f32,
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
            hue: 277.,
            queue: Vec::new(),
        }
    }

    pub fn hue(mut self, degrees: f32) -> Self {
        self.hue = degrees;
        self
    }

    pub fn task(mut self, task: BoardTask) -> Self {
        self.queue.push(task);
        self
    }

    /// Minutes until the queue is empty.
    pub fn busy(&self) -> u32 {
        self.queue.iter().map(BoardTask::left).sum()
    }
}

/// The agent with room in its queue that frees up first (ties go to the
/// earlier column).
pub fn first_free(agents: &[BoardAgent]) -> Option<&BoardAgent> {
    agents
        .iter()
        .filter(|a| a.queue.len() < QUEUE_LIMIT)
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

/// The card that follows the pointer.
struct Ghost {
    task: BoardTask,
}

impl Render for Ghost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Luna::get(cx);
        task_card(&self.task, &t)
            .w(px(CARD_W))
            .bg(t.surface_overlay)
            .border_color(t.accent.opacity(0.5))
            .shadow(t.menu_shadow())
            .font_family(luna::SANS)
            .text_color(t.text)
            .opacity(0.96)
    }
}

struct BoardState {
    over: Option<DropTarget>,
    dragging: Option<SharedString>,
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

    /// Whether the Auto switch is on.
    pub fn auto(mut self, on: bool) -> Self {
        self.auto = on;
        self
    }

    /// A line under the header, e.g. why Auto picked an agent.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Flashes an agent's column (after an automatic assignment).
    pub fn pulse(mut self, agent: impl Into<SharedString>, at: Instant) -> Self {
        self.pulse = Some((agent.into(), at));
        self
    }

    /// A task was dropped on an agent or back on the backlog.
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

fn monogram(name: &str, hue: f32, size: f32, t: &Luna) -> impl IntoElement {
    let (wash, ink) = if t.dark {
        (oklch(0.30, 0.055, hue), oklch(0.86, 0.10, hue))
    } else {
        (oklch(0.945, 0.035, hue), oklch(0.47, 0.13, hue))
    };
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(radius::SM))
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(
            name.chars()
                .next()
                .map(|c| c.to_ascii_uppercase().to_string())
                .unwrap_or_default(),
        )
}

/// Title, tag and estimate – shared by backlog cards and the drag ghost.
fn task_card(task: &BoardTask, t: &Luna) -> gpui::Div {
    div()
        .h(px(CARD_H))
        .pl(px(6.))
        .pr(px(10.))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_card)
        .flex()
        .items_center()
        .gap(px(4.))
        .child(
            svg()
                .path(ICON_GRIP)
                .size(px(14.))
                .flex_none()
                .text_color(t.text_faint.opacity(0.7)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    // Explicit width: GPUI truncates nowrap text at the first
                    // width it measures, which in nested flex isn't the final one.
                    div()
                        .w(px(CARD_W - 38.))
                        .truncate()
                        .text_size(px(text::BODY))
                        .font_weight(FontWeight::MEDIUM)
                        .line_height(px(18.))
                        .child(task.title.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(text::CAPTION))
                        .child(
                            div()
                                .h(px(18.))
                                .px(px(6.))
                                .flex()
                                .items_center()
                                .rounded(px(radius::XS))
                                .bg(t.fill(FillTier::Tertiary))
                                .text_color(t.text_muted)
                                .child(task.tag.clone()),
                        )
                        .child(
                            div()
                                .font_family(luna::MONO)
                                .text_color(t.text_faint)
                                .child(format!("~{}m", task.minutes)),
                        ),
                ),
        )
}

impl RenderOnce for DelegateBoard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| BoardState {
                over: None,
                dragging: None,
            },
        );
        let dragging_now = cx.has_active_drag();
        let reduced = motion::reduced_motion(cx);
        let (over, dragging) = {
            let s = state.read(cx);
            if dragging_now {
                (s.over.clone(), s.dragging.clone())
            } else {
                (None, None)
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

        let header = header_view(&key, self.clock, self.auto, self.on_auto.clone(), &t);
        let note = self.note.clone().map(|note| {
            motion::rise_in(
                eid(format!("{key}-note-{note}")),
                MENU_IN,
                -4.,
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .px(px(PAD))
                    .h(px(30.))
                    .border_b_1()
                    .border_color(t.border)
                    .bg(t.accent_wash.opacity(if t.dark { 0.35 } else { 0.6 }))
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .child(svg().path(ICON_AUTO).size(px(13.)).text_color(t.accent))
                    .child(note),
            )
        });

        let backlog = backlog_view(
            &key,
            &self.backlog,
            dragging.as_ref(),
            over.as_ref(),
            dragged.as_ref(),
            &state,
            self.on_assign.clone(),
            &t,
        );
        let col_w = (BOARD_W - 2. * PAD - 3. * COL_GAP) / 4.;
        let columns = self
            .agents
            .iter()
            .map(|agent| {
                agent_column(
                    &key,
                    agent,
                    col_w,
                    self.clock,
                    over.as_ref(),
                    dragged.as_ref(),
                    dragging.as_ref(),
                    self.pulse
                        .as_ref()
                        .filter(|(id, _)| id == &agent.id)
                        .map(|(_, at)| *at),
                    &state,
                    self.on_assign.clone(),
                    reduced,
                    window,
                    &t,
                )
            })
            .collect::<Vec<_>>();

        let el = luna::card(&t)
            .id(self.id.clone())
            .w(px(BOARD_W))
            .font_family(luna::SANS)
            .text_size(px(text::BODY))
            .text_color(t.text)
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(header)
            .children(note)
            .child(backlog)
            .child(
                div()
                    .px(px(PAD))
                    .pb(px(PAD))
                    .flex()
                    .gap(px(COL_GAP))
                    .children(columns),
            );
        motion::keep_alive(window);
        el
    }
}

fn header_view(
    key: &str,
    clock: u32,
    auto: bool,
    on_auto: Option<ToggleHandler>,
    t: &Luna,
) -> impl IntoElement {
    let knob = motion::follow(&format!("{key}-auto"), if auto { 1. } else { 0. }, MENU_IN);
    let switch_key = format!("{key}-switch");
    div()
        .h(px(56.))
        .px(px(PAD))
        .flex()
        .items_center()
        .gap(px(10.))
        .border_b_1()
        .border_color(t.border)
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
                .child(
                    svg()
                        .path(ICON_BOARD)
                        .size(px(16.))
                        .text_color(t.text_muted),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(text::CALLOUT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(20.))
                        .child("Delegate"),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .text_color(t.text_faint)
                        .child("Drop a task on an agent to queue it there"),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .text_size(px(text::FOOTNOTE))
                .font_family(luna::MONO)
                .text_color(t.text_muted)
                .child(
                    svg()
                        .path(ICON_CLOCK)
                        .size(px(13.))
                        .text_color(t.text_faint),
                )
                .child(clock_label(clock)),
        )
        .child(div().w(px(1.)).h(px(18.)).mx(px(4.)).bg(t.border))
        .child(
            div()
                .id(eid(format!("{key}-auto-toggle")))
                .flex()
                .items_center()
                .gap(px(8.))
                .h(px(28.))
                .pl(px(8.))
                .pr(px(6.))
                .rounded(px(radius::MD))
                .cursor_pointer()
                .hover_fill(
                    switch_key,
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                )
                .on_click(move |_, window, cx| {
                    if let Some(f) = on_auto.as_ref() {
                        f(!auto, window, cx);
                    }
                })
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if auto { t.text } else { t.text_muted })
                        .child("Auto-assign"),
                )
                .child(
                    div()
                        .relative()
                        .w(px(30.))
                        .h(px(18.))
                        .rounded_full()
                        .bg(motion::mix(
                            t.fill(FillTier::Primary),
                            t.accent_strong,
                            knob,
                        ))
                        .child(
                            div()
                                .absolute()
                                .top(px(2.))
                                .left(px(2. + 12. * knob))
                                .size(px(14.))
                                .rounded_full()
                                .bg(gpui::white())
                                .shadow(vec![crate::primitives::shadow(
                                    0.,
                                    1.,
                                    2.,
                                    0.,
                                    gpui::hsla(0., 0., 0., 0.25),
                                )]),
                        ),
                ),
        )
}

#[allow(clippy::too_many_arguments)]
fn backlog_view(
    key: &str,
    backlog: &[BoardTask],
    dragging: Option<&SharedString>,
    over: Option<&DropTarget>,
    dragged: Option<&(BoardTask, DropTarget)>,
    state: &Entity<BoardState>,
    on_assign: Option<AssignHandler>,
    t: &Luna,
) -> impl IntoElement {
    let target_over = over == Some(&DropTarget::Backlog)
        && dragged.is_some_and(|(_, from)| from != &DropTarget::Backlog);
    let cards = backlog.iter().enumerate().map(|(i, task)| {
        let x = motion::follow(
            &format!("{key}-b-{}", task.id),
            i as f32 * (CARD_W + CARD_GAP),
            GLIDE,
        );
        let lifted = dragging == Some(&task.id);
        let card_key = format!("{key}-card-{}", task.id);
        let drag_state = state.clone();
        let payload = DraggedTask {
            task: task.clone(),
            from: DropTarget::Backlog,
        };
        let el = task_card(task, t)
            .id(eid(format!("card-{}", task.id)))
            .absolute()
            .left(px(x))
            .top_0()
            .w(px(CARD_W))
            .cursor_grab()
            .when(lifted, |d| {
                d.opacity(0.35)
                    .border_dashed()
                    .border_color(t.border_strong)
            })
            .when(!lifted, |d| {
                d.hover_border(card_key, t.border, t.border_strong)
            })
            .on_drag(payload, move |dragged: &DraggedTask, _, _, cx| {
                drag_state.update(cx, |s, _| s.dragging = Some(dragged.task.id.clone()));
                let task = dragged.task.clone();
                cx.new(|_| Ghost { task })
            });
        el.with_animation(
            eid(format!("{key}-in-{}", task.id)),
            MENU_IN.with_duration(280).animation(),
            |el, p| el.opacity(p).top(px(6. * (1. - p))),
        )
    });

    let move_state = state.clone();
    let drop_state = state.clone();
    div()
        .id("backlog")
        .px(px(PAD))
        .pt(px(12.))
        .pb(px(14.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .bg(if target_over {
            t.accent_wash.opacity(0.5)
        } else {
            gpui::transparent_black()
        })
        .on_drag_move::<DraggedTask>(move |ev, _, cx| {
            let inside = ev.bounds.contains(&ev.event.position);
            move_state.update(cx, |s, cx| {
                if inside && s.over.as_ref() != Some(&DropTarget::Backlog) {
                    s.over = Some(DropTarget::Backlog);
                    cx.notify();
                } else if !inside && s.over.as_ref() == Some(&DropTarget::Backlog) {
                    s.over = None;
                    cx.notify();
                }
            });
        })
        .on_drop::<DraggedTask>(move |dragged, window, cx| {
            drop_state.update(cx, |s, cx| {
                s.over = None;
                s.dragging = None;
                cx.notify();
            });
            if dragged.from != DropTarget::Backlog
                && let Some(f) = on_assign.as_ref()
            {
                f(&dragged.task.id, &DropTarget::Backlog, window, cx);
            }
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(text::FOOTNOTE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_muted)
                .child("Backlog")
                .child(
                    div()
                        .min_w(px(18.))
                        .h(px(16.))
                        .px(px(4.))
                        .rounded(px(radius::XS))
                        .bg(t.fill(FillTier::Tertiary))
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family(luna::MONO)
                        .text_size(px(10.5))
                        .child(backlog.len().to_string()),
                )
                .when(target_over, |d| {
                    d.child(
                        div()
                            .text_color(t.accent)
                            .font_weight(FontWeight::NORMAL)
                            .child("· drop to unassign"),
                    )
                }),
        )
        .child(
            div()
                .relative()
                .h(px(CARD_H))
                .children(cards)
                .when(backlog.is_empty(), |d| {
                    d.child(
                        div()
                            .h_full()
                            .rounded(px(radius::MD))
                            .border_1()
                            .border_dashed()
                            .border_color(t.border_strong)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child("Everything is assigned"),
                    )
                }),
        )
}

#[allow(clippy::too_many_arguments)]
fn agent_column(
    key: &str,
    agent: &BoardAgent,
    width: f32,
    clock: u32,
    over: Option<&DropTarget>,
    dragged: Option<&(BoardTask, DropTarget)>,
    dragging: Option<&SharedString>,
    pulse: Option<Instant>,
    state: &Entity<BoardState>,
    on_assign: Option<AssignHandler>,
    reduced: bool,
    window: &mut Window,
    t: &Luna,
) -> gpui::AnyElement {
    let target = DropTarget::Agent(agent.id.clone());
    let incoming = dragged.filter(|(_, from)| from != &target);
    let full = agent.queue.len() >= QUEUE_LIMIT;
    let hovered = over == Some(&target) && incoming.is_some();
    let previewing = hovered && !full;
    let refusing = hovered && full;
    let extra = if previewing {
        incoming.map_or(0, |(task, _)| task.left())
    } else {
        0
    };
    let busy = agent.busy();
    let busy_v = motion::follow(&format!("{key}-busy-{}", agent.id), busy as f32, GLIDE);
    let extra_v = motion::follow(&format!("{key}-extra-{}", agent.id), extra as f32, MENU_IN);
    let free_at = clock + busy + extra;

    // Auto-assign pulse: the border lights up and fades back.
    let pulse_t = pulse
        .map(|at| at.elapsed().as_secs_f32() / (1.1 * motion::speed_scale()))
        .filter(|p| *p < 1.);
    if pulse_t.is_some() {
        window.request_animation_frame();
    }
    let pulse_amount = pulse_t.map_or(0., |p| (1. - p).powf(1.5));
    let border = if previewing {
        t.accent.opacity(0.7)
    } else if refusing {
        t.danger.opacity(0.6)
    } else {
        motion::mix(t.border, t.accent.opacity(0.8), pulse_amount)
    };

    let bar_w = width - 20.;
    let fill_w = (busy_v / HORIZON).clamp(0., 1.) * bar_w;
    let ghost_w = ((busy_v + extra_v) / HORIZON).clamp(0., 1.) * bar_w - fill_w;
    let load = div()
        .mt(px(10.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .relative()
                .h(px(6.))
                .w(px(bar_w))
                .rounded(px(3.))
                .bg(t.fill(FillTier::Secondary))
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .h_full()
                        .w(px(fill_w))
                        .bg(t.accent),
                )
                .when(ghost_w > 0.5, |d| {
                    d.child(
                        div()
                            .absolute()
                            .left(px(fill_w))
                            .top_0()
                            .h_full()
                            .w(px(ghost_w))
                            .bg(t.accent.opacity(0.38)),
                    )
                })
                // Half-hour ticks.
                .children((1..3).map(|i| {
                    div()
                        .absolute()
                        .left(px(bar_w * i as f32 * 30. / HORIZON))
                        .top_0()
                        .h_full()
                        .w(px(1.))
                        .bg(t.surface_card.opacity(0.9))
                })),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(text::CAPTION))
                .child(if refusing {
                    div().text_color(t.danger).child("Queue is full")
                } else if busy + extra == 0 {
                    div().text_color(t.success).child("Free now")
                } else {
                    div()
                        .text_color(if previewing { t.text } else { t.text_muted })
                        .child(format!("Free at {}", clock_label(free_at)))
                })
                .when(previewing, |d| {
                    d.child(
                        div()
                            .h(px(16.))
                            .px(px(5.))
                            .rounded(px(radius::XS))
                            .bg(t.accent_wash)
                            .flex()
                            .items_center()
                            .font_family(luna::MONO)
                            .text_color(if t.dark { t.text } else { t.accent })
                            .child(format!("+{extra}m")),
                    )
                }),
        );

    let chips = agent.queue.iter().enumerate().map(|(i, task)| {
        let y = motion::follow_from(
            &format!("{key}-q-{}-{}", agent.id, task.id),
            i as f32 * (CHIP_H + CHIP_GAP) + 8.,
            i as f32 * (CHIP_H + CHIP_GAP),
            GLIDE,
        );
        queue_chip(
            key,
            agent,
            task,
            i == 0,
            y,
            width - 20. - 2. - 18. - 12. - 6.,
            dragging,
            state,
            reduced,
            t,
        )
    });
    let slot = previewing.then(|| {
        let n = agent.queue.len().min(QUEUE_LIMIT - 1);
        let title = incoming
            .map(|(task, _)| task.title.clone())
            .unwrap_or_default();
        motion::fade_in(
            eid(format!("{key}-slot-{}", agent.id)),
            div()
                .absolute()
                .left_0()
                .right_0()
                .top(px(n as f32 * (CHIP_H + CHIP_GAP)))
                .h(px(CHIP_H))
                .px(px(10.))
                .rounded(px(radius::MD))
                .border_1()
                .border_dashed()
                .border_color(t.accent.opacity(0.6))
                .bg(t.accent_wash.opacity(0.4))
                .flex()
                .items_center()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_muted)
                .child(div().truncate().child(title)),
        )
    });

    let move_state = state.clone();
    let drop_state = state.clone();
    let move_target = target.clone();
    let drop_target = target.clone();
    div()
        .id(eid(format!("col-{}", agent.id)))
        .flex_none()
        .w(px(width))
        .p(px(10.))
        .rounded(px(radius::LG))
        .border_1()
        .border_color(border)
        .bg(if previewing {
            t.accent_wash.opacity(if t.dark { 0.35 } else { 0.5 })
        } else {
            t.fill(FillTier::Quaternary)
        })
        .on_drag_move::<DraggedTask>(move |ev, _, cx| {
            let inside = ev.bounds.contains(&ev.event.position);
            move_state.update(cx, |s, cx| {
                if inside && s.over.as_ref() != Some(&move_target) {
                    s.over = Some(move_target.clone());
                    cx.notify();
                } else if !inside && s.over.as_ref() == Some(&move_target) {
                    s.over = None;
                    cx.notify();
                }
            });
        })
        .on_drop::<DraggedTask>(move |dragged, window, cx| {
            drop_state.update(cx, |s, cx| {
                s.over = None;
                s.dragging = None;
                cx.notify();
            });
            if dragged.from != drop_target
                && !full
                && let Some(f) = on_assign.as_ref()
            {
                f(&dragged.task.id, &drop_target, window, cx);
            }
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(monogram(&agent.name, agent.hue, 26., t))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(text::BODY))
                                .font_weight(FontWeight::MEDIUM)
                                .line_height(px(16.))
                                .child(agent.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(text::CAPTION))
                                .line_height(px(14.))
                                .text_color(t.text_faint)
                                .truncate()
                                .child(agent.model.clone()),
                        ),
                ),
        )
        .child(load)
        .child(
            div()
                .mt(px(12.))
                .relative()
                .h(px(QUEUE_LIMIT as f32 * (CHIP_H + CHIP_GAP) - CHIP_GAP))
                .children(chips)
                .children(slot)
                .when(agent.queue.is_empty() && !previewing, |d| {
                    d.child(
                        div()
                            .h(px(CHIP_H))
                            .rounded(px(radius::MD))
                            .border_1()
                            .border_dashed()
                            .border_color(t.border_strong)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child("Idle · drop work here"),
                    )
                }),
        )
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn queue_chip(
    key: &str,
    agent: &BoardAgent,
    task: &BoardTask,
    running: bool,
    y: f32,
    title_w: f32,
    dragging: Option<&SharedString>,
    state: &Entity<BoardState>,
    reduced: bool,
    t: &Luna,
) -> impl IntoElement {
    let lifted = dragging == Some(&task.id);
    let progress = motion::follow(
        &format!("{key}-p-{}", task.id),
        task.done as f32 / task.minutes as f32,
        GLIDE,
    );
    let chip_key = format!("{key}-chip-{}", task.id);
    let chip = div()
        .id(eid(format!("chip-{}-{}", agent.id, task.id)))
        .absolute()
        .left_0()
        .right_0()
        .top(px(y))
        .h(px(CHIP_H))
        .pl(px(8.))
        .pr(px(10.))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_card)
        .overflow_hidden()
        .flex()
        .items_center()
        .gap(px(6.))
        .when(running, |d| {
            let spinner = svg()
                .path(ICON_SPINNER)
                .size(px(12.))
                .flex_none()
                .text_color(t.accent);
            d.child(if reduced {
                spinner.into_any_element()
            } else {
                spinner
                    .with_animation(
                        eid(format!("{key}-spin-{}", task.id)),
                        Animation::new(Duration::from_millis(900)).repeat(),
                        |s, p| s.with_transformation(Transformation::rotate(percentage(p))),
                    )
                    .into_any_element()
            })
        })
        .when(!running, |d| {
            d.child(
                svg()
                    .path(ICON_GRIP)
                    .size(px(12.))
                    .flex_none()
                    .text_color(t.text_faint.opacity(0.7)),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .w(px(title_w))
                        .truncate()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .text_color(if running { t.text } else { t.text_muted })
                        .child(task.title.clone()),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .text_size(px(10.5))
                        .line_height(px(14.))
                        .text_color(t.text_faint)
                        .child(task.tag.clone())
                        .child(div().font_family(luna::MONO).child(if running {
                            format!("{}m left", task.left())
                        } else {
                            format!("~{}m", task.left())
                        })),
                ),
        )
        .when(running, |d| {
            d.child(
                div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .h(px(2.))
                    .w(relative(progress.clamp(0., 1.)))
                    .bg(t.accent),
            )
        })
        .when(lifted, |d| {
            d.opacity(0.35)
                .border_dashed()
                .border_color(t.border_strong)
        });
    let chip = if running {
        chip
    } else {
        let drag_state = state.clone();
        let payload = DraggedTask {
            task: task.clone(),
            from: DropTarget::Agent(agent.id.clone()),
        };
        chip.cursor_grab()
            .when(!lifted, |d| {
                d.hover_border(chip_key, t.border, t.border_strong)
            })
            .on_drag(payload, move |dragged: &DraggedTask, _, _, cx| {
                drag_state.update(cx, |s, _| s.dragging = Some(dragged.task.id.clone()));
                let task = dragged.task.clone();
                cx.new(|_| Ghost { task })
            })
    };
    chip.with_animation(
        eid(format!("{key}-chip-in-{}-{}", agent.id, task.id)),
        MENU_IN.with_duration(280).animation(),
        |el, p| el.opacity(p),
    )
}

// ---------------------------------------------------------------------------
// Story: four agents, a short backlog and a clock that runs a minute a tick
// ---------------------------------------------------------------------------

struct Sim {
    agents: Vec<BoardAgent>,
    backlog: Vec<BoardTask>,
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
                .hue(150.)
                .task(BoardTask::new("crash", "Triage crash reports", "ops", 15).done(6)),
            BoardAgent::new("ivo", "Ivo", "lumen-4 large")
                .hue(32.)
                .task(BoardTask::new("auth", "Refactor auth middleware", "api", 30).done(8))
                .task(BoardTask::new("tls", "Bump TLS library", "infra", 12)),
            BoardAgent::new("noor", "Noor", "lumen-4 large")
                .hue(277.)
                .task(BoardTask::new("changelog", "Draft Q3 changelog", "docs", 25).done(10)),
            BoardAgent::new("sol", "Sol", "lumen-4 mini").hue(200.),
        ];
        let backlog = vec![
            BoardTask::new("flaky", "Fix flaky cart test", "tests", 20),
            BoardTask::new("tickets", "Summarize tickets", "research", 15),
            BoardTask::new("paging", "Paginate /invoices", "api", 25),
            BoardTask::new("onboard", "Edit onboarding copy", "docs", 10),
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
            clock: 14 * 60 + 5,
            auto: false,
            note: None,
            pulse: None,
            tick: 0,
            _task: task,
        }
    }

    /// Every 400ms: a sim minute passes every third tick, Auto hands out
    /// work every fourth.
    fn step(&mut self) {
        self.tick += 1;
        if self.tick.is_multiple_of(3) {
            self.clock += 1;
            for agent in &mut self.agents {
                if let Some(task) = agent.queue.first_mut() {
                    task.done += 1;
                    if task.done >= task.minutes {
                        agent.queue.remove(0);
                    }
                }
            }
        }
        if self.auto && self.tick.is_multiple_of(4) && !self.backlog.is_empty() {
            let Some(agent_id) = first_free(&self.agents).map(|a| a.id.clone()) else {
                return;
            };
            let task = self.backlog.remove(0);
            if let Some(agent) = self.agents.iter_mut().find(|a| a.id == agent_id) {
                let free_at = clock_label(self.clock + agent.busy());
                self.note = Some(
                    format!(
                        "Auto sent \u{201c}{}\u{201d} to {} – free first, at {free_at}",
                        task.title, agent.name
                    )
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
                    if agent.queue.len() < QUEUE_LIMIT {
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
