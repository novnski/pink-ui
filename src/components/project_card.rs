//! Project Card – a project on Pink's board ticket card, with a progress ring
//! that has one segment per subtask.
//!
//! The frame, title, status badge and footer are Pink's board card (Pink:
//! shell/board/card.rs `BoardPage::render_card`): a resting
//! `fill(Quaternary)` plate with a `separator(true)` edge, a 13/18 title over
//! glyph-and-word badges, and a hairline footer. People are Pink's project
//! tiles. Clicking the header folds the card open on Pink's disclosure: a
//! status picker, a people filter and the subtasks. Everything is linked by
//! hover – point at a subtask and its ring segment lifts, at a person and
//! their subtasks and segments stand out, at a segment and the footer names
//! its subtask. Ticking a subtask fills its segment.
//!
//! ```ignore
//! ProjectCard::new("billing", "Billing v2 migration")
//!     .due("Nov 14")
//!     .person(ProjectPerson::new("Ana Kovač", 250.))
//!     .subtask(Subtask::new("Map old plans to new prices", 0).done(true))
//! ```

use std::{cell::Cell, f32::consts::PI, rc::Rc, sync::Arc};

use gpui::{
    AnyView, App, Bounds, Div, ElementId, FontWeight, Hsla, IntoElement, MouseMoveEvent,
    PathBuilder, Pixels, Role, SharedString, Toggled, Transformation, Window, canvas, div, point,
    prelude::*, px, radians, svg,
};
use web_time::Instant;

use crate::{
    HoverFill,
    motion::{self, FOLD_TWEEN_WINDOW, HOVER_FADE, QUICK, TOOL_BODY_RISE},
    pink::{self, ControlSize, FillTier, Pink, Segment, radius, space},
    story,
};

pub const NAME: &str = "Project Card";
pub const DESCRIPTION: &str = "A project as a quiet ticket card: its status and due date beside a ring with one segment per subtask, linked by hover to its people and subtasks, that folds open into a status picker, a people filter and a checklist";

const ICON_CALENDAR: &str = "icons/project-card/calendar.svg";
const ICON_CHECK: &str = "icons/project-card/check.svg";
const ICON_CHEVRON: &str = "icons/project-card/alt-arrow-right.svg";
const ICON_OPEN: &str = "icons/project-card/arrow-right-up.svg";
const ICON_ON_TRACK: &str = "icons/project-card/target.svg";
const ICON_AT_RISK: &str = "icons/project-card/danger-triangle.svg";
const ICON_BLOCKED: &str = "icons/project-card/danger-circle.svg";

/// The ring spans the title and badge lines (18 + 8 + 14).
const RING: f32 = 40.0;
const RING_R: f32 = 17.0;
const RING_STROKE: f32 = 3.0;
/// Pink's smallest project tile (the sidebar's project row).
const TILE: f32 = 18.0;
/// Pink's activity alert row height.
const ROW_H: f32 = 28.0;
/// Card padding (Pink board card `px 10`), for the picker's width.
const PAD_X: f32 = 10.0;

/// Where a project stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectStatus {
    OnTrack,
    AtRisk,
    Blocked,
    Done,
}

impl ProjectStatus {
    pub const ALL: [ProjectStatus; 4] = [
        ProjectStatus::OnTrack,
        ProjectStatus::AtRisk,
        ProjectStatus::Blocked,
        ProjectStatus::Done,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ProjectStatus::OnTrack => "On track",
            ProjectStatus::AtRisk => "At risk",
            ProjectStatus::Blocked => "Blocked",
            ProjectStatus::Done => "Done",
        }
    }

    /// The badge glyph: the meaning never rides on color alone.
    fn glyph(self) -> &'static str {
        match self {
            ProjectStatus::OnTrack => ICON_ON_TRACK,
            ProjectStatus::AtRisk => ICON_AT_RISK,
            ProjectStatus::Blocked => ICON_BLOCKED,
            ProjectStatus::Done => ICON_CHECK,
        }
    }

    /// The badge tone (Pink: board/card.rs `Tone::color` – Running, Caution,
    /// Danger, Success).
    fn color(self, t: &Pink) -> Hsla {
        match self {
            ProjectStatus::OnTrack => t.readable_tone(t.accent, 1.0),
            ProjectStatus::AtRisk => t.readable_tone(t.warning, 0.95),
            ProjectStatus::Blocked => t.readable_tone(t.danger, 0.95),
            ProjectStatus::Done => t.readable_tone(t.success, 0.95),
        }
    }
}

