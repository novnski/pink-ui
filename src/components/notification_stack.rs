//! Notification Stack – notice cards stacked behind each other: dismiss the
//! front one to reveal the next.
//!
//! Only the front card can be read; the cards behind peek out below it, each
//! a little lower and smaller. Dismissing the front card slides it away and
//! the next one moves up, one notice at a time. Every card carries quiet line
//! art in its top-right corner that fades out towards the text.
//!
//! Clean: hairline-shadow cards, Inter at 13/15px and one blue mark in the
//! art. Classic: bordered cards with a soft drop shadow and gray art.
//!
//! ```ignore
//! NotificationStack::new("notices", vec![
//!     Notice::new("update", "Update ready", "Restart to get the latest version.")
//!         .badge("New")
//!         .action(NoticeAction::new("Restart").on_click(|_, _| {})),
//! ])
//! .on_dismiss(|notice, _, _| println!("dismissed {}", notice.id))
//! ```

use std::{
    collections::{HashMap, HashSet},
    f32::consts::PI,
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use gpui::{
    AnyView, App, Bounds, BoxShadow, ElementId, FontWeight, Hsla, IntoElement, PathBuilder, Pixels,
    SharedString, Task, Window, canvas, div, fill, font, point, prelude::*, px, size, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, EASE, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, ring, shadow, white_a},
    story,
};

pub const NAME: &str = "Notification Stack";
pub const DESCRIPTION: &str =
    "Notice cards stacked behind each other – dismiss the front one to reveal the next";

/// How far each card behind peeks out below the one in front, in px.
const PEEK: f32 = 12.;
/// Cards moving up the stack and the stack's height: `460ms`.
const STACK: MotionSpec = MotionSpec::new(460, EASE_OUT_QUINT);
/// The dismissed card sliding out: `transform 320ms`.
const LEAVE: MotionSpec = MotionSpec::new(320, EASE_OUT_QUINT);
/// The dismissed card fading: `opacity 240ms ease`.
const LEAVE_FADE: MotionSpec = MotionSpec::new(240, EASE);
/// Cards past the third fading: `opacity 300ms ease`.
const CARD_FADE: MotionSpec = MotionSpec::new(300, EASE);
/// A card's contents showing or hiding as it reaches the front: `260ms ease`.
const CONTENT_FADE: MotionSpec = MotionSpec::new(260, EASE);

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type DismissHandler = Rc<dyn Fn(&Notice, &mut Window, &mut App)>;

/// Line art for a card's top-right corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoticePattern {
    /// Rings around the corner, crossed by three lines.
    #[default]
    Orbit,
    /// A grid with one tinted cell.
    Grid,
    /// Rays fanning out of the corner, with an arc.
    Rays,
}

/// The button under a notice.
#[derive(Clone)]
pub struct NoticeAction {
    pub label: SharedString,
    /// Opens something elsewhere: shows an arrow after the label.
    pub external: bool,
    pub on_click: Option<Handler>,
}

impl NoticeAction {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            external: false,
            on_click: None,
        }
    }

    pub fn external(mut self, external: bool) -> Self {
        self.external = external;
        self
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

#[derive(Clone)]
pub struct Notice {
    pub id: SharedString,
    /// Small label in the pill, e.g. "Notice".
    pub badge: Option<SharedString>,
    pub title: SharedString,
    pub text: SharedString,
    pub action: Option<NoticeAction>,
    pub pattern: NoticePattern,
}

impl Notice {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            badge: None,
            title: title.into(),
            text: text.into(),
            action: None,
            pattern: NoticePattern::default(),
        }
    }

    pub fn badge(mut self, badge: impl Into<SharedString>) -> Self {
        self.badge = Some(badge.into());
        self
    }

    pub fn action(mut self, action: NoticeAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn pattern(mut self, pattern: NoticePattern) -> Self {
        self.pattern = pattern;
        self
    }
}

