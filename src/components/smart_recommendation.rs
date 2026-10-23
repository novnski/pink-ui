//! Smart Recommendation – suggestion card with a confidence signal,
//! alternatives to switch to and accept with undo.
//!
//! Clean: one white card with hairline shadows; the signal and the actions sit
//! in its last row. Classic: the white card sits on a gray tray, and the
//! actions live in the tray below it, like the Chat Composer.
//!
//! The heading folds the reason away (the chevron turns, the reason's height
//! glides), Alternatives opens a menu above the button to pick another option
//! (Escape, a click outside or a pick closes it; arrow keys move through it),
//! and Accept swaps the actions for a green "accepted" note with Undo.
//!
//! ```ignore
//! SmartRecommendation::new("sync", vec![
//!     Recommendation::new("incremental", "Incremental sync", Signal::Strong)
//!         .reason("Switching to **Incremental sync** saves about 40 minutes."),
//!     Recommendation::new("full", "Full refresh", Signal::Weak),
//! ])
//! .on_accept(|option, _, _| println!("{}", option.label))
//! ```

use std::{f32::consts::FRAC_PI_2, ops::Range, rc::Rc, sync::Arc};

use gpui::{
    AnyView, App, BoxShadow, ElementId, Entity, FocusHandle, Font, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseButton, Rgba, SharedString, Stateful, StyledText, TextRun, Transformation,
    Window, canvas, deferred, div, font, linear_color_stop, linear_gradient, prelude::*, px,
    radians, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, EASE_OUT_QUINT, MotionSpec},
    primitives::{self, SANS, black_a, hex, hexa, white_a},
    story,
};

pub const NAME: &str = "Smart Recommendation";
pub const DESCRIPTION: &str =
    "Suggestion card with a confidence signal, alternatives to switch to and accept with undo";

/// The reason folding in and out: `grid-template-rows 340ms`.
const FOLD: MotionSpec = MotionSpec::new(340, EASE_OUT_QUINT);
/// The chevron turning: `transform 280ms`.
const TURN: MotionSpec = MotionSpec::new(280, EASE_OUT_QUINT);
/// `ui-src-swap`: fades in from 2px below.
const SWAP: MotionSpec = MotionSpec::new(300, EASE_OUT_QUINT);
const SWAP_REASON: MotionSpec = MotionSpec::new(360, EASE_OUT_QUINT);
/// `ui-src-drop`: the menu rises 4px into place.
const DROP: MotionSpec = MotionSpec::new(180, EASE_OUT_QUINT);
/// The body line height the Classic card inherits.
const INHERITED_LINE: f32 = 18.571;

/// How sure the recommendation is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Signal {
    #[default]
    None,
    Weak,
    Good,
    Strong,
}

impl Signal {
    fn level(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        ["No signal", "Weak signal", "Good signal", "Strong signal"][self.level()]
    }
}

#[derive(Clone)]
pub struct Recommendation {
    pub id: SharedString,
    /// Short name, used in the accept button, e.g. "Incremental sync".
    pub label: SharedString,
    /// One sentence why, shown in the card; `**text**` is set in medium ink.
    pub reason: SharedString,
    pub signal: Signal,
}

impl Recommendation {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        signal: Signal,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            reason: SharedString::default(),
            signal,
        }
    }

    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = reason.into();
        self
    }
}

/// Splits `**bold**` markup into the plain text and the bold ranges.
fn parse_reason(source: &str) -> (SharedString, Vec<Range<usize>>) {
    let mut text = String::new();
    let mut bold = Vec::new();
    for (ix, part) in source.split("**").enumerate() {
        let start = text.len();
        text.push_str(part);
        if ix % 2 == 1 && !part.is_empty() {
            bold.push(start..text.len());
        }
    }
    (text.into(), bold)
}

