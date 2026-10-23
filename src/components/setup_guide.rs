//! Setup Guide – an onboarding checklist on a pink card: a summary row with a
//! progress ring that folds the guide away, task groups that open one at a
//! time, and tasks that strike through as they're checked off.
//!
//! The summary row is pink's composer checklist row (count and next step,
//! a chevron, a dismiss cross), grown to carry the guide's title and pink's
//! upload ring. Only one group is open at a time. The selected task in the
//! open group shows its hint and call to action; finishing the last task of
//! a group folds it away and opens the next unfinished one. Dismissing the
//! guide leaves a small button that brings it back.
//!
//! ```ignore
//! SetupGuide::new("launch", "Launch checklist")
//!     .group(
//!         SetupGroup::new("project", "Start a project", ICON)
//!             .task(SetupTask::new("folder", "Add a project folder").done())
//!             .task(SetupTask::new("invite", "Invite two teammates").minutes(3)),
//!     )
//!     .on_toggle(|task, done, _, _| println!("{task}: {done}"))
//! ```

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
};

use gpui::{
    AnyElement, AnyView, App, Div, ElementId, Entity, FontWeight, Hsla, IntoElement, PathBuilder,
    Pixels, Role, SharedString, Task, Toggled, Transformation, Window, canvas, div, point,
    prelude::*, px, radians, relative, svg,
};
use web_time::{Duration, Instant};

use crate::{
    HoverFill,
    motion::{self, EMPHASIZED, TOOL_CONNECTOR_REVEAL},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, Tone, layout, radius},
    story,
};

pub const NAME: &str = "Setup Guide";
pub const DESCRIPTION: &str = "An onboarding checklist on a quiet card – a progress ring and a 4/10 · Next line that fold the guide away, task groups that open one at a time, the selected task's hint and call to action, tasks that strike through as they're checked off, and a small button that brings a dismissed guide back";

const ICON_ARROW_DOWN: &str = "icons/setup-guide/alt-arrow-down.svg";
const ICON_ARROW_RIGHT: &str = "icons/setup-guide/alt-arrow-right.svg";
const ICON_CLOSE: &str = "icons/setup-guide/close.svg";
const ICON_CHECK: &str = "icons/setup-guide/check.svg";
const ICON_CHECKLIST: &str = "icons/setup-guide/checklist.svg";

/// pink: composer/checklist.rs – the summary row's type size.
const SUMMARY_TEXT: f32 = 11.5;
/// pink: composer/checklist.rs – the summary's chevron and cross.
const SUMMARY_GLYPH: f32 = 12.0;
/// pink: composer/checklist.rs – the dismiss button.
const SUMMARY_DISMISS: f32 = 24.0;
/// pink: transcript.rs `DISCLOSURE_CHEVRON_SIZE`.
const DISCLOSURE_CHEVRON: f32 = 12.0;
/// pink: loaders.rs – the upload ring's diameter in the transcript.
const RING: f32 = 34.0;
/// pink: loaders.rs `RING_STROKE`.
const RING_STROKE: f32 = 2.5;
/// pink: loaders.rs `RING_SEGMENTS`.
const RING_SEGMENTS: f32 = 64.0;
/// pink: loaders.rs `RING_LABEL_RATIO` (9 pt in the 34 pt ring).
const RING_LABEL_RATIO: f32 = 9.0 / 34.0;
/// Task rows start under their group's title: the 28 px identity tile plus
/// the head's 10 px gap.
const TASK_INDENT: f32 = 28.0 + 10.0;
/// The strike sits on the x-height of a 13/18 label.
const STRIKE_Y: f32 = 9.5;

/// Delay before a finished group folds away and the next one opens: the
/// strike draws over `TOOL_CONNECTOR_REVEAL` (480 ms), then a beat.
const ADVANCE_AFTER: Duration = Duration::from_millis(700);

