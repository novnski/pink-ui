//! Integration Settings – the settings card of a connected service (Pink:
//! settings/accounts.rs `render_provider_card`).
//!
//! A section card. Its header holds the service's tile, its name with a
//! Connected badge and a line under it, and the switch that turns it on or off.
//! The switch applies at once and folds the card to its header, as Pink's
//! provider cards do. Settings rows follow: the access token in mono with
//! show, copy and rotate; the workspace address, checked as you type; the
//! sync cadence as a segmented control; the time zone as a pop-up menu. The
//! rows' edits wait in a footer row with Discard and Save (Pink's automation
//! editor footer), which folds open on the first change and closes once
//! saved.
//!
//! ```ignore
//! IntegrationSettings::new("ledgerly", "Ledgerly", "LG")
//!     .domain(".ledgerly.app")
//!     .on_save(|values, _, _| persist(values))
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};
use web_time::Instant;

use gpui::{
    Anchor, AnyElement, AnyView, App, ClickEvent, Context, DispatchPhase, Div, ElementId, Entity,
    IntoElement, MouseButton, MouseDownEvent, Role, SharedString, Subscription, Task, Toggled,
    Window, anchored, canvas, deferred, div, prelude::*, px,
};

use crate::{
    motion::{self, DISCLOSURE, MENU_OUT, MENU_RISE_DOWN},
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, Segment, TextStyle, Tone, TypeStyleExt,
        layout, loaders, space,
    },
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Integration Settings";
pub const DESCRIPTION: &str = "A connected service on a Pink settings card – a switch that folds the card to its header when off, a token to show, copy and rotate, an address checked as you type, a sync cadence on a segmented track, a time-zone pop-up menu, and a footer that holds edits until you save";

const ICON_EYE: &str = "icons/integration-settings/eye.svg";
const ICON_EYE_CLOSED: &str = "icons/integration-settings/eye-closed.svg";
const ICON_COPY: &str = "icons/integration-settings/copy.svg";
const ICON_COPY_CHECK: &str = "icons/integration-settings/copy-check.svg";
const ICON_REFRESH: &str = "icons/integration-settings/refresh.svg";

/// Pink's settings column: `max_w 720` less its 32 px gutters.
const CARD_W: f32 = 656.;
/// The header's mark slot (Pink: a 16 brand mark in a 20 box).
const MARK_SLOT: f32 = 20.;
/// The address field (Pink connections.rs "Nearby address").
const ADDRESS_W: f32 = 260.;
/// The pop-up button and its menu (Pink provider_defaults.rs `SELECT_WIDTH`,
/// `MENU_WIDTH`), and the list's height budget before it scrolls.
const SELECT_W: f32 = 220.;
const MENU_W: f32 = 260.;
const MENU_MAX_H: f32 = 320.;
/// The footer row: its hairline, `py 12` and a Md button.
const FOOTER_H: f32 = 1. + 12. + 28. + 12.;
/// How long a copy reads "Copied" (Pink devices.rs `copy_id`).
const COPIED: Duration = Duration::from_millis(1500);

/// Sync cadences: label and the line explaining it.
const CADENCES: [(&str, &str); 3] = [
    ("Real-time", "Changes go out within a few seconds."),
    ("Hourly", "Batched at the top of every hour."),
    ("Nightly", "One batch at 02:00 in the time zone below."),
];

/// Time zones offered by the menu: the offset, then the city.
pub const ZONES: [&str; 6] = [
    "UTC−08:00 · Los Angeles",
    "UTC−05:00 · New York",
    "UTC±00:00 · Lisbon",
    "UTC+01:00 · Amsterdam",
    "UTC+05:30 · Bengaluru",
    "UTC+09:00 · Seoul",
];

type SaveHandler = Rc<dyn Fn(&SettingsValues, &mut Window, &mut App)>;

/// What the card commits: the switch on its own, the rows on Save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsValues {
    pub enabled: bool,
    pub subdomain: SharedString,
    /// Index into the cadences: real-time, hourly, nightly.
    pub cadence: usize,
    /// Index into [`ZONES`].
    pub zone: usize,
}

/// The footer's round trip: saving, the "Saved" line, then folding away
/// with that line still showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveStatus {
    Idle,
    Saving,
    Saved,
    Leaving,
}