/// Text runs for the reason: regular in `text`, the bold ranges in medium
/// `ink`. (Highlight colors would be blended over `text`, not replace it.)
fn reason_runs(reason: &str, bold: &[Range<usize>], text: Hsla, ink: Hsla) -> Vec<TextRun> {
    let run = |len: usize, weight: FontWeight, color: Hsla| TextRun {
        len,
        font: Font {
            weight,
            ..font(SANS)
        },
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::new();
    let mut at = 0;
    for range in bold {
        if range.start > at {
            runs.push(run(range.start - at, FontWeight::NORMAL, text));
        }
        runs.push(run(range.len(), FontWeight::MEDIUM, ink));
        at = range.end;
    }
    if reason.len() > at {
        runs.push(run(reason.len() - at, FontWeight::NORMAL, text));
    }
    runs
}

type Handler = Rc<dyn Fn(&Recommendation, &mut Window, &mut App)>;

/// The `--ui-src-*` / `--ui-sr-*` tokens.
struct Palette {
    tray: Option<Hsla>,
    card: Hsla,
    card_border: Option<Hsla>,
    card_shadow: Vec<BoxShadow>,
    strong: Hsla,
    ink: Hsla,
    text: Hsla,
    /// The signal label.
    signal: Hsla,
    faint: Hsla,
    icon: Hsla,
    chevron: Hsla,
    line: Hsla,
    head_hover: Hsla,
    bar: Hsla,
    /// Clean: the main signal's first bar at no signal; Classic: its first bar.
    level0: Hsla,
    warn: Hsla,
    accent: Hsla,
    /// The spark and the Recommended tag.
    accent_text: Hsla,
    ok: Hsla,
    ok_bg: Hsla,
    pill: Hsla,
    pill_text: Hsla,
    tag: Hsla,
    btn: Hsla,
    btn_hover: Hsla,
    btn_shadow: Vec<BoxShadow>,
    btn_shadow_hover: Vec<BoxShadow>,
    /// Clean: an open Alternatives button.
    btn_open: Hsla,
    /// Primary: top/bottom, hover top/bottom (solid in Clean).
    primary: [Hsla; 4],
    primary_fg: Hsla,
    primary_shadow: Vec<BoxShadow>,
    /// Classic: `inset 0 1px 0` highlight on the primary.
    primary_inset: Option<Hsla>,
    menu: Hsla,
    menu_shadow: Vec<BoxShadow>,
    option_hover: Hsla,
    option_on: Hsla,
    option_line: Option<Hsla>,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, dark: bool) -> Self {
        match (look, dark) {
            (Look::Clean, false) => Palette {
                tray: None,
                card: hex(0xffffff),
                card_border: None,
                card_shadow: clean_shadow(black_a(0.06), black_a(0.1), black_a(0.05), 2.),
                strong: hex(0x000000),
                ink: black_a(0.88),
                text: black_a(0.61),
                signal: black_a(0.61),
                faint: black_a(0.27),
                icon: black_a(0.447),
                chevron: black_a(0.447),
                line: black_a(0.06),
                head_hover: black_a(0.03),
                bar: black_a(0.1),
                level0: black_a(0.27),
                warn: hex(0xe8a20c),
                accent: hexa(0x0060e2, 0.765),
                accent_text: hexa(0x0060e2, 0.765),
                ok: hex(0x1a9a52),
                ok_bg: hexa(0x1a9a52, 0.08),
                pill: hexa(0x5b5bd6, 0.1),
                pill_text: hex(0x5b5bd6),
                tag: hexa(0x0060e2, 0.08),
                btn: hex(0xffffff),
                btn_hover: hex(0xffffff),
                btn_shadow: clean_shadow(black_a(0.06), black_a(0.1), black_a(0.05), 2.),
                btn_shadow_hover: clean_shadow(black_a(0.1), black_a(0.12), black_a(0.07), 3.),
                btn_open: black_a(0.03),
                primary: [black_a(0.88), black_a(0.88), black_a(0.8), black_a(0.8)],
                primary_fg: hex(0xfcfcfc),
                primary_shadow: Vec::new(),
                primary_inset: None,
                menu: hex(0xffffff),
                menu_shadow: vec![
                    ring(0.5, black_a(0.08)),
                    shadow(0., 12., 28., -10., black_a(0.16)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                option_hover: black_a(0.03),
                option_on: hexa(0x0060e2, 0.05),
                option_line: Some(hexa(0x0060e2, 0.35)),
                focus: hexa(0x0060e2, 0.35),
            },
            (Look::Clean, true) => Palette {
                tray: None,
                card: hex(0x1c1c1e),
                card_border: None,
                card_shadow: vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                strong: hex(0xffffff),
                ink: white_a(0.9),
                text: white_a(0.62),
                signal: white_a(0.62),
                faint: white_a(0.3),
                icon: white_a(0.46),
                chevron: white_a(0.46),
                line: white_a(0.08),
                head_hover: white_a(0.04),
                bar: white_a(0.14),
                level0: white_a(0.3),
                warn: hex(0xf0b232),
                accent: hex(0x4d8dff),
                accent_text: hex(0x4d8dff),
                ok: hex(0x3ec27a),
                ok_bg: hexa(0x1a9a52, 0.14),
                pill: hexa(0x5b5bd6, 0.22),
                pill_text: hex(0x9d9dff),
                tag: hexa(0x4d8dff, 0.16),
                btn: hex(0x1c1c1e),
                btn_hover: hex(0x1c1c1e),
                btn_shadow: vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                btn_shadow_hover: vec![
                    ring(0.5, white_a(0.16)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                btn_open: white_a(0.04),
                primary: [white_a(0.9), white_a(0.9), white_a(0.82), white_a(0.82)],
                primary_fg: hex(0x161618),
                primary_shadow: Vec::new(),
                primary_inset: None,
                menu: hex(0x232326),
                menu_shadow: vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 12., 28., -10., black_a(0.6)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                option_hover: white_a(0.04),
                option_on: hexa(0x4d8dff, 0.14),
                option_line: Some(hexa(0x4d8dff, 0.4)),
                focus: hexa(0x4d8dff, 0.5),
            },
            (Look::Classic, false) => Palette {
                tray: Some(hex(0xf0f0f0)),
                card: hex(0xffffff),
                card_border: Some(hex(0xe5e5e5)),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
                strong: hex(0x171717),
                ink: hex(0x171717),
                text: hex(0x404040),
                signal: hex(0x8f8f8f),
                faint: hex(0x8f8f8f),
                icon: hex(0x8f8f8f),
                chevron: hex(0x8f8f8f),
                line: hex(0xe5e5e5),
                head_hover: hex(0xf8f8f8),
                bar: hex(0xd4d4d4),
                level0: hex(0x8f8f8f),
                warn: hex(0xf59e0b),
                accent: hex(0x2f6bff),
                accent_text: hex(0x2f6bff),
                ok: hex(0x1f8a4c),
                ok_bg: hex(0x1f8a4c).opacity(0.),
                pill: hex(0xf4f0ff),
                pill_text: hex(0x7c4dff),
                tag: hex(0xeef3ff),
                btn: hex(0xffffff),
                btn_hover: hex(0xf7f7f7),
                btn_shadow: classic_btn_shadow(hex(0xe5e5e5)),
                btn_shadow_hover: classic_btn_shadow(hex(0xe5e5e5)),
                btn_open: hex(0xffffff),
                primary: [hex(0x323137), hex(0x201e25), hex(0x3b3a41), hex(0x26242b)],
                primary_fg: hex(0xffffff),
                primary_shadow: vec![
                    ring(1., hex(0x0d0d0d)),
                    shadow(0., 2., 4., 0., black_a(0.1)),
                ],
                primary_inset: Some(white_a(0.12)),
                menu: hex(0xffffff),
                menu_shadow: vec![
                    ring(1., hex(0xe5e5e5)),
                    shadow(0., 12., 30., -8., black_a(0.2)),
                ],
                option_hover: hex(0xf5f5f5),
                option_on: hex(0xf5f8ff),
                option_line: None,
                focus: hex(0x2f6bff),
            },
            (Look::Classic, true) => Palette {
                tray: Some(hex(0x161618)),
                card: hex(0x232326),
                card_border: Some(white_a(0.08)),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                strong: hex(0xededed),
                ink: hex(0xededed),
                text: hex(0xc4c4c4),
                signal: hex(0x8f8f8f),
                faint: hex(0x8f8f8f),
                icon: hex(0x8f8f8f),
                chevron: hex(0x8f8f8f),
                line: white_a(0.08),
                head_hover: white_a(0.06),
                bar: white_a(0.14),
                level0: hex(0x8f8f8f),
                warn: hex(0xf59e0b),
                accent: hex(0x2f6bff),
                accent_text: hex(0x6b95ff),
                ok: hex(0x3dbb6f),
                ok_bg: hex(0x3dbb6f).opacity(0.),
                pill: hexa(0x8b5cf6, 0.18),
                pill_text: hex(0xb49cff),
                tag: hexa(0x2f6bff, 0.16),
                btn: hex(0x2a2a2d),
                btn_hover: hex(0x313135),
                btn_shadow: classic_btn_shadow(white_a(0.08)),
                btn_shadow_hover: classic_btn_shadow(white_a(0.08)),
                btn_open: hex(0x2a2a2d),
                primary: [hex(0xf4f4f5), hex(0xe4e4e7), hex(0xffffff), hex(0xececef)],
                primary_fg: hex(0x121213),
                primary_shadow: vec![
                    ring(1., black_a(0.6)),
                    shadow(0., 2., 4., 0., black_a(0.35)),
                ],
                primary_inset: Some(hex(0xffffff)),
                menu: hex(0x2a2a2d),
                menu_shadow: vec![
                    ring(1., white_a(0.1)),
                    shadow(0., 12., 30., -8., black_a(0.5)),
                ],
                option_hover: hex(0x313135),
                option_on: hexa(0x2f6bff, 0.16),
                option_line: None,
                focus: hex(0x2f6bff),
            },
        }
    }

    /// The color of the bars up to `level`.
    fn level_color(&self, level: usize) -> Hsla {
        match level {
            1 => self.warn,
            2 => self.accent,
            _ => self.ok,
        }
    }
}

/// Clean's hairline plus a soft lift.
fn clean_shadow(hairline: Hsla, edge: Hsla, lift: Hsla, lift_blur: f32) -> Vec<BoxShadow> {
    vec![
        ring(0.5, hairline),
        shadow(0., 1., 1., -1., edge),
        shadow(0., 1., lift_blur, 0., lift),
    ]
}

fn classic_btn_shadow(border: Hsla) -> Vec<BoxShadow> {
    vec![ring(1., border), shadow(0., 1., 2., 0., black_a(0.04))]
}

/// `box-shadow: x y blur spread color` (CSS values; `primitives::shadow`
/// converts them for GPUI).
fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    primitives::shadow(x, y, blur, spread, color)
}

/// `box-shadow: 0 0 0 <width> color`.
fn ring(width: f32, color: Hsla) -> BoxShadow {
    shadow(0., 0., 0., width, color)
}

/// `top` painted over `bottom` (source-over).
fn over(top: Hsla, bottom: Hsla) -> Hsla {
    let (t, b) = (Rgba::from(top), Rgba::from(bottom));
    let a = t.a + b.a * (1. - t.a);
    if a <= f32::EPSILON {
        return top.opacity(0.);
    }
    let channel = |x: f32, y: f32| (x * t.a + y * b.a * (1. - t.a)) / a;
    Hsla::from(Rgba {
        r: channel(t.r, b.r),
        g: channel(t.g, b.g),
        b: channel(t.b, b.b),
        a,
    })
}

/// Eases each shadow layer toward its hover twin.
fn mix_shadows(rest: &[BoxShadow], hover: &[BoxShadow], t: f32) -> Vec<BoxShadow> {
    if rest.len() != hover.len() {
        return if t < 0.5 {
            rest.to_vec()
        } else {
            hover.to_vec()
        };
    }
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
            inset: a.inset,
        })
        .collect()
}

fn icon_path(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/smart-recommendation/clean/{name}.svg").into(),
        Look::Classic => format!("icons/smart-recommendation/{name}.svg").into(),
    }
}

