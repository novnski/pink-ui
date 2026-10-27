//! Toaster – Pink's feedback toast (Pink: shell.rs `Shell::render_toast`).
//!
//! A [`Toasts`] entity holds the one notice on screen, like Pink's
//! `Shell::sidebar_notice`: `push` a [`Toast`] and it takes the place of
//! whatever was showing, `replace` swaps the content of a toast you pushed,
//! and `promise` keeps a progress notice up until a future resolves into its
//! result. [`Toaster`] floats the notice as a 34pt capsule centered just above
//! its anchor (a `.relative()` parent such as the composer column), arriving
//! with a short fade and rise. Status notices clear after 4s and errors after
//! 10s; Dismiss clears one at once, and an expired notice simply disappears.
//!
//! ```ignore
//! let toasts = cx.new(|_| Toasts::new());
//! toasts.update(cx, |q, cx| {
//!     q.push(Toast::status("PINK task link copied"), cx);
//! });
//! div()
//!     .relative()
//!     .child(composer)
//!     .child(Toaster::new("toasts", toasts.clone()))
//! ```

use std::{future::Future, rc::Rc};
use web_time::Duration;

use gpui::{
    AnyView, App, BoxShadow, Context, ElementId, Entity, IntoElement, Role, SharedString, Task,
    WeakEntity, Window, div, point, prelude::*, px, relative, svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt, TOAST_IN, TOAST_IN_RISE},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, icon_size, layout, radius, space, text},
    story,
};

pub const NAME: &str = "Toaster";
pub const DESCRIPTION: &str = "One quiet capsule of feedback above the composer: a status or error glyph and a single line that fade and rise into place, an optional action, and a notice that clears itself after a few seconds or on dismiss";

/// Pink: icons.rs `INFO_CIRCLE` (Solar Linear), the status glyph.
const ICON_STATUS: &str = "icons/toaster/info-circle.svg";
/// Pink: icons.rs `DANGER_TRIANGLE` (Solar Linear), the error glyph.
const ICON_ERROR: &str = "icons/toaster/danger-triangle.svg";
/// Pink: icons.rs `CLOSE`, the dismiss glyph.
const ICON_CLOSE: &str = "icons/toaster/close.svg";

/// Capsule height (Pink: shell.rs `TOAST_HEIGHT`).
pub const TOAST_HEIGHT: f32 = 34.;
/// Space between the capsule and its anchor's top edge (Pink: render_toast
/// places the lane at `top(-(TOAST_HEIGHT + 8))`).
pub const ANCHOR_GAP: f32 = 8.;
/// Widest the capsule grows before the message truncates (Pink: render_toast).
const MAX_WIDTH: f32 = 520.;
/// Message size (Pink: render_toast `ui_rems(12.5)`).
const TEXT_SIZE: f32 = 12.5;
/// The round dismiss button (Pink: render_toast "toast-dismiss").
const DISMISS_SIZE: f32 = 24.;
/// Its close glyph (Pink: render_toast `CLOSE` at 10).
const DISMISS_GLYPH: f32 = 10.;

/// A status notice's lifetime (Pink: `SidebarNotice::transient_status`).
const STATUS_LIFETIME: Duration = Duration::from_secs(4);
/// An error notice's lifetime (Pink: `SidebarNotice::transient_error`).
const ERROR_LIFETIME: Duration = Duration::from_secs(10);

pub type ToastId = u64;

type ActionHandler = Rc<dyn Fn(ToastId, &mut Window, &mut App)>;

/// What a notice reports; picks its glyph and tone (Pink: `SidebarNoticeTone`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    /// Confirmation or progress: `INFO_CIRCLE` in the accent.
    Status,
    /// A refusal or failure: `DANGER_TRIANGLE` in readable danger.
    Error,
}

impl ToastKind {
    /// How long a notice of this kind stays up by default: 4s for status,
    /// 10s for errors.
    pub fn lifetime(self) -> Duration {
        match self {
            ToastKind::Status => STATUS_LIFETIME,
            ToastKind::Error => ERROR_LIFETIME,
        }
    }
}

#[derive(Clone)]
pub struct ToastAction {
    pub label: SharedString,
    handler: ActionHandler,
}

