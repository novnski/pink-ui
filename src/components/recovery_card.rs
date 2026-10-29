//! Recovery Card: the `pink-recovery-card` package
//! (`components/recovery-card`), PINK's timeline notice (transcript.rs
//! `transcript_notice`, `trailer_action`) and the turn recovery it shows
//! (`render_recovery_trailer`, novnski/pink#193), as a single-GPUI package
//! the app pins. The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. This module adds the host side (colours, glyphs, roles,
//! focus, hover and clicks) and the story.
//!
//! One card per failure names its cause. While a recovery is active, the
//! run's own error is absorbed into the card, which states the engine's
//! detail or, when that is empty, the run's error; the separate "Run error"
//! card ([`RunError`]) is for a failure nothing is recovering. The headline
//! says what is happening in plain words ("Retrying in 5s, attempt 2 of 3",
//! "Checking delivery"). A clock marks checking and a refresh glyph
//! retrying, both in the warning tone; anything that needs a person is
//! danger with a triangle. Actions ride the title row: Send now (Continue
//! when held) as the solid primary, Open Providers for an expired sign-in,
//! and Cancel.
//!
//! ```ignore
//! RecoveryCard::new("recovery", Recovery::Scheduled { retry_in: 5, attempt: 2, of: 3 })
//!     .run_error("The provider closed the stream before the turn finished.")
//!     .on_send_now(|_, _| {})
//!     .on_cancel(|_, _| {})
//! ```

#[allow(dead_code)]
mod package {
    include!("../../components/recovery-card/src/lib.rs");
}
pub use package::*;

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, ClipboardItem, Div, ElementId, Hsla, IntoElement, Role, SharedString,
    Stateful, Window, div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    pink::{self, Pink, TextStyle, TypeStyleExt, layout},
    story,
};

pub const NAME: &str = "Recovery Card";
pub const DESCRIPTION: &str = "The transcript notice for a turn that failed and is recovering: one card per failure that names its cause, a plain headline such as Retrying in 5s, attempt 2 of 3, and Send now, Open Providers or Cancel on its title row";

const ICON_CLOCK: &str = "icons/recovery-card/clock-circle.svg";
const ICON_REFRESH: &str = "icons/recovery-card/refresh.svg";
const ICON_DANGER: &str = "icons/recovery-card/danger-triangle.svg";
const ICON_COPY: &str = "icons/recovery-card/copy.svg";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A turn's recovery notice, as PINK's `render_recovery_trailer` hosts the
/// package.
#[derive(IntoElement)]
pub struct RecoveryCard {
    id: ElementId,
    recovery: Recovery,
    detail: Option<SharedString>,
    run_error: Option<SharedString>,
    observe_only: bool,
    sign_in_expired: bool,
    on_send_now: Option<Handler>,
    on_open_providers: Option<Handler>,
    on_cancel: Option<Handler>,
}

impl RecoveryCard {
    pub fn new(id: impl Into<ElementId>, recovery: Recovery) -> Self {
        Self {
            id: id.into(),
            recovery,
            detail: None,
            run_error: None,
            observe_only: false,
            sign_in_expired: false,
            on_send_now: None,
            on_open_providers: None,
            on_cancel: None,
        }
    }

    /// The engine's own account of the failure.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// The failed run's error. While the recovery is active the card names
    /// it as the cause when there is no detail, and the host shows no
    /// separate [`RunError`] for it.
    pub fn run_error(mut self, message: impl Into<SharedString>) -> Self {
        self.run_error = Some(message.into());
        self
    }

    /// The engine only observes (or needs its configuration fixed): no Send
    /// now (PINK `RecoveryAction::ObserveOnly`, `FixConfiguration`).
    pub fn observe_only(mut self, observe_only: bool) -> Self {
        self.observe_only = observe_only;
        self
    }