/// `outline: 2px solid; outline-offset: <offset>` as a ring `out` px outside
/// the element (`out = offset + 2`).
fn focus_ring(out: f32, radius: f32, color: Hsla) -> impl IntoElement {
    div()
        .absolute()
        .top(px(-out))
        .left(px(-out))
        .right(px(-out))
        .bottom(px(-out))
        .rounded(px((radius + out).max(0.)))
        .border_2()
        .border_color(color)
}

struct Focus {
    head: FocusHandle,
    alt: FocusHandle,
    accept: FocusHandle,
    undo: FocusHandle,
    options: Vec<FocusHandle>,
}

struct State {
    chosen: SharedString,
    accepted: Option<SharedString>,
    open: bool,
    menu: bool,
    /// The pointer is on the Alternatives button: a press there toggles the
    /// menu instead of counting as a click outside it.
    alt_hovered: bool,
    /// Focus rings show only after keyboard use (`:focus-visible`).
    keyboard: bool,
    /// The reason paragraph's height, measured each frame for the fold.
    reason_height: f32,
    focus: Focus,
}

#[derive(IntoElement)]
pub struct SmartRecommendation {
    id: ElementId,
    title: SharedString,
    options: Vec<Recommendation>,
    on_accept: Option<Handler>,
    on_undo: Option<Handler>,
    default_open: bool,
    look: Option<Look>,
}

