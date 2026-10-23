//! OTP Input – the verification code field: one box per character, a focus
//! ring that glides from box to box, paste, a calm shake on errors and a
//! morph into Verified.
//!
//! Clean: white cells with hairline shadows, a thin blue outline and caret,
//! quiet gray helper text and a tab-style "Resend code" button. Classic:
//! gray boxes with 1px borders that turn white once filled, a dark ring with
//! a soft halo, and a blue "Resend code" link.
//!
//! The field keeps the code itself (or follows `value` when controlled):
//! typing replaces the character under the ring, Backspace removes it, the
//! arrows, Home and End move the ring, and a pasted "482 913" fills the boxes
//! one after another. Verifying runs a light band over the cells, a wrong
//! code shakes, shows the message and clears for the next try, a right one
//! folds the boxes into a single "Verified" chip. A resend countdown runs
//! below.
//!
//! ```ignore
//! OtpInput::new("otp")
//!     .separator(true)
//!     .align(OtpAlign::Center)
//!     .on_complete(|code, _, _| println!("check {code}"))
//!     .on_resend(|_, _| {})
//! ```

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use web_time::{Duration, Instant};

use gpui::{
    AnyView, App, BoxShadow, Context, CursorStyle, ElementId, FocusHandle, FontWeight, Hsla,
    IntoElement, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, Pixels, Point, SharedString,
    StyledText, Task, TextRun, Window, canvas, div, font, linear_color_stop, linear_gradient,
    point, prelude::*, px, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{
        self, AnimationExt, CubicBezier, EASE, EASE_IN_OUT, EASE_OUT, EASE_OUT_QUINT, LINEAR,
        MotionSpec,
    },
    primitives::{SANS, black_a, color_mix, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "OTP Input";
pub const DESCRIPTION: &str = "The verification code field done right – a gliding focus ring, paste and SMS autofill, a calm shake on errors and a morph into Verified";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type CodeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Where the code stands, set by the owner after checking it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OtpStatus {
    #[default]
    Idle,
    /// Gray digits under a passing light band; the field is read-only.
    Verifying,
    /// A shake, red cells and the error message; the code clears for a retry.
    Error,
    /// The boxes fold into the "Verified" chip.
    Success,
}

/// Placement of the boxes and the lines below them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OtpAlign {
    #[default]
    Start,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Separator {
    None,
    Middle,
    After(usize),
}

/// "0:24"
fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Keeps the allowed characters only – the spaces and dashes of a pasted
/// "482 913" fall away; letters (when allowed) turn into capitals.
fn clean_code(text: &str, numeric: bool, length: usize) -> String {
    text.chars()
        .filter_map(|c| {
            if numeric {
                c.is_ascii_digit().then_some(c)
            } else {
                c.is_ascii_alphanumeric().then(|| c.to_ascii_uppercase())
            }
        })
        .take(length)
        .collect()
}

/// Typing with the active box selected: the character under the ring is
/// replaced (or appended in the first empty box). `None` when nothing changes.
fn type_into(
    code: &str,
    active: usize,
    typed: &str,
    numeric: bool,
    length: usize,
) -> Option<(String, usize)> {
    let raw = if active < code.len() {
        format!("{}{}{}", &code[..active], typed, &code[active + 1..])
    } else {
        format!("{code}{typed}")
    };
    let next = clean_code(&raw, numeric, length);
    // The caret counts the kept characters in front of it
    let caret = clean_code(
        &raw[..active.min(code.len()) + typed.len()],
        numeric,
        length,
    )
    .len();
    (next != code).then_some((next, caret))
}

/// A paste: a whole code replaces everything; a part is written from the
/// active box on.
fn paste_into(
    code: &str,
    active: usize,
    pasted: &str,
    numeric: bool,
    length: usize,
) -> Option<(String, usize)> {
    let text = clean_code(pasted, numeric, length);
    if text.is_empty() {
        return None;
    }
    if text.len() >= length {
        return Some((text, length - 1));
    }
    let active = active.min(code.len());
    let mut next = code[..active].to_string() + &text;
    next.push_str(code.get(active + text.len()..).unwrap_or(""));
    next.truncate(length);
    Some((next, active + text.len()))
}

/// Backspace or Delete: the active character is selected, so both remove
/// it; in an empty box Backspace takes the one before.
fn delete_at(code: &str, active: usize, backspace: bool) -> Option<(String, usize)> {
    let mut next = code.to_string();
    if active < code.len() {
        next.remove(active);
        Some((next, active))
    } else if backspace && active > 0 {
        next.remove(active - 1);
        Some((next, active - 1))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// The `--ui-otpc-*` (Clean) and `--ui-otp-*` (Classic) tokens.
struct Palette {
    /// Clean cells and the chip; Classic filled boxes and the pill.
    card: Hsla,
    /// Box background while empty (Classic `--ui-otp-bg`).
    empty: Hsla,
    /// The box edge: Clean hairline, Classic 1px border.
    edge: Hsla,
    /// Classic border of a filled box (`--ui-otp-line-strong`).
    edge_filled: Hsla,
    /// Clean `--ui-otpc-lift`.
    lift: Vec<BoxShadow>,
    ink: Hsla,
    text: Hsla,
    muted: Hsla,
    faint: Hsla,
    /// Box background while verifying (Clean wash on the card, Classic `--ui-otp-wait`).
    wait: Hsla,
    /// Clean pressed button.
    fill: Hsla,
    /// Clean button hover.
    wash: Hsla,
    shine: Hsla,
    /// Clean: the 0.5px outline. Classic: the 1.5px inset ring.
    ring: Hsla,
    /// Clean: the 2px focus glow. Classic: the 4px halo.
    halo: Hsla,
    /// Classic: the extra 6px keyboard-focus ring.
    focus: Hsla,
    caret: Hsla,
    danger: Hsla,
    /// Clean: error cell fill (the tint over the card).
    danger_fill: Hsla,
    /// Classic: error border, ring halo.
    danger_edge: Hsla,
    danger_halo: Hsla,
    success: Hsla,
    /// Clean: the chip's success fill (the tint over the card).
    success_fill: Hsla,
    /// Classic: the blue Resend link.
    accent: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        let dark = cx.is_dark();
        match (look, dark) {
            (Look::Clean, false) => {
                let card = hex(0xffffff);
                Palette {
                    card,
                    empty: card,
                    edge: black_a(0.06),
                    edge_filled: black_a(0.06),
                    lift: vec![
                        shadow(0., 1., 1., -1., black_a(0.1)),
                        shadow(0., 1., 2., 0., black_a(0.05)),
                    ],
                    ink: black_a(0.88),
                    text: black_a(0.61),
                    muted: black_a(0.45),
                    faint: black_a(0.27),
                    wait: over(card, black_a(0.03)),
                    fill: black_a(0.06),
                    wash: black_a(0.03),
                    shine: white_a(0.82),
                    ring: hexa(0x0060e2, 0.765),
                    halo: hexa(0x0060e2, 0.35),
                    focus: hexa(0x0060e2, 0.),
                    caret: hexa(0x0060e2, 0.765),
                    danger: hex(0xe5484d),
                    danger_fill: over(card, hexa(0xe5484d, 0.08)),
                    danger_edge: black_a(0.06),
                    danger_halo: hexa(0x0060e2, 0.35),
                    success: hex(0x1a9a52),
                    success_fill: over(card, hexa(0x1a9a52, 0.08)),
                    accent: hexa(0x0060e2, 0.765),
                }
            }
            (Look::Clean, true) => {
                let card = hex(0x1c1c1e);
                Palette {
                    card,
                    empty: card,
                    edge: white_a(0.1),
                    edge_filled: white_a(0.1),
                    lift: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                    ink: white_a(0.9),
                    text: white_a(0.62),
                    muted: white_a(0.46),
                    faint: white_a(0.3),
                    wait: over(card, white_a(0.03)),
                    fill: white_a(0.08),
                    wash: white_a(0.03),
                    shine: white_a(0.1),
                    ring: hex(0x4d8dff),
                    halo: hexa(0x4d8dff, 0.5),
                    focus: hexa(0x4d8dff, 0.),
                    caret: hex(0x4d8dff),
                    danger: hex(0xff6369),
                    danger_fill: over(card, hexa(0xff6369, 0.14)),
                    danger_edge: white_a(0.1),
                    danger_halo: hexa(0x4d8dff, 0.5),
                    success: hex(0x3ec27a),
                    success_fill: over(card, hexa(0x3ec27a, 0.14)),
                    accent: hex(0x4d8dff),
                }
            }
            (Look::Classic, false) => Palette {
                card: hex(0xffffff),
                empty: hex(0xfafafa),
                edge: hex(0xe4e4e7),
                edge_filled: hex(0xcfcfd4),
                lift: Vec::new(),
                ink: hex(0x171717),
                text: hex(0x404040),
                muted: hex(0x8f8f8f),
                faint: hex(0xc4c4c8),
                wait: hex(0xf2f2f3),
                fill: hex(0xf2f2f3),
                wash: hex(0xfafafa),
                shine: white_a(0.95),
                ring: hex(0x171717),
                halo: hexa(0x171717, 0.07),
                focus: hexa(0x171717, 0.22),
                caret: hex(0x171717),
                danger: hex(0xe5484d),
                danger_fill: hex(0xfafafa),
                danger_edge: hexa(0xe5484d, 0.6),
                danger_halo: hexa(0xe5484d, 0.12),
                success: hex(0x2f9e5b),
                success_fill: hex(0xffffff),
                accent: hex(0x2f6bff),
            },
            (Look::Classic, true) => Palette {
                card: hex(0x19191b),
                empty: hex(0x1d1d20),
                edge: hex(0x2a2a2e),
                edge_filled: hex(0x3a3a40),
                lift: Vec::new(),
                ink: hex(0xededed),
                text: hex(0xc8c8cc),
                muted: hex(0x8b8b92),
                faint: hex(0x4a4a50),
                wait: hex(0x1d1d20),
                fill: hex(0x1d1d20),
                wash: hex(0x1d1d20),
                shine: white_a(0.07),
                ring: hex(0xededed),
                halo: white_a(0.08),
                focus: white_a(0.3),
                caret: hex(0xededed),
                danger: hex(0xff6369),
                danger_fill: hex(0x1d1d20),
                danger_edge: hexa(0xff6369, 0.55),
                danger_halo: hexa(0xff6369, 0.14),
                success: hex(0x4cc38a),
                success_fill: hex(0x19191b),
                accent: hex(0x6d8bff),
            },
        }
    }
}

/// `color` composited over `base`: translucent tints are flattened onto the
/// card so the hairline shadows under them stay hidden, as in the browser.
fn over(base: Hsla, color: Hsla) -> Hsla {
    color_mix(Hsla { a: 1., ..color }, base, color.a)
}

/// Sizes from the two stylesheets.
struct Metrics {
    /// `--ui-otp-box`: the largest box.
    box_max: f32,
    gap: f32,
    sep_width: f32,
    /// The container query: under this width the gaps tighten.
    narrow_below: f32,
    narrow_gap: f32,
    narrow_sep: f32,
    sep_height: f32,
    /// Clean subtracts 6px for the ring's glow.
    inset: f32,
    height_ratio: f32,
    radius_ratio: f32,
    field_pad: f32,
    caret_width: f32,
    font_size: Pixels,
    line_height: Pixels,
    msg_pad: f32,
    msg_size: f32,
    msg_line: f32,
    alert: f32,
    resend_gap: f32,
    pill_pad: (f32, f32),
    pill_size: f32,
    check: f32,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                box_max: 40.,
                gap: 8.,
                sep_width: 8.,
                narrow_below: 340.,
                narrow_gap: 6.,
                narrow_sep: 6.,
                sep_height: 1.,
                inset: 6.,
                height_ratio: 1.1,
                radius_ratio: 0.2,
                field_pad: 3.,
                caret_width: 1.,
                font_size: px(13.),
                line_height: px(20.),
                msg_pad: 8.,
                msg_size: 12.,
                msg_line: 16.,
                alert: 14.,
                resend_gap: 12.,
                pill_pad: (12., 14.),
                pill_size: 13.,
                check: 16.,
            },
            Look::Classic => Metrics {
                box_max: 48.,
                gap: 8.,
                sep_width: 10.,
                narrow_below: 360.,
                narrow_gap: 6.,
                narrow_sep: 8.,
                sep_height: 2.,
                inset: 0.,
                height_ratio: 1.18,
                radius_ratio: 0.24,
                field_pad: 0.,
                caret_width: 1.5,
                font_size: px(13.5),
                line_height: px(13.5 * 1.45),
                msg_pad: 10.,
                msg_size: 13.,
                msg_line: 13. * 1.45,
                alert: 15.,
                resend_gap: 14.,
                pill_pad: (12., 15.),
                pill_size: 14.,
                check: 17.,
            },
        }
    }
}

