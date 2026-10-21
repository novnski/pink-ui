//! Tips Panel – a grid of short tips, each with one action; a taken tip turns
//! into a done state and the count in the heading follows.
//!
//! Clean: one flat card whose edges are hairline shadows; the heading and the
//! Close row stay put while only the tip list scrolls. Classic: a gray tray
//! holding the heading and the footer, with the tips in a white card on top.
//! Tips rise in one after another when the panel mounts, a taken tip fills
//! with a faint wash and its button pops into a check and a done label.
//!
//! ```ignore
//! TipsPanel::new("tips", vec![
//!     Tip::new("search", "Quick search", "Try it")
//!         .description("Press ")
//!         .kbd("⌘K")
//!         .description(" to jump to any page.")
//!         .done_label("Got it"),
//! ])
//! .on_close(|_, _| {})
//! ```

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    AnyView, App, BoxShadow, ElementId, FontWeight, Hsla, IntoElement, Pixels, SharedString,
    Stateful, Task, TextRun, Window, div, font, linear_color_stop, linear_gradient, prelude::*, px,
    svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, AnimationExt, EASE, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, shadow, white_a},
    story,
};

pub const NAME: &str = "Tips Panel";
pub const DESCRIPTION: &str =
    "Grid of short tips with one action each – taken tips turn into a done state";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One run of a tip's description: plain text or a key cap (`<Kbd>`).
#[derive(Clone, Debug, PartialEq)]
pub enum TipText {
    Text(SharedString),
    Kbd(SharedString),
}

#[derive(Clone)]
pub struct Tip {
    pub id: SharedString,
    pub title: SharedString,
    /// Short explanation, in runs so it can hold a key cap.
    pub description: Vec<TipText>,
    /// Button text.
    pub action: SharedString,
    /// Text after the action was taken, e.g. "Enabled" (default "Done").
    pub done_label: Option<SharedString>,
    /// Shows the action as the dark button – use once for the most important tip.
    pub primary: bool,
    pub on_action: Option<Handler>,
}

impl Tip {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        action: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: Vec::new(),
            action: action.into(),
            done_label: None,
            primary: false,
            on_action: None,
        }
    }

    /// Appends text to the description.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description.push(TipText::Text(text.into()));
        self
    }

    /// Appends a key cap to the description, e.g. `⌘K`.
    pub fn kbd(mut self, key: impl Into<SharedString>) -> Self {
        self.description.push(TipText::Kbd(key.into()));
        self
    }

    pub fn done_label(mut self, label: impl Into<SharedString>) -> Self {
        self.done_label = Some(label.into());
        self
    }

    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    pub fn on_action(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }
}

/// The `--ui-tpc-*` (Clean) and `--ui-tp-*` (Classic) tokens.
struct Palette {
    /// Width of the 0.5px hairlines once snapped to device pixels.
    hair: Pixels,
    /// Classic only: the gray tray around the card.
    tray: Option<Hsla>,
    card: Hsla,
    /// Clean: the panel's own shadow. Classic: the card's.
    card_shadow: Vec<BoxShadow>,
    card_border: Option<Hsla>,
    title: Hsla,
    ink: Hsla,
    tip_title: Hsla,
    text: Hsla,
    muted: Hsla,
    line: Hsla,
    done_bg: Hsla,
    ok: Hsla,
    kbd_bg: Hsla,
    kbd_shadow: Vec<BoxShadow>,
    btn_bg: Hsla,
    btn_bg_hover: Hsla,
    btn_fg: Hsla,
    btn_shadow: Vec<BoxShadow>,
    btn_shadow_hover: Vec<BoxShadow>,
    primary: (Hsla, Hsla),
    primary_hover: (Hsla, Hsla),
    primary_shadow: Vec<BoxShadow>,
    /// Classic: the `inset 0 1px 0` highlight on the dark button.
    primary_highlight: Option<Hsla>,
    on_primary: Hsla,
}