/// Someone on the project, drawn as a tinted monogram tile.
#[derive(Clone, Debug)]
pub struct ProjectPerson {
    pub name: SharedString,
    /// Hue of the tile's tint, in degrees (Pink's project hues are 250, 150,
    /// 40, 205, 325 and 12).
    pub hue: f32,
}

impl ProjectPerson {
    pub fn new(name: impl Into<SharedString>, hue: f32) -> Self {
        Self {
            name: name.into(),
            hue,
        }
    }

    fn first_name(&self) -> SharedString {
        self.name
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
            .into()
    }

    /// Pink's tile tint for this hue (Pink: sidebar_projects.rs `project_tint`).
    fn tint(&self) -> Hsla {
        gpui::hsla(self.hue.rem_euclid(360.) / 360., 0.62, 0.68, 1.)
    }

    fn tile(&self, t: &Pink) -> Div {
        pink::monogram_tile(self.tint(), &self.name, TILE, t)
    }
}

/// One subtask; each gets a segment of the ring.
#[derive(Clone, Debug)]
pub struct Subtask {
    pub title: SharedString,
    /// Index into the card's people.
    pub assignee: usize,
    pub due: Option<SharedString>,
    pub done: bool,
}

impl Subtask {
    pub fn new(title: impl Into<SharedString>, assignee: usize) -> Self {
        Self {
            title: title.into(),
            assignee,
            due: None,
            done: false,
        }
    }

    pub fn due(mut self, due: impl Into<SharedString>) -> Self {
        self.due = Some(due.into());
        self
    }

    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }
}

type StatusHandler = Rc<dyn Fn(ProjectStatus, &mut Window, &mut App)>;
type TaskHandler = Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;
type OpenHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ProjectCard {
    id: ElementId,
    name: SharedString,
    due: Option<SharedString>,
    status: ProjectStatus,
    people: Vec<ProjectPerson>,
    subtasks: Vec<Subtask>,
    default_open: bool,
    width: f32,
    on_status: Option<StatusHandler>,
    on_toggle: Option<TaskHandler>,
    on_open: Option<OpenHandler>,
}

impl ProjectCard {
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            due: None,
            status: ProjectStatus::OnTrack,
            people: Vec::new(),
            subtasks: Vec::new(),
            default_open: false,
            width: 372.,
            on_status: None,
            on_toggle: None,
            on_open: None,
        }
    }

    pub fn due(mut self, due: impl Into<SharedString>) -> Self {
        self.due = Some(due.into());
        self
    }

    /// The starting status; the picker owns it afterwards.
    pub fn status(mut self, status: ProjectStatus) -> Self {
        self.status = status;
        self
    }

    pub fn person(mut self, person: ProjectPerson) -> Self {
        self.people.push(person);
        self
    }

    pub fn subtask(mut self, subtask: Subtask) -> Self {
        self.subtasks.push(subtask);
        self
    }

    pub fn subtasks(mut self, subtasks: impl IntoIterator<Item = Subtask>) -> Self {
        self.subtasks.extend(subtasks);
        self
    }

    /// Starts unfolded (as if the header had been clicked).
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Card width in px (default 372).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn on_status(
        mut self,
        handler: impl Fn(ProjectStatus, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_status = Some(Rc::new(handler));
        self
    }

    /// Called with the subtask index and its new done state.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(usize, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    pub fn on_open(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_open = Some(Rc::new(handler));
        self
    }
}

struct CardState {
    open: bool,
    /// When the header last toggled the fold (drives Pink's fold clock).
    toggled_at: Option<Instant>,
    status: ProjectStatus,
    done: Vec<bool>,
    hot_task: Option<usize>,
    hot_person: Option<usize>,
    /// A person picked in the filter (sticks until clicked again).
    person: Option<usize>,
    ring_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    body_h: Rc<Cell<f32>>,
}

/// Which ring segment (if any) sits under `pos`.
fn segment_at(bounds: Bounds<Pixels>, pos: gpui::Point<Pixels>, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let c = bounds.center();
    let (dx, dy) = (f32::from(pos.x - c.x), f32::from(pos.y - c.y));
    let dist = (dx * dx + dy * dy).sqrt();
    if !(RING_R - 6.0..=RING_R + 6.0).contains(&dist) {
        return None;
    }
    // Angle clockwise from 12 o'clock, 0..2π.
    let a = (dx.atan2(-dy) + 2. * PI) % (2. * PI);
    Some(((a / (2. * PI)) * n as f32) as usize % n)
}