/// Where the boxes sit, from the width the field lays out in.
#[derive(Clone)]
struct Layout {
    n: usize,
    size: f32,
    height: f32,
    radius: f32,
    gap: f32,
    sep_at: Option<usize>,
    sep_width: f32,
    width: f32,
    font: f32,
    line: f32,
}

impl Layout {
    fn new(m: &Metrics, look: Look, n: usize, sep_at: Option<usize>, fit: Option<f32>) -> Self {
        let narrow = fit.is_some_and(|w| w <= m.narrow_below);
        let gap = if narrow { m.narrow_gap } else { m.gap };
        let sep_width = if narrow { m.narrow_sep } else { m.sep_width };
        let sep_space = if sep_at.is_some() {
            gap + sep_width
        } else {
            0.
        };
        let size = match fit {
            Some(w) => m
                .box_max
                .min((w - m.inset - (n as f32 - 1.) * gap - sep_space) / n as f32)
                .max(16.),
            None => m.box_max,
        };
        let height = size * m.height_ratio;
        let font = match look {
            Look::Clean => (size * 0.375).min(15.),
            Look::Classic => size * 0.46,
        };
        let line = match look {
            Look::Clean => 24.,
            Look::Classic => font * 1.45,
        };
        Layout {
            n,
            size,
            height,
            radius: size * m.radius_ratio,
            gap,
            sep_at,
            sep_width,
            width: n as f32 * size + (n as f32 - 1.) * gap + sep_space,
            font,
            line,
        }
    }

    fn box_x(&self, i: usize) -> f32 {
        let after_sep = self.sep_at.is_some_and(|k| i >= k);
        i as f32 * (self.size + self.gap)
            + if after_sep {
                self.gap + self.sep_width
            } else {
                0.
            }
    }

    fn sep_x(&self) -> Option<f32> {
        self.sep_at.map(|k| k as f32 * (self.size + self.gap))
    }

    /// A click picks the box under the pointer (4px of slack to its right).
    fn pick(&self, x: f32) -> usize {
        (0..self.n)
            .find(|&i| x < self.box_x(i) + self.size + 4.)
            .unwrap_or(self.n - 1)
    }
}

// ---------------------------------------------------------------------------
// Transitions
// ---------------------------------------------------------------------------

/// One CSS transition on one value: retargeting starts from where the value
/// is now, after the spec's delay.
#[derive(Clone, Copy)]
struct Track {
    from: f32,
    to: f32,
    start: Instant,
    delay: f32,
    duration: f32,
    curve: CubicBezier,
}

impl Track {
    fn at(value: f32) -> Self {
        Track {
            from: value,
            to: value,
            start: Instant::now(),
            delay: 0.,
            duration: 0.,
            curve: LINEAR,
        }
    }

    fn elapsed(&self, now: Instant) -> f32 {
        now.saturating_duration_since(self.start).as_secs_f32() * 1000.
    }

    fn value(&self, now: Instant) -> f32 {
        let t = self.elapsed(now) - self.delay;
        if self.from == self.to || t >= self.duration {
            return self.to;
        }
        if t <= 0. {
            return self.from;
        }
        motion::lerp(self.from, self.to, self.curve.eval(t / self.duration))
    }

    fn moving(&self, now: Instant) -> bool {
        self.from != self.to && self.elapsed(now) < self.delay + self.duration
    }

    fn go(&mut self, to: f32, spec: MotionSpec, now: Instant, snap: bool) {
        if to == self.to {
            return;
        }
        let scale = motion::speed_scale();
        *self = Track {
            from: if snap { to } else { self.value(now) },
            to,
            start: now,
            delay: spec.delay_ms as f32 * scale,
            duration: spec.duration_ms as f32 * scale,
            curve: spec.curve,
        };
    }
}

const fn spec(ms: u64, curve: CubicBezier) -> MotionSpec {
    MotionSpec::new(ms, curve)
}

