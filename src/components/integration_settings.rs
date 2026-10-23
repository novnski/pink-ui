//! Integration Settings – the settings card of a connected service.
//!
//! A header with the service, its status and an on/off switch (off dims and
//! locks the form), then label-and-control rows: an access token you can
//! reveal, copy and rotate, a validated workspace address, a sync cadence
//! segmented control with a gliding thumb and a time-zone menu. Any change
//! raises a save bar from the bottom edge; saving morphs into a check.
//!
//! ```ignore
//! IntegrationSettings::new("ledgerly", "Ledgerly", "LG")
//!     .subdomain("acme-books")
//!     .on_save(|values, _, _| persist(values))
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    Animation, AnyView, App, ClipboardItem, Context, ElementId, Entity, FontWeight, Hsla,
    IntoElement, KeyDownEvent, SharedString, Subscription, Task, Transformation, Window, deferred,
    div, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt, GLIDE, MotionSpec},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, Tone, radius, space, text},
    primitives::{InputEvent, TextInput, ring, shadow},
    story,
};

pub const NAME: &str = "Integration Settings";
pub const DESCRIPTION: &str = "Settings card for a connected service: a switch that pauses it, a token you can reveal and rotate, a validated address, sync cadence, time zone and a save bar that rises when something changes";

const ICON_KEY: &str = "icons/integration-settings/key.svg";
const ICON_EYE: &str = "icons/integration-settings/eye.svg";
const ICON_EYE_OFF: &str = "icons/integration-settings/eye-off.svg";
const ICON_ROTATE: &str = "icons/integration-settings/refresh.svg";
const ICON_COPY: &str = "icons/integration-settings/copy.svg";
const ICON_CHECK: &str = "icons/integration-settings/check.svg";
const ICON_ALERT: &str = "icons/integration-settings/alert.svg";
const ICON_CHEVRON: &str = "icons/integration-settings/chevron-down.svg";
const ICON_CLOCK: &str = "icons/integration-settings/clock.svg";
const ICON_SPINNER: &str = "icons/integration-settings/spinner.svg";

const LABEL_W: f32 = 176.;
const SEGMENT_W: f32 = 92.;
const SAVE_BAR_H: f32 = 56.;

/// Sync cadences: label and the line explaining it.
const CADENCES: [(&str, &str); 3] = [
    ("Real-time", "Changes go out within a few seconds."),
    ("Hourly", "Batched at the top of every hour."),
    ("Nightly", "One batch at 02:00 in the time zone below."),
];

/// Time zones offered by the menu.
pub const ZONES: [&str; 6] = [
    "UTC−08:00 · Los Angeles",
    "UTC−05:00 · New York",
    "UTC±00:00 · Lisbon",
    "UTC+01:00 · Amsterdam",
    "UTC+05:30 · Bengaluru",
    "UTC+09:00 · Seoul",
];

type SaveHandler = Rc<dyn Fn(&SettingsValues, &mut Window, &mut App)>;

/// What the save bar commits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsValues {
    pub enabled: bool,
    pub subdomain: SharedString,
    /// Index into the cadences: real-time, hourly, nightly.
    pub cadence: usize,
    /// Index into [`ZONES`].
    pub zone: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveStatus {
    Idle,
    Saving,
    Saved,
}

/// Why an address can't be used, if it can't.
fn validate(subdomain: &str) -> Option<&'static str> {
    let len = subdomain.chars().count();
    if len == 0 {
        Some("Enter the address your team signs in with")
    } else if !subdomain
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        Some("Only a–z, 0–9 and dashes")
    } else if subdomain.starts_with('-') || subdomain.ends_with('-') {
        Some("Can't start or end with a dash")
    } else if len < 3 {
        Some("At least 3 characters")
    } else if len > 32 {
        Some("At most 32 characters")
    } else {
        None
    }
}