impl SmartRecommendation {
    /// The first option is the recommendation, the rest are offered as alternatives.
    pub fn new(id: impl Into<ElementId>, options: Vec<Recommendation>) -> Self {
        Self {
            id: id.into(),
            title: "Smart recommendation".into(),
            options,
            on_accept: None,
            on_undo: None,
            default_open: true,
            look: None,
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn on_accept(
        mut self,
        handler: impl Fn(&Recommendation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_accept = Some(Rc::new(handler));
        self
    }

    pub fn on_undo(
        mut self,
        handler: impl Fn(&Recommendation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }

    /// Starts with the reason folded away when `false`.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Overrides the global Clean/Classic look for this card.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for SmartRecommendation {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx.is_dark());
        let key = format!("sr-{}", self.id);
        let options = Rc::new(self.options);
        if options.is_empty() {
            return div().id(self.id);
        }

        let first = options[0].id.clone();
        let default_open = self.default_open;
        let count = options.len();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| State {
                chosen: first,
                accepted: None,
                open: default_open,
                menu: false,
                alt_hovered: false,
                keyboard: false,
                reason_height: 0.,
                focus: Focus {
                    head: cx.focus_handle().tab_stop(true),
                    alt: cx.focus_handle().tab_stop(true),
                    accept: cx.focus_handle().tab_stop(true),
                    undo: cx.focus_handle().tab_stop(true),
                    options: Vec::new(),
                },
            },
        );
        state.update(cx, |s, cx| {
            while s.focus.options.len() < count {
                s.focus.options.push(cx.focus_handle().tab_stop(true));
            }
        });
        let s = state.read(cx);
        let (open, menu, keyboard, measured) = (s.open, s.menu, s.keyboard, s.reason_height);
        let current_ix = options.iter().position(|o| o.id == s.chosen).unwrap_or(0);
        let current = options[current_ix].clone();
        let recommended = current_ix == 0;
        let done = s.accepted.as_ref() == Some(&current.id);
        let focus_head = s.focus.head.clone();
        let focus_alt = s.focus.alt.clone();
        let focus_accept = s.focus.accept.clone();
        let focus_undo = s.focus.undo.clone();
        let focus_options = s.focus.options.clone();
        let visible = |handle: &FocusHandle, window: &Window| keyboard && handle.is_focused(window);

        // ---- Heading: folds the reason in and out --------------------------
        let head_key = format!("{key}-head");
        let head_t = motion::hover_t(&head_key);
        let turn = motion::follow(
            &format!("{key}-chevron"),
            if open { 0. } else { -FRAC_PI_2 },
            TURN,
        );
        let (head_radius, head_ring) = if clean { (10., 0.) } else { (12., 4.) };
        // Spacing by margins: GPUI's layout drops flex gaps in a row that
        // also has an auto margin, which pushed the chevron off its spot.
        let head_gap = px(if clean { 8. } else { 10. });
        let head = div()
            .id("head")
            .track_focus(&focus_head)
            .relative()
            .flex()
            .items_center()
            .w_full()
            .cursor_pointer()
            .hover_fill(head_key.clone(), p.head_hover.opacity(0.), p.head_hover)
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.open = !s.open;
                        cx.notify();
                    })
                }
            })
            .map(|d| {
                if clean {
                    d.h(px(44.)).pl(px(16.)).pr(px(12.)).rounded_t(px(10.))
                } else {
                    d.h(px(40.)).px(px(10.)).rounded(px(12.))
                }
            })
            .child(
                svg()
                    .path(icon_path(look, if clean { "mark" } else { "spark" }))
                    .size(px(16.))
                    .flex_none()
                    .mr(head_gap)
                    .text_color(p.accent_text),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.strong)
                    .when(!clean, |d| d.text_size(px(15.)))
                    .child(self.title.clone()),
            )
            .when(!recommended, |d| {
                let pill = div()
                    .flex_none()
                    .ml(head_gap)
                    .font_weight(FontWeight::MEDIUM)
                    .bg(p.pill)
                    .text_color(p.pill_text)
                    .child("Alternative");
                let pill = if clean {
                    pill.px(px(6.))
                        .rounded(px(5.))
                        .text_size(px(12.))
                        .line_height(px(20.))
                } else {
                    pill.px(px(8.))
                        .py(px(1.))
                        .rounded_full()
                        .text_size(px(11.5))
                        .line_height(px(INHERITED_LINE))
                };
                d.child(motion::rise_in(
                    ElementId::Name(format!("{key}-pill").into()),
                    SWAP,
                    2.,
                    pill,
                ))
            })
            .child(div().flex_1().min_w(head_gap))
            .child(
                div()
                    .flex_none()
                    .size(px(if clean { 20. } else { 16. }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(icon_path(look, "chevron"))
                            .size(px(16.))
                            .text_color(if clean {
                                motion::mix(p.chevron, p.ink, head_t)
                            } else {
                                p.chevron
                            })
                            .with_transformation(Transformation::rotate(radians(turn))),
                    ),
            )
            .when(visible(&focus_head, window), |d| {
                d.child(focus_ring(head_ring, head_radius, p.focus))
            });

        // ---- The reason, which folds ---------------------------------------
        let fold = motion::follow(&format!("{key}-fold"), if open { 1. } else { 0. }, FOLD);
        let settled = (fold - if open { 1. } else { 0. }).abs() < 0.001;
        let reason_pb = if open {
            if clean { 14. } else { 8. }
        } else {
            0.
        };
        let (reason_text, bold) = parse_reason(&current.reason);
        let runs = reason_runs(&reason_text, &bold, p.text, p.ink);
        let paragraph = div()
            .relative()
            .child(StyledText::new(reason_text).with_runs(runs))
            .child(
                canvas(
                    {
                        let state = state.clone();
                        move |bounds, _, cx| {
                            let height = f32::from(bounds.size.height);
                            state.update(cx, |s, cx| {
                                if (s.reason_height - height).abs() > 0.5 {
                                    s.reason_height = height;
                                    cx.notify();
                                }
                            });
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        let reason = div()
            .px(px(if clean { 16. } else { 10. }))
            .pb(px(reason_pb))
            .text_color(p.text)
            .map(|d| {
                if clean {
                    d.text_size(px(13.)).line_height(px(20.))
                } else {
                    d.text_size(px(14.)).line_height(px(21.))
                }
            })
            .child(paragraph);
        let body = div()
            .overflow_hidden()
            .when(!(open && settled), |d| {
                d.h(px((measured + reason_pb) * fold))
            })
            .child(motion::rise_in(
                ElementId::Name(format!("{key}-reason-{}", current.id).into()),
                SWAP_REASON,
                2.,
                reason,
            ));

        // ---- Signal ---------------------------------------------------------
        let level = current.signal.level();
        let heights = if clean { [5., 8., 11.] } else { [6., 9., 12.] };
        // The level colors only reach the menu's mini bars, so the main
        // signal stays neutral; at no signal Clean shades the whole
        // bar group and Classic its first bar.
        let main_bars = div()
            .flex()
            .items_end()
            .gap(px(2.))
            .when(clean && level == 0, |d| d.bg(p.level0))
            .children(heights.iter().enumerate().map(|(ix, h)| {
                let color = if !clean && level == 0 && ix == 0 {
                    p.level0
                } else {
                    p.bar
                };
                div().w(px(3.)).h(px(*h)).rounded(px(1.)).bg(color)
            }));
        let signal = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .whitespace_nowrap()
            .text_color(p.signal)
            .map(|d| {
                if clean {
                    d.text_size(px(12.)).line_height(px(16.))
                } else {
                    d.text_size(px(13.)).line_height(px(INHERITED_LINE))
                }
            })
            .mr(px(10.))
            .child(main_bars)
            .child(current.signal.text());

        // ---- Actions ----------------------------------------------------------
        let button_h = if clean { 28. } else { 32. };
        let mut actions = div()
            .flex()
            .items_center()
            .gap(px(if clean { 8. } else { 6. }))
            .ml_auto();

        if options.len() > 1 && !done {
            let alt_key = format!("{key}-alt");
            let mut alt =
                action_button("alternatives", &alt_key, look, &p, false, menu, &focus_alt)
                    .when(clean, |d| d.pl(px(8.)))
                    .on_hover({
                        let state = state.clone();
                        let alt_key = alt_key.clone();
                        move |hovered, window, cx| {
                            motion::set_hover(&alt_key, *hovered, motion::reduced_motion(cx));
                            state.update(cx, |s, _| s.alt_hovered = *hovered);
                            window.refresh();
                        }
                    })
                    .on_click({
                        let state = state.clone();
                        let first_option = focus_options.first().cloned();
                        move |event, window, cx| {
                            let opened = state.update(cx, |s, cx| {
                                s.menu = !s.menu;
                                cx.notify();
                                s.menu
                            });
                            // Opened from the keyboard: move into the menu
                            if opened
                                && event.is_keyboard()
                                && let Some(handle) = &first_option
                            {
                                window.focus(handle, cx);
                            }
                        }
                    })
                    .when(clean, |d| {
                        d.child(
                            svg()
                                .path(icon_path(look, "swap"))
                                .size(px(16.))
                                .flex_none()
                                .text_color(p.icon),
                        )
                    })
                    .child("Alternatives");
            if visible(&focus_alt, window) {
                alt = alt.child(focus_ring(
                    if clean { 3. } else { 4. },
                    if clean { 6. } else { 10. },
                    p.focus,
                ));
            }
            let wrapper = div().relative().child(alt).when(menu, |d| {
                d.child(alternatives_menu(
                    &key,
                    look,
                    &p,
                    &options,
                    current_ix,
                    &state,
                    &focus_options,
                    keyboard,
                    button_h,
                    window,
                ))
            });
            actions = actions.child(wrapper);
        }

        if done {
            let check = icon_path(look, "check");
            let note = div()
                .flex()
                .items_center()
                .gap(px(6.))
                .min_w_0()
                .font_weight(FontWeight::MEDIUM)
                .text_size(px(13.))
                .text_color(p.ok)
                .map(|d| {
                    if clean {
                        d.h(px(28.))
                            .pl(px(8.))
                            .pr(px(10.))
                            .rounded(px(6.))
                            .bg(p.ok_bg)
                            .line_height(px(20.))
                    } else {
                        d.pr(px(4.)).line_height(px(INHERITED_LINE))
                    }
                })
                .child(
                    svg()
                        .path(check)
                        .size(px(if clean { 16. } else { 14. }))
                        .flex_none()
                        .text_color(p.ok),
                )
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(format!("{} accepted", current.label)),
                );
            let undo_key = format!("{key}-undo");
            let mut undo = action_button("undo", &undo_key, look, &p, false, false, &focus_undo)
                .when(clean, |d| d.pl(px(8.)))
                .on_hover(motion::hover_listener(undo_key))
                .on_click({
                    let state = state.clone();
                    let on_undo = self.on_undo.clone();
                    let current = current.clone();
                    move |_, window, cx| {
                        state.update(cx, |s, cx| {
                            s.accepted = None;
                            cx.notify();
                        });
                        if let Some(f) = &on_undo {
                            f(&current, window, cx);
                        }
                    }
                })
                .when(clean, |d| {
                    d.child(
                        svg()
                            .path(icon_path(look, "undo"))
                            .size(px(16.))
                            .flex_none()
                            .text_color(p.icon),
                    )
                })
                .child("Undo");
            if visible(&focus_undo, window) {
                undo = undo.child(focus_ring(
                    if clean { 3. } else { 4. },
                    if clean { 6. } else { 10. },
                    p.focus,
                ));
            }
            actions = actions
                .child(motion::rise_in(
                    ElementId::Name(format!("{key}-done-{}", current.id).into()),
                    SWAP,
                    2.,
                    note,
                ))
                .child(undo);
        } else {
            let accept_key = format!("{key}-accept");
            let mut accept =
                action_button("accept", &accept_key, look, &p, true, false, &focus_accept)
                    .on_hover(motion::hover_listener(accept_key))
                    .on_click({
                        let state = state.clone();
                        let on_accept = self.on_accept.clone();
                        let current = current.clone();
                        move |_, window, cx| {
                            state.update(cx, |s, cx| {
                                s.accepted = Some(current.id.clone());
                                s.menu = false;
                                cx.notify();
                            });
                            if let Some(f) = &on_accept {
                                f(&current, window, cx);
                            }
                        }
                    })
                    .child(format!("Accept {}", current.label.to_lowercase()));
            if visible(&focus_accept, window) {
                accept = accept.child(focus_ring(
                    if clean { 3. } else { 4. },
                    if clean { 6. } else { 10. },
                    p.focus,
                ));
            }
            actions = actions.child(accept);
        }

        let foot = div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            // Column spacing comes from the signal's margin (see the head)
            .gap_y(px(8.))
            .map(|d| {
                if clean {
                    // The browser snaps the 0.5px hairline to one device pixel;
                    // GPUI would drop it.
                    d.min_h(px(48.))
                        .py(px(10.))
                        .pl(px(16.))
                        .pr(px(10.))
                        .border_t_1()
                        .border_color(p.line)
                } else {
                    d.min_h(px(50.)).py(px(8.)).pl(px(16.)).pr(px(8.))
                }
            })
            .child(signal)
            .child(actions);

        // ---- Keyboard: Escape closes the menu, arrows walk it, Tab moves on --
        let on_key = {
            let state = state.clone();
            let focus_alt = focus_alt.clone();
            let focus_options = focus_options[..count].to_vec();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let key = event.keystroke.key.as_str();
                let menu = state.read(cx).menu;
                if matches!(
                    key,
                    "tab" | "up" | "down" | "home" | "end" | "escape" | "enter" | "space"
                ) {
                    state.update(cx, |s, cx| {
                        if !s.keyboard {
                            s.keyboard = true;
                            cx.notify();
                        }
                    });
                }
                match key {
                    "escape" if menu => {
                        state.update(cx, |s, cx| {
                            s.menu = false;
                            cx.notify();
                        });
                        window.focus(&focus_alt, cx);
                        cx.stop_propagation();
                    }
                    "up" | "down" | "home" | "end" if menu => {
                        let len = focus_options.len();
                        let at = focus_options.iter().position(|f| f.is_focused(window));
                        let next = match key {
                            "down" => at.map_or(0, |i| (i + 1) % len),
                            "up" => at.map_or(len - 1, |i| (i + len - 1) % len),
                            "home" => 0,
                            _ => len - 1,
                        };
                        window.focus(&focus_options[next], cx);
                        cx.stop_propagation();
                    }
                    "tab" => {
                        if event.keystroke.modifiers.shift {
                            window.focus_prev(cx);
                        } else {
                            window.focus_next(cx);
                        }
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }
        };

        let root = div()
            .id(self.id.clone())
            .w_full()
            .max_w(px(440.))
            .font_family(SANS)
            .text_color(p.ink)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, {
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        if s.keyboard {
                            s.keyboard = false;
                            cx.notify();
                        }
                    })
                }
            });
        motion::keep_alive(window);
        if clean {
            root.text_size(px(13.))
                .line_height(px(20.))
                .rounded(px(10.))
                .bg(p.card)
                .shadow(p.card_shadow.clone())
                .child(head)
                .child(body)
                .child(foot)
        } else {
            let card = div()
                .pt(px(6.))
                .px(px(8.))
                .pb(px(8.))
                .bg(p.card)
                .rounded(px(16.))
                .shadow(p.card_shadow.clone())
                .when_some(p.card_border, |d, border| d.border_1().border_color(border))
                .child(head)
                .child(body);
            root.text_size(px(13.))
                .line_height(px(INHERITED_LINE))
                .p(px(6.))
                .rounded(px(22.))
                .when_some(p.tray, |d, tray| d.bg(tray))
                .child(card)
                .child(foot)
        }
    }
}