#[derive(Clone, Copy)]
struct Tracks {
    ring_x: Track,
    ring: Track,
    focus: Track,
    error: Track,
    verify: Track,
    slide: Track,
    fade: Track,
    sep: Track,
    pill: Track,
    pill_fill: Track,
    pill_show: Track,
    label: Track,
    check: Track,
    msg: Track,
    msg_show: Track,
    msg_rise: Track,
    resend: Track,
}

impl Tracks {
    fn new() -> Self {
        let zero = Track::at(0.);
        Tracks {
            ring_x: zero,
            ring: zero,
            focus: zero,
            error: zero,
            verify: zero,
            slide: zero,
            fade: zero,
            sep: zero,
            pill: zero,
            pill_fill: zero,
            pill_show: zero,
            label: zero,
            check: zero,
            msg: zero,
            msg_show: zero,
            msg_rise: zero,
            resend: Track::at(1.),
        }
    }

    fn all(&self) -> [&Track; 17] {
        [
            &self.ring_x,
            &self.ring,
            &self.focus,
            &self.error,
            &self.verify,
            &self.slide,
            &self.fade,
            &self.sep,
            &self.pill,
            &self.pill_fill,
            &self.pill_show,
            &self.label,
            &self.check,
            &self.msg,
            &self.msg_show,
            &self.msg_rise,
            &self.resend,
        ]
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// What a change asks the owner to hear about.
#[derive(Default)]
struct Effects {
    change: Option<SharedString>,
    complete: Option<SharedString>,
}

impl Effects {
    fn send(
        self,
        on_change: &Option<CodeHandler>,
        on_complete: &Option<CodeHandler>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let (Some(code), Some(f)) = (self.change, on_change) {
            f(&code, window, cx);
        }
        if let (Some(code), Some(f)) = (self.complete, on_complete) {
            f(&code, window, cx);
        }
    }
}

struct OtpState {
    focus: FocusHandle,
    /// The code as of the last render or edit.
    code: String,
    /// The code when uncontrolled.
    inner: String,
    /// The box the next character goes to.
    idx: usize,
    error_shown: bool,
    /// The wrong code fades out.
    clearing: bool,
    /// No typing between the shake and the clearing.
    locked: bool,
    /// Seconds until Resend.
    left: u32,
    resend_in: u32,
    prev_code: String,
    /// Per box: when its character arrived (keys the pop) and its stagger.
    stamps: Vec<u64>,
    delays: Vec<u64>,
    counter: u64,
    prev_status: OtpStatus,
    status_at: Instant,
    shake_at: Option<Instant>,
    /// The boxes' top-left on screen, for picking a box under the pointer.
    origin: Rc<Cell<Point<Pixels>>>,
    fresh: bool,
    tracks: Tracks,
    filled: Vec<Track>,
    // The latest props, for timers and listeners.
    layout: Option<Layout>,
    numeric: bool,
    controlled: bool,
    on_change: Option<CodeHandler>,
    clear_task: Option<Task<()>>,
    countdown: Option<Task<()>>,
}

impl OtpState {
    fn new(props: &OtpInput, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        if props.auto_focus && !props.disabled {
            window.focus(&focus, cx);
        }
        let code = clean_code(
            props.value.as_ref().unwrap_or(&props.default_value),
            props.numeric,
            props.length.max(1),
        );
        let mut state = OtpState {
            focus,
            idx: code.len().min(props.length.max(1) - 1),
            inner: code.clone(),
            prev_code: code.clone(),
            code,
            error_shown: false,
            clearing: false,
            locked: false,
            left: props.resend_in,
            resend_in: props.resend_in,
            stamps: Vec::new(),
            delays: Vec::new(),
            counter: 0,
            prev_status: props.status,
            status_at: Instant::now(),
            shake_at: None,
            origin: Rc::new(Cell::new(point(px(0.), px(0.)))),
            fresh: true,
            tracks: Tracks::new(),
            filled: Vec::new(),
            layout: None,
            numeric: props.numeric,
            controlled: props.value.is_some(),
            on_change: None,
            clear_task: None,
            countdown: None,
        };
        state.start_countdown(window, cx);
        state
    }

    fn length(&self) -> usize {
        self.layout.as_ref().map_or(1, |l| l.n)
    }

    fn active(&self) -> usize {
        self.idx.min(self.code.len()).min(self.length() - 1)
    }

    fn start_countdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.left == 0 {
            self.countdown = None;
            return;
        }
        self.countdown = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let more = this.update(cx, |s, cx| {
                    s.left = s.left.saturating_sub(1);
                    cx.notify();
                    s.left > 0
                });
                if !matches!(more, Ok(true)) {
                    break;
                }
            }
        }));
    }

    fn commit(&mut self, next: String, next_idx: usize, user: bool) -> Effects {
        let n = self.length();
        let changed = next != self.code;
        if !self.controlled {
            self.inner = next.clone();
        }
        let mut effects = Effects::default();
        if changed {
            effects.change = Some(next.clone().into());
        }
        if user {
            if self.error_shown && !next.is_empty() {
                self.error_shown = false;
            }
            if next.len() == n && changed {
                effects.complete = Some(next.clone().into());
            }
        }
        self.idx = next_idx.min(next.len()).min(n - 1);
        self.code = next;
        effects
    }

    /// One key press; `None` lets it bubble on.
    fn key(
        &mut self,
        keystroke: &Keystroke,
        blocked: bool,
        cx: &mut Context<Self>,
    ) -> Option<Effects> {
        let n = self.length();
        let len = self.code.len();
        let active = self.active();
        let read_only = blocked || self.locked;
        let mods = keystroke.modifiers;
        let key = keystroke.key.as_str();

        if (mods.secondary() || mods.control) && !mods.alt && key == "v" {
            if read_only {
                return Some(Effects::default());
            }
            let pasted = cx
                .read_from_clipboard()
                .and_then(|item| item.text())
                .unwrap_or_default();
            return Some(
                match paste_into(&self.code, active, &pasted, self.numeric, n) {
                    Some((next, caret)) => self.commit(next, caret, true),
                    None => Effects::default(),
                },
            );
        }

        if !mods.shift && !mods.platform && !mods.control {
            let last = len.min(n - 1);
            let target = match key {
                "left" => Some(active.saturating_sub(1)),
                "right" => Some(active + 1),
                "home" | "up" => Some(0),
                "end" | "down" => Some(last),
                _ => None,
            };
            if let Some(target) = target {
                self.idx = target.min(last);
                return Some(Effects::default());
            }
        }

        match key {
            "backspace" | "delete" => {
                if read_only {
                    return Some(Effects::default());
                }
                return Some(match delete_at(&self.code, active, key == "backspace") {
                    Some((next, caret)) => self.commit(next, caret, true),
                    None => Effects::default(),
                });
            }
            "enter" | "tab" | "escape" => return None,
            _ => {}
        }

        if mods.control || mods.platform || mods.alt || mods.function {
            return None;
        }
        let typed = keystroke.key_char.as_ref()?;
        if read_only {
            return Some(Effects::default());
        }
        Some(
            match type_into(&self.code, active, typed, self.numeric, n) {
                Some((next, caret)) => self.commit(next, caret, true),
                None => Effects::default(),
            },
        )
    }

    /// The `useEffect` on status: error shakes, shows the message and clears
    /// the code after a beat.
    fn status_changed(
        &mut self,
        status: OtpStatus,
        clear_on_error: bool,
        reduced: bool,
        now: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prev_status = status;
        self.status_at = now;
        if status != OtpStatus::Error {
            self.error_shown = false;
            return;
        }
        self.error_shown = true;
        if !reduced {
            self.shake_at = Some(now);
        }
        if !clear_on_error {
            return;
        }
        self.locked = true;
        self.clear_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(700))
                .await;
            if this
                .update(cx, |s, cx| {
                    s.clearing = true;
                    cx.notify();
                })
                .is_err()
            {
                return;
            }
            cx.background_executor()
                .timer(Duration::from_millis(280))
                .await;
            let Ok((on_change, effects)) = this.update(cx, |s, cx| {
                s.clearing = false;
                s.locked = false;
                let effects = s.commit(String::new(), 0, false);
                cx.notify();
                (s.on_change.clone(), effects)
            }) else {
                return;
            };
            cx.update(|window, cx| effects.send(&on_change, &None, window, cx))
                .ok();
        }));
    }
}

