//! Setup Guide – an onboarding checklist: a progress ring, accordion task
//! groups and tasks that check off with a drawn strike-through.
//!
//! Only one group is open at a time. The selected task in the open group
//! unfolds its hint and call to action; finishing the last task of a group
//! folds it away and opens the next unfinished one. The panel minimizes to
//! its header and can be dismissed to a small "resume" pill.
//!
//! ```ignore
//! SetupGuide::new("launch", "Launch checklist")
//!     .group(
//!         SetupGroup::new("team", "Bring your team", ICON)
//!             .task(SetupTask::new("invite", "Invite two teammates").minutes(2))
//!             .task(SetupTask::new("roles", "Set default roles").done()),
//!     )
//!     .on_toggle(|task, done, _, _| println!("{task}: {done}"))
//! ```

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    f32::consts::PI,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use gpui::{
    AnyElement, AnyView, App, ElementId, FillOptions, FillRule, FontWeight, Hsla, IntoElement,
    PathBuilder, PathStyle, Pixels, Point, SharedString, Task, Transformation, Window, canvas, div,
    point, prelude::*, px, radians, relative, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, AnimationExt, GLIDE, MotionSpec},
    story,
};

pub const NAME: &str = "Setup Guide";
pub const DESCRIPTION: &str = "Onboarding checklist with a progress ring, accordion task groups and tasks that strike through as they get done";

const ICON_CHECK: &str = "icons/setup-guide/check.svg";
const ICON_CHEVRON: &str = "icons/setup-guide/chevron-down.svg";
const ICON_MINIMIZE: &str = "icons/setup-guide/minimize.svg";
const ICON_CLOSE: &str = "icons/setup-guide/x.svg";
const ICON_SPARKLE: &str = "icons/setup-guide/sparkle.svg";
const ICON_ARROW: &str = "icons/setup-guide/arrow-right.svg";

/// Delay before a finished group folds away and the next one opens.
const ADVANCE_AFTER: Duration = Duration::from_millis(700);
/// The strike-through drawing across a finished task.
const STRIKE: MotionSpec = MotionSpec::new(360, motion::EASE_OUT_QUINT);

type ToggleHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type TaskHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One checklist item.
#[derive(Clone)]
pub struct SetupTask {
    pub id: SharedString,
    pub title: SharedString,
    /// A hint shown while the task is selected.
    pub hint: Option<SharedString>,
    /// Label of the call to action shown while the task is selected.
    pub action: Option<SharedString>,
    /// Rough effort, summed into the "min left" estimate.
    pub minutes: u32,
    pub done: bool,
}

impl SetupTask {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            hint: None,
            action: None,
            minutes: 1,
            done: false,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn action(mut self, label: impl Into<SharedString>) -> Self {
        self.action = Some(label.into());
        self
    }

    pub fn minutes(mut self, minutes: u32) -> Self {
        self.minutes = minutes;
        self
    }

    /// Starts the task checked off.
    pub fn done(mut self) -> Self {
        self.done = true;
        self
    }
}

/// A titled group of tasks with its own glyph.
#[derive(Clone)]
pub struct SetupGroup {
    pub id: SharedString,
    pub title: SharedString,
    pub icon: SharedString,
    pub tasks: Vec<SetupTask>,
}

impl SetupGroup {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        icon: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: icon.into(),
            tasks: Vec::new(),
        }
    }

    pub fn task(mut self, task: SetupTask) -> Self {
        self.tasks.push(task);
        self
    }
}

/// Group ids with their task ids, in order – what the state needs to move on.
type Outline = Vec<(SharedString, Vec<SharedString>)>;

struct GuideState {
    seeded: bool,
    done: HashSet<SharedString>,
    /// Bumped each time a task gets checked, so its check mark pops again.
    checks: HashMap<SharedString, usize>,
    open: Option<SharedString>,
    active: Option<SharedString>,
    minimized: bool,
    dismissed: bool,
    heights: HashMap<SharedString, Rc<Cell<f32>>>,
    advance: Option<Task<()>>,
}