/// The `--ui-nsc-*` / `--ui-ns-*` tokens, by role.
struct Palette {
    card: Hsla,
    card_border: Option<Hsla>,
    card_shadow: Vec<BoxShadow>,
    title: Hsla,
    ink: Hsla,
    text: Hsla,
    badge_bg: Hsla,
    badge_fg: Hsla,
    badge_border: Option<Hsla>,
    close: Hsla,
    close_hover_bg: Hsla,
    art: Hsla,
    dot: Hsla,
    art_fill: Hsla,
    btn_bg: Hsla,
    btn_hover_bg: Hsla,
    btn_fg: Hsla,
    btn_icon: Hsla,
    btn_shadow: Vec<BoxShadow>,
    btn_shadow_hover: Vec<BoxShadow>,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => {
                let shadow_rest = vec![
                    ring(0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ];
                Palette {
                    card: hex(0xffffff),
                    card_border: None,
                    card_shadow: shadow_rest.clone(),
                    title: hex(0x000000),
                    ink: black_a(0.88),
                    text: black_a(0.61),
                    badge_bg: black_a(0.06),
                    badge_fg: black_a(0.61),
                    badge_border: None,
                    close: black_a(0.447),
                    close_hover_bg: black_a(0.03),
                    art: black_a(0.07),
                    dot: hexa(0x0060e2, 0.765),
                    art_fill: hexa(0x0060e2, 0.05),
                    btn_bg: hex(0xffffff),
                    btn_hover_bg: hex(0xffffff),
                    btn_fg: hex(0x000000),
                    btn_icon: black_a(0.447),
                    btn_shadow: shadow_rest,
                    btn_shadow_hover: vec![
                        ring(0.5, black_a(0.1)),
                        shadow(0., 1., 1., -1., black_a(0.12)),
                        shadow(0., 1., 3., 0., black_a(0.07)),
                    ],
                }
            }
            (Look::Clean, true) => {
                let shadow_rest = vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ];
                Palette {
                    card: hex(0x1c1c1e),
                    card_border: None,
                    card_shadow: shadow_rest.clone(),
                    title: hex(0xffffff),
                    ink: white_a(0.9),
                    text: white_a(0.62),
                    badge_bg: white_a(0.08),
                    badge_fg: white_a(0.62),
                    badge_border: None,
                    close: white_a(0.46),
                    close_hover_bg: white_a(0.04),
                    art: white_a(0.07),
                    dot: hex(0x4d8dff),
                    art_fill: hexa(0x4d8dff, 0.1),
                    btn_bg: white_a(0.04),
                    btn_hover_bg: white_a(0.04),
                    btn_fg: hex(0xffffff),
                    btn_icon: white_a(0.46),
                    btn_shadow: shadow_rest,
                    btn_shadow_hover: vec![
                        ring(0.5, white_a(0.16)),
                        shadow(0., 1., 2., 0., black_a(0.4)),
                    ],
                }
            }
            (Look::Classic, false) => {
                let line = hex(0xe5e5e5);
                let btn_shadow = vec![ring(1., line), shadow(0., 1., 2., 0., black_a(0.04))];
                Palette {
                    card: hex(0xffffff),
                    card_border: Some(line),
                    card_shadow: vec![
                        shadow(0., 1., 2., 0., black_a(0.03)),
                        shadow(0., 8., 24., 0., black_a(0.035)),
                    ],
                    title: hex(0x171717),
                    ink: hex(0x171717),
                    text: hex(0x737373),
                    badge_bg: hex(0xffffff),
                    badge_fg: hex(0x525252),
                    badge_border: Some(line),
                    close: hex(0x737373),
                    close_hover_bg: black_a(0.05),
                    art: hex(0xe3e3e3),
                    dot: hex(0xd4d4d4),
                    art_fill: hex(0xf5f5f5),
                    btn_bg: hex(0xffffff),
                    btn_hover_bg: hex(0xf7f7f7),
                    btn_fg: hex(0x171717),
                    btn_icon: hex(0x737373),
                    btn_shadow: btn_shadow.clone(),
                    btn_shadow_hover: btn_shadow,
                }
            }
            (Look::Classic, true) => {
                let line = white_a(0.1);
                let btn_shadow = vec![ring(1., line), shadow(0., 1., 2., 0., black_a(0.4))];
                Palette {
                    card: hex(0x232326),
                    card_border: Some(line),
                    card_shadow: vec![
                        shadow(0., 1., 2., 0., black_a(0.4)),
                        shadow(0., 8., 24., 0., black_a(0.35)),
                    ],
                    title: hex(0xededed),
                    ink: hex(0xededed),
                    text: hex(0x8f8f8f),
                    badge_bg: hex(0x2a2a2d),
                    badge_fg: hex(0xc4c4c4),
                    badge_border: Some(line),
                    close: hex(0x8f8f8f),
                    close_hover_bg: white_a(0.06),
                    art: white_a(0.08),
                    dot: white_a(0.22),
                    art_fill: white_a(0.04),
                    btn_bg: hex(0x2a2a2d),
                    btn_hover_bg: hex(0x313135),
                    btn_fg: hex(0xededed),
                    btn_icon: hex(0x8f8f8f),
                    btn_shadow: btn_shadow.clone(),
                    btn_shadow_hover: btn_shadow,
                }
            }
        }
    }
}