/// Everything one frame draws, read out of the state.
struct Frame {
    code: String,
    active: usize,
    focused: bool,
    read_only: bool,
    clearing: bool,
    stamps: Vec<u64>,
    delays: Vec<u64>,
    left: u32,
    filled: Vec<f32>,
    ring_x: f32,
    ring: f32,
    focus: f32,
    error: f32,
    verify: f32,
    slide: f32,
    fade: f32,
    sep: f32,
    pill: f32,
    pill_fill: f32,
    pill_show: f32,
    label: f32,
    check: f32,
    msg: f32,
    msg_show: f32,
    msg_rise: f32,
    resend: f32,
    /// ms since the status changed (the shimmer's clock).
    status_ms: f32,
    shake: f32,
    animating: bool,
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct OtpInput {
    id: ElementId,
    length: usize,
    separator: Separator,
    numeric: bool,
    value: Option<SharedString>,
    default_value: SharedString,
    on_change: Option<CodeHandler>,
    on_complete: Option<CodeHandler>,
    status: OtpStatus,
    error_message: SharedString,
    clear_on_error: bool,
    success_label: SharedString,
    on_resend: Option<Handler>,
    resend_in: u32,
    align: OtpAlign,
    disabled: bool,
    auto_focus: bool,
    fit_width: Option<Pixels>,
    look: Option<Look>,
}

impl OtpInput {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            length: 6,
            separator: Separator::None,
            numeric: true,
            value: None,
            default_value: SharedString::default(),
            on_change: None,
            on_complete: None,
            status: OtpStatus::Idle,
            error_message: "That code didn't work. Try again.".into(),
            clear_on_error: true,
            success_label: "Verified".into(),
            on_resend: None,
            resend_in: 30,
            align: OtpAlign::Start,
            disabled: false,
            auto_focus: false,
            fit_width: None,
            look: None,
        }
    }

    /// Number of boxes (6).
    pub fn length(mut self, length: usize) -> Self {
        self.length = length.max(1);
        self
    }

    /// A small dash between the boxes, in the middle.
    pub fn separator(mut self, on: bool) -> Self {
        self.separator = if on {
            Separator::Middle
        } else {
            Separator::None
        };
        self
    }

    /// A small dash after `boxes` boxes.
    pub fn separator_after(mut self, boxes: usize) -> Self {
        self.separator = Separator::After(boxes);
        self
    }

    /// Digits only (default); `false` allows letters too, shown in capitals.
    pub fn numeric(mut self, numeric: bool) -> Self {
        self.numeric = numeric;
        self
    }

    /// The code – makes the field controlled; pair it with `on_change`.
    pub fn value(mut self, code: impl Into<SharedString>) -> Self {
        self.value = Some(code.into());
        self
    }

    /// Starting code when uncontrolled.
    pub fn default_value(mut self, code: impl Into<SharedString>) -> Self {
        self.default_value = code.into();
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Called once the last box is filled – typed or pasted.
    pub fn on_complete(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_complete = Some(Rc::new(handler));
        self
    }

    pub fn status(mut self, status: OtpStatus) -> Self {
        self.status = status;
        self
    }

    /// Shown below the boxes while the status is `Error`.
    pub fn error_message(mut self, message: impl Into<SharedString>) -> Self {
        self.error_message = message.into();
        self
    }

    /// Clears the boxes shortly after an error, ready for the next try (on).
    pub fn clear_on_error(mut self, clear: bool) -> Self {
        self.clear_on_error = clear;
        self
    }

    /// Text in the chip after a successful check ("Verified").
    pub fn success_label(mut self, label: impl Into<SharedString>) -> Self {
        self.success_label = label.into();
        self
    }

    /// Shows "Resend code" below the boxes once the countdown is over.
    pub fn on_resend(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_resend = Some(Rc::new(handler));
        self
    }

    /// Seconds until the code can be resent (30).
    pub fn resend_in(mut self, seconds: u32) -> Self {
        self.resend_in = seconds;
        self
    }

    pub fn align(mut self, align: OtpAlign) -> Self {
        self.align = align;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn auto_focus(mut self, auto_focus: bool) -> Self {
        self.auto_focus = auto_focus;
        self
    }

    /// The width the field lays out in. Like the CSS container query, the
    /// boxes shrink to fit it and the gaps tighten on narrow widths; unset,
    /// the boxes keep their full size.
    pub fn fit_width(mut self, width: Pixels) -> Self {
        self.fit_width = Some(width);
        self
    }

    /// Overrides the global Clean/Classic look for this field.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }

    /// Reads the state for this frame and moves every transition toward
    /// where the props now put it.
    fn sync(
        &self,
        s: &mut OtpState,
        layout: &Layout,
        look: Look,
        reduced: bool,
        window: &mut Window,
        cx: &mut Context<OtpState>,
    ) -> Frame {
        let now = Instant::now();
        let n = layout.n;
        s.layout = Some(layout.clone());
        s.numeric = self.numeric;
        s.controlled = self.value.is_some();
        s.on_change = self.on_change.clone();
        s.resend_in = self.resend_in;
        s.code = match &self.value {
            Some(value) => clean_code(value, self.numeric, n),
            None => clean_code(&s.inner, self.numeric, n),
        };

        // Every character remembers when it arrived, so it pops in once;
        // pasted codes arrive one after another.
        if s.prev_code != s.code {
            let before: Vec<char> = s.prev_code.chars().collect();
            let changed: Vec<bool> = s
                .code
                .chars()
                .enumerate()
                .map(|(i, c)| before.get(i) != Some(&c))
                .collect();
            let first = changed.iter().position(|&c| c).unwrap_or(0);
            let bulk = changed.iter().filter(|&&c| c).count() > 1;
            s.stamps.resize(n, 0);
            s.delays.resize(n, 0);
            for (i, &c) in changed.iter().enumerate() {
                if c {
                    s.counter += 1;
                    s.stamps[i] = s.counter;
                    s.delays[i] = if bulk { (i - first) as u64 * 45 } else { 0 };
                }
            }
            s.prev_code = s.code.clone();
        }

        if self.status != s.prev_status {
            s.status_changed(self.status, self.clear_on_error, reduced, now, window, cx);
        }

        let len = s.code.len();
        let active = s.active();
        let focused = s.focus.is_focused(window);
        let busy = matches!(self.status, OtpStatus::Verifying | OtpStatus::Success);
        let read_only = busy || s.locked || self.disabled;
        let show_error = self.status == OtpStatus::Error && s.error_shown;
        let show_ring = !busy && !self.disabled && (focused || (len > 0 && len < n));
        let success = self.status == OtpStatus::Success;
        let snap = reduced || s.fresh;
        s.fresh = false;

        let on = |b: bool| if b { 1. } else { 0. };
        let t = &mut s.tracks;
        t.ring_x
            .go(layout.box_x(active), spec(260, EASE_OUT_QUINT), now, snap);
        t.ring.go(on(show_ring), spec(180, EASE), now, snap);
        t.focus.go(
            on(focused && look == Look::Classic),
            spec(220, EASE),
            now,
            snap,
        );
        t.error.go(on(show_error), spec(220, EASE), now, snap);
        t.verify.go(
            on(self.status == OtpStatus::Verifying),
            spec(220, EASE),
            now,
            snap,
        );
        // The cells slide together and fade while one frame morphs into the chip
        if success {
            t.slide.go(1., spec(340, EASE_OUT_QUINT), now, snap);
            t.fade.go(1., spec(220, EASE).with_delay(120), now, snap);
            t.sep.go(1., spec(200, EASE), now, snap);
            t.pill
                .go(1., spec(460, EASE_OUT_QUINT).with_delay(220), now, snap);
            t.pill_fill
                .go(1., spec(300, EASE).with_delay(300), now, snap);
            t.pill_show
                .go(1., spec(160, EASE).with_delay(120), now, snap);
            t.label.go(1., spec(240, EASE).with_delay(460), now, snap);
            t.check
                .go(1., spec(380, EASE_OUT_QUINT).with_delay(560), now, snap);
        } else {
            t.slide.go(0., spec(380, EASE_OUT_QUINT), now, snap);
            t.fade.go(0., spec(260, EASE), now, snap);
            t.sep.go(0., spec(200, EASE), now, snap);
            t.pill.go(0., spec(300, EASE_OUT_QUINT), now, snap);
            t.pill_fill.go(0., spec(300, EASE), now, snap);
            t.pill_show
                .go(0., spec(160, EASE).with_delay(140), now, snap);
            t.label.go(0., spec(160, EASE), now, snap);
            t.check.go(0., spec(120, EASE), now, snap);
        }
        // The error message grows in below, then folds away
        t.msg
            .go(on(show_error), spec(300, EASE_OUT_QUINT), now, snap);
        if show_error {
            t.msg_show.go(1., spec(200, EASE).with_delay(60), now, snap);
            t.msg_rise
                .go(1., spec(300, EASE_OUT_QUINT).with_delay(60), now, snap);
        } else {
            t.msg_show.go(0., spec(200, EASE), now, snap);
            t.msg_rise.go(0., spec(300, EASE_OUT_QUINT), now, snap);
        }
        t.resend.go(on(!success), spec(220, EASE), now, snap);

        s.filled.resize(n, Track::at(0.));
        for (i, track) in s.filled.iter_mut().enumerate() {
            track.go(on(i < len), spec(220, EASE), now, snap);
        }

        let scale = motion::speed_scale();
        let status_ms = now.saturating_duration_since(s.status_at).as_secs_f32() * 1000. / scale;
        let shake = s.shake_at.map_or(0., |at| {
            let p = now.saturating_duration_since(at).as_secs_f32() * 1000. / (420. * scale);
            if p >= 1. {
                0.
            } else {
                shake_offset(EASE_OUT.eval(p))
            }
        });
        if shake == 0. {
            s.shake_at = None;
        }

        let t = &s.tracks;
        let animating = t.all().iter().any(|t| t.moving(now))
            || s.filled.iter().any(|t| t.moving(now))
            || s.shake_at.is_some()
            || (self.status == OtpStatus::Verifying && !reduced);
        Frame {
            code: s.code.clone(),
            active,
            focused,
            read_only,
            clearing: s.clearing,
            stamps: s.stamps.clone(),
            delays: s.delays.clone(),
            left: s.left,
            filled: s.filled.iter().map(|t| t.value(now)).collect(),
            ring_x: t.ring_x.value(now),
            ring: t.ring.value(now),
            focus: t.focus.value(now),
            error: t.error.value(now),
            verify: t.verify.value(now),
            slide: t.slide.value(now),
            fade: t.fade.value(now),
            sep: t.sep.value(now),
            pill: t.pill.value(now),
            pill_fill: t.pill_fill.value(now),
            pill_show: t.pill_show.value(now),
            label: t.label.value(now),
            check: t.check.value(now),
            msg: t.msg.value(now),
            msg_show: t.msg_show.value(now),
            msg_rise: t.msg_rise.value(now),
            resend: t.resend.value(now),
            status_ms,
            shake,
            animating,
        }
    }
}

/// The shake keyframes: none, -6px, 5px, -3px, 2px, none.
fn shake_offset(p: f32) -> f32 {
    const KEYS: [f32; 6] = [0., -6., 5., -3., 2., 0.];
    let x = p.clamp(0., 1.) * 5.;
    let i = (x.floor() as usize).min(4);
    motion::lerp(KEYS[i], KEYS[i + 1], x - i as f32)
}

/// `ui-otp-shimmer`: -120% → 120% over the first 55% of 1.3s, then rest.
fn shimmer_position(ms: f32) -> Option<f32> {
    if ms < 0. {
        return None;
    }
    let phase = (ms % 1300.) / 1300.;
    Some(if phase < 0.55 {
        motion::lerp(-1.2, 1.2, EASE_IN_OUT.eval(phase / 0.55))
    } else {
        1.2
    })
}

/// `ui-otp-blink`: 1 → 0 → 1 over 1.1s.
fn blink(t: f32) -> f32 {
    if t < 0.5 {
        1. - EASE_IN_OUT.eval(t * 2.)
    } else {
        EASE_IN_OUT.eval((t - 0.5) * 2.)
    }
}

impl RenderOnce for OtpInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx);
        let m = Metrics::get(look);
        let reduced = motion::reduced_motion(cx);
        let hair = (1. / window.scale_factor()).max(0.5);
        let n = self.length.max(1);
        let sep_at = match self.separator {
            Separator::None => None,
            Separator::Middle => Some(n / 2),
            Separator::After(k) => Some(k),
        }
        .filter(|&k| k > 0 && k < n);
        let layout = Layout::new(&m, look, n, sep_at, self.fit_width.map(f32::from));
        let key = self.id.to_string();

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| OtpState::new(&self, window, cx),
        );
        let f = state.update(cx, |s, cx| self.sync(s, &layout, look, reduced, window, cx));
        if f.animating {
            window.request_animation_frame();
        }
        let (focus_handle, origin) = {
            let s = state.read(cx);
            (s.focus.clone(), s.origin.clone())
        };
        let center = self.align == OtpAlign::Center;
        let verifying = self.status == OtpStatus::Verifying;
        let blocked =
            matches!(self.status, OtpStatus::Verifying | OtpStatus::Success) || self.disabled;

        // ----- Boxes -----
        let lift: Vec<BoxShadow> = p
            .lift
            .iter()
            .map(|s| BoxShadow {
                color: s.color.opacity(1. - f.verify),
                ..s.clone()
            })
            .collect();
        let box_shadow = |edge: Hsla| {
            let mut shadows = vec![shadow(0., 0., 0., hair, edge)];
            shadows.extend(lift.iter().cloned());
            shadows
        };
        let scale = motion::lerp(1., 0.7, f.slide);
        let (w, h) = (layout.size * scale, layout.height * scale);
        let mut boxes = Vec::with_capacity(n);
        for i in 0..n {
            let filled = f.filled.get(i).copied().unwrap_or(0.);
            let slide_x = ((n as f32 - 1.) / 2. - i as f32) * (layout.size + layout.gap) * f.slide;
            let left = layout.box_x(i) + (layout.size - w) / 2. + slide_x;
            let top = (layout.height - h) / 2.;
            let (bg, fg) = if clean {
                let bg = motion::mix(p.card, p.wait, f.verify);
                let fg = motion::mix(p.ink, p.text, f.verify);
                (
                    motion::mix(bg, p.danger_fill, f.error),
                    motion::mix(fg, p.danger, f.error),
                )
            } else {
                let bg = motion::mix(motion::mix(p.empty, p.card, filled), p.wait, f.verify);
                (bg, motion::mix(p.ink, p.text, f.verify))
            };
            let mut cell = div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(w))
                .h(px(h))
                .rounded(px(layout.radius * scale))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .bg(bg)
                .text_color(fg)
                .text_size(px(layout.font * scale))
                .line_height(px(layout.line))
                .font_weight(FontWeight::MEDIUM)
                .when(f.fade > 0., |d| d.opacity(1. - f.fade));
            cell = if clean {
                cell.shadow(box_shadow(p.edge))
            } else {
                let edge =
                    motion::mix(motion::mix(p.edge, p.edge_filled, filled), p.edge, f.verify);
                cell.border_1()
                    .border_color(motion::mix(edge, p.danger_edge, f.error))
            };

            if let Some(ch) = f.code.chars().nth(i) {
                let stamp = f.stamps.get(i).copied().unwrap_or(0);
                let size = layout.font * scale;
                let glyph = div().relative().child(ch.to_string());
                cell = if reduced {
                    cell.child(glyph.when(f.clearing, |d| d.opacity(0.)))
                } else if f.clearing {
                    // The wrong code fades out, right to left
                    let spec = MotionSpec::new(240, EASE).with_delay((n - i) as u64 * 22);
                    cell.child(glyph.with_animation(
                        ElementId::Name(format!("out-{i}-{stamp}").into()),
                        spec.animation(),
                        move |el, t| {
                            el.opacity(1. - t)
                                .top(px(-4. * t))
                                .text_size(px(size * motion::lerp(1., 0.8, t)))
                        },
                    ))
                } else {
                    // Digits pop in – a small lift, growing from 0.7
                    let delay = f.delays.get(i).copied().unwrap_or(0);
                    let spec = MotionSpec::new(280, EASE_OUT_QUINT).with_delay(delay);
                    cell.child(glyph.with_animation(
                        ElementId::Name(format!("pop-{i}-{stamp}").into()),
                        spec.animation(),
                        move |el, t| {
                            el.opacity(t)
                                .top(px(4. * (1. - t)))
                                .text_size(px(size * motion::lerp(0.7, 1., t)))
                        },
                    ))
                };
            }

            // Verifying: a lighter band passes over the cells, one after another
            if verifying
                && !reduced
                && let Some(pos) = shimmer_position(f.status_ms - i as f32 * 70.)
            {
                // `linear-gradient(transparent 20%, shine 50%, transparent 80%)`
                // as two two-stop halves
                let clear = p.shine.opacity(0.);
                cell = cell.child(
                    div()
                        .absolute()
                        .top_0()
                        .left(px(pos * w))
                        .w(px(w))
                        .h_full()
                        .flex()
                        .child(div().w(px(w * 0.2)))
                        .child(div().w(px(w * 0.3)).h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(clear, 0.),
                            linear_color_stop(p.shine, 1.),
                        )))
                        .child(div().w(px(w * 0.3)).h_full().bg(linear_gradient(
                            90.,
                            linear_color_stop(p.shine, 0.),
                            linear_color_stop(clear, 1.),
                        ))),
                );
            }
            boxes.push(cell);
        }

        let sep = layout.sep_x().map(|x| {
            div()
                .absolute()
                .left(px(x))
                .top(px((layout.height - m.sep_height) / 2.))
                .w(px(layout.sep_width))
                .h(px(m.sep_height))
                .rounded(px(1.))
                .bg(p.faint)
                .opacity(1. - f.sep)
        });

        // ----- The gliding ring and its caret -----
        let ring = (f.ring > 0.).then(|| {
            let x = f.ring_x;
            let outline = |spread: f32, width: f32, color: Hsla| {
                div()
                    .absolute()
                    .left(px(x - spread))
                    .top(px(-spread))
                    .w(px(layout.size + spread * 2.))
                    .h(px(layout.height + spread * 2.))
                    .rounded(px(layout.radius + spread))
                    .border(px(width))
                    .border_color(color)
            };
            let caret_on = f.focused && f.code.chars().nth(f.active).is_none() && !f.read_only;
            let caret_h = if clean {
                (layout.height * 0.42).min(18.)
            } else {
                layout.height * 0.4
            };
            let caret = caret_on.then(|| {
                let bar = div()
                    .absolute()
                    .left(px(x + (layout.size - m.caret_width) / 2.))
                    .top(px((layout.height - caret_h) / 2.))
                    .w(px(m.caret_width))
                    .h(px(caret_h))
                    .rounded(px(1.))
                    .bg(motion::mix(
                        p.caret,
                        p.danger,
                        if clean { 0. } else { f.error },
                    ));
                if reduced {
                    bar.into_any_element()
                } else {
                    bar.with_animation(
                        "caret",
                        gpui::Animation::new(Duration::from_millis(1100)).repeat(),
                        |el, t| el.opacity(blink(t)),
                    )
                    .into_any_element()
                }
            });
            let layer = div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .opacity(f.ring);
            if clean {
                layer
                    .child(outline(2., 2., p.halo))
                    .child(outline(hair, hair, p.ring))
                    .children(caret)
            } else {
                // The keyboard-focus ring outranks the red error ring in the CSS
                let error = f.error * (1. - f.focus);
                layer
                    .when(f.focus > 0., |d| {
                        d.child(outline(6., 6., p.focus.opacity(f.focus)))
                    })
                    .child(outline(4., 4., motion::mix(p.halo, p.danger_halo, error)))
                    .child(outline(0., 1.5, motion::mix(p.ring, p.danger, error)))
                    .children(caret)
            }
        });

        // ----- The Verified chip -----
        let pill = (f.pill_show > 0.).then(|| {
            let label_w = text_width(
                window,
                &self.success_label,
                px(m.pill_size),
                FontWeight::MEDIUM,
            );
            let inner_w = m.pill_pad.0 + m.check + 6. + f32::from(label_w) + m.pill_pad.1;
            let pw = motion::lerp(layout.width, inner_w, f.pill);
            let (ph, pr) = if clean {
                (layout.height, layout.radius)
            } else {
                let ph = motion::lerp(layout.height, 36., f.pill);
                (ph, motion::lerp(layout.radius, 999., f.pill).min(ph / 2.))
            };
            let folder = if clean { "clean/" } else { "" };
            // The check draws itself left to right (the stroke-dashoffset reveal)
            let reveal = if f.check > 0. {
                0.1875 + 0.625 * f.check
            } else {
                0.
            };
            let check = div().size(px(m.check)).flex_none().child(
                div()
                    .w(px(m.check * reveal))
                    .h(px(m.check))
                    .overflow_hidden()
                    .child(
                        svg()
                            .path(format!("icons/otp-input/{folder}check.svg"))
                            .size(px(m.check))
                            .text_color(p.success),
                    ),
            );
            let chip = div()
                .absolute()
                .left(px((layout.width - pw) / 2.))
                .top(px((layout.height - ph) / 2.))
                .w(px(pw))
                .h(px(ph))
                .rounded(px(pr))
                .overflow_hidden()
                .opacity(f.pill_show)
                .bg(motion::mix(p.card, p.success_fill, f.pill_fill))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left(px((pw - inner_w) / 2.))
                        .w(px(inner_w))
                        .h_full()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .pl(px(m.pill_pad.0))
                        .text_size(px(m.pill_size))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.ink)
                        .whitespace_nowrap()
                        .opacity(f.label)
                        .child(check)
                        .child(self.success_label.clone()),
                );
            if clean {
                chip.shadow(box_shadow(p.edge))
            } else {
                chip.border_1().border_color(p.edge_filled)
            }
        });

        // ----- Input: keys, paste, picking a box -----
        let on_change = self.on_change.clone();
        let on_complete = self.on_complete.clone();
        let key_state = state.clone();
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let effects = key_state.update(cx, |s, cx| {
                let effects = s.key(&event.keystroke, blocked, cx);
                if effects.is_some() {
                    cx.notify();
                }
                effects
            });
            if let Some(effects) = effects {
                cx.stop_propagation();
                effects.send(&on_change, &on_complete, window, cx);
            }
        };
        let pick_state = state.clone();
        let pick_focus = focus_handle.clone();
        let pick_origin = origin.clone();
        let disabled = self.disabled;
        let on_pick = move |event: &MouseDownEvent, window: &mut Window, cx: &mut App| {
            if disabled {
                return;
            }
            window.focus(&pick_focus, cx);
            let x = f32::from(event.position.x - pick_origin.get().x);
            pick_state.update(cx, |s, cx| {
                if let Some(layout) = &s.layout {
                    s.idx = layout.pick(x).min(s.code.len()).min(layout.n - 1);
                }
                cx.notify();
            });
        };
        let blur_focus = focus_handle.clone();
        let tracker = canvas(
            move |bounds, _, _| origin.set(bounds.origin),
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();

        let slots = div()
            .id("slots")
            .relative()
            .flex_none()
            .w(px(layout.width))
            .h(px(layout.height))
            .left(px(f.shake))
            .when(!self.disabled, |d| d.track_focus(&focus_handle))
            .cursor(if f.read_only {
                CursorStyle::Arrow
            } else {
                CursorStyle::IBeam
            })
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, on_pick)
            .on_mouse_down_out(move |_, window, cx| {
                if blur_focus.is_focused(window) {
                    window.blur(cx);
                }
            })
            .children(boxes)
            .children(sep)
            .children(ring)
            .children(pill)
            .child(tracker);

        let field = div()
            .flex()
            .w_full()
            .min_w_0()
            .py(px(m.field_pad))
            .when(center, |d| d.justify_center())
            .child(slots);

        // ----- The error message -----
        let msg_full = m.msg_pad + m.msg_line;
        let folder = if clean { "clean/" } else { "" };
        let msg = div()
            .w_full()
            .h(px(msg_full * f.msg))
            .overflow_hidden()
            .when(f.msg > 0. || f.msg_show > 0., |d| {
                d.child(
                    div()
                        .relative()
                        .top(px(-4. * (1. - f.msg_rise)))
                        .opacity(f.msg_show)
                        .pt(px(m.msg_pad))
                        .flex()
                        .items_center()
                        .when(center, |d| d.justify_center())
                        .text_size(px(m.msg_size))
                        .line_height(px(m.msg_line))
                        .text_color(p.text)
                        .child(
                            svg()
                                .path(format!("icons/otp-input/{folder}alert.svg"))
                                .size(px(m.alert))
                                .flex_none()
                                .mr(px(6.))
                                .text_color(p.danger),
                        )
                        .child(self.error_message.clone()),
                )
            });

        // ----- Resend -----
        let resend = self.on_resend.clone().map(|on_resend| {
            let size = px(if clean { 12. } else { 13. });
            let zero = text_width(window, "0", size, FontWeight::NORMAL);
            let time = div()
                .min_w(zero * 3.4)
                .when(clean, |d| d.text_color(p.ink))
                .child(clock(f.left));
            let swap: gpui::AnyElement = if f.left > 0 {
                motion::rise_in(
                    "swap-wait",
                    spec(260, EASE_OUT_QUINT),
                    3.,
                    div()
                        .flex()
                        .text_color(p.text)
                        .child("Resend in\u{a0}")
                        .child(time),
                )
                .into_any_element()
            } else {
                let can = !blocked;
                let resend_state = state.clone();
                let button = div()
                    .id("resend")
                    .when(can, |d| {
                        d.cursor_pointer().on_click(move |_, window, cx| {
                            resend_state.update(cx, |s, cx| {
                                s.left = s.resend_in;
                                s.start_countdown(window, cx);
                                cx.notify();
                            });
                            on_resend(window, cx);
                        })
                    })
                    .child("Resend code");
                let button = if clean {
                    let edge = p.edge.opacity(0.5 / hair);
                    let button = button
                        .py(px(4. - hair))
                        .px(px(10. - hair))
                        .rounded(px(6.))
                        .border(px(hair))
                        .border_color(edge)
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM);
                    if can {
                        button
                            .hover_fill_fg(
                                format!("{key}-resend"),
                                p.wash.opacity(0.),
                                p.wash,
                                p.text,
                                p.ink,
                            )
                            .active(|s| s.bg(p.fill))
                    } else {
                        button.text_color(p.faint)
                    }
                } else {
                    let button = button
                        .px(px(8.))
                        .py(px(6.))
                        .mx(px(-8.))
                        .my(px(-6.))
                        .font_weight(FontWeight::MEDIUM);
                    if can {
                        button.text_color(p.accent).hover(|s| s.underline())
                    } else {
                        button.text_color(p.muted)
                    }
                };
                motion::rise_in("swap-go", spec(260, EASE_OUT_QUINT), 3., button).into_any_element()
            };
            let gap = if clean {
                px(8.)
            } else {
                text_width(window, "x x", size, FontWeight::NORMAL)
                    - text_width(window, "xx", size, FontWeight::NORMAL)
            };
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .when(center, |d| d.justify_center())
                .gap(gap)
                .mt(px(m.resend_gap))
                .when(clean, |d| d.min_h(px(28.)).line_height(px(16.)))
                // `line-height: 1.45` follows the 13px font here
                .when(!clean, |d| d.line_height(px(13. * 1.45)))
                .text_size(size)
                .text_color(p.muted)
                .opacity(f.resend)
                .child("Didn't get a code?")
                .child(swap)
        });

        div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .when(center, |d| d.items_center())
            .map(|d| match self.fit_width {
                Some(w) => d.w(w),
                None => d.w_full(),
            })
            .font_family(SANS)
            .text_size(m.font_size)
            .line_height(m.line_height)
            .text_color(if clean { p.ink } else { p.text })
            .when(self.disabled, |d| d.opacity(0.55))
            .child(field)
            .child(msg)
            .children(resend)
    }
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

