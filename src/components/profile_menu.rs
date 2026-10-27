//! Profile Menu – the account menu: the signed-in identity, the account
//! actions and the profile setup progress, on Pink's popover card.
//!
//! The card is Pink's menu card (`popover.rs` `popover_card`, 280 pt wide).
//! The identity at the top is the avatar and identity text of Pink's
//! Settings > Accounts row; the actions are Pink menu rows with one
//! highlight that the pointer and the arrow keys share (`menu_row_nav`),
//! and a "New" pill is Pink's model-picker badge. Sign out sits in its own
//! section after a full-bleed hairline, as Pink's Settings footer sets it
//! off. The setup block closing the card counts the finished steps over the
//! 5 pt meter of Pink's account usage meter, and its next step is one more
//! row of the same list.
//!
//! ↑ and ↓ move the highlight (wrapping), Home and End jump to the ends,
//! Enter or Space runs the row under it and Esc dismisses. The card takes
//! focus when it appears, like a Pink menu opening. The caller anchors it;
//! the story hangs it above an account button, where it rises 6 pt into
//! place and fades out like Pink's `anchored_menu_above`.
//!
//! ```ignore
//! ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
//!     .setup(ProfileSetup::new(3, 5).next("Add a profile photo").on_continue(|_, _| {}))
//!     .on_select(|id, _, _| println!("{id}"))
//! ```

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnimationExt, AnyElement, AnyView, App, Div, ElementId, Entity, FocusHandle, FontWeight,
    IntoElement, KeyDownEvent, MouseButton, Role, SharedString, Stateful, Window, div, prelude::*,
    px, relative, svg,
};
use web_time::Instant;

use crate::{
    HoverFill,
    motion::{self, MENU_IN, MENU_RISE_UP},
    pink::{self, FillTier, Pink, TextStyle, TypeStyleExt, icon_size, layout, radius},
    story,
};

pub const NAME: &str = "Profile Menu";
pub const DESCRIPTION: &str = "The account menu above your name: who is signed in, the account actions on one highlight the pointer and arrow keys share, a New badge, Sign out set apart, and the profile setup count over a quiet meter with its next step";

// Solar Linear glyphs, as Pink ships them (`crates/ui/assets/icons`).
const ICON_SETTINGS: &str = "icons/profile-menu/settings.svg";
const ICON_DOCUMENT: &str = "icons/profile-menu/document.svg";
const ICON_WIDGET: &str = "icons/profile-menu/widget.svg";
const ICON_LOGOUT: &str = "icons/profile-menu/logout-2.svg";
const ICON_ARROW: &str = "icons/profile-menu/arrow-right.svg";

/// The identity avatar (Pink `settings/accounts.rs` `render_account_row`).
const AVATAR: f32 = 32.0;
/// The setup meter's track (Pink `settings/accounts.rs` `usage_meter`).
const METER_HEIGHT: f32 = 5.0;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type SelectHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
/// Runs the row at an index of the highlight list.
type Run = Rc<dyn Fn(usize, &mut Window, &mut App)>;

#[derive(Clone)]
pub struct ProfileMenuItem {
    pub id: SharedString,
    pub label: SharedString,
    /// A 16 pt leading glyph, an SVG path under `assets/`, tinted
    /// `text_muted`.
    pub icon: Option<SharedString>,
    /// A pill after the label (Pink's "New" badge).
    pub badge: Option<SharedString>,
    /// Opens a new section: a full-bleed hairline above the row.
    pub separated: bool,
    pub on_select: Option<Handler>,
}