    /// The failure is an expired sign-in: offer Open Providers (PINK
    /// `RecoveryFailure::AuthenticationExpired`).
    pub fn sign_in_expired(mut self, expired: bool) -> Self {
        self.sign_in_expired = expired;
        self
    }

    /// Send now, or Continue while held.
    pub fn on_send_now(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_send_now = Some(Rc::new(f));
        self
    }

    pub fn on_open_providers(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_open_providers = Some(Rc::new(f));
        self
    }

    pub fn on_cancel(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_cancel = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RecoveryCard {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let tone = match self.recovery.tone() {
            Tone::Warning => t.warning,
            Tone::Danger => t.danger,
        };
        let icon = match self.recovery.glyph() {
            Glyph::Clock => ICON_CLOCK,
            Glyph::Refresh => ICON_REFRESH,
            Glyph::Danger => ICON_DANGER,
        };
        let actions = self
            .recovery
            .actions(self.observe_only, self.sign_in_expired)
            .into_iter()
            .map(|card_action| {
                let (name, handler) = match card_action.action {
                    RecoveryAction::SendNow => ("send", self.on_send_now.clone()),
                    RecoveryAction::OpenProviders => ("providers", self.on_open_providers.clone()),
                    RecoveryAction::Cancel => ("cancel", self.on_cancel.clone()),
                };
                action_button(
                    &key,
                    name,
                    card_action.label,
                    card_action.primary,
                    handler,
                    &t,
                )
            })
            .collect();
        let cause = recovery_cause(
            &self.recovery,
            self.detail.as_ref(),
            self.run_error.as_ref(),
        );
        notice(tone, icon, self.recovery.headline(), cause, actions, &t)
            .id(self.id)
            .font_family(pink::SANS)
    }
}

/// A run's error with nothing recovering it (PINK `error_chip`): the danger
/// notice titled "Run error" with the message as its detail and a Copy error
/// button.
#[derive(IntoElement)]
pub struct RunError {
    id: ElementId,
    message: SharedString,
}

impl RunError {
    pub fn new(id: impl Into<ElementId>, message: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            message: message.into(),
        }
    }
}

impl RenderOnce for RunError {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = format!("{}-copy", self.id);
        let ring = t.focus_ring();
        let copied = self.message.clone();
        let copy = copy_button()
            .id(SharedString::from(key.clone()))
            .role(Role::Button)
            .aria_label("Copy error")
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .on_key_down(pink::swallow_activation_key)
            .hover_fill(key, gpui::transparent_black(), t.ink(0.08))
            .tooltip(pink::text_tooltip("Copy error"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copied.to_string()))
            })
            .child(
                svg()
                    .path(ICON_COPY)
                    .size(ui_rems(COPY_GLYPH))
                    .text_color(t.content_muted),
            )
            .into_any_element();
        run_error_frame().id(self.id).child(
            notice(
                t.danger,
                ICON_DANGER,
                RUN_ERROR_TITLE.into(),
                Some(self.message),
                vec![copy],
                &t,
            )
            .font_family(pink::SANS),
        )
    }
}

/// The timeline's one notice (PINK `transcript_notice`): the package's card,
/// title row and detail with the host's colours, glyph and alert role.
fn notice(
    tone: Hsla,
    icon: &'static str,
    title_text: SharedString,
    detail_text: Option<SharedString>,
    actions: Vec<AnyElement>,
    t: &Pink,
) -> Div {
    let style = NoticeStyle {
        tone,
        glyph: t.readable_tone(tone, 1.0),
        title: t.text,
        detail: t.content_muted,
    };
    let said = spoken(&title_text, detail_text.as_deref());
    card(&style)
        .child(
            title_row()
                .child(
                    svg()
                        .path(icon)
                        .size(glyph_size())
                        .flex_none()
                        .text_color(style.glyph),
                )
                .child(
                    title(&style, title_text)
                        .id("notice-title")
                        .role(Role::Alert)
                        .aria_label(said),
                )
                .children(actions),
        )
        .when_some(detail_text, |el, text| el.child(detail(&style).child(text)))
}