/// The Alternatives / Undo button (`primary: false`) or the dark Accept chip.
/// Hover is wired by the caller; Clean icon buttons pull their left padding in.
fn action_button(
    id: &'static str,
    key: &str,
    look: Look,
    p: &Palette,
    primary: bool,
    expanded: bool,
    focus: &FocusHandle,
) -> Stateful<gpui::Div> {
    let clean = look == Look::Clean;
    let t = motion::hover_t(key);
    let base = div()
        .id(id)
        .track_focus(focus)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .whitespace_nowrap()
        .font_weight(FontWeight::MEDIUM)
        .text_size(px(13.))
        .cursor_pointer()
        .map(|d| {
            if clean {
                d.h(px(28.))
                    .px(px(10.))
                    .rounded(px(6.))
                    .line_height(px(20.))
            } else {
                d.h(px(32.))
                    .px(px(12.))
                    .rounded(px(10.))
                    .line_height(px(INHERITED_LINE))
            }
        });
    if primary {
        let [top, bottom, hover_top, hover_bottom] = p.primary;
        let top = motion::mix(top, hover_top, t);
        let bottom = motion::mix(bottom, hover_bottom, t);
        base.bg(linear_gradient(
            180.,
            linear_color_stop(top, 0.),
            linear_color_stop(bottom, 1.),
        ))
        .text_color(p.primary_fg)
        .shadow(p.primary_shadow.clone())
        .when_some(p.primary_inset, |d, inset| {
            d.child(
                div()
                    .absolute()
                    .inset_0()
                    .rounded(px(10.))
                    .border_t_1()
                    .border_color(inset),
            )
        })
    } else {
        // GPUI paints the shadow under the whole button, so the open wash is
        // laid over the card here instead of left translucent. Clean keeps
        // the wash while hovered; Classic hovers as usual.
        let open = over(p.btn_open, p.card);
        let (rest, hover) = match (expanded, clean) {
            (true, true) => (open, open),
            (true, false) => (open, p.btn_hover),
            (false, _) => (p.btn, p.btn_hover),
        };
        base.bg(motion::mix(rest, hover, t))
            .text_color(p.strong)
            .shadow(mix_shadows(&p.btn_shadow, &p.btn_shadow_hover, t))
    }
}