impl ProfileMenuItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
            separated: false,
            on_select: None,
        }
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    /// Sets the row apart from the ones above it.
    pub fn separated(mut self) -> Self {
        self.separated = true;
        self
    }

    pub fn on_select(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

#[derive(Clone)]
pub struct ProfileSetup {
    /// Steps finished so far.
    pub done: u32,
    pub total: u32,
    /// The next open step, a row under the meter while steps remain.
    pub next: Option<SharedString>,
    pub on_continue: Option<Handler>,
}

impl ProfileSetup {
    pub fn new(done: u32, total: u32) -> Self {
        Self {
            done,
            total,
            next: None,
            on_continue: None,
        }
    }

    pub fn next(mut self, step: impl Into<SharedString>) -> Self {
        self.next = Some(step.into());
        self
    }

    pub fn on_continue(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_continue = Some(Rc::new(handler));
        self
    }
}

/// Settings, Docs, Apps (with a "New" badge) and, in its own section, Sign
/// out (Pink's Settings account action and its `logout-2` glyph).
pub fn default_items() -> Vec<ProfileMenuItem> {
    vec![
        ProfileMenuItem::new("settings", "Settings").icon(ICON_SETTINGS),
        ProfileMenuItem::new("docs", "Docs").icon(ICON_DOCUMENT),
        ProfileMenuItem::new("apps", "Apps")
            .icon(ICON_WIDGET)
            .badge("New"),
        ProfileMenuItem::new("logout", "Sign out")
            .icon(ICON_LOGOUT)
            .separated(),
    ]
}

/// The card's keyboard state (Pink `Popup` with its `menu_cursor`).
struct MenuState {
    /// The one highlight that the arrow keys and the pointer share: an index
    /// into the actions, then the setup's next step.
    cursor: Option<usize>,
    focus: FocusHandle,
    /// Focus the card on its first frame (Pink `pending_menu_focus`).
    pending_focus: bool,
}

#[derive(IntoElement)]
pub struct ProfileMenu {
    id: ElementId,
    name: SharedString,
    email: SharedString,
    items: Option<Vec<ProfileMenuItem>>,
    setup: Option<ProfileSetup>,
    on_select: Option<SelectHandler>,
    on_dismiss: Option<Handler>,
}

impl ProfileMenu {
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        email: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            email: email.into(),
            items: None,
            setup: None,
            on_select: None,
            on_dismiss: None,
        }
    }

    /// Replaces the default actions.
    pub fn items(mut self, items: Vec<ProfileMenuItem>) -> Self {
        self.items = Some(items);
        self
    }

    /// Profile setup progress closing the card – leave unset to hide it.
    pub fn setup(mut self, setup: ProfileSetup) -> Self {
        self.setup = Some(setup);
        self
    }

    /// Any action's id when it runs, after the item's own handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Esc, or a press outside the card: the host closes the menu.
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ProfileMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let items = Rc::new(self.items.unwrap_or_else(default_items));
        let setup = self.setup;
        let next_step = setup
            .as_ref()
            .filter(|s| s.done < s.total)
            .and_then(|s| s.next.clone());
        let count = items.len() + usize::from(next_step.is_some());

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| MenuState {
                cursor: None,
                focus: cx.focus_handle().tab_stop(true),
                pending_focus: true,
            },
        );
        let (due, focus, cursor) = state.update(cx, |s, _| {
            let due = std::mem::take(&mut s.pending_focus);
            (due, s.focus.clone(), s.cursor.filter(|&ix| ix < count))
        });
        if due {
            window.focus(&focus, cx);
        }

        let run: Run = {
            let items = items.clone();
            let on_select = self.on_select.clone();
            let on_continue = setup.as_ref().and_then(|s| s.on_continue.clone());
            Rc::new(move |ix, window, cx| match items.get(ix) {
                Some(item) => {
                    if let Some(f) = &item.on_select {
                        f(window, cx);
                    }
                    if let Some(f) = &on_select {
                        f(&item.id, window, cx);
                    }
                }
                None => {
                    if let Some(f) = &on_continue {
                        f(window, cx);
                    }
                }
            })
        };

        let mut children: Vec<AnyElement> = vec![
            identity(&key, &self.name, &self.email, &t).into_any_element(),
            pink::menu_separator(&t).into_any_element(),
        ];
        for (ix, item) in items.iter().enumerate() {
            if item.separated && ix > 0 {
                children.push(pink::menu_separator(&t).into_any_element());
            }
            children.push(
                nav_row(&key, &item.id, ix, cursor, &state, &run, &t)
                    .aria_label(item.label.clone())
                    .when_some(item.icon.clone(), |row, path| {
                        row.child(
                            svg()
                                .path(path)
                                .size(px(icon_size::MD))
                                .flex_none()
                                .text_color(t.text_muted),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(div().min_w_0().truncate().child(item.label.clone()))
                            .when_some(item.badge.clone(), |label, badge| {
                                label.child(new_badge(badge, &t))
                            }),
                    )
                    .into_any_element(),
            );
        }
        if let Some(setup) = &setup {
            children.push(pink::menu_separator(&t).into_any_element());
            children.push(setup_progress(&key, setup, &t).into_any_element());
            if let Some(step) = next_step {
                children.push(
                    nav_row(&key, "next", items.len(), cursor, &state, &run, &t)
                        .aria_label(SharedString::from(format!("Next: {step}")))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .flex_none()
                                        .text_color(t.readable_muted(0.85))
                                        .child("Next:"),
                                )
                                .child(div().min_w_0().truncate().child(step)),
                        )
                        .child(
                            svg()
                                .path(ICON_ARROW)
                                .size(px(icon_size::SM))
                                .flex_none()
                                .text_color(t.text_muted),
                        )
                        .into_any_element(),
                );
            }
        }

        // Pink `on_menu_key`: the arrows, Home and End move the shared
        // highlight, Enter or Space runs its row, Esc dismisses.
        let on_key = {
            let state = state.clone();
            let on_dismiss = self.on_dismiss.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let cursor = state.read(cx).cursor.filter(|&ix| ix < count);
                let moved = match event.keystroke.key.as_str() {
                    "down" => menu_step(cursor, count, 1),
                    "up" => menu_step(cursor, count, -1),
                    "home" => (count > 0).then_some(0),
                    "end" => count.checked_sub(1),
                    "enter" | "space" => {
                        if let Some(ix) = cursor {
                            run(ix, window, cx);
                        }
                        cx.stop_propagation();
                        return;
                    }
                    "escape" => {
                        match &on_dismiss {
                            Some(f) => f(window, cx),
                            None => state.update(cx, |s, cx| {
                                s.cursor = None;
                                cx.notify();
                            }),
                        }
                        cx.stop_propagation();
                        return;
                    }
                    _ => return,
                };
                state.update(cx, |s, cx| {
                    s.cursor = moved;
                    cx.notify();
                });
                cx.stop_propagation();
            }
        };

        pink::popover_card(&t)
            .id(SharedString::from(format!("{key}-card")))
            .role(Role::Menu)
            .aria_label(SharedString::from(format!("Account: {}", self.name)))
            .track_focus(&focus)
            .w(px(layout::MENU_W_M))
            .flex()
            .flex_col()
            .line_height(px(TextStyle::Body.line_height()))
            .on_key_down(on_key)
            .when_some(self.on_dismiss, |card, f| {
                card.on_mouse_down_out(move |_, window, cx| f(window, cx))
            })
            .children(children)
    }
}