/// Why an address can't be used, if it can't.
fn validate(subdomain: &str) -> Option<&'static str> {
    let len = subdomain.chars().count();
    if len == 0 {
        Some("Enter the address your team signs in with.")
    } else if !subdomain
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        Some("Use only a–z, 0–9 and dashes.")
    } else if subdomain.starts_with('-') || subdomain.ends_with('-') {
        Some("It can't start or end with a dash.")
    } else if len < 3 {
        Some("Use at least 3 characters.")
    } else if len > 32 {
        Some("Use at most 32 characters.")
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
    /// The token's readable head ("lg_live_").
    prefix: SharedString,
    token: SharedString,
    revealed: bool,
    rotations: usize,
    rotating: Option<Task<()>>,
    copied: Option<Task<()>>,
    menu_open: bool,
    /// When the menu's exit fade began; the card stays mounted until it ends.
    menu_closing: Option<Instant>,
    /// The trigger press found the menu mounted, so its click leaves it
    /// closed (Pink `note_trigger_press`).
    menu_pressed_open: bool,
    menu_reap: Option<Task<()>>,
    save: SaveStatus,
    save_task: Option<Task<()>>,
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
            prefix: SharedString::default(),
            token: SharedString::default(),
            revealed: false,
            rotations: 0,
            rotating: None,
            copied: None,
            menu_open: false,
            menu_closing: None,
            menu_pressed_open: false,
            menu_reap: None,
            save: SaveStatus::Idle,
            save_task: None,
        }
    }

    fn dirty(&self) -> bool {
        self.draft != self.saved
    }

    fn can_save(&self) -> bool {
        self.save != SaveStatus::Saving && self.dirty() && validate(&self.draft.subdomain).is_none()
    }

    fn discard(&mut self, cx: &mut Context<Self>) {
        self.draft = self.saved.clone();
        let text = self.saved.subdomain.clone();
        self.input.update(cx, |i, cx| i.set_text(text, cx));
        cx.notify();
    }

    /// The switch applies at once (Pink saves settings immediately); the
    /// rows' pending edits stay in the draft. Returns what was committed.
    fn set_enabled(&mut self, on: bool, cx: &mut Context<Self>) -> SettingsValues {
        self.saved.enabled = on;
        self.draft.enabled = on;
        // The trigger folds away with the rows.
        self.menu_open = false;
        self.menu_closing = None;
        self.menu_reap = None;
        cx.notify();
        self.saved.clone()
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
                s.token = make_token(&s.prefix, s.rotations);
                s.revealed = true;
                s.rotating = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(self.token.to_string()));
        self.copied = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED).await;
            this.update(cx, |s, cx| {
                s.copied = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn menu_mounted(&self) -> bool {
        self.menu_open || self.menu_closing.is_some()
    }

    fn open_menu(&mut self) {
        self.menu_open = true;
        self.menu_closing = None;
        self.menu_reap = None;
    }

    /// Starts the menu's exit fade and unmounts it once the fade has run
    /// (Pink `Popup::close` + `reap_popup`).
    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if !self.menu_open {
            return;
        }
        self.menu_open = false;
        let since = Instant::now();
        self.menu_closing = Some(since);
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        self.menu_reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                if s.menu_closing == Some(since) {
                    s.menu_closing = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Saving, then "Saved" for a moment, then the footer folds away.
    fn save(&mut self, cx: &mut Context<Self>) {
        if !self.can_save() {
            return;
        }
        self.save = SaveStatus::Saving;
        self.close_menu(cx);
        let fold = DISCLOSURE.total().mul_f32(motion::speed_scale());
        self.save_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(900))
                .await;
            this.update(cx, |s, cx| {
                s.saved = s.draft.clone();
                s.save = SaveStatus::Saved;
                cx.notify();
            })
            .ok();
            cx.background_executor()
                .timer(Duration::from_millis(1300))
                .await;
            this.update(cx, |s, cx| {
                s.save = SaveStatus::Leaving;
                cx.notify();
            })
            .ok();
            cx.background_executor().timer(fold).await;
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

/// A token's readable head from the service's monogram: "LG" → "lg_live_".
fn token_prefix(monogram: &str) -> SharedString {
    let head: String = monogram
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    format!("{}_live_", if head.is_empty() { "sk" } else { &head }).into()
}

/// A plausible live token; the body changes with every rotation.
fn make_token(prefix: &str, seed: usize) -> SharedString {
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
    format!("{prefix}{body}").into()
}

/// The token with its body hidden but for the last four characters.
fn masked(prefix: &str, token: &str) -> SharedString {
    let tail: String = token
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{prefix}{}{tail}", "•".repeat(8)).into()
}

/// A segmented control's width from its widest label (Pink
/// settings/provider_defaults.rs `segmented_width`).
fn segmented_width(labels: &[&str]) -> f32 {
    let widest = labels
        .iter()
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(4) as f32;
    (widest * 6.6 + 24.0).max(56.0) * labels.len().max(1) as f32
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
}

impl IntegrationSettings {
    /// `monogram` names the service's tile, which shows its first letter or
    /// digit, and heads its access token.
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
        }
    }

    /// The line under the service name while it is on.
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

    /// Called with the committed values: when the switch flips, and when
    /// Save commits the rows.
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
        // driver ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl IntegrationSettings {
    fn build(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            SettingsState::new,
        );
        if !state.read(cx).seeded {
            let values = self.values.clone();
            let prefix = token_prefix(&self.monogram);
            state.update(cx, |s, cx| {
                s.seeded = true;
                s.saved = values.clone();
                s.draft = values.clone();
                s.token = make_token(&prefix, 0);
                s.prefix = prefix;
                let text = values.subdomain.clone();
                s.input.update(cx, |i, cx| i.set_text(text, cx));
            });
        }
        if state.read(cx).colors_dark != Some(t.dark) {
            let (placeholder, caret, selection) = (t.text_faint, t.caret, t.selection);
            state.update(cx, |s, cx| {
                s.colors_dark = Some(t.dark);
                s.input.update(cx, |i, _| {
                    i.set_colors(Some(placeholder), Some(caret), Some(selection))
                });
            });
        }

        let (on, saving, footer_open) = {
            let s = state.read(cx);
            let on = s.saved.enabled;
            let footer_open =
                on && (s.dirty() || matches!(s.save, SaveStatus::Saving | SaveStatus::Saved));
            (on, s.save == SaveStatus::Saving, footer_open)
        };

        let card = pink::section_card(&t)
            .id(self.id.clone())
            .mt_0()
            .w(px(CARD_W))
            .font_family(pink::SANS)
            .text_size(px(13.))
            .text_color(t.text)
            .child(self.header(&key, &state, on, saving, &t));
        // Off folds the card to its header (Pink: the provider card returns
        // before its rows).
        if !on {
            return card.into_any_element();
        }

        let fold = motion::follow(
            &format!("{key}-footer"),
            if footer_open { 1. } else { 0. },
            DISCLOSURE,
        );
        card.child(token_row(&key, &state, &t, cx))
            .child(address_row(&key, &state, &self.domain, &t, window, cx))
            .child(cadence_row(&key, &state, &t, cx))
            .child(zone_row(&key, &state, &t, cx))
            .when(fold > 0.001, |card| {
                card.child(footer(&key, &state, fold, self.on_save.clone(), &t, cx))
            })
            .into_any_element()
    }

    /// The card's header (Pink: accounts.rs `render_provider_card` header):
    /// the mark, the Title3 name and a Connected badge over the summary, and the
    /// switch. Off dims the mark and the name and says why nothing syncs.
    fn header(
        &self,
        key: &str,
        state: &Entity<SettingsState>,
        on: bool,
        saving: bool,
        t: &Pink,
    ) -> Div {
        let tint = pink::project_tint((loaders::fnv1a(self.service.as_bytes()) % 6) as usize);
        let title_line = div()
            .w_full()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x(px(space::S8))
            .gap_y(px(space::S2))
            .child(
                div()
                    .flex_none()
                    .size(px(MARK_SLOT))
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!on, |el| el.opacity(0.55))
                    .child(pink::monogram_tile(tint, &self.monogram, MARK_SLOT, t)),
            )
            .child(
                div()
                    .type_style(TextStyle::Title3)
                    .text_color(if on { t.text } else { t.readable_muted(0.85) })
                    .child(self.service.clone()),
            )
            // Pink's active badge while on; off says so only through the
            // dimmed mark and name and the line below.
            .when(on, |line| {
                line.child(pink::badge("Connected", Tone::Success, t).line_height(px(14.)))
            });
        let description: Option<SharedString> = if !on {
            Some("Turned off. Nothing syncs until you turn it back on.".into())
        } else if self.summary.is_empty() {
            None
        } else {
            Some(self.summary.clone())
        };

        let id = format!("{key}-enabled");
        let label = format!("Enable {}", self.service);
        let switch = if saving {
            pink::switch_control_disabled(id, label, on, "Saving changes", t).into_any_element()
        } else {
            let state = state.clone();
            let on_save = self.on_save.clone();
            pink::switch_control(id, label, on, t)
                .on_click(move |_, window, cx| {
                    let values = state.update(cx, |s, cx| s.set_enabled(!s.saved.enabled, cx));
                    if let Some(f) = &on_save {
                        f(&values, window, cx);
                    }
                })
                .into_any_element()
        };

        div()
            .w_full()
            .px(px(layout::ROW_PAD_X))
            .pt(px(14.))
            .pb(px(12.))
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x(px(space::S12))
            .gap_y(px(space::S8))
            .child(
                div()
                    .flex_1()
                    .min_w(px(layout::ROW_TEXT_MIN))
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .child(title_line)
                    .when_some(description, |column, copy| {
                        column.child(pink::row_description(copy, t))
                    }),
            )
            .child(pink::row_controls().child(switch))
    }
}

