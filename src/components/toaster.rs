//! Toaster – calm notifications you can raise from anywhere.
//!
//! A [`Toasts`] entity is the queue: `push` a [`Toast`], `replace` it later,
//! or hand `promise` a future and the toast morphs from a spinner into its
//! result. [`Toaster`] renders the queue as a stack: the newest toast in
//! front, older ones tucked behind it. Hovering the stack spreads it into a
//! list and pauses every timer; dragging a toast sideways flicks it away.
//!
//! ```ignore
//! let toasts = cx.new(|_| Toasts::new());
//! toasts.update(cx, |t, cx| {
//!     t.push(Toast::success("Link copied").description("Anyone with the link can view"), cx);
//! });
//! div().child(Toaster::new("toasts", toasts.clone()))
//! ```

use std::{cell::Cell, collections::HashMap, f32::consts::PI, future::Future, rc::Rc};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnyView, App, Context, DispatchPhase, ElementId, Entity, FontWeight, Hsla,
    IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent, Pixels, SharedString, Task,
    Transformation, Window, canvas, div, prelude::*, px, radians, relative, svg,
};

use crate::{
    motion::{self, AnimationExt, GLIDE, MotionSpec},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, radius, space, text},
    story,
};

pub const NAME: &str = "Toaster";
pub const DESCRIPTION: &str = "A stack of quiet notifications that spreads open on hover, pauses its timers, morphs promise toasts into their result and flicks away with a drag";

const ICON_SUCCESS: &str = "icons/toaster/check-circle.svg";
const ICON_ERROR: &str = "icons/toaster/alert.svg";
const ICON_INFO: &str = "icons/toaster/info.svg";
const ICON_SPINNER: &str = "icons/toaster/spinner.svg";
const ICON_CLOSE: &str = "icons/toaster/x.svg";

/// How long a toast takes to fade out once dismissed.
const LEAVE: Duration = Duration::from_millis(320);
/// Vertical sliver of each tucked toast that shows above the front one.
const PEEK: f32 = 10.;
/// Horizontal inset per step back in the stack (stands in for a scale).
const STEP_INSET: f32 = 12.;
const GAP: f32 = 8.;
/// Distance a drag must cover before the toast is flicked away.
const SWIPE_DISMISS: f32 = 90.;
/// Toast height used until the real one is measured.
const FALLBACK_HEIGHT: f32 = 64.;

pub type ToastId = u64;

type ActionHandler = Rc<dyn Fn(ToastId, &mut Window, &mut App)>;

/// What a toast is about; picks its glyph and tint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Neutral,
    Success,
    Info,
    Warning,
    Error,
    /// A spinner; never times out until replaced.
    Loading,
}

#[derive(Clone)]
pub struct ToastAction {
    pub label: SharedString,
    handler: ActionHandler,
}

/// One notification.
#[derive(Clone)]
pub struct Toast {
    pub title: SharedString,
    pub description: Option<SharedString>,
    pub kind: ToastKind,
    pub action: Option<ToastAction>,
    /// `None` keeps the toast up until it's dismissed.
    pub duration: Option<Duration>,
}

impl Toast {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: None,
            kind: ToastKind::Neutral,
            action: None,
            duration: Some(Duration::from_millis(5000)),
        }
    }

    pub fn success(title: impl Into<SharedString>) -> Self {
        Self::new(title).kind(ToastKind::Success)
    }

    pub fn info(title: impl Into<SharedString>) -> Self {
        Self::new(title).kind(ToastKind::Info)
    }

    pub fn warning(title: impl Into<SharedString>) -> Self {
        Self::new(title).kind(ToastKind::Warning)
    }

    /// Errors stay a little longer by default.
    pub fn error(title: impl Into<SharedString>) -> Self {
        Self::new(title)
            .kind(ToastKind::Error)
            .duration(Duration::from_millis(8000))
    }

    pub fn loading(title: impl Into<SharedString>) -> Self {
        Self::new(title).kind(ToastKind::Loading).sticky()
    }

    pub fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        if kind == ToastKind::Loading {
            self.duration = None;
        }
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A button on the toast, e.g. "Undo". Pressing it also dismisses the toast.
    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        handler: impl Fn(ToastId, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some(ToastAction {
            label: label.into(),
            handler: Rc::new(handler),
        });
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Stays until dismissed.
    pub fn sticky(mut self) -> Self {
        self.duration = None;
        self
    }
}

