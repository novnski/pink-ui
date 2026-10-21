//! Reaction Bar – like, comment, repost and share in one row.
//!
//! Clean: a row of quiet chips with a hairline edge; taking an action tints
//! the chip blue. Classic: one white pill on a soft shadow; taking an action
//! colors its icon (pink like, blue comment, violet repost). Either way the
//! icon pops (shrinks, springs past full size, settles), the heart and the
//! bubble fill in, and the count rolls in digit by digit while the chip
//! glides to its new width. Arrow keys, Home and End move between the
//! actions, Enter or Space takes the focused one.
//!
//! ```ignore
//! ReactionBar::new("post-reactions")
//!     .reaction(ReactionKey::Like, ReactionState::new(128))
//!     .reaction(ReactionKey::Comment, ReactionState::new(24))
//!     .on_react(|key, state, _, _| println!("{key:?} → {state:?}"))
//! ```

use std::{rc::Rc, sync::Arc};

use gpui::{
    AnyView, App, BoxShadow, ElementId, FocusHandle, FontFeatures, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseDownEvent, Pixels, SharedString, TextRun, Transformation, Window, div, font,
    prelude::*, px, size, svg,
};

use crate::{
    ActiveTheme, Look,
    motion::{self, AnimationExt as _, CubicBezier, LINEAR, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Reaction Bar";
pub const DESCRIPTION: &str = "Like, comment, repost and share in one pill – icons pop and fill, counts roll in digit by digit";

/// `--ui-rb-ease`: the count's width glide and the digit roll.
const ROLL_EASE: CubicBezier = CubicBezier::new(0.85, 0.0, 0.15, 1.0);
/// The press pop's per-keyframe curve.
const POP_EASE: CubicBezier = CubicBezier::new(0.65, 0.0, 0.35, 1.0);
/// `ui-rb-pop`: 320ms, eased per keyframe in [`pop_scale`].
const POP: MotionSpec = MotionSpec::new(320, LINEAR);
/// `ui-rb-roll` and the count's `transition: width`.
const ROLL: MotionSpec = MotionSpec::new(400, ROLL_EASE);

/// One of the four actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReactionKey {
    Like,
    Comment,
    Repost,
    Share,
}

impl ReactionKey {
    pub const ALL: [ReactionKey; 4] = [
        ReactionKey::Like,
        ReactionKey::Comment,
        ReactionKey::Repost,
        ReactionKey::Share,
    ];

    fn slug(self) -> &'static str {
        match self {
            ReactionKey::Like => "like",
            ReactionKey::Comment => "comment",
            ReactionKey::Repost => "repost",
            ReactionKey::Share => "share",
        }
    }

    /// The verb and the plural noun, e.g. "Like" / "likes".
    pub fn labels(self) -> (&'static str, &'static str) {
        match self {
            ReactionKey::Like => ("Like", "likes"),
            ReactionKey::Comment => ("Comment", "comments"),
            ReactionKey::Repost => ("Repost", "reposts"),
            ReactionKey::Share => ("Share", "shares"),
        }
    }

    /// The heart and the bubble fill in once taken.
    fn fills(self) -> bool {
        matches!(self, ReactionKey::Like | ReactionKey::Comment)
    }

    fn icon(self, look: Look) -> SharedString {
        match look {
            Look::Clean => format!("icons/reaction-bar/clean/{}.svg", self.slug()).into(),
            Look::Classic => format!("icons/reaction-bar/{}.svg", self.slug()).into(),
        }
    }

    fn fill_icon(self) -> SharedString {
        format!("icons/reaction-bar/{}-fill.svg", self.slug()).into()
    }
}

/// A count and whether the viewer has taken the action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReactionState {
    pub count: u64,
    pub active: bool,
}