#[allow(clippy::too_many_arguments)]
fn alternatives_menu(
    key: &str,
    look: Look,
    p: &Palette,
    options: &Rc<Vec<Recommendation>>,
    current_ix: usize,
    state: &Entity<State>,
    focus: &[FocusHandle],
    keyboard: bool,
    button_h: f32,
    window: &Window,
) -> impl IntoElement {
    let clean = look == Look::Clean;
    let heights = if clean { [5., 8., 11.] } else { [6., 9., 12.] };
    let rows = options.iter().enumerate().map(|(ix, option)| {
        let row_key = format!("{key}-option-{}", option.id);
        let checked = ix == current_ix;
        let level = option.signal.level();
        let mini = div().flex().flex_none().items_end().gap(px(2.)).children(
            heights.iter().enumerate().map(|(bar, h)| {
                let color = if level > bar {
                    p.level_color(level)
                } else if clean && level == 0 && bar == 0 {
                    p.faint
                } else {
                    p.bar
                };
                div().w(px(3.)).h(px(*h)).rounded(px(1.)).bg(color)
            }),
        );
        let tag = (ix == 0).then(|| {
            let tag = div()
                .flex_none()
                .font_weight(FontWeight::MEDIUM)
                .bg(p.tag)
                .text_color(p.accent_text)
                .child("Recommended");
            if clean {
                tag.px(px(5.))
                    .rounded(px(5.))
                    .text_size(px(11.))
                    .line_height(px(18.))
            } else {
                tag.px(px(6.))
                    .rounded_full()
                    .text_size(px(10.5))
                    .line_height(px(16.))
            }
        });
        let rest = if checked {
            p.option_on
        } else {
            p.option_hover.opacity(0.)
        };
        let hover = if checked { p.option_on } else { p.option_hover };
        let handle = focus[ix].clone();
        let ring = keyboard && handle.is_focused(window);
        div()
            .id(ElementId::Name(row_key.clone().into()))
            .track_focus(&handle)
            .relative()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .text_size(px(13.))
            .text_color(p.ink)
            .cursor_pointer()
            .bg(motion::hover_blend(&row_key, rest, hover))
            .on_hover(motion::hover_listener(row_key))
            .map(|d| {
                if clean {
                    // The checked row's inset 0.5px hairline: one pixel at
                    // half strength, taken back out of the padding.
                    let line = p.option_line.filter(|_| checked);
                    let inset = if line.is_some() { 1. } else { 0. };
                    d.h(px(32.))
                        .pl(px(8. - inset))
                        .pr(px(10. - inset))
                        .rounded(px(6.))
                        .line_height(px(20.))
                        .when_some(line, |d, line| d.border_1().border_color(line.opacity(0.5)))
                } else {
                    d.min_h(px(34.))
                        .px(px(10.))
                        .rounded(px(8.))
                        .line_height(px(INHERITED_LINE))
                }
            })
            .on_click({
                let state = state.clone();
                let id = option.id.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.chosen = id.clone();
                        s.menu = false;
                        s.open = true;
                        cx.notify();
                    })
                }
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .min_w_0()
                    .child(div().min_w_0().truncate().child(option.label.clone()))
                    .children(tag),
            )
            .child(mini)
            .when(ring, |d| {
                d.child(focus_ring(
                    if clean { 0. } else { 4. },
                    if clean { 6. } else { 8. },
                    p.focus,
                ))
            })
    });
    let panel = div()
        .id("menu")
        .occlude()
        .flex()
        .flex_col()
        .gap(px(2.))
        .p(px(4.))
        .w(px(if clean { 248. } else { 260. }))
        .rounded(px(if clean { 10. } else { 12. }))
        .bg(p.menu)
        .shadow(p.menu_shadow.clone())
        .on_mouse_down_out({
            let state = state.clone();
            move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if !s.alt_hovered && s.menu {
                        s.menu = false;
                        cx.notify();
                    }
                })
            }
        })
        .children(rows);
    deferred(
        div()
            .absolute()
            .right_0()
            .bottom(px(button_h + if clean { 6. } else { 8. }))
            .child(motion::rise_in(
                ElementId::Name(format!("{key}-menu").into()),
                DROP,
                4.,
                panel,
            )),
    )
    .with_priority(1)
}

/// The demo's data: a nightly data sync.
pub fn demo_options() -> Vec<Recommendation> {
    vec![
        Recommendation::new("incremental", "Incremental sync", Signal::Strong).reason(
            "Only 4% of rows changed since yesterday – switching to **Incremental sync** saves about 40 minutes.",
        ),
        Recommendation::new("full", "Full refresh", Signal::Weak).reason(
            "A **Full refresh** rebuilds every table. Safest after schema changes, but takes about 50 minutes.",
        ),
        Recommendation::new("skip", "Skip tonight", Signal::None).reason(
            "**Skip tonight** keeps yesterday's data. Fine if nobody needs fresh numbers tomorrow morning.",
        ),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div().w(px(440.)).child(SmartRecommendation::new(
            "smart-recommendation",
            demo_options(),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_markup() {
        let (text, bold) = parse_reason("Only 4% – switching to **Incremental sync** saves time.");
        assert_eq!(&text[bold[0].clone()], "Incremental sync");
        assert!(!text.contains("**"));
        let (text, bold) = parse_reason("**Skip tonight** keeps data.");
        assert_eq!(&text[bold[0].clone()], "Skip tonight");
    }
}
