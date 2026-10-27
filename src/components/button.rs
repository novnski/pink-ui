//! Button – Pink's push buttons: Primary, Secondary, Ghost and Danger in three
//! sizes, with glyphs, icon-only squares and a busy spinner.
//!
//! A Pink component, built on the kit's control frame (Pink: `button.rs`
//! `button` / `icon_button` / `glyph`). Sizes follow Pink's control ladder:
//! Sm is 24 high with 11/14 type, radius 6 and 12 px glyphs, for toolbars and
//! inline row actions; Md is 28 high (12/16, radius 8, 14 px glyphs), for the
//! settings control column and panel headers; Lg is 32 high (13/18, radius 8,
//! 16 px glyphs), for dialogs.
//!
//! Primary is the one solid, `text`-colored plate in a group. Secondary is a
//! faint plate with an edge, Ghost a faint plate whose muted label brightens on
//! hover, and Danger the solid red plate. Hovers ease over 150 ms, keyboard
//! focus draws the 2 px inset ring, and a disabled button rests at 45%. An
//! icon-only Ghost button is Pink's square icon button: transparent until
//! hovered, with the glyph brightening alongside the wash. While `loading`,
//! Pink's gradient spinner takes the leading glyph's place. The button then
//! stops taking clicks but keeps full strength (Pink `goal_control`).
//!
//! ```ignore
//! Button::new("save").label("Save changes").on_click(|_, _, _| {});
//! Button::new("check")
//!     .secondary()
//!     .icon(button::icon("refresh"))
//!     .label(if checking { "Checking…" } else { "Check for updates" })
//!     .loading(checking);
//! ```

use std::{collections::HashMap, rc::Rc, time::Duration};

use gpui::{
    AnyView, App, ClickEvent, Context, Div, ElementId, Hsla, IntoElement, SharedString, Task,
    Window, div, prelude::*, px,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink, layout, loaders},
    story,
};

pub const NAME: &str = "Button";
pub const DESCRIPTION: &str = "Primary, secondary, ghost and danger push buttons in three sizes, with glyphs, icon-only squares and a busy spinner";

/// The demo's glyphs, copied from Pink's Solar Linear set: `refresh`, `copy`,
/// `copy-check`, `more-horizontal` and `arrow-right-up`.
pub fn icon(name: &str) -> SharedString {
    format!("icons/button/{name}.svg").into()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// The one solid action in a group: a `text`-colored plate (easing to 86%
    /// on hover) with an `on_solid` label (Pink `ButtonKind::Primary`).
    #[default]
    Primary,
    /// A faint filled plate with a `separator(true)` edge and a `text` label
    /// (Pink `ButtonKind::Secondary`).
    Secondary,
    /// A quiet action on a faint resting plate; the muted label brightens to
    /// `text` on hover (Pink `ButtonKind::Ghost`). Icon-only, it is Pink's
    /// transparent `icon_button`.
    Ghost,
    /// The solid red plate for deleting (Pink `ButtonKind::Danger`).
    Destructive,
}

impl ButtonVariant {
    fn kind(self) -> ButtonKind {
        match self {
            ButtonVariant::Primary => ButtonKind::Primary,
            ButtonVariant::Secondary => ButtonKind::Secondary,
            ButtonVariant::Ghost => ButtonKind::Ghost,
            ButtonVariant::Destructive => ButtonKind::Danger,
        }
    }

    /// The glyph tone (Pink `button::glyph`): the shared `readable_muted(0.95)`
    /// on the faint plates, the label color on the solid ones. It doesn't
    /// follow the hover; only the icon button's glyph does.
    fn glyph_tone(self, t: &Pink) -> Hsla {
        match self {
            ButtonVariant::Primary => t.on_solid,
            ButtonVariant::Destructive => t.on_danger(),
            ButtonVariant::Secondary | ButtonVariant::Ghost => t.glyph_tone(),
        }
    }