struct SettingsState {
    seeded: bool,
    input: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
    colors_dark: Option<bool>,
    saved: SettingsValues,
    draft: SettingsValues,
    token: SharedString,
    revealed: bool,
    rotations: usize,
    rotating: Option<Task<()>>,
    copied: Option<Task<()>>,
    zone_open: bool,
    save: SaveStatus,
    save_task: Option<Task<()>>,
    /// Bumps when a save lands, so the check pops.
    saves: usize,
}

impl SettingsState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("your-team"));
        let sub = cx.subscribe(&input, |s: &mut Self, _, event: &InputEvent, cx| {
            if let InputEvent::Change(text) = event {
                s.draft.subdomain = text.clone();
                cx.notify();
            }
        });
        let values = SettingsValues {
            enabled: true,
            subdomain: SharedString::default(),
            cadence: 0,
            zone: 0,
        };
        Self {
            seeded: false,
            input,
            _subscriptions: vec![sub],
            colors_dark: None,
            saved: values.clone(),
            draft: values,
            token: SharedString::default(),
            revealed: false,
            rotations: 0,
            rotating: None,
            copied: None,
            zone_open: false,
            save: SaveStatus::Idle,
            save_task: None,
            saves: 0,
        }
    }

    fn dirty(&self) -> bool {
        self.draft != self.saved
    }

    fn discard(&mut self, cx: &mut Context<Self>) {
        self.draft = self.saved.clone();
        let text = self.saved.subdomain.clone();
        self.input.update(cx, |i, cx| i.set_text(text, cx));
        cx.notify();
    }

    fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.draft.enabled = enabled;
        self.zone_open = false;
        self.input.update(cx, |i, cx| i.set_disabled(!enabled, cx));
        cx.notify();
    }

    fn rotate(&mut self, cx: &mut Context<Self>) {
        if self.rotating.is_some() {
            return;
        }
        self.rotating = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1100))
                .await;
            this.update(cx, |s, cx| {
                s.rotations += 1;
                s.token = make_token(s.rotations);
                s.revealed = true;
                s.rotating = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(self.token.to_string()));
        self.copied = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1400))
                .await;
            this.update(cx, |s, cx| {
                s.copied = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn can_save(&self) -> bool {
        self.save == SaveStatus::Idle && self.dirty() && validate(&self.draft.subdomain).is_none()
    }

    /// Plays the round trip: a spinner, then a check, then the bar folds away.
    fn save(&mut self, cx: &mut Context<Self>) {
        if !self.can_save() {
            return;
        }
        self.save = SaveStatus::Saving;
        self.zone_open = false;
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(900))
                .await;
            this.update(cx, |s, cx| {
                s.saved = s.draft.clone();
                s.save = SaveStatus::Saved;
                s.saves += 1;
                cx.notify();
            })
            .ok();
            cx.background_executor()
                .timer(Duration::from_millis(1300))
                .await;
            this.update(cx, |s, cx| {
                s.save = SaveStatus::Idle;
                s.save_task = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

/// A plausible live token; the suffix changes with every rotation.
fn make_token(seed: usize) -> SharedString {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    let mut x = 0x9e37_79b9_u32.wrapping_mul(seed as u32 + 7);
    let body: String = (0..24)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            ALPHABET[(x % ALPHABET.len() as u32) as usize] as char
        })
        .collect();
    format!("lgr_live_{body}").into()
}

fn masked(token: &str) -> String {
    let tail: String = token
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("lgr_live_{}{}", "•".repeat(8), tail)
}

#[derive(IntoElement)]
pub struct IntegrationSettings {
    id: ElementId,
    service: SharedString,
    monogram: SharedString,
    summary: SharedString,
    domain: SharedString,
    values: SettingsValues,
    on_save: Option<SaveHandler>,
    width: gpui::Pixels,
}

impl IntegrationSettings {
    /// `monogram` is the two letters on the service tile.
    pub fn new(
        id: impl Into<ElementId>,
        service: impl Into<SharedString>,
        monogram: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            service: service.into(),
            monogram: monogram.into(),
            summary: SharedString::default(),
            domain: ".example.com".into(),
            values: SettingsValues {
                enabled: true,
                subdomain: SharedString::default(),
                cadence: 0,
                zone: 0,
            },
            on_save: None,
            width: px(580.),
        }
    }

    /// The line under the service name.
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = summary.into();
        self
    }

    /// The fixed part after the editable address, e.g. ".ledgerly.app".
    pub fn domain(mut self, domain: impl Into<SharedString>) -> Self {
        self.domain = domain.into();
        self
    }

    /// The saved values the card starts from.
    pub fn values(mut self, values: SettingsValues) -> Self {
        self.values = values;
        self
    }

    pub fn on_save(
        mut self,
        handler: impl Fn(&SettingsValues, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_save = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for IntegrationSettings {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let element = self.build(window, cx);
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl IntegrationSettings {
    fn build(self, window: &mut Window, cx: &mut App) -> gpui::AnyElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            SettingsState::new,
        );
        if !state.read(cx).seeded {
            let values = self.values.clone();
            state.update(cx, |s, cx| {
                s.seeded = true;
                s.saved = values.clone();
                s.draft = values.clone();
                s.token = make_token(0);
                let text = values.subdomain.clone();
                s.input.update(cx, |i, cx| {
                    i.set_text(text, cx);
                    i.set_disabled(!values.enabled, cx);
                });
            });
        }
        if state.read(cx).colors_dark != Some(t.dark) {
            let (faint, caret, selection) = (t.text_faint, t.accent, t.selection);
            state.update(cx, |s, cx| {
                s.colors_dark = Some(t.dark);
                s.input.update(cx, |i, _| {
                    i.set_colors(Some(faint), Some(caret), Some(selection))
                });
            });
        }

        let s = state.read(cx);
        let enabled = s.draft.enabled;
        let live = motion::follow(&format!("{key}-live"), if enabled { 1. } else { 0. }, GLIDE);
        let dirty = s.dirty();
        let save = s.save;
        let bar_open = dirty || save != SaveStatus::Idle;
        let bar = motion::follow(
            &format!("{key}-bar"),
            if bar_open { SAVE_BAR_H } else { 0. },
            GLIDE,
        );

        let header = {
            let toggle_state = state.clone();
            div()
                .flex()
                .items_center()
                .gap(px(space::S12))
                .p(px(space::S16))
                .child(
                    div()
                        .size(px(40.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::LG))
                        .bg(motion::mix(t.fill(FillTier::Tertiary), t.accent_wash, live))
                        .text_size(px(text::CALLOUT))
                        .font_weight(FontWeight::BOLD)
                        .text_color(motion::mix(
                            t.text_muted,
                            if t.dark { t.text } else { t.accent },
                            live,
                        ))
                        .child(self.monogram.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(space::S8))
                                .child(
                                    div()
                                        .text_size(px(text::CALLOUT))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(self.service.clone()),
                                )
                                .child(if enabled {
                                    pink::badge("Connected", Tone::Success, &t)
                                } else {
                                    pink::badge("Paused", Tone::Neutral, &t)
                                }),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_muted)
                                .child(if enabled {
                                    self.summary.clone()
                                } else {
                                    "Paused – nothing is sent until you turn it back on".into()
                                }),
                        ),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("{key}-enabled")))
                        .cursor_pointer()
                        .child(switch(&format!("{key}-switch"), enabled, &t))
                        .on_click(move |_, _, cx| {
                            toggle_state.update(cx, |s, cx| s.set_enabled(!s.draft.enabled, cx))
                        }),
                )
        };

        let token_row = token_row(&key, &state, &t, cx);
        let address_row = address_row(&key, &state, &self.domain, &t, window, cx);
        let cadence_row = cadence_row(&key, &state, &t, cx);
        let zone_row = zone_row(&key, &state, &t, cx);

        let body = div()
            .relative()
            .flex()
            .flex_col()
            .opacity(0.45 + 0.55 * live)
            .child(token_row)
            .child(address_row)
            .child(cadence_row)
            .child(zone_row)
            .when(!enabled, |d| {
                // Locks the form while the integration is paused.
                d.child(
                    div()
                        .id(SharedString::from(format!("{key}-lock")))
                        .absolute()
                        .inset_0()
                        .occlude(),
                )
            });

        let save_bar = save_bar(&key, &state, bar, self.on_save.clone(), &t, cx);

        pink::card(&t)
            .id(self.id.clone())
            .w(self.width)
            .font_family(pink::SANS)
            .text_color(t.text)
            .text_size(px(text::BODY))
            .overflow_hidden()
            .child(header)
            .child(body)
            .child(save_bar)
            .into_any_element()
    }
}