/// The fold key of the guide's body (minimize); groups fold under their id.
const BODY: &str = "\u{0}body";

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
    open: Option<SharedString>,
    active: Option<SharedString>,
    minimized: bool,
    dismissed: bool,
    /// Bumped on every resume, so the card fades in again.
    shown: usize,
    /// When each fold (the body, a group) last changed direction.
    folds: HashMap<SharedString, Instant>,
    heights: HashMap<SharedString, Rc<Cell<f32>>>,
    advance: Option<Task<()>>,
}

impl GuideState {
    fn new() -> Self {
        Self {
            seeded: false,
            done: HashSet::new(),
            open: None,
            active: None,
            minimized: false,
            dismissed: false,
            shown: 0,
            folds: HashMap::new(),
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

    /// Opens `group` (or nothing), folding the previous one away.
    fn set_open(&mut self, group: Option<SharedString>) {
        if self.open == group {
            return;
        }
        let now = Instant::now();
        for key in self.open.iter().chain(group.iter()) {
            self.folds.insert(key.clone(), now);
        }
        self.open = group;
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
                self.set_open(Some(id.clone()));
                self.active = self.first_open(tasks);
            }
            None => {
                self.set_open(None);
                self.active = None;
            }
        }
    }

    fn set_minimized(&mut self, minimized: bool) {
        if self.minimized != minimized {
            self.minimized = minimized;
            self.folds.insert(BODY.into(), Instant::now());
        }
    }

    fn toggle(&mut self, outline: Rc<Outline>, task: &SharedString, cx: &mut gpui::Context<Self>) {
        let done = !self.done.contains(task);
        if done {
            self.done.insert(task.clone());
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
            // Let the strike draw, then fold the group and open the next one.
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
            width: px(layout::DIALOG_WIDTH),
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
    fn build(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let t = Pink::get(cx);
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
                // The first group is open from the start, not folding open.
                s.folds.clear();
            });
        }

        let total: usize = self.groups.iter().map(|g| g.tasks.len()).sum();
        let (done_count, minutes_left, next) = {
            let s = state.read(cx);
            let all = || self.groups.iter().flat_map(|g| g.tasks.iter());
            let done = all().filter(|t| s.done.contains(&t.id)).count();
            let left: u32 = all()
                .filter(|t| !s.done.contains(&t.id))
                .map(|t| t.minutes)
                .sum();
            // pink's checklist names the step in hand, else the next pending one.
            let next = s
                .active
                .as_ref()
                .and_then(|id| all().find(|t| &t.id == id && !s.done.contains(&t.id)))
                .or_else(|| all().find(|t| !s.done.contains(&t.id)))
                .map(|t| t.title.clone());
            (done, left, next)
        };
        let complete = total > 0 && done_count == total;

        if state.read(cx).dismissed {
            return resume_button(&key, &self.title, &state, done_count, total, &t)
                .into_any_element();
        }

        // pink: composer/checklist.rs – "{done}/{total} · {detail}".
        let detail: SharedString = match next {
            Some(title) => format!("Next: {title}").into(),
            None if complete => "Checklist complete".into(),
            None => "No active step".into(),
        };
        let summary: SharedString = format!("{done_count}/{total} · {detail}").into();
        let minimized = state.read(cx).minimized;

        let header = summary_row(
            &key,
            &self.title,
            &summary,
            (done_count, total),
            (!complete && total > 0).then_some(minutes_left),
            !minimized,
            &state,
            self.on_dismiss.clone(),
            &t,
        );

        let groups: Vec<_> = self
            .groups
            .iter()
            .map(|group| {
                group_view(
                    &key,
                    group,
                    &state,
                    &outline,
                    self.on_toggle.clone(),
                    self.on_action.clone(),
                    &t,
                    window,
                    cx,
                )
            })
            .collect();

        let body = div()
            .border_t_1()
            .border_color(t.border)
            .p(px(layout::CARD_INSET))
            .flex()
            .flex_col()
            .gap(px(layout::MENU_GAP))
            .children(groups);
        let body = (!self.groups.is_empty())
            .then(|| fold(&state, BODY.into(), !minimized, body, window, cx).0);