/// A glyph + word badge (Pink: board/card.rs `badge`): a 12 px glyph and an
/// 11/14 word, both in the tone, gap 4, no plate.
fn badge(glyph: &'static str, label: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.))
        .min_w_0()
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(color)
        .child(
            svg()
                .path(glyph)
                .size(px(12.))
                .flex_none()
                .text_color(color),
        )
        .child(div().min_w_0().truncate().child(label.into()))
}

/// A subtask's check (Pink: markdown/render.rs task list item): 16 px,
/// radius 4, a `border` edge; checked, the accent with a 12 px check in `bg`.
fn check_box(checked: bool, t: &Pink) -> Div {
    div()
        .flex_none()
        .size(px(16.))
        .rounded(px(radius::XS))
        .border_1()
        .border_color(if checked { t.accent } else { t.border })
        .bg(if checked {
            t.accent
        } else {
            gpui::transparent_black()
        })
        .flex()
        .items_center()
        .justify_center()
        .when(checked, |el| {
            el.child(svg().path(ICON_CHECK).size(px(12.)).text_color(t.bg))
        })
}

impl RenderOnce for ProjectCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial_done: Vec<bool> = self.subtasks.iter().map(|s| s.done).collect();
        let (initial_status, default_open) = (self.status, self.default_open);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |_, _| CardState {
                open: default_open,
                toggled_at: None,
                status: initial_status,
                done: initial_done,
                hot_task: None,
                hot_person: None,
                person: None,
                ring_bounds: Rc::new(Cell::new(None)),
                body_h: Rc::new(Cell::new(0.)),
            },
        );
        let s = state.read(cx);
        let (open, toggled_at, status, done, hot_task, hot_person, person) = (
            s.open,
            s.toggled_at,
            s.status,
            s.done.clone(),
            s.hot_task,
            s.hot_person,
            s.person,
        );
        let (ring_bounds, body_h) = (s.ring_bounds.clone(), s.body_h.clone());
        let tasks = Rc::new(self.subtasks);
        let people = Rc::new(self.people);
        let n = tasks.len();
        let done_count = done.iter().filter(|d| **d).count();
        let focus = t.focus_ring();

        // One clock for the chevron, the height and the body fade (Pink:
        // transcript.rs tool fold).
        let window_ms = FOLD_TWEEN_WINDOW.mul_f32(motion::speed_scale());
        let elapsed = toggled_at
            .map(|at| at.elapsed())
            .filter(|e| *e < window_ms && !motion::reduced_motion(cx));
        let (fold, body_alpha) = motion::fold_progress(open, elapsed);
        if elapsed.is_some() {
            window.request_animation_frame();
        }

        // Who the card is focused on right now: a hovered person wins over
        // the picked one.
        let focus_person = hot_person.or(person);
        let task_hot = |ix: usize| -> bool {
            hot_task == Some(ix) || focus_person.is_some_and(|p| tasks[ix].assignee == p)
        };
        let any_hot = hot_task.is_some() || focus_person.is_some();

        // ---- ring ----------------------------------------------------------
        let segments: Vec<(f32, f32, f32)> = (0..n)
            .map(|ix| {
                let fill = motion::follow(
                    &format!("{key}-seg{ix}-done"),
                    if done[ix] { 1. } else { 0. },
                    QUICK,
                );
                let hot = motion::follow(
                    &format!("{key}-seg{ix}-hot"),
                    if task_hot(ix) { 1. } else { 0. },
                    HOVER_FADE,
                );
                let dim = motion::follow(
                    &format!("{key}-seg{ix}-dim"),
                    if any_hot && !task_hot(ix) { 1. } else { 0. },
                    HOVER_FADE,
                );
                (fill, hot, dim)
            })
            .collect();
        let ring = {
            // Neutral like Pink's usage meter (Pink: shell/usage.rs
            // `usage_meter`, `text_muted` for what is used). A 3 px arc needs
            // more than the meter's fill track, so open segments take the
            // card's own edge tone; a lifted segment brightens toward `text`.
            let track = t.separator(true);
            let (done_c, lifted_open, lifted_done) =
                (t.text_muted, t.text_muted.opacity(0.5), t.text);
            let bounds_cell = ring_bounds.clone();
            let segs = segments.clone();
            let paint = canvas(
                move |bounds, _, _| bounds_cell.set(Some(bounds)),
                move |bounds, _, window, _| {
                    let c = bounds.center();
                    let n = segs.len().max(1) as f32;
                    let gap = if segs.len() > 1 { 0.2 } else { 0. };
                    for (ix, (fill, hot, dim)) in segs.iter().enumerate() {
                        let a0 = -PI / 2. + ix as f32 / n * 2. * PI + gap / 2.;
                        let a1 = -PI / 2. + (ix + 1) as f32 / n * 2. * PI - gap / 2.;
                        let p =
                            |a: f32| point(c.x + px(RING_R * a.cos()), c.y + px(RING_R * a.sin()));
                        let mut path = PathBuilder::stroke(px(RING_STROKE + hot));
                        path.move_to(p(a0));
                        path.arc_to(
                            point(px(RING_R), px(RING_R)),
                            px(0.),
                            a1 - a0 > PI,
                            true,
                            p(a1),
                        );
                        let rest = motion::mix(track, done_c, *fill);
                        let lifted = motion::mix(lifted_open, lifted_done, *fill);
                        let color = motion::mix(rest, lifted, *hot);
                        let color = color.opacity(color.a * (1. - 0.6 * dim));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full();
            let (state_move, state_out) = (state.clone(), state.clone());
            div()
                .id(SharedString::from(format!("{key}-ring")))
                .relative()
                .size(px(RING))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .child(paint)
                .child(
                    div()
                        .text_size(px(11.))
                        .line_height(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .font_features(pink::tabular_figures())
                        .text_color(t.text)
                        .child(format!("{done_count}/{n}")),
                )
                .on_mouse_move(move |e: &MouseMoveEvent, _, cx| {
                    let Some(bounds) = ring_bounds.get() else {
                        return;
                    };
                    let seg = segment_at(bounds, e.position, n);
                    state_move.update(cx, |s, cx| {
                        if s.hot_task != seg {
                            s.hot_task = seg;
                            cx.notify();
                        }
                    });
                })
                .on_hover(move |hovered, _, cx| {
                    if !*hovered {
                        state_out.update(cx, |s, cx| {
                            s.hot_task = None;
                            cx.notify();
                        });
                    }
                })
        };

        // ---- header: title, badges, ring (Pink board card title + status) ---
        let status_badge = motion::status_swap(
            SharedString::from(format!("{key}-status-{status:?}")),
            badge(status.glyph(), status.label(), status.color(&t)),
        );
        let header = {
            let state = state.clone();
            let label = format!(
                "{}, {}, {done_count} of {n} subtasks done",
                self.name,
                status.label()
            );
            div()
                .id(SharedString::from(format!("{key}-head")))
                .role(Role::Button)
                .aria_label(label)
                .aria_expanded(open)
                .tab_index(0)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S10))
                // Outset so the focus ring clears the title and the ring.
                .m(px(-space::S4))
                .p(px(space::S4))
                .rounded(px(radius::SM))
                .cursor_pointer()
                .focus_visible(move |s| pink::focus_ring(s, focus))
                .on_key_down(pink::swallow_activation_key)
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.open = !s.open;
                        s.toggled_at = Some(Instant::now());
                        cx.notify();
                    })
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(space::S8))
                        .child(
                            // Pink: board/card.rs title, with the tool group's
                            // turning chevron after it (transcript.rs).
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(space::S4))
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_size(px(13.))
                                        .line_height(px(18.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(t.text)
                                        .child(self.name.clone()),
                                )
                                .child(
                                    svg()
                                        .path(ICON_CHEVRON)
                                        .flex_none()
                                        .size(px(12.))
                                        .with_transformation(Transformation::rotate(radians(
                                            PI / 2. * fold,
                                        )))
                                        .text_color(t.content_faint),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .gap_x(px(space::S10))
                                .gap_y(px(space::S4))
                                .child(status_badge)
                                .when_some(self.due.clone(), |row, due| {
                                    row.child(badge(ICON_CALENDAR, due, t.readable_muted(0.9)))
                                }),
                        ),
                )
                .child(ring)
        };

        // ---- unfolded body -----------------------------------------------------
        let selected_ix = ProjectStatus::ALL
            .iter()
            .position(|st| *st == status)
            .unwrap_or(0);
        let statuses = {
            let state = state.clone();
            let on_status = self.on_status.clone();
            pink::segmented(
                format!("{key}-status"),
                ProjectStatus::ALL
                    .iter()
                    .map(|st| Segment::new(format!("{st:?}"), st.label()))
                    .collect(),
                selected_ix,
                self.width - 2. - PAD_X * 2.,
                ControlSize::Sm,
                &t,
                move |ix, window, cx| {
                    let st = ProjectStatus::ALL[*ix];
                    state.update(cx, |s, cx| {
                        s.status = st;
                        cx.notify();
                    });
                    if let Some(f) = &on_status {
                        f(st, window, cx);
                    }
                },
            )
        };

        // People filter: Pink chips, each with the person's tile.
        let people_row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(space::S4))
            .children(people.iter().enumerate().map(|(ix, p)| {
                let picked = person == Some(ix);
                let open_count = tasks
                    .iter()
                    .enumerate()
                    .filter(|(i, task)| task.assignee == ix && !done[*i])
                    .count();
                let (s_click, s_hover) = (state.clone(), state.clone());
                let id = format!("{key}-who-{ix}");
                // The chip eases its own hover wash, so the linked hover
                // lives on a wrapper.
                div()
                    .id(SharedString::from(format!("{id}-hover")))
                    .on_hover(move |hovered, _, cx| {
                        s_hover.update(cx, |s, cx| {
                            if *hovered {
                                s.hot_person = Some(ix);
                            } else if s.hot_person == Some(ix) {
                                s.hot_person = None;
                            }
                            cx.notify();
                        });
                    })
                    .child(
                        pink::chip(id, picked, ControlSize::Sm, &t)
                            .pl(px(2.))
                            .on_click(move |_, _, cx| {
                                s_click.update(cx, |s, cx| {
                                    s.person = if s.person == Some(ix) { None } else { Some(ix) };
                                    cx.notify();
                                })
                            })
                            .child(p.tile(&t))
                            .child(p.first_name())
                            .child(
                                div()
                                    .font_features(pink::tabular_figures())
                                    .text_color(t.readable_faint(0.8))
                                    .child(open_count.to_string()),
                            ),
                    )
            }));

        // Subtasks: Pink's activity alert rows with its task-list check.
        let rows = tasks.iter().enumerate().map(|(ix, task)| {
            let row_key = format!("{key}-task-{ix}");
            let (_, hot, dim) = segments[ix];
            let is_done = done[ix];
            let (s_toggle, s_hover) = (state.clone(), state.clone());
            let on_toggle = self.on_toggle.clone();
            let wash = t.fill(FillTier::Tertiary);
            div()
                .id(SharedString::from(row_key))
                .role(Role::CheckBox)
                .aria_label(task.title.clone())
                .aria_toggled(if is_done {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .tab_index(0)
                .h(px(ROW_H))
                .mx(px(-space::S6))
                .px(px(space::S6))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S8))
                .rounded(px(radius::SM))
                .cursor_pointer()
                .bg(motion::mix(wash.opacity(0.), wash, hot))
                .opacity(1. - 0.55 * dim)
                .focus_visible(move |s| pink::focus_ring(s, focus))
                .on_key_down(pink::swallow_activation_key)
                .on_hover(move |hovered, _, cx| {
                    s_hover.update(cx, |s, cx| {
                        if *hovered {
                            s.hot_task = Some(ix);
                        } else if s.hot_task == Some(ix) {
                            s.hot_task = None;
                        }
                        cx.notify();
                    });
                })
                .on_click(move |_, window, cx| {
                    let mut now_done = false;
                    s_toggle.update(cx, |s, cx| {
                        s.done[ix] = !s.done[ix];
                        now_done = s.done[ix];
                        cx.notify();
                    });
                    if let Some(f) = &on_toggle {
                        f(ix, now_done, window, cx);
                    }
                })
                .child(check_box(is_done, &t))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(if is_done {
                            t.readable_faint(0.8)
                        } else {
                            t.text.opacity(0.85)
                        })
                        .child(task.title.clone()),
                )
                .when_some(task.due.clone(), |row, due| {
                    row.child(
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .line_height(px(14.))
                            .text_color(t.readable_muted(0.55))
                            .child(due),
                    )
                })
                .when_some(people.get(task.assignee), |row, p| row.child(p.tile(&t)))
        });

        let body = div()
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(space::S8))
            .pt(px(space::S8))
            .when(body_alpha < 1., |el| {
                el.top(px(TOOL_BODY_RISE * (1. - body_alpha)))
                    .opacity(body_alpha)
            })
            .child(statuses)
            .when(!people.is_empty(), |el| el.child(people_row))
            .child(div().flex().flex_col().gap(px(space::S2)).children(rows))
            .child({
                let body_h = body_h.clone();
                canvas(
                    move |bounds, _, _| body_h.set(f32::from(bounds.size.height)),
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            });
        let measured = body_h.get();
        // The body stays mounted while the fold closes over it, and once
        // (clipped to nothing) so its first opening knows its height.
        let body_slot = div()
            .overflow_hidden()
            .when(fold < 0.999, |d| d.h(px(measured * fold)))
            .when(open || elapsed.is_some() || measured == 0., |d| {
                d.child(body)
            });

        // ---- footer (Pink board card footer) -----------------------------------
        let first_open = |of: Option<usize>| {
            tasks
                .iter()
                .enumerate()
                .find(|(ix, task)| !done[*ix] && of.is_none_or(|p| task.assignee == p))
                .map(|(_, task)| task.title.clone())
        };
        let meta: SharedString = match (hot_task.filter(|_| !open), focus_person) {
            // Pointing at a segment of the folded card names its subtask.
            (Some(ix), _) => {
                let task = &tasks[ix];
                let mut parts = vec![task.title.to_string()];
                if let Some(p) = people.get(task.assignee) {
                    parts.push(p.first_name().to_string());
                }
                parts.push(match (&task.due, done[ix]) {
                    (_, true) => "done".into(),
                    (Some(due), false) => format!("due {due}"),
                    (None, false) => "open".into(),
                });
                parts.join(", ").into()
            }
            (None, Some(p)) => {
                let left = tasks
                    .iter()
                    .enumerate()
                    .filter(|(ix, task)| task.assignee == p && !done[*ix])
                    .count();
                let who = people.get(p).map(|p| p.first_name()).unwrap_or_default();
                match first_open(Some(p)) {
                    Some(next) => format!("{who}, {left} left, next: {next}").into(),
                    None => format!("{who}, all done").into(),
                }
            }
            (None, None) => match (n, first_open(None)) {
                (0, _) => "No subtasks yet".into(),
                (_, None) => "Every subtask is done".into(),
                (_, Some(next)) => format!("{} left, next: {next}", n - done_count).into(),
            },
        };
        // The corner swaps content with the fold (Pink's sidebar corner):
        // the people at rest, Open once unfolded – the working trailer's quiet
        // action (Pink: transcript.rs Stop, 11/16 `content_muted`, hover wash).
        let corner = if open {
            let open_key = format!("{key}-open");
            let on_open = self.on_open.clone();
            div()
                .id(SharedString::from(open_key.clone()))
                .role(Role::Button)
                .aria_label("Open project")
                .tab_index(0)
                .flex_none()
                .h(px(TILE))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S4))
                .px(px(space::S6))
                .rounded(px(radius::CONTROL))
                .cursor_pointer()
                .line_height(px(16.))
                .text_color(t.content_muted)
                .opacity(body_alpha)
                .hover_wash(open_key, t.fill(FillTier::Tertiary))
                .focus_visible(move |s| pink::focus_ring(s, focus))
                .on_key_down(pink::swallow_activation_key)
                .when_some(on_open, |el, f| {
                    el.on_click(move |_, window, cx| f(window, cx))
                })
                .child("Open")
                .child(
                    svg()
                        .path(ICON_OPEN)
                        .size(px(12.))
                        .flex_none()
                        .text_color(t.content_muted),
                )
                .into_any_element()
        } else {
            div()
                .flex_none()
                .h(px(TILE))
                .flex()
                .flex_row()
                .gap(px(space::S2))
                .opacity(if elapsed.is_some() {
                    1. - body_alpha
                } else {
                    1.
                })
                .children(people.iter().enumerate().map(|(ix, p)| {
                    let s_hover = state.clone();
                    p.tile(&t)
                        .id(SharedString::from(format!("{key}-face-{ix}")))
                        .tooltip(pink::text_tooltip(p.name.clone()))
                        .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
                        .on_hover(move |hovered, _, cx| {
                            s_hover.update(cx, |s, cx| {
                                if *hovered {
                                    s.hot_person = Some(ix);
                                } else if s.hot_person == Some(ix) {
                                    s.hot_person = None;
                                }
                                cx.notify();
                            });
                        })
                }))
                .into_any_element()
        };
        let footer = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::S8))
            .mt(px(space::S8))
            .pt(px(space::S6))
            .border_t_1()
            .border_color(t.separator(false))
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(t.readable_muted(0.8))
            .child(div().flex_1().min_w_0().truncate().child(meta))
            .child(corner);

        // ---- the card (Pink board card frame) ------------------------------
        let hover_edge = t.accent.opacity(0.45);
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .flex()
            .flex_col()
            .px(px(PAD_X))
            .py(px(9.))
            .rounded(px(radius::LG))
            .border_1()
            .border_color(t.separator(true))
            .bg(t.fill(FillTier::Quaternary))
            .font_family(pink::SANS)
            .text_color(t.text)
            .hover(move |s| s.border_color(hover_edge))
            .child(header)
            .child(body_slot)
            .child(footer)
    }
}