/// The quiet line under a row title (Pink: settings/widgets.rs `meta_line`):
/// 12 in `readable_muted(0.8)`, fragments split by a faint "·".
fn meta_line(fragments: Vec<AnyElement>, t: &Pink) -> Div {
    let dot = if t.dark {
        t.text_muted.opacity(0.3)
    } else {
        t.text_muted
    };
    let mut line = div()
        .min_w_0()
        .mt(px(space::TEXT_STACK_GAP))
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(px(space::S8))
        .gap_y(px(space::S2))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(t.readable_muted(0.8));
    for (ix, fragment) in fragments.into_iter().enumerate() {
        if ix > 0 {
            line = line.child(div().text_color(dot).child("·"));
        }
        line = line.child(div().min_w_0().flex_shrink(1.0).child(fragment));
    }
    line
}

/// The access token (Pink: settings/devices.rs `device_row`'s id): a mono
/// 11 fragment under the title that reads "Copied" for a moment after Copy,
/// with show/hide and copy icon buttons and a Ghost Rotate that reads
/// "Rotating…" while it runs.
fn token_row(key: &str, state: &Entity<SettingsState>, t: &Pink, cx: &App) -> Div {
    let s = state.read(cx);
    let rotating = s.rotating.is_some();
    let copied = s.copied.is_some();
    let revealed = s.revealed;
    let rotations = s.rotations;
    let shown: SharedString = if copied {
        "Copied".into()
    } else if revealed {
        s.token.clone()
    } else {
        masked(&s.prefix, &s.token)
    };
    let token = div()
        .font_family(pink::MONO)
        .text_size(px(11.))
        .text_color(if copied {
            t.readable_tone(t.success_muted, 0.9)
        } else {
            t.readable_muted(0.8)
        })
        .child(shown);
    // A new token settles in like a status label.
    let token = if rotations > 0 {
        motion::status_swap(
            SharedString::from(format!("{key}-token-{rotations}")),
            token,
        )
        .into_any_element()
    } else {
        token.into_any_element()
    };

    let reveal = {
        let state = state.clone();
        let label = if revealed { "Hide token" } else { "Show token" };
        pink::icon_button(
            format!("{key}-reveal"),
            if revealed { ICON_EYE_CLOSED } else { ICON_EYE },
            ControlSize::Sm,
            t,
        )
        .aria_label(label)
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
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
            if copied { ICON_COPY_CHECK } else { ICON_COPY },
            ControlSize::Sm,
            t,
        )
        .aria_label(if copied { "Token copied" } else { "Copy token" })
        .tooltip(pink::text_tooltip(if copied {
            "Copied"
        } else {
            "Copy token"
        }))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.copy(cx)))
    };
    let rotate = {
        let state = state.clone();
        pink::button_enabled(
            format!("{key}-rotate"),
            ButtonKind::Ghost,
            ControlSize::Sm,
            !rotating,
            t,
        )
        .aria_label("Rotate token")
        .child(pink::glyph(ICON_REFRESH, ControlSize::Sm))
        .child(if rotating { "Rotating…" } else { "Rotate" })
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.rotate(cx)))
    };

    let description = if rotations > 0 {
        "Rotated just now. The old token stops working in 24 hours."
    } else {
        "Signs every request. Rotate it if it ever leaks."
    };
    pink::card_row(false, t)
        .child(
            pink::row_text("Access token", Some(description.into()), t)
                .child(meta_line(vec![token], t)),
        )
        .child(pink::row_controls().child(reveal).child(copy).child(rotate))
}