fn icon(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/notification-stack/clean/{name}.svg").into(),
        Look::Classic => format!("icons/notification-stack/{name}.svg").into(),
    }
}

/// Eases one shadow stack into another layer by layer (the CSS
/// `box-shadow` transition); stacks of different lengths snap.
fn mix_shadows(from: &[BoxShadow], to: &[BoxShadow], t: f32) -> Vec<BoxShadow> {
    if from.len() != to.len() {
        return if t < 0.5 { from.to_vec() } else { to.to_vec() };
    }
    from.iter()
        .zip(to)
        .map(|(a, b)| BoxShadow {
            color: motion::mix(a.color, b.color, t),
            offset: point(
                a.offset.x + (b.offset.x - a.offset.x) * t,
                a.offset.y + (b.offset.y - a.offset.y) * t,
            ),
            blur_radius: a.blur_radius + (b.blur_radius - a.blur_radius) * t,
            spread_radius: a.spread_radius + (b.spread_radius - a.spread_radius) * t,
            inset: a.inset,
        })
        .collect()
}

/// The advance of `ch` in Inter at `size`.
fn advance(window: &Window, size: f32, ch: char) -> f32 {
    let text = window.text_system();
    let id = text.resolve_font(&font(SANS));
    text.advance(id, px(size), ch)
        .map(|s| f32::from(s.width))
        .unwrap_or(size * 0.3)
}

/// `max-width: <n>ch` for a paragraph (`ch` is the width of `0`). GPUI
/// counts the space after a line's last word when wrapping, where browsers
/// let it hang, so the box gets one space more.
fn paragraph_width(window: &Window, size: f32, n: f32) -> f32 {
    n * advance(window, size, '0') + advance(window, size, ' ')
}

/// UI state that lives across renders.
struct StackState {
    items: Vec<Notice>,
    /// Dismissed, still sliding out.
    leaving: HashSet<SharedString>,
    /// Each card's natural height, measured last frame.
    heights: HashMap<SharedString, f32>,
    /// The stack's width, measured last frame.
    width: Option<f32>,
}

#[derive(IntoElement)]
pub struct NotificationStack {
    id: ElementId,
    notices: Vec<Notice>,
    on_dismiss: Option<DismissHandler>,
    look: Option<Look>,
}

impl NotificationStack {
    /// The stack keeps its own copy of `notices` from the first render on.
    pub fn new(id: impl Into<ElementId>, notices: Vec<Notice>) -> Self {
        Self {
            id: id.into(),
            notices,
            on_dismiss: None,
            look: None,
        }
    }

    /// Called after a notice was dismissed.
    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&Notice, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }

    /// Overrides the global Clean/Classic look for this stack.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

