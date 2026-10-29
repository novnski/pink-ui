// PINK's timeline notice (transcript.rs `transcript_notice`, `notice_detail`
// and `trailer_action`, audit TL-08) and the turn recovery it shows
// (`render_recovery_trailer`, novnski/pink#193): one card per failure that
// names its cause. Run errors, turn recovery and undelivered sends share the
// card: a tinted 10 pt plate with a 14 pt glyph, a 13 pt semibold title in a
// 24 pt row, optional 12/17 detail indented under the title and 24 pt
// actions on the title row.
//
// The recovery model lives here too, so PINK and the gallery word and order
// a recovery the same way: the headline in plain words ("Retrying in 5s,
// attempt 2 of 3"), its tone and glyph, which actions it offers and which
// one is the solid primary, and the cause it names.
//
// The package draws only. The host supplies the colours already resolved
// (the tone, the readable glyph ink, the title and detail ink, an action's
// edge and text), its glyphs and the copy; it adds the ids, the roles,
// accessible names and focus rings, the hover fills, the clipboard write and
// the listeners. Plain `//` comments: the pink-ui gallery compiles this file
// in place with `include!` for its story.

use gpui::{AbsoluteLength, Div, FontWeight, Hsla, SharedString, div, prelude::*, px, rems};

/// The card's corner radius (PINK `radius::LG`). Fixed, like every radius.
pub const RADIUS: f32 = 10.0;
/// The card's padding and the gap between the title row and the detail.
pub const PAD_X: f32 = 12.0;
pub const PAD_Y: f32 = 10.0;
pub const GAP: f32 = 4.0;
/// The card's tint and edge: the tone at these alphas.
pub const FILL_ALPHA: f32 = 0.08;
pub const EDGE_ALPHA: f32 = 0.24;
/// The title row: at least an action tall, its parts this far apart.
pub const ROW_MIN_HEIGHT: f32 = 24.0;
pub const ROW_GAP: f32 = 8.0;
/// The tone glyph before the title.
pub const GLYPH: f32 = 14.0;
/// The title, semibold.
pub const TITLE_TEXT: f32 = 13.0;
/// The detail under the title: indented to the title's edge (the glyph and
/// the row gap), held to a readable measure.
pub const DETAIL_TEXT: f32 = 12.0;
pub const DETAIL_LINE_HEIGHT: f32 = 17.0;
pub const DETAIL_INSET: f32 = GLYPH + ROW_GAP;
pub const DETAIL_MAX_WIDTH: f32 = 680.0;
/// A notice action on the title row.
pub const ACTION_HEIGHT: f32 = 24.0;
pub const ACTION_PAD_X: f32 = 10.0;
pub const ACTION_RADIUS: f32 = 6.0;
pub const ACTION_TEXT: f32 = 12.0;
/// The run error's Copy error button, its radius and glyph.
pub const COPY_SIZE: f32 = 24.0;
pub const COPY_RADIUS: f32 = 6.0;
pub const COPY_GLYPH: f32 = 12.0;
/// The run error notice's own vertical margin in the timeline.
pub const RUN_ERROR_PAD_Y: f32 = 4.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What a notice draws, already resolved by the host.
pub struct NoticeStyle {
    /// The notice's tone: the card is tinted and edged with it.
    pub tone: Hsla,
    /// The glyph: the tone held readable on the card.
    pub glyph: Hsla,
    /// The title.
    pub title: Hsla,
    /// The detail.
    pub detail: Hsla,
}

/// The card: the title row over the detail, tinted with the tone. The host
/// adds the [`title_row`] and, when there is one, the [`detail`].
pub fn card(style: &NoticeStyle) -> Div {
    div()
        .w_full()
        .rounded(px(RADIUS))
        .border_1()
        .border_color(style.tone.opacity(EDGE_ALPHA))
        .bg(style.tone.opacity(FILL_ALPHA))
        .px(ui_rems(PAD_X))
        .py(ui_rems(PAD_Y))
        .flex()
        .flex_col()
        .gap(ui_rems(GAP))
}

/// The title row. The host adds the glyph ([`glyph_size`]), the [`title`]
/// and the actions.
pub fn title_row() -> Div {
    div()
        .min_h(ui_rems(ROW_MIN_HEIGHT))
        .flex()
        .flex_row()
        .items_center()
        .gap(ui_rems(ROW_GAP))
}

/// The glyph's size, for the host's icon element.
pub fn glyph_size() -> AbsoluteLength {
    ui_rems(GLYPH)
}

/// The title, filling the row up to the actions. The host adds the id and
/// the alert role that announces the title and detail once.
pub fn title(style: &NoticeStyle, label: impl Into<SharedString>) -> Div {
    div()
        .min_w_0()
        .flex_1()
        .text_size(ui_rems(TITLE_TEXT))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(style.title)
        .child(label.into())
}

/// The detail, aligned under the title. The host adds the text (plain, or
/// with its own highlights painted in).
pub fn detail(style: &NoticeStyle) -> Div {
    div()
        .pl(ui_rems(DETAIL_INSET))
        .max_w(ui_rems(DETAIL_MAX_WIDTH))
        .text_size(ui_rems(DETAIL_TEXT))
        .line_height(ui_rems(DETAIL_LINE_HEIGHT))
        .text_color(style.detail)
}

/// What a notice action draws, already resolved by the host. The primary
/// action is a solid plate with a clear edge; the others are outlined.
pub struct ActionStyle {
    pub edge: Hsla,
    pub text: Hsla,
}