impl ReactionState {
    pub fn new(count: u64) -> Self {
        Self {
            count,
            active: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// 1234 → "1.2K", like social apps.
pub fn short(n: u64) -> String {
    fn one_decimal(v: f64) -> String {
        let s = format!("{v:.1}");
        s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
    }
    if n >= 1_000_000 {
        format!("{}M", one_decimal(n as f64 / 1_000_000.))
    } else if n >= 10_000 {
        format!("{}K", (n as f64 / 1000.).round())
    } else if n >= 1000 {
        format!("{}K", one_decimal(n as f64 / 1000.))
    } else {
        n.to_string()
    }
}

/// Screen-reader style label, e.g. "Like, 128 likes".
pub fn aria_label(key: ReactionKey, count: u64) -> String {
    let (verb, noun) = key.labels();
    match count {
        0 => verb.to_string(),
        1 => format!("{verb}, 1 {}", &noun[..noun.len() - 1]),
        n => format!("{verb}, {n} {noun}"),
    }
}

type ReactHandler = Rc<dyn Fn(ReactionKey, ReactionState, &mut Window, &mut App)>;

/// The `--ui-rb-*` / `--ui-rbc-*` tokens.
struct Palette {
    /// Count color at rest and under the pointer.
    fg: Hsla,
    fg_hover: Hsla,
    icon: Hsla,
    icon_hover: Hsla,
    /// Hover background.
    wash: Hsla,
    /// Clean chips: hairline edge, and the tint and edge of a taken action.
    line: Option<Hsla>,
    mine: Option<(Hsla, Hsla)>,
    /// Color of a taken action, per key (like, comment, repost, share).
    tones: [Hsla; 4],
    focus: Hsla,
    /// Classic: the pill's fill and shadow.
    pill: Option<(Hsla, Vec<BoxShadow>)>,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => {
                let accent = hexa(0x0060e2, 0.765);
                Palette {
                    fg: black_a(0.61),
                    fg_hover: black_a(0.88),
                    icon: black_a(0.447),
                    icon_hover: black_a(0.88),
                    wash: black_a(0.03),
                    line: Some(black_a(0.06)),
                    mine: Some((hexa(0x0060e2, 0.05), hexa(0x0060e2, 0.35))),
                    tones: [accent; 4],
                    focus: hexa(0x0060e2, 0.35),
                    pill: None,
                }
            }
            (Look::Clean, true) => {
                let accent = hex(0x4d8dff);
                Palette {
                    fg: white_a(0.62),
                    fg_hover: white_a(0.9),
                    icon: white_a(0.46),
                    icon_hover: white_a(0.9),
                    wash: white_a(0.04),
                    line: Some(white_a(0.08)),
                    mine: Some((hexa(0x4d8dff, 0.14), hexa(0x4d8dff, 0.4))),
                    tones: [accent; 4],
                    focus: hexa(0x4d8dff, 0.45),
                    pill: None,
                }
            }
            (Look::Classic, false) => Palette {
                fg: hex(0x404040),
                fg_hover: hex(0x404040),
                icon: hex(0x404040),
                icon_hover: hex(0x404040),
                wash: hex(0xf5f5f5),
                line: None,
                mine: None,
                tones: [hex(0xec4899), hex(0x2f6bff), hex(0x8b5cf6), hex(0x171717)],
                focus: hex(0x2f6bff),
                pill: Some((
                    hex(0xffffff),
                    vec![
                        shadow(0., 0., 0., 1., hex(0xececec)),
                        shadow(0., 1., 2., 0., black_a(0.04)),
                        shadow(0., 4., 12., 0., black_a(0.03)),
                    ],
                )),
            },
            (Look::Classic, true) => Palette {
                fg: hex(0xc4c4c4),
                fg_hover: hex(0xc4c4c4),
                icon: hex(0xc4c4c4),
                icon_hover: hex(0xc4c4c4),
                wash: white_a(0.06),
                line: None,
                mine: None,
                tones: [hex(0xf472b6), hex(0x6b95ff), hex(0xa78bfa), hex(0xededed)],
                focus: hex(0x6b95ff),
                pill: Some((
                    hex(0x232326),
                    vec![
                        shadow(0., 0., 0., 1., white_a(0.08)),
                        shadow(0., 1., 2., 0., black_a(0.4)),
                        shadow(0., 4., 12., 0., black_a(0.35)),
                    ],
                )),
            },
        }
    }
}

/// Sizes from the two stylesheets.
struct Metrics {
    gap: Pixels,
    height: Pixels,
    min_width: Pixels,
    pad_x: Pixels,
    radius: Pixels,
    icon: Pixels,
    digit_size: Pixels,
    digits_pad: (Pixels, Pixels),
    /// `outline-offset` of the focus ring.
    focus_offset: Pixels,
    /// Icon color and fill transition.
    color_spec: MotionSpec,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                gap: px(4.),
                height: px(28.),
                min_width: px(0.),
                pad_x: px(8.),
                radius: px(6.),
                icon: px(16.),
                digit_size: px(13.),
                digits_pad: (px(6.), px(2.)),
                focus_offset: px(1.),
                color_spec: MotionSpec::new(160, motion::EASE),
            },
            Look::Classic => Metrics {
                gap: px(2.),
                height: px(42.),
                min_width: px(42.),
                pad_x: px(10.),
                radius: px(21.),
                icon: px(22.),
                digit_size: px(14.),
                digits_pad: (px(6.), px(0.)),
                focus_offset: px(2.),
                color_spec: MotionSpec::new(200, motion::EASE),
            },
        }
    }
}