/// Where one card sits this frame.
struct Placement {
    /// translateY, px.
    y: f32,
    /// translateX, px (only while leaving).
    x: f32,
    scale: f32,
    opacity: f32,
    /// The contents' opacity: only the front card shows them.
    content: f32,
}

impl RenderOnce for NotificationStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Rc::new(Palette::get(look, cx));
        let key = self.id.to_string();

        let state = {
            let notices = self.notices.clone();
            window.use_keyed_state(
                ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
                cx,
                move |_, _| StackState {
                    items: notices,
                    leaving: HashSet::new(),
                    heights: HashMap::new(),
                    width: None,
                },
            )
        };
        let (items, leaving, heights, width) = {
            let s = state.read(cx);
            (
                s.items.clone(),
                s.leaving.clone(),
                s.heights.clone(),
                s.width.unwrap_or(380.),
            )
        };

        let visible: Vec<&Notice> = items.iter().filter(|n| !leaving.contains(&n.id)).collect();
        let behind = visible.len().saturating_sub(1).min(2);
        // Until every card has been measured once, place them without easing
        let ready = items.iter().all(|n| heights.contains_key(&n.id));
        let front = visible
            .first()
            .and_then(|n| heights.get(&n.id))
            .copied()
            .unwrap_or(0.);
        let stack_target = if visible.is_empty() {
            0.
        } else {
            front + behind as f32 * PEEK
        };
        let ease = |name: &str, id: &str, target: f32, spec: MotionSpec| {
            if ready {
                motion::follow(&format!("{key}-{id}-{name}"), target, spec)
            } else {
                target
            }
        };
        let stack_height = ease("stack", "", stack_target, STACK);

        let dismiss: DismissHandler = {
            let state = state.clone();
            let on_dismiss = self.on_dismiss.clone();
            Rc::new(move |notice: &Notice, window: &mut Window, cx: &mut App| {
                let id = notice.id.clone();
                state.update(cx, |s, cx| {
                    if !s.leaving.insert(id.clone()) {
                        return;
                    }
                    let delay = LEAVE.total().mul_f32(motion::speed_scale());
                    // Holds only a weak handle: harmless if the stack unmounts first
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(delay).await;
                        this.update(cx, |s, cx| {
                            s.items.retain(|n| n.id != id);
                            s.leaving.remove(&id);
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                    cx.notify();
                });
                if let Some(f) = &on_dismiss {
                    f(notice, window, cx);
                }
            })
        };

        let text_max = if clean {
            paragraph_width(window, 13., 38.)
        } else {
            paragraph_width(window, 14., 34.)
        };
        let mut cards: Vec<(i32, gpui::AnyElement)> = items
            .iter()
            .map(|notice| {
                let depth = visible.iter().position(|n| n.id == notice.id);
                let gone = depth.is_none();
                let depth = depth.unwrap_or(0);
                let height = heights.get(&notice.id).copied();
                let id = notice.id.as_ref();
                // Cards behind line up on the front card's bottom edge, a
                // little lower and smaller each
                let y = if gone || depth == 0 {
                    0.
                } else {
                    front - height.unwrap_or(front) + depth.min(2) as f32 * PEEK
                };
                let scale = if gone {
                    0.98
                } else {
                    1. - depth.min(2) as f32 * 0.05
                };
                let transform = if gone { LEAVE } else { STACK };
                let place = Placement {
                    y: ease("y", id, y, transform),
                    x: ease("x", id, if gone { 40. } else { 0. }, transform),
                    scale: ease("scale", id, scale, transform),
                    opacity: ease(
                        "opacity",
                        id,
                        if gone || depth > 2 { 0. } else { 1. },
                        if gone { LEAVE_FADE } else { CARD_FADE },
                    ),
                    content: ease(
                        "content",
                        id,
                        if !gone && depth == 0 { 1. } else { 0. },
                        CONTENT_FADE,
                    ),
                };
                let z = if gone { 10 } else { 10 - depth as i32 };
                let interactive = !gone && depth == 0;
                let card = card(CardProps {
                    key: &key,
                    look,
                    notice,
                    p: &p,
                    place,
                    height,
                    width,
                    text_max,
                    interactive,
                    state: &state,
                    dismiss: dismiss.clone(),
                });
                (z, card.into_any_element())
            })
            .collect();
        // CSS paints higher z-index on top, ties in document order
        cards.sort_by_key(|(z, _)| *z);

        let measure = {
            let state = state.clone();
            canvas(
                move |bounds, window, cx| {
                    let w = f32::from(bounds.size.width);
                    let changed = state.update(cx, |s, _| {
                        let changed = s.width.is_none_or(|old| (old - w).abs() > 0.5);
                        if changed {
                            s.width = Some(w);
                        }
                        changed
                    });
                    if changed {
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .max_w(px(380.))
            .h(px(stack_height))
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(if clean { 20. } else { 18.571 }))
            .text_color(p.ink)
            .child(measure)
            .children(cards.into_iter().map(|(_, card)| card))
    }
}

struct CardProps<'a> {
    key: &'a str,
    look: Look,
    notice: &'a Notice,
    p: &'a Palette,
    place: Placement,
    height: Option<f32>,
    width: f32,
    /// The paragraph's `max-width`.
    text_max: f32,
    interactive: bool,
    state: &'a gpui::Entity<StackState>,
    dismiss: DismissHandler,
}

/// One notice card, placed and scaled in the stack.
fn card(props: CardProps) -> impl IntoElement {
    let CardProps {
        key,
        look,
        notice,
        p,
        place,
        height,
        width,
        text_max,
        interactive,
        state,
        dismiss,
    } = props;
    let clean = look == Look::Clean;
    let border = if p.card_border.is_some() { 2. } else { 0. };

    // `scale()` around the bottom edge's center: GPUI can't scale a box, so
    // the card shrinks towards it instead (the contents keep their size)
    let s = place.scale;
    let inset = width * (1. - s) / 2.;
    let scaled = (1. - s).abs() > 0.001;
    let top = place.y + height.map_or(0., |h| h * (1. - s));

    // Records the card's natural height for the next frame
    let measure = {
        let state = state.clone();
        let id = notice.id.clone();
        canvas(
            move |bounds, window, cx| {
                let h = f32::from(bounds.size.height) + border;
                let changed = state.update(cx, |s, _| {
                    let changed = s.heights.get(&id).is_none_or(|old| (old - h).abs() > 0.5);
                    if changed {
                        s.heights.insert(id.clone(), h);
                    }
                    changed
                });
                if changed {
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    };

    let close_key = format!("{key}-{}-close", notice.id);
    let close = div()
        .id("close")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .rounded(px(if clean { 6. } else { 8. }))
        .when(!clean, |d| d.mt(px(-2.)).mr(px(-6.)))
        .child(
            svg()
                .path(icon(look, "close"))
                .size(px(16.))
                .flex_none()
                .text_color(motion::hover_blend(&close_key, p.close, p.ink)),
        )
        .when(interactive, |d| {
            let dismiss = dismiss.clone();
            let notice = notice.clone();
            d.cursor_pointer()
                .hover_fill(
                    close_key.clone(),
                    p.close_hover_bg.opacity(0.),
                    p.close_hover_bg,
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    dismiss(&notice, window, cx);
                })
        });

    let badge = notice.badge.clone().map(|badge| {
        let pill = div()
            .flex_none()
            .bg(p.badge_bg)
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.badge_fg)
            .child(badge.to_uppercase());
        if clean {
            pill.px(px(6.))
                .py(px(2.))
                .rounded(px(5.))
                .text_size(px(10.))
                .line_height(px(16.))
        } else {
            // The inset ring becomes a border, taken out of the padding
            pill.px(px(8.))
                .py(px(1.))
                .rounded(px(999.))
                .when_some(p.badge_border, |d, c| d.border_1().border_color(c))
                .text_size(px(11.))
                .line_height(px(18.))
        }
    });
    let top_row = div()
        .flex()
        .w_full()
        .items_center()
        .justify_between()
        .min_h(px(if clean { 28. } else { 26. }))
        .mb(px(if clean { 8. } else { 14. }))
        .child(badge.unwrap_or_else(div))
        .child(close);

    let title = div()
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.title)
        .when(clean, |d| {
            d.pr(px(4.)).text_size(px(15.)).line_height(px(24.))
        })
        .when(!clean, |d| d.text_size(px(16.)).line_height(px(22.)))
        .child(notice.title.clone());
    let text = div()
        .text_color(p.text)
        .when(clean, |d| {
            d.mt(px(2.))
                .pr(px(4.))
                .max_w(px(text_max))
                .text_size(px(13.))
                .line_height(px(20.))
        })
        .when(!clean, |d| {
            d.mt(px(6.))
                .max_w(px(text_max))
                .text_size(px(14.))
                .line_height(px(21.))
        })
        .child(notice.text.clone());

    let button = notice.action.clone().map(|action| {
        let btn_key = format!("{key}-{}-action", notice.id);
        let t = motion::hover_t(&btn_key);
        div()
            .id("action")
            .flex()
            .flex_none()
            .items_center()
            .whitespace_nowrap()
            .text_size(px(13.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.btn_fg)
            .bg(motion::mix(p.btn_bg, p.btn_hover_bg, t))
            .shadow(mix_shadows(&p.btn_shadow, &p.btn_shadow_hover, t))
            .when(clean, |d| {
                d.gap(px(6.))
                    .h(px(28.))
                    .mt(px(12.))
                    .pl(px(10.))
                    .pr(px(if action.external { 8. } else { 10. }))
                    .rounded(px(6.))
                    .line_height(px(20.))
            })
            .when(!clean, |d| {
                d.gap(px(8.))
                    .h(px(32.))
                    .mt(px(16.))
                    .px(px(12.))
                    .rounded(px(9.))
            })
            .child(action.label.clone())
            .when(action.external, |d| {
                d.child(
                    svg()
                        .path(icon(look, "arrow"))
                        .size(px(if clean { 16. } else { 14. }))
                        .flex_none()
                        .text_color(p.btn_icon),
                )
            })
            .when(interactive, |d| {
                d.cursor_pointer()
                    .on_hover(motion::hover_listener(btn_key))
                    .when_some(action.on_click.clone(), |d, f| {
                        d.on_click(move |_, window, cx| f(window, cx))
                    })
            })
    });

    let body = div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .items_start()
        .when(clean, |d| d.pt(px(12.)).pr(px(12.)).pb(px(16.)).pl(px(16.)))
        .when(!clean, |d| d.pt(px(18.)).px(px(20.)).pb(px(20.)))
        .child(measure)
        .child(top_row)
        .child(title)
        .child(text)
        .children(button);

    let art = (place.content > 0.001).then(|| {
        let colors = ArtColors {
            line: p.art,
            dot: p.dot,
            fill: p.art_fill,
            width: if clean { 0.75 } else { 1. },
        };
        let kind = notice.pattern;
        div()
            .absolute()
            .top_0()
            .right_0()
            .w(px(220.))
            .h(px(160.))
            .overflow_hidden()
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| paint_pattern(bounds, kind, colors, window),
                )
                .size_full(),
            )
    });

    div()
        .id(ElementId::Name(format!("card-{}", notice.id).into()))
        .absolute()
        .top(px(top))
        .left(px(inset + place.x))
        .right(px(inset - place.x))
        .when(scaled && height.is_some(), |d| {
            d.h(px(height.unwrap_or(0.) * s))
        })
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(if clean { 10. } else { 20. }))
        .bg(p.card)
        .shadow(p.card_shadow.clone())
        .when_some(p.card_border, |d, c| d.border_1().border_color(c))
        .opacity(place.opacity)
        .when(interactive, |d| d.occlude())
        .children(art.map(|art| art.opacity(place.content)))
        .child(body.opacity(place.content))
}

