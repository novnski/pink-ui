//! Message Actions – The quiet bar under an AI message – copy turns into a check, regenerate spins and unfolds a version switcher, thumbs pop and ask why, read aloud becomes a tiny equalizer
//!
//! - Copy: the icon morphs into a green check and the tooltip says "Copied".
//! - Regenerate: the arrow spins once (and keeps spinning while a new version
//!   is written); from two versions on a "‹ 2 / 3 ›" switcher unfolds next to it.
//! - Thumbs up / down pop when pressed; thumbs down unfolds a feedback row with
//!   reasons, an optional "Other" field and a send button.
//! - Read aloud: the speaker morphs into a bouncing equalizer while it plays.
//!
//! The bar stays faded until the message is hovered, something in it has
//! focus or an action is running. Clean: ghost buttons, the feedback row sits
//! in the same card. Classic: line glyphs and a white feedback card.
//! [`MessageText`] crossfades the message itself when the version changes.
//!
//! ```ignore
//! MessageActions::new("reply-actions")
//!     .text(reply.clone())
//!     .versions(versions.len())
//!     .on_regenerate(|window, cx| { /* write a new version */ })
//!     .on_feedback(|kind, reason, window, cx| { /* store the vote */ })
//! ```

use std::{cell::Cell, collections::HashMap, f32::consts::TAU, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnyElement, AnyView, App, BoxShadow, ClipboardItem, Context, Div, ElementId, Entity,
    FocusHandle, FontWeight, Hsla, IntoElement, KeyDownEvent, MouseButton, Pixels, SharedString,
    Stateful, Subscription, Svg, Task, Transformation, Window, canvas, div, prelude::*, px,
    radians, relative, size, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    interaction::HoverColors,
    motion::{self, AnimationExt, CubicBezier, EASE, EASE_IN_OUT, EASE_OUT_QUINT, MotionSpec},
    primitives::{InputEvent, SANS, TextInput, black_a, hex, hexa, ring, shadow, white_a},
    story,
};

pub const NAME: &str = "Message Actions";
pub const DESCRIPTION: &str = "The quiet bar under an AI message – copy turns into a check, regenerate spins and unfolds a version switcher, thumbs pop and ask why, read aloud becomes a tiny equalizer";

/// One action in the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageAction {
    Copy,
    Regenerate,
    /// Thumbs up and thumbs down.
    Feedback,
    /// Read aloud.
    Speak,
}

/// Copy, Regenerate, the thumbs and Read aloud.
pub const DEFAULT_ACTIONS: [MessageAction; 4] = [
    MessageAction::Copy,
    MessageAction::Regenerate,
    MessageAction::Feedback,
    MessageAction::Speak,
];

/// Which thumb is pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackKind {
    Up,
    Down,
}

/// The quick reasons of the feedback row. "Other" opens a small text field.
pub fn default_reasons() -> Vec<SharedString> {
    [
        "Not accurate",
        "Too long",
        "Didn't follow instructions",
        "Other",
    ]
    .into_iter()
    .map(SharedString::from)
    .collect()
}

const OTHER: &str = "Other";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type CopyHandler = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;
type VersionHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type FeedbackHandler =
    Rc<dyn Fn(Option<FeedbackKind>, Option<SharedString>, &mut Window, &mut App)>;
type SpeakHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// `cubic-bezier(0.32, 0.72, 0.24, 1)` – the Classic stylesheet's `--ui-ma-ease`.
const CLASSIC_EASE: CubicBezier = CubicBezier::new(0.32, 0.72, 0.24, 1.0);

/// The `--ui-ma-*` / `--ui-mac-*` tokens, named by role.
struct Palette {
    /// Tooltip (Clean), feedback card (Classic) and "Other" field (Clean).
    card: Hsla,
    strong: Hsla,
    ink: Hsla,
    text: Hsla,
    muted: Hsla,
    /// The "/" of the version count, the field's placeholder.
    faint: Hsla,
    /// Resting icon color.
    icon: Hsla,
    line: Hsla,
    /// Hover wash of buttons and steps.
    wash: Hsla,
    /// Background of a pressed button (Clean).
    pressed: Hsla,
    chip: Hsla,
    chip_hover: Hsla,
    chip_on: (Hsla, Hsla),
    send: (Hsla, Hsla),
    send_hover: Hsla,
    accent: Hsla,
    accent_wash: Hsla,
    ok: Hsla,
    ok_wash: Hsla,
    tip: (Hsla, Hsla),
    tip_shadow: Vec<BoxShadow>,
    field: Hsla,
    field_shadow: Vec<BoxShadow>,
    panel_shadow: Vec<BoxShadow>,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, window: &Window, cx: &App) -> Self {
        let mut p = Self::tokens(look, cx);
        let scale = window.scale_factor();
        for list in [&mut p.tip_shadow, &mut p.field_shadow, &mut p.panel_shadow] {
            *list = css_shadows(std::mem::take(list), scale);
        }
        p
    }

    fn tokens(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => Palette {
                card: hex(0xffffff),
                strong: hex(0x000000),
                ink: black_a(0.88),
                text: black_a(0.61),
                muted: black_a(0.45),
                faint: black_a(0.27),
                icon: black_a(0.447),
                line: black_a(0.06),
                wash: black_a(0.03),
                pressed: black_a(0.06),
                chip: black_a(0.),
                chip_hover: black_a(0.03),
                chip_on: (black_a(0.06), black_a(0.88)),
                send: (black_a(0.88), hex(0xfcfcfc)),
                send_hover: black_a(0.8),
                accent: hexa(0x0060e2, 0.765),
                accent_wash: hexa(0x0060e2, 0.1),
                ok: hex(0x1a9a52),
                ok_wash: hexa(0x1a9a52, 0.08),
                tip: (hex(0xffffff), black_a(0.88)),
                tip_shadow: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.08)),
                    shadow(0., 12., 28., -10., black_a(0.16)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                field: hex(0xffffff),
                field_shadow: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                panel_shadow: vec![],
                focus: hexa(0x0060e2, 0.35),
            },
            (Look::Clean, true) => Palette {
                card: hex(0x1c1c1e),
                strong: hex(0xffffff),
                ink: white_a(0.9),
                text: white_a(0.62),
                muted: white_a(0.46),
                faint: white_a(0.3),
                icon: white_a(0.5),
                line: white_a(0.08),
                wash: white_a(0.04),
                pressed: white_a(0.08),
                chip: white_a(0.),
                chip_hover: white_a(0.04),
                chip_on: (white_a(0.08), white_a(0.9)),
                send: (white_a(0.9), hex(0x1c1c1e)),
                send_hover: white_a(0.8),
                accent: hex(0x4d8dff),
                accent_wash: hexa(0x4d8dff, 0.16),
                ok: hex(0x3ec27a),
                ok_wash: hexa(0x3ec27a, 0.14),
                tip: (hex(0x1c1c1e), white_a(0.9)),
                tip_shadow: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.12)),
                    shadow(0., 12., 28., -10., black_a(0.6)),
                ],
                field: hex(0x1c1c1e),
                field_shadow: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                panel_shadow: vec![],
                focus: hexa(0x4d8dff, 0.5),
            },
            (Look::Classic, false) => Palette {
                card: hex(0xffffff),
                strong: hex(0x171717),
                ink: hex(0x171717),
                text: hex(0x404040),
                muted: hex(0x8f8f8f),
                faint: hex(0x8f8f8f),
                icon: hex(0x8f8f8f),
                line: hex(0xececec),
                wash: hexa(0x171717, 0.06),
                pressed: hexa(0x171717, 0.),
                chip: hex(0xf4f4f5),
                chip_hover: hex(0xececec),
                chip_on: (hexa(0x2f6bff, 0.1), hex(0x2f6bff)),
                send: (hex(0x171717), hex(0xffffff)),
                send_hover: hex(0x171717),
                accent: hex(0x2f6bff),
                accent_wash: hexa(0x2f6bff, 0.1),
                ok: hex(0x18794e),
                ok_wash: hexa(0x22a05a, 0.11),
                tip: (hex(0x171717), hex(0xffffff)),
                tip_shadow: vec![],
                field: hex(0xf4f4f5),
                field_shadow: vec![],
                panel_shadow: vec![
                    ring(1., hex(0xececec)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ],
                focus: hexa(0x171717, 0.22),
            },
            (Look::Classic, true) => Palette {
                card: hex(0x19191b),
                strong: hex(0xededed),
                ink: hex(0xededed),
                text: hex(0xc8c8cc),
                muted: hex(0x8b8b92),
                faint: hex(0x8b8b92),
                icon: hex(0x8b8b92),
                line: hex(0x2a2a2e),
                wash: white_a(0.07),
                pressed: white_a(0.),
                chip: hex(0x222225),
                chip_hover: hex(0x2a2a2e),
                chip_on: (hexa(0x4f82ff, 0.16), hex(0x8fb1ff)),
                send: (hex(0xededed), hex(0x19191b)),
                send_hover: hex(0xededed),
                accent: hex(0x8fb1ff),
                accent_wash: hexa(0x4f82ff, 0.16),
                ok: hex(0x6ee7a8),
                ok_wash: hexa(0x6ee7a8, 0.12),
                tip: (hex(0xededed), hex(0x171717)),
                tip_shadow: vec![],
                field: hex(0x222225),
                field_shadow: vec![],
                panel_shadow: vec![
                    ring(1., hex(0x2a2a2e)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ],
                focus: white_a(0.3),
            },
        }
    }
}