struct Entry {
    key: ReactionKey,
    count: u64,
    active: bool,
    /// Taps so far; each one restarts the pop.
    pops: u32,
    focus: FocusHandle,
}

struct BarState {
    entries: Vec<Entry>,
    /// Focus came from the keyboard, so the ring shows (`:focus-visible`).
    keyboard: bool,
}

#[derive(IntoElement)]
pub struct ReactionBar {
    id: ElementId,
    reactions: Vec<(ReactionKey, ReactionState)>,
    actions: Vec<ReactionKey>,
    on_react: Option<ReactHandler>,
    look: Option<Look>,
}

impl ReactionBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            reactions: Vec::new(),
            actions: ReactionKey::ALL.to_vec(),
            on_react: None,
            look: None,
        }
    }

    /// Starting count and whether the action is already taken (read once,
    /// when the bar first renders).
    pub fn reaction(mut self, key: ReactionKey, state: ReactionState) -> Self {
        self.reactions.retain(|(k, _)| *k != key);
        self.reactions.push((key, state));
        self
    }

    /// The actions to show, in order – hide the ones you don't need.
    pub fn actions(mut self, actions: impl IntoIterator<Item = ReactionKey>) -> Self {
        self.actions = actions.into_iter().collect();
        self
    }

    /// Called after an action was toggled, with its new state.
    pub fn on_react(
        mut self,
        handler: impl Fn(ReactionKey, ReactionState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_react = Some(Rc::new(handler));
        self
    }

    /// Overrides the global Clean/Classic look for this bar.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

/// `ui-rb-pop`: 1 → 0.75 (35%) → 1.12 (70%) → 1, each step eased.
fn pop_scale(t: f32) -> f32 {
    let step = |from_t: f32, to_t: f32, from: f32, to: f32| {
        motion::lerp(from, to, POP_EASE.eval((t - from_t) / (to_t - from_t)))
    };
    if t < 0.35 {
        step(0., 0.35, 1., 0.75)
    } else if t < 0.7 {
        step(0.35, 0.7, 0.75, 1.12)
    } else {
        step(0.7, 1., 1.12, 1.)
    }
}

/// Width of the count's text as the digits lay it out, one box per glyph.
/// Inter Medium with `font-variant-numeric: tabular-nums`.
fn digits_font() -> gpui::Font {
    gpui::Font {
        weight: FontWeight::MEDIUM,
        features: FontFeatures(Arc::new(vec![("tnum".into(), 1)])),
        ..font(SANS)
    }
}

fn digits_width(window: &Window, text: &str, font_size: Pixels) -> Pixels {
    let face = digits_font();
    text.chars()
        .map(|ch| {
            let glyph: SharedString = ch.to_string().into();
            let run = TextRun {
                len: glyph.len(),
                font: face.clone(),
                color: gpui::black(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_line(glyph, font_size, &[run], None)
                .width
                // A text box is as wide as its line, rounded up
                .ceil()
        })
        .fold(px(0.), |a, b| a + b)
}

impl RenderOnce for ReactionBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let p = Palette::get(look, cx);
        let m = Metrics::get(look);
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);

        let reactions = self.reactions.clone();
        let actions = self.actions.clone();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            move |_, cx| BarState {
                entries: actions
                    .iter()
                    .map(|&k| {
                        let start = reactions
                            .iter()
                            .find(|(r, _)| *r == k)
                            .map(|(_, s)| *s)
                            .unwrap_or_default();
                        Entry {
                            key: k,
                            count: start.count,
                            active: start.active,
                            pops: 0,
                            focus: cx.focus_handle(),
                        }
                    })
                    .collect(),
                keyboard: false,
            },
        );

        let (snapshot, keyboard): (Vec<_>, bool) = {
            let s = state.read(cx);
            (
                s.entries
                    .iter()
                    .map(|e| (e.key, e.count, e.active, e.pops, e.focus.clone()))
                    .collect(),
                s.keyboard,
            )
        };
        let handles: Vec<FocusHandle> = snapshot.iter().map(|e| e.4.clone()).collect();

        let buttons: Vec<_> = snapshot
            .into_iter()
            .enumerate()
            .map(|(ix, (rkey, count, active, pops, focus))| {
                let bkey = format!("{key}-{}", rkey.slug());
                let hover = motion::hover_t(&bkey);
                let on = motion::follow(
                    &format!("{bkey}-on"),
                    if active { 1. } else { 0. },
                    m.color_spec,
                );
                let tone = p.tones[ix_of(rkey)];

                let mut bg = motion::mix(p.wash.opacity(0.), p.wash, hover);
                let mut edge = p.line;
                if let Some((mine, mine_line)) = p.mine {
                    bg = motion::mix(bg, mine, on);
                    edge = edge.map(|line| motion::mix(line, mine_line, on));
                }
                // Clean eases the label with the chip; Classic only eases the icon.
                let fg = match look {
                    Look::Clean => motion::mix(motion::mix(p.fg, p.fg_hover, hover), tone, on),
                    Look::Classic if active => tone,
                    Look::Classic => p.fg,
                };
                let icon_color = motion::mix(motion::mix(p.icon, p.icon_hover, hover), tone, on);

                let icon_size = m.icon;
                let outline = rkey.icon(look);
                let fill = rkey.fills().then(|| rkey.fill_icon());
                let icon = move |scale: f32| {
                    let scaled = Transformation::scale(size(scale, scale));
                    div()
                        .relative()
                        .flex_none()
                        .size(icon_size)
                        .child(
                            svg()
                                .path(outline.clone())
                                .size(icon_size)
                                .text_color(icon_color)
                                .with_transformation(scaled),
                        )
                        .when_some(fill.clone(), |d, fill| {
                            d.child(
                                svg()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .path(fill)
                                    .size(icon_size)
                                    .text_color(icon_color.opacity(on))
                                    .with_transformation(scaled),
                            )
                        })
                };
                let icon = if pops > 0 && !reduced {
                    div()
                        .flex_none()
                        .with_animation(
                            ElementId::Name(format!("{bkey}-pop-{pops}").into()),
                            POP.animation(),
                            move |d, t| d.child(icon(pop_scale(t))),
                        )
                        .into_any_element()
                } else {
                    icon(1.).into_any_element()
                };

                let focused = keyboard && focus.is_focused(window);
                let on_react = self.on_react.clone();
                let click_state = state.clone();
                let mouse_state = state.clone();

                div()
                    .id(ElementId::Name(bkey.clone().into()))
                    .track_focus(&focus)
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(m.height)
                    .min_w(m.min_width)
                    .px(m.pad_x)
                    .rounded(m.radius)
                    .cursor_pointer()
                    .bg(bg)
                    .text_color(fg)
                    .on_hover(motion::hover_listener(bkey.clone()))
                    .on_mouse_down(gpui::MouseButton::Left, move |_: &MouseDownEvent, _, cx| {
                        mouse_state.update(cx, |s, _| s.keyboard = false);
                    })
                    // Enter and Space on a focused chip click it too (GPUI does that)
                    .on_click(move |_, window, cx| {
                        toggle(&click_state, ix, on_react.as_ref(), window, cx)
                    })
                    // `box-shadow: inset 0 0 0 .5px` – under the content, like CSS
                    .when_some(edge, |d, edge| {
                        d.child(
                            div()
                                .absolute()
                                .inset_0()
                                .rounded(m.radius)
                                .border(px(0.5))
                                .border_color(edge),
                        )
                    })
                    .when(focused, |d| {
                        let o = m.focus_offset + px(2.);
                        d.child(
                            div()
                                .absolute()
                                .top(-o)
                                .left(-o)
                                .right(-o)
                                .bottom(-o)
                                .rounded(m.radius + o)
                                .border_2()
                                .border_color(p.focus),
                        )
                    })
                    .child(icon)
                    .child(count_view(window, &bkey, count, &m, reduced))
            })
            .collect();

        let nav_state = state.clone();
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let len = handles.len();
            if len == 0 {
                return;
            }
            let current = handles.iter().position(|f| f.is_focused(window));
            let next = match event.keystroke.key.as_str() {
                "right" | "down" => current.map_or(0, |i| (i + 1) % len),
                "left" | "up" => current.map_or(len - 1, |i| (i + len - 1) % len),
                "home" => 0,
                "end" => len - 1,
                _ => return,
            };
            nav_state.update(cx, |s, cx| {
                s.keyboard = true;
                cx.notify();
            });
            window.focus(&handles[next]);
            cx.stop_propagation();
        };

        let root = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .gap(m.gap)
            .max_w_full()
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .on_key_down(on_key)
            .children(buttons);
        match p.pill {
            Some((fill, pill_shadow)) => root
                .py(px(4.))
                .px(px(10.))
                .rounded(px(999.))
                .bg(fill)
                .shadow(pill_shadow),
            None => root,
        }
    }
}