/// One row of the highlight list (Pink `popover::menu_row_nav` without the
/// check column): the pointer moves the shared highlight onto it, and a
/// click runs it.
fn nav_row(
    key: &str,
    id: &str,
    ix: usize,
    cursor: Option<usize>,
    state: &Entity<MenuState>,
    run: &Run,
    t: &Pink,
) -> Stateful<Div> {
    let state = state.clone();
    let run = run.clone();
    pink::menu_row_nav(false, cursor == Some(ix), t)
        .pr(px(8.))
        .id(SharedString::from(format!("{key}-row-{id}")))
        .role(Role::MenuItem)
        .on_mouse_move(move |_, _, cx| {
            state.update(cx, |s, cx| {
                if s.cursor != Some(ix) {
                    s.cursor = Some(ix);
                    cx.notify();
                }
            })
        })
        .on_click(move |_, window, cx| run(ix, window, cx))
}

/// The signed-in identity (Pink `settings/accounts.rs` `render_account_row`
/// and `identity_text`): the initial avatar, then the name in 13/18 medium
/// over the email in 12/16 muted, each with its full text in a tooltip.
fn identity(key: &str, name: &SharedString, email: &SharedString, t: &Pink) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(8.))
        .py(px(8.))
        .child(avatar(initial(name), t))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    identity_text(format!("{key}-name"), name)
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text),
                )
                .child(
                    identity_text(format!("{key}-email"), email)
                        .type_style(TextStyle::Footnote)
                        .text_color(t.readable_muted(0.85)),
                ),
        )
}