#[derive(Clone, Copy)]
struct ArtColors {
    line: Hsla,
    dot: Hsla,
    fill: Hsla,
    /// Stroke width in px.
    width: f32,
}

/// `radial-gradient(120% 110% at 100% 0%, #000 20%, transparent 70%)` over
/// the 220×160 art box: how much of the art shows at a point.
fn art_mask(x: f32, y: f32) -> f32 {
    let dx = (x - 220.) / (220. * 1.2);
    let dy = y / (160. * 1.1);
    let d = (dx * dx + dy * dy).sqrt();
    ((0.7 - d) / 0.5).clamp(0., 1.)
}

/// Paints the corner art. GPUI has no masks, so every stroke is cut into
/// short pieces, each drawn at the gradient mask's strength under it.
fn paint_pattern(
    bounds: Bounds<Pixels>,
    kind: NoticePattern,
    colors: ArtColors,
    window: &mut Window,
) {
    const STEPS: usize = 32;
    let origin = bounds.origin;
    let at = |x: f32, y: f32| point(origin.x + px(x), origin.y + px(y));

    let mut segments: Vec<((f32, f32), (f32, f32))> = Vec::new();
    let mut line = |a: (f32, f32), b: (f32, f32)| {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let n = (len / 3.).ceil().max(1.) as usize;
        for i in 0..n {
            let t0 = i as f32 / n as f32;
            let t1 = (i + 1) as f32 / n as f32;
            segments.push((
                (a.0 + (b.0 - a.0) * t0, a.1 + (b.1 - a.1) * t0),
                (a.0 + (b.0 - a.0) * t1, a.1 + (b.1 - a.1) * t1),
            ));
        }
    };
    let arc = |line: &mut dyn FnMut((f32, f32), (f32, f32)),
               c: (f32, f32),
               r: f32,
               from: f32,
               to: f32| {
        let n = ((r * (to - from).abs()) / 3.).ceil().max(8.) as usize;
        let pt = |a: f32| (c.0 + r * a.cos(), c.1 + r * a.sin());
        for i in 0..n {
            let a0 = from + (to - from) * i as f32 / n as f32;
            let a1 = from + (to - from) * (i + 1) as f32 / n as f32;
            line(pt(a0), pt(a1));
        }
    };

    let mut dots: Vec<(f32, f32)> = Vec::new();
    let mut cells: Vec<(f32, f32, f32, f32)> = Vec::new();
    match kind {
        NoticePattern::Orbit => {
            for r in [46., 86., 128.] {
                arc(&mut line, (190., 10.), r, 0., 2. * PI);
            }
            line((60., 160.), (220., 0.));
            line((0., 58.), (220., 58.));
            line((150., 0.), (150., 160.));
            dots.push((150., 58.));
        }
        NoticePattern::Grid => {
            for x in [30., 60., 90., 120., 150., 180., 210.] {
                line((x, 0.), (x, 160.));
            }
            for y in [20., 50., 80., 110., 140.] {
                line((0., y), (220., y));
            }
            cells.push((150., 50., 30., 30.));
            dots.push((180., 50.));
        }
        NoticePattern::Rays => {
            for i in 0..8 {
                let a = i as f32 * PI / 16.;
                line((210., 6.), (210. - a.cos() * 230., 6. + a.sin() * 230.));
            }
            // M120 6 a90 90 0 0 0 90 90: a quarter circle around (210, 6)
            arc(&mut line, (210., 6.), 90., PI, PI / 2.);
            dots.push((146., 70.));
        }
    }

    for &(x, y, w, h) in &cells {
        let strength = art_mask(x + w / 2., y + h / 2.);
        window.paint_quad(fill(
            Bounds::new(at(x, y), size(px(w), px(h))),
            colors.fill.opacity(strength),
        ));
    }

    let mut buckets: Vec<Option<PathBuilder>> = (0..=STEPS).map(|_| None).collect();
    for (a, b) in segments {
        let strength = art_mask((a.0 + b.0) / 2., (a.1 + b.1) / 2.);
        let step = (strength * STEPS as f32).round() as usize;
        if step == 0 {
            continue;
        }
        let builder = buckets[step].get_or_insert_with(|| PathBuilder::stroke(px(colors.width)));
        builder.move_to(at(a.0, a.1));
        builder.line_to(at(b.0, b.1));
    }
    for (step, builder) in buckets.into_iter().enumerate() {
        if let Some(path) = builder.and_then(|b| b.build().ok()) {
            window.paint_path(path, colors.line.opacity(step as f32 / STEPS as f32));
        }
    }

    for (x, y) in dots {
        let mut builder = PathBuilder::fill();
        for i in 0..24 {
            let a = i as f32 / 24. * 2. * PI;
            let p = at(x + 3.5 * a.cos(), y + 3.5 * a.sin());
            if i == 0 {
                builder.move_to(p);
            } else {
                builder.line_to(p);
            }
        }
        builder.close();
        if let Ok(path) = builder.build() {
            window.paint_path(path, colors.dot.opacity(art_mask(x, y)));
        }
    }
}