// ---------------------------------------------------------------------------
// Demo
// ---------------------------------------------------------------------------

const DEMO_CODE: &str = "482913";
const DEMO_WRONG: &str = "481973";

fn tries_message(tries: u32) -> String {
    if tries > 0 {
        let word = if tries == 1 { "try" } else { "tries" };
        format!("That code didn't work. {tries} {word} left.")
    } else {
        "That code didn't work. Request a new code.".into()
    }
}

/// The demo: types a wrong code, then the right one, and checks
/// each; any click or key takes over from the auto-play.
struct DemoState {
    code: SharedString,
    status: OtpStatus,
    tries: u32,
    playing: bool,
    timers: Vec<Task<()>>,
}

impl DemoState {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut demo = DemoState {
            code: SharedString::default(),
            status: OtpStatus::Idle,
            tries: 3,
            playing: true,
            timers: Vec::new(),
        };
        demo.play(cx);
        demo
    }

    fn after(
        &mut self,
        ms: u64,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
    ) {
        self.timers.push(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(ms))
                .await;
            this.update(cx, |s, cx| {
                f(s, cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// The server check: a second of verifying, then the verdict.
    fn verify(&mut self, code: SharedString, cx: &mut Context<Self>) {
        self.status = OtpStatus::Verifying;
        self.after(1000, cx, move |s, cx| {
            if code.as_ref() == DEMO_CODE {
                s.status = OtpStatus::Success;
                s.after(3200, cx, |s, _| {
                    s.code = SharedString::default();
                    s.status = OtpStatus::Idle;
                    s.tries = 3;
                });
            } else {
                s.tries = s.tries.saturating_sub(1);
                s.status = OtpStatus::Error;
            }
        });
    }

    fn type_code(&mut self, code: &'static str, at: &mut u64, cx: &mut Context<Self>) {
        for end in 1..=code.len() {
            let prefix = SharedString::from(&code[..end]);
            self.after(*at, cx, move |s, _| s.code = prefix);
            *at += 150;
        }
    }

    fn play(&mut self, cx: &mut Context<Self>) {
        let mut at = 700;
        self.type_code(DEMO_WRONG, &mut at, cx);
        self.after(at + 150, cx, |s, cx| s.verify(DEMO_WRONG.into(), cx));
        at += 2850;
        self.after(at, cx, |s, _| s.status = OtpStatus::Idle);
        self.type_code(DEMO_CODE, &mut at, cx);
        self.after(at + 150, cx, |s, cx| {
            s.playing = false;
            s.verify(DEMO_CODE.into(), cx);
        });
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if !self.playing {
            return;
        }
        self.playing = false;
        self.timers.clear();
        self.code = SharedString::default();
        self.status = OtpStatus::Idle;
        cx.notify();
    }
}

/// The demo card's colors: one set per look and mode.
struct DemoColors {
    card: Hsla,
    card_shadow: Vec<BoxShadow>,
    icon_fg: Hsla,
    icon_bg: Hsla,
    title: Hsla,
    text: Hsla,
    strong: Hsla,
    hint: Hsla,
    line: Hsla,
    tray: Hsla,
}

impl DemoColors {
    fn get(look: Look, hair: f32, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => DemoColors {
                card: hex(0xffffff),
                card_shadow: vec![
                    shadow(0., 0., 0., hair, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ],
                icon_fg: black_a(0.447),
                icon_bg: hex(0xffffff),
                title: hex(0x000000),
                text: black_a(0.61),
                strong: black_a(0.88),
                hint: black_a(0.45),
                line: black_a(0.06),
                tray: hex(0xffffff),
            },
            (Look::Clean, true) => DemoColors {
                card: hex(0x1c1c1e),
                card_shadow: vec![
                    shadow(0., 0., 0., hair, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                icon_fg: white_a(0.5),
                icon_bg: hex(0x1c1c1e),
                title: hex(0xffffff),
                text: white_a(0.62),
                strong: white_a(0.9),
                hint: white_a(0.46),
                line: white_a(0.08),
                tray: hex(0x1c1c1e),
            },
            (Look::Classic, false) => DemoColors {
                card: hex(0xffffff),
                card_shadow: vec![
                    shadow(0., 0., 0., 1., hex(0xececec)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ],
                icon_fg: hex(0x8f8f8f),
                icon_bg: hex(0xf4f4f5),
                title: hex(0x171717),
                text: hex(0x8f8f8f),
                strong: hex(0x404040),
                hint: hex(0x8f8f8f),
                line: hex(0xececec),
                tray: hex(0xf0f0f0),
            },
            (Look::Classic, true) => DemoColors {
                card: hex(0x19191b),
                card_shadow: vec![
                    shadow(0., 0., 0., 1., hex(0x2a2a2e)),
                    shadow(0., 1., 2., 0., black_a(0.2)),
                ],
                icon_fg: hex(0x8b8b92),
                icon_bg: hex(0x222225),
                title: hex(0xededed),
                text: hex(0x8b8b92),
                strong: hex(0xc8c8cc),
                hint: hex(0x8b8b92),
                line: hex(0x2a2a2e),
                tray: hex(0x111113),
            },
        }
    }
}

#[derive(IntoElement)]
struct OtpDemo;

impl RenderOnce for OtpDemo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let demo = window.use_keyed_state("otp-demo", cx, |_, cx| DemoState::new(cx));
        let (code, status, tries) = {
            let s = demo.read(cx);
            (s.code.clone(), s.status, s.tries)
        };
        let look = cx.look();
        let clean = look == Look::Clean;
        let hair = (1. / window.scale_factor()).max(0.5);
        let c = DemoColors::get(look, hair, cx);

        let otp = OtpInput::new("otp")
            .separator(true)
            .align(OtpAlign::Center)
            .value(code)
            .status(status)
            .error_message(tries_message(tries))
            .resend_in(30)
            .fit_width(px(if clean { 352. } else { 344. }))
            .on_change({
                let demo = demo.clone();
                move |code, _, cx| {
                    demo.update(cx, |s, cx| {
                        s.code = code.clone();
                        if s.status == OtpStatus::Error && !code.is_empty() {
                            s.status = OtpStatus::Idle;
                        }
                        cx.notify();
                    })
                }
            })
            .on_complete({
                let demo = demo.clone();
                move |code, _, cx| {
                    demo.update(cx, |s, cx| {
                        s.verify(code.clone(), cx);
                        cx.notify();
                    })
                }
            })
            .on_resend({
                let demo = demo.clone();
                move |_, cx| {
                    demo.update(cx, |s, cx| {
                        s.tries = 3;
                        cx.notify();
                    })
                }
            });

        let sent = rich_text(&[
            ("We sent a code to ", c.text, FontWeight::NORMAL),
            ("maya@acme.com", c.strong, FontWeight::MEDIUM),
        ]);
        let demo_code = rich_text(&[
            ("Demo code: ", c.hint, FontWeight::NORMAL),
            ("482\u{a0}913", c.strong, FontWeight::MEDIUM),
        ]);

        let icon = div().flex().items_center().justify_center().bg(c.icon_bg);
        let icon = if clean {
            icon.size(px(36.))
                .rounded(px(8.))
                .mb(px(12.))
                .shadow(c.card_shadow.clone())
                .child(
                    svg()
                        .path("icons/otp-input/clean/mail.svg")
                        .size(px(16.))
                        .text_color(c.icon_fg),
                )
        } else {
            icon.size(px(44.))
                .rounded(px(12.))
                .mb(px(14.))
                .border_1()
                .border_color(c.line)
                .child(
                    svg()
                        .path("icons/otp-input/mail.svg")
                        .size(px(22.))
                        .text_color(c.icon_fg),
                )
        };
        let heading = div()
            .text_color(c.title)
            .when(clean, |d| {
                d.text_size(px(15.))
                    .line_height(px(24.))
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!clean, |d| {
                d.text_size(px(16.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::SEMIBOLD)
            })
            .child("Check your email");
        let lead = div()
            .text_color(c.text)
            .mb(px(20.))
            .when(clean, |d| {
                d.mt(px(2.)).text_size(px(13.)).line_height(px(20.))
            })
            .when(!clean, |d| {
                d.mt(px(4.)).text_size(px(13.5)).line_height(px(17.))
            })
            .child(sent);
        let body = div()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(28.))
            .px(px(24.))
            .child(icon)
            .child(heading)
            .child(lead)
            .child(otp);
        let foot = div()
            .flex()
            .justify_center()
            .text_color(c.hint)
            .child(demo_code);

        let stop_mouse = demo.clone();
        let stop_key = demo.clone();
        div()
            .id("otp-demo")
            .font_family(SANS)
            .capture_any_mouse_down(move |_, _, cx| stop_mouse.update(cx, |s, cx| s.stop(cx)))
            .capture_key_down(move |_, _, cx| stop_key.update(cx, |s, cx| s.stop(cx)))
            .child(if clean {
                div()
                    .w(px(400.))
                    .rounded(px(10.))
                    .bg(c.card)
                    .shadow(c.card_shadow)
                    .child(body.pb(px(20.)))
                    .child(
                        foot.py(px(10.))
                            .px(px(16.))
                            .border_t(px(hair))
                            .border_color(c.line)
                            .text_size(px(12.))
                            .line_height(px(16.)),
                    )
            } else {
                div()
                    .w(px(400.))
                    .p(px(4.))
                    .rounded(px(20.))
                    .bg(c.tray)
                    .child(
                        body.pb(px(22.))
                            .rounded(px(16.))
                            .bg(c.card)
                            .shadow(c.card_shadow),
                    )
                    .child(
                        foot.pt(px(10.))
                            .px(px(12.))
                            .pb(px(8.))
                            .text_size(px(12.5))
                            .line_height(px(19.)),
                    )
            })
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| OtpDemo)
}

/// A line of Inter in several colors and weights (`<strong>` inside a `<p>`).
/// GPUI's highlight colors blend into the base color and keep its alpha, so
/// the runs are spelled out instead.
fn rich_text(parts: &[(&str, Hsla, FontWeight)]) -> StyledText {
    let text: String = parts.iter().map(|(t, _, _)| *t).collect();
    let runs = parts
        .iter()
        .map(|&(t, color, weight)| {
            let mut face = font(SANS);
            face.weight = weight;
            TextRun {
                len: t.len(),
                font: face,
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            }
        })
        .collect();
    StyledText::new(text).with_runs(runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_pasted_codes() {
        assert_eq!(clean_code("482 913", true, 6), "482913");
        assert_eq!(clean_code("48-29-13-77", true, 6), "482913");
        assert_eq!(clean_code("ab-12", false, 6), "AB12");
        assert_eq!(clean_code("ab12", true, 6), "12");
    }

    #[test]
    fn typing_replaces_the_active_box() {
        assert_eq!(type_into("", 0, "4", true, 6), Some(("4".into(), 1)));
        assert_eq!(type_into("481", 1, "2", true, 6), Some(("421".into(), 2)));
        assert_eq!(type_into("481", 3, "x", true, 6), None);
        assert_eq!(
            type_into("482910", 5, "3", true, 6),
            Some(("482913".into(), 6))
        );
    }

    #[test]
    fn pasting_fills_from_the_active_box() {
        assert_eq!(
            paste_into("12", 1, "482 913", true, 6),
            Some(("482913".into(), 5))
        );
        assert_eq!(paste_into("12", 2, "34", true, 6), Some(("1234".into(), 4)));
        assert_eq!(
            paste_into("1299", 2, "34", true, 6),
            Some(("1234".into(), 4))
        );
        assert_eq!(paste_into("12", 1, "--", true, 6), None);
    }

    #[test]
    fn deleting_removes_the_selected_character() {
        assert_eq!(delete_at("481", 1, true), Some(("41".into(), 1)));
        assert_eq!(delete_at("481", 3, true), Some(("48".into(), 2)));
        assert_eq!(delete_at("481", 3, false), None);
        assert_eq!(delete_at("", 0, true), None);
    }

    #[test]
    fn boxes_make_room_for_the_separator() {
        let m = Metrics::get(Look::Clean);
        let l = Layout::new(&m, Look::Clean, 6, Some(3), Some(352.));
        assert_eq!(l.size, 40.);
        assert_eq!(l.box_x(3), 3. * 48. + 16.);
        assert_eq!(l.width, 296.);
        assert_eq!(l.pick(0.), 0);
        assert_eq!(l.pick(42.), 0);
        assert_eq!(l.pick(1000.), 5);
        // Classic in the 344px demo card: the narrow gaps
        let m = Metrics::get(Look::Classic);
        let l = Layout::new(&m, Look::Classic, 6, Some(3), Some(344.));
        assert_eq!((l.size, l.gap, l.width), (48., 6., 332.));
    }

    #[test]
    fn shake_ends_at_rest() {
        assert_eq!(shake_offset(0.), 0.);
        assert_eq!(shake_offset(0.2), -6.);
        assert_eq!(shake_offset(1.), 0.);
    }

    #[test]
    fn track_eases_after_its_delay() {
        let mut t = Track::at(0.);
        let now = Instant::now();
        t.go(1., spec(100, LINEAR).with_delay(100), now, false);
        assert_eq!(t.value(now + Duration::from_millis(50)), 0.);
        assert!(t.moving(now + Duration::from_millis(150)));
        assert_eq!(t.value(now + Duration::from_millis(300)), 1.);
    }
}