struct Entry {
    id: ToastId,
    toast: Toast,
    /// Bumped by `replace`, so the glyph and text morph in again.
    version: usize,
    remaining: Duration,
    total: Duration,
    running_since: Option<Instant>,
    leaving: Option<Instant>,
    /// -1 / 1 when flicked sideways, 0 when it times out or is closed.
    leave_dir: f32,
    /// Where the stack last placed it, so a leaving toast fades in place.
    slot: Cell<(f32, f32)>,
}

impl Entry {
    fn progress_left(&self, now: Instant) -> Option<f32> {
        if self.toast.duration.is_none() || self.total.is_zero() {
            return None;
        }
        let elapsed = self
            .running_since
            .map(|since| now.saturating_duration_since(since))
            .unwrap_or_default();
        let left = self.remaining.saturating_sub(elapsed);
        Some(left.as_secs_f32() / self.total.as_secs_f32())
    }
}

struct Swipe {
    id: ToastId,
    start: f32,
    dx: f32,
}

/// The toast queue. Create it once (`cx.new(|_| Toasts::new())`), push from
/// anywhere, and render it with a [`Toaster`].
pub struct Toasts {
    entries: Vec<Entry>,
    next_id: ToastId,
    /// The pointer is over the stack: spread out and hold the timers.
    hovered: bool,
    swipe: Option<Swipe>,
    ticker: Option<Task<()>>,
    ticking: bool,
    heights: HashMap<ToastId, Rc<Cell<f32>>>,
    jobs: Vec<Task<()>>,
}

impl Default for Toasts {
    fn default() -> Self {
        Self::new()
    }
}

impl Toasts {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 1,
            hovered: false,
            swipe: None,
            ticker: None,
            ticking: false,
            heights: HashMap::new(),
            jobs: Vec::new(),
        }
    }

    /// Shows `toast` in front of the stack.
    pub fn push(&mut self, toast: Toast, cx: &mut Context<Self>) -> ToastId {
        let id = self.next_id;
        self.next_id += 1;
        let total = toast.duration.unwrap_or_default();
        self.entries.push(Entry {
            id,
            toast,
            version: 0,
            remaining: total,
            total,
            running_since: (!self.hovered).then(Instant::now),
            leaving: None,
            leave_dir: 0.,
            slot: Cell::new((0., 0.)),
        });
        self.ensure_ticker(cx);
        cx.notify();
        id
    }

    /// Swaps a toast's content (and restarts its timer); the glyph and text morph.
    pub fn replace(&mut self, id: ToastId, toast: Toast, cx: &mut Context<Self>) {
        let hovered = self.hovered;
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.id == id && e.leaving.is_none())
        {
            let total = toast.duration.unwrap_or_default();
            entry.toast = toast;
            entry.version += 1;
            entry.total = total;
            entry.remaining = total;
            entry.running_since = (!hovered).then(Instant::now);
            cx.notify();
        }
    }

    /// Shows `loading` until `future` resolves, then morphs into its toast.
    pub fn promise(
        &mut self,
        loading: Toast,
        future: impl Future<Output = Toast> + 'static,
        cx: &mut Context<Self>,
    ) -> ToastId {
        let id = self.push(loading.kind(ToastKind::Loading), cx);
        self.jobs.push(cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |t, cx| t.replace(id, result, cx)).ok();
        }));
        id
    }

    pub fn dismiss(&mut self, id: ToastId, cx: &mut Context<Self>) {
        self.dismiss_towards(id, 0., cx);
    }

    pub fn dismiss_all(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        for entry in self.entries.iter_mut().filter(|e| e.leaving.is_none()) {
            entry.leaving = Some(now);
        }
        self.ensure_ticker(cx);
        cx.notify();
    }

    /// Toasts on screen, leaving ones excluded.
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.leaving.is_none()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn dismiss_towards(&mut self, id: ToastId, dir: f32, cx: &mut Context<Self>) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.id == id && e.leaving.is_none())
        {
            entry.leaving = Some(Instant::now());
            entry.leave_dir = dir;
            self.ensure_ticker(cx);
            cx.notify();
        }
    }

    fn set_hovered(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        let now = Instant::now();
        for entry in self.entries.iter_mut().filter(|e| e.leaving.is_none()) {
            if hovered {
                if let Some(since) = entry.running_since.take() {
                    entry.remaining = entry
                        .remaining
                        .saturating_sub(now.saturating_duration_since(since));
                }
            } else {
                entry.running_since = Some(now);
            }
        }
        cx.notify();
    }

    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                let keep = this.update(cx, |t, cx| t.tick(cx)).unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }

    /// Expires timers and drops toasts that finished leaving. Returns whether
    /// the ticker should keep running.
    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        let now = Instant::now();
        let mut changed = false;
        for entry in &mut self.entries {
            if entry.leaving.is_some() || entry.toast.duration.is_none() {
                continue;
            }
            if let Some(since) = entry.running_since
                && now.saturating_duration_since(since) >= entry.remaining
            {
                entry.leaving = Some(now);
                changed = true;
            }
        }
        let before = self.entries.len();
        self.entries.retain(|e| {
            e.leaving.is_none_or(|at| {
                now.saturating_duration_since(at) < LEAVE.mul_f32(motion::speed_scale())
            })
        });
        let live: Vec<ToastId> = self.entries.iter().map(|e| e.id).collect();
        self.heights.retain(|id, _| live.contains(id));
        if self.entries.len() != before {
            changed = true;
        }
        if changed {
            cx.notify();
        }
        if self.entries.is_empty() {
            self.ticking = false;
            self.hovered = false;
            return false;
        }
        true
    }
}

