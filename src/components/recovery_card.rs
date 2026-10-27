//! Recovery Card – a Pink component (Pink: `transcript.rs`
//! `render_recovery_trailer`, `transcript_notice`, `trailer_action`).
//!
//! PINK's transcript notice for a turn that failed and is recovering, as of
//! PINK 0.30.6 (novnski/pink#193, UI rethink #174): one card per failure
//! that names its cause. While a recovery is active, the run's own error is
//! absorbed into the card, which states the engine's detail or, when that is
//! empty, the run's error; the separate "Run error" card ([`RunError`]) is
//! for a failure nothing is recovering. The headline says what is happening
//! in plain words ("Retrying in 5s, attempt 2 of 3", "Checking delivery").
//! A clock marks checking and a refresh glyph retrying, both in the warning
//! tone; anything that needs a person is danger with a triangle. Actions
//! ride the title row: Send now (Continue when held) as the solid primary,
//! Open Providers for an expired sign-in, and Cancel.
//!
//! The notice is PINK's timeline notice (audit TL-08): a 10 pt card tinted
//! `tone@0.08` with a `tone@0.24` edge, `px 12 py 10`, a 14 pt glyph and a
//! 13 pt semibold title in a 24 pt row, the 12/17 detail indented 22 under
//! it, and 24 pt actions.
//!
//! ```ignore
//! RecoveryCard::new("recovery", Recovery::Scheduled { retry_in: 5, attempt: 2, of: 3 })
//!     .run_error("The provider closed the stream before the turn finished.")
//!     .on_send_now(|_, _| {})
//!     .on_cancel(|_, _| {})
//! ```

use std::rc::Rc;

use gpui::{
    AnyElement, AnyView, App, ClipboardItem, Div, ElementId, FontWeight, Hsla, IntoElement, Role,
    SharedString, Stateful, Window, div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    pink::{self, Pink, TextStyle, TypeStyleExt, layout, radius},
    story,
};

pub const NAME: &str = "Recovery Card";
pub const DESCRIPTION: &str = "The transcript notice for a turn that failed and is recovering: one card per failure that names its cause, a plain headline such as Retrying in 5s, attempt 2 of 3, and Send now, Open Providers or Cancel on its title row";

const ICON_CLOCK: &str = "icons/recovery-card/clock-circle.svg";
const ICON_REFRESH: &str = "icons/recovery-card/refresh.svg";
const ICON_DANGER: &str = "icons/recovery-card/danger-triangle.svg";
const ICON_COPY: &str = "icons/recovery-card/copy.svg";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// Where a turn's recovery stands (Pink proto `RecoveryStatus`). Completed,
/// cancelled and superseded recoveries show no card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// A retry is scheduled: `retry_in` seconds from now, the `attempt`th of
    /// `of` (both counted from 1).
    Scheduled {
        retry_in: u32,
        attempt: u32,
        of: u32,
    },
    /// The retry is running.
    Running,
    /// The engine is confirming whether the provider received the turn.
    CheckingDelivery,
    /// The engine can't recover on its own.
    RecoveryNeeded,
    /// The provider's sign-in expired on this machine.
    WaitingForSignIn { machine: SharedString },
    /// Recovery is paused until someone continues it.
    Held,
    /// Every automatic retry has been used.
    Exhausted,
}

impl Recovery {
    /// Pink `render_recovery_trailer`'s headline: plain words, no joiners.
    pub fn headline(&self) -> SharedString {
        match self {
            Self::Scheduled {
                retry_in,
                attempt,
                of,
            } => format!("Retrying in {retry_in}s, attempt {attempt} of {of}").into(),
            Self::Running => "Recovering safely…".into(),
            Self::CheckingDelivery => "Checking delivery".into(),
            Self::RecoveryNeeded => "Recovery needed".into(),
            Self::WaitingForSignIn { machine } => format!("Sign in on {machine}").into(),
            Self::Held => "Recovery held".into(),
            Self::Exhausted => "Automatic retry limit reached".into(),
        }
    }

