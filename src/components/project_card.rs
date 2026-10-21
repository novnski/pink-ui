//! Project Card – a two-line project summary that unfolds while you look.
//!
//! At rest the card shows the name, status, due date, the people on it and a
//! progress ring with one segment per subtask. Hovering unfolds it (click the
//! header to keep it open): a status picker, a people filter and the
//! subtasks. Everything is linked by hover – point at a subtask and its ring
//! segment lifts, point at a person and their subtasks and segments stand
//! out, point at a segment and its subtask lights up. Ticking a subtask fills
//! its segment.
//!
//! ```ignore
//! ProjectCard::new("billing", "Billing v2 migration")
//!     .due("Due Nov 14")
//!     .person(ProjectPerson::new("Ana Kovač", 30.))
//!     .subtask(Subtask::new("Map old plans to new prices", 0).done(true))
//! ```

use std::{cell::Cell, f32::consts::PI, rc::Rc};

use gpui::{
    AnyView, App, Bounds, ElementId, FontWeight, Hsla, IntoElement, MouseMoveEvent, PathBuilder,
    Pixels, SharedString, Window, canvas, div, point, prelude::*, px, svg,
};

use crate::{
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, oklch, radius, space, text},
    motion::{self, GLIDE, HOVER_FADE, MENU_IN},
    story,
};

pub const NAME: &str = "Project Card";
pub const DESCRIPTION: &str = "A two-line project summary that unfolds on hover into a status picker, a people filter and its subtasks – all linked to a progress ring with one segment per subtask";

const ICON_CHECK: &str = "icons/project-card/check.svg";
const ICON_ARROW: &str = "icons/project-card/arrow.svg";
const ICON_CALENDAR: &str = "icons/project-card/calendar.svg";

const RING: f32 = 44.0;
const RING_R: f32 = 17.0;
const TASK_H: f32 = 32.0;

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

    fn tone(self) -> Tone {
        match self {
            ProjectStatus::OnTrack => Tone::Success,
            ProjectStatus::AtRisk => Tone::Warning,
            ProjectStatus::Blocked => Tone::Danger,
            ProjectStatus::Done => Tone::Accent,
        }
    }
}

/// Someone on the project, drawn as a tinted monogram.
#[derive(Clone, Debug)]
pub struct ProjectPerson {
    pub name: SharedString,
    /// Hue of the monogram, in degrees.
    pub hue: f32,
}

impl ProjectPerson {
    pub fn new(name: impl Into<SharedString>, hue: f32) -> Self {
        Self {
            name: name.into(),
            hue,
        }
    }

    fn initials(&self) -> String {
        self.name
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }

    fn first_name(&self) -> SharedString {
        self.name
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string()
            .into()
    }