/// Renders a [`Toasts`] queue as a stack anchored to its bottom edge.
#[derive(IntoElement)]
pub struct Toaster {
    id: ElementId,
    toasts: Entity<Toasts>,
    width: Pixels,
    max_visible: usize,
}

impl Toaster {
    pub fn new(id: impl Into<ElementId>, toasts: Entity<Toasts>) -> Self {
        Self {
            id: id.into(),
            toasts,
            width: px(356.),
            max_visible: 3,
        }
    }

    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into();
        self
    }

    /// How many toasts stay visible in the folded stack.
    pub fn max_visible(mut self, count: usize) -> Self {
        self.max_visible = count.max(1);
        self
    }
}

impl RenderOnce for Toaster {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let element = self.build(window, cx);
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl Toaster {
    fn build(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let now = Instant::now();

        // Measured heights, one cell per toast.
        let cells: HashMap<ToastId, Rc<Cell<f32>>> = self.toasts.update(cx, |q, _| {
            let ids: Vec<ToastId> = q.entries.iter().map(|e| e.id).collect();
            ids.into_iter()
                .map(|id| (id, q.heights.entry(id).or_default().clone()))
                .collect()
        });
        let q = self.toasts.read(cx);
        let expanded = q.hovered;
        let height_of = |id: ToastId| {
            let h = cells.get(&id).map(|c| c.get()).unwrap_or(0.);
            if h > 0. { h } else { FALLBACK_HEIGHT }
        };

        // Live toasts, newest first.
        let live: Vec<&Entry> = q
            .entries
            .iter()
            .rev()
            .filter(|e| e.leaving.is_none())
            .collect();
        let front_h = live.first().map(|e| height_of(e.id)).unwrap_or(0.);
        let mut offsets = HashMap::new();
        let mut y = 0.;
        for (depth, entry) in live.iter().enumerate() {
            let h = height_of(entry.id);
            let (bottom, inset) = if expanded {
                (y, 0.)
            } else {
                let d = depth.min(self.max_visible) as f32;
                (d * PEEK, d * STEP_INSET)
            };
            offsets.insert(entry.id, (depth, bottom, inset));
            y += h + GAP;
        }
        let stack_h = if live.is_empty() {
            0.
        } else if expanded {
            y - GAP
        } else {
            front_h + (live.len().min(self.max_visible) - 1) as f32 * PEEK
        };
        let stack_h = motion::follow(&format!("{key}-height"), stack_h, GLIDE);

        let mut animating = false;
        let swipe = q.swipe.as_ref().map(|s| (s.id, s.dx));
        let toasts_view: Vec<_> = q
            .entries
            .iter()
            .map(|entry| {
                let ekey = format!("{key}-{}", entry.id);
                let leaving = entry.leaving.is_some();
                let (depth, bottom, inset) = match offsets.get(&entry.id) {
                    Some(&(d, b, i)) => {
                        entry.slot.set((b, i));
                        (d, b, i)
                    }
                    None => {
                        let (b, i) = entry.slot.get();
                        (usize::MAX, b, i)
                    }
                };
                let hidden = depth != usize::MAX && depth > self.max_visible && !expanded;
                let y = motion::follow_from(&format!("{ekey}-y"), bottom - 16., bottom, GLIDE);
                let inset = motion::follow(&format!("{ekey}-inset"), inset, GLIDE);
                let mut alpha_target = if hidden || leaving { 0. } else { 1. };
                if depth == self.max_visible && !expanded {
                    alpha_target = 0.;
                }
                let alpha = motion::follow_from(
                    &format!("{ekey}-alpha"),
                    0.,
                    alpha_target,
                    MotionSpec::new(260, motion::EASE),
                );
                let content_alpha = motion::follow(
                    &format!("{ekey}-content"),
                    if expanded || depth == 0 || leaving {
                        1.
                    } else {
                        0.
                    },
                    MotionSpec::new(200, motion::EASE),
                );
                // Sideways: the live drag, a spring back, or the flick away.
                let dragging = swipe.filter(|(id, _)| *id == entry.id).map(|(_, dx)| dx);
                let dx = match (dragging, leaving) {
                    (Some(dx), false) => {
                        motion::follow(&format!("{ekey}-x"), dx, MotionSpec::new(0, motion::LINEAR))
                    }
                    (_, true) => motion::follow(
                        &format!("{ekey}-x"),
                        entry.leave_dir * f32::from(self.width),
                        GLIDE,
                    ),
                    _ => motion::follow(&format!("{ekey}-x"), 0., GLIDE),
                };
                let drop = if leaving && entry.leave_dir == 0. {
                    motion::follow_from(&format!("{ekey}-drop"), 0., -8., GLIDE)
                } else {
                    0.
                };
                let fade = (1. - dx.abs() / (f32::from(self.width) * 0.8)).clamp(0., 1.);
                if entry.running_since.is_some() || leaving {
                    animating = true;
                }
                let height = (!expanded && depth != 0 && depth != usize::MAX).then_some(front_h);
                toast_card(
                    &ekey,
                    entry,
                    &self.toasts,
                    height,
                    content_alpha,
                    entry.progress_left(now),
                    cells.get(&entry.id).cloned().unwrap_or_default(),
                    &t,
                )
                .absolute()
                .bottom(px(y + drop))
                .left(px(inset + dx))
                .right(px(inset - dx))
                .opacity(alpha * fade)
            })
            .collect();

        if animating {
            window.request_animation_frame();
        }

        let toasts = self.toasts.clone();
        let swiping = q.swipe.is_some();
        div()
            .id(self.id.clone())
            .relative()
            .w(self.width)
            .h(px(stack_h))
            .font_family(pink::SANS)
            .on_hover(move |hovered, _, cx| {
                toasts.update(cx, |q, cx| q.set_hovered(*hovered, cx));
            })
            .children(toasts_view)
            .when(swiping, |d| d.child(swipe_tracker(self.toasts.clone())))
            .into_any_element()
    }
}

/// While a toast is held, follows the pointer anywhere in the window.
fn swipe_tracker(toasts: Entity<Toasts>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |_, _, window, _| {
            let moving = toasts.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                moving.update(cx, |q, cx| {
                    if let Some(swipe) = q.swipe.as_mut() {
                        swipe.dx = f32::from(e.position.x) - swipe.start;
                        cx.notify();
                    }
                });
            });
            let releasing = toasts.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble {
                    return;
                }
                releasing.update(cx, |q, cx| {
                    if let Some(swipe) = q.swipe.take() {
                        if swipe.dx.abs() > SWIPE_DISMISS {
                            q.dismiss_towards(swipe.id, swipe.dx.signum(), cx);
                        }
                        cx.notify();
                    }
                });
            });
        },
    )
    .absolute()
    .size_full()
}