/// Dark mode, scheduled maintenance and a workspace invite – the demo.
pub fn demo_notices() -> Vec<Notice> {
    vec![
        Notice::new(
            "dark-mode",
            "Dark mode is here",
            "Every screen now follows your system theme – or pick one yourself in settings.",
        )
        .badge("New")
        .action(NoticeAction::new("Try it"))
        .pattern(NoticePattern::Orbit),
        Notice::new(
            "maintenance",
            "Scheduled maintenance",
            "We'll be offline on Sunday from 02:00 to 03:00 UTC. Your data stays safe.",
        )
        .badge("Notice")
        .action(NoticeAction::new("View status").external(true))
        .pattern(NoticePattern::Grid),
        Notice::new(
            "invite",
            "Mara invited you to Design Ops",
            "Join the workspace to see shared projects, files and comments.",
        )
        .badge("Invite")
        .action(NoticeAction::new("Accept invite"))
        .pattern(NoticePattern::Rays),
    ]
}

/// The demo brings the notices back 1.2s after the last one is dismissed.
struct Demo {
    generation: usize,
    remaining: usize,
    _reset: Option<Task<()>>,
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let count = demo_notices().len();
        let demo = window.use_keyed_state("notification-stack-demo", cx, move |_, _| Demo {
            generation: 0,
            remaining: count,
            _reset: None,
        });
        let generation = demo.read(cx).generation;
        let on_dismiss = move |_: &Notice, _: &mut Window, cx: &mut App| {
            demo.update(cx, |d, cx| {
                if d.remaining > 1 {
                    d.remaining -= 1;
                    return;
                }
                d._reset = Some(cx.spawn(async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(1200))
                        .await;
                    this.update(cx, |d, cx| {
                        d.remaining = count;
                        d.generation += 1;
                        cx.notify();
                    })
                    .ok();
                }));
            })
        };
        NotificationStack::new(("notification-stack", generation), demo_notices())
            .on_dismiss(on_dismiss)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_mask_fades_from_the_corner() {
        assert_eq!(art_mask(220., 0.), 1.);
        assert_eq!(art_mask(0., 160.), 0.);
        let mid = art_mask(150., 58.);
        assert!(mid > 0.3 && mid < 0.8, "{mid}");
    }

    #[test]
    fn shadows_mix_layer_by_layer() {
        let a = vec![shadow(0., 1., 2., 0., black_a(0.1))];
        let b = vec![shadow(0., 3., 4., 0., black_a(0.3))];
        let m = mix_shadows(&a, &b, 0.5);
        assert_eq!(m[0].offset.y, px(2.));
        // CSS blurs of 2 and 4 are Gaussian sigmas of 1 and 2.
        assert_eq!(m[0].blur_radius, px(1.5));
    }
}
