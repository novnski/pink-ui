//! Profile Menu – account card with the signed-in user, menu actions and
//! profile setup progress.
//!
//! Clean: one flat card with hairline-separated sections. Classic: the white
//! menu card sits on a gray tray, with the setup progress in the tray below.
//! Arrow keys, Home and End move between the actions like a native menu, and
//! the progress bar fills once the card first renders.
//!
//! ```ignore
//! ProfileMenu::new("account", "Lena Hartmann", "lena@fastmail.com")
//!     .setup(ProfileSetup::new(3, 5).next("Add a profile photo").on_continue(|_, _| {}))
//! ```

use std::rc::Rc;

use gpui::{
    AnyView, App, BoxShadow, ElementId, FocusHandle, FontWeight, Hsla, IntoElement, KeyDownEvent,
    Pixels, SharedString, Window, div, prelude::*, px, relative, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, FILL},
    primitives::{SANS, black_a, hairline, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Profile Menu";
pub const DESCRIPTION: &str =
    "Account card with the signed-in user, menu actions and profile setup progress";

const AVATAR: &str = "icons/profile-menu/avatar.svg";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub struct ProfileMenuItem {
    pub id: SharedString,
    pub label: SharedString,
    /// An SVG path under `assets/`, tinted with the item color.
    pub icon: Option<SharedString>,
    /// Small pill on the right, e.g. "New".
    pub badge: Option<SharedString>,
    pub on_select: Option<Handler>,
}

impl ProfileMenuItem {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            badge: None,
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
    /// The next open step, shown as a shortcut under the bar.
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

/// Icon paths for a look: Clean draws thinner strokes.
fn icon_path(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/profile-menu/clean/{name}.svg").into(),
        Look::Classic => format!("icons/profile-menu/{name}.svg").into(),
    }
}

/// Settings, Docs, Apps (with a "New" badge) and Logout.
pub fn default_items(look: Look) -> Vec<ProfileMenuItem> {
    vec![
        ProfileMenuItem::new("settings", "Settings").icon(icon_path(look, "settings")),
        ProfileMenuItem::new("docs", "Docs").icon(icon_path(look, "docs")),
        ProfileMenuItem::new("apps", "Apps")
            .icon(icon_path(look, "apps"))
            .badge("New"),
        ProfileMenuItem::new("logout", "Logout").icon(icon_path(look, "logout")),
    ]
}

/// The `--ui-pm-*` / `--ui-pmc-*` tokens plus the sizes that differ per look.
struct Palette {
    card: Hsla,
    card_shadow: Vec<BoxShadow>,
    /// Classic draws a 1px border around the card; Clean uses hairlines inside it.
    card_border: Option<Hsla>,
    line: Hsla,
    strong: Hsla,
    ink: Hsla,
    item: Hsla,
    text: Hsla,
    muted: Hsla,
    icon: Hsla,
    hover: Hsla,
    fill: Hsla,
    avatar: Hsla,
    badge: Hsla,
    badge_wash: Hsla,
    tray: Option<Hsla>,
    track: Hsla,
    next_hover: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        let dark = cx.is_dark();
        match (look, dark) {
            (Look::Clean, false) => Palette {
                card: hex(0xffffff),
                card_shadow: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.08)),
                    shadow(0., 12., 28., -10., black_a(0.16)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                card_border: None,
                line: black_a(0.06),
                strong: hex(0x000000),
                ink: black_a(0.88),
                item: black_a(0.88),
                text: black_a(0.61),
                muted: black_a(0.45),
                icon: black_a(0.447),
                hover: black_a(0.06),
                fill: hexa(0x0060e2, 0.765),
                avatar: hexa(0x0060e2, 0.765),
                badge: hex(0xe11d48),
                badge_wash: hexa(0xe11d48, 0.08),
                tray: None,
                track: black_a(0.06),
                next_hover: black_a(0.03),
            },
            (Look::Clean, true) => Palette {
                card: hex(0x1c1c1e),
                card_shadow: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                card_border: None,
                line: white_a(0.08),
                strong: hex(0xffffff),
                ink: white_a(0.9),
                item: white_a(0.9),
                text: white_a(0.62),
                muted: white_a(0.46),
                icon: white_a(0.46),
                hover: white_a(0.08),
                fill: hex(0x4d8dff),
                avatar: hex(0x4d8dff),
                badge: hex(0xfb7185),
                badge_wash: hexa(0xe11d48, 0.14),
                tray: None,
                track: white_a(0.1),
                next_hover: white_a(0.04),
            },
            (Look::Classic, false) => Palette {
                card: hex(0xffffff),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
                card_border: Some(hex(0xe5e5e5)),
                line: hex(0xececec),
                strong: hex(0x171717),
                ink: hex(0x171717),
                item: hex(0x404040),
                text: hex(0x404040),
                muted: hex(0x8f8f8f),
                icon: hex(0x737373),
                hover: hex(0xf5f5f5),
                fill: hex(0x2f6bff),
                avatar: hex(0x2f6bff),
                badge: hex(0xe11d48),
                badge_wash: hex(0xfff0f3),
                tray: Some(hex(0xf0f0f0)),
                track: black_a(0.07),
                next_hover: black_a(0.04),
            },
            (Look::Classic, true) => Palette {
                card: hex(0x232326),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                card_border: Some(white_a(0.08)),
                line: white_a(0.08),
                strong: hex(0xededed),
                ink: hex(0xededed),
                item: hex(0xc4c4c4),
                text: hex(0xc4c4c4),
                muted: hex(0x8f8f8f),
                icon: hex(0x8f8f8f),
                hover: hex(0x2a2a2d),
                fill: hex(0x2f6bff),
                avatar: hex(0x6b95ff),
                badge: hex(0xfb7185),
                badge_wash: hexa(0xe11d48, 0.16),
                tray: Some(hex(0x161618)),
                track: white_a(0.1),
                next_hover: white_a(0.06),
            },
        }
    }
}