fn identity_text(id: String, text: &SharedString) -> Stateful<Div> {
    div()
        .id(SharedString::from(id))
        .min_w_0()
        .truncate()
        .tooltip(pink::text_tooltip(text.clone()))
        .tooltip_show_delay(layout::TOOLTIP_DELAY)
        .child(text.clone())
}

/// Pink's account avatar: a 32 pt disc, 1 px `border`, the Quaternary fill
/// and the initial in footnote semibold at `readable_muted(0.9)`.
fn avatar(initial: SharedString, t: &Pink) -> Div {
    div()
        .flex_none()
        .size(px(AVATAR))
        .rounded_full()
        .border_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .flex()
        .items_center()
        .justify_center()
        .type_style(TextStyle::Footnote)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.readable_muted(0.9))
        .child(initial)
}

/// The avatar's letter: the name's first character, uppercased, or "?"
/// (Pink `render_account_row`).
fn initial(name: &str) -> SharedString {
    name.chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "?".into())
        .into()
}

/// Pink `pickers.rs` `new_model_badge`: 16 tall, `px 5`, radius 4, accent at
/// 0.14, 10.5/14 semibold in the floored accent.
fn new_badge(label: SharedString, t: &Pink) -> Div {
    div()
        .flex_none()
        .h(px(16.))
        .px(px(5.))
        .flex()
        .items_center()
        .rounded(px(radius::XS))
        .bg(t.accent.opacity(0.14))
        .text_size(px(10.5))
        .line_height(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.readable_tone(t.accent, 1.0))
        .child(label)
}

/// "Profile setup" and its count over the meter. The meter is Pink's
/// account usage track (5 pt capsule, Secondary fill) with the neutral fill
/// of Pink's usage meters, and glides to a new count over `EMPHASIZED`.
fn setup_progress(key: &str, setup: &ProfileSetup, t: &Pink) -> Div {
    let done = setup.done.min(setup.total);
    let ratio = if setup.total > 0 {
        done as f32 / setup.total as f32
    } else {
        0.
    };
    let fill = motion::follow(&format!("{key}-setup-fill"), ratio, motion::EMPHASIZED);
    div()
        .px(px(8.))
        .pt(px(6.))
        .pb(px(8.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .type_style(TextStyle::Footnote)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child("Profile setup"),
                )
                .child(
                    div()
                        .flex_none()
                        .type_style(TextStyle::Caption)
                        .font_features(pink::tabular_figures())
                        .text_color(t.readable_muted(0.85))
                        .child(format!("{done} of {}", setup.total)),
                ),
        )
        .child(
            div()
                .w_full()
                .h(px(METER_HEIGHT))
                .rounded_full()
                .overflow_hidden()
                .bg(t.fill(FillTier::Secondary))
                .when(fill > 0.001, |track| {
                    track.child(
                        div()
                            .h_full()
                            .w(relative(fill))
                            .rounded_full()
                            .bg(t.text_muted),
                    )
                }),
        )
}