impl GuideState {
    fn new() -> Self {
        Self {
            seeded: false,
            done: HashSet::new(),
            checks: HashMap::new(),
            open: None,
            active: None,
            minimized: false,
            dismissed: false,
            heights: HashMap::new(),
            advance: None,
        }
    }

    fn height(&mut self, key: &SharedString) -> Rc<Cell<f32>> {
        self.heights.entry(key.clone()).or_default().clone()
    }

    fn first_open(&self, tasks: &[SharedString]) -> Option<SharedString> {
        tasks.iter().find(|t| !self.done.contains(*t)).cloned()
    }

    /// Opens `group` (or the first unfinished one) and selects its first open task.
    fn focus_group(&mut self, outline: &Outline, group: Option<&SharedString>) {
        let pick = group
            .and_then(|g| outline.iter().find(|(id, _)| id == g))
            .or_else(|| {
                outline
                    .iter()
                    .find(|(_, tasks)| self.first_open(tasks).is_some())
            });
        match pick {
            Some((id, tasks)) => {
                self.open = Some(id.clone());
                self.active = self.first_open(tasks);
            }
            None => {
                self.open = None;
                self.active = None;
            }
        }
    }

    fn toggle(&mut self, outline: Rc<Outline>, task: &SharedString, cx: &mut gpui::Context<Self>) {
        let done = !self.done.contains(task);
        if done {
            self.done.insert(task.clone());
            *self.checks.entry(task.clone()).or_default() += 1;
        } else {
            self.done.remove(task);
        }
        let Some((group, tasks)) = outline.iter().find(|(_, tasks)| tasks.contains(task)) else {
            return;
        };
        if done && self.active.as_ref() == Some(task) {
            self.active = self.first_open(tasks);
        } else if !done {
            self.active = Some(task.clone());
        }
        let group_done = tasks.iter().all(|t| self.done.contains(t));
        self.advance = None;
        if done && group_done {
            // Let the check land, then fold the group and open the next one.
            let group = group.clone();
            self.advance = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(ADVANCE_AFTER).await;
                this.update(cx, |s, cx| {
                    if s.open.as_ref() == Some(&group) {
                        s.focus_group(&outline, None);
                    }
                    s.advance = None;
                    cx.notify();
                })
                .ok();
            }));
        }
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct SetupGuide {
    id: ElementId,
    title: SharedString,
    groups: Vec<SetupGroup>,
    on_toggle: Option<ToggleHandler>,
    on_action: Option<TaskHandler>,
    on_dismiss: Option<Handler>,
    width: Pixels,
}

impl SetupGuide {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            groups: Vec::new(),
            on_toggle: None,
            on_action: None,
            on_dismiss: None,
            width: px(392.),
        }
    }

    pub fn group(mut self, group: SetupGroup) -> Self {
        self.groups.push(group);
        self
    }

    pub fn groups(mut self, groups: impl IntoIterator<Item = SetupGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Called when a task is checked (`true`) or unchecked.
    pub fn on_toggle(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    /// Called when the selected task's call to action is pressed.
    pub fn on_action(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }

    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }
}