/// One settings row: the label column and the control column.
fn row(label: &'static str, help: impl Into<SharedString>, t: &Pink) -> gpui::Div {
    div()
        .flex()
        .gap(px(space::S16))
        .px(px(space::S16))
        .py(px(space::S16))
        .border_t_1()
        .border_color(t.border)
        .child(
            div()
                .w(px(LABEL_W))
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(text::BODY))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .text_color(t.text_muted)
                        .child(help.into()),
                ),
        )
}

fn field(t: &Pink, focused: bool, invalid: bool) -> gpui::Div {
    let edge = if invalid {
        t.danger.opacity(0.7)
    } else if focused {
        t.accent.opacity(0.6)
    } else {
        t.border
    };
    div()
        .flex()
        .items_center()
        .gap(px(space::S8))
        .h(px(32.))
        .px(px(space::S10))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(edge)
        .bg(t.input_bg)
        .when(focused, |d| {
            d.shadow(vec![ring(
                3.,
                if invalid { t.danger } else { t.accent }.opacity(0.14),
            )])
        })
}

fn token_row(key: &str, state: &Entity<SettingsState>, t: &Pink, cx: &App) -> impl IntoElement {
    let s = state.read(cx);
    let rotating = s.rotating.is_some();
    let copied = s.copied.is_some();
    let shown: SharedString = if s.revealed {
        s.token.clone()
    } else {
        masked(&s.token).into()
    };
    let text = div()
        .flex_1()
        .min_w_0()
        .truncate()
        .font_family(pink::MONO)
        .text_size(px(text::FOOTNOTE))
        .text_color(if rotating { t.text_faint } else { t.text })
        .child(shown);
    let text = if s.rotations > 0 {
        motion::rise_in(
            SharedString::from(format!("{key}-token-{}", s.rotations)),
            motion::VIEW_IN,
            4.,
            text,
        )
        .into_any_element()
    } else {
        text.into_any_element()
    };

    let reveal = {
        let state = state.clone();
        pink::icon_button(
            format!("{key}-reveal"),
            if s.revealed { ICON_EYE_OFF } else { ICON_EYE },
            ControlSize::Sm,
            t,
        )
        .on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                s.revealed = !s.revealed;
                cx.notify();
            })
        })
    };
    let copy = {
        let state = state.clone();
        pink::icon_button(
            format!("{key}-copy"),
            if copied { ICON_CHECK } else { ICON_COPY },
            ControlSize::Sm,
            t,
        )
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.copy(cx)))
    };
    let rotate = {
        let state = state.clone();
        pink::button(
            format!("{key}-rotate"),
            ButtonKind::Secondary,
            ControlSize::Lg,
            t,
        )
        .child(spinning(
            &format!("{key}-rotate-icon"),
            if rotating { ICON_SPINNER } else { ICON_ROTATE },
            rotating,
            t.text_muted,
        ))
        .child(if rotating { "Rotating…" } else { "Rotate" })
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.rotate(cx)))
    };

    row(
        "Access token",
        if s.rotations > 0 {
            "Rotated just now. The old token stops working in 24 hours."
        } else {
            "Signs every request. Rotate it if it ever leaks."
        },
        t,
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(space::S8))
            .child(
                field(t, false, false)
                    .flex_1()
                    .min_w_0()
                    .pr(px(space::S4))
                    .bg(t.fill(FillTier::Quaternary))
                    .child(
                        svg()
                            .path(ICON_KEY)
                            .size(px(14.))
                            .flex_none()
                            .text_color(t.text_faint),
                    )
                    .child(text)
                    .child(reveal)
                    .child(copy),
            )
            .child(rotate),
    )
}