/// Sizes from the two stylesheets.
struct Metrics {
    button: Pixels,
    button_radius: Pixels,
    step_radius: Pixels,
    step_glyph: Pixels,
    step_disabled: f32,
    count_pad: Pixels,
    count_line: Pixels,
    tip_pad: (Pixels, Pixels),
    tip_size: Pixels,
    tip_line: Pixels,
    eq_bar: f32,
    pop: f32,
    ask_size: Pixels,
    close_glyph: Pixels,
    close_radius: Pixels,
    chip_gap: Pixels,
    chip_height: Pixels,
    chip_pad: Pixels,
    chip_radius: Pixels,
    chip_size: Pixels,
    chip_weight: FontWeight,
    send_min: Pixels,
    send_pad: Pixels,
    send_size: Pixels,
    send_disabled: f32,
    tick: Pixels,
    field_height: Pixels,
    field_radius: Pixels,
    field_size: Pixels,
    focus_offset: f32,
    ease: CubicBezier,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                button: px(28.),
                button_radius: px(6.),
                step_radius: px(6.),
                step_glyph: px(16.),
                step_disabled: 0.45,
                count_pad: px(2.),
                count_line: px(16.),
                tip_pad: (px(2.), px(8.)),
                tip_size: px(12.),
                tip_line: px(20.),
                eq_bar: 11.,
                pop: 0.22,
                ask_size: px(13.),
                close_glyph: px(16.),
                close_radius: px(6.),
                chip_gap: px(8.),
                chip_height: px(28.),
                chip_pad: px(10.),
                chip_radius: px(6.),
                chip_size: px(13.),
                chip_weight: FontWeight::MEDIUM,
                send_min: px(60.),
                send_pad: px(10.),
                send_size: px(13.),
                send_disabled: 0.3,
                tick: px(14.),
                field_height: px(32.),
                field_radius: px(6.),
                field_size: px(13.),
                focus_offset: 1.,
                ease: EASE_OUT_QUINT,
            },
            Look::Classic => Metrics {
                button: px(30.),
                button_radius: px(8.),
                step_radius: px(7.),
                step_glyph: px(14.),
                step_disabled: 0.4,
                count_pad: px(1.),
                count_line: px(20.),
                tip_pad: (px(3.), px(7.)),
                tip_size: px(11.5),
                tip_line: px(16.),
                eq_bar: 12.,
                pop: 0.28,
                ask_size: px(12.5),
                close_glyph: px(14.),
                close_radius: px(7.),
                chip_gap: px(6.),
                chip_height: px(26.),
                chip_pad: px(9.),
                chip_radius: px(8.),
                chip_size: px(12.),
                chip_weight: FontWeight::NORMAL,
                send_min: px(58.),
                send_pad: px(11.),
                send_size: px(12.),
                send_disabled: 0.32,
                tick: px(13.),
                field_height: px(30.),
                field_radius: px(8.),
                field_size: px(12.5),
                focus_offset: 2.,
                ease: CLASSIC_EASE,
            },
        }
    }
}

/// A CSS `box-shadow` list for GPUI, which paints the layers the other way
/// round (CSS puts the first on top). Each layer is already converted by
/// `primitives::shadow`.
fn css_shadows(layers: Vec<BoxShadow>, _scale: f32) -> Vec<BoxShadow> {
    crate::primitives::shadows(layers)
}

fn icon(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/message-actions/clean/{name}.svg").into(),
        Look::Classic => format!("icons/message-actions/{name}.svg").into(),
    }
}

// ---------------------------------------------------------------------------
// Motion
// ---------------------------------------------------------------------------

/// Tooltip opacity: `140ms ease`, after a 380ms beat when it comes from hover.
const TIP_FADE: MotionSpec = MotionSpec::new(140, EASE);
const TIP_DELAY: u64 = 380;
/// Swapping text: the new label fades in over 300ms, the old one out over 220ms.
const SWAP_IN: MotionSpec = MotionSpec::new(300, EASE);
const SWAP_OUT: MotionSpec = MotionSpec::new(220, EASE);
/// How long a replaced label stays mounted (the `setTimeout(…, 300)`).
const SWAP_HOLD: Duration = Duration::from_millis(300);
/// Folds open: height 420ms on the look's curve, opacity 320ms after 80ms.
const FOLD_OPEN: MotionSpec = MotionSpec::new(320, EASE).with_delay(80);
const FOLD_CLOSE: MotionSpec = MotionSpec::new(220, EASE);
/// The equalizer bars: (period, phase offset) in ms, `ui-ma-eq` with negative delays.
const EQ: [(f32, f32); 4] = [(900., 0.), (760., 300.), (1000., 520.), (820., 140.)];

/// One device pixel: what Chrome paints for a 0.5px border. GPUI clips each
/// border side to its own strip, so a literal 0.5px side can vanish.
fn hairline(window: &Window) -> Pixels {
    px(1. / window.scale_factor())
}

/// An inset `0 0 0 0.5px` shadow as a border: one device pixel at the
/// coverage the browser's antialiasing gives it.
fn inset_hairline(window: &Window, color: Hsla) -> (Pixels, Hsla) {
    let scale = window.scale_factor();
    (px(1. / scale), color.opacity((0.5 * scale).min(1.)))
}

/// GPUI counts the space after a line's last word when wrapping (browsers
/// let it hang), so wrapped text gets this much slack on the right to break
/// where the browser does.
const HANGING_SPACE: Pixels = px(4.);

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000. / motion::speed_scale()
}

/// `ui-ma-pop`: scale(1) → scale(peak) rotate(-8deg) at 40% → scale(1), with
/// the easing applied per keyframe segment like CSS does.
fn pop(t: f32, peak: f32, curve: CubicBezier) -> (f32, f32) {
    if !(0. ..1.).contains(&t) {
        return (1., 0.);
    }
    let k = if t < 0.4 {
        curve.eval(t / 0.4)
    } else {
        1. - curve.eval((t - 0.4) / 0.6)
    };
    (1. + (peak - 1.) * k, -8f32.to_radians() * k)
}

/// `ui-ma-eq`: scaleY 0.3 → 1 → 0.3, ease-in-out per half.
fn eq_scale(elapsed_ms: f32, bar: usize) -> f32 {
    let (period, offset) = EQ[bar % EQ.len()];
    let phase = ((elapsed_ms + offset) / period).rem_euclid(1.);
    let (from, to, t) = if phase < 0.5 {
        (0.3, 1., phase * 2.)
    } else {
        (1., 0.3, (phase - 0.5) * 2.)
    };
    motion::lerp(from, to, EASE_IN_OUT.eval(t))
}

/// Eases a 0/1 flag over `spec`.
fn eased(key: &str, on: bool, spec: MotionSpec) -> f32 {
    motion::follow(key, if on { 1. } else { 0. }, spec)
}

/// One transition with a real `transition-delay`: holds, then eases.
/// (`motion::follow` folds a spec's delay into its curve instead.)
#[derive(Clone, Copy)]
struct Fade {
    from: f32,
    to: f32,
    at: Instant,
    spec: MotionSpec,
}

impl Fade {
    fn value(&self, now: Instant) -> f32 {
        let total = self.spec.total().mul_f32(motion::speed_scale());
        if total.is_zero() {
            return self.to;
        }
        let raw = now.saturating_duration_since(self.at).as_secs_f32() / total.as_secs_f32();
        motion::lerp(self.from, self.to, self.spec.progress(raw.min(1.)))
    }
}

/// The delayed fades of one bar (tooltips, folds), kept with its state.
#[derive(Default)]
struct Fades {
    fades: HashMap<(&'static str, &'static str), Fade>,
    moving: bool,
}

impl Fades {
    /// Eases `slot` toward 1 (on) or 0 under `spec`; a new slot starts settled.
    fn ease(
        &mut self,
        slot: (&'static str, &'static str),
        on: bool,
        spec: MotionSpec,
        now: Instant,
        reduced: bool,
    ) -> f32 {
        let to = if on { 1. } else { 0. };
        let fade = self.fades.entry(slot).or_insert(Fade {
            from: to,
            to,
            at: now,
            spec,
        });
        if fade.to != to {
            let current = fade.value(now);
            *fade = Fade {
                from: if reduced { to } else { current },
                to,
                at: now,
                spec,
            };
        }
        let v = fade.value(now);
        self.moving |= v != fade.to;
        v
    }
}

/// The label a swap slot shows, and the one it replaced with when.
type SwapEntry = (SharedString, Option<(SharedString, Instant)>);

/// Remembers the label each swap slot shows, so a change can crossfade.
#[derive(Default)]
struct Swaps(HashMap<&'static str, SwapEntry>);

impl Swaps {
    /// Records `id` for `slot`; returns the label it replaced while that one
    /// is still fading out.
    fn track(
        &mut self,
        slot: &'static str,
        id: &SharedString,
        now: Instant,
    ) -> Option<SharedString> {
        let entry = self.0.entry(slot).or_insert_with(|| (id.clone(), None));
        if &entry.0 != id {
            let old = std::mem::replace(&mut entry.0, id.clone());
            entry.1 = Some((old, now));
        }
        match &entry.1 {
            Some((old, at)) if now.saturating_duration_since(*at) < SWAP_HOLD => Some(old.clone()),
            _ => {
                entry.1 = None;
                None
            }
        }
    }
}

/// Text that changes without a jump: the old label fades out underneath while
/// the new one fades in (`Swap` in the TSX).
fn swap(
    slot: &str,
    now: &SharedString,
    old: Option<SharedString>,
    render: impl Fn(&SharedString) -> AnyElement,
) -> Div {
    let item = |id: &SharedString| {
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(4.))
            .child(render(id))
    };
    div()
        .relative()
        .flex()
        .child(item(now).with_animation(
            ElementId::Name(format!("{slot}-in-{now}").into()),
            SWAP_IN.animation(),
            |el, t| el.opacity(t),
        ))
        .when_some(old, |d, old| {
            d.child(div().absolute().inset_0().flex().justify_center().child(
                item(&old).with_animation(
                    ElementId::Name(format!("{slot}-out-{old}").into()),
                    SWAP_OUT.animation(),
                    |el, t| el.opacity(1. - t),
                ),
            ))
        })
}