impl RenderOnce for SetupGuide {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let element = self.build(window, cx);
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl SetupGuide {
    fn build(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| GuideState::new(),
        );
        let outline: Rc<Outline> = Rc::new(
            self.groups
                .iter()
                .map(|g| (g.id.clone(), g.tasks.iter().map(|t| t.id.clone()).collect()))
                .collect(),
        );
        if !state.read(cx).seeded {
            let groups = self.groups.clone();
            let outline = outline.clone();
            state.update(cx, |s, _| {
                s.seeded = true;
                for task in groups.iter().flat_map(|g| g.tasks.iter()) {
                    if task.done {
                        s.done.insert(task.id.clone());
                    }
                }
                s.focus_group(&outline, None);
            });
        }

        let total: usize = self.groups.iter().map(|g| g.tasks.len()).sum();
        let (done_count, minutes_left) = {
            let s = state.read(cx);
            let all = self.groups.iter().flat_map(|g| g.tasks.iter());
            let done = all.clone().filter(|t| s.done.contains(&t.id)).count();
            let left: u32 = all
                .filter(|t| !s.done.contains(&t.id))
                .map(|t| t.minutes)
                .sum();
            (done, left)
        };
        let complete = total > 0 && done_count == total;

        if state.read(cx).dismissed {
            return resume_pill(&key, &state, done_count, total, &t).into_any_element();
        }

        let ratio = if total == 0 {
            0.
        } else {
            done_count as f32 / total as f32
        };
        let ratio = motion::follow(&format!("{key}-ratio"), ratio, GLIDE);
        let ring_color = motion::mix(t.accent, t.success, if complete { 1. } else { 0. });
        let minimized = state.read(cx).minimized;
        let fold = motion::follow(
            &format!("{key}-fold"),
            if minimized { 1. } else { 0. },
            GLIDE,
        );

        let subtitle: SharedString = if complete {
            "Everything is in place".into()
        } else {
            format!("{done_count} of {total} done · about {minutes_left} min left").into()
        };

        let header = div()
            .flex()
            .items_center()
            .gap(px(space::S12))
            .p(px(space::S16))
            .child(
                div()
                    .relative()
                    .size(px(44.))
                    .flex_none()
                    .child(progress_ring(
                        ratio,
                        44.,
                        3.5,
                        t.fill(FillTier::Primary),
                        ring_color,
                    ))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(text::CAPTION))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text)
                            .child(if complete {
                                svg()
                                    .path(ICON_CHECK)
                                    .size(px(16.))
                                    .text_color(t.success)
                                    .into_any_element()
                            } else {
                                format!("{done_count}/{total}").into_any_element()
                            }),
                    ),
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
                            .text_color(t.text)
                            .child(self.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(t.text_muted)
                            .child(subtitle),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(px(space::S2))
                    .child({
                        let state = state.clone();
                        let id: SharedString = format!("{key}-minimize").into();
                        let hover = format!("luna-icon-{id}");
                        div()
                            .id(id.clone())
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::MD))
                            .cursor_pointer()
                            .hover_fill(
                                hover.clone(),
                                t.fill(FillTier::Tertiary).opacity(0.),
                                t.fill(FillTier::Tertiary),
                            )
                            .child(
                                svg()
                                    .path(ICON_MINIMIZE)
                                    .size(px(14.))
                                    .text_color(motion::hover_blend(&hover, t.text_muted, t.text))
                                    .with_transformation(Transformation::rotate(radians(
                                        PI * fold,
                                    ))),
                            )
                            .on_click(move |_, _, cx| {
                                state.update(cx, |s, cx| {
                                    s.minimized = !s.minimized;
                                    cx.notify();
                                })
                            })
                    })
                    .child({
                        let state = state.clone();
                        let on_dismiss = self.on_dismiss.clone();
                        luna::icon_button(format!("{key}-dismiss"), ICON_CLOSE, ControlSize::Md, &t)
                            .on_click(move |_, window, cx| {
                                state.update(cx, |s, cx| {
                                    s.dismissed = true;
                                    cx.notify();
                                });
                                if let Some(f) = &on_dismiss {
                                    f(window, cx);
                                }
                            })
                    }),
            );

        let groups: Vec<_> = self
            .groups
            .iter()
            .enumerate()
            .map(|(ix, group)| {
                group_view(
                    &key,
                    ix,
                    group,
                    &state,
                    &outline,
                    self.on_toggle.clone(),
                    self.on_action.clone(),
                    &t,
                    cx,
                )
            })
            .collect();

        let body_height = state.update(cx, |s, _| s.height(&"body".into()));
        let body = div()
            .flex()
            .flex_col()
            .child(luna::divider(&t))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S2))
                    .p(px(space::S6))
                    .children(groups),
            )
            .when(complete, |d| {
                d.child(motion::rise_in(
                    SharedString::from(format!("{key}-complete")),
                    motion::ROW_IN,
                    6.,
                    div()
                        .mx(px(space::S12))
                        .mb(px(space::S12))
                        .p(px(space::S12))
                        .flex()
                        .items_center()
                        .gap(px(space::S10))
                        .rounded(px(radius::MD))
                        .bg(t.success.opacity(if t.dark { 0.12 } else { 0.08 }))
                        .child(svg().path(ICON_SPARKLE).size(px(16.)).text_color(t.success))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(px(text::BODY))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(t.text)
                                        .child("You're ready to launch"),
                                )
                                .child(
                                    div()
                                        .text_size(px(text::FOOTNOTE))
                                        .text_color(t.text_muted)
                                        .child("Your workspace has everything it needs."),
                                ),
                        ),
                ))
            });

        luna::card(&t)
            .id(self.id.clone())
            .w(self.width)
            .font_family(luna::SANS)
            .text_color(t.text)
            .overflow_hidden()
            .child(header)
            .child(collapse(
                &format!("{key}-body"),
                !minimized,
                body_height,
                body,
            ))
            .into_any_element()
    }
}