        let shown = state.read(cx).shown;
        motion::fade_quick(
            SharedString::from(format!("{key}-shown-{shown}")),
            pink::card(&t)
                .id(self.id.clone())
                .w(self.width)
                .font_family(pink::SANS)
                .text_color(t.text)
                .overflow_hidden()
                .flex()
                .flex_col()
                .child(header)
                .children(body),
        )
        .into_any_element()
    }
}

/// The guide's header (pink: composer/checklist.rs `render_checklist`'s
/// summary row): one button that folds the guide – here holding pink's
/// upload ring, the title and the checklist's "done/total · Next: …" line,
/// with the chevron – and the 24 px dismiss cross, gap 4. Both wash in
/// `element_hover` on hover, as pink's do.
#[allow(clippy::too_many_arguments)]
fn summary_row(
    key: &str,
    title: &SharedString,
    summary: &SharedString,
    (done, total): (usize, usize),
    minutes_left: Option<u32>,
    expanded: bool,
    state: &Entity<GuideState>,
    on_dismiss: Option<Handler>,
    t: &Pink,
) -> Div {
    let ring = t.focus_ring();
    let hover = t.element_hover;
    let complete = total > 0 && done == total;
    let ratio = if total == 0 {
        0.
    } else {
        done as f32 / total as f32
    };
    let percent = (ratio * 100.).round() as u8;

    let progress = progress_ring(
        motion::follow(&format!("{key}-ratio"), ratio, EMPHASIZED),
        if complete {
            t.success
        } else {
            t.text.opacity(0.95)
        },
        t.text.opacity(0.22),
        if complete {
            svg()
                .path(ICON_CHECK)
                .size(px(pink::icon_size::SM))
                .flex_none()
                .text_color(t.readable_icon(t.success))
                .into_any_element()
        } else {
            div()
                .text_size(px(RING * RING_LABEL_RATIO))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text.opacity(0.95))
                .child(SharedString::from(format!("{percent}%")))
                .into_any_element()
        },
    );

    let text = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(pink::space::S8))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(pink::text::BODY))
                        .line_height(px(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(title.clone()),
                )
                .when_some(minutes_left, |row, minutes| {
                    // pink: transcript.rs working timer – 11 px tabular, faint.
                    row.child(
                        div()
                            .flex_none()
                            .text_size(px(pink::text::CAPTION))
                            .line_height(px(18.))
                            .font_features(pink::tabular_figures())
                            .text_color(t.content_faint)
                            .child(format!("{minutes} min left")),
                    )
                }),
        )
        .child(
            div().flex().child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(SUMMARY_TEXT))
                    .line_height(px(16.))
                    .text_color(t.text_muted)
                    .child(summary.clone()),
            ),
        );

    let toggle = {
        let state = state.clone();
        div()
            .id(SharedString::from(format!("{key}-expand")))
            .role(Role::Button)
            .aria_label(SharedString::from(format!(
                "{} {title}: {summary}",
                if expanded { "Collapse" } else { "Expand" }
            )))
            .aria_expanded(expanded)
            .tab_index(0)
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(pink::space::S12))
            .px(px(pink::space::S8))
            .py(px(pink::space::S8))
            .rounded(px(radius::inner(radius::SECTION_CARD, layout::CARD_INSET)))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .on_key_down(pink::swallow_activation_key)
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.set_minimized(!s.minimized);
                    cx.notify();
                })
            })
            .child(progress)
            .child(text)
            .child(
                svg()
                    .path(if expanded {
                        ICON_ARROW_DOWN
                    } else {
                        ICON_ARROW_RIGHT
                    })
                    .size(px(SUMMARY_GLYPH))
                    .flex_none()
                    .text_color(t.text_muted),
            )
    };

    let dismiss = {
        let state = state.clone();
        div()
            .id(SharedString::from(format!("{key}-dismiss")))
            .role(Role::Button)
            .aria_label(SharedString::from(format!("Dismiss {title}")))
            .tab_index(0)
            .flex_none()
            .size(px(SUMMARY_DISMISS))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::SM))
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .on_key_down(pink::swallow_activation_key)
            .child(
                svg()
                    .path(ICON_CLOSE)
                    .size(px(SUMMARY_GLYPH))
                    .flex_none()
                    .text_color(t.text_muted),
            )
            .on_click(move |_, window, cx| {
                state.update(cx, |s, cx| {
                    s.dismissed = true;
                    s.advance = None;
                    cx.notify();
                });
                if let Some(f) = &on_dismiss {
                    f(window, cx);
                }
            })
    };

    div()
        .flex()
        .items_center()
        .gap(px(pink::space::S4))
        .min_w_0()
        .p(px(layout::CARD_INSET))
        .child(toggle)
        .child(dismiss)
}