fn address_row(
    key: &str,
    state: &Entity<SettingsState>,
    domain: &SharedString,
    t: &Pink,
    window: &Window,
    cx: &App,
) -> impl IntoElement {
    let s = state.read(cx);
    let input = s.input.clone();
    let focused = input.read(cx).is_focused(window);
    let subdomain = s.draft.subdomain.clone();
    let error = validate(&subdomain);
    let changed = s.draft.subdomain != s.saved.subdomain;
    // Only complain once the address was edited, or it can't be saved anyway.
    let show_error = error.is_some() && (changed || !focused);
    let status: Option<gpui::AnyElement> = if show_error {
        Some(
            svg()
                .path(ICON_ALERT)
                .size(px(15.))
                .flex_none()
                .text_color(t.danger)
                .into_any_element(),
        )
    } else if error.is_none() {
        Some(
            svg()
                .path(ICON_CHECK)
                .size(px(15.))
                .flex_none()
                .text_color(t.success)
                .into_any_element(),
        )
    } else {
        None
    };
    let help: SharedString = match error.filter(|_| show_error) {
        Some(message) => message.into(),
        None => format!("Your books open at https://{subdomain}{domain}").into(),
    };

    let focus_input = input.clone();
    row(
        "Workspace address",
        "Where your team signs in to Ledgerly.",
        t,
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(space::S6))
            .child(
                field(t, focused, show_error)
                    .id(SharedString::from(format!("{key}-address")))
                    .pr_0()
                    .overflow_hidden()
                    .cursor_text()
                    .on_click(move |_, window, cx| TextInput::focus(&focus_input, window, cx))
                    .child(div().text_color(t.text_faint).child("https://"))
                    .child(div().flex_1().min_w_0().ml(px(-6.)).child(input))
                    .children(status)
                    .child(
                        // The fixed domain, as an addon segment.
                        div()
                            .h_full()
                            .flex()
                            .items_center()
                            .px(px(space::S10))
                            .border_l_1()
                            .border_color(t.border)
                            .bg(t.fill(FillTier::Quaternary))
                            .text_color(t.text_muted)
                            .child(domain.clone()),
                    ),
            )
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .line_height(px(16.))
                    .text_color(if show_error { t.danger } else { t.text_muted })
                    .child(help),
            ),
    )
}