/// A notice button (PINK `trailer_action`): the primary is a solid plate
/// easing to 0.88; the others are outlined in `border_strong` with an
/// `ink(0.06)` hover.
fn action_button(
    key: &str,
    name: &str,
    label: &'static str,
    primary: bool,
    on_click: Option<Handler>,
    t: &Pink,
) -> AnyElement {
    let (fill, hover, text, edge) = if primary {
        (
            t.solid,
            t.solid.opacity(0.88),
            t.on_solid,
            gpui::transparent_black(),
        )
    } else {
        (
            gpui::transparent_black(),
            t.ink(0.06),
            t.text,
            t.border_strong,
        )
    };
    let id = format!("{key}-{name}");
    let ring = t.focus_ring();
    action(&ActionStyle { edge, text })
        .id(SharedString::from(id.clone()))
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .hover_fill(id, fill, hover)
        .when_some(on_click, |el, f| {
            el.on_click(move |_, window, cx| f(window, cx))
        })
        .child(label)
        .into_any_element()
}

// ---------------------------------------------------------------------------
// Story: every recovery state, and the plain run error it absorbs.
// ---------------------------------------------------------------------------

const RUN_ERROR: &str = "The provider closed the stream before the turn finished.";

fn sample(caption: &'static str, card: impl IntoElement, t: &Pink) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .type_style(TextStyle::Footnote)
                .text_color(t.readable_muted(0.8))
                .child(caption),
        )
        .child(card)
}

fn demo_card(id: &'static str, recovery: Recovery) -> RecoveryCard {
    RecoveryCard::new(id, recovery)
        .on_send_now(|_, _| {})
        .on_open_providers(|_, _| {})
        .on_cancel(|_, _| {})
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let column = |cards: Vec<Stateful<Div>>| {
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(14.))
                .children(cards)
        };
        let left = vec![
            sample(
                "Retrying: the run's error is the cause",
                demo_card(
                    "rc-scheduled",
                    Recovery::Scheduled {
                        retry_in: 5,
                        attempt: 2,
                        of: 3,
                    },
                )
                .run_error(RUN_ERROR),
                &t,
            )
            .id("s1"),
            sample(
                "Checking delivery after a restart",
                demo_card("rc-checking", Recovery::CheckingDelivery)
                    .detail("Run interrupted by engine restart, reconciling delivery"),
                &t,
            )
            .id("s2"),
            sample(
                "Recovering",
                demo_card("rc-running", Recovery::Running).run_error(RUN_ERROR),
                &t,
            )
            .id("s3"),
            sample(
                "Held",
                demo_card("rc-held", Recovery::Held)
                    .detail("Recovery is paused while the thread's checkout is busy."),
                &t,
            )
            .id("s4"),
        ];
        let right = vec![
            sample(
                "Sign-in expired",
                demo_card(
                    "rc-sign-in",
                    Recovery::WaitingForSignIn {
                        machine: "Studio Mac".into(),
                    },
                )
                .sign_in_expired(true)
                .observe_only(true)
                .detail("The provider's sign-in expired. Sign in again, then retry."),
                &t,
            )
            .id("s5"),
            sample(
                "Recovery needed",
                demo_card("rc-needed", Recovery::RecoveryNeeded).detail(
                    "The worktree has uncommitted changes. Commit or stash them, then retry.",
                ),
                &t,
            )
            .id("s6"),
            sample(
                "Retry limit reached",
                demo_card("rc-exhausted", Recovery::Exhausted).detail("Three attempts failed."),
                &t,
            )
            .id("s7"),
            sample(
                "A run error with nothing recovering it",
                RunError::new("rc-run-error", RUN_ERROR),
                &t,
            )
            .id("s8"),
        ];
        div()
            .w(px(860.))
            .flex()
            .flex_row()
            .gap(px(24.))
            .child(column(left))
            .child(column(right))
    })
}