/// One notice: a single line of copy and its kind.
#[derive(Clone)]
pub struct Toast {
    pub message: SharedString,
    pub kind: ToastKind,
    pub action: Option<ToastAction>,
    /// `None` keeps the notice up until it's dismissed or replaced.
    pub duration: Option<Duration>,
}

impl Toast {
    /// A status notice that clears after 4s.
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
            kind: ToastKind::Status,
            action: None,
            duration: Some(STATUS_LIFETIME),
        }
    }

    /// A status notice that clears after 4s (Pink `transient_status`).
    pub fn status(message: impl Into<SharedString>) -> Self {
        Self::new(message)
    }

    /// An error notice that clears after 10s (Pink `transient_error`).
    pub fn error(message: impl Into<SharedString>) -> Self {
        Self::new(message).kind(ToastKind::Error)
    }

    /// Sets the kind along with its default lifetime; call
    /// [`duration`](Self::duration) or [`sticky`](Self::sticky) after it to
    /// override that.
    pub fn kind(mut self, kind: ToastKind) -> Self {
        self.kind = kind;
        self.duration = Some(kind.lifetime());
        self
    }

    /// A capsule action beside Dismiss, e.g. "Undo". Pressing it also clears
    /// the notice.
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

    /// Stays until dismissed or replaced (Pink `SidebarNotice::status`).
    pub fn sticky(mut self) -> Self {
        self.duration = None;
        self
    }
}

/// The notice on screen. `version` is bumped by `replace`, so the capsule
/// arrives again just as a newly published Pink notice does.
#[derive(Clone)]
struct Notice {
    id: ToastId,
    version: usize,
    toast: Toast,
}

/// The notice slot. Create it once (`cx.new(|_| Toasts::new())`), push from
/// anywhere, and render it with a [`Toaster`].
pub struct Toasts {
    current: Option<Notice>,
    next_id: ToastId,
    /// Clears the current notice when its lifetime runs out; replaced (and so
    /// cancelled) by every new notice.
    expiry: Option<Task<()>>,
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
            current: None,
            next_id: 1,
            expiry: None,
            jobs: Vec::new(),
        }
    }

    /// Shows `toast` in place of whatever was showing.
    pub fn push(&mut self, toast: Toast, cx: &mut Context<Self>) -> ToastId {
        let id = self.next_id;
        self.next_id += 1;
        self.show(
            Notice {
                id,
                version: 0,
                toast,
            },
            cx,
        );
        id
    }

    /// Swaps the content of toast `id` (and restarts its lifetime) while it is
    /// still the one showing.
    pub fn replace(&mut self, id: ToastId, toast: Toast, cx: &mut Context<Self>) {
        if let Some(version) = self
            .current
            .as_ref()
            .filter(|n| n.id == id)
            .map(|n| n.version + 1)
        {
            self.show(Notice { id, version, toast }, cx);
        }
    }

    /// Shows `progress` until `future` resolves, then its result. The result
    /// is published even if the progress notice was dismissed, as Pink's
    /// progress messages are.
    pub fn promise(
        &mut self,
        progress: Toast,
        future: impl Future<Output = Toast> + 'static,
        cx: &mut Context<Self>,
    ) -> ToastId {
        let id = self.push(progress.sticky(), cx);
        self.jobs.push(cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |q, cx| {
                if q.current_id() == Some(id) {
                    q.replace(id, result, cx);
                } else {
                    q.push(result, cx);
                }
            })
            .ok();
        }));
        id
    }

    /// Clears toast `id` if it is still the one showing.
    pub fn dismiss(&mut self, id: ToastId, cx: &mut Context<Self>) {
        if self.current_id() == Some(id) {
            self.dismiss_all(cx);
        }
    }

    /// Clears whatever is showing.
    pub fn dismiss_all(&mut self, cx: &mut Context<Self>) {
        if self.current.take().is_some() {
            self.expiry = None;
            cx.notify();
        }
    }

    pub fn is_empty(&self) -> bool {
        self.current.is_none()
    }

    fn current_id(&self) -> Option<ToastId> {
        self.current.as_ref().map(|n| n.id)
    }

    fn show(&mut self, notice: Notice, cx: &mut Context<Self>) {
        let (id, version) = (notice.id, notice.version);
        self.expiry = notice.toast.duration.map(|lifetime| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(lifetime).await;
                this.update(cx, |q, cx| {
                    if q.current
                        .as_ref()
                        .is_some_and(|n| n.id == id && n.version == version)
                    {
                        q.current = None;
                        cx.notify();
                    }
                })
                .ok();
            })
        });
        self.current = Some(notice);
        cx.notify();
    }
}