/// Pink's project hues (violet, green, amber, blue, Pink, coral).
const HUES: [f32; 6] = [250., 150., 40., 205., 325., 12.];

fn billing() -> ProjectCard {
    ProjectCard::new("project-billing", "Billing v2 migration")
        .due("Nov 14")
        .status(ProjectStatus::AtRisk)
        .person(ProjectPerson::new("Ana Kovač", HUES[0]))
        .person(ProjectPerson::new("Kofi Mensah", HUES[1]))
        .person(ProjectPerson::new("Mei Watanabe", HUES[2]))
        .subtasks([
            Subtask::new("Map old plans to new price IDs", 0).done(true),
            Subtask::new("Dual-write invoices to both ledgers", 1).done(true),
            Subtask::new("Backfill 2023 subscriptions", 1).done(true),
            Subtask::new("Reconcile tax lines per region", 2).due("Nov 6"),
            Subtask::new("Switch webhooks to v2 events", 0).due("Nov 8"),
            Subtask::new("Shadow-run the month-end close", 2).due("Nov 12"),
            Subtask::new("Turn off the v1 writer", 1).due("Nov 14"),
        ])
}

fn help_center() -> ProjectCard {
    ProjectCard::new("project-help", "Help center redesign")
        .due("Dec 2")
        .status(ProjectStatus::OnTrack)
        .person(ProjectPerson::new("Lena Hart", HUES[3]))
        .person(ProjectPerson::new("Omar Saleh", HUES[4]))
        .subtasks([
            Subtask::new("Audit the top 50 articles", 0).done(true),
            Subtask::new("New article template", 1).done(true),
            Subtask::new("Search that forgives typos", 1).due("Nov 20"),
            Subtask::new("Dark-mode screenshots", 0).due("Nov 26"),
            Subtask::new("Feedback widget on every page", 1).due("Dec 1"),
        ])
}