/// A notice action on the title row: `ACTION_HEIGHT` tall, `ACTION_RADIUS`
/// corners, 12 pt medium. The host adds the id, role, name, focus ring, the
/// fill and its hover fade, the click and the label.
pub fn action(style: &ActionStyle) -> Div {
    div()
        .flex_none()
        .h(ui_rems(ACTION_HEIGHT))
        .px(ui_rems(ACTION_PAD_X))
        .flex()
        .items_center()
        .rounded(px(ACTION_RADIUS))
        .border_1()
        .border_color(style.edge)
        .text_size(ui_rems(ACTION_TEXT))
        .font_weight(FontWeight::MEDIUM)
        .text_color(style.text)
        .cursor_pointer()
}

/// The run error's Copy error button frame. The host adds the id, role,
/// name, hover fill, tooltip, the clipboard write and its glyph
/// (`COPY_GLYPH` rems).
pub fn copy_button() -> Div {
    div()
        .size(ui_rems(COPY_SIZE))
        .flex_none()
        .rounded(px(COPY_RADIUS))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
}

/// The run error notice's frame in the timeline: the card with its margin.
pub fn run_error_frame() -> Div {
    div().py(ui_rems(RUN_ERROR_PAD_Y)).w_full()
}

/// The run error's title: a failure nothing is recovering.
pub const RUN_ERROR_TITLE: &str = "Run error";
/// An undelivered send's title; its one action is [`RETRY_LABEL`].
pub const UNDELIVERED_TITLE: &str = "Not delivered";
pub const RETRY_LABEL: &str = "Retry";

/// Where a turn's recovery stands (PINK proto `RecoveryStatus`). Completed,
/// cancelled and superseded recoveries show no card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recovery {
    /// A retry is scheduled: `retry_in` seconds from now, the `attempt`th of
    /// `of` (both counted from 1).
    Scheduled { retry_in: u64, attempt: u32, of: u32 },
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

/// A recovery's tone: warning while it is still on its way, danger when it
/// needs a person.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Warning,
    Danger,
}

/// A recovery's glyph, for the host's icon set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// A clock: checking delivery.
    Clock,
    /// A refresh arrow: retrying.
    Refresh,
    /// A danger triangle: a person is needed.
    Danger,
}

/// An action a recovery card offers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Send the turn again now (worded Continue while held).
    SendNow,
    /// Open Settings, Providers to sign in again.
    OpenProviders,
    /// Stop recovering.
    Cancel,
}

/// One action on the card, in title-row order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CardAction {
    pub action: RecoveryAction,
    pub label: &'static str,
    /// The solid primary; every other action is outlined.
    pub primary: bool,
}

impl Recovery {
    /// The headline: plain words, no joiners.
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

    /// Still recovering (PINK `RecoveryStatus::active`). Only an active
    /// recovery absorbs the run's error and can be cancelled.
    pub fn active(&self) -> bool {
        !matches!(self, Self::Exhausted)
    }

    /// A warning while checking or retrying, otherwise danger.
    pub fn tone(&self) -> Tone {
        match self {
            Self::CheckingDelivery | Self::Scheduled { .. } | Self::Running => Tone::Warning,
            _ => Tone::Danger,
        }
    }

    /// A clock while checking, a refresh arrow while retrying, otherwise a
    /// danger triangle.
    pub fn glyph(&self) -> Glyph {
        match self {
            Self::CheckingDelivery => Glyph::Clock,
            Self::Scheduled { .. } | Self::Running => Glyph::Refresh,
            _ => Glyph::Danger,
        }
    }

    /// The card's actions, in order. Send now (Continue while held) applies
    /// while the recovery waits on something a person can push past, unless
    /// the engine only observes or needs its configuration fixed
    /// (`observe_only`). Open Providers answers an expired sign-in and is
    /// the primary when there is no Send now. Cancel stops any active
    /// recovery that is not mid-run.
    pub fn actions(&self, observe_only: bool, sign_in_expired: bool) -> Vec<CardAction> {
        let can_send = self.active()
            && !observe_only
            && !matches!(self, Self::Running | Self::CheckingDelivery);
        let mut actions = Vec::new();
        if can_send {
            actions.push(CardAction {
                action: RecoveryAction::SendNow,
                label: if *self == Self::Held {
                    "Continue"
                } else {
                    "Send now"
                },
                primary: true,
            });
        }
        if sign_in_expired {
            // The accounts live on Settings, Providers: name the page the
            // user lands on.
            actions.push(CardAction {
                action: RecoveryAction::OpenProviders,
                label: "Open Providers",
                primary: !can_send,
            });
        }
        if self.active() && *self != Self::Running {
            actions.push(CardAction {
                action: RecoveryAction::Cancel,
                label: "Cancel",
                primary: false,
            });
        }
        actions
    }
}

/// The card's cause: the engine's detail, else the run's own error while the
/// recovery is active, since the card absorbs that error's own notice.
pub fn recovery_cause(
    recovery: &Recovery,
    detail: Option<&SharedString>,
    run_error: Option<&SharedString>,
) -> Option<SharedString> {
    detail
        .filter(|detail| !detail.trim().is_empty())
        .or_else(|| run_error.filter(|_| recovery.active()))
        .cloned()
}

/// What the alert role announces: the title and its detail together.
pub fn spoken(title: &str, detail: Option<&str>) -> SharedString {
    match detail {
        Some(detail) => format!("{title}: {detail}").into(),
        None => SharedString::from(title.to_owned()),
    }
}