/// Reports the laid-out size of its parent into `cell` (height or width), so
/// folds know how far to open. Asks for one more frame when it changed.
fn measure(cell: Rc<Cell<f32>>, width: bool) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let v = f32::from(if width {
                bounds.size.width
            } else {
                bounds.size.height
            });
            if (cell.get() - v).abs() > 0.25 {
                cell.set(v);
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// A block that unfolds vertically to its content's height (`grid-template-rows:
/// 0fr → 1fr`) and fades. 3px of slack on every side keep rings and shadows
/// from being clipped, like the CSS `margin: -3px; padding: 3px`.
fn fold(
    key: &str,
    open: bool,
    o: f32,
    ease: CubicBezier,
    cell: Rc<Cell<f32>>,
    content: Div,
) -> Div {
    let target = if open { cell.get() } else { 0. };
    let h = motion::follow(&format!("{key}-h"), target, MotionSpec::new(420, ease));
    div()
        .m(px(-3.))
        .h(px(h + 6.))
        .overflow_hidden()
        .opacity(o)
        .child(
            div()
                .p(px(3.))
                .child(content.relative().child(measure(cell, false))),
        )
}

/// An SVG glyph, scaled and turned around its center.
fn glyph(path: SharedString, dim: Pixels, color: Hsla, scale: f32, turn: f32) -> Svg {
    svg()
        .path(path)
        .size(dim)
        .flex_none()
        .text_color(color)
        .with_transformation(Transformation::scale(size(scale, scale)).with_rotation(radians(turn)))
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// The controls of the bar, for focus, hover and tooltips.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Btn {
    Copy,
    Regenerate,
    Up,
    Down,
    Speak,
    Prev,
    Next,
    Close,
    Send,
}

impl Btn {
    const ALL: [Btn; 9] = [
        Btn::Copy,
        Btn::Regenerate,
        Btn::Up,
        Btn::Down,
        Btn::Speak,
        Btn::Prev,
        Btn::Next,
        Btn::Close,
        Btn::Send,
    ];

    fn name(self) -> &'static str {
        match self {
            Btn::Copy => "copy",
            Btn::Regenerate => "regenerate",
            Btn::Up => "up",
            Btn::Down => "down",
            Btn::Speak => "speak",
            Btn::Prev => "prev",
            Btn::Next => "next",
            Btn::Close => "close",
            Btn::Send => "send",
        }
    }
}

/// What the bar was last rendered with – the actions read it.
#[derive(Clone)]
struct Props {
    text: Option<SharedString>,
    count: usize,
    version: Option<usize>,
    default_version: Option<usize>,
    regenerating: bool,
    reasons: Vec<SharedString>,
    speaking: Option<bool>,
    speak_duration: Option<Duration>,
    on_version_change: Option<VersionHandler>,
    on_copy: Option<CopyHandler>,
    on_regenerate: Option<Handler>,
    on_feedback: Option<FeedbackHandler>,
    on_speak: Option<SpeakHandler>,
}

impl Default for Props {
    fn default() -> Self {
        Props {
            text: None,
            count: 1,
            version: None,
            default_version: None,
            regenerating: false,
            reasons: default_reasons(),
            speaking: None,
            speak_duration: None,
            on_version_change: None,
            on_copy: None,
            on_regenerate: None,
            on_feedback: None,
            on_speak: None,
        }
    }
}

/// Everything the bar keeps between frames. Normally keyed by the bar's id;
/// create one yourself and pass it with [`MessageActions::state`] to drive
/// the bar from code (a demo, a shortcut, a test).
pub struct MessageActionsState {
    props: Props,
    inner: usize,
    seen_count: Option<usize>,
    vote: Option<FeedbackKind>,
    voted_at: Option<Instant>,
    panel: bool,
    reason: Option<usize>,
    sent: bool,
    copied: bool,
    spun_at: Option<Instant>,
    speaking: bool,
    speaking_since: Instant,
    born: Instant,
    /// The control under the pointer (tooltips).
    tip: Option<Btn>,
    /// The control the mouse is pressing (`:active` shrink).
    pressing: Option<Btn>,
    /// Focus last moved by keyboard – focus rings and tooltips show.
    keyboard: bool,
    hovered: bool,
    focus: HashMap<Btn, FocusHandle>,
    chips: Vec<FocusHandle>,
    other: Entity<TextInput>,
    swaps: Swaps,
    fades: Fades,
    versions_width: Rc<Cell<f32>>,
    panel_height: Rc<Cell<f32>>,
    other_height: Rc<Cell<f32>>,
    copy_timer: Option<Task<()>>,
    sent_timer: Option<Task<()>>,
    speak_timer: Option<Task<()>>,
    _other_events: Subscription,
}

impl MessageActionsState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let other = cx.new(|cx| TextInput::new(window, cx).placeholder("Tell us more (optional)"));
        let other_events =
            cx.subscribe_in(&other, window, |this, _, event, window, cx| match event {
                InputEvent::Submit(_) => this.send(window, cx),
                InputEvent::Escape => this.close_feedback(window, cx),
                InputEvent::Tab => {
                    this.keyboard = true;
                    window.focus_next(cx);
                }
                _ => {}
            });
        let now = Instant::now();
        MessageActionsState {
            props: Props::default(),
            inner: 0,
            seen_count: None,
            vote: None,
            voted_at: None,
            panel: false,
            reason: None,
            sent: false,
            copied: false,
            spun_at: None,
            speaking: false,
            speaking_since: now,
            born: now,
            tip: None,
            pressing: None,
            keyboard: false,
            hovered: false,
            focus: Btn::ALL
                .iter()
                .map(|b| (*b, cx.focus_handle().tab_stop(true)))
                .collect(),
            chips: Vec::new(),
            other,
            swaps: Swaps::default(),
            fades: Fades::default(),
            versions_width: Rc::new(Cell::new(0.)),
            panel_height: Rc::new(Cell::new(0.)),
            other_height: Rc::new(Cell::new(0.)),
            copy_timer: None,
            sent_timer: None,
            speak_timer: None,
            _other_events: other_events,
        }
    }

    /// Takes the latest props; a new version arriving shows it and resets the vote.
    fn sync(&mut self, props: Props, cx: &mut Context<Self>) {
        let count = props.count.max(1);
        match self.seen_count {
            None => {
                self.inner = props.default_version.unwrap_or(count - 1).min(count - 1);
            }
            Some(seen) if count > seen => {
                self.inner = count - 1;
                self.vote = None;
                self.panel = false;
            }
            _ => {}
        }
        self.seen_count = Some(count);
        while self.chips.len() < props.reasons.len() {
            self.chips.push(cx.focus_handle().tab_stop(true));
        }
        self.props = props;
    }

    /// The version on show (0-based).
    pub fn version(&self) -> usize {
        let count = self.props.count.max(1);
        self.props.version.unwrap_or(self.inner).min(count - 1)
    }

    pub fn vote(&self) -> Option<FeedbackKind> {
        self.vote
    }

    pub fn is_speaking(&self) -> bool {
        self.props.speaking.unwrap_or(self.speaking)
    }

    /// Copy: writes the text to the clipboard and shows the check for 1.6s.
    pub fn copy(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.props.text.clone();
        if let Some(text) = &text {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        }
        if let Some(f) = self.props.on_copy.clone() {
            f(text, window, cx);
        }
        self.copied = true;
        self.copy_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1600))
                .await;
            this.update(cx, |s, cx| {
                s.copied = false;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Regenerate: spins the arrow and asks for a new version.
    pub fn regenerate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.spun_at = Some(Instant::now());
        if let Some(f) = self.props.on_regenerate.clone() {
            f(window, cx);
        }
        cx.notify();
    }

    /// Thumbs up, or takes it back.
    pub fn thumbs_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let f = self.props.on_feedback.clone();
        if self.vote == Some(FeedbackKind::Up) {
            self.vote = None;
            if let Some(f) = f {
                f(None, None, window, cx);
            }
        } else {
            self.vote = Some(FeedbackKind::Up);
            self.voted_at = Some(Instant::now());
            self.panel = false;
            if let Some(f) = f {
                f(Some(FeedbackKind::Up), None, window, cx);
            }
        }
        cx.notify();
    }

    /// Thumbs down: unfolds the feedback row, or takes the vote back.
    pub fn thumbs_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let f = self.props.on_feedback.clone();
        if self.vote == Some(FeedbackKind::Down) {
            self.vote = None;
            self.panel = false;
            if let Some(f) = f {
                f(None, None, window, cx);
            }
        } else {
            self.vote = Some(FeedbackKind::Down);
            self.voted_at = Some(Instant::now());
            self.panel = true;
            self.sent = false;
            self.reason = None;
            self.sent_timer = None;
            self.other.update(cx, |input, cx| input.clear(cx));
            if let Some(f) = f {
                f(Some(FeedbackKind::Down), None, window, cx);
            }
        }
        cx.notify();
    }

    /// Picks a reason in the feedback row, or unpicks it.
    pub fn toggle_reason(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.sent || ix >= self.props.reasons.len() {
            return;
        }
        self.reason = if self.reason == Some(ix) {
            None
        } else {
            Some(ix)
        };
        cx.notify();
    }

    /// Sends the picked reason; the row says thanks and folds away after 1.5s.
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(reason) = self
            .reason
            .and_then(|ix| self.props.reasons.get(ix).cloned())
        else {
            return;
        };
        if self.sent {
            return;
        }
        let reason = if reason.as_ref() == OTHER {
            let typed = self.other.read(cx).text();
            let typed = typed.trim();
            if typed.is_empty() {
                reason
            } else {
                SharedString::from(typed.to_string())
            }
        } else {
            reason
        };
        if let Some(f) = self.props.on_feedback.clone() {
            f(Some(FeedbackKind::Down), Some(reason), window, cx);
        }
        self.sent = true;
        let wait = if motion::reduced_motion(cx) {
            1200
        } else {
            1500
        };
        self.sent_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(wait))
                .await;
            this.update(cx, |s, cx| {
                s.panel = false;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Folds the feedback row away and puts focus back on thumbs down.
    pub fn close_feedback(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.panel = false;
        if let Some(handle) = self.focus.get(&Btn::Down) {
            window.focus(handle, cx);
        }
        cx.notify();
    }

    /// Shows version `ix` (clamped).
    pub fn go_to(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let count = self.props.count.max(1);
        let next = ix.min(count - 1);
        if next == self.version() {
            return;
        }
        if self.props.version.is_none() {
            self.inner = next;
        }
        if let Some(f) = self.props.on_version_change.clone() {
            f(next, window, cx);
        }
        cx.notify();
    }

    pub fn previous_version(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ix = self.version();
        if ix > 0 {
            self.go_to(ix - 1, window, cx);
        }
    }

    pub fn next_version(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ix = self.version();
        self.go_to(ix + 1, window, cx);
    }

    /// Read aloud: starts, or stops a reading in progress.
    pub fn toggle_speaking(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let next = !self.is_speaking();
        if self.props.speaking.is_none() {
            self.speaking = next;
            self.speaking_since = Instant::now();
            self.speak_timer = None;
            if next {
                let words = self
                    .props
                    .text
                    .as_ref()
                    .map_or(0, |t| t.split_whitespace().count());
                let duration = self
                    .props
                    .speak_duration
                    .unwrap_or(Duration::from_millis((words as u64 * 380).max(2600)));
                self.speak_timer = Some(cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor().timer(duration).await;
                    this.update_in(cx, |s, window, cx| {
                        s.speaking = false;
                        if let Some(f) = s.props.on_speak.clone() {
                            f(false, window, cx);
                        }
                        cx.notify();
                    })
                    .ok();
                }));
            }
        } else {
            self.speaking_since = Instant::now();
        }
        if let Some(f) = self.props.on_speak.clone() {
            f(next, window, cx);
        }
        cx.notify();
    }

    fn set_tip(&mut self, btn: Btn, hovered: bool, cx: &mut Context<Self>) {
        if hovered {
            self.tip = Some(btn);
        } else if self.tip == Some(btn) {
            self.tip = None;
        }
        cx.notify();
    }

    fn press(&mut self, btn: Option<Btn>, cx: &mut Context<Self>) {
        if self.pressing != btn {
            self.pressing = btn;
            cx.notify();
        }
    }
}