fn sso() -> ProjectCard {
    ProjectCard::new("project-sso", "SSO for team workspaces")
        .due("Nov 30")
        .status(ProjectStatus::Blocked)
        .person(ProjectPerson::new("Rui Costa", HUES[5]))
        .person(ProjectPerson::new("Ana Kovač", HUES[0]))
        .subtasks([
            Subtask::new("SAML metadata upload", 0).done(true),
            Subtask::new("Okta test tenant", 0).due("Nov 12"),
            Subtask::new("Just-in-time provisioning", 1).due("Nov 22"),
            Subtask::new("Enforce SSO per workspace", 1).due("Nov 30"),
        ])
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .flex()
            .items_start()
            .gap(px(space::S24))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S12))
                    .child(help_center().width(340.))
                    .child(sso().width(340.)),
            )
            .child(billing().width(380.).default_open(true))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::size;

    #[test]
    fn ring_hit_test_runs_clockwise_from_top() {
        let b = Bounds::new(point(px(0.), px(0.)), size(px(RING), px(RING)));
        let c = RING / 2.;
        // Just right of 12 o'clock is the first of four segments.
        assert_eq!(segment_at(b, point(px(c + 2.), px(c - RING_R)), 4), Some(0));
        // 3 o'clock starts the second.
        assert_eq!(segment_at(b, point(px(c + RING_R), px(c + 1.)), 4), Some(1));
        // 9 o'clock (just above) is the last.
        assert_eq!(segment_at(b, point(px(c - RING_R), px(c - 1.)), 4), Some(3));
        // The middle is not on the ring.
        assert_eq!(segment_at(b, point(px(c), px(c)), 4), None);
    }
}