    fn colors(&self, t: &Luna) -> (Hsla, Hsla) {
        if t.dark {
            (oklch(0.36, 0.07, self.hue), oklch(0.9, 0.07, self.hue))
        } else {
            (oklch(0.93, 0.045, self.hue), oklch(0.42, 0.11, self.hue))
        }
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
    pinned_open: bool,
    hovered: bool,
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
    if !(RING_R - 9.0..=RING_R + 9.0).contains(&dist) {
        return None;
    }
    // Angle clockwise from 12 o'clock, 0..2π.
    let a = (dx.atan2(-dy) + 2. * PI) % (2. * PI);
    Some(((a / (2. * PI)) * n as f32) as usize % n)
}

impl RenderOnce for ProjectCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let initial_done: Vec<bool> = self.subtasks.iter().map(|s| s.done).collect();
        let (initial_status, default_open) = (self.status, self.default_open);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            move |_, _| CardState {
                pinned_open: default_open,
                hovered: false,
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
        let (pinned_open, hovered, status, done, hot_task, hot_person, person) = (
            s.pinned_open,
            s.hovered,
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

        let open = motion::follow(
            &format!("{key}-open"),
            if pinned_open || hovered { 1. } else { 0. },
            GLIDE,
        );
        // Who the card is focused on right now: a hovered person wins over
        // the picked one.
        let focus_person = hot_person.or(person);
        let task_hot = |ix: usize| -> bool {
            hot_task == Some(ix) || focus_person.is_some_and(|p| tasks[ix].assignee == p)
        };
        let any_hot = hot_task.is_some() || focus_person.is_some();

        // ---- ring --------------------------------------------------------------
        let segments: Vec<(f32, f32, f32)> = (0..n)
            .map(|ix| {
                let fill = motion::follow(
                    &format!("{key}-seg{ix}-done"),
                    if done[ix] { 1. } else { 0. },
                    GLIDE,
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
            let track = if t.dark {
                t.fill(FillTier::Primary)
            } else {
                t.border_strong
            };
            let (accent, muted) = (t.accent, t.text_muted);
            let bounds_cell = ring_bounds.clone();
            let segs = segments.clone();
            let paint = canvas(
                move |bounds, _, _| bounds_cell.set(Some(bounds)),
                move |bounds, _, window, _| {
                    let c = bounds.center();
                    let n = segs.len().max(1) as f32;
                    let gap = if segs.len() > 1 { 0.16 } else { 0. };
                    for (ix, (fill, hot, dim)) in segs.iter().enumerate() {
                        let a0 = -PI / 2. + ix as f32 / n * 2. * PI + gap / 2.;
                        let a1 = -PI / 2. + (ix + 1) as f32 / n * 2. * PI - gap / 2.;
                        let r = RING_R + hot * 1.0;
                        let p = |a: f32| point(c.x + px(r * a.cos()), c.y + px(r * a.sin()));
                        let mut path = PathBuilder::stroke(px(3. + 2. * hot));
                        path.move_to(p(a0));
                        path.arc_to(point(px(r), px(r)), px(0.), a1 - a0 > PI, true, p(a1));
                        let lifted = motion::mix(track, muted, hot * 0.8);
                        let color = motion::mix(lifted, accent, *fill);
                        let color = color.opacity(1. - 0.6 * dim);
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
            let state_move = state.clone();
            let state_out = state.clone();
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
                        .text_size(px(text::CAPTION))
                        .font_weight(FontWeight::SEMIBOLD)
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

        // ---- header ------------------------------------------------------------
        let tone = status.tone();
        let tone_color = tone.color(&t);
        // An overlapping stack: each face tucks 5px under the previous one.
        let stack_w = 20. + people.len().saturating_sub(1) as f32 * 15.;
        let avatars = div()
            .flex()
            .flex_none()
            .w(px(stack_w))
            .items_center()
            .children(people.iter().enumerate().map(|(ix, p)| {
                let on = motion::follow(
                    &format!("{key}-face{ix}"),
                    if focus_person == Some(ix) { 1. } else { 0. },
                    HOVER_FADE,
                );
                let dim = focus_person.is_some() && focus_person != Some(ix);
                avatar(p, 20., &t)
                    .when(ix > 0, |d| d.ml(px(-5.)))
                    .border_2()
                    .border_color(motion::mix(t.surface_card, t.accent, on))
                    .opacity(if dim { 0.45 } else { 1. })
            }));
        let header = {
            let state = state.clone();
            div()
                .id(SharedString::from(format!("{key}-head")))
                .flex()
                .items_center()
                .gap(px(space::S12))
                .px(px(space::S16))
                .py(px(space::S12))
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.pinned_open = !s.pinned_open;
                        cx.notify();
                    })
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(space::S4))
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::CALLOUT))
                                .line_height(px(20.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(self.name.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S8))
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_faint)
                                .child(motion::rise_in(
                                    SharedString::from(format!("{key}-status-{status:?}")),
                                    MENU_IN,
                                    3.,
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(space::S4))
                                        .h(px(20.))
                                        .px(px(6.))
                                        .rounded(px(radius::SM))
                                        .bg(tone_color.opacity(if t.dark { 0.16 } else { 0.10 }))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(tone_color)
                                        .child(div().size(px(6.)).rounded_full().bg(tone_color))
                                        .child(status.label()),
                                ))
                                .when_some(self.due.clone(), |d, due| {
                                    d.child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(space::S4))
                                            .child(
                                                svg()
                                                    .path(ICON_CALENDAR)
                                                    .size(px(12.))
                                                    .text_color(t.text_faint),
                                            )
                                            .child(due),
                                    )
                                })
                                .child(div().flex_1())
                                .child(avatars),
                        ),
                )
                .child(ring)
        };

        // ---- unfolded body ---------------------------------------------------------
        // Status: a segmented picker whose thumb glides and takes the tone.
        let seg_w = (self.width - space::S16 * 2. - 4.) / ProjectStatus::ALL.len() as f32;
        let selected_ix = ProjectStatus::ALL
            .iter()
            .position(|st| *st == status)
            .unwrap_or(0);
        let thumb_x = motion::follow(
            &format!("{key}-st-thumb"),
            selected_ix as f32 * seg_w,
            GLIDE,
        );
        let statuses = div()
            .relative()
            .flex()
            .h(px(28.))
            .p(px(2.))
            .rounded(px(radius::MD))
            .bg(t.fill(FillTier::Tertiary))
            .child(
                div()
                    .absolute()
                    .top(px(2.))
                    .left(px(2. + thumb_x))
                    .w(px(seg_w))
                    .h(px(24.))
                    .rounded(px(radius::SM))
                    .bg(if t.dark {
                        t.surface_raised
                    } else {
                        t.surface_card
                    })
                    .border_1()
                    .border_color(tone_color.opacity(0.35))
                    .shadow(t.card_shadow()),
            )
            .children(ProjectStatus::ALL.iter().map(|st| {
                let st = *st;
                let c = st.tone().color(&t);
                let state = state.clone();
                let on_status = self.on_status.clone();
                let hover_key = format!("{key}-st-{st:?}");
                let on = motion::follow(
                    &format!("{hover_key}-on"),
                    if st == status { 1. } else { 0. },
                    HOVER_FADE,
                );
                let rest = motion::mix(t.text_muted, t.text, on);
                div()
                    .id(SharedString::from(hover_key.clone()))
                    .relative()
                    .w(px(seg_w))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(space::S6))
                    .cursor_pointer()
                    .text_size(px(text::FOOTNOTE))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(motion::hover_blend(&hover_key, rest, t.text))
                    .on_hover(motion::hover_listener(hover_key))
                    .on_click(move |_, window, cx| {
                        state.update(cx, |s, cx| {
                            s.status = st;
                            cx.notify();
                        });
                        if let Some(f) = &on_status {
                            f(st, window, cx);
                        }
                    })
                    .child(div().size(px(6.)).rounded_full().bg(c))
                    .child(st.label())
            }));
        let people_row =
            div()
                .flex()
                .flex_wrap()
                .gap(px(space::S4))
                .children(people.iter().enumerate().map(|(ix, p)| {
                    let picked = person == Some(ix);
                    let open_count = tasks
                        .iter()
                        .enumerate()
                        .filter(|(i, t)| t.assignee == ix && !done[*i])
                        .count();
                    let (s_click, s_hover) = (state.clone(), state.clone());
                    let id = format!("{key}-who-{ix}");
                    // luna's chip eases its own hover wash, so the linked hover
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
                            luna::chip(id, picked, ControlSize::Sm, &t)
                                .pl(px(3.))
                                .gap(px(space::S4))
                                .on_click(move |_, _, cx| {
                                    s_click.update(cx, |s, cx| {
                                        s.person =
                                            if s.person == Some(ix) { None } else { Some(ix) };
                                        cx.notify();
                                    })
                                })
                                .child(avatar(p, 18., &t))
                                .child(p.first_name())
                                .child(
                                    div()
                                        .font_family(luna::MONO)
                                        .text_size(px(10.))
                                        .opacity(0.65)
                                        .child(open_count.to_string()),
                                ),
                        )
                }));

        let rows = tasks.iter().enumerate().map(|(ix, task)| {
            let row_key = format!("{key}-task-{ix}");
            let hot = segments[ix].1;
            let fill = segments[ix].0;
            let dim = focus_person.is_some_and(|p| task.assignee != p);
            let (s_toggle, s_hover) = (state.clone(), state.clone());
            let on_toggle = self.on_toggle.clone();
            let hover_key = row_key.clone();
            let check = div()
                .id(SharedString::from(format!("{row_key}-check")))
                .size(px(16.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(motion::mix(t.border_strong, t.accent, fill))
                .bg(t.accent.opacity(fill))
                .cursor_pointer()
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
                .when(fill > 0.05, |d| {
                    d.child(
                        svg()
                            .path(ICON_CHECK)
                            .size(px(12.))
                            .text_color(t.on_accent.opacity(fill)),
                    )
                });
            let who = people.get(task.assignee);
            div()
                .id(SharedString::from(row_key))
                .h(px(TASK_H))
                .mx(px(-space::S6))
                .px(px(space::S6))
                .flex()
                .items_center()
                .gap(px(space::S10))
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Tertiary).opacity(hot))
                .opacity(if dim { 0.4 } else { 1. })
                .on_hover(move |hovered, window, cx| {
                    motion::set_hover(&hover_key, *hovered, motion::reduced_motion(cx));
                    s_hover.update(cx, |s, cx| {
                        if *hovered {
                            s.hot_task = Some(ix);
                        } else if s.hot_task == Some(ix) {
                            s.hot_task = None;
                        }
                        cx.notify();
                    });
                    window.refresh();
                })
                .child(check)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(text::BODY))
                        .text_color(motion::mix(t.text, t.text_faint, fill))
                        .when(done[ix], |d| d.line_through())
                        .child(task.title.clone()),
                )
                .when_some(task.due.clone(), |d, due| {
                    d.child(
                        div()
                            .flex_none()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(due),
                    )
                })
                .when_some(who, |d, p| d.child(avatar(p, 18., &t)))
        });

        let next = tasks
            .iter()
            .enumerate()
            .find(|(ix, _)| !done[*ix])
            .map(|(_, task)| task.title.clone());
        let left = n - done_count;
        let open_key = format!("{key}-open-btn");
        let nudge = motion::hover_t(&format!("luna-btn-{open_key}")) * 2.;
        let on_open = self.on_open.clone();
        let footer =
            div()
                .flex()
                .items_center()
                .gap(px(space::S8))
                .pt(px(space::S10))
                .border_t_1()
                .border_color(t.border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_faint)
                        .child(match (left, next) {
                            (0, _) | (_, None) => "Every subtask is done".to_string(),
                            (left, Some(next)) => format!("{left} left · next: {next}"),
                        }),
                )
                .child(
                    luna::button(open_key, ButtonKind::Secondary, ControlSize::Sm, &t)
                        .child("Open")
                        .child(div().w(px(14.)).pl(px(nudge)).child(
                            luna::glyph(ICON_ARROW, ControlSize::Sm).text_color(t.text_muted),
                        ))
                        .when_some(on_open, |b, f| {
                            b.on_click(move |_, window, cx| f(window, cx))
                        }),
                );

        let body = div()
            .relative()
            .flex()
            .flex_col()
            .gap(px(space::S10))
            .px(px(space::S16))
            .pt(px(space::S12))
            .pb(px(space::S12))
            .border_t_1()
            .border_color(t.border)
            .child(statuses)
            .child(people_row)
            .child(div().flex().flex_col().pt(px(space::S2)).children(rows))
            .child(footer)
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
        let body_slot = div()
            .overflow_hidden()
            .when(open < 0.999, |d| d.h(px(measured * open)))
            .opacity(open.powf(0.7))
            .when(open > 0.001 || measured == 0., |d| d.child(body));

        motion::keep_alive(window);
        let hover_state = state.clone();
        luna::card(&t)
            .id(self.id.clone())
            .w(px(self.width))
            .overflow_hidden()
            .font_family(luna::SANS)
            .text_color(t.text)
            .border_color(motion::mix(t.border, t.border_strong, open))
            .shadow(if open > 0.5 {
                t.menu_shadow()
            } else {
                t.card_shadow()
            })
            .on_hover(move |hovered, _, cx| {
                let hovered = *hovered;
                hover_state.update(cx, |s, cx| {
                    s.hovered = hovered;
                    if !hovered {
                        s.hot_task = None;
                        s.hot_person = None;
                    }
                    cx.notify();
                })
            })
            .child(header)
            .child(body_slot)
    }
}