#[allow(clippy::too_many_arguments)]
fn group_view(
    key: &str,
    ix: usize,
    group: &SetupGroup,
    state: &gpui::Entity<GuideState>,
    outline: &Rc<Outline>,
    on_toggle: Option<ToggleHandler>,
    on_action: Option<TaskHandler>,
    t: &Luna,
    cx: &mut App,
) -> AnyElement {
    let gkey = format!("{key}-group-{}", group.id);
    let height = state.update(cx, |s, _| s.height(&group.id));
    let s = state.read(cx);
    let open = s.open.as_ref() == Some(&group.id);
    let done = group
        .tasks
        .iter()
        .filter(|t| s.done.contains(&t.id))
        .count();
    let complete = !group.tasks.is_empty() && done == group.tasks.len();
    let turn = motion::follow(
        &format!("{gkey}-chevron"),
        if open { 1. } else { 0. },
        GLIDE,
    );
    let hover_key = format!("{gkey}-head");

    let tile_bg = if complete {
        t.success.opacity(if t.dark { 0.16 } else { 0.10 })
    } else {
        t.fill(FillTier::Tertiary)
    };
    let head = {
        let state = state.clone();
        let outline = outline.clone();
        let gid = group.id.clone();
        div()
            .id(SharedString::from(hover_key.clone()))
            .flex()
            .items_center()
            .gap(px(space::S10))
            .h(px(40.))
            .px(px(space::S10))
            .rounded(px(radius::MD))
            .cursor_pointer()
            .hover_fill(
                hover_key.clone(),
                t.fill(FillTier::Quaternary).opacity(0.),
                t.fill(FillTier::Quaternary),
            )
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if s.open.as_ref() == Some(&gid) {
                        s.open = None;
                        s.active = None;
                    } else {
                        s.focus_group(&outline, Some(&gid));
                    }
                    s.advance = None;
                    cx.notify();
                })
            })
            .child(
                div()
                    .size(px(26.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::SM))
                    .bg(tile_bg)
                    .child(
                        svg()
                            .path(if complete {
                                SharedString::from(ICON_CHECK)
                            } else {
                                group.icon.clone()
                            })
                            .size(px(14.))
                            .text_color(if complete { t.success } else { t.text_muted }),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(text::BODY))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(if complete { t.text_muted } else { t.text })
                    .child(group.title.clone()),
            )
            .child(if complete {
                luna::badge("Done", Tone::Success, t).into_any_element()
            } else {
                div()
                    .text_size(px(text::FOOTNOTE))
                    .font_family(luna::MONO)
                    .text_color(t.text_faint)
                    .child(format!("{done}/{}", group.tasks.len()))
                    .into_any_element()
            })
            .child(
                svg()
                    .path(ICON_CHEVRON)
                    .size(px(14.))
                    .flex_none()
                    .text_color(motion::hover_blend(&hover_key, t.text_faint, t.text_muted))
                    .with_transformation(Transformation::rotate(radians(PI * turn))),
            )
    };

    let active = s.active.clone();
    let tasks: Vec<_> = group
        .tasks
        .iter()
        .map(|task| {
            let is_done = s.done.contains(&task.id);
            let is_active = active.as_ref() == Some(&task.id) && !is_done;
            let checks = s.checks.get(&task.id).copied().unwrap_or(0);
            task_row(
                key,
                ix,
                task,
                is_done,
                is_active,
                checks,
                state,
                outline,
                on_toggle.clone(),
                on_action.clone(),
                t,
            )
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .child(head)
        .child(collapse(
            &gkey,
            open,
            height,
            div()
                .flex()
                .flex_col()
                .gap(px(space::S2))
                .pt(px(space::S2))
                .pb(px(space::S6))
                .children(tasks),
        ))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn task_row(
    key: &str,
    group_ix: usize,
    task: &SetupTask,
    done: bool,
    active: bool,
    checks: usize,
    state: &gpui::Entity<GuideState>,
    outline: &Rc<Outline>,
    on_toggle: Option<ToggleHandler>,
    on_action: Option<TaskHandler>,
    t: &Luna,
) -> impl IntoElement {
    let tkey = format!("{key}-task-{}", task.id);
    let strike = motion::follow(
        &format!("{tkey}-strike"),
        if done { 1. } else { 0. },
        STRIKE,
    );
    let label_color = motion::mix(t.text, t.text_faint, strike);

    let check = {
        let state = state.clone();
        let outline = outline.clone();
        let id = task.id.clone();
        let hover = format!("{tkey}-box");
        let border = motion::hover_blend(&hover, t.border_strong, t.text_faint);
        let mark = svg().path(ICON_CHECK).size(px(12.)).text_color(t.on_accent);
        div()
            .id(SharedString::from(hover.clone()))
            .mt(px(2.))
            .size(px(16.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(5.))
            .border_1()
            .cursor_pointer()
            .on_hover(motion::hover_listener(hover))
            .map(|d| {
                if done {
                    d.bg(t.accent_strong).border_color(t.accent_strong)
                } else {
                    d.border_color(border).bg(t.input_bg)
                }
            })
            .when(done, |d| {
                d.child(if checks == 0 {
                    mark.into_any_element()
                } else {
                    mark.with_animation(
                        SharedString::from(format!("{tkey}-check-{checks}")),
                        MotionSpec::new(260, motion::EASE_OUT_QUINT).animation(),
                        |el, p| el.size(px(5. + 7. * p)).opacity(p),
                    )
                    .into_any_element()
                })
            })
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                let now_done = state.update(cx, |s, cx| {
                    s.toggle(outline.clone(), &id, cx);
                    s.done.contains(&id)
                });
                if let Some(f) = &on_toggle {
                    f(&id, now_done, window, cx);
                }
            })
    };

    let label = div()
        .relative()
        .flex_none()
        .max_w_full()
        .text_size(px(text::BODY))
        .line_height(px(20.))
        .text_color(label_color)
        .child(task.title.clone())
        .child(
            div()
                .absolute()
                .left_0()
                .top(px(10.))
                .h(px(1.))
                .w(relative(strike))
                .bg(t.text_faint),
        );

    let details = (active && (task.hint.is_some() || task.action.is_some())).then(|| {
        let id = task.id.clone();
        motion::rise_in(
            SharedString::from(format!("{tkey}-details")),
            motion::ROW_IN,
            4.,
            div()
                .flex()
                .flex_col()
                .gap(px(space::S8))
                .pt(px(space::S2))
                .when_some(task.hint.clone(), |d, hint| {
                    d.child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(17.))
                            .text_color(t.text_muted)
                            .child(hint),
                    )
                })
                .when_some(task.action.clone(), |d, label| {
                    let action_key = format!("{tkey}-action");
                    let shift = motion::hover_t(&format!("luna-btn-{action_key}")) * 2.;
                    d.child(
                        div().flex().child(
                            luna::button(action_key, ButtonKind::Secondary, ControlSize::Sm, t)
                                .child(label)
                                .child(
                                    div().w(px(14.)).pl(px(shift)).child(
                                        luna::glyph(ICON_ARROW, ControlSize::Sm)
                                            .text_color(t.text_muted),
                                    ),
                                )
                                .on_click(move |_, window, cx| {
                                    cx.stop_propagation();
                                    if let Some(f) = &on_action {
                                        f(&id, window, cx);
                                    }
                                }),
                        ),
                    )
                }),
        )
    });

    let row_key = format!("{tkey}-row");
    let state = state.clone();
    let id = task.id.clone();
    div()
        .id(SharedString::from(format!("{row_key}-{group_ix}")))
        .flex()
        .items_start()
        .gap(px(space::S10))
        .ml(px(space::S6))
        .px(px(space::S10))
        .py(px(space::S6))
        .rounded(px(radius::MD))
        .when(!done, |d| d.cursor_pointer())
        .map(|d| {
            if active {
                d.bg(t.fill(FillTier::Quaternary))
            } else {
                d.hover_fill(
                    row_key.clone(),
                    t.fill(FillTier::Quaternary).opacity(0.),
                    t.fill(FillTier::Quaternary),
                )
            }
        })
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                if !s.done.contains(&id) {
                    s.active = Some(id.clone());
                    cx.notify();
                }
            })
        })
        .child(check)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(div().flex().child(label))
                .children(details),
        )
}