fn cadence_row(key: &str, state: &Entity<SettingsState>, t: &Pink, cx: &App) -> impl IntoElement {
    let s = state.read(cx);
    let current = s.draft.cadence.min(CADENCES.len() - 1);
    let thumb = motion::follow(
        &format!("{key}-cadence"),
        current as f32 * SEGMENT_W,
        MotionSpec::new(320, motion::EASE_OUT_QUINT),
    );
    let segments = CADENCES.iter().enumerate().map(|(ix, (label, _))| {
        let state = state.clone();
        let hover = format!("{key}-cadence-{ix}");
        div()
            .id(SharedString::from(hover.clone()))
            .relative()
            .w(px(SEGMENT_W))
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(px(text::FOOTNOTE))
            .font_weight(FontWeight::MEDIUM)
            .map(|d| {
                if ix == current {
                    d.text_color(t.text)
                } else {
                    d.hover_fg(hover, t.text_muted, t.text)
                }
            })
            .child(*label)
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.draft.cadence = ix;
                    cx.notify();
                })
            })
    });

    row("Sync cadence", "How often new invoices are pushed.", t).child(
        div()
            .flex()
            .flex_col()
            .gap(px(space::S6))
            .child(
                div()
                    .relative()
                    .flex()
                    .h(px(32.))
                    .p(px(3.))
                    .rounded(px(radius::MD))
                    .bg(t.fill(FillTier::Tertiary))
                    .child(
                        div()
                            .absolute()
                            .top(px(3.))
                            .bottom(px(3.))
                            .left(px(3. + thumb))
                            .w(px(SEGMENT_W))
                            .rounded(px(radius::SM))
                            .bg(if t.dark {
                                t.surface_raised_hover
                            } else {
                                t.surface_card
                            })
                            .shadow(vec![shadow(
                                0.,
                                1.,
                                2.,
                                0.,
                                gpui::black().opacity(if t.dark { 0.4 } else { 0.08 }),
                            )]),
                    )
                    .children(segments),
            )
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .child(CADENCES[current].1),
            ),
    )
}