#[allow(clippy::too_many_arguments)]
fn toast_card(
    key: &str,
    entry: &Entry,
    toasts: &Entity<Toasts>,
    height: Option<f32>,
    content_alpha: f32,
    progress: Option<f32>,
    measured: Rc<Cell<f32>>,
    t: &Pink,
) -> gpui::Stateful<gpui::Div> {
    let toast = &entry.toast;
    let id = entry.id;
    let hover_key = format!("{key}-card");
    let hover = motion::hover_t(&hover_key);
    let tone = tone_color(toast.kind, t);
    let version = entry.version;

    let glyph: gpui::AnyElement = match toast.kind {
        ToastKind::Loading => svg()
            .path(ICON_SPINNER)
            .size(px(16.))
            .text_color(t.text_muted)
            .with_animation(
                SharedString::from(format!("{key}-spin")),
                Animation::new(Duration::from_millis(800)).repeat(),
                |svg, p| svg.with_transformation(Transformation::rotate(radians(p * 2. * PI))),
            )
            .into_any_element(),
        kind => {
            let icon = svg()
                .path(match kind {
                    ToastKind::Success => ICON_SUCCESS,
                    ToastKind::Error | ToastKind::Warning => ICON_ERROR,
                    _ => ICON_INFO,
                })
                .size(px(16.))
                .text_color(tone);
            if version == 0 {
                icon.into_any_element()
            } else {
                // Morph out of the spinner: the glyph swells into place.
                icon.with_animation(
                    SharedString::from(format!("{key}-glyph-{version}")),
                    MotionSpec::new(340, motion::EASE_OUT_QUINT).animation(),
                    |svg, p| svg.size(px(8. + 8. * p)).opacity(p),
                )
                .into_any_element()
            }
        }
    };

    let words = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(space::S2))
        .child(
            div()
                .text_size(px(text::BODY))
                .line_height(px(18.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(toast.title.clone()),
        )
        .when_some(toast.description.clone(), |d, desc| {
            d.child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .line_height(px(16.))
                    .text_color(t.text_muted)
                    .child(desc),
            )
        });
    let words = if version == 0 {
        words.into_any_element()
    } else {
        motion::rise_in(
            SharedString::from(format!("{key}-words-{version}")),
            motion::VIEW_IN,
            4.,
            words,
        )
        .into_any_element()
    };

    let action = toast.action.clone().map(|action| {
        let toasts = toasts.clone();
        pink::button(
            format!("{key}-action"),
            ButtonKind::Secondary,
            ControlSize::Sm,
            t,
        )
        .child(action.label.clone())
        .on_click(move |_, window, cx| {
            (action.handler)(id, window, cx);
            toasts.update(cx, |q, cx| q.dismiss(id, cx));
        })
    });

    let close = {
        let toasts = toasts.clone();
        pink::icon_button(format!("{key}-close"), ICON_CLOSE, ControlSize::Sm, t)
            .opacity(0.35 + 0.65 * hover)
            .on_click(move |_, _, cx| toasts.update(cx, |q, cx| q.dismiss(id, cx)))
    };

    let body = div()
        .relative()
        .flex()
        .items_start()
        .gap(px(space::S10))
        .pl(px(space::S12))
        .pr(px(space::S8))
        .py(px(space::S12))
        .opacity(content_alpha)
        .child(div().pt(px(1.)).flex_none().child(glyph))
        .child(words)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(space::S4))
                .flex_none()
                .children(action)
                .child(close),
        )
        .child(measure(measured));

    let grab = toasts.clone();
    div()
        .id(SharedString::from(hover_key.clone()))
        .overflow_hidden()
        .when_some(height, |d, h| d.h(px(h)))
        .bg(t.surface_overlay)
        .border_1()
        .border_color(t.border)
        .rounded(px(radius::LG))
        .shadow(t.menu_shadow())
        .cursor_grab()
        .on_hover(motion::hover_listener(hover_key))
        .on_mouse_down(MouseButton::Left, move |e, _, cx| {
            let start = f32::from(e.position.x);
            grab.update(cx, |q, cx| {
                q.swipe = Some(Swipe { id, start, dx: 0. });
                cx.notify();
            });
        })
        .child(body)
        .when_some(progress, |d, left| {
            // The time left, draining along the bottom edge.
            d.child(
                div()
                    .absolute()
                    .left(px(radius::LG))
                    .right(px(radius::LG))
                    .bottom_0()
                    .h(px(2.))
                    .opacity(content_alpha)
                    .child(
                        div()
                            .h_full()
                            .w(relative(left.clamp(0., 1.)))
                            .rounded(px(1.))
                            .bg(tone.opacity(0.5)),
                    ),
            )
        })
}