/// Sizes from the two stylesheets.
struct Metrics {
    max_width: Pixels,
    avatar: Pixels,
    user_gap: Pixels,
    name_size: Pixels,
    name_line: Pixels,
    email_size: Pixels,
    email_line: Pixels,
    row_height: Pixels,
    row_pad: Pixels,
    row_radius: Pixels,
    row_gap: Pixels,
    icon: Pixels,
    label_size: Pixels,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                max_width: px(260.),
                avatar: px(32.),
                user_gap: px(10.),
                name_size: px(13.),
                name_line: px(20.),
                email_size: px(12.),
                email_line: px(16.),
                row_height: px(32.),
                row_pad: px(8.),
                row_radius: px(6.),
                row_gap: px(10.),
                icon: px(16.),
                label_size: px(13.),
            },
            Look::Classic => Metrics {
                max_width: px(320.),
                avatar: px(42.),
                user_gap: px(14.),
                name_size: px(15.),
                name_line: px(22.),
                email_size: px(13.),
                email_line: px(20.),
                row_height: px(36.),
                row_pad: px(12.),
                row_radius: px(12.),
                row_gap: px(12.),
                icon: px(20.),
                label_size: px(14.),
            },
        }
    }
}

#[derive(IntoElement)]
pub struct ProfileMenu {
    id: ElementId,
    name: SharedString,
    email: SharedString,
    items: Option<Vec<ProfileMenuItem>>,
    setup: Option<ProfileSetup>,
    look: Option<Look>,
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
            look: None,
        }
    }

    /// Replaces the default actions.
    pub fn items(mut self, items: Vec<ProfileMenuItem>) -> Self {
        self.items = Some(items);
        self
    }

    /// Profile setup progress under the actions – leave unset to hide it.
    pub fn setup(mut self, setup: ProfileSetup) -> Self {
        self.setup = Some(setup);
        self
    }

    /// Overrides the global Clean/Classic look for this menu.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for ProfileMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let p = Palette::get(look, cx);
        let m = Metrics::get(look);
        let hair = hairline(window);
        let key = self.id.to_string();
        let items = self.items.unwrap_or_else(|| default_items(look));

        // One focus handle per action, kept across frames.
        let count = items.len();
        let focus = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "focus".into()),
            cx,
            |_, cx| {
                (0..count)
                    .map(|_| cx.focus_handle())
                    .collect::<Vec<FocusHandle>>()
            },
        );
        let handles = focus.read(cx).clone();

        let rows: Vec<_> = items
            .iter()
            .zip(handles.clone())
            .map(|(item, handle)| {
                let hover_key = format!("{key}-item-{}", item.id);
                let on_select = item.on_select.clone();
                let icon_color = motion::hover_blend(&hover_key, p.icon, p.ink);
                div()
                    .id(item.id.clone())
                    .track_focus(&handle)
                    .flex()
                    .items_center()
                    .gap(m.row_gap)
                    .w_full()
                    .h(m.row_height)
                    .px(m.row_pad)
                    .rounded(m.row_radius)
                    .cursor_pointer()
                    .hover_fill_fg(hover_key, p.hover.opacity(0.), p.hover, p.item, p.ink)
                    .focus(|s| s.bg(p.hover).text_color(p.ink))
                    .when_some(on_select, |this, f| {
                        this.on_click(move |_, window, cx| f(window, cx))
                    })
                    .when_some(item.icon.clone(), |this, path| {
                        this.child(
                            svg()
                                .path(path)
                                .size(m.icon)
                                .flex_none()
                                .text_color(icon_color),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(m.label_size)
                            .line_height(px(20.))
                            .child(item.label.clone()),
                    )
                    .when_some(item.badge.clone(), |this, badge| {
                        let pill = div()
                            .flex_none()
                            .bg(p.badge_wash)
                            .font_weight(FontWeight::MEDIUM)
                            .line_height(px(18.))
                            .text_color(p.badge)
                            .child(badge);
                        this.child(match look {
                            Look::Clean => pill.px(px(6.)).rounded(px(5.)).text_size(px(11.)),
                            Look::Classic => pill
                                .px(px(8.))
                                .py(px(2.))
                                .rounded(px(999.))
                                .text_size(px(12.)),
                        })
                    })
            })
            .collect();

        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let len = handles.len();
            if len == 0 {
                return;
            }
            let current = handles.iter().position(|f| f.is_focused(window));
            let next = match event.keystroke.key.as_str() {
                "down" => current.map_or(0, |i| (i + 1) % len),
                "up" => current.map_or(len - 1, |i| (i + len - 1) % len),
                "home" => 0,
                "end" => len - 1,
                _ => return,
            };
            window.focus(&handles[next]);
            cx.stop_propagation();
        };

        let user = div()
            .flex()
            .items_center()
            .gap(m.user_gap)
            .child(
                svg()
                    .path(AVATAR)
                    .size(m.avatar)
                    .flex_none()
                    .text_color(p.avatar),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(m.name_size)
                            .line_height(m.name_line)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.strong)
                            .child(self.name.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(m.email_size)
                            .line_height(m.email_line)
                            .text_color(p.muted)
                            .child(self.email.clone()),
                    ),
            );
        let user = match look {
            // A hairline under the user, running edge to edge
            Look::Clean => user
                .mx(px(-4.))
                .mb(px(4.))
                .pt(px(8.))
                .px(px(12.))
                .pb(px(10.))
                .border_b(hair)
                .border_color(p.line),
            Look::Classic => user.px(px(10.)).pb(px(12.)),
        };
        let list = div()
            .id("items")
            .flex()
            .flex_col()
            .when(look == Look::Classic, |d| d.gap(px(2.)))
            .on_key_down(on_key)
            .children(rows);

        let card = div()
            .bg(p.card)
            .shadow(p.card_shadow.clone())
            .when_some(p.card_border, |d, border| d.border_1().border_color(border))
            .child(user)
            .child(list);
        let setup = self
            .setup
            .map(|setup| setup_progress(&key, look, setup, &p, hair));

        let root = div()
            .id(self.id.clone())
            .w_full()
            .max_w(m.max_width)
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(p.ink);
        match look {
            Look::Clean => root.child(card.p(px(4.)).rounded(px(10.)).children(setup)),
            Look::Classic => root
                .p(px(6.))
                .rounded(px(22.))
                .when_some(p.tray, |d, tray| d.bg(tray))
                .child(card.pt(px(16.)).px(px(8.)).pb(px(8.)).rounded(px(16.)))
                .children(setup),
        }
    }
}