/// A task group: a disclosure head and its tasks, which fold open and shut
/// like pink's tool groups.
#[allow(clippy::too_many_arguments)]
fn group_view(
    key: &str,
    group: &SetupGroup,
    state: &Entity<GuideState>,
    outline: &Rc<Outline>,
    on_toggle: Option<ToggleHandler>,
    on_action: Option<TaskHandler>,
    t: &Pink,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let gkey = format!("{key}-group-{}", group.id);
    let (open, done, active) = {
        let s = state.read(cx);
        let done: Vec<bool> = group.tasks.iter().map(|t| s.done.contains(&t.id)).collect();
        (s.open.as_ref() == Some(&group.id), done, s.active.clone())
    };
    let done_count = done.iter().filter(|d| **d).count();
    let complete = !group.tasks.is_empty() && done_count == group.tasks.len();

    let tasks: Vec<_> = group
        .tasks
        .iter()
        .zip(&done)
        .map(|(task, &is_done)| {
            let is_active = active.as_ref() == Some(&task.id) && !is_done;
            task_row(
                key,
                task,
                is_done,
                is_active,
                state,
                outline,
                on_toggle.clone(),
                on_action.clone(),
                t,
            )
        })
        .collect();
    let (tasks, progress) = fold(
        state,
        group.id.clone(),
        open,
        div()
            .flex()
            .flex_col()
            .gap(px(layout::MENU_GAP))
            .pt(px(layout::MENU_GAP))
            .pb(px(pink::space::S4))
            .children(tasks),
        window,
        cx,
    );

    // pink: popover.rs `menu_row` anatomy (gap 10, px 8 / py 6, r8 inside the
    // card's 4 px inset) with a quiet `hover_wash`, a settings `row_tile`, and
    // transcript.rs's tool-group chevron turning 0 → 90° with the fold.
    let ring = t.focus_ring();
    let hover_key = format!("{gkey}-head");
    let head = {
        let state = state.clone();
        let outline = outline.clone();
        let gid = group.id.clone();
        div()
            .id(SharedString::from(hover_key.clone()))
            .role(Role::Button)
            .aria_label(group.title.clone())
            .aria_expanded(open)
            .tab_index(0)
            .flex()
            .items_center()
            .gap(px(pink::space::S10))
            .px(px(pink::space::S8))
            .py(px(pink::space::S6))
            .rounded(px(radius::MD))
            .cursor_pointer()
            .hover_wash(hover_key, t.fill(FillTier::Tertiary))
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .on_key_down(pink::swallow_activation_key)
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if s.open.as_ref() == Some(&gid) {
                        s.set_open(None);
                        s.active = None;
                    } else {
                        s.focus_group(&outline, Some(&gid));
                    }
                    s.advance = None;
                    cx.notify();
                })
            })
            .child(pink::row_tile(group.icon.clone(), t))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(pink::text::BODY))
                    .line_height(px(18.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(group.title.clone()),
            )
            .child(if complete {
                // pink: settings `badge_active`, as the permission rows' "Ready".
                pink::badge("Done", Tone::Success, t)
                    .line_height(px(14.))
                    .into_any_element()
            } else {
                // pink: activity_center.rs – a section's trailing count.
                div()
                    .flex_none()
                    .text_size(px(pink::text::CAPTION))
                    .font_features(pink::tabular_figures())
                    .text_color(t.readable_faint(0.7))
                    .child(format!("{done_count}/{}", group.tasks.len()))
                    .into_any_element()
            })
            .child(
                svg()
                    .path(ICON_ARROW_RIGHT)
                    .size(px(DISCLOSURE_CHEVRON))
                    .flex_none()
                    .text_color(t.content_faint)
                    .with_transformation(Transformation::rotate(radians(FRAC_PI_2 * progress))),
            )
    };

    div()
        .flex()
        .flex_col()
        .child(head)
        .child(tasks)
        .into_any_element()
}