fn zone_row(key: &str, state: &Entity<SettingsState>, t: &Pink, cx: &App) -> impl IntoElement {
    let s = state.read(cx);
    let zone = s.draft.zone.min(ZONES.len() - 1);
    let open = s.zone_open;
    let toggle = state.clone();
    let hover = format!("{key}-zone");
    let button = div()
        .id(SharedString::from(hover.clone()))
        .flex()
        .items_center()
        .gap(px(space::S8))
        .w(px(SEGMENT_W * 3. + 6.))
        .h(px(32.))
        .px(px(space::S10))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(if open {
            t.accent.opacity(0.6)
        } else {
            t.border
        })
        .when(open, |d| d.shadow(vec![ring(3., t.accent.opacity(0.14))]))
        .cursor_pointer()
        .hover_fill(hover, t.input_bg, t.fill(FillTier::Tertiary))
        .child(
            svg()
                .path(ICON_CLOCK)
                .size(px(14.))
                .text_color(t.text_faint),
        )
        .child(div().flex_1().child(ZONES[zone]))
        .child(
            svg()
                .path(ICON_CHEVRON)
                .size(px(14.))
                .text_color(t.text_faint),
        )
        .on_click(move |_, _, cx| {
            toggle.update(cx, |s, cx| {
                s.zone_open = !s.zone_open;
                cx.notify();
            })
        });

    let menu = open.then(|| {
        let items = ZONES.iter().enumerate().map(|(ix, label)| {
            let state = state.clone();
            let hover = format!("{key}-zone-{ix}");
            div()
                .id(SharedString::from(hover.clone()))
                .flex()
                .items_center()
                .gap(px(space::S8))
                .h(px(30.))
                .px(px(space::S8))
                .rounded(px(radius::SM))
                .cursor_pointer()
                .hover_fill(hover, t.element_hover.opacity(0.), t.element_hover)
                .child(div().flex_1().child(*label))
                .when(ix == zone, |d| {
                    d.child(svg().path(ICON_CHECK).size(px(14.)).text_color(t.accent))
                })
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.draft.zone = ix;
                        s.zone_open = false;
                        cx.notify();
                    })
                })
        });
        let close = state.clone();
        let escape = state.clone();
        deferred(
            div().absolute().top(px(36.)).left_0().child(motion::pop_in(
                SharedString::from(format!("{key}-zone-menu")),
                pink::overlay(t)
                    .id(SharedString::from(format!("{key}-zone-list")))
                    .w(px(SEGMENT_W * 3. + 6.))
                    .p(px(space::S4))
                    .flex()
                    .flex_col()
                    .on_mouse_down_out(move |_, _, cx| {
                        close.update(cx, |s, cx| {
                            s.zone_open = false;
                            cx.notify();
                        })
                    })
                    .on_key_down(move |e: &KeyDownEvent, _, cx| {
                        if e.keystroke.key == "escape" {
                            escape.update(cx, |s, cx| {
                                s.zone_open = false;
                                cx.notify();
                            })
                        }
                    })
                    .children(items),
            )),
        )
        .with_priority(1)
    });

    row("Time zone", "Used for nightly batches and report dates.", t)
        .child(div().relative().child(button).children(menu))
}