    /// Pink `RecoveryStatus::active`: still recovering. Only an active
    /// recovery absorbs the run's error.
    pub fn active(&self) -> bool {
        !matches!(self, Self::Exhausted)
    }

    fn checking(&self) -> bool {
        matches!(self, Self::CheckingDelivery)
    }

    fn retrying(&self) -> bool {
        matches!(self, Self::Scheduled { .. } | Self::Running)
    }

    /// Send now applies while the recovery waits on something a person can
    /// push past (Pink `can_send`).
    fn can_send(&self) -> bool {
        self.active() && !matches!(self, Self::Running | Self::CheckingDelivery)
    }

    /// Tone and glyph (Pink `render_recovery_trailer`): a warning clock while
    /// checking, a warning refresh while retrying, otherwise a danger
    /// triangle.
    fn look(&self, t: &Pink) -> (Hsla, &'static str) {
        if self.checking() {
            (t.warning, ICON_CLOCK)
        } else if self.retrying() {
            (t.warning, ICON_REFRESH)
        } else {
            (t.danger, ICON_DANGER)
        }
    }
}

/// The card's cause (Pink `render_recovery_trailer`, PINK 0.30.6): the
/// engine's detail, else the run's own error while the recovery is active,
/// since the card absorbs that error's inline card.
pub fn recovery_cause(
    recovery: &Recovery,
    detail: Option<&SharedString>,
    run_error: Option<&SharedString>,
) -> Option<SharedString> {
    detail
        .filter(|d| !d.trim().is_empty())
        .or_else(|| run_error.filter(|_| recovery.active()))
        .cloned()
}

/// A turn's recovery notice.
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
    /// now (Pink `RecoveryAction::ObserveOnly`, `FixConfiguration`).
    pub fn observe_only(mut self, observe_only: bool) -> Self {
        self.observe_only = observe_only;
        self
    }

    /// The failure is an expired sign-in: offer Open Providers (Pink
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
        let (tone, icon) = self.recovery.look(&t);
        let can_send = self.recovery.can_send() && !self.observe_only;
        let mut actions: Vec<AnyElement> = Vec::new();
        if can_send {
            let label = if self.recovery == Recovery::Held {
                "Continue"
            } else {
                "Send now"
            };
            actions.push(action_button(
                &key,
                "send",
                label,
                true,
                self.on_send_now,
                &t,
            ));
        }
        if self.sign_in_expired {
            // The accounts live on Settings, Providers: name the page the
            // user lands on.
            actions.push(action_button(
                &key,
                "providers",
                "Open Providers",
                !can_send,
                self.on_open_providers,
                &t,
            ));
        }
        if self.recovery.active() && self.recovery != Recovery::Running {
            actions.push(action_button(
                &key,
                "cancel",
                "Cancel",
                false,
                self.on_cancel,
                &t,
            ));
        }
        let cause = recovery_cause(
            &self.recovery,
            self.detail.as_ref(),
            self.run_error.as_ref(),
        );
        transcript_notice(tone, icon, self.recovery.headline(), cause, actions, &t)
            .id(self.id)
            .font_family(pink::SANS)
    }
}

/// A run's error with nothing recovering it (Pink `error_chip`): the danger
/// notice titled "Run error" with the message as its detail and a 24 pt
/// Copy error button.
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
        let copy = div()
            .id(SharedString::from(key.clone()))
            .role(Role::Button)
            .aria_label("Copy error")
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .on_key_down(pink::swallow_activation_key)
            .size(px(24.))
            .flex_none()
            .rounded(px(radius::SM))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover_fill(key, gpui::transparent_black(), t.ink(0.08))
            .tooltip(pink::text_tooltip("Copy error"))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(copied.to_string()))
            })
            .child(
                svg()
                    .path(ICON_COPY)
                    .size(px(12.))
                    .text_color(t.content_muted),
            )
            .into_any_element();
        div().id(self.id).py(px(4.)).w_full().child(
            transcript_notice(
                t.danger,
                ICON_DANGER,
                "Run error".into(),
                Some(self.message),
                vec![copy],
                &t,
            )
            .font_family(pink::SANS),
        )
    }
}