/// The workspace address (Pink connections.rs "Nearby address" in the
/// automation editor's field frame): the editable part, then the fixed
/// domain in the hint tone. The description says where the team signs in,
/// or, once the address can't be used, why (Pink's field notes in the
/// danger tone).
fn address_row(
    key: &str,
    state: &Entity<SettingsState>,
    domain: &SharedString,
    t: &Pink,
    window: &Window,
    cx: &App,
) -> Div {
    let s = state.read(cx);
    let input = s.input.clone();
    let focused = input.read(cx).is_focused(window);
    let subdomain = s.draft.subdomain.clone();
    let changed = s.draft.subdomain != s.saved.subdomain;
    // Only complain once the address was edited, or when it can't be saved anyway.
    let error = validate(&subdomain).filter(|_| changed || !focused);
    let description = match error {
        Some(reason) => {
            pink::row_description(reason, t).text_color(t.readable_tone(t.danger_muted, 1.0))
        }
        None => pink::row_description(
            format!("Your team signs in at https://{subdomain}{domain}"),
            t,
        ),
    };

    let focus = input.clone();
    pink::card_row(false, t)
        .child(
            div()
                .flex_1()
                .min_w(px(layout::ROW_TEXT_MIN))
                .flex()
                .flex_col()
                .gap(px(space::S2))
                .child(pink::row_title("Workspace address", t))
                .child(description),
        )
        .child(
            pink::row_controls().child(
                pink::input_frame(t)
                    .id(SharedString::from(format!("{key}-address")))
                    .w(px(ADDRESS_W))
                    .flex()
                    .items_center()
                    .gap(px(space::S2))
                    .line_height(px(16.))
                    .overflow_hidden()
                    .cursor_text()
                    .on_click(move |_, window, cx| TextInput::focus(&focus, window, cx))
                    .child(div().flex_1().min_w_0().child(input))
                    .child(
                        div()
                            .flex_none()
                            .text_color(t.text_faint)
                            .child(domain.clone()),
                    ),
            ),
        )
}