/// The pill a dismissed guide leaves behind.
fn resume_pill(
    key: &str,
    state: &gpui::Entity<GuideState>,
    done: usize,
    total: usize,
    t: &Luna,
) -> impl IntoElement {
    let state = state.clone();
    let ratio = if total == 0 {
        0.
    } else {
        done as f32 / total as f32
    };
    motion::pop_in(
        SharedString::from(format!("{key}-resume")),
        luna::button(
            format!("{key}-resume-btn"),
            ButtonKind::Secondary,
            ControlSize::Lg,
            t,
        )
        .font_family(luna::SANS)
        .gap(px(space::S8))
        .child(progress_ring(
            ratio,
            16.,
            2.5,
            t.fill(FillTier::Primary),
            t.accent,
        ))
        .child("Resume launch checklist")
        .child(
            div()
                .text_color(t.text_faint)
                .font_family(luna::MONO)
                .child(format!("{done}/{total}")),
        )
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                s.dismissed = false;
                cx.notify();
            })
        }),
    )
}

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------

/// A content block whose height glides between its natural height and 0.
/// The height is measured after layout, so the first open frame is natural.
fn collapse(
    key: &str,
    open: bool,
    measured: Rc<Cell<f32>>,
    content: impl IntoElement,
) -> impl IntoElement {
    let natural = measured.get();
    let height =
        (natural > 0.).then(|| motion::follow(key, if open { natural } else { 0. }, GLIDE));
    let reveal = match height {
        Some(h) if natural > 0. => (h / natural).clamp(0., 1.),
        _ => {
            if open {
                1.
            } else {
                0.
            }
        }
    };
    div()
        .overflow_hidden()
        .map(|d| match height {
            Some(h) => d.h(px(h)),
            None if open => d,
            None => d.h(px(0.)),
        })
        .child(
            div()
                .relative()
                .flex_none()
                .opacity(reveal)
                .child(content)
                .child(measure(measured)),
        )
}