fn ix_of(key: ReactionKey) -> usize {
    ReactionKey::ALL.iter().position(|k| *k == key).unwrap_or(0)
}

fn toggle(
    state: &gpui::Entity<BarState>,
    ix: usize,
    on_react: Option<&ReactHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let next = state.update(cx, |s, cx| {
        let entry = &mut s.entries[ix];
        let was = entry.active;
        entry.active = !was;
        entry.count = if was {
            entry.count.saturating_sub(1)
        } else {
            entry.count + 1
        };
        entry.pops += 1;
        cx.notify();
        (
            entry.key,
            ReactionState {
                count: entry.count,
                active: entry.active,
            },
        )
    });
    if let Some(handler) = on_react {
        handler(next.0, next.1, window, cx);
    }
}

/// The count: its box glides to the new width while each changed digit
/// rolls in from below.
fn count_view(
    window: &Window,
    bkey: &str,
    count: u64,
    m: &Metrics,
    reduced: bool,
) -> impl IntoElement {
    let text = if count > 0 {
        short(count)
    } else {
        String::new()
    };
    let natural = if text.is_empty() {
        px(0.)
    } else {
        digits_width(window, &text, m.digit_size) + m.digits_pad.0 + m.digits_pad.1
    };
    // The first render settles at once, like the web's `data-smooth` flag.
    let width = motion::follow(&format!("{bkey}-w"), f32::from(natural), ROLL);
    let len = text.chars().count();

    let digits = text.chars().enumerate().map(|(i, ch)| {
        let glyph = div().flex_none().child(ch.to_string());
        if reduced {
            return glyph.into_any_element();
        }
        // Keyed by place and digit: only the digits that changed roll in.
        glyph
            .with_animation(
                ElementId::Name(format!("{bkey}-digit-{}-{ch}", len - i).into()),
                ROLL.animation(),
                |el, t| el.relative().top(px(14. * (1. - t))).opacity(t),
            )
            .into_any_element()
    });

    div()
        .flex_none()
        .w(px(width))
        .h(px(20.))
        .overflow_hidden()
        .child(
            div()
                .flex()
                .flex_none()
                .when(!text.is_empty(), |d| {
                    d.pl(m.digits_pad.0).pr(m.digits_pad.1)
                })
                .font(digits_font())
                .text_size(m.digit_size)
                .line_height(px(20.))
                .whitespace_nowrap()
                .children(digits),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        ReactionBar::new("reaction-bar")
            .reaction(ReactionKey::Like, ReactionState::new(128))
            .reaction(ReactionKey::Comment, ReactionState::new(24))
            .reaction(ReactionKey::Repost, ReactionState::new(9))
            .reaction(ReactionKey::Share, ReactionState::new(0))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_counts() {
        assert_eq!(short(0), "0");
        assert_eq!(short(999), "999");
        assert_eq!(short(1000), "1K");
        assert_eq!(short(1234), "1.2K");
        assert_eq!(short(12_345), "12K");
        assert_eq!(short(1_500_000), "1.5M");
        assert_eq!(short(2_000_000), "2M");
    }

    #[test]
    fn labels() {
        assert_eq!(aria_label(ReactionKey::Like, 0), "Like");
        assert_eq!(aria_label(ReactionKey::Like, 1), "Like, 1 like");
        assert_eq!(aria_label(ReactionKey::Repost, 9), "Repost, 9 reposts");
    }

    #[test]
    fn pop_curve_keyframes() {
        assert!((pop_scale(0.) - 1.).abs() < 1e-4);
        assert!((pop_scale(0.35) - 0.75).abs() < 1e-3);
        assert!((pop_scale(0.7) - 1.12).abs() < 1e-3);
        assert!((pop_scale(1.) - 1.).abs() < 1e-4);
    }
}