fn save_bar(
    key: &str,
    state: &Entity<SettingsState>,
    height: f32,
    on_save: Option<SaveHandler>,
    t: &Pink,
    cx: &App,
) -> impl IntoElement {
    let s = state.read(cx);
    let invalid = validate(&s.draft.subdomain).is_some();
    let status = s.save;
    let saves = s.saves;
    let message: gpui::AnyElement = match status {
        SaveStatus::Saved => motion::rise_in(
            SharedString::from(format!("{key}-saved-{saves}")),
            motion::VIEW_IN,
            4.,
            div()
                .flex()
                .items_center()
                .gap(px(space::S8))
                .text_color(t.success)
                .child(svg().path(ICON_CHECK).size(px(15.)).text_color(t.success))
                .child("Settings saved"),
        )
        .into_any_element(),
        _ if invalid => div()
            .text_color(t.danger)
            .child("Fix the workspace address to save")
            .into_any_element(),
        _ => div()
            .flex()
            .items_center()
            .gap(px(space::S8))
            .text_color(t.text_muted)
            .child(div().size(px(6.)).rounded_full().bg(t.warning))
            .child("You have unsaved changes")
            .into_any_element(),
    };

    let discard = {
        let state = state.clone();
        pink::button(
            format!("{key}-discard"),
            ButtonKind::Ghost,
            ControlSize::Md,
            t,
        )
        .child("Discard")
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.discard(cx)))
    };
    let commit = {
        let state = state.clone();
        pink::button(
            format!("{key}-save"),
            ButtonKind::Primary,
            ControlSize::Md,
            t,
        )
        .when(invalid, |d| d.opacity(0.45).cursor_default())
        .when(status == SaveStatus::Saving, |d| {
            d.child(spinning(
                &format!("{key}-saving"),
                ICON_SPINNER,
                true,
                t.on_solid,
            ))
        })
        .child(if status == SaveStatus::Saving {
            "Saving…"
        } else {
            "Save changes"
        })
        .on_click(move |_, window, cx| {
            let values = state
                .read(cx)
                .can_save()
                .then(|| state.read(cx).draft.clone());
            if let Some(values) = values {
                if let Some(f) = &on_save {
                    f(&values, window, cx);
                }
                state.update(cx, |s, cx| s.save(cx));
            }
        })
    };

    div().h(px(height)).overflow_hidden().child(
        div()
            .h(px(SAVE_BAR_H))
            .flex()
            .items_center()
            .gap(px(space::S8))
            .px(px(space::S16))
            .border_t_1()
            .border_color(t.border)
            .bg(t.fill(FillTier::Quaternary))
            .child(div().flex_1().text_size(px(text::FOOTNOTE)).child(message))
            .when(status != SaveStatus::Saved, |d| {
                d.child(discard).child(commit)
            }),
    )
}

/// A glyph that spins while `spin` is set.
fn spinning(id: &str, path: &'static str, spin: bool, color: Hsla) -> gpui::AnyElement {
    let icon = svg().path(path).size(px(14.)).flex_none().text_color(color);
    if spin {
        icon.with_animation(
            SharedString::from(id.to_string()),
            Animation::new(Duration::from_millis(760)).repeat(),
            |svg, p| {
                svg.with_transformation(Transformation::rotate(radians(p * std::f32::consts::TAU)))
            },
        )
        .into_any_element()
    } else {
        icon.into_any_element()
    }
}

/// An on/off switch whose knob glides and whose track fills with the accent.
fn switch(key: &str, on: bool, t: &Pink) -> impl IntoElement {
    let k = motion::follow(
        &format!("{key}-knob"),
        if on { 1. } else { 0. },
        MotionSpec::new(260, motion::EASE_OUT_QUINT),
    );
    div()
        .w(px(36.))
        .h(px(20.))
        .flex_none()
        .p(px(2.))
        .rounded_full()
        .bg(motion::mix(t.surface_raised_hover, t.accent_strong, k))
        .child(
            div()
                .size(px(16.))
                .ml(px(16. * k))
                .rounded_full()
                .bg(gpui::white())
                .shadow(vec![shadow(0., 1., 2., 0., gpui::black().opacity(0.22))]),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        IntegrationSettings::new("integration-settings", "Ledgerly", "Lg")
            .summary("Last sync 4 min ago · 1,284 invoices mirrored")
            .domain(".ledgerly.app")
            .values(SettingsValues {
                enabled: true,
                subdomain: "northwind-books".into(),
                cadence: 1,
                zone: 3,
            })
    })
}