    /// The spinner's row tints: the accent glyph palette on the faint plates
    /// (Pink `gradient_spinner`), a single label-colored tint on the solid
    /// plates, where violet would sink into the fill.
    fn spinner_tints(self, t: &Pink) -> [Hsla; 3] {
        match self {
            ButtonVariant::Primary => [t.on_solid; 3],
            ButtonVariant::Destructive => [t.on_danger(); 3],
            ButtonVariant::Secondary | ButtonVariant::Ghost => t.glyph,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonSize {
    /// 24 high, 11/14 type, 12 px glyphs: toolbars and inline row actions.
    Sm,
    /// 28 high, 12/16 type, 14 px glyphs: the settings control column.
    #[default]
    Md,
    /// 32 high, 13/18 type, 16 px glyphs: dialogs.
    Lg,
}

impl ButtonSize {
    fn control(self) -> ControlSize {
        match self {
            ButtonSize::Sm => ControlSize::Sm,
            ButtonSize::Md => ControlSize::Md,
            ButtonSize::Lg => ControlSize::Lg,
        }
    }

    /// The spinner's cell: 2.5 in a 12–14 px glyph box (Pink
    /// `composer/goals.rs` `SPINNER_CELL`), 3 in the 16 px one (Pink's
    /// sign-in poll spinner).
    fn spinner_cell(self) -> f32 {
        match self {
            ButtonSize::Sm | ButtonSize::Md => 2.5,
            ButtonSize::Lg => 3.0,
        }
    }
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: Option<SharedString>,
    variant: ButtonVariant,
    size: ButtonSize,
    icon: Option<SharedString>,
    icon_right: Option<SharedString>,
    tooltip: Option<SharedString>,
    loading: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            label: None,
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            icon: None,
            icon_right: None,
            tooltip: None,
            loading: false,
            disabled: false,
            on_click: None,
        }
    }

    /// The text; leave it out for an icon-only square button.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn destructive(self) -> Self {
        self.variant(ButtonVariant::Destructive)
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// An SVG under `assets/` before the label, sized for the button (see
    /// [`icon`] for the demo's set).
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// An SVG under `assets/` after the label, like Pink's "Go to chat ↗".
    pub fn icon_right(mut self, path: impl Into<SharedString>) -> Self {
        self.icon_right = Some(path.into());
        self
    }

    /// Pink's text tooltip after the shared 400 ms delay; it also names the
    /// button for assistive tech when there is no label. Icon-only buttons
    /// should always have one.
    pub fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Swaps the leading glyph for Pink's gradient spinner and stops taking
    /// clicks, at full strength. Pair it with a busy label ("Checking…").
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Rests the button at 45% with no hover, pointer, tab stop or clicks.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

/// Pink's gradient spinner centered in a glyph-sized box, so it takes the
/// glyph's place without moving the label (Pink: `composer/goals.rs`
/// `goal_control`, spinning).
fn busy_glyph(variant: ButtonVariant, size: ButtonSize, t: &Pink) -> Div {
    div()
        .flex_none()
        .size(px(size.control().glyph()))
        .flex()
        .items_center()
        .justify_center()
        .child(loaders::gradient_spinner(
            size.spinner_cell(),
            variant.spinner_tints(t),
        ))
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let size = self.size.control();
        let id = SharedString::from(self.id.to_string());
        let square = self.label.is_none();
        let enabled = !self.disabled && !self.loading;
        let name = self.label.clone().or(self.tooltip.clone());

        let el = if square && self.variant == ButtonVariant::Ghost && !self.loading {
            // Pink: button.rs icon_button
            let glyph = self.icon.or(self.icon_right).unwrap_or_default();
            pink::icon_button_enabled(id, glyph, size, !self.disabled, &t)
        } else {
            // Pink: button.rs button / button_keyed
            let tone = self.variant.glyph_tone(&t);
            let leading = if self.loading {
                Some(busy_glyph(self.variant, self.size, &t).into_any_element())
            } else {
                self.icon
                    .map(|path| pink::glyph(path, size).text_color(tone).into_any_element())
            };
            // In a square the spinner stands in for whichever glyph it had.
            let trailing = self
                .icon_right
                .filter(|_| !(square && self.loading))
                .map(|path| pink::glyph(path, size).text_color(tone));
            pink::button_enabled(id, self.variant.kind(), size, enabled, &t)
                // The control that is working keeps full strength for its spinner.
                .when(self.loading && !self.disabled, |el| el.opacity(1.))
                .when(square, |el| el.w(px(size.height())).px(px(0.)))
                // A busy icon-only Ghost stays the transparent icon button.
                .when(square && self.variant == ButtonVariant::Ghost, |el| {
                    el.bg(gpui::transparent_black())
                })
                .children(leading)
                .children(self.label)
                .children(trailing)
        };

        motion::keep_alive(window);
        el.when_some(name, |el, name| el.aria_label(name))
            .when_some(self.tooltip, |el, tip| {
                el.tooltip(pink::text_tooltip(tip))
                    .tooltip_show_delay(layout::TOOLTIP_DELAY)
            })
            .when_some(self.on_click.filter(|_| enabled), |el, handler| {
                el.on_click(move |event, window, cx| handler(event, window, cx))
            })
    }
}

// ---------------------------------------------------------------------------
// Story: a settings page and a confirmation dialog
// ---------------------------------------------------------------------------

/// A release check against GitHub.
const CHECK_MS: u64 = 1800;
const DOWNLOAD_MS: u64 = 2600;
/// Pink holds a "Copied" confirmation for 1.5 s (`settings/devices.rs`).
const COPIED_MS: u64 = 1500;
const DELETE_MS: u64 = 1600;
/// The settings column holding the two cards.
const PAGE_W: f32 = 500.;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Check {
    #[default]
    Idle,
    Running,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Update {
    #[default]
    Ready,
    Downloading,
    Downloaded,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Removal {
    #[default]
    Confirm,
    Deleting,
    Deleted,
}

#[derive(Default)]
struct Demo {
    check: Check,
    update: Update,
    copied: bool,
    removal: Removal,
    tasks: HashMap<&'static str, Task<()>>,
}

impl Demo {
    /// Runs `done` after `ms`, replacing any timer pending under `key`.
    fn after(
        &mut self,
        key: &'static str,
        ms: u64,
        cx: &mut Context<Self>,
        done: impl FnOnce(&mut Self) + 'static,
    ) {
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            this.update(cx, |demo, cx| {
                done(demo);
                cx.notify();
            })
            .ok();
        });
        self.tasks.insert(key, task);
        cx.notify();
    }

    fn check(&mut self, cx: &mut Context<Self>) {
        if self.check == Check::Running {
            return;
        }
        self.check = Check::Running;
        self.after("check", CHECK_MS, cx, |demo| demo.check = Check::Done);
    }

    fn apply(&mut self, cx: &mut Context<Self>) {
        match self.update {
            Update::Ready => {
                self.update = Update::Downloading;
                self.after("update", DOWNLOAD_MS, cx, |demo| {
                    demo.update = Update::Downloaded
                });
            }
            Update::Downloading => {}
            // "Restarting" brings the demo back to the start.
            Update::Downloaded => {
                self.update = Update::Ready;
                self.check = Check::Idle;
                cx.notify();
            }
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        self.copied = true;
        self.after("copy", COPIED_MS, cx, |demo| demo.copied = false);
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        if self.removal != Removal::Confirm {
            return;
        }
        self.removal = Removal::Deleting;
        self.after("delete", DELETE_MS, cx, |demo| {
            demo.removal = Removal::Deleted
        });
    }

    fn reset_removal(&mut self, cx: &mut Context<Self>) {
        self.removal = Removal::Confirm;
        cx.notify();
    }
}

/// A settings row: the title and description on the left, the buttons in
/// Pink's control column on the right.
fn row(title: &'static str, description: &'static str, buttons: Vec<Button>, t: &Pink) -> Div {
    pink::card_row(false, t)
        .child(pink::row_text(title, Some(description.into()), t))
        .child(pink::row_controls().children(buttons))
}

fn showcase(window: &mut Window, cx: &mut App) -> Div {
    let t = Pink::get(cx);
    let demo = window.use_keyed_state("button-demo", cx, |_, _| Demo::default());
    let (check, update, copied, removal) = {
        let d = demo.read(cx);
        (d.check, d.update, d.copied, d.removal)
    };
    let act = |action: fn(&mut Demo, &mut Context<Demo>)| {
        let demo = demo.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| demo.update(cx, action)
    };
    let checking = check == Check::Running;

    // Md: the settings control column (Pink's Updates page).
    let updates = pink::section_card(&t)
        .child(pink::section_header(
            "Updates",
            Some("PINK looks for new releases on GitHub.".into()),
            &t,
        ))
        .child(row(
            "PINK 0.42.1",
            match check {
                Check::Idle => "Last checked 2 hours ago",
                Check::Running => "Checking GitHub Releases…",
                Check::Done => "Last checked just now",
            },
            vec![
                Button::new("updates-check")
                    .secondary()
                    .icon(icon("refresh"))
                    .label(if checking {
                        "Checking…"
                    } else {
                        "Check for updates"
                    })
                    .loading(checking)
                    .on_click(act(Demo::check)),
            ],
            &t,
        ))
        .child(row(
            "PINK 0.43.0",
            match update {
                Update::Ready => "Faster board loading and quieter notifications.",
                Update::Downloading => "Downloading 38 MB…",
                Update::Downloaded => "Restart PINK to finish updating.",
            },
            vec![
                // Stays in place, disabled, while a check runs.
                Button::new("updates-apply")
                    .label(match update {
                        Update::Ready => "Update and restart",
                        Update::Downloading => "Downloading…",
                        Update::Downloaded => "Restart to update",
                    })
                    .loading(update == Update::Downloading)
                    .disabled(checking)
                    .on_click(act(Demo::apply)),
            ],
            &t,
        ));

    // Sm: inline row actions.
    let sm = ButtonSize::Sm;
    let session = pink::section_card(&t)
        .child(pink::section_header(
            "Session",
            Some("Fix flaky login test, 14 turns".into()),
            &t,
        ))
        .child(row(
            "Share link",
            "Anyone with the link can read the transcript.",
            vec![
                Button::new("session-copy-link")
                    .secondary()
                    .size(sm)
                    .icon(icon(if copied { "copy-check" } else { "copy" }))
                    .label(if copied { "Copied" } else { "Copy link" })
                    .on_click(act(Demo::copy)),
                Button::new("session-share-more")
                    .ghost()
                    .size(sm)
                    .icon(icon("more-horizontal"))
                    .tooltip("More sharing options"),
            ],
            &t,
        ))
        .child(row(
            "Linked issue",
            "#214: Login test times out on CI",
            vec![
                Button::new("session-open-issue")
                    .ghost()
                    .size(sm)
                    .icon(icon("arrow-right-up"))
                    .label("Open on GitHub"),
            ],
            &t,
        ))
        .child(row(
            "Parent chat",
            "Split from “Stabilize the CI pipeline”.",
            vec![
                Button::new("session-go-to-chat")
                    .secondary()
                    .size(sm)
                    .label("Go to chat")
                    .icon_right(icon("arrow-right-up")),
            ],
            &t,
        ))
        .child(row(
            "Export transcript",
            "Available once the session finishes.",
            vec![
                Button::new("session-export")
                    .secondary()
                    .size(sm)
                    .label("Export")
                    .disabled(true),
            ],
            &t,
        ));

    // Lg: a dialog's button row (Pink `popover::btn_ghost` / `btn_danger`).
    let lg = ButtonSize::Lg;
    let (title, body, buttons) = match removal {
        Removal::Confirm | Removal::Deleting => (
            "Delete session?",
            "“Fix flaky login test” and its PINK-owned local records will be permanently deleted. This can’t be undone.",
            vec![
                Button::new("dialog-cancel")
                    .ghost()
                    .size(lg)
                    .label("Cancel")
                    .disabled(removal == Removal::Deleting)
                    .on_click(act(Demo::reset_removal)),
                Button::new("dialog-delete")
                    .destructive()
                    .size(lg)
                    .label(if removal == Removal::Deleting {
                        "Deleting…"
                    } else {
                        "Delete"
                    })
                    .loading(removal == Removal::Deleting)
                    .on_click(act(Demo::delete)),
            ],
        ),
        Removal::Deleted => (
            "Session deleted",
            "“Fix flaky login test” was removed from this Mac.",
            vec![
                Button::new("dialog-done")
                    .size(lg)
                    .label("Done")
                    .on_click(act(Demo::reset_removal)),
            ],
        ),
    };
    let dialog = pink::dialog_card(&t)
        .child(pink::dialog_title(title, &t))
        .child(div().mt(px(6.)).child(pink::dialog_body(body, &t)))
        .child(
            div()
                .mt(px(16.))
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(8.))
                .children(buttons),
        );

    div()
        .font_family(pink::SANS)
        .text_color(t.text)
        .flex()
        .flex_row()
        .items_start()
        .gap(px(layout::SECTION_GAP))
        .child(
            div()
                .w(px(PAGE_W))
                .flex()
                .flex_col()
                .child(updates)
                .child(session),
        )
        .child(div().mt(px(layout::SECTION_GAP)).child(dialog))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| showcase(window, cx))
}