/// Records its parent's height in `cell` (and asks for a frame when it changes).
fn measure(cell: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            if (cell.get() - h).abs() > 0.5 {
                cell.set(h);
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A track ring with an arc filled to `ratio`, starting at 12 o'clock.
fn progress_ring(ratio: f32, size: f32, width: f32, track: Hsla, fill: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let center = bounds.center();
            let r = size / 2. - width / 2.;
            if let Some(path) = annulus(center, r, width) {
                window.paint_path(path, track);
            }
            let ratio = ratio.clamp(0., 1.);
            if ratio >= 0.999 {
                if let Some(path) = annulus(center, r, width) {
                    window.paint_path(path, fill);
                }
            } else if ratio > 0.001
                && let Some(path) = arc(center, r, width, -PI / 2., ratio * 2. * PI)
            {
                window.paint_path(path, fill);
            }
        },
    )
    .size(px(size))
    .flex_none()
}

fn fill_builder() -> PathBuilder {
    PathBuilder::fill().with_style(PathStyle::Fill(
        FillOptions::default().with_fill_rule(FillRule::NonZero),
    ))
}

fn polar(c: Point<Pixels>, r: f32, a: f32) -> Point<Pixels> {
    point(c.x + px(r * a.cos()), c.y + px(r * a.sin()))
}