/// Renders a [`Toasts`] slot as Pink's toast capsule. Place it as a child of
/// the element it floats above, which must be `.relative()`: the capsule sits
/// centered, [`TOAST_HEIGHT`] + [`ANCHOR_GAP`] above that element's top edge.
#[derive(IntoElement)]
pub struct Toaster {
    id: ElementId,
    toasts: Entity<Toasts>,
}

impl Toaster {
    pub fn new(id: impl Into<ElementId>, toasts: Entity<Toasts>) -> Self {
        Self {
            id: id.into(),
            toasts,
        }
    }
}

impl RenderOnce for Toaster {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        // Pink: render_toast's lane, centered over the composer column.
        let lane = div()
            .absolute()
            .top(px(-(TOAST_HEIGHT + ANCHOR_GAP)))
            .left_0()
            .right_0()
            .flex()
            .justify_center();
        let Some(notice) = self.toasts.read(cx).current.clone() else {
            return lane;
        };
        let element = lane.child(capsule(&key, notice, &self.toasts, &t));
        // The dismiss hover fade needs frames on hosts without a driver.
        motion::keep_alive(window);
        element
    }
}

/// The capsule itself (Pink: shell.rs `Shell::render_toast`): a 34pt pill on
/// the opaque tooltip surface with a hairline edge and the menu shadows, a
/// 14pt tone glyph, one truncating line of 12.5pt copy, an optional action
/// capsule and a 24pt round Dismiss. It arrives over [`TOAST_IN`]: 220ms
/// expo-out, opacity plus an 8pt rise.
fn capsule(key: &str, notice: Notice, toasts: &Entity<Toasts>, t: &Pink) -> impl IntoElement {
    let Notice { id, version, toast } = notice;
    let is_error = toast.kind == ToastKind::Error;
    let tone = if is_error {
        t.readable_tone(t.danger, 1.0)
    } else {
        t.accent
    };
    let ring = t.focus_ring();

    let dismiss = {
        let toasts = toasts.clone();
        let hover_key = format!("{key}-dismiss");
        div()
            .id(SharedString::from(hover_key.clone()))
            .role(Role::Button)
            .aria_label("Dismiss notice")
            .tab_index(0)
            .size(px(DISMISS_SIZE))
            .flex_none()
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover_wash(hover_key, t.fill(FillTier::Tertiary))
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .on_click(move |_, _, cx| toasts.update(cx, |q, cx| q.dismiss(id, cx)))
            .on_key_down(pink::swallow_activation_key)
            .child(
                svg()
                    .path(ICON_CLOSE)
                    .size(px(DISMISS_GLYPH))
                    .flex_none()
                    .text_color(t.text_muted),
            )
    };

    // Not in Pink's toast: an action in Pink's notice-action capsule (Pink:
    // notice.rs `notice_action`), riding the row as notice chips' do.
    let action = toast.action.clone().map(|action| {
        let toasts = toasts.clone();
        pink::notice_action(format!("{key}-action"), t)
            .aria_label(action.label.clone())
            .child(action.label.clone())
            .on_click(move |_, window, cx| {
                (action.handler)(id, window, cx);
                toasts.update(cx, |q, cx| q.dismiss(id, cx));
            })
    });

    let pill = div()
        .id(SharedString::from(format!("{key}-toast")))
        .role(Role::Status)
        .aria_label(toast.message.clone())
        .h(px(TOAST_HEIGHT))
        .max_w(px(MAX_WIDTH))
        .pl(px(space::S12))
        .pr(px(space::S4))
        .rounded_full()
        .border_1()
        .border_color(t.hairline(0.10))
        .bg(t.tooltip_surface())
        .shadow(t.menu_shadows())
        .flex()
        .items_center()
        .gap(px(space::S8))
        .font_family(pink::SANS)
        .text_size(px(TEXT_SIZE))
        .text_color(t.text)
        .child(
            svg()
                .path(if is_error { ICON_ERROR } else { ICON_STATUS })
                .size(px(icon_size::SM))
                .flex_none()
                .text_color(tone),
        )
        .child(div().min_w_0().truncate().child(toast.message))
        .children(action)
        .child(dismiss);

    div().relative().child(pill).with_animation(
        SharedString::from(format!("{key}-in-{id}-{version}")),
        TOAST_IN.animation(),
        |el, p| el.opacity(p).top(px(TOAST_IN_RISE * (1. - p))),
    )
}

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