/// The timeline's one notice (Pink `transcript_notice`, audit TL-08).
fn transcript_notice(
    tone: Hsla,
    icon: &'static str,
    title: SharedString,
    detail: Option<SharedString>,
    actions: Vec<AnyElement>,
    t: &Pink,
) -> Div {
    let glyph = t.readable_tone(tone, 1.0);
    div()
        .w_full()
        .rounded(px(radius::LG))
        .border_1()
        .border_color(tone.opacity(0.24))
        .bg(tone.opacity(0.08))
        .px(px(12.))
        .py(px(10.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .min_h(px(24.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(svg().path(icon).size(px(14.)).flex_none().text_color(glyph))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(title),
                )
                .children(actions),
        )
        .when_some(detail, |card, detail| {
            // Pink `notice_detail`: aligned under the title.
            card.child(
                div()
                    .pl(px(22.))
                    .max_w(px(680.))
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(t.content_muted)
                    .child(detail),
            )
        })
}

/// A notice button (Pink `trailer_action`, audit TL-08): 24 pt, `px 10`,
/// radius 6, 12 medium. The primary is a solid plate easing to 0.88; the
/// others are outlined in `border_strong` with an `ink(0.06)` hover.
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
    div()
        .id(SharedString::from(id.clone()))
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .flex_none()
        .h(px(24.))
        .px(px(10.))
        .flex()
        .items_center()
        .rounded(px(radius::SM))
        .border_1()
        .border_color(edge)
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(text)
        .cursor_pointer()
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

fn card(id: &'static str, recovery: Recovery) -> RecoveryCard {
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
                card(
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
                card("rc-checking", Recovery::CheckingDelivery)
                    .detail("Run interrupted by engine restart, reconciling delivery"),
                &t,
            )
            .id("s2"),
            sample(
                "Recovering",
                card("rc-running", Recovery::Running).run_error(RUN_ERROR),
                &t,
            )
            .id("s3"),
            sample(
                "Held",
                card("rc-held", Recovery::Held)
                    .detail("Recovery is paused while the thread's checkout is busy."),
                &t,
            )
            .id("s4"),
        ];
        let right = vec![
            sample(
                "Sign-in expired",
                card(
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
                card("rc-needed", Recovery::RecoveryNeeded).detail(
                    "The worktree has uncommitted changes. Commit or stash them, then retry.",
                ),
                &t,
            )
            .id("s6"),
            sample(
                "Retry limit reached",
                card("rc-exhausted", Recovery::Exhausted).detail("Three attempts failed."),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// PINK 0.30.6. Failure modes: a headline with a dot joiner or a
    /// fraction; an active recovery ignoring the run's error; an exhausted
    /// one absorbing it; a blank detail beating the run's error.
    #[test]
    fn one_card_names_its_cause() {
        let scheduled = Recovery::Scheduled {
            retry_in: 5,
            attempt: 2,
            of: 3,
        };
        assert_eq!(scheduled.headline(), "Retrying in 5s, attempt 2 of 3");
        let error: SharedString = RUN_ERROR.into();
        let detail: SharedString = "Run interrupted by engine restart".into();
        assert_eq!(
            recovery_cause(&scheduled, None, Some(&error)),
            Some(error.clone())
        );
        assert_eq!(
            recovery_cause(&scheduled, Some(&detail), Some(&error)),
            Some(detail.clone())
        );
        let blank: SharedString = " ".into();
        assert_eq!(
            recovery_cause(&scheduled, Some(&blank), Some(&error)),
            Some(error.clone())
        );
        assert_eq!(
            recovery_cause(&Recovery::Exhausted, None, Some(&error)),
            None
        );
        assert!(Recovery::Held.can_send() && !Recovery::CheckingDelivery.can_send());
        assert!(!Recovery::Exhausted.can_send());
    }
}