/// The sync cadence (Pink provider_defaults.rs `choice_row`): a Md
/// segmented control sized from its widest label; the description explains
/// the chosen cadence.
fn cadence_row(key: &str, state: &Entity<SettingsState>, t: &Pink, cx: &App) -> Div {
    let current = state.read(cx).draft.cadence.min(CADENCES.len() - 1);
    let labels: Vec<&str> = CADENCES.iter().map(|(label, _)| *label).collect();
    let segments = labels
        .iter()
        .enumerate()
        .map(|(ix, label)| Segment::new(ix.to_string(), *label))
        .collect();
    let state = state.clone();
    pink::card_row(false, t)
        .child(pink::row_text(
            "Sync cadence",
            Some(CADENCES[current].1.into()),
            t,
        ))
        .child(pink::row_controls().child(pink::segmented(
            format!("{key}-cadence"),
            segments,
            current,
            segmented_width(&labels),
            ControlSize::Md,
            t,
            move |ix, _, cx| {
                state.update(cx, |s, cx| {
                    s.draft.cadence = *ix;
                    cx.notify();
                })
            },
        )))
}

/// The time zone (Pink provider_defaults.rs `select_menu`): a 220 pop-up
/// button whose 260 menu hangs right-aligned under it, each row the city
/// over its offset with the current one checked.
fn zone_row(key: &str, state: &Entity<SettingsState>, t: &Pink, cx: &App) -> Div {
    let s = state.read(cx);
    let zone = s.draft.zone.min(ZONES.len() - 1);
    let (open, closing) = (s.menu_open, s.menu_closing);

    let mut trigger = pink::select(format!("{key}-zone"), ZONES[zone], ControlSize::Md, open, t)
        .aria_label("Time zone")
        .relative()
        .w(px(SELECT_W))
        .max_w_full()
        .on_mouse_down(MouseButton::Left, {
            let state = state.clone();
            move |_, _, cx| state.update(cx, |s, _| s.menu_pressed_open = s.menu_mounted())
        })
        .on_click({
            let state = state.clone();
            move |event: &ClickEvent, _, cx| {
                state.update(cx, |s, cx| {
                    // GPUI's keyboard click has no press note, so it toggles.
                    let was_open = std::mem::take(&mut s.menu_pressed_open);
                    if event.is_keyboard() && s.menu_open {
                        s.close_menu(cx);
                    } else if !was_open {
                        s.open_menu();
                    }
                    cx.notify();
                })
            }
        })
        .on_key_down(pink::swallow_activation_key);

    if open || closing.is_some() {
        let rows: Vec<AnyElement> = ZONES
            .iter()
            .enumerate()
            .map(|(ix, entry)| {
                let active = ix == zone;
                let (offset, city) = entry.split_once(" · ").unwrap_or((entry, ""));
                let state = state.clone();
                pink::menu_row(format!("{key}-zone-row-{ix}"), active, t)
                    .role(Role::Button)
                    .aria_label(*entry)
                    .aria_toggled(if active {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .tab_index(0)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.close_menu(cx);
                            s.draft.zone = ix;
                            cx.notify();
                        })
                    })
                    .on_key_down(pink::swallow_activation_key)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().min_w_0().truncate().child(city.to_string()))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .text_color(t.readable_muted(0.8))
                                    .child(offset.to_string()),
                            ),
                    )
                    .when(active, |row| {
                        row.child(pink::line_glyph(
                            pink::glyphs::CHECK,
                            14.,
                            t.selected_row_icon(t.accent),
                        ))
                    })
                    .into_any_element()
            })
            .collect();
        let card = pink::popover_card(t)
            .w(px(MENU_W))
            .on_mouse_down_out({
                let state = state.clone();
                move |_, _, cx| state.update(cx, |s, cx| s.close_menu(cx))
            })
            .flex()
            .flex_col()
            .child(
                // A long list scrolls inside the card instead of running off
                // the window.
                div()
                    .id(SharedString::from(format!("{key}-zone-list")))
                    .max_h(px(MENU_MAX_H))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(layout::MENU_GAP))
                    .children(rows),
            );
        let exit = closing.map(|since| {
            if motion::reduced_motion(cx) {
                1.
            } else {
                motion::menu_exit_progress(since)
            }
        });
        trigger = trigger.child(anchored_menu_below_end(
            SharedString::from(format!("{key}-zone-menu")),
            exit,
            card,
        ));
    }

    pink::card_row(false, t)
        .child(pink::row_text(
            "Time zone",
            Some("Nightly batches and report dates use it.".into()),
            t,
        ))
        .child(pink::row_controls().child(trigger))
}