/// A full ring of stroke `w` around radius `r`.
fn annulus(c: Point<Pixels>, r: f32, w: f32) -> Option<gpui::Path<Pixels>> {
    let steps = 96;
    let mut b = fill_builder();
    b.move_to(polar(c, r + w / 2., 0.));
    for i in 1..steps {
        b.line_to(polar(c, r + w / 2., 2. * PI * i as f32 / steps as f32));
    }
    b.close();
    b.move_to(polar(c, r - w / 2., 0.));
    for i in (1..steps).rev() {
        b.line_to(polar(c, r - w / 2., 2. * PI * i as f32 / steps as f32));
    }
    b.close();
    b.build().ok()
}

/// An arc band with round caps from `start` sweeping `sweep` radians.
fn arc(c: Point<Pixels>, r: f32, w: f32, start: f32, sweep: f32) -> Option<gpui::Path<Pixels>> {
    let steps = ((sweep.abs() / (2. * PI)) * 96.).ceil().max(2.) as usize;
    let (outer, inner, half) = (r + w / 2., r - w / 2., w / 2.);
    let end = start + sweep;
    let mut b = fill_builder();
    b.move_to(polar(c, outer, start));
    for i in 1..=steps {
        b.line_to(polar(c, outer, start + sweep * i as f32 / steps as f32));
    }
    let tip = polar(c, r, end);
    for i in 1..8 {
        b.line_to(polar(tip, half, end + PI * i as f32 / 8.));
    }
    for i in (0..=steps).rev() {
        b.line_to(polar(c, inner, start + sweep * i as f32 / steps as f32));
    }
    let tail = polar(c, r, start);
    for i in 1..8 {
        b.line_to(polar(tail, half, start + PI + PI * i as f32 / 8.));
    }
    b.close();
    b.build().ok()
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        SetupGuide::new("setup-guide", "Launch checklist").groups([
            SetupGroup::new("brand", "Make it yours", "icons/setup-guide/palette.svg")
                .task(SetupTask::new("logo", "Upload a logo").done())
                .task(SetupTask::new("accent", "Pick an accent color").done())
                .task(
                    SetupTask::new("url", "Choose a workspace address")
                        .minutes(2)
                        .hint("Short and lowercase works best – teammates type it every day.")
                        .action("Choose address"),
                ),
            SetupGroup::new("team", "Bring your team", "icons/setup-guide/user-plus.svg")
                .task(
                    SetupTask::new("invite", "Invite two teammates")
                        .minutes(3)
                        .hint("Invites stay open for 7 days.")
                        .action("Send invites"),
                )
                .task(
                    SetupTask::new("roles", "Set default roles")
                        .minutes(2)
                        .hint("New members join as editors unless you change it.")
                        .action("Review roles"),
                )
                .task(
                    SetupTask::new("project", "Create a first project")
                        .minutes(3)
                        .action("New project"),
                ),
            SetupGroup::new("stack", "Connect your stack", "icons/setup-guide/plug.svg")
                .task(
                    SetupTask::new("calendar", "Sync a calendar")
                        .minutes(2)
                        .action("Connect calendar"),
                )
                .task(
                    SetupTask::new("repo", "Link a code host")
                        .minutes(3)
                        .action("Link repository"),
                ),
            SetupGroup::new("secure", "Lock it down", "icons/setup-guide/shield.svg")
                .task(
                    SetupTask::new("2fa", "Require two-step sign-in")
                        .minutes(1)
                        .action("Turn on"),
                )
                .task(SetupTask::new("sessions", "Review session length").minutes(1)),
        ])
    })
}