/// Count, bar and the next step: the last row of the Clean card, or the gray
/// extension under the Classic one.
fn setup_progress(
    key: &str,
    look: Look,
    setup: ProfileSetup,
    p: &Palette,
    hair: Pixels,
) -> impl IntoElement {
    let ratio = if setup.total > 0 {
        (setup.done as f32 / setup.total as f32).clamp(0., 1.)
    } else {
        0.
    };
    // Fills from empty the first time the card renders, then follows changes.
    let fill = motion::follow_from(&format!("{key}-fill"), 0., ratio, FILL);
    let open = setup.next.is_some() && setup.done < setup.total;
    let next_key = format!("{key}-next");
    let arrow_shift = motion::hover_t(&next_key) * 2.;
    let clean = look == Look::Clean;

    div()
        .when(clean, |d| {
            d.mt(px(4.))
                .mx(px(-4.))
                .mb(px(-4.))
                .pt(px(10.))
                .px(px(12.))
                .pb(px(if open { 6. } else { 12. }))
                .border_t(hair)
                .border_color(p.line)
        })
        .when(!clean, |d| d.px(px(18.)).pt(px(10.)).pb(px(12.)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .text_size(px(if clean { 12. } else { 13. }))
                .line_height(px(if clean { 16. } else { 18. }))
                .child(
                    div()
                        .text_color(p.text)
                        .font_weight(FontWeight::MEDIUM)
                        .child("Profile setup"),
                )
                .child(div().text_color(p.muted).child(format!(
                    "{}/{}",
                    setup.done.min(setup.total),
                    setup.total
                ))),
        )
        .child(
            div()
                .mt(px(if clean { 8. } else { 7. }))
                .h(px(4.))
                .w_full()
                .overflow_hidden()
                .rounded(px(2.))
                .bg(p.track)
                .child(div().h_full().w(relative(fill)).rounded(px(2.)).bg(p.fill)),
        )
        .when(open, |this| {
            let on_continue = setup.on_continue.clone();
            let arrow = if clean {
                "icons/profile-menu/clean/arrow.svg"
            } else {
                "icons/profile-menu/arrow.svg"
            };
            let arrow_size = px(if clean { 16. } else { 14. });
            this.child(
                div()
                    .id("next")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .mt(px(6.))
                    .mx(px(-8.))
                    .px(px(8.))
                    .when(clean, |d| d.h(px(28.)).rounded(px(6.)))
                    .when(!clean, |d| d.py(px(3.)).rounded(px(8.)))
                    .cursor_pointer()
                    .hover_fill(next_key.clone(), p.next_hover.opacity(0.), p.next_hover)
                    .when_some(on_continue, |this, f| {
                        this.on_click(move |_, window, cx| f(window, cx))
                    })
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(4.))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(p.ink)
                            .child(div().text_color(p.muted).child("Next:"))
                            .child(setup.next.clone().unwrap_or_default()),
                    )
                    .child(
                        div()
                            .flex_none()
                            .w(arrow_size + px(2.))
                            .pl(px(arrow_shift))
                            .child(
                                svg()
                                    .path(arrow)
                                    .size(arrow_size)
                                    .text_color(motion::hover_blend(&next_key, p.icon, p.ink)),
                            ),
                    ),
            )
        })
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        ProfileMenu::new(
            "profile-menu",
            "Lena Hartmann",
            "lena.hartmann@fastmail.com",
        )
        .setup(ProfileSetup::new(3, 5).next("Add a profile photo"))
    })
}