const ICON_COPY: &str = "icons/toaster/copy.svg";
const ICON_ARCHIVE: &str = "icons/toaster/archive-minimalistic.svg";
const ICON_SIDE_CHAT: &str = "icons/toaster/chat-round-line.svg";
const ICON_FORK: &str = "icons/toaster/arrow-fork.svg";
const ICON_ATTACH: &str = "icons/toaster/paperclip.svg";
const ICON_SEND: &str = "icons/toaster/arrow-up.svg";

/// The transcript's row gutter (Pink: transcript.rs, 48 at desktop widths).
const TRANSCRIPT_GUTTER: f32 = 48.;

fn copy_link(q: &mut Toasts, cx: &mut Context<Toasts>) {
    q.push(Toast::status("PINK task link copied"), cx);
}

fn archive_chat(q: &mut Toasts, cx: &mut Context<Toasts>) {
    let undo = cx.entity().downgrade();
    q.push(
        Toast::status("Chat archived").action("Undo", move |_, _, cx| {
            undo.update(cx, |q, cx| {
                q.push(Toast::status("Chat restored to the sidebar"), cx);
            })
            .ok();
        }),
        cx,
    );
}

fn new_side_chat(q: &mut Toasts, cx: &mut Context<Toasts>) {
    let wait = cx.background_executor().timer(Duration::from_millis(2200));
    q.promise(
        Toast::status("Creating side chat…"),
        async move {
            wait.await;
            Toast::status("Side chat created. Open it from Related chats.")
        },
        cx,
    );
}

fn fork_refused(q: &mut Toasts, cx: &mut Context<Toasts>) {
    q.push(
        Toast::error(
            "Side chats can't fork or carry their context into another chat. \
             Use New side chat for a fresh side chat under the same main chat.",
        ),
        cx,
    );
}

type Step = fn(&mut Toasts, &mut Context<Toasts>);

/// The arrival loop: each step waits, then raises a notice. From the first
/// step on a notice is always up: progress, its result, a copied link, an
/// archive with Undo, then a refusal.
const SCRIPT: [(u64, Option<Step>); 5] = [
    (400, Some(new_side_chat)),
    (5600, Some(copy_link)),
    (4000, Some(archive_chat)),
    (4000, Some(fork_refused)),
    (6000, None),
];

struct Demo {
    toasts: Entity<Toasts>,
    /// Plays [`SCRIPT`] until the first click on a trigger.
    autoplay: Option<Task<()>>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let toasts = cx.new(|_| Toasts::new());
        let queue: WeakEntity<Toasts> = toasts.downgrade();
        let autoplay = cx.spawn(async move |_, cx| {
            loop {
                for (wait, step) in SCRIPT {
                    cx.background_executor()
                        .timer(Duration::from_millis(wait))
                        .await;
                    if let Some(step) = step
                        && queue.update(cx, step).is_err()
                    {
                        return;
                    }
                }
            }
        });
        Self {
            toasts,
            autoplay: Some(autoplay),
        }
    }
}

fn trigger(
    demo: &Entity<Demo>,
    id: &str,
    icon: &'static str,
    label: &'static str,
    step: Step,
    t: &Pink,
) -> impl IntoElement {
    let demo = demo.clone();
    pink::button(
        SharedString::from(format!("toaster-demo-{id}")),
        ButtonKind::Secondary,
        ControlSize::Md,
        t,
    )
    .child(pink::glyph(icon, ControlSize::Md))
    .child(label)
    .on_click(move |_, _, cx| {
        let toasts = demo.update(cx, |d, _| {
            d.autoplay = None;
            d.toasts.clone()
        });
        toasts.update(cx, step);
    })
}