/// Steps the highlight, wrapping at both ends; `None` enters at the end the
/// step points from (Pink `popover::menu_step`).
fn menu_step(active: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count = count as isize;
    let next = match active {
        None if delta >= 0 => 0,
        None => count - 1,
        Some(at) => (at as isize + delta).rem_euclid(count),
    };
    Some(next as usize)
}

/// The demo's setup steps; three are done when it starts.
const STEPS: [&str; 5] = [
    "Verify your email",
    "Choose a display name",
    "Pick a theme",
    "Add a profile photo",
    "Connect GitHub",
];

/// The demo's account button and its menu (Pink `Popup`).
struct Demo {
    open: bool,
    /// When the exit fade began; the card stays mounted until it ends.
    closing: Option<Instant>,
    /// The button press found the menu up, so its click leaves it closed
    /// (Pink `note_trigger_press`).
    pressed_while_open: bool,
    done: u32,
    trigger: FocusHandle,
}

impl Demo {
    fn mounted(&self) -> bool {
        self.open
            || self
                .closing
                .is_some_and(|since| motion::menu_exit_progress(since) < 1.0)
    }
}

/// Starts the exit fade and hands focus back to the account button.
fn close_demo(demo: &Entity<Demo>, window: &mut Window, cx: &mut App) {
    let trigger = demo.update(cx, |d, cx| {
        if !d.open {
            return None;
        }
        d.open = false;
        d.closing = Some(Instant::now());
        cx.notify();
        Some(d.trigger.clone())
    });
    if let Some(handle) = trigger {
        window.focus(&handle, cx);
    }
}

/// Hangs the menu above its trigger, 6 pt off it and shifted left by the
/// card inset so its row text lines up with the button's (Pink
/// `popover::anchored_menu_above_leading`). It rises 6 pt into place over
/// `MENU_IN`; while `exit` runs it fades over `MENU_OUT` under an occluder,
/// so the dying rows take no clicks (Pink `menu_motion_up`).
fn anchored_above(card: ProfileMenu, exit: Option<f32>, window: &mut Window) -> impl IntoElement {
    if exit.is_some() {
        window.request_animation_frame();
    }
    let fade = 1. - exit.unwrap_or(0.);
    div()
        .absolute()
        .bottom_full()
        .left(px(-layout::CARD_INSET))
        .pb(px(layout::POPOVER_GAP))
        .child(
            div()
                .relative()
                .child(card)
                .when(exit.is_some(), |layer| {
                    layer.child(div().absolute().inset_0().occlude())
                })
                .with_animation(
                    "profile-menu-demo-layer",
                    MENU_IN.animation(),
                    move |el, p| el.opacity(p * fade).top(px(MENU_RISE_UP * (1. - p))),
                ),
        )
}