// ---------------------------------------------------------------------------
// The bar
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct MessageActions {
    id: ElementId,
    props: Props,
    actions: Vec<MessageAction>,
    quiet: bool,
    message_hovered: bool,
    in_tray: bool,
    state: Option<Entity<MessageActionsState>>,
    look: Option<Look>,
}

impl MessageActions {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            props: Props::default(),
            actions: DEFAULT_ACTIONS.to_vec(),
            quiet: true,
            message_hovered: false,
            in_tray: false,
            state: None,
            look: None,
        }
    }

    /// The message text – copied by Copy and used to time Read aloud.
    pub fn text(mut self, text: impl Into<SharedString>) -> Self {
        self.props.text = Some(text.into());
        self
    }

    /// Which actions to show, in this order.
    pub fn actions(mut self, actions: impl IntoIterator<Item = MessageAction>) -> Self {
        self.actions = actions.into_iter().collect();
        self
    }

    /// How many versions the message has; from 2 on the switcher shows.
    pub fn versions(mut self, count: usize) -> Self {
        self.props.count = count.max(1);
        self
    }

    /// Controlled version (0-based).
    pub fn version(mut self, ix: usize) -> Self {
        self.props.version = Some(ix);
        self
    }

    /// Starting version when uncontrolled – the newest by default.
    pub fn default_version(mut self, ix: usize) -> Self {
        self.props.default_version = Some(ix);
        self
    }

    pub fn on_version_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.props.on_version_change = Some(Rc::new(f));
        self
    }

    /// Called after Copy, with the text.
    pub fn on_copy(
        mut self,
        f: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.props.on_copy = Some(Rc::new(f));
        self
    }

    /// Called when Regenerate is pressed – add a version and the switcher jumps to it.
    pub fn on_regenerate(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.props.on_regenerate = Some(Rc::new(f));
        self
    }

    /// Keeps the arrow spinning while a new version is written.
    pub fn regenerating(mut self, regenerating: bool) -> Self {
        self.props.regenerating = regenerating;
        self
    }

    /// `Some(kind)` when a thumb is pressed, `None` when taken back, and
    /// `Some(Down)` with a reason when the feedback row is sent.
    pub fn on_feedback(
        mut self,
        f: impl Fn(Option<FeedbackKind>, Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.props.on_feedback = Some(Rc::new(f));
        self
    }

    /// Quick reasons in the feedback row; "Other" opens a text field.
    pub fn reasons(mut self, reasons: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.props.reasons = reasons.into_iter().map(Into::into).collect();
        self
    }

    /// Called with true when Read aloud starts and false when it stops.
    pub fn on_speak(mut self, f: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.props.on_speak = Some(Rc::new(f));
        self
    }

    /// Controlled playing state, e.g. tied to your speech player.
    pub fn speaking(mut self, speaking: bool) -> Self {
        self.props.speaking = Some(speaking);
        self
    }

    /// How long an uncontrolled reading plays – estimated from the text by default.
    pub fn speak_duration(mut self, duration: Duration) -> Self {
        self.props.speak_duration = Some(duration);
        self
    }

    /// Faded until the message is hovered (default true).
    pub fn quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Whether the message this bar belongs to is hovered – wakes a quiet bar.
    pub fn message_hovered(mut self, hovered: bool) -> Self {
        self.message_hovered = hovered;
        self
    }

    /// Classic: the bar sits in the gray tray under the message, inset 4px
    /// (6px on the left), and the feedback card spans the full width.
    pub fn in_tray(mut self) -> Self {
        self.in_tray = true;
        self
    }

    /// Drives the bar from a state you own instead of one keyed by its id.
    pub fn state(mut self, state: Entity<MessageActionsState>) -> Self {
        self.state = Some(state);
        self
    }

    /// Overrides the global Clean/Classic look.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

/// Per-render context shared by the builders below.
struct Bar {
    look: Look,
    p: Palette,
    m: Metrics,
    key: String,
    state: Entity<MessageActionsState>,
    focus: HashMap<Btn, FocusHandle>,
    chips: Vec<FocusHandle>,
    focused: Option<Btn>,
    keyboard: bool,
    pressing: Option<Btn>,
    olds: HashMap<&'static str, SharedString>,
    /// Width and alpha factor of an inset 0.5px hairline.
    hairline: (Pixels, f32),
    /// Tooltip opacity and rise per button.
    tips: HashMap<Btn, (f32, f32)>,
}

impl Bar {
    fn k(&self, part: &str) -> String {
        format!("{}-{part}", self.key)
    }

    /// The `:active` shrink of a pressed icon.
    fn press_scale(&self, btn: Btn) -> f32 {
        motion::follow(
            &self.k(&format!("press-{}", btn.name())),
            if self.pressing == Some(btn) { 0.9 } else { 1. },
            MotionSpec::new(160, self.m.ease),
        )
    }

    /// A 2px focus outline around a control, shown for keyboard focus.
    fn focus_ring(&self, on: bool, radius: Pixels) -> Option<Div> {
        let off = self.m.focus_offset + 2.;
        on.then(|| {
            div()
                .absolute()
                .top(px(-off))
                .left(px(-off))
                .right(px(-off))
                .bottom(px(-off))
                .rounded(radius + px(off))
                .border_2()
                .border_color(self.p.focus)
        })
    }

    /// Mouse handlers for the `:active` shrink, hover for tooltips.
    fn pressable(&self, btn: Btn, el: Stateful<Div>) -> Stateful<Div> {
        let (down, up, out) = (self.state.clone(), self.state.clone(), self.state.clone());
        el.on_mouse_down(MouseButton::Left, move |_, _, cx| {
            down.update(cx, |s, cx| s.press(Some(btn), cx))
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            up.update(cx, |s, cx| s.press(None, cx))
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            out.update(cx, |s, cx| s.press(None, cx))
        })
    }

    /// A 28px (30px) ghost icon button with its tooltip above it.
    #[allow(clippy::too_many_arguments)]
    fn button(
        &self,
        btn: Btn,
        tip: SharedString,
        rest: HoverColors,
        hover: HoverColors,
        hairline: bool,
        content: impl IntoElement,
        on_click: impl Fn(&mut MessageActionsState, &mut Window, &mut Context<MessageActionsState>)
        + 'static,
    ) -> Stateful<Div> {
        let (p, m) = (&self.p, &self.m);
        let state = self.state.clone();
        let focused = self.keyboard && self.focused == Some(btn);
        let handle = self.focus[&btn].clone();
        let button = div()
            .id(btn.name())
            .track_focus(&handle)
            .tab_index(0)
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .size(m.button)
            .rounded(m.button_radius)
            .cursor_pointer()
            .when(self.look == Look::Clean, |d| d.border(self.hairline.0))
            .hover_colors(
                self.k(&format!("btn-{}", btn.name())),
                HoverColors {
                    border: rest.border.opacity(self.hairline.1),
                    ..rest
                },
                HoverColors {
                    border: hover.border.opacity(self.hairline.1),
                    ..hover
                },
            )
            .when(!hairline, |d| d.border_color(rest.border.opacity(0.)))
            .on_click(move |_, window, cx| state.update(cx, |s, cx| on_click(s, window, cx)))
            .child(content)
            .children(self.focus_ring(focused, m.button_radius));
        let button = self.pressable(btn, button);

        // Tooltip: on hover after a beat, on keyboard focus, and while flashing.
        let (o, y) = self.tips.get(&btn).copied().unwrap_or((0., 0.));
        let slot: &'static str = btn.name();
        let tip = div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(m.button + px(6.))
            .flex()
            .justify_center()
            .child(
                div()
                    .relative()
                    .top(px(3. * (1. - y)))
                    .flex_none()
                    .opacity(o)
                    .py(m.tip_pad.0)
                    .px(m.tip_pad.1)
                    .rounded(px(6.))
                    .bg(p.tip.0)
                    .shadow(p.tip_shadow.clone())
                    .text_size(m.tip_size)
                    .line_height(m.tip_line)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.tip.1)
                    .whitespace_nowrap()
                    .child(swap(
                        &format!("tip-{slot}"),
                        &tip,
                        self.olds.get(slot).cloned(),
                        |s| s.clone().into_any_element(),
                    )),
            );
        let tips = self.state.clone();
        div()
            .id(ElementId::Name(format!("item-{slot}").into()))
            .relative()
            .flex()
            .flex_none()
            .on_hover(move |hovered, _, cx| tips.update(cx, |s, cx| s.set_tip(btn, *hovered, cx)))
            .child(button)
            .child(tip)
    }

    /// Resting and hover colors of a bar button.
    fn colors(
        &self,
        pressed: bool,
        accent: bool,
        done: bool,
        busy: bool,
    ) -> (HoverColors, HoverColors) {
        let p = &self.p;
        let clean = self.look == Look::Clean;
        let mut rest = HoverColors {
            bg: p.wash.opacity(0.),
            border: p.line.opacity(0.),
            fg: if busy { p.ink } else { p.icon },
        };
        let mut hover = HoverColors {
            bg: p.wash,
            border: p.line.opacity(0.),
            fg: p.ink,
        };
        if pressed {
            if clean {
                rest = HoverColors {
                    bg: p.pressed,
                    border: p.line,
                    fg: p.ink,
                };
                hover = rest;
            } else if accent {
                rest = HoverColors {
                    bg: p.accent_wash,
                    border: p.line.opacity(0.),
                    fg: p.accent,
                };
                hover = rest;
            } else {
                rest.fg = p.ink;
            }
        }
        if done {
            rest.fg = p.ok;
            hover.fg = p.ok;
        }
        (rest, hover)
    }
}