/// A menu hung from the trigger's bottom-right corner, 6 pt below it and
/// kept 8 pt inside the window (Pink popover.rs `anchored_menu_below_end`).
/// It drops 6 pt into place on [`motion::MENU_IN`]; while `exit` runs it
/// fades on [`MENU_OUT`] under an occluder, so the dying rows take no clicks.
fn anchored_menu_below_end(
    id: SharedString,
    exit: Option<f32>,
    card: impl IntoElement,
) -> impl IntoElement {
    let inner = div()
        .occlude()
        .pt(px(layout::POPOVER_GAP))
        .child(div().relative().child(press_guard()).child(card));
    let layer = match exit {
        Some(t) => motion::menu_out(
            SharedString::from(format!("{id}-out")),
            t,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(id, MENU_RISE_DOWN, inner).into_any_element(),
    };
    div().absolute().bottom_0().right_0().size_0().child(
        deferred(
            anchored()
                .anchor(Anchor::TopRight)
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(layer),
        )
        .priority(1),
    )
}

/// Swallows the press that dismisses the menu: the card closes on it, and
/// nothing behind the menu (the trigger included) acts on it too (Pink
/// `frosted_menu_rounded`'s guard).
fn press_guard() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && !bounds.contains(&event.position) {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

/// The footer that holds the rows' edits (Pink: automations.rs editor
/// footer, a `fill(Quaternary)` card row): the status on the left (what
/// Save needs, "Saving…", then a success check with "Saved"), Discard and
/// Save on the right. It folds open and shut over `DISCLOSURE`.
fn footer(
    key: &str,
    state: &Entity<SettingsState>,
    fold: f32,
    on_save: Option<SaveHandler>,
    t: &Pink,
    cx: &App,
) -> Div {
    let s = state.read(cx);
    let invalid = validate(&s.draft.subdomain).is_some();
    let status = s.save;
    let dirty = s.dirty();
    let saved = !dirty && matches!(status, SaveStatus::Saved | SaveStatus::Leaving);

    let (copy, tone) = if status == SaveStatus::Saving {
        ("Saving…", None)
    } else if saved {
        ("Saved", None)
    } else if invalid {
        (
            "Fix the workspace address to save.",
            Some(t.readable_tone(t.danger_muted, 1.0)),
        )
    } else {
        ("Changes apply when you save.", None)
    };
    let line = div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(space::S6))
        .when(saved, |line| {
            // Pink accounts.rs update message: a 14 check, 1 pt down.
            line.child(div().flex_none().mt(px(1.)).child(pink::line_glyph(
                pink::glyphs::CHECK,
                14.,
                t.readable_tone(t.success, 1.0),
            )))
        })
        .child(pink::row_description(copy, t).when_some(tone, |line, tone| line.text_color(tone)));
    let status_line = motion::status_swap(loaders::label_id(&format!("{key}-status"), copy), line);

    let buttons = (!saved).then(|| {
        let busy = status == SaveStatus::Saving;
        let discard = {
            let state = state.clone();
            pink::button_enabled(
                format!("{key}-discard"),
                ButtonKind::Ghost,
                ControlSize::Md,
                !busy,
                t,
            )
            .aria_label("Discard changes")
            .child("Discard")
            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.discard(cx)))
        };
        let commit = {
            let state = state.clone();
            pink::button_enabled(
                format!("{key}-save"),
                ButtonKind::Primary,
                ControlSize::Md,
                !busy && !invalid,
                t,
            )
            .aria_label("Save changes")
            .child("Save")
            .on_click(move |_, window, cx| {
                let s = state.read(cx);
                let values = s.can_save().then(|| s.draft.clone());
                if let Some(values) = values {
                    if let Some(f) = &on_save {
                        f(&values, window, cx);
                    }
                    state.update(cx, |s, cx| s.save(cx));
                }
            })
        };
        pink::row_controls().child(discard).child(commit)
    });

    div().h(px(FOOTER_H * fold)).overflow_hidden().child(
        pink::card_row(false, t)
            .h(px(FOOTER_H))
            .bg(t.fill(FillTier::Quaternary))
            .child(
                div()
                    .flex_1()
                    .min_w(px(layout::ROW_TEXT_MIN))
                    .child(status_line),
            )
            .children(buttons),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .flex()
            .flex_col()
            .gap(px(layout::SECTION_GAP))
            .child(
                IntegrationSettings::new("integration-settings", "Ledgerly", "LG")
                    .summary("Last sync 4 min ago · 1,284 invoices mirrored")
                    .domain(".ledgerly.app")
                    .values(SettingsValues {
                        enabled: true,
                        subdomain: "northwind-books".into(),
                        cadence: 1,
                        zone: 3,
                    }),
            )
            .child(
                IntegrationSettings::new("integration-settings-off", "Parcelport", "PP")
                    .summary("Labels and tracking for 3 carriers")
                    .domain(".parcelport.io")
                    .values(SettingsValues {
                        enabled: false,
                        subdomain: "northwind".into(),
                        cadence: 2,
                        zone: 1,
                    }),
            )
    })
}