fn tone_color(kind: ToastKind, t: &Pink) -> Hsla {
    match kind {
        ToastKind::Success => t.success,
        ToastKind::Error => t.danger,
        ToastKind::Warning => t.warning,
        ToastKind::Info => t.accent,
        ToastKind::Neutral | ToastKind::Loading => t.text_muted,
    }
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

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

fn demo_button(
    id: &str,
    icon: &'static str,
    label: &'static str,
    t: &Pink,
) -> gpui::Stateful<gpui::Div> {
    pink::button(
        SharedString::from(format!("toaster-demo-{id}")),
        ButtonKind::Secondary,
        ControlSize::Lg,
        t,
    )
    .flex_1()
    .justify_start()
    .child(pink::glyph(icon, ControlSize::Lg).text_color(t.text_muted))
    .child(label)
}

fn deploy(toasts: &mut Toasts, cx: &mut Context<Toasts>) {
    let wait = cx.background_executor().timer(Duration::from_millis(2400));
    toasts.promise(
        Toast::loading("Deploying v2.4.0…").description("Rolling out to 12 regions"),
        async move {
            wait.await;
            Toast::success("v2.4.0 is live").description("12 regions healthy · took 48s")
        },
        cx,
    );
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let toasts =
            window.use_keyed_state("toaster-demo-queue", cx, |_, cx: &mut Context<Toasts>| {
                let mut q = Toasts::new();
                // Something to look at on arrival: a toast, then a deploy that resolves.
                q.jobs.push(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(350))
                        .await;
                    this.update(cx, |q, cx| {
                        q.push(
                            Toast::info("Preview ready")
                                .description("atlas-pr-482.preview.app")
                                .duration(Duration::from_secs(14)),
                            cx,
                        );
                    })
                    .ok();
                    cx.background_executor()
                        .timer(Duration::from_millis(700))
                        .await;
                    this.update(cx, deploy).ok();
                }));
                q
            });

        let actions = {
            let deploy_q = toasts.clone();
            let copy_q = toasts.clone();
            let archive_q = toasts.clone();
            let migrate_q = toasts.clone();
            div()
                .flex()
                .flex_col()
                .gap(px(space::S8))
                .child(
                    div()
                        .flex()
                        .gap(px(space::S8))
                        .child(
                            demo_button("deploy", "icons/toaster/send.svg", "Deploy", &t)
                                .on_click(move |_, _, cx| deploy_q.update(cx, deploy)),
                        )
                        .child(
                            demo_button("copy", "icons/toaster/link.svg", "Copy preview link", &t)
                                .on_click(move |_, _, cx| {
                                    copy_q.update(cx, |q, cx| {
                                        q.push(
                                            Toast::success("Link copied")
                                                .description("Anyone at Atlas can open it"),
                                            cx,
                                        );
                                    })
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(space::S8))
                        .child(
                            demo_button(
                                "archive",
                                "icons/toaster/archive.svg",
                                "Archive build",
                                &t,
                            )
                            .on_click(move |_, _, cx| {
                                let undo_q = archive_q.clone();
                                archive_q.update(cx, |q, cx| {
                                    q.push(
                                        Toast::new("Build #1187 archived").action(
                                            "Undo",
                                            move |_, _, cx| {
                                                undo_q.update(cx, |q, cx| {
                                                    q.push(Toast::new("Build #1187 restored"), cx);
                                                })
                                            },
                                        ),
                                        cx,
                                    );
                                })
                            }),
                        )
                        .child(
                            demo_button(
                                "migrate",
                                "icons/toaster/refresh.svg",
                                "Run migrations",
                                &t,
                            )
                            .on_click(move |_, _, cx| {
                                migrate_q.update(cx, |q, cx| {
                                    q.push(
                                        Toast::error("Migration 0042 failed")
                                            .description("Lock timeout on table orders")
                                            .action("Retry", |_, _, _| {}),
                                        cx,
                                    );
                                })
                            }),
                        ),
                )
        };

        let panel = pink::card(&t)
            .w(px(380.))
            .p(px(space::S16))
            .flex()
            .flex_col()
            .gap(px(space::S16))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S12))
                    .child(
                        div()
                            .size(px(36.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(radius::MD))
                            .bg(t.fill(FillTier::Tertiary))
                            .text_size(px(text::FOOTNOTE))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.text_muted)
                            .child("AW"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(text::CALLOUT))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.text)
                                    .child("Atlas web · v2.4.0"),
                            )
                            .child(
                                div()
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_muted)
                                    .child("Release candidate · built 6 min ago"),
                            ),
                    ),
            )
            .child(actions);

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(space::S12))
            .font_family(pink::SANS)
            .child(panel)
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child("Hover the stack to spread it · drag a toast sideways to dismiss"),
            )
            .child(
                div()
                    .h(px(176.))
                    .w(px(380.))
                    .flex()
                    .flex_col()
                    .justify_end()
                    .items_center()
                    .child(Toaster::new("toaster", toasts)),
            )
    })
}