impl RenderOnce for MessageActions {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let p = Palette::get(look, window, cx);
        let m = Metrics::get(look);
        let clean = look == Look::Clean;
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);
        let state = match self.state.clone() {
            Some(state) => state,
            None => window.use_keyed_state(
                ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
                cx,
                MessageActionsState::new,
            ),
        };

        let props = self.props.clone();
        let count = props.count.max(1);
        let reasons = props.reasons.clone();
        let regenerating = props.regenerating;
        let (placeholder, caret) = (p.faint, p.ink);
        state.update(cx, |s, cx| {
            s.sync(props, cx);
            s.other.update(cx, |input, _| {
                input.set_colors(Some(placeholder), Some(caret), None)
            });
        });

        // A snapshot of the state for this frame.
        let s = state.read(cx);
        let index = s.version();
        let playing = s.is_speaking();
        let (vote, voted_at, panel, reason, sent, copied) =
            (s.vote, s.voted_at, s.panel, s.reason, s.sent, s.copied);
        let (spun_at, speaking_since, born) = (s.spun_at, s.speaking_since, s.born);
        let (tip, pressing, keyboard, hovered) = (s.tip, s.pressing, s.keyboard, s.hovered);
        let focus = s.focus.clone();
        let chip_focus = s.chips.clone();
        let other = s.other.clone();
        let cells = (
            s.versions_width.clone(),
            s.panel_height.clone(),
            s.other_height.clone(),
        );
        let focused = focus
            .iter()
            .find(|(_, h)| h.is_focused(window))
            .map(|(b, _)| *b);
        let focused_chip = chip_focus.iter().position(|h| h.is_focused(window));
        let focus_within =
            focused.is_some() || focused_chip.is_some() || other.read(cx).is_focused(window);
        let other_open = reason
            .and_then(|ix| reasons.get(ix))
            .is_some_and(|r| r.as_ref() == OTHER)
            && !sent;

        // Labels that crossfade when they change.
        let up_tip: SharedString = if vote == Some(FeedbackKind::Up) {
            "Thanks!".into()
        } else {
            "Good response".into()
        };
        let labels: [(&'static str, SharedString); 8] = [
            ("copy", if copied { "Copied" } else { "Copy" }.into()),
            (
                "regenerate",
                if regenerating {
                    "Writing…"
                } else {
                    "Regenerate"
                }
                .into(),
            ),
            ("up", up_tip),
            ("down", "Bad response".into()),
            ("speak", if playing { "Stop" } else { "Read aloud" }.into()),
            ("count-now", (index + 1).to_string().into()),
            ("count-total", count.to_string().into()),
            ("send", if sent { "sent" } else { "send" }.into()),
        ];
        let now = Instant::now();
        let olds = state.update(cx, |s, _| {
            labels
                .iter()
                .filter_map(|(slot, id)| s.swaps.track(slot, id, now).map(|old| (*slot, old)))
                .collect::<HashMap<_, _>>()
        });
        let label = |slot: &str| {
            labels
                .iter()
                .find(|(s, _)| *s == slot)
                .map(|(_, l)| l.clone())
                .unwrap_or_default()
        };

        // Fades with a real delay: tooltips wait 380ms on hover, folds and
        // the switcher fade in a beat after they start to open.
        let ease = m.ease;
        let (tips, panel_o, other_o, versions_o, draw, fading) = state.update(cx, |s, _| {
            let f = &mut s.fades;
            f.moving = false;
            let mut tips = HashMap::new();
            for btn in [Btn::Copy, Btn::Regenerate, Btn::Up, Btn::Down, Btn::Speak] {
                let kb = keyboard && focused == Some(btn);
                let flash = btn == Btn::Copy && copied;
                let show = tip == Some(btn) || kb || flash;
                let delay = if show && !kb && !flash { TIP_DELAY } else { 0 };
                let o = f.ease(
                    ("tip-o", btn.name()),
                    show,
                    TIP_FADE.with_delay(delay),
                    now,
                    reduced,
                );
                let y = f.ease(
                    ("tip-y", btn.name()),
                    show,
                    MotionSpec::new(200, ease).with_delay(delay),
                    now,
                    reduced,
                );
                tips.insert(btn, (o, y));
            }
            let fold_spec = |open: bool| if open { FOLD_OPEN } else { FOLD_CLOSE };
            let panel_o = f.ease(("fold", "panel"), panel, fold_spec(panel), now, reduced);
            let other_o = f.ease(
                ("fold", "other"),
                other_open,
                fold_spec(other_open),
                now,
                reduced,
            );
            let switcher = count > 1;
            let versions_o = f.ease(
                ("versions", "o"),
                switcher,
                if switcher {
                    MotionSpec::new(320, EASE).with_delay(100)
                } else {
                    MotionSpec::new(220, EASE)
                },
                now,
                reduced,
            );
            // Classic draws the check stroke in (dashoffset, 340ms after
            // 60ms) and drops it at once when it goes.
            let draw = f.ease(
                ("copy", "draw"),
                copied,
                if copied {
                    MotionSpec::new(340, ease).with_delay(60)
                } else {
                    MotionSpec::new(0, EASE)
                },
                now,
                reduced,
            );
            (tips, panel_o, other_o, versions_o, draw, f.moving)
        });

        let bar = Bar {
            look,
            p,
            m,
            key: key.clone(),
            state: state.clone(),
            focus: focus.clone(),
            chips: chip_focus.clone(),
            focused,
            keyboard,
            pressing,
            olds,
            hairline: {
                let (w, c) = inset_hairline(window, black_a(1.));
                (w, c.a)
            },
            tips,
        };
        let (p, m) = (&bar.p, &bar.m);
        let mut animating = fading;

        let mut parts: Vec<AnyElement> = Vec::new();
        for action in &self.actions {
            match action {
                MessageAction::Copy => {
                    // The copy glyph blurs away while the check comes in.
                    let t = eased(&bar.k("copy-o"), copied, MotionSpec::new(200, EASE));
                    let sc = eased(&bar.k("copy-s"), copied, MotionSpec::new(300, m.ease));
                    let press = bar.press_scale(Btn::Copy);
                    let (rest, hover) = bar.colors(false, false, copied, false);
                    let fg = motion::mix(rest.fg, hover.fg, motion::hover_t(&bar.k("btn-copy")));
                    let check = glyph(
                        icon(look, "check"),
                        px(16.),
                        fg.opacity(t),
                        motion::lerp(0.6, 1., sc) * press,
                        0.,
                    );
                    // Classic draws the check stroke in; a left-to-right wipe stands in.
                    let draw = if clean { 1. } else { draw };
                    let content = div()
                        .relative()
                        .size(px(16.))
                        .child(div().absolute().top_0().left_0().child(glyph(
                            icon(look, "copy"),
                            px(16.),
                            fg.opacity(1. - t),
                            motion::lerp(1., 0.6, sc) * press,
                            0.,
                        )))
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .h(px(16.))
                                .w(px(16. * draw))
                                .overflow_hidden()
                                .child(check),
                        );
                    parts.push(
                        bar.button(
                            Btn::Copy,
                            label("copy"),
                            rest,
                            hover,
                            false,
                            content,
                            |s, window, cx| s.copy(window, cx),
                        )
                        .into_any_element(),
                    );
                }
                MessageAction::Regenerate => {
                    let (turn, spinning) = match (reduced, spun_at.or(regenerating.then_some(born)))
                    {
                        (false, Some(at)) => {
                            let e = ms(now.saturating_duration_since(at));
                            if regenerating {
                                ((e / 900.).fract() * TAU, true)
                            } else if e < 640. {
                                (m.ease.eval(e / 640.) * TAU, true)
                            } else {
                                (0., false)
                            }
                        }
                        _ => (0., false),
                    };
                    animating |= spinning;
                    let (rest, hover) = bar.colors(false, false, false, regenerating);
                    let fg =
                        motion::mix(rest.fg, hover.fg, motion::hover_t(&bar.k("btn-regenerate")));
                    let press = bar.press_scale(Btn::Regenerate);
                    let button = bar.button(
                        Btn::Regenerate,
                        label("regenerate"),
                        rest,
                        hover,
                        false,
                        glyph(icon(look, "redo"), px(16.), fg, press, turn),
                        |s, window, cx| s.regenerate(window, cx),
                    );
                    parts.push(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .child(button)
                            .child(versions(
                                &bar,
                                index,
                                count,
                                cells.0.clone(),
                                versions_o,
                                &label,
                            ))
                            .into_any_element(),
                    );
                }
                MessageAction::Feedback => {
                    for (btn, kind) in
                        [(Btn::Up, FeedbackKind::Up), (Btn::Down, FeedbackKind::Down)]
                    {
                        let pressed = vote == Some(kind);
                        let name = if kind == FeedbackKind::Up {
                            "thumb-up"
                        } else {
                            "thumb-down"
                        };
                        let (pop_scale, pop_turn) = match (pressed, voted_at, reduced) {
                            (true, Some(at), false) => {
                                let t = ms(now.saturating_duration_since(at)) / 420.;
                                animating |= t < 1.;
                                pop(t, 1. + m.pop, m.ease)
                            }
                            _ => (1., 0.),
                        };
                        let (rest, hover) = bar.colors(pressed, false, false, false);
                        let fg = motion::mix(
                            rest.fg,
                            hover.fg,
                            motion::hover_t(&bar.k(&format!("btn-{}", btn.name()))),
                        );
                        let scale = pop_scale * bar.press_scale(btn);
                        // Classic fills the outline when pressed (`fill 200ms`).
                        let fill = if clean {
                            0.
                        } else {
                            eased(
                                &bar.k(&format!("fill-{}", btn.name())),
                                pressed,
                                MotionSpec::new(200, EASE),
                            )
                        };
                        let content = div()
                            .relative()
                            .size(px(16.))
                            .child(div().absolute().top_0().left_0().child(glyph(
                                icon(look, name),
                                px(16.),
                                fg,
                                scale,
                                pop_turn,
                            )))
                            .when(!clean, |d| {
                                d.child(div().absolute().top_0().left_0().child(glyph(
                                    icon(look, &format!("{name}-filled")),
                                    px(16.),
                                    fg.opacity(fill),
                                    scale,
                                    pop_turn,
                                )))
                            });
                        let tip = if btn == Btn::Up {
                            label("up")
                        } else {
                            label("down")
                        };
                        parts.push(
                            bar.button(
                                btn,
                                tip,
                                rest,
                                hover,
                                pressed,
                                content,
                                move |s, window, cx| {
                                    if btn == Btn::Up {
                                        s.thumbs_up(window, cx)
                                    } else {
                                        s.thumbs_down(window, cx)
                                    }
                                },
                            )
                            .into_any_element(),
                        );
                    }
                }
                MessageAction::Speak => {
                    let t = eased(&bar.k("speak-o"), playing, MotionSpec::new(200, EASE));
                    let sc = eased(&bar.k("speak-s"), playing, MotionSpec::new(300, m.ease));
                    let press = bar.press_scale(Btn::Speak);
                    let (rest, hover) = bar.colors(playing, true, false, false);
                    let fg = motion::mix(rest.fg, hover.fg, motion::hover_t(&bar.k("btn-speak")));
                    let eq_color = if clean { p.accent } else { fg };
                    let elapsed = ms(now.saturating_duration_since(speaking_since));
                    animating |= playing && !reduced;
                    let layer = motion::lerp(0.6, 1., sc) * press;
                    let bars = (0..4).map(|i| {
                        let s = if !playing {
                            0.3
                        } else if reduced {
                            if i % 2 == 0 { 0.8 } else { 0.5 }
                        } else {
                            eq_scale(elapsed, i)
                        };
                        div()
                            .w(px(2.))
                            .h(px(m.eq_bar * s * layer))
                            .rounded(px(1.))
                            .bg(eq_color.opacity(t))
                    });
                    let content = div()
                        .relative()
                        .size(px(16.))
                        .child(div().absolute().top_0().left_0().child(glyph(
                            icon(look, "speaker"),
                            px(16.),
                            fg.opacity(1. - t),
                            motion::lerp(1., 0.6, sc) * press,
                            0.,
                        )))
                        .child(
                            div()
                                .absolute()
                                .inset_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(2.))
                                .children(bars),
                        );
                    parts.push(
                        bar.button(
                            Btn::Speak,
                            label("speak"),
                            rest,
                            hover,
                            playing,
                            content,
                            |s, window, cx| s.toggle_speaking(window, cx),
                        )
                        .into_any_element(),
                    );
                }
            }
        }

        // The bar is quiet until the message is hovered, focused or busy.
        let active = copied || panel || playing || regenerating;
        let awake = !self.quiet || hovered || self.message_hovered || focus_within || active;
        let opacity = motion::follow(
            &bar.k("quiet"),
            if awake { 1. } else { 0.6 },
            MotionSpec::new(240, EASE),
        );
        let tray = self.in_tray && !clean;
        let bar_el = div()
            .flex()
            .items_center()
            .gap(px(2.))
            .min_h(m.button)
            .opacity(opacity)
            .when(tray, |d| d.pt(px(4.)).pr(px(4.)).pb(px(4.)).pl(px(6.)))
            .children(parts);

        let feedback = self.actions.contains(&MessageAction::Feedback).then(|| {
            feedback_row(
                &bar,
                &reasons,
                reason,
                focused_chip,
                sent,
                panel,
                other_open,
                tray,
                &other,
                &label,
                (cells.1.clone(), panel_o),
                (cells.2.clone(), other_o),
            )
        });

        if animating {
            window.request_animation_frame();
        }
        // Followers started in this render need frames; the story's
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);

        let hover_state = state.clone();
        let key_state = state.clone();
        let mouse_state = state.clone();
        div()
            .id(self.id.clone())
            .w_full()
            .min_w_0()
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(if clean { p.ink } else { p.text })
            .on_hover(move |h, _, cx| {
                hover_state.update(cx, |s, cx| {
                    s.hovered = *h;
                    cx.notify();
                })
            })
            .capture_any_mouse_down(move |_, _, cx| {
                mouse_state.update(cx, |s, cx| {
                    if s.keyboard {
                        s.keyboard = false;
                        cx.notify();
                    }
                })
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let stroke = &event.keystroke;
                key_state.update(cx, |s, cx| {
                    s.keyboard = true;
                    cx.notify();
                });
                if stroke.key == "tab" {
                    if stroke.modifiers.shift {
                        window.focus_prev(cx);
                    } else {
                        window.focus_next(cx);
                    }
                    cx.stop_propagation();
                }
            })
            .child(bar_el)
            .children(feedback)
    }
}