/// A 1px inset edge (the light user bubble's, Pink: transcript.rs).
fn inset_edge(color: gpui::Hsla) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color,
        offset: point(px(0.), px(0.)),
        blur_radius: px(0.),
        spread_radius: px(1.),
        inset: true,
    }]
}

/// The bottom of a chat: the last exchange (Pink: transcript.rs user bubble
/// and bare assistant prose, 14/22).
fn exchange(t: &Pink) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(space::S16))
        .text_size(px(text::PROSE))
        .line_height(px(22.))
        .text_color(t.text)
        .child(
            div().flex().justify_end().child(
                div()
                    .max_w(relative(0.8))
                    .px(px(space::S16))
                    .py(px(space::S10))
                    .rounded(px(radius::BUBBLE))
                    .bg(t.user_bubble_bg())
                    .when_some(t.user_bubble_edge(), |d, edge| d.shadow(inset_edge(edge)))
                    .child("Move the billing settings work into its own side chat."),
            ),
        )
        .child(div().child(
            "Done. The invoice and tax settings now live in a side chat, so this thread can \
             stay on the checkout flow. I left a short brief at the top of it with the files \
             involved.",
        ))
}

/// A resting composer pill for the toast to float above (Pink: composer.rs
/// `composer_surface`, opaque: `input_glass_bg` with `shadow_lg`; recessed
/// Send disc, Attach at rest).
fn composer(t: &Pink) -> impl IntoElement {
    let disc = || {
        div()
            .size(px(layout::COMPOSER_CONTROL))
            .flex_none()
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
    };
    div()
        .flex()
        .flex_col()
        .rounded(px(radius::COMPOSER_PILL))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
        .shadow(pink::shadow_lg())
        .child(
            div()
                .min_h(px(60.))
                .pt(px(space::S16))
                .pb(px(space::S4))
                .px(px(space::S16))
                .text_size(px(text::PROSE))
                .line_height(px(22.75))
                .text_color(t.placeholder)
                .child("Do anything…"),
        )
        .child(
            div()
                .h(px(46.))
                .px(px(space::S10))
                .pt(px(space::S2))
                .pb(px(space::S12))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    disc().child(
                        svg()
                            .path(ICON_ATTACH)
                            .size(px(icon_size::MD))
                            .relative()
                            .left(px(1.))
                            .text_color(t.text_muted),
                    ),
                )
                .child(
                    disc().bg(t.ink(0.08)).child(
                        svg()
                            .path(ICON_SEND)
                            .size(px(icon_size::SM))
                            .text_color(t.text.opacity(0.35)),
                    ),
                ),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("toaster-demo", cx, |_, cx| Demo::new(cx));
        let toasts = demo.read(cx).toasts.clone();

        let triggers = div()
            .flex()
            .flex_wrap()
            .justify_center()
            .gap(px(space::S8))
            .child(trigger(
                &demo,
                "copy",
                ICON_COPY,
                "Copy task link",
                copy_link,
                &t,
            ))
            .child(trigger(
                &demo,
                "archive",
                ICON_ARCHIVE,
                "Archive chat",
                archive_chat,
                &t,
            ))
            .child(trigger(
                &demo,
                "side-chat",
                ICON_SIDE_CHAT,
                "New side chat",
                new_side_chat,
                &t,
            ))
            .child(trigger(
                &demo,
                "fork",
                ICON_FORK,
                "Fork from here",
                fork_refused,
                &t,
            ));

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(space::S32))
            .font_family(pink::SANS)
            .child(triggers)
            .child(
                // The bottom of a chat pane on Pink's content canvas, with
                // the transcript's 48pt gutter (Pink: transcript.rs).
                div()
                    .px(px(TRANSCRIPT_GUTTER))
                    .pt(px(space::S24))
                    .pb(px(space::S16))
                    .rounded(px(radius::PANEL))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.bg)
                    .child(
                        div()
                            .w(px(640.))
                            .flex()
                            .flex_col()
                            .child(exchange(&t))
                            .child(
                                // The composer column the toast mounts in
                                // (Pink: shell.rs "persistent-composer"),
                                // with room above it for the capsule's lane.
                                div()
                                    .relative()
                                    .mt(px(TOAST_HEIGHT + ANCHOR_GAP + space::S24))
                                    .child(composer(&t))
                                    .child(Toaster::new("toaster", toasts)),
                            ),
                    ),
            )
    })
}