/// One task: pink's markdown task-list item (a 16 px checkbox centered on the
/// first line, gap 8) in a row with pink's quiet `fill(Tertiary)` hover. The
/// selected task opens into an inset cell (pink: shell/usage.rs metric cell,
/// `fill(Quaternary)` inside a `separator(false)` edge, r8) holding its hint
/// and call to action; a checked one strikes through in `content_muted`,
/// pink's markdown strikethrough, drawn like a rail.
#[allow(clippy::too_many_arguments)]
fn task_row(
    key: &str,
    task: &SetupTask,
    done: bool,
    active: bool,
    state: &Entity<GuideState>,
    outline: &Rc<Outline>,
    on_toggle: Option<ToggleHandler>,
    on_action: Option<TaskHandler>,
    t: &Pink,
) -> impl IntoElement {
    let tkey = format!("{key}-task-{}", task.id);
    let strike = motion::follow(
        &format!("{tkey}-strike"),
        if done { 1. } else { 0. },
        TOOL_CONNECTOR_REVEAL,
    );

    let check = {
        let state = state.clone();
        let outline = outline.clone();
        let id = task.id.clone();
        let focus = t.text;
        // pink: markdown/render.rs task marker – the box in an 18 px wide slot
        // one line tall, its edge turning `text` on keyboard focus.
        div()
            .flex_none()
            .min_w(px(18.))
            .h(px(18.))
            .flex()
            .items_center()
            .child(
                pink::checkbox_box(done, t)
                    .id(SharedString::from(format!("{tkey}-box")))
                    .role(Role::CheckBox)
                    .aria_label(task.title.clone())
                    .aria_toggled(if done { Toggled::True } else { Toggled::False })
                    .tab_index(0)
                    .cursor_pointer()
                    .focus_visible(move |s| s.border_color(focus))
                    .on_key_down(pink::swallow_activation_key)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        let now_done = state.update(cx, |s, cx| {
                            s.toggle(outline.clone(), &id, cx);
                            s.done.contains(&id)
                        });
                        if let Some(f) = &on_toggle {
                            f(&id, now_done, window, cx);
                        }
                    }),
            )
    };

    let label = div()
        .relative()
        .flex_none()
        .max_w_full()
        .text_size(px(pink::text::BODY))
        .line_height(px(18.))
        .text_color(motion::mix(t.text, t.content_muted, strike))
        .child(task.title.clone())
        .when(strike > 0., |label| {
            label.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(STRIKE_Y))
                    .h(px(1.))
                    .w(relative(strike))
                    .bg(t.content_muted),
            )
        });

    let details = (active && (task.hint.is_some() || task.action.is_some())).then(|| {
        let id = task.id.clone();
        motion::rise_in(
            SharedString::from(format!("{tkey}-details")),
            motion::TRANSCRIPT_IN,
            motion::TRANSCRIPT_IN_RISE,
            div()
                .flex()
                .flex_col()
                .gap(px(pink::space::S8))
                .pt(px(pink::space::S2))
                .pb(px(pink::space::S2))
                .when_some(task.hint.clone(), |d, hint| {
                    d.child(pink::row_description(hint, t))
                })
                .when_some(task.action.clone(), |d, label| {
                    d.child(
                        div().flex().child(
                            pink::button(
                                format!("{tkey}-action"),
                                ButtonKind::Secondary,
                                ControlSize::Sm,
                                t,
                            )
                            .child(label)
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
        .id(SharedString::from(row_key.clone()))
        .flex()
        .items_start()
        .gap(px(pink::space::S8))
        .ml(px(TASK_INDENT))
        // The 8 / 6 inset less the edge every row carries (transparent at
        // rest, so selecting a row never moves it).
        .px(px(pink::space::S8 - 1.))
        .py(px(pink::space::S6 - 1.))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(gpui::transparent_black())
        .map(|row| {
            if active {
                row.bg(t.fill(FillTier::Quaternary))
                    .border_color(t.separator(false))
            } else if done {
                row
            } else {
                row.cursor_pointer()
                    .hover_wash(row_key, t.fill(FillTier::Tertiary))
            }
        })
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                if !s.done.contains(&id) && s.active.as_ref() != Some(&id) {
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

/// What a dismissed guide leaves behind: a Secondary button that brings it
/// back, with pink's checklist glyph and the count.
fn resume_button(
    key: &str,
    title: &SharedString,
    state: &Entity<GuideState>,
    done: usize,
    total: usize,
    t: &Pink,
) -> impl IntoElement {
    let state = state.clone();
    motion::fade_quick(
        SharedString::from(format!("{key}-resume")),
        pink::button(
            format!("{key}-resume-btn"),
            ButtonKind::Secondary,
            ControlSize::Md,
            t,
        )
        .font_family(pink::SANS)
        .aria_label(SharedString::from(format!("Show {title}")))
        .child(pink::glyph(ICON_CHECKLIST, ControlSize::Md))
        .child(title.clone())
        .child(
            div()
                .font_features(pink::tabular_figures())
                .text_color(t.readable_faint(0.8))
                .child(format!("{done}/{total}")),
        )
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                s.dismissed = false;
                s.shown += 1;
                cx.notify();
            })
        }),
    )
}

// ---------------------------------------------------------------------------
// Drawing helpers
// ---------------------------------------------------------------------------

/// Folds `content` open or shut on pink's tool-group fold
/// (`motion::fold_progress`: the height on `DISCLOSURE`, the body fading in
/// over `TOOL_BODY_FADE` with a 6 px settle, clear by 60% of a closing
/// fold). The natural height is measured after layout; a settled, measured
/// fold that is shut stops rendering its content. Returns the fold and its
/// progress (0 shut, 1 open) for the chevron.
fn fold(
    state: &Entity<GuideState>,
    key: SharedString,
    open: bool,
    content: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> (Div, f32) {
    let (since, measured) = state.update(cx, |s, _| (s.folds.get(&key).copied(), s.height(&key)));
    let elapsed = since.map(|since| since.elapsed()).filter(|elapsed| {
        !motion::reduced_motion(cx)
            && *elapsed < motion::FOLD_TWEEN_WINDOW.mul_f32(motion::speed_scale())
    });
    let (progress, alpha) = motion::fold_progress(open, elapsed);
    if elapsed.is_some() {
        window.request_animation_frame();
    }
    let natural = measured.get();
    let settled_shut = !open && elapsed.is_none() && natural > 0.;
    let frame = div().overflow_hidden().map(|d| match elapsed {
        Some(_) if natural > 0. => d.h(px(natural * progress)),
        _ if open => d,
        _ => d.h(px(0.)),
    });
    if settled_shut {
        return (frame, progress);
    }
    (
        frame.child(
            div()
                .relative()
                .flex_none()
                .top(px(motion::TOOL_BODY_RISE * (1. - alpha)))
                .opacity(alpha)
                .child(content)
                .child(measure(measured)),
        ),
        progress,
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

/// Pink's upload progress ring (pink: loaders.rs `upload_progress_ring`) at
/// its 34 px size: a full track and an arc growing clockwise from 12
/// o'clock, both 2.5 px polylines of 64 segments per turn, around a centered
/// label. Pink paints it white-on-wash over a thumbnail; on a card the same
/// alphas sit on the text tone (track 22%, arc and label 95%).
fn progress_ring(ratio: f32, arc: Hsla, track: Hsla, center: AnyElement) -> Div {
    let ratio = ratio.clamp(0., 1.);
    let ring = canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let c = bounds.center();
            let r = f32::from(bounds.size.width.min(bounds.size.height)) / 2. - RING_STROKE;
            let mut paint_arc = |sweep: f32, color: Hsla| {
                if sweep <= 0.001 {
                    return;
                }
                let steps = ((RING_SEGMENTS * sweep).ceil() as usize).max(2);
                let at = |i: usize| {
                    let theta =
                        -FRAC_PI_2 + std::f32::consts::TAU * sweep * (i as f32 / steps as f32);
                    point(c.x + px(r * theta.cos()), c.y + px(r * theta.sin()))
                };
                let mut builder = PathBuilder::stroke(px(RING_STROKE));
                builder.move_to(at(0));
                for i in 1..=steps {
                    builder.line_to(at(i));
                }
                if let Ok(path) = builder.build() {
                    window.paint_path(path, color);
                }
            };
            paint_arc(1., track);
            paint_arc(ratio, arc);
        },
    )
    .absolute()
    .inset_0();
    div()
        .relative()
        .flex_none()
        .size(px(RING))
        .flex()
        .items_center()
        .justify_center()
        .child(ring)
        .child(center)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        let launch = SetupGuide::new("setup-guide", "Launch checklist").groups([
            SetupGroup::new("brand", "Make it yours", "icons/setup-guide/palette.svg")
                .task(SetupTask::new("logo", "Upload a logo").done())
                .task(SetupTask::new("accent", "Pick an accent color").done())
                .task(SetupTask::new("url", "Choose a workspace address").done()),
            SetupGroup::new(
                "project",
                "Start a project",
                "icons/setup-guide/folder-with-files.svg",
            )
            .task(SetupTask::new("folder", "Add a project folder").done())
            .task(
                SetupTask::new("roles", "Set default roles")
                    .minutes(2)
                    .hint("New members join as editors until you change it.")
                    .action("Review roles"),
            )
            .task(
                SetupTask::new("invite", "Invite two teammates")
                    .minutes(3)
                    .hint("They see the project's chats and files once they join.")
                    .action("Send invites"),
            ),
            SetupGroup::new(
                "stack",
                "Connect your stack",
                "icons/setup-guide/webhook.svg",
            )
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
            SetupGroup::new(
                "secure",
                "Lock it down",
                "icons/setup-guide/shield-check.svg",
            )
            .task(
                SetupTask::new("2fa", "Require two-step sign-in")
                    .minutes(1)
                    .hint("Everyone signs in again with a code the next time.")
                    .action("Turn on"),
            )
            .task(SetupTask::new("sessions", "Review session length").minutes(1)),
        ]);

        // A finished guide: the ring turns to a success check and every group
        // rests folded with its Done badge.
        let finished = SetupGuide::new("setup-guide-done", "Workspace basics").groups([
            SetupGroup::new("profile", "Make it yours", "icons/setup-guide/palette.svg")
                .task(SetupTask::new("avatar", "Add a profile photo").done())
                .task(SetupTask::new("theme", "Choose light or dark").done()),
            SetupGroup::new(
                "access",
                "Lock it down",
                "icons/setup-guide/shield-check.svg",
            )
            .task(SetupTask::new("passkey", "Add a passkey").done())
            .task(SetupTask::new("recovery", "Save recovery codes").done()),
        ]);

        div()
            .flex()
            .flex_wrap()
            .items_start()
            .justify_center()
            .gap(px(pink::space::S24))
            .child(launch)
            .child(finished)
    })
}