/// The "‹ 2 / 3 ›" switcher: unfolds next to Regenerate from two versions on.
fn versions(
    bar: &Bar,
    index: usize,
    count: usize,
    cell: Rc<Cell<f32>>,
    o: f32,
    label: &dyn Fn(&str) -> SharedString,
) -> impl IntoElement {
    let (p, m) = (&bar.p, &bar.m);
    let open = count > 1;
    // Closed, the `0fr` track still holds the clip's 2px right padding.
    let w = motion::follow(
        &bar.k("versions-w"),
        if open { cell.get().max(2.) } else { 2. },
        MotionSpec::new(420, m.ease),
    );
    let step = |btn: Btn, enabled: bool| {
        let state = bar.state.clone();
        let focused = bar.keyboard && bar.focused == Some(btn);
        let dim = eased(
            &bar.k(&format!("step-dim-{}", btn.name())),
            !enabled,
            MotionSpec::new(160, EASE),
        );
        let hover_key = bar.k(&format!("step-{}", btn.name()));
        let t = if enabled {
            motion::hover_t(&hover_key)
        } else {
            0.
        };
        let path = icon(bar.look, if btn == Btn::Prev { "prev" } else { "next" });
        let el = div()
            .id(btn.name())
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(20.))
            .h(m.button)
            .rounded(m.step_radius)
            .opacity(motion::lerp(1., m.step_disabled, dim))
            .bg(motion::mix(p.wash.opacity(0.), p.wash, t))
            .on_hover(motion::hover_listener(hover_key))
            .child(glyph(
                path,
                m.step_glyph,
                motion::mix(p.icon, p.ink, t),
                1.,
                0.,
            ))
            .children(bar.focus_ring(focused, m.step_radius));
        if open && enabled {
            el.track_focus(&bar.focus[&btn])
                .tab_index(0)
                .cursor_pointer()
                .on_click(move |_, window, cx| {
                    state.update(cx, |s, cx| {
                        if btn == Btn::Prev {
                            s.previous_version(window, cx)
                        } else {
                            s.next_version(window, cx)
                        }
                    })
                })
        } else {
            el
        }
    };
    let number = |slot: &'static str, color: Hsla, weight: FontWeight| {
        swap(slot, &label(slot), bar.olds.get(slot).cloned(), move |s| {
            div()
                .text_color(color)
                .font_weight(weight)
                .child(s.clone())
                .into_any_element()
        })
    };
    let clean = bar.look == Look::Clean;
    let count_el = div()
        .flex()
        .items_center()
        .gap(px(2.))
        .px(m.count_pad)
        .text_size(px(12.))
        .line_height(m.count_line)
        .text_color(if clean { p.faint } else { p.muted })
        .whitespace_nowrap()
        .child(number("count-now", p.ink, FontWeight::MEDIUM))
        .child("/")
        .child(number("count-total", p.muted, FontWeight::NORMAL));
    div().flex().w(px(w)).overflow_hidden().opacity(o).child(
        div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .pr(px(2.))
            .child(step(Btn::Prev, index > 0))
            .child(count_el)
            .child(step(Btn::Next, index + 1 < count))
            .child(measure(cell, true)),
    )
}