/// A round monogram in the person's hue.
fn avatar(p: &ProjectPerson, size: f32, t: &Luna) -> gpui::Div {
    let (bg, fg) = p.colors(t);
    div()
        .size(px(size))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(bg)
        .text_size(px((size * 0.42).round()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(p.initials())
}

fn billing() -> ProjectCard {
    ProjectCard::new("project-billing", "Billing v2 migration")
        .due("Nov 14")
        .status(ProjectStatus::AtRisk)
        .person(ProjectPerson::new("Ana Kovač", 25.))
        .person(ProjectPerson::new("Kofi Mensah", 150.))
        .person(ProjectPerson::new("Mei Watanabe", 285.))
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
        .person(ProjectPerson::new("Lena Hart", 210.))
        .person(ProjectPerson::new("Omar Saleh", 70.))
        .subtasks([
            Subtask::new("Audit the top 50 articles", 0).done(true),
            Subtask::new("New article template", 1).done(true),
            Subtask::new("Search that forgives typos", 1).due("Nov 20"),
            Subtask::new("Dark-mode screenshots", 0).due("Nov 26"),
            Subtask::new("Feedback widget on every page", 1).due("Dec 1"),
        ])
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .flex()
            .items_start()
            .gap(px(space::S24))
            .child(help_center())
            .child(billing().default_open(true))
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