/// An account button at the foot of a sidebar with the menu open above it:
/// actions close the menu, the next step completes in place and the meter
/// follows, Esc or a press outside closes it and the button reopens it.
pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("profile-menu-demo", cx, |_, cx| Demo {
            open: true,
            closing: None,
            pressed_while_open: false,
            done: 3,
            trigger: cx.focus_handle().tab_stop(true),
        });
        let t = Pink::get(cx);
        let (open, closing, done, trigger_focus) = {
            let d = demo.read(cx);
            (d.open, d.closing, d.done, d.trigger.clone())
        };
        let reduced = motion::reduced_motion(cx);
        let exit = closing
            .filter(|_| !open)
            .map(|since| {
                if reduced {
                    1.
                } else {
                    motion::menu_exit_progress(since)
                }
            })
            .filter(|p| *p < 1.);

        let menu = (open || exit.is_some()).then(|| {
            let total = STEPS.len() as u32;
            let mut setup = ProfileSetup::new(done, total);
            if let Some(step) = STEPS.get(done as usize) {
                let demo = demo.clone();
                setup = setup.next(*step).on_continue(move |_, cx| {
                    demo.update(cx, |d, cx| {
                        d.done = (d.done + 1).min(total);
                        cx.notify();
                    })
                });
            }
            let card = ProfileMenu::new(
                "profile-menu",
                "Lena Hartmann",
                "lena.hartmann@fastmail.com",
            )
            .setup(setup)
            .on_select({
                let demo = demo.clone();
                move |_, window, cx| close_demo(&demo, window, cx)
            })
            .on_dismiss({
                let demo = demo.clone();
                move |window, cx| close_demo(&demo, window, cx)
            });
            anchored_above(card, exit, window)
        });

        // Pink `settings_nav_row`: 30 pt, `px 8 / py 5`, radius 8, 13/18;
        // transparent easing to the sidebar hover fill, the selected fill
        // while the menu is up.
        let ring = t.focus_ring();
        let (rest, hover, label) = if open {
            (t.glass_selected_bg(), t.glass_selected_bg(), t.text)
        } else {
            (
                t.sidebar_hover_bg().opacity(0.),
                t.sidebar_hover_bg(),
                t.readable_muted(0.95),
            )
        };
        let trigger = div()
            .id("profile-menu-demo-trigger")
            .track_focus(&trigger_focus)
            .role(Role::Button)
            .aria_label("Account")
            .aria_expanded(open)
            .w_full()
            .min_h(px(30.))
            .px(px(8.))
            .py(px(5.))
            .flex()
            .items_center()
            .gap(px(8.))
            .rounded(px(radius::MENU_ITEM))
            .text_size(px(13.))
            .line_height(px(18.))
            .cursor_pointer()
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .hover_fill_fg("profile-menu-demo-trigger", rest, hover, label, t.text)
            .on_key_down(pink::swallow_activation_key)
            .on_mouse_down(MouseButton::Left, {
                let demo = demo.clone();
                move |_, _, cx| demo.update(cx, |d, _| d.pressed_while_open = d.mounted())
            })
            .on_click({
                let demo = demo.clone();
                move |_, window, cx| {
                    let (was_up, open) = demo.update(cx, |d, _| {
                        (std::mem::take(&mut d.pressed_while_open), d.open)
                    });
                    if was_up {
                        return;
                    }
                    if open {
                        close_demo(&demo, window, cx);
                    } else {
                        demo.update(cx, |d, cx| {
                            d.open = true;
                            d.closing = None;
                            cx.notify();
                        });
                    }
                }
            })
            .child(pink::avatar("L", &t))
            .child(div().flex_1().min_w_0().truncate().child("Lena Hartmann"));

        div()
            .w(px(layout::MENU_W_M - 2. * layout::CARD_INSET))
            .h(px(340.))
            .flex()
            .flex_col()
            .justify_end()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(div().relative().w_full().children(menu).child(trigger))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_wraps_and_enters_from_either_end() {
        assert_eq!(menu_step(None, 5, 1), Some(0));
        assert_eq!(menu_step(None, 5, -1), Some(4));
        assert_eq!(menu_step(Some(4), 5, 1), Some(0));
        assert_eq!(menu_step(Some(0), 5, -1), Some(4));
        assert_eq!(menu_step(Some(1), 0, 1), None);
    }

    #[test]
    fn avatar_initial_follows_pink() {
        assert_eq!(initial("lena hartmann").as_ref(), "L");
        assert_eq!(initial("élise").as_ref(), "É");
        assert_eq!(initial("").as_ref(), "?");
    }

    #[test]
    fn sign_out_opens_its_own_section() {
        let items = default_items();
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_ref()).collect();
        assert_eq!(labels, ["Settings", "Docs", "Apps", "Sign out"]);
        assert!(items[3].separated);
        assert_eq!(items[2].badge.as_deref(), Some("New"));
    }
}