/// "What went wrong?" with the reasons, the send button and the "Other" field.
#[allow(clippy::too_many_arguments)]
fn feedback_row(
    bar: &Bar,
    reasons: &[SharedString],
    picked: Option<usize>,
    focused_chip: Option<usize>,
    sent: bool,
    open: bool,
    other_open: bool,
    tray: bool,
    other: &Entity<TextInput>,
    label: &dyn Fn(&str) -> SharedString,
    // The measured height and the opacity of each fold.
    panel_fold: (Rc<Cell<f32>>, f32),
    other_fold: (Rc<Cell<f32>>, f32),
) -> impl IntoElement {
    let (p, m) = (&bar.p, &bar.m);
    let clean = bar.look == Look::Clean;
    let state = bar.state.clone();

    let close_key = bar.k("close");
    let close_t = motion::hover_t(&close_key);
    let close_state = state.clone();
    let close = div()
        .id("close")
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(24.))
        .mr(px(-4.))
        .rounded(m.close_radius)
        .cursor_pointer()
        .bg(motion::mix(p.wash.opacity(0.), p.wash, close_t))
        .on_hover(motion::hover_listener(close_key))
        .child(glyph(
            icon(bar.look, "close"),
            m.close_glyph,
            motion::mix(p.icon, p.ink, close_t),
            1.,
            0.,
        ))
        .children(bar.focus_ring(
            bar.keyboard && bar.focused == Some(Btn::Close),
            m.close_radius,
        ))
        .on_click(move |_, window, cx| close_state.update(cx, |s, cx| s.close_feedback(window, cx)))
        .when(open, |d| {
            d.track_focus(&bar.focus[&Btn::Close]).tab_index(0)
        });

    let head = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .min_h(px(24.))
        .child(
            div()
                .text_size(m.ask_size)
                .line_height(px(20.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.strong)
                .child("What went wrong?"),
        )
        .child(close);

    let chips = reasons.iter().enumerate().map(|(ix, reason)| {
        let on = picked == Some(ix);
        let state = state.clone();
        let hover_key = bar.k(&format!("chip-{ix}"));
        let t = if sent {
            0.
        } else {
            motion::hover_t(&hover_key)
        };
        let on_t = eased(
            &bar.k(&format!("chip-on-{ix}")),
            on,
            MotionSpec::new(140, EASE),
        );
        let (bg, fg) = (
            motion::mix(p.chip, p.chip_hover, t),
            motion::mix(p.text, p.ink, t),
        );
        let (bg, fg) = (
            motion::mix(bg, p.chip_on.0, on_t),
            motion::mix(fg, p.chip_on.1, on_t),
        );
        let dim = eased(
            &bar.k(&format!("chip-dim-{ix}")),
            sent && !on,
            MotionSpec::new(200, EASE),
        );
        let handle = bar.chips.get(ix).cloned();
        let focused = bar.keyboard && focused_chip == Some(ix);
        div()
            .id(ElementId::NamedInteger("chip".into(), ix as u64))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(m.chip_height)
            .px(m.chip_pad - if clean { bar.hairline.0 } else { px(0.) })
            .rounded(m.chip_radius)
            .when(clean, |d| {
                d.border(bar.hairline.0)
                    .border_color(p.line.opacity(bar.hairline.1))
            })
            .bg(bg)
            .text_color(fg)
            .text_size(m.chip_size)
            .line_height(px(20.))
            .font_weight(m.chip_weight)
            .whitespace_nowrap()
            .opacity(motion::lerp(1., 0.45, dim))
            .on_hover(motion::hover_listener(hover_key))
            .child(reason.clone())
            .children(bar.focus_ring(focused, m.chip_radius))
            .when(!sent, |d| {
                d.cursor_pointer()
                    .on_click(move |_, _, cx| state.update(cx, |s, cx| s.toggle_reason(ix, cx)))
            })
            .when_some(handle.filter(|_| open && !sent), |d, h| {
                d.track_focus(&h).tab_index(0)
            })
    });

    // Send: the dark chip; once sent a soft green fill with a tick.
    let can_send = picked.is_some();
    let send_key = bar.k("send");
    let send_t = if can_send && !sent {
        motion::hover_t(&send_key)
    } else {
        0.
    };
    let sent_t = eased(&bar.k("sent"), sent, MotionSpec::new(220, EASE));
    let send_off = eased(&bar.k("send-off"), !can_send, MotionSpec::new(200, EASE));
    let bg = motion::mix(
        motion::mix(p.send.0, p.send_hover, send_t),
        p.ok_wash,
        sent_t,
    );
    let fg = motion::mix(p.send.1, p.ok, sent_t);
    let tick = icon(bar.look, "tick");
    let tick_size = m.tick;
    let send_state = state.clone();
    let send = div()
        .id("send")
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(m.send_min)
        .h(m.chip_height)
        .px(m.send_pad)
        .rounded(m.chip_radius)
        .bg(bg)
        .text_color(fg)
        .text_size(m.send_size)
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .whitespace_nowrap()
        .opacity(motion::lerp(1., m.send_disabled, send_off))
        .on_hover(motion::hover_listener(send_key))
        .child(swap(
            "send",
            &label("send"),
            bar.olds.get("send").cloned(),
            move |id| {
                if id.as_ref() == "sent" {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(glyph(tick.clone(), tick_size, fg, 1., 0.))
                        .child("Thanks!")
                        .into_any_element()
                } else {
                    "Send".into_any_element()
                }
            },
        ))
        .children(bar.focus_ring(
            bar.keyboard && bar.focused == Some(Btn::Send),
            m.chip_radius,
        ))
        .when(can_send && !sent, |d| {
            d.cursor_pointer()
                .on_click(move |_, window, cx| send_state.update(cx, |s, cx| s.send(window, cx)))
        })
        .when(open && can_send && !sent, |d| {
            d.track_focus(&bar.focus[&Btn::Send]).tab_index(0)
        });

    let field = div()
        .mt(px(8.))
        .h(m.field_height)
        .px(px(10.))
        .flex()
        .items_center()
        .rounded(m.field_radius)
        .bg(p.field)
        .shadow(p.field_shadow.clone())
        .text_size(m.field_size)
        .line_height(px(20.))
        .text_color(p.ink)
        .child(other.clone());

    let panel = div()
        .child(head)
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(m.chip_gap)
                .mt(m.chip_gap)
                .children(chips)
                // `margin-left: auto` – as a growing wrapper, since taffy drops
                // the gaps of a wrapped line that has an auto margin.
                .child(div().flex().flex_grow_1().justify_end().child(send)),
        )
        .child(fold(
            &bar.k("other"),
            other_open,
            other_fold.1,
            m.ease,
            other_fold.0,
            div().child(field),
        ));
    let panel = if clean {
        panel.pt(px(10.)).px(px(8.)).pb(px(8.))
    } else {
        panel
            .pt(px(12.))
            .px(px(14.))
            .pb(px(14.))
            .rounded(px(16.))
            .bg(p.card)
            .shadow(p.panel_shadow.clone())
            .when(!tray, |d| d.max_w(px(440.)))
    };
    let escape_state = state.clone();
    let panel = div()
        .id("feedback")
        .when(!clean, |d| d.pt(px(if tray { 2. } else { 6. })))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                cx.stop_propagation();
                escape_state.update(cx, |s, cx| s.close_feedback(window, cx));
            }
        })
        .child(panel);
    fold(
        &bar.k("panel"),
        open,
        panel_fold.1,
        m.ease,
        panel_fold.0,
        div().child(panel),
    )
}

// ---------------------------------------------------------------------------
// Message text
// ---------------------------------------------------------------------------

struct TextState {
    version: usize,
    text: SharedString,
    out: Option<(SharedString, usize, Instant)>,
    height: Rc<Cell<f32>>,
}

/// The message itself, crossfading when `version` changes: the old text fades
/// out underneath while the new one fades in, and the block eases to its new
/// height.
#[derive(IntoElement)]
pub struct MessageText {
    id: ElementId,
    version: usize,
    text: SharedString,
    look: Option<Look>,
}

impl MessageText {
    pub fn new(id: impl Into<ElementId>, version: usize, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            version,
            text: text.into(),
            look: None,
        }
    }

    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for MessageText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let ease = Metrics::get(look).ease;
        let key = self.id.to_string();
        let (version, text) = (self.version, self.text.clone());
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "text".into()),
            cx,
            |_, _| TextState {
                version,
                text: text.clone(),
                out: None,
                height: Rc::new(Cell::new(0.)),
            },
        );
        let now = Instant::now();
        let (out, cell) = state.update(cx, |s, _| {
            if s.version != version {
                let old = std::mem::replace(&mut s.text, text.clone());
                s.out = Some((old, s.version, now));
                s.version = version;
            } else {
                s.text = text.clone();
            }
            if s.out
                .as_ref()
                .is_some_and(|(_, _, at)| now.saturating_duration_since(*at) >= SWAP_HOLD)
            {
                s.out = None;
            }
            (s.out.clone(), s.height.clone())
        });
        let measured = cell.get();
        let height = (measured > 0.)
            .then(|| motion::follow(&format!("{key}-h"), measured, MotionSpec::new(360, ease)));
        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .overflow_hidden()
            .when_some(height, |d, h| d.h(px(h)))
            .child(
                div()
                    .relative()
                    .mr(-HANGING_SPACE)
                    .child(div().child(text).with_animation(
                        ElementId::NamedInteger("in".into(), version as u64),
                        MotionSpec::new(420, EASE).animation(),
                        |el, t| el.opacity(t),
                    ))
                    .when_some(out, |d, (old, v, _)| {
                        d.child(div().absolute().top_0().left_0().right_0().child(
                            div().child(old).with_animation(
                                ElementId::NamedInteger("out".into(), v as u64),
                                MotionSpec::new(260, EASE).animation(),
                                |el, t| el.opacity(1. - t),
                            ),
                        ))
                    })
                    .child(measure(cell, false)),
            )
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

const ASK: &str = "Summarize the 2.4 release notes in one sentence.";
const MODEL: &str = "Sonnet 5.5";
const ANSWERS: [&str; 3] = [
    "Version 2.4 adds shared workspaces, cuts sync time in half and fixes the export bug on Windows.",
    "Shared workspaces arrive in 2.4, sync is twice as fast, and exports on Windows work again.",
    "In short: teams can now share a workspace, syncing takes half as long and Windows exports are fixed.",
];

/// One beat of the demo's auto-play: what it clicks after how long.
#[derive(Clone, Copy)]
enum Step {
    Copy,
    ThumbsDown,
    Reason(usize),
    Send,
    Regenerate,
    Previous,
    Next,
    Restart,
}

const SCRIPT: [(u64, Step); 8] = [
    (700, Step::Copy),
    (1000, Step::ThumbsDown),
    (700, Step::Reason(1)),
    (900, Step::Send),
    (2600, Step::Regenerate),
    (2800, Step::Previous),
    (2000, Step::Next),
    (3000, Step::Restart),
];

struct Demo {
    round: u64,
    versions: usize,
    version: usize,
    writing: bool,
    /// The viewer took over: auto-play stopped and Copy copies the answer.
    live: bool,
    message_hovered: bool,
    actions: Entity<MessageActionsState>,
    autoplay: Option<Task<()>>,
    writing_task: Option<Task<()>>,
}