impl Palette {
    /// `hair` is a 0.5px CSS hairline snapped to device pixels the way the
    /// browser draws it: one device pixel, never thinner than 0.5px.
    fn get(look: Look, hair: f32, cx: &App) -> Self {
        let dark = cx.is_dark();
        match (look, dark) {
            (Look::Clean, false) => {
                let shadow_rest = vec![
                    shadow(0., 0., 0., hair, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ];
                Palette {
                    hair: px(hair),
                    tray: None,
                    card: hex(0xffffff),
                    card_shadow: shadow_rest.clone(),
                    card_border: None,
                    title: hex(0x000000),
                    ink: black_a(0.88),
                    tip_title: hex(0x000000),
                    text: black_a(0.61),
                    muted: black_a(0.45),
                    line: black_a(0.06),
                    done_bg: black_a(0.015),
                    ok: hex(0x1a9a52),
                    kbd_bg: hex(0xffffff),
                    kbd_shadow: shadow_rest.clone(),
                    btn_bg: hex(0xffffff),
                    btn_bg_hover: hex(0xffffff),
                    btn_fg: hex(0x000000),
                    btn_shadow: shadow_rest,
                    btn_shadow_hover: vec![
                        shadow(0., 0., 0., hair, black_a(0.1)),
                        shadow(0., 1., 1., -1., black_a(0.12)),
                        shadow(0., 1., 3., 0., black_a(0.07)),
                    ],
                    primary: (black_a(0.88), black_a(0.88)),
                    primary_hover: (black_a(0.8), black_a(0.8)),
                    primary_shadow: Vec::new(),
                    primary_highlight: None,
                    on_primary: hex(0xfcfcfc),
                }
            }
            (Look::Clean, true) => {
                let shadow_rest = vec![
                    shadow(0., 0., 0., hair, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ];
                Palette {
                    hair: px(hair),
                    tray: None,
                    card: hex(0x1c1c1e),
                    card_shadow: shadow_rest.clone(),
                    card_border: None,
                    title: hex(0xffffff),
                    ink: white_a(0.9),
                    tip_title: hex(0xffffff),
                    text: white_a(0.62),
                    muted: white_a(0.46),
                    line: white_a(0.08),
                    done_bg: white_a(0.03),
                    ok: hex(0x5fd394),
                    kbd_bg: hex(0x1c1c1e),
                    kbd_shadow: shadow_rest.clone(),
                    btn_bg: hex(0x232326),
                    btn_bg_hover: hex(0x232326),
                    btn_fg: hex(0xffffff),
                    btn_shadow: shadow_rest,
                    btn_shadow_hover: vec![
                        shadow(0., 0., 0., hair, white_a(0.16)),
                        shadow(0., 1., 2., 0., black_a(0.4)),
                    ],
                    primary: (white_a(0.9), white_a(0.9)),
                    primary_hover: (white_a(0.8), white_a(0.8)),
                    primary_shadow: Vec::new(),
                    primary_highlight: None,
                    on_primary: hex(0x1c1c1e),
                }
            }
            (Look::Classic, false) => {
                let btn_shadow = vec![
                    shadow(0., 0., 0., 1., hex(0xe5e5e5)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ];
                Palette {
                    hair: px(hair),
                    tray: Some(hex(0xf0f0f0)),
                    card: hex(0xffffff),
                    card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
                    card_border: Some(hex(0xe5e5e5)),
                    title: hex(0x171717),
                    ink: hex(0x171717),
                    tip_title: hex(0x171717),
                    text: hex(0x737373),
                    muted: hex(0x737373),
                    line: hex(0xececec),
                    done_bg: hex(0xfafafa),
                    ok: hex(0x2f6bff),
                    kbd_bg: hex(0xffffff),
                    kbd_shadow: vec![
                        shadow(0., 0., 0., 1., hex(0xe5e5e5)),
                        shadow(0., 1., 0., 0., hex(0xe5e5e5)),
                    ],
                    btn_bg: hex(0xffffff),
                    btn_bg_hover: hex(0xf7f7f7),
                    btn_fg: hex(0x171717),
                    btn_shadow: btn_shadow.clone(),
                    btn_shadow_hover: btn_shadow,
                    primary: (hex(0x323137), hex(0x201e25)),
                    primary_hover: (hex(0x3b3a41), hex(0x26242b)),
                    primary_shadow: vec![
                        shadow(0., 0., 0., 1., hex(0x0d0d0d)),
                        shadow(0., 2., 4., 0., black_a(0.1)),
                    ],
                    primary_highlight: Some(white_a(0.12)),
                    on_primary: hex(0xffffff),
                }
            }
            (Look::Classic, true) => {
                let btn_shadow = vec![
                    shadow(0., 0., 0., 1., white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ];
                Palette {
                    hair: px(hair),
                    tray: Some(hex(0x161618)),
                    card: hex(0x232326),
                    card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                    card_border: Some(white_a(0.08)),
                    title: hex(0xededed),
                    ink: hex(0xededed),
                    tip_title: hex(0xededed),
                    text: hex(0x8f8f8f),
                    muted: hex(0x8f8f8f),
                    line: white_a(0.08),
                    done_bg: hex(0x2a2a2d),
                    ok: hex(0x6b95ff),
                    kbd_bg: hex(0x2a2a2d),
                    kbd_shadow: vec![
                        shadow(0., 0., 0., 1., white_a(0.1)),
                        shadow(0., 1., 0., 0., white_a(0.1)),
                    ],
                    btn_bg: hex(0x2a2a2d),
                    btn_bg_hover: hex(0x313135),
                    btn_fg: hex(0xededed),
                    btn_shadow: btn_shadow.clone(),
                    btn_shadow_hover: btn_shadow,
                    primary: (hex(0xf4f4f5), hex(0xe4e4e7)),
                    primary_hover: (hex(0xffffff), hex(0xececef)),
                    primary_shadow: vec![
                        shadow(0., 0., 0., 1., black_a(0.6)),
                        shadow(0., 2., 4., 0., black_a(0.3)),
                    ],
                    primary_highlight: Some(hex(0xffffff)),
                    on_primary: hex(0x121213),
                }
            }
        }
    }
}

/// Sizes from the two stylesheets.
struct Metrics {
    grid_pad: Pixels,
    grid_gap: Pixels,
    tip_pad: (Pixels, Pixels, Pixels),
    tip_radius: Pixels,
    tip_title_size: Pixels,
    text_margin: (Pixels, Pixels),
    btn_height: Pixels,
    btn_pad: Pixels,
    btn_radius: Pixels,
    kbd_height: Pixels,
    kbd_radius: Pixels,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                grid_pad: px(12.),
                grid_gap: px(8.),
                tip_pad: (px(12.), px(12.), px(12.)),
                tip_radius: px(8.),
                tip_title_size: px(13.),
                text_margin: (px(2.), px(12.)),
                btn_height: px(28.),
                btn_pad: px(10.),
                btn_radius: px(6.),
                kbd_height: px(18.),
                kbd_radius: px(5.),
            },
            Look::Classic => Metrics {
                grid_pad: px(14.),
                grid_gap: px(12.),
                tip_pad: (px(16.), px(16.), px(14.)),
                tip_radius: px(14.),
                tip_title_size: px(14.),
                text_margin: (px(6.), px(14.)),
                btn_height: px(30.),
                btn_pad: px(12.),
                btn_radius: px(9.),
                kbd_height: px(20.),
                kbd_radius: px(6.),
            },
        }
    }
}

/// Which tips were taken, in the order they were.
struct TipsState {
    done: Vec<SharedString>,
}

#[derive(IntoElement)]
pub struct TipsPanel {
    id: ElementId,
    title: SharedString,
    tips: Vec<Tip>,
    on_close: Option<Handler>,
    close_label: SharedString,
    max_height: Pixels,
    look: Option<Look>,
}

impl TipsPanel {
    pub fn new(id: impl Into<ElementId>, tips: Vec<Tip>) -> Self {
        Self {
            id: id.into(),
            title: "Tips".into(),
            tips,
            on_close: None,
            close_label: "Close".into(),
            max_height: px(420.),
            look: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Shows the Close button in the footer.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    pub fn close_label(mut self, label: impl Into<SharedString>) -> Self {
        self.close_label = label.into();
        self
    }

    /// Height of the tip list before it scrolls (`--ui-tpc-max-height`, 420px).
    pub fn max_height(mut self, height: Pixels) -> Self {
        self.max_height = height;
        self
    }

    /// Overrides the global Clean/Classic look for this panel.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for TipsPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let hair = (1. / window.scale_factor()).max(0.5);
        let p = Palette::get(look, hair, cx);
        let m = Metrics::get(look);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            |_, _| TipsState { done: Vec::new() },
        );
        let done = state.read(cx).done.clone();

        let tiles: Vec<_> = self
            .tips
            .iter()
            .enumerate()
            .map(|(ix, tip)| {
                let is_done = done.contains(&tip.id);
                let tile = tip_tile(&key, look, tip, is_done, &state, window, &p, &m);
                // `ui-tp-in`: 480ms, staggered 50ms after an 80ms beat
                let spec = MotionSpec::new(480, EASE_OUT_QUINT).with_delay(80 + ix as u64 * 50);
                motion::rise_in(
                    ElementId::NamedInteger("tip-in".into(), ix as u64),
                    spec,
                    4.,
                    tile,
                )
                .into_any_element()
            })
            .collect();

        // Three equal columns; rows stretch their tiles to the tallest one.
        let mut rows = Vec::new();
        let mut tiles = tiles.into_iter().peekable();
        while tiles.peek().is_some() {
            let mut row = div().flex().gap(m.grid_gap);
            for _ in 0..3 {
                row = match tiles.next() {
                    Some(tile) => row.child(div().flex().flex_1().min_w_0().child(tile)),
                    None => row.child(div().flex_1().min_w_0()),
                };
            }
            rows.push(row);
        }
        let grid = div()
            .flex()
            .flex_col()
            .gap(m.grid_gap)
            .p(m.grid_pad)
            .children(rows);

        let count = div()
            .flex_none()
            .whitespace_nowrap()
            .text_color(if clean { p.muted } else { p.text })
            .text_size(px(if clean { 12. } else { 13. }))
            .line_height(px(if clean { 16. } else { 20. }))
            .child(format!("{}/{} done", done.len(), self.tips.len()));
        let title = div()
            .text_color(p.title)
            .when(clean, |d| {
                d.text_size(px(15.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!clean, |d| {
                d.text_size(px(16.))
                    .line_height(px(22.))
                    .font_weight(FontWeight::SEMIBOLD)
            })
            .child(self.title.clone());
        let head = div()
            .flex()
            .justify_between()
            .gap(px(12.))
            .child(title)
            .child(count);
        let head = if clean {
            head.items_center()
                .py(px(12.))
                .px(px(16.))
                .border_b(p.hair)
                .border_color(p.line)
        } else {
            head.items_baseline().pt(px(13.)).px(px(20.)).pb(px(12.))
        };

        let body = div()
            .id("tips-body")
            .max_h(self.max_height)
            .overflow_y_scroll()
            .child(grid);

        let foot = self.on_close.clone().map(|on_close| {
            let close = button(
                format!("{key}-close"),
                self.close_label.clone(),
                false,
                &p,
                &m,
            )
            .on_click(move |_, window, cx| on_close(window, cx));
            let foot = div().flex();
            if clean {
                foot.justify_end()
                    .p(px(8.))
                    .border_t(p.hair)
                    .border_color(p.line)
                    .child(close)
            } else {
                foot.pt(px(8.)).px(px(10.)).pb(px(10.)).child(close)
            }
        });

        let root = div()
            .id(self.id.clone())
            .w_full()
            .max_w(px(760.))
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(p.ink);
        if clean {
            root.flex()
                .flex_col()
                .rounded(px(10.))
                .bg(p.card)
                .shadow(p.card_shadow.clone())
                .child(head)
                .child(body)
                .children(foot)
        } else {
            let has_foot = foot.is_some();
            root.p(px(6.))
                .when(!has_foot, |d| d.pb(px(8.)))
                .rounded(px(22.))
                .when_some(p.tray, |d, tray| d.bg(tray))
                .child(head)
                .child(
                    body.bg(p.card)
                        .when_some(p.card_border, |d, c| d.border_1().border_color(c))
                        .rounded(px(16.))
                        .shadow(p.card_shadow.clone()),
                )
                .children(foot)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn tip_tile(
    key: &str,
    look: Look,
    tip: &Tip,
    is_done: bool,
    state: &gpui::Entity<TipsState>,
    window: &Window,
    p: &Palette,
    m: &Metrics,
) -> Stateful<gpui::Div> {
    let clean = look == Look::Clean;
    // `transition: background-color 260ms ease`
    let wash = motion::follow(
        &format!("{key}-done-{}", tip.id),
        if is_done { 1. } else { 0. },
        MotionSpec::new(260, EASE),
    );
    let (pt, px_, pb) = m.tip_pad;
    // Clean draws the tile edge as an inset 0.5px shadow: one device pixel at
    // half strength on a 1x screen.
    let (edge, edge_color, edge_inset) = if clean {
        (p.hair, p.line.opacity(0.5 / f32::from(p.hair)), p.hair)
    } else {
        (px(1.), p.line, px(0.))
    };

    let action = if is_done {
        let label = tip.done_label.clone().unwrap_or_else(|| "Done".into());
        let folder = if clean { "clean/" } else { "" };
        let check = div()
            .relative()
            .size(px(16.))
            .flex_none()
            .child(
                svg()
                    .absolute()
                    .size(px(16.))
                    .path(format!("icons/tips-panel/{folder}check-disc.svg"))
                    .text_color(p.ok),
            )
            .child(
                svg()
                    .absolute()
                    .size(px(16.))
                    .path(format!("icons/tips-panel/{folder}check-mark.svg"))
                    .text_color(hex(0xffffff)),
            );
        // `ui-tp-pop`: fades in over 360ms (the 0.94 scale has no GPUI equivalent)
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .h(m.btn_height)
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.ok)
            .child(check)
            .child(label)
            .with_animation(
                ElementId::Name(format!("{}-done", tip.id).into()),
                MotionSpec::new(360, EASE_OUT_QUINT).animation(),
                |el, t| el.opacity(t),
            )
            .into_any_element()
    } else {
        let on_action = tip.on_action.clone();
        let id = tip.id.clone();
        let state = state.clone();
        button(
            format!("{key}-btn-{}", tip.id),
            tip.action.clone(),
            tip.primary,
            p,
            m,
        )
        .on_click(move |_, window, cx| {
            state.update(cx, |s, cx| {
                if !s.done.contains(&id) {
                    s.done.push(id.clone());
                }
                cx.notify();
            });
            if let Some(f) = &on_action {
                f(window, cx);
            }
        })
        .into_any_element()
    };

    div()
        .id(tip.id.clone())
        .flex()
        .flex_col()
        .items_start()
        .w_full()
        // the browser's inset shadow takes no room, a GPUI border does
        .pt(pt - edge_inset)
        .px(px_ - edge_inset)
        .pb(pb - edge_inset)
        .rounded(m.tip_radius)
        .border(edge)
        .border_color(edge_color)
        .bg(motion::mix(p.done_bg.opacity(0.), p.done_bg, wash))
        .child(
            div()
                .w_full()
                .text_size(m.tip_title_size)
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.tip_title)
                .child(tip.title.clone()),
        )
        .child(
            description(&tip.description, window, p, m)
                .flex_1()
                .w_full()
                .mt(m.text_margin.0)
                .mb(m.text_margin.1)
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(p.text),
        )
        .child(action)
}

/// The description: plain text wraps natively; with a key cap inside, the
/// words flow as wrapping items so the cap can sit inline. Each word gets its
/// measured width and the space after it, and the row may run one space past
/// its edge so a line's trailing space hangs like in a browser.
fn description(runs: &[TipText], window: &Window, p: &Palette, m: &Metrics) -> gpui::Div {
    if runs.iter().all(|r| matches!(r, TipText::Text(_))) {
        let text: String = runs
            .iter()
            .map(|r| match r {
                TipText::Text(t) | TipText::Kbd(t) => t.as_ref(),
            })
            .collect();
        return div().child(text);
    }

    // (piece, is key cap, followed by a space)
    let mut pieces: Vec<(SharedString, bool, bool)> = Vec::new();
    for run in runs {
        match run {
            TipText::Text(text) => {
                if text.starts_with(char::is_whitespace)
                    && let Some(last) = pieces.last_mut()
                {
                    last.2 = true;
                }
                let words: Vec<&str> = text.split_whitespace().collect();
                let trailing = text.ends_with(char::is_whitespace);
                for (ix, word) in words.iter().enumerate() {
                    let last = ix + 1 == words.len();
                    pieces.push((word.to_string().into(), false, !last || trailing));
                }
            }
            TipText::Kbd(key) => pieces.push((key.clone(), true, false)),
        }
    }

    let size = px(13.);
    let space = text_width(window, "x x", size, FontWeight::NORMAL)
        - text_width(window, "xx", size, FontWeight::NORMAL);
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .content_start()
        .mr(-space)
        .children(pieces.into_iter().map(|(piece, is_kbd, gap)| {
            let item = if is_kbd {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_none()
                    .min_w(px(20.))
                    .h(m.kbd_height)
                    .px(px(5.))
                    .rounded(m.kbd_radius)
                    .bg(p.kbd_bg)
                    .shadow(p.kbd_shadow.clone())
                    .text_size(px(11.))
                    .line_height(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.ink)
                    .child(piece)
            } else {
                let width = text_width(window, &piece, size, FontWeight::NORMAL);
                div().flex_none().w(width).whitespace_nowrap().child(piece)
            };
            item.when(gap, |d| d.mr(space))
        }))
}

/// `.ui-tpc__btn` / `.ui-tp__btn`, plain or primary.
fn button(
    key: String,
    label: SharedString,
    primary: bool,
    p: &Palette,
    m: &Metrics,
) -> Stateful<gpui::Div> {
    let base = div()
        .id(SharedString::from(key.clone()))
        .flex()
        .flex_none()
        .items_center()
        .h(m.btn_height)
        .px(m.btn_pad)
        .rounded(m.btn_radius)
        .text_size(px(13.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .cursor_pointer()
        .child(label);
    if primary {
        let (top, bottom) = p.primary;
        let (hover_top, hover_bottom) = p.primary_hover;
        let base = base
            .text_color(p.on_primary)
            .shadow(p.primary_shadow.clone())
            .when_some(p.primary_highlight, |d, c| d.border_t_1().border_color(c));
        if top == bottom {
            // Clean: a flat chip whose fill eases on hover
            base.hover_fill(key, top, hover_top)
        } else {
            // Classic: a gradient, which CSS can't transition either
            base.bg(linear_gradient(
                180.,
                linear_color_stop(top, 0.),
                linear_color_stop(bottom, 1.),
            ))
            .hover(move |s| {
                s.bg(linear_gradient(
                    180.,
                    linear_color_stop(hover_top, 0.),
                    linear_color_stop(hover_bottom, 1.),
                ))
            })
        }
    } else {
        let t = motion::hover_t(&key);
        base.text_color(p.btn_fg)
            .shadow(blend_shadows(&p.btn_shadow, &p.btn_shadow_hover, t))
            .hover_fill(key, p.btn_bg, p.btn_bg_hover)
    }
}

/// Eases a list of shadows toward another of the same length.
fn blend_shadows(rest: &[BoxShadow], hover: &[BoxShadow], t: f32) -> Vec<BoxShadow> {
    rest.iter()
        .zip(hover)
        .map(|(a, b)| BoxShadow {
            color: motion::mix(a.color, b.color, t),
            offset: gpui::point(
                px(motion::lerp(
                    f32::from(a.offset.x),
                    f32::from(b.offset.x),
                    t,
                )),
                px(motion::lerp(
                    f32::from(a.offset.y),
                    f32::from(b.offset.y),
                    t,
                )),
            ),
            blur_radius: px(motion::lerp(
                f32::from(a.blur_radius),
                f32::from(b.blur_radius),
                t,
            )),
            spread_radius: px(motion::lerp(
                f32::from(a.spread_radius),
                f32::from(b.spread_radius),
                t,
            )),
        })
        .collect()
}

/// The advance of `text` in Inter at `size`.
fn text_width(window: &Window, text: &str, size: Pixels, weight: FontWeight) -> Pixels {
    let mut face = font(SANS);
    face.weight = weight;
    let run = TextRun {
        len: text.len(),
        font: face,
        color: hex(0x000000),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(SharedString::from(text.to_string()), size, &[run], None)
        .width
}

/// The website's tips.
pub fn demo_tips() -> Vec<Tip> {
    vec![
        Tip::new("invite", "Invite your team", "Invite")
            .description("Work on projects together and keep feedback in one place.")
            .done_label("Invite sent"),
        Tip::new("search", "Quick search", "Try it")
            .description("Press ")
            .kbd("⌘K")
            .description(" to jump to any page, project or setting.")
            .done_label("Got it"),
        Tip::new("digest", "Weekly digest", "Turn on")
            .description("Get a short summary of what changed every Monday morning.")
            .done_label("Turned on"),
        Tip::new("slack", "Connect Slack", "Connect")
            .description("Post an update to a channel when something needs your attention.")
            .done_label("Connected"),
        Tip::new("mobile", "Mobile app", "Get the app")
            .description("Check activity and approve requests while you're on the go.")
            .done_label("Link sent"),
        Tip::new("pro", "Ready for Pro?", "See plans")
            .description("Unlimited projects, priority support and advanced analytics.")
            .done_label("Opened")
            .primary(),
        Tip::new("theme", "Dark mode", "Switch")
            .description("Switch the theme or follow your system setting automatically.")
            .done_label("Switched"),
        Tip::new("api", "API keys", "Create key")
            .description("Automate your workflow and pull data into your own tools.")
            .done_label("Key created"),
        Tip::new("export", "Export data", "Export")
            .description("Download every project as CSV whenever you need a copy.")
            .done_label("Export started"),
    ]
}

/// The demo's Close: the panel fades out, then a fresh one mounts 900ms later.
struct DemoState {
    generation: u32,
    closing: Option<Instant>,
    _reopen: Option<Task<()>>,
}

impl DemoState {
    fn close(&mut self, cx: &mut gpui::Context<Self>) {
        if self.closing.is_some() {
            return;
        }
        self.closing = Some(Instant::now());
        self._reopen = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(900))
                .await;
            this.update(cx, |s, cx| {
                s.generation += 1;
                s.closing = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

#[derive(IntoElement)]
struct TipsDemo;

impl RenderOnce for TipsDemo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let demo = window.use_keyed_state("tips-demo", cx, |_, _| DemoState {
            generation: 0,
            closing: None,
            _reopen: None,
        });
        let (generation, closing) = {
            let s = demo.read(cx);
            (s.generation, s.closing)
        };
        // `transition: opacity .26s` (the 0.98 scale is left out)
        let opacity = closing.map_or(1., |since| {
            let t = since.elapsed().as_secs_f32() / (0.26 * motion::speed_scale());
            if t < 1. {
                window.request_animation_frame();
            }
            1. - EASE.eval(t.min(1.))
        });
        let look = cx.look();
        let max_height = match look {
            Look::Clean => px(330.),
            Look::Classic => px(310.),
        };
        div().w(px(720.)).opacity(opacity).child(
            TipsPanel::new(
                SharedString::from(format!("tips-panel-{generation}")),
                demo_tips(),
            )
            .max_height(max_height)
            .on_close(move |_, cx| demo.update(cx, |s, cx| s.close(cx))),
        )
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| TipsDemo)
}