impl Demo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let actions = cx.new(|cx| MessageActionsState::new(window, cx));
        let autoplay = Some(Self::autoplay(actions.clone(), window, cx));
        Demo {
            round: 0,
            versions: 1,
            version: 0,
            writing: false,
            live: false,
            message_hovered: false,
            actions,
            autoplay,
            writing_task: None,
        }
    }

    fn autoplay(
        first: Entity<MessageActionsState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn_in(window, async move |demo, cx| {
            let mut actions = first;
            loop {
                for (wait, step) in SCRIPT {
                    cx.background_executor()
                        .timer(Duration::from_millis(wait))
                        .await;
                    let next = cx.update(|window, cx| {
                        let a = &actions;
                        match step {
                            Step::Copy => a.update(cx, |s, cx| s.copy(window, cx)),
                            Step::ThumbsDown => a.update(cx, |s, cx| s.thumbs_down(window, cx)),
                            Step::Reason(ix) => a.update(cx, |s, cx| s.toggle_reason(ix, cx)),
                            Step::Send => a.update(cx, |s, cx| s.send(window, cx)),
                            Step::Regenerate => a.update(cx, |s, cx| s.regenerate(window, cx)),
                            Step::Previous => a.update(cx, |s, cx| s.previous_version(window, cx)),
                            Step::Next => a.update(cx, |s, cx| s.next_version(window, cx)),
                            Step::Restart => {
                                return demo.update(cx, |d, cx| d.restart(window, cx)).ok();
                            }
                        }
                        None
                    });
                    match next {
                        Ok(Some(fresh)) => actions = fresh,
                        Ok(None) => {}
                        Err(_) => return,
                    }
                }
            }
        })
    }

    /// Back to one version with a fresh bar, like the demo's `key` bump.
    fn restart(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<MessageActionsState> {
        self.round += 1;
        self.versions = 1;
        self.version = 0;
        self.writing = false;
        self.writing_task = None;
        self.actions = cx.new(|cx| MessageActionsState::new(window, cx));
        cx.notify();
        self.actions.clone()
    }

    /// The demo's `onRegenerate`: "writes" for a second, then adds a version.
    fn regenerate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.writing {
            return;
        }
        self.writing = true;
        self.writing_task = Some(cx.spawn_in(window, async move |demo, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1000))
                .await;
            demo.update(cx, |d, cx| {
                d.writing = false;
                d.versions = (d.versions + 1).min(ANSWERS.len());
                d.version = d.versions - 1;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn take_over(&mut self, cx: &mut Context<Self>) {
        if !self.live {
            self.live = true;
            self.autoplay = None;
            cx.notify();
        }
    }
}

/// The demo stage tokens (`.ma-demo__*` / `.mac-demo__*`).
struct Stage {
    tray: Option<Hsla>,
    card: Hsla,
    card_shadow: Vec<BoxShadow>,
    card_text: Hsla,
    ask: (Hsla, Hsla),
    ask_edge: Option<Hsla>,
    who: Hsla,
    answer: Hsla,
    foot: Hsla,
}

impl Stage {
    fn get(look: Look, dark: bool) -> Self {
        match (look, dark) {
            (Look::Clean, false) => Stage {
                tray: None,
                card: hex(0xffffff),
                card_shadow: vec![
                    shadow(0., 0., 0., 0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                card_text: black_a(0.88),
                ask: (black_a(0.03), black_a(0.88)),
                ask_edge: Some(black_a(0.06)),
                who: black_a(0.45),
                answer: black_a(0.88),
                foot: black_a(0.06),
            },
            (Look::Clean, true) => Stage {
                tray: None,
                card: hex(0x1c1c1e),
                card_shadow: vec![
                    shadow(0., 0., 0., 0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                card_text: white_a(0.9),
                ask: (white_a(0.04), white_a(0.9)),
                ask_edge: Some(white_a(0.08)),
                who: white_a(0.46),
                answer: white_a(0.9),
                foot: white_a(0.08),
            },
            (Look::Classic, false) => Stage {
                tray: Some(hex(0xf0f0f0)),
                card: hex(0xffffff),
                card_shadow: vec![
                    ring(1., hex(0xececec)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ],
                card_text: hex(0x404040),
                ask: (hex(0xf4f4f5), hex(0x171717)),
                ask_edge: None,
                who: hex(0x8f8f8f),
                answer: hex(0x171717),
                foot: hex(0xececec),
            },
            (Look::Classic, true) => Stage {
                tray: Some(hex(0x111113)),
                card: hex(0x19191b),
                card_shadow: vec![ring(1., hex(0x2a2a2e))],
                card_text: hex(0xc8c8cc),
                ask: (hex(0x222225), hex(0xededed)),
                ask_edge: None,
                who: hex(0x8b8b92),
                answer: hex(0xededed),
                foot: hex(0x2a2a2e),
            },
        }
    }
}

fn demo_view(demo: &Entity<Demo>, window: &Window, cx: &mut App) -> Stateful<Div> {
    let look = cx.look();
    let clean = look == Look::Clean;
    let mut st = Stage::get(look, cx.is_dark());
    st.card_shadow = css_shadows(st.card_shadow, window.scale_factor());
    let d = demo.read(cx);
    let answer = SharedString::from(ANSWERS[d.version.min(ANSWERS.len() - 1)]);
    let weak = demo.downgrade();
    // The ask bubble's inset hairline is a border here, so it takes its
    // width out of the padding.
    let edge_w = if st.ask_edge.is_some() {
        hairline(window)
    } else {
        px(0.)
    };

    let (on_regenerate, on_version) = (weak.clone(), weak.clone());
    let actions = MessageActions::new(ElementId::NamedInteger("message-actions".into(), d.round))
        .state(d.actions.clone())
        .versions(d.versions)
        .version(d.version)
        .regenerating(d.writing)
        .message_hovered(d.message_hovered)
        .on_regenerate(move |window, cx| {
            on_regenerate
                .update(cx, |d, cx| d.regenerate(window, cx))
                .ok();
        })
        .on_version_change(move |ix, _, cx| {
            on_version
                .update(cx, |d, cx| {
                    d.version = ix;
                    cx.notify();
                })
                .ok();
        })
        .when(d.live, |a| a.text(answer.clone()))
        .when(!clean, |a| a.in_tray());

    // While a new version is written the answer breathes (`ma-demo-writing`).
    let text =
        div()
            .text_color(st.answer)
            .child(MessageText::new("ma-answer", d.version, answer.clone()));
    let text = if d.writing {
        text.with_animation(
            "ma-writing",
            Animation::new(Duration::from_millis(1000)).repeat(),
            |el, t| {
                let k = if t < 0.5 { t * 2. } else { 2. - t * 2. };
                el.opacity(1. - 0.6 * EASE_IN_OUT.eval(k))
            },
        )
        .into_any_element()
    } else {
        text.into_any_element()
    };

    let body = div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .justify_end()
                .mb(px(if clean { 14. } else { 12. }))
                .child(
                    div()
                        .max_w(relative(0.85))
                        .py(px(6.) - edge_w)
                        .px(px(10.) - edge_w)
                        .rounded(px(if clean { 8. } else { 12. }))
                        .bg(st.ask.0)
                        .text_color(st.ask.1)
                        .when_some(st.ask_edge, |d, edge| {
                            d.border(edge_w)
                                .border_color(inset_hairline(window, edge).1)
                        })
                        .child(ASK),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .mb(px(4.))
                .text_size(px(12.))
                .when(clean, |d| d.line_height(px(16.)))
                .text_color(st.who)
                .child(
                    div()
                        .size(px(8.))
                        .rounded(px(if clean { 2. } else { 3. }))
                        .bg(hex(0xd97757)),
                )
                .child(MODEL),
        )
        .child(text);

    let hover = weak.clone();
    let message = div()
        .id("ma-message")
        .on_hover(move |h, _, cx| {
            hover
                .update(cx, |d, cx| {
                    d.message_hovered = *h;
                    cx.notify();
                })
                .ok();
        })
        .text_size(px(13.))
        .line_height(px(20.));
    let message = if clean {
        message
            .rounded(px(10.))
            .bg(st.card)
            .shadow(st.card_shadow)
            .text_color(st.card_text)
            .child(body.pt(px(16.)).px(px(16.)).pb(px(14.)))
            .child(
                div()
                    .border_t(hairline(window))
                    .border_color(st.foot)
                    .p(px(8.))
                    .child(actions),
            )
    } else {
        message
            .p(px(4.))
            .rounded(px(20.))
            .when_some(st.tray, |d, tray| d.bg(tray))
            .child(
                body.pt(px(12.))
                    .px(px(14.))
                    .pb(px(14.))
                    .rounded(px(16.))
                    .bg(st.card)
                    .shadow(st.card_shadow)
                    .text_color(st.card_text),
            )
            .child(actions)
    };

    // Any pointer or key on the demo stops the auto-play.
    let (enter, press, key) = (weak.clone(), weak.clone(), weak);
    div()
        .id("ma-demo")
        .w(px(400.))
        .h(px(330.))
        .flex()
        .flex_col()
        .on_hover(move |h, _, cx| {
            if *h {
                enter.update(cx, |d, cx| d.take_over(cx)).ok();
            }
        })
        .capture_any_mouse_down(move |_, _, cx| {
            press.update(cx, |d, cx| d.take_over(cx)).ok();
        })
        .capture_key_down(move |_, _, cx| {
            key.update(cx, |d, cx| d.take_over(cx)).ok();
        })
        .child(message)
}

pub fn story(window: &mut Window, cx: &mut App) -> AnyView {
    let demo = cx.new(|cx| Demo::new(window, cx));
    story::view(cx, move |window, cx| demo_view(&demo, window, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_peaks_at_forty_percent() {
        let (s0, r0) = pop(0., 1.22, EASE_OUT_QUINT);
        assert!((s0 - 1.).abs() < 1e-4 && r0.abs() < 1e-4);
        let (s, r) = pop(0.4, 1.22, EASE_OUT_QUINT);
        assert!((s - 1.22).abs() < 1e-3);
        assert!((r + 8f32.to_radians()).abs() < 1e-3);
        assert_eq!(pop(1., 1.22, EASE_OUT_QUINT), (1., 0.));
    }

    #[test]
    fn equalizer_stays_in_range() {
        for bar in 0..4 {
            for step in 0..200 {
                let s = eq_scale(step as f32 * 13., bar);
                assert!((0.3 - 1e-3..=1. + 1e-3).contains(&s));
            }
        }
        assert!((eq_scale(0., 0) - 0.3).abs() < 1e-3);
        assert!((eq_scale(450., 0) - 1.).abs() < 1e-3);
    }

    #[test]
    fn swap_keeps_old_label_briefly() {
        let mut swaps = Swaps::default();
        let t0 = Instant::now();
        assert_eq!(swaps.track("tip", &"Copy".into(), t0), None);
        let old = swaps.track("tip", &"Copied".into(), t0 + Duration::from_millis(10));
        assert_eq!(old, Some(SharedString::from("Copy")));
        assert_eq!(
            swaps.track("tip", &"Copied".into(), t0 + Duration::from_millis(400)),
            None
        );
    }

    #[test]
    fn fades_hold_through_their_delay() {
        let mut fades = Fades::default();
        let t0 = Instant::now();
        let spec = MotionSpec::new(140, EASE).with_delay(380);
        assert_eq!(fades.ease(("tip", "a"), false, spec, t0, false), 0.);
        assert_eq!(fades.ease(("tip", "a"), true, spec, t0, false), 0.);
        let held = fades.ease(
            ("tip", "a"),
            true,
            spec,
            t0 + Duration::from_millis(300),
            false,
        );
        assert_eq!(held, 0.);
        assert!(fades.moving);
        let done = fades.ease(
            ("tip", "a"),
            true,
            spec,
            t0 + Duration::from_millis(600),
            false,
        );
        assert_eq!(done, 1.);
    }
}
