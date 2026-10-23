//! Motion kit: pink's motion catalog, ported from the Pink app's
//! `crates/ui/src/motion.rs` (and the loader math in
//! `crates/proto/src/motion.rs`) so components animate exactly the way pink
//! does.
//!
//! - [`CubicBezier`] evaluates CSS `cubic-bezier()` exactly; pink's curves are
//!   constants ([`EASE_OUT_EXPO`], [`EASE_OUT_QUINT`], [`EASE_TAILWIND`],
//!   [`CURVE_STANDARD`], [`DISCLOSURE_CURVE`], ...).
//! - [`MotionSpec`] is one transition (duration + delay + curve) and turns
//!   into a GPUI [`Animation`] with the delay folded in. The catalog carries
//!   pink's presets under pink's names: the original catalog ([`FADE_IN`],
//!   [`MENU_IN`], [`MENU_OUT`], [`HOVER_FADE`], ...), the DS-15 ladder
//!   ([`MICRO`], [`QUICK`], [`STANDARD`], [`EMPHASIZED`], [`DISCLOSURE`]),
//!   the working-line specs ([`ORB_BREATH`], [`STATUS_SHIMMER`],
//!   [`STATUS_SWAP`]) and the specs pink's surfaces define locally
//!   ([`RESORT`], [`TOOL_ROW_REVEAL`], [`TOAST_IN`], ...). pink-ui's older
//!   names ([`GLIDE`], [`ROW_IN`], [`VIEW_IN`], [`FILL`]) are aliases of the
//!   pink preset that plays the same role.
//! - Element helpers ([`fade_in`], [`rise_in`], [`pop_in`], [`menu_in_from`],
//!   [`menu_out`], [`dialog_in`], [`status_swap`], ...) cover the entrance and
//!   exit keyframes pink's surfaces share.
//! - The pulse clock ([`pulse_delta`], [`pulse_phase`]) drives every loader
//!   loop on one shared, self-parking 30fps clock, like pink's; the loaders
//!   themselves live in [`crate::pink::loaders`].
//! - Hover fades: GPUI's `.hover()` styles snap, while pink eases every wash
//!   over [`HOVER_FADE`] (Tailwind `transition-colors`: 150ms
//!   `cubic-bezier(0.4, 0, 0.2, 1)`). [`hover_t`] and [`hover_listener`]
//!   (wrapped by [`crate::interaction::HoverFill`]) ease a per-element
//!   progress instead.
//! - [`follow`] eases a value toward a changing target (switch knobs,
//!   segmented thumbs, fold heights) without replaying on remount.
//!
//! Frames: call [`frame`] once at the top of your root view's render (the
//! gallery does). It advances the fades and keeps frames coming while any
//! fade is moving. Components also call [`keep_alive`], so fades still
//! finish in an app that skips [`frame`]; only stale-entry cleanup needs it.
//! Loader loops need neither: the pulse clock notifies the views that paint
//! them.
//!
//! Reduced motion: [`set_reduced_motion`] drives GPUI's own
//! `App::reduce_motion` flag, as pink does, so every `with_animation`
//! element snaps by itself (oneshots to their end, loops to their start), the
//! pulse clock parks, and hover fades and followers snap.
//!
//! translateY is a relative `top` inset: taffy applies it after layout, so
//! like a CSS transform it never moves siblings. GPUI has no scale transform
//! for divs, so pink's scale keyframes (`menu-in`, `dialog-in`) are fade +
//! translate, exactly as pink approximates them.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use web_time::{Duration, Instant};

use gpui::{
    Animation, AnimationElement, App, ElementId, EntityId, Global, Hsla, IntoElement, Rgba,
    SharedString, Styled, Window, px,
};

pub use gpui::AnimationExt;

// ---------------------------------------------------------------------------
// Cubic bezier
// ---------------------------------------------------------------------------

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function (endpoints fixed at
/// (0,0) and (1,1)). Solves x(t) = input by 8 Newton steps with a 32-step
/// bisection fallback, the UnitBezier approach pink uses (`CubicBezier`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezier {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    fn coefficients(a: f32, b: f32) -> (f32, f32, f32) {
        let c = 3.0 * a;
        let bb = 3.0 * (b - a) - c;
        let aa = 1.0 - c - bb;
        (aa, bb, c)
    }

    fn sample_x(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        ((a * t + b) * t + c) * t
    }

    fn sample_y(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.y1, self.y2);
        ((a * t + b) * t + c) * t
    }

    fn sample_x_derivative(&self, t: f32) -> f32 {
        let (a, b, c) = Self::coefficients(self.x1, self.x2);
        (3.0 * a * t + 2.0 * b) * t + c
    }

    fn solve_t_for_x(&self, x: f32) -> f32 {
        let mut t = x;
        for _ in 0..8 {
            let err = self.sample_x(t) - x;
            if err.abs() < 1e-6 {
                return t;
            }
            let d = self.sample_x_derivative(t);
            if d.abs() < 1e-6 {
                break;
            }
            t -= err / d;
        }
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..32 {
            let mid = (lo + hi) / 2.0;
            if self.sample_x(mid) < x {
                lo = mid
            } else {
                hi = mid
            }
        }
        (lo + hi) / 2.0
    }

    /// Eased output for progress `x ∈ [0, 1]` (clamped – f32 rounding can
    /// push the curve a hair past 1, and GPUI's animation element asserts its
    /// delta stays in range).
    pub fn eval(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        self.sample_y(self.solve_t_for_x(x)).clamp(0.0, 1.0)
    }

    /// This curve as a GPUI easing closure.
    pub fn easing(self) -> impl Fn(f32) -> f32 + 'static {
        move |x| self.eval(x)
    }
}

/// Pink's signature entrance curve, `cubic-bezier(0.16, 1, 0.3, 1)`
/// (pink `EASE_OUT_EXPO`): [`FADE_IN`], [`TOOL_ROW_REVEAL`], toasts.
pub const EASE_OUT_EXPO: CubicBezier = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
/// CSS `ease-out` (pink `EASE_OUT`): width/height, resize, collapse.
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.58, 1.0);
/// CSS `ease` (pink `EASE`): quick fades, menu exit, dialog entrance.
pub const EASE: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
/// `easeOutQuint`, `cubic-bezier(0.22, 1, 0.36, 1)` (pink `EASE_OUT_QUINT`):
/// popovers in, the working-line status swap, connector draws.
pub const EASE_OUT_QUINT: CubicBezier = CubicBezier::new(0.22, 1.0, 0.36, 1.0);
/// Sidebar resort glide and the new-thread handoff (pink `EASE_RESORT`, an
/// alias of [`EASE_OUT_QUINT`]).
pub const EASE_RESORT: CubicBezier = EASE_OUT_QUINT;
/// CSS `ease-in-out` (pink `EASE_IN_OUT`): the scroll-to-row glide.
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);
/// Tailwind's default transition curve, `cubic-bezier(0.4, 0, 0.2, 1)`
/// (pink `EASE_TAILWIND`): every hover wash rides it ([`HOVER_FADE`]).
pub const EASE_TAILWIND: CubicBezier = CubicBezier::new(0.4, 0.0, 0.2, 1.0);
/// DS-15 standard curve, `cubic-bezier(0.2, 0, 0, 1)` (pink
/// `CURVE_STANDARD`): quick start, gentle landing, for state changes in place.
pub const CURVE_STANDARD: CubicBezier = CubicBezier::new(0.2, 0.0, 0.0, 1.0);
/// DS-15 decelerate curve (pink `CURVE_DECELERATE` = [`EASE_OUT_EXPO`]):
/// things arriving on screen (menus, sheets, panes).
pub const CURVE_DECELERATE: CubicBezier = EASE_OUT_EXPO;
/// DS-15 accelerate curve, `cubic-bezier(0.4, 0, 1, 1)` (pink
/// `CURVE_ACCELERATE`): things leaving the screen ([`exit_of`]).
pub const CURVE_ACCELERATE: CubicBezier = CubicBezier::new(0.4, 0.0, 1.0, 1.0);
/// The [`DISCLOSURE`] curve, `cubic-bezier(0.32, 0.72, 0, 1)`: a critically
/// damped spring sampled as a curve – fast response, no overshoot, soft
/// landing (pink `DISCLOSURE.curve`).
pub const DISCLOSURE_CURVE: CubicBezier = CubicBezier::new(0.32, 0.72, 0.0, 1.0);
/// CSS `ease-in`. pink-ui only: pink's exits use [`CURVE_ACCELERATE`].
pub const EASE_IN: CubicBezier = CubicBezier::new(0.42, 0.0, 1.0, 1.0);
/// `cubic-bezier(0.34, 1.56, 0.64, 1)`, a small overshoot. pink-ui only:
/// pink never overshoots (its disclosure "spring" is critically damped), so
/// Pink components should not use it.
pub const EASE_OUT_BACK: CubicBezier = CubicBezier::new(0.34, 1.56, 0.64, 1.0);
/// Linear. Pink's loops are linear over their period ([`MotionSpec::repeating`]).
pub const LINEAR: CubicBezier = CubicBezier::new(0.0, 0.0, 1.0, 1.0);

// ---------------------------------------------------------------------------
// Motion specs
// ---------------------------------------------------------------------------

/// One transition: duration + optional delay + curve (pink `MotionSpec`).
/// GPUI's `Animation` has no delay, so the animation runs for
/// `delay + duration` and [`progress`](Self::progress) holds 0 until the
/// delay has passed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionSpec {
    pub duration_ms: u64,
    pub delay_ms: u64,
    pub curve: CubicBezier,
}

impl MotionSpec {
    pub const fn new(duration_ms: u64, curve: CubicBezier) -> Self {
        Self {
            duration_ms,
            delay_ms: 0,
            curve,
        }
    }

    pub const fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    pub const fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Wall-clock span of the whole timeline (delay + duration).
    pub fn total(&self) -> Duration {
        Duration::from_millis(self.delay_ms + self.duration_ms)
    }

    /// Eased progress (0..1) for a raw timeline delta (0..1 across [`total`](Self::total)).
    pub fn progress(&self, raw_delta: f32) -> f32 {
        let total = (self.delay_ms + self.duration_ms) as f32;
        if total <= 0.0 || self.duration_ms == 0 {
            return 1.0;
        }
        let t =
            (raw_delta.clamp(0.0, 1.0) * total - self.delay_ms as f32) / self.duration_ms as f32;
        self.curve.eval(t.clamp(0.0, 1.0))
    }

    /// Eased progress for a wall-clock `elapsed` since the transition began
    /// (honours [`speed_scale`]). For tweens driven off a stored `Instant`.
    pub fn progress_at(&self, elapsed: Duration) -> f32 {
        let total = self.total().mul_f32(speed_scale()).as_secs_f32();
        if total <= 0.0 {
            return 1.0;
        }
        self.progress(elapsed.as_secs_f32() / total)
    }

    /// A one-shot GPUI [`Animation`] for this spec (delay folded in, eased).
    pub fn animation(&self) -> Animation {
        let spec = *self;
        Animation::new(spec.total().mul_f32(speed_scale())).with_easing(move |d| spec.progress(d))
    }

    /// A repeating [`Animation`] with linear easing over the raw period, for
    /// loops whose easing happens in the animator. (Pink's loaders run on the
    /// [pulse clock](pulse_delta) instead.)
    pub fn repeating(&self) -> Animation {
        Animation::new(self.total().mul_f32(speed_scale())).repeat()
    }
}

// --- Pink's original catalog (pink `motion.rs` "Motion specs") -------------

/// Entrances: 500ms expo-out fade + 4px rise (pink `FADE_IN`, played by
/// [`fade_in_rise`]).
pub const FADE_IN: MotionSpec = MotionSpec::new(500, EASE_OUT_EXPO);
/// Quick opacity-only fade: 150ms `ease` (pink `FADE_QUICK`).
pub const FADE_QUICK: MotionSpec = MotionSpec::new(150, EASE);
/// Popovers and menus opening: 180ms ease-out-quint, a full fade plus a 6pt
/// travel from the trigger side (pink `MENU_IN`; see [`menu_in_from`]).
pub const MENU_IN: MotionSpec = MotionSpec::new(180, EASE_OUT_QUINT);
/// Popovers closing: 120ms `ease`, opacity only – quicker than the entrance
/// so a closing menu gets out of the way (pink `MENU_OUT`; see [`menu_out`]).
pub const MENU_OUT: MotionSpec = MotionSpec::new(120, EASE);
/// Dialogs opening: 180ms `ease`, fade + 2px rise standing in for
/// `scale(0.96)` (pink `DIALOG_IN`; see [`dialog_in`]).
pub const DIALOG_IN: MotionSpec = MotionSpec::new(180, EASE);
/// Boot splash exit: 500ms fade + 6px lift after a 150ms hold (pink
/// `SPLASH_OUT`; see [`splash_out`]).
pub const SPLASH_OUT: MotionSpec = MotionSpec::new(500, EASE).with_delay(150);
/// Sidebar / pane width and height transitions: 200ms ease-out (pink `RESIZE`).
pub const RESIZE: MotionSpec = MotionSpec::new(200, EASE_OUT);
/// Tab drag-reorder slides: 150ms ease-out (pink `TAB_SLIDE`).
pub const TAB_SLIDE: MotionSpec = MotionSpec::new(150, EASE_OUT);
/// Collapsing heights (diff files, sidebar groups, the composer morph):
/// 180ms ease-out (pink `COLLAPSE`, pink's most used height tween).
pub const COLLAPSE: MotionSpec = MotionSpec::new(180, EASE_OUT);
/// The reversible new-thread ↔ session handoff: 420ms [`EASE_RESORT`]
/// (pink `NEW_THREAD_TRANSITION`).
pub const NEW_THREAD_TRANSITION: MotionSpec = MotionSpec::new(420, EASE_RESORT);
/// Scroll-to-row glide: 500ms ease-in-out over the whole distance (pink
/// `SCROLL_GLIDE`).
pub const SCROLL_GLIDE: MotionSpec = MotionSpec::new(500, EASE_IN_OUT);
/// Every hover wash: Tailwind `transition-colors`, 150ms over
/// [`EASE_TAILWIND`] (pink `HOVER_FADE`). [`crate::interaction::HoverFill`]
/// and [`hover_t`] ride it.
pub const HOVER_FADE: MotionSpec = MotionSpec::new(150, EASE_TAILWIND);
/// Pink wave / mark loader period: 2.4s (pink `PINK_PULSE`; the curve is
/// unused, loops are linear). Also the skeleton-row pulse.
pub const PINK_PULSE: MotionSpec = MotionSpec::new(PINK_PULSE_MS, EASE);
/// Gradient matrix and mini spinner wave period: 750ms (pink `GRADIENT_SPIN`).
pub const GRADIENT_SPIN: MotionSpec = MotionSpec::new(GRADIENT_SPIN_MS, EASE);

// --- DS-15 unified ladder (pink `motion.rs` "Unified motion tokens") ------
//
// New UI picks from this short ladder instead of a bespoke duration. Exits
// run at 0.75x their entrance ([`exit_of`]). Reduced motion snaps every
// oneshot animation and parks every loop.

/// Hover, press and tooltip fades: 120ms [`CURVE_STANDARD`] (pink `MICRO`).
pub const MICRO: MotionSpec = MotionSpec::new(120, CURVE_STANDARD);
/// Switch tracks, segment thumbs, selection rings, chevrons: 160ms
/// [`CURVE_STANDARD`] (pink `QUICK`; the segmented thumb follows on it).
pub const QUICK: MotionSpec = MotionSpec::new(160, CURVE_STANDARD);
/// Menus and popovers in, tab slides, toasts: 240ms [`CURVE_DECELERATE`]
/// (pink `STANDARD`).
pub const STANDARD: MotionSpec = MotionSpec::new(240, CURVE_DECELERATE);
/// Panes, sheets, route changes: 360ms [`CURVE_DECELERATE`] (pink `EMPHASIZED`).
pub const EMPHASIZED: MotionSpec = MotionSpec::new(360, CURVE_DECELERATE);
/// Disclosures (tool folds, thinking, settings groups, the toggle-switch
/// knob): 300ms [`DISCLOSURE_CURVE`] (pink `DISCLOSURE`).
pub const DISCLOSURE: MotionSpec = MotionSpec::new(300, DISCLOSURE_CURVE);

/// The exit counterpart of an entrance spec: 0.75x as long, accelerating
/// (pink `exit_of`).
pub const fn exit_of(spec: MotionSpec) -> MotionSpec {
    MotionSpec::new(spec.duration_ms * 3 / 4, CURVE_ACCELERATE)
}

/// Working-line orb breath: one slow sine per 1600ms period (pink
/// `ORB_BREATH`; the curve is unused).
pub const ORB_BREATH: MotionSpec = MotionSpec::new(1600, EASE);
/// Working-line text shimmer period: 3400ms, the same sweep as the live
/// tool-group title (pink `STATUS_SHIMMER`; the curve is unused).
pub const STATUS_SHIMMER: MotionSpec = MotionSpec::new(3400, EASE);
/// Working-line status change: a 180ms fade and 2px settle (pink
/// `STATUS_SWAP`; see [`status_swap`]).
pub const STATUS_SWAP: MotionSpec = MotionSpec::new(180, EASE_OUT_QUINT);

// --- Specs pink's surfaces define locally ----------------------------------

/// Sidebar resort glide: 260ms [`EASE_RESORT`] per-row translate, the View
/// Transitions equivalent (pink `shell.rs` `RESORT`).
pub const RESORT: MotionSpec = MotionSpec::new(260, EASE_RESORT);
/// A newly selected chat settling in: 160ms [`CURVE_STANDARD`], opacity plus
/// a 4px settle (pink `shell.rs` "transcript-in"; rise [`TRANSCRIPT_IN_RISE`]).
pub const TRANSCRIPT_IN: MotionSpec = MotionSpec::new(160, CURVE_STANDARD);
/// [`TRANSCRIPT_IN`]'s starting offset: content settles 4px up into place.
pub const TRANSCRIPT_IN_RISE: f32 = 4.0;
/// Toast capsule entrance: 220ms expo-out, opacity plus an 8px rise (pink
/// `shell.rs` "toast-in"; rise [`TOAST_IN_RISE`]).
pub const TOAST_IN: MotionSpec = MotionSpec::new(220, EASE_OUT_EXPO);
/// [`TOAST_IN`]'s starting offset.
pub const TOAST_IN_RISE: f32 = 8.0;
/// Command palette entrance: 160ms ease-out-quint, a short fade and 6pt rise
/// (pink `command_palette.rs` "command-palette-in"; rise [`PALETTE_IN_RISE`]).
pub const PALETTE_IN: MotionSpec = MotionSpec::new(160, EASE_OUT_QUINT);
/// [`PALETTE_IN`]'s starting offset.
pub const PALETTE_IN_RISE: f32 = 6.0;
/// Lightbox image entrance: 260ms expo-out, fade plus a 10px rise (pink
/// `attachments.rs` "attachment-lightbox-image").
pub const LIGHTBOX_IN: MotionSpec = MotionSpec::new(260, EASE_OUT_EXPO);
/// Lightbox backdrop fade: 200ms ease-out, opacity only (pink
/// `attachments.rs` "attachment-lightbox-in").
pub const LIGHTBOX_BACKDROP_IN: MotionSpec = MotionSpec::new(200, EASE_OUT);
/// Composer attachment card entrance: 240ms expo-out, fade plus an 8px rise
/// (pink `composer.rs` appshot entrance).
pub const ATTACHMENT_IN: MotionSpec = MotionSpec::new(240, EASE_OUT_EXPO);
/// Composer send/stop glyph crossfade: 180ms ease-out-quint, opacity only
/// (pink `composer.rs` "composer-primary-glyph").
pub const GLYPH_SWAP: MotionSpec = MotionSpec::new(180, EASE_OUT_QUINT);
/// Workbench hover / drop-preview fade: 120ms `ease` (pink workbench
/// `PREVIEW_FADE_MS`).
pub const PREVIEW_FADE: MotionSpec = MotionSpec::new(120, EASE);
/// Workbench tab / pane insertion: 160ms ease-out (pink workbench `INSERT_MS`).
pub const INSERT: MotionSpec = MotionSpec::new(160, EASE_OUT);
/// Menus below their trigger drop 6pt into place (pink `popover.rs`
/// `MENU_RISE_DOWN`); pass to [`menu_in_from`].
pub const MENU_RISE_DOWN: f32 = -6.0;
/// Menus above their trigger rise 6pt into place (pink `MENU_RISE_UP`).
pub const MENU_RISE_UP: f32 = 6.0;

/// Tool rows and groups fold on the shared disclosure curve (pink
/// `transcript.rs` `TOOL_FOLD` = [`DISCLOSURE`]); the chevron turns with it.
pub const TOOL_FOLD: MotionSpec = DISCLOSURE;
/// An opening tool body fades in over 180ms ease-out while its height springs
/// open (pink `TOOL_BODY_FADE_MS`; see [`fold_progress`]).
pub const TOOL_BODY_FADE: MotionSpec = MotionSpec::new(180, EASE_OUT);
/// A closing tool body is fully transparent by this fraction of the fold
/// (pink `TOOL_BODY_FADE_OUT_FRACTION`).
pub const TOOL_BODY_FADE_OUT_FRACTION: f32 = 0.6;
/// The fading body also settles this far: `top = 6 * (1 - alpha)` (pink
/// `transcript.rs` tool panel).
pub const TOOL_BODY_RISE: f32 = 6.0;
/// A new tool row grows from 0 to its height over 360ms expo-out (pink
/// `TOOL_ROW_REVEAL`; see [`reveal_progress`]).
pub const TOOL_ROW_REVEAL: MotionSpec = MotionSpec::new(360, EASE_OUT_EXPO);
/// The activity-rail connector draws briskly and eases into the branch tip:
/// 480ms ease-out-quint (pink `TOOL_CONNECTOR_REVEAL`; see
/// [`tool_connector_parts`]).
pub const TOOL_CONNECTOR_REVEAL: MotionSpec = MotionSpec::new(480, EASE_OUT_QUINT);
/// The first row of a new tool group arrives after this beat (pink
/// `TOOL_FIRST_ROW_DELAY_MS`).
pub const TOOL_FIRST_ROW_DELAY_MS: u64 = 90;
/// Each further arriving row starts this much later (pink `TOOL_ROW_STAGGER_MS`).
pub const TOOL_ROW_STAGGER_MS: u64 = 65;
/// A fold animates while its toggle is younger than this; past it the fold
/// renders statically so a remount never replays it (pink `FOLD_TWEEN_WINDOW`).
pub const FOLD_TWEEN_WINDOW: Duration = Duration::from_millis(400);

// --- pink-ui names, now aliases of the pink preset playing the role --------

/// Content swapping in place (a new label, panel or message replacing the
/// old one, with a small rise): pink's [`TRANSCRIPT_IN`], 160ms
/// [`CURVE_STANDARD`] with a 4px settle.
pub const VIEW_IN: MotionSpec = TRANSCRIPT_IN;
/// Rows and items entering a list: pink's [`TOOL_ROW_REVEAL`], 360ms
/// expo-out (pink staggers arrivals by [`TOOL_FIRST_ROW_DELAY_MS`] +
/// [`TOOL_ROW_STAGGER_MS`]).
pub const ROW_IN: MotionSpec = TOOL_ROW_REVEAL;
/// Heights, widths and positions gliding: pink's [`DISCLOSURE`] (300ms
/// critically damped spring), the curve of pink's folds and switch knob.
/// Pink picks a narrower preset where one fits: [`QUICK`] for thumbs and
/// chevrons, [`RESORT`] for list reorders, [`RESIZE`] / [`COLLAPSE`] for
/// pane and height tweens.
pub const GLIDE: MotionSpec = DISCLOSURE;
/// Progress fills and meters easing to a value. Pink has no fill preset, so
/// this is the DS-15 rung for something arriving on screen at length:
/// [`EMPHASIZED`] (360ms expo-out, no delay).
pub const FILL: MotionSpec = EMPHASIZED;

// ---------------------------------------------------------------------------
// Element helpers
// ---------------------------------------------------------------------------

/// Opacity 0 → 1 over [`FADE_QUICK`]: pink's `fade_quick`. (Pink's own
/// `fade_in`, which also rises, is [`fade_in_rise`]; this name keeps its
/// opacity-only meaning, so it is safe on absolutely positioned elements.)
pub fn fade_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    fade_quick(id, element)
}

/// Quick opacity-only fade over [`FADE_QUICK`] (pink `fade_quick`).
pub fn fade_quick<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, FADE_QUICK.animation(), |el, t| el.opacity(t))
}

/// Pink's standard entrance (pink `fade_in`): opacity 0 → 1 plus
/// translateY 4 → 0 over [`FADE_IN`] (500ms expo-out). Sets `.relative()`.
pub fn fade_in_rise<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, FADE_IN.animation(), |el, t| {
        el.relative().opacity(t).top(px(4.0 * (1.0 - t)))
    })
}

/// A composition arriving as one object (pink `settle_down`): opacity 0 → 1
/// while settling 10px down into place over [`FADE_IN`].
pub fn settle_down<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, FADE_IN.animation(), |el, t| {
        el.relative().opacity(t).top(px(-10.0 * (1.0 - t)))
    })
}

/// Opacity 0 → 1 while rising `rise` px into place over `spec`. Pass a
/// negative `rise` to drop in. Sets `.relative()`, so for absolutely
/// positioned elements use `with_animation` directly.
pub fn rise_in<E>(
    id: impl Into<ElementId>,
    spec: MotionSpec,
    rise: f32,
    element: E,
) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, spec.animation(), move |el, t| {
        el.relative().opacity(t).top(px(rise * (1.0 - t)))
    })
}

/// Popover entrance for a menu below its trigger: pink's `menu_in`, a fade
/// plus a 6pt drop ([`MENU_RISE_DOWN`]) over [`MENU_IN`], standing in for
/// pink's `scale(0.96)`.
pub fn pop_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    menu_in(id, element)
}

/// Popover entrance (pink `menu_in`): fade + translateY −6 → 0 over [`MENU_IN`].
pub fn menu_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    menu_in_from(id, MENU_RISE_DOWN, element)
}

/// Popover entrance from the trigger side (pink `menu_in_from`): `rise` is
/// the starting offset in pt – [`MENU_RISE_DOWN`] (−6) for a menu below its
/// trigger, [`MENU_RISE_UP`] (+6) for one above it.
pub fn menu_in_from<E>(id: impl Into<ElementId>, rise: f32, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, MENU_IN.animation(), move |el, t| {
        el.relative().opacity(t).top(px(rise * (1.0 - t)))
    })
}

/// Popover exit (pink `menu_out`): opacity 1 → 0 over [`MENU_OUT`]. The eased
/// progress `t` comes from the caller ([`menu_exit_progress`] of the closing
/// instant): `with_animation`'s clock replays from 0 on remount, and a replay
/// mid-exit would flash the menu back to full opacity. The animation wrapper
/// only pumps frames for the exit's span. Give it a fresh id (pink uses
/// `"{id}-out"`), and lay an occluding overlay over the dying menu so its
/// rows take no clicks.
pub fn menu_out<E>(id: impl Into<ElementId>, t: f32, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, MENU_OUT.animation(), move |el, _| el.opacity(1.0 - t))
}

/// Eased [`MENU_OUT`] progress (0..1) for a popover that began closing at
/// `since`, from the wall clock (pink `popover.rs` `exit_progress`).
/// Monotonic by construction, unlike an animation element's own clock.
pub fn menu_exit_progress(since: Instant) -> f32 {
    MENU_OUT.progress_at(since.elapsed())
}

/// Dialog entrance (pink `dialog_in`): fade + a 2px rise over [`DIALOG_IN`]
/// (pink's `scale(0.96 → 1)` approximated).
pub fn dialog_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, DIALOG_IN.animation(), |el, t| {
        el.relative().opacity(t).top(px(2.0 * (1.0 - t)))
    })
}

/// Boot-splash exit (pink `splash_out`): hold 150ms, then fade out and lift
/// 6px over 500ms ([`SPLASH_OUT`]).
pub fn splash_out<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, SPLASH_OUT.animation(), |el, t| {
        el.opacity(1.0 - t).top(px(-6.0 * t))
    })
}

/// A status label arriving (pink's working line): a 180ms fade and 2px
/// settle over [`STATUS_SWAP`]. Key the id on the label
/// ([`crate::pink::loaders::label_id`]) so every new label replays it; reduced
/// motion snaps it.
pub fn status_swap<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, STATUS_SWAP.animation(), |el, t| {
        el.relative().opacity(t).top(px(2.0 * (1.0 - t)))
    })
}

/// Linear interpolation.
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

// ---------------------------------------------------------------------------
// Tool-row reveal and fold timing (pink `transcript.rs`)
// ---------------------------------------------------------------------------

/// Eased progress of a reveal that started at `start` (pink
/// `tool_row_reveal_progress` / `tool_connector_reveal_progress`): 1 when
/// there is no start or motion is reduced. The curve runs over the spec's
/// whole span.
pub fn reveal_progress(
    spec: MotionSpec,
    start: Option<Instant>,
    now: Instant,
    reduced: bool,
) -> f32 {
    let Some(start) = start.filter(|_| !reduced) else {
        return 1.0;
    };
    let total = spec.total().mul_f32(speed_scale()).as_secs_f32();
    if total <= 0.0 {
        return 1.0;
    }
    let elapsed = now.saturating_duration_since(start).as_secs_f32();
    spec.curve.eval(elapsed / total)
}

/// When the `arrival_ix`-th new row of a tool group starts revealing,
/// relative to the sync that found it: [`TOOL_FIRST_ROW_DELAY_MS`] for a new
/// group, then [`TOOL_ROW_STAGGER_MS`] per row (pink `transcript.rs` reveal
/// starts).
pub fn tool_row_arrival_delay(arrival_ix: usize, new_group: bool) -> Duration {
    let first = if new_group {
        TOOL_FIRST_ROW_DELAY_MS
    } else {
        0
    };
    Duration::from_millis(first + arrival_ix as u64 * TOOL_ROW_STAGGER_MS)
}

/// Splits one arrival into a continuous tree draw (pink
/// `tool_connector_parts`): `(incoming, branch)` progress of the trunk and
/// the elbow. A row with a predecessor waits for the previous row's
/// continuation, then draws its incoming trunk over 0.45..0.72; the branch
/// overlaps the end of it (from 0.68, or 0.58 without a predecessor) so
/// there is no dead frame at the bend.
pub fn tool_connector_parts(progress: f32, has_predecessor: bool) -> (f32, f32) {
    let progress = progress.clamp(0.0, 1.0);
    let (incoming_start, incoming_end, branch_start) = if has_predecessor {
        (0.45, 0.72, 0.68)
    } else {
        (0.0, 0.62, 0.58)
    };
    let incoming = ((progress - incoming_start) / (incoming_end - incoming_start)).clamp(0.0, 1.0);
    let branch = ((progress - branch_start) / (1.0 - branch_start)).clamp(0.0, 1.0);
    (incoming, branch)
}

/// The outgoing trunk belongs to the row already present but is timed by the
/// next row's arrival (pink `tool_connector_continuation`): it draws over the
/// first 45% of that arrival.
pub fn tool_connector_continuation(next_progress: Option<f32>) -> f32 {
    next_progress
        .map(|progress| (progress / 0.45).clamp(0.0, 1.0))
        .unwrap_or(0.0)
}

/// One clock for a fold's chevron, height spring and body fade, so the three
/// agree mid-fold (pink `transcript.rs` tool fold). `elapsed` is the time
/// since the toggle while the fold is animating (`None` once settled, past
/// [`FOLD_TWEEN_WINDOW`], or under reduced motion). Returns
/// `(progress, body_alpha)`: `progress` 0 = closed, 1 = open on
/// [`TOOL_FOLD`]; an opening body fades in over [`TOOL_BODY_FADE`], a
/// closing one is clear by [`TOOL_BODY_FADE_OUT_FRACTION`] of the fold. Rise
/// the body by `TOOL_BODY_RISE * (1 - body_alpha)`.
pub fn fold_progress(open: bool, elapsed: Option<Duration>) -> (f32, f32) {
    let Some(elapsed) = elapsed else {
        return (if open { 1.0 } else { 0.0 }, 1.0);
    };
    let scale = speed_scale();
    let elapsed_ms = elapsed.as_secs_f32() * 1000.0 / scale;
    let raw = elapsed_ms / TOOL_FOLD.duration_ms as f32;
    let t = TOOL_FOLD.curve.eval(raw.clamp(0.0, 1.0));
    if open {
        let fade = (elapsed_ms / TOOL_BODY_FADE.duration_ms as f32).clamp(0.0, 1.0);
        (t, TOOL_BODY_FADE.curve.eval(fade))
    } else {
        let fade = (raw / TOOL_BODY_FADE_OUT_FRACTION).clamp(0.0, 1.0);
        (1.0 - t, 1.0 - fade)
    }
}

/// Layout transitions take longer when they travel farther (pink
/// `user_resize_duration_ms`): 220ms plus 0.32ms per px, capped at 850ms.
pub fn user_resize_duration_ms(height_delta: f32) -> u64 {
    (220.0 + height_delta.max(0.0) * 0.32).min(850.0).round() as u64
}

/// The spec for a distance-scaled height change (pink `user_resize_spec`):
/// ease-out for ordinary folds, ease-in-out past 500px so a large fold does
/// not vanish in its first frames.
pub fn user_resize_spec(height_delta: f32) -> MotionSpec {
    let curve = if height_delta > 500.0 {
        EASE_IN_OUT
    } else {
        EASE_OUT
    };
    MotionSpec::new(user_resize_duration_ms(height_delta), curve)
}

// ---------------------------------------------------------------------------
// Resize-edge feedback (pink `motion.rs`)
// ---------------------------------------------------------------------------

/// A pane resize limit acknowledges a held pointer with this small
/// displacement (pink `RESIZE_EDGE_NUDGE`).
pub const RESIZE_EDGE_NUDGE: f32 = 5.0;
/// The bounce's whole span (pink `RESIZE_EDGE_BOUNCE_MS`).
pub const RESIZE_EDGE_BOUNCE_MS: u64 = 220;
/// Share of the span spent easing out to the nudge (pink
/// `RESIZE_EDGE_BOUNCE_OUT_FRACTION`); the rest eases home.
pub const RESIZE_EDGE_BOUNCE_OUT_FRACTION: f32 = 0.32;

/// Which limit a resize hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeEdge {
    Min,
    Max,
}

/// One clamped resize sample (pink `ResizeDragSample`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResizeDragSample {
    pub width: f32,
    pub edge: Option<ResizeEdge>,
    pub starts_bounce: bool,
}

/// Clamps a resize sample while latching its constrained edge, so a held
/// pointer starts one bounce rather than one per drag event (pink
/// `resize_drag_sample`).
pub fn resize_drag_sample(
    requested: f32,
    min: f32,
    max: f32,
    latched_edge: Option<ResizeEdge>,
    reduced_motion: bool,
) -> ResizeDragSample {
    debug_assert!(min <= max);
    let edge = if requested <= min {
        Some(ResizeEdge::Min)
    } else if requested >= max {
        Some(ResizeEdge::Max)
    } else {
        None
    };
    ResizeDragSample {
        width: requested.clamp(min, max),
        starts_bounce: !reduced_motion && edge.is_some() && edge != latched_edge,
        edge,
    }
}

/// A running edge bounce (pink `ResizeEdgeBounce`).
#[derive(Debug, Clone, Copy)]
pub struct ResizeEdgeBounce {
    pub edge: ResizeEdge,
    pub started: Instant,
}

impl ResizeEdgeBounce {
    pub fn new(edge: ResizeEdge) -> Self {
        Self {
            edge,
            started: Instant::now(),
        }
    }
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Rounded two-phase pulse for a bounce at raw progress `raw` (0..1 of
/// [`RESIZE_EDGE_BOUNCE_MS`]): smoothstep out to the nudge, then a little
/// longer smoothstep home; both joins have zero velocity (pink
/// `resize_bounce_offset`). Negative at the min edge.
pub fn resize_bounce_offset(edge: ResizeEdge, raw: f32) -> f32 {
    let raw = raw.clamp(0.0, 1.0);
    let magnitude = if raw < RESIZE_EDGE_BOUNCE_OUT_FRACTION {
        smoothstep(raw / RESIZE_EDGE_BOUNCE_OUT_FRACTION)
    } else {
        1.0 - smoothstep(
            (raw - RESIZE_EDGE_BOUNCE_OUT_FRACTION) / (1.0 - RESIZE_EDGE_BOUNCE_OUT_FRACTION),
        )
    } * RESIZE_EDGE_NUDGE;
    match edge {
        ResizeEdge::Min => -magnitude,
        ResizeEdge::Max => magnitude,
    }
}

// ---------------------------------------------------------------------------
// Pulse clock: the shared, throttled drive for every loader loop
// ---------------------------------------------------------------------------
//
// Pink does not run its loaders as `with_animation(..repeat())` elements,
// which redraw at the display rate for as long as they are mounted. One
// shared 30fps clock drives them instead: a loader reads its phase from one
// epoch (so every instance stays phase-locked) and renews a lease on the view
// that paints it; the clock notifies leased views every tick and parks once
// the last lease lapses. A window with no loader mounted schedules nothing.

/// Repeat-tick interval for the loaders, about 30fps (pink `PULSE_TICK`).
const PULSE_TICK: Duration = Duration::from_millis(33);

/// How long a view stays ticked after its last loader paint (pink
/// `PULSE_LEASE`). An unmounted loader stops renewing and the view drops off.
const PULSE_LEASE: Duration = Duration::from_millis(300);

struct PulseClock {
    epoch: Instant,
    leases: HashMap<EntityId, PulseLease>,
    tick: u64,
    running: bool,
}

struct PulseLease {
    until: Instant,
    stride: u64,
}

impl PulseLease {
    fn renew(&mut self, now: Instant, stride: u64) {
        self.until = now + PULSE_LEASE;
        self.stride = self.stride.min(stride);
    }

    fn take_tick(&mut self, tick: u64) -> bool {
        if !tick.is_multiple_of(self.stride) {
            return false;
        }
        // Each paint re-establishes the fastest mounted animation, so a
        // retired 30Hz loop never pins a 15Hz loader at 30Hz.
        self.stride = u64::MAX;
        true
    }
}

impl Global for PulseClock {}

impl Default for PulseClock {
    fn default() -> Self {
        Self {
            epoch: Instant::now(),
            leases: HashMap::new(),
            tick: 0,
            running: false,
        }
    }
}

/// Current phase `[0, 1)` of a looping `spec`, plus a lease that keeps
/// `view` re-rendering at ~30fps while its loader stays mounted (pink
/// `pulse_delta`). Every caller shares one epoch, so instances stay
/// phase-locked. Reduced motion returns 0 and schedules nothing.
pub fn pulse_delta(spec: &MotionSpec, view: EntityId, cx: &mut App) -> f32 {
    pulse_delta_every(spec, view, 1, cx)
}

/// [`pulse_delta`] on a 15Hz stride for coarse cell loaders (pink
/// `pulse_delta_slow`; the gradient spinner). Period and phase are the same.
pub fn pulse_delta_slow(spec: &MotionSpec, view: EntityId, cx: &mut App) -> f32 {
    pulse_delta_every(spec, view, 2, cx)
}

/// Keeps `view` ticking on the pulse clock without reading a phase, for an
/// animation timed off its own start (pink `pulse_lease`; the tool-group
/// shimmer). Renew it on every paint while the animation is live.
pub fn pulse_lease(view: EntityId, cx: &mut App) {
    pulse_lease_every(view, 1, cx);
}

/// [`pulse_delta`] for the view currently rendering: what builders use, since
/// they have a `Window` rather than their view's id. Call it while rendering
/// (a `RenderOnce::render` or a `canvas` prepaint).
pub fn pulse_phase(spec: &MotionSpec, window: &Window, cx: &mut App) -> f32 {
    pulse_delta(spec, window.current_view(), cx)
}

/// [`pulse_delta_slow`] for the view currently rendering.
pub fn pulse_phase_slow(spec: &MotionSpec, window: &Window, cx: &mut App) -> f32 {
    pulse_delta_slow(spec, window.current_view(), cx)
}

/// [`pulse_lease`] for the view currently rendering.
pub fn pulse_keep(window: &Window, cx: &mut App) {
    pulse_lease(window.current_view(), cx);
}

fn pulse_delta_every(spec: &MotionSpec, view: EntityId, stride: u64, cx: &mut App) -> f32 {
    if cx.reduce_motion() {
        return 0.0;
    }
    pulse_lease_every(view, stride, cx);
    let period = spec.total().mul_f32(speed_scale()).as_secs_f32();
    if period <= 0.0 {
        return 0.0;
    }
    let clock = cx.default_global::<PulseClock>();
    (clock.epoch.elapsed().as_secs_f32() / period).fract()
}

fn pulse_lease_every(view: EntityId, stride: u64, cx: &mut App) {
    if cx.reduce_motion() {
        return;
    }
    let clock = cx.default_global::<PulseClock>();
    let now = Instant::now();
    clock
        .leases
        .entry(view)
        .or_insert(PulseLease {
            until: now + PULSE_LEASE,
            stride,
        })
        .renew(now, stride);
    if !clock.running {
        clock.running = true;
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor().timer(PULSE_TICK).await;
                let parked = cx.update(|cx| {
                    let clock = cx.default_global::<PulseClock>();
                    let now = Instant::now();
                    clock.leases.retain(|_, lease| lease.until > now);
                    if clock.leases.is_empty() {
                        clock.running = false;
                        return true;
                    }
                    clock.tick = clock.tick.wrapping_add(1);
                    let tick = clock.tick;
                    let views: Vec<EntityId> = clock
                        .leases
                        .iter_mut()
                        .filter_map(|(view, lease)| lease.take_tick(tick).then_some(*view))
                        .collect();
                    for view in views {
                        cx.notify(view);
                    }
                    false
                });
                if parked {
                    break;
                }
            }
        })
        .detach();
    }
}

// ---------------------------------------------------------------------------
// Loader math (pink `crates/proto/src/motion.rs`, rendered by
// `crate::pink::loaders`)
// ---------------------------------------------------------------------------
//
// Everything is a pure function of a phase in 0..1, so a caller can drive it
// from the pulse clock or from wall-clock time and get identical output.

/// Pink loader pulse period in ms (pink_proto `PINK_PULSE_MS`).
pub const PINK_PULSE_MS: u64 = 2_400;
/// Gradient matrix spinner wave period in ms (pink_proto `GRADIENT_SPIN_MS`).
pub const GRADIENT_SPIN_MS: u64 = 750;
/// Cells in the pink wave loader (pink_proto `PINK_CELLS`).
pub const PINK_CELLS: usize = 5;
/// Side length of the gradient spinner matrix (pink_proto `MATRIX_SIDE`).
pub const MATRIX_SIDE: usize = 3;
/// Pink loader cells rest at this opacity between pulses (pink_proto
/// `PULSE_MIN_OPACITY`).
pub const PULSE_MIN_OPACITY: f32 = 0.08;
/// …and at this scale (pink_proto `PULSE_MIN_SCALE`).
pub const PULSE_MIN_SCALE: f32 = 0.9;
/// Per-cell stagger as a fraction of the pulse period, 0.15s of 2.4s
/// (pink_proto `PULSE_STAGGER`).
pub const PULSE_STAGGER: f32 = 0.15 / 2.4;
/// Per-row tints of pink's "sunrise" gradient (pink_proto `GSPIN_ROW_TINTS`):
/// cool blue at the top, through amber, to pink. Pink's GPUI loaders tint
/// from the accent glyph palette instead; kept for parity.
pub const GSPIN_ROW_TINTS: [u32; MATRIX_SIDE] = [0xB6D3EF, 0xEDB185, 0xF888A0];
/// Opacity a gradient-spinner cell rests at between pulses (pink_proto
/// `GSPIN_DIM`).
pub const GSPIN_DIM: f32 = 0.1;

/// A cell's phase, given the loader's raw phase and the cell's index (pink
/// `staggered_phase`).
pub fn staggered_phase(raw_delta: f32, index: usize, stagger: f32) -> f32 {
    (raw_delta - index as f32 * stagger).rem_euclid(1.0)
}

/// Cosine pulse: 0 at phase 0, 1 at phase 0.5, back to 0 at phase 1 (pink
/// `pulse_wave`).
pub fn pulse_wave(phase: f32) -> f32 {
    0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()
}

/// Pink loader cell opacity for a phase: 0.08 → 1 → 0.08 (pink `pulse_opacity`).
pub fn pulse_opacity(phase: f32) -> f32 {
    PULSE_MIN_OPACITY + (1.0 - PULSE_MIN_OPACITY) * pulse_wave(phase)
}

/// Pink loader cell scale for a phase: 0.9 → 1 → 0.9 (pink `pulse_scale`).
pub fn pulse_scale(phase: f32) -> f32 {
    PULSE_MIN_SCALE + (1.0 - PULSE_MIN_SCALE) * pulse_wave(phase)
}

/// Gradient-spin cell opacity for a local phase `t` (pink `gspin_opacity`,
/// from pink's `gradient-spin-pulse` keyframes): full at the cycle start,
/// easing down to `dim` by 45%, resting at `dim` until 92%, then rising back
/// to full. Per-cell phase offsets sweep this pulse across the grid.
pub fn gspin_opacity(t: f32, dim: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    if t < 0.45 {
        lerp(1.0, dim, t / 0.45)
    } else if t < 0.92 {
        dim
    } else {
        lerp(dim, 1.0, (t - 0.92) / 0.08)
    }
}

/// The pink mark's pixels: `(x, y)` of each 100×100 cell on the 820×940
/// canvas (pink_proto `MARK_CELLS`, from pink's `logo.tsx`).
#[rustfmt::skip]
pub const MARK_CELLS: [(f32, f32); 34] = [
    (0., 600.), (0., 720.), (240., 840.), (240., 720.), (120., 840.), (120., 600.), (240., 600.),
    (0., 480.), (0., 360.), (480., 840.), (480., 720.), (120., 360.), (120., 240.), (240., 360.),
    (600., 720.), (480., 600.), (360., 360.), (240., 240.), (600., 600.), (720., 600.), (720., 480.),
    (240., 120.), (600., 380.), (720., 240.), (720., 0.), (480., 240.), (480., 0.), (120., 480.),
    (240., 480.), (360., 840.), (360., 720.), (360., 600.), (360., 480.), (120., 720.),
];

/// Fraction of the pulse cycle the mark's light sweep occupies (pink_proto
/// `MARK_SPREAD`).
pub const MARK_SPREAD: f32 = 0.55;

/// Per-cell stagger along the mark's flight axis (pink `mark_cell_stagger`).
/// It *adds* phase, so a larger value leads: the tail tip `(720, 0)` leads at
/// about [`MARK_SPREAD`], the head `(0, 840)` trails at 0.
pub fn mark_cell_stagger(x: f32, y: f32) -> f32 {
    let t = (820.0 - x + y) / 1660.0;
    (1.0 - t) * MARK_SPREAD
}

// ---------------------------------------------------------------------------
// Hover fades (CSS `transition-colors` parity)
// ---------------------------------------------------------------------------
//
// A per-element-key hover progress, advanced from wall time on every read
// (pink `HoverFades`). The store is a main-thread thread_local rather than a
// GPUI global so builders can blend colors without threading `cx`
// everywhere. Entries are stamped with the frame counter on every read;
// [`frame`] prunes entries that went a whole frame unread (their element
// unmounted mid-hover and will never get a leave event) and entries that
// settled back at rest.

#[derive(Debug, Clone, Copy)]
struct FadeEntry {
    origin: f32,
    target: f32,
    started: Instant,
    seen: u64,
}

impl FadeEntry {
    fn value(&self, now: Instant, duration: Duration) -> f32 {
        let elapsed = now.saturating_duration_since(self.started);
        if duration.is_zero() || elapsed >= duration {
            return self.target;
        }
        let raw = elapsed.as_secs_f32() / duration.as_secs_f32();
        lerp(self.origin, self.target, HOVER_FADE.curve.eval(raw))
    }

    fn settled(&self, now: Instant, duration: Duration) -> bool {
        self.origin == self.target || now.saturating_duration_since(self.started) >= duration
    }
}

#[derive(Default)]
struct HoverFades {
    entries: HashMap<String, FadeEntry>,
    frame: u64,
}

impl HoverFades {
    fn duration() -> Duration {
        HOVER_FADE.total().mul_f32(speed_scale())
    }

    /// Pointer entered (`hovered`) or left the element behind `key`; a flip
    /// mid-flight re-anchors at the current value so the blend is continuous.
    fn set_at(&mut self, key: &str, hovered: bool, reduced: bool, now: Instant) {
        let target = if hovered { 1.0 } else { 0.0 };
        let duration = Self::duration();
        let current = self
            .entries
            .get(key)
            .map(|e| e.value(now, duration))
            .unwrap_or(0.0);
        if target == 0.0 && !self.entries.contains_key(key) {
            return;
        }
        let origin = if reduced { target } else { current };
        let seen = self.frame;
        self.entries.insert(
            key.to_string(),
            FadeEntry {
                origin,
                target,
                started: now,
                seen,
            },
        );
    }

    fn value_at(&mut self, key: &str, now: Instant) -> f32 {
        self.value_at_or(key, now, 0.0)
    }

    fn value_at_or(&mut self, key: &str, now: Instant, absent: f32) -> f32 {
        let frame = self.frame;
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.seen = frame;
                entry.value(now, Self::duration())
            }
            None => absent,
        }
    }

    fn active(&self, now: Instant) -> bool {
        let duration = Self::duration();
        self.entries.values().any(|e| !e.settled(now, duration))
    }

    fn tick_at(&mut self, now: Instant) -> bool {
        self.frame += 1;
        let frame = self.frame;
        let duration = Self::duration();
        let mut active = false;
        self.entries.retain(|_, entry| {
            if entry.seen + 1 < frame {
                return false;
            }
            let settled = entry.settled(now, duration);
            if !settled {
                active = true;
            }
            !(settled && entry.target == 0.0)
        });
        active
    }
}

thread_local! {
    static HOVER_FADES: RefCell<HoverFades> = RefCell::new(HoverFades::default());
    static FOLLOWERS: RefCell<Followers> = RefCell::new(Followers::default());
    static FRAME_REDUCED: Cell<bool> = const { Cell::new(false) };
}

/// Hover progress (0..1) for `key` this frame.
pub fn hover_t(key: &str) -> f32 {
    HOVER_FADES.with(|fades| fades.borrow_mut().value_at(key, Instant::now()))
}

/// Hover progress with a fallback for a cached element whose fade entry was
/// pruned between renders while it stayed hovered (pink `hover_t_or`).
pub fn hover_t_or(key: &str, absent: f32) -> f32 {
    HOVER_FADES.with(|fades| fades.borrow_mut().value_at_or(key, Instant::now(), absent))
}

/// Records a hover flip for `key` (reduced motion snaps).
pub fn set_hover(key: &str, hovered: bool, reduced: bool) {
    HOVER_FADES.with(|fades| {
        fades
            .borrow_mut()
            .set_at(key, hovered, reduced, Instant::now())
    });
}

/// An `.on_hover` listener driving the fade for `key`. Pair it with
/// [`hover_t`] / [`hover_blend`] reads of the same key.
pub fn hover_listener(
    key: impl Into<SharedString>,
) -> impl Fn(&bool, &mut Window, &mut App) + 'static {
    let key = key.into();
    move |hovered, window, cx| {
        set_hover(&key, *hovered, reduced_motion(cx));
        // Event dispatch can't request an animation frame (that resolves the
        // current view); refresh redraws, and the render tail keeps frames
        // coming while the fade is in flight.
        window.refresh();
    }
}

/// True while any hover fade whose key starts with `prefix` is in flight
/// (pink `hover_fading_with_prefix`): a cached view calls it from its own
/// render to keep its frames coming.
pub fn hover_fading_with_prefix(prefix: &str) -> bool {
    let now = Instant::now();
    HOVER_FADES.with(|fades| {
        let fades = fades.borrow();
        let duration = HoverFades::duration();
        fades
            .entries
            .iter()
            .any(|(key, entry)| key.starts_with(prefix) && !entry.settled(now, duration))
    })
}

/// `rest → hover` at `key`'s current hover progress (pink `hover_blend`).
pub fn hover_blend(key: &str, rest: Hsla, hover: Hsla) -> Hsla {
    mix(rest, hover, hover_t(key))
}

/// Blends two colors by `t` the way browsers transition them: sRGB with
/// premultiplied alpha, so a wash fading in from transparent never goes grey
/// (pink `mix`).
pub fn mix(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return from;
    }
    if t >= 1.0 {
        return to;
    }
    let (f, g) = (Rgba::from(from), Rgba::from(to));
    let a = lerp(f.a, g.a, t);
    if a <= f32::EPSILON {
        return Hsla::from(Rgba { a: 0.0, ..g });
    }
    Hsla::from(Rgba {
        r: lerp(f.r * f.a, g.r * g.a, t) / a,
        g: lerp(f.g * f.a, g.g * g.a, t) / a,
        b: lerp(f.b * f.a, g.b * g.a, t) / a,
        a,
    })
}

// ---------------------------------------------------------------------------
// Value followers
// ---------------------------------------------------------------------------
//
// A value whose target changes eases from where it was to where it now is
// (pink `Followers`). The first time a key is seen it settles immediately,
// so mounting never replays the motion (the failure `with_animation` has on
// remount).

#[derive(Debug, Clone, Copy)]
struct FollowEntry {
    origin: f32,
    target: f32,
    started: Instant,
    /// Wall-clock span of the whole timeline, delay included.
    duration: Duration,
    spec: MotionSpec,
    seen: u64,
}

impl FollowEntry {
    fn value(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.started);
        if self.duration.is_zero() || elapsed >= self.duration {
            return self.target;
        }
        let raw = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        // Holds through the spec's delay, then eases, like a CSS transition
        // (identical to pink's `curve.eval(raw)` for delay-free specs).
        lerp(self.origin, self.target, self.spec.progress(raw))
    }

    fn settled(&self, now: Instant) -> bool {
        self.origin == self.target || now.saturating_duration_since(self.started) >= self.duration
    }
}

#[derive(Default)]
struct Followers {
    entries: HashMap<String, FollowEntry>,
    frame: u64,
}

impl Followers {
    fn follow_at(
        &mut self,
        key: &str,
        target: f32,
        from: Option<f32>,
        spec: MotionSpec,
        reduced: bool,
        now: Instant,
    ) -> f32 {
        let frame = self.frame;
        let duration = spec.total().mul_f32(speed_scale());
        match self.entries.get_mut(key) {
            Some(entry) => {
                if entry.target != target {
                    let current = entry.value(now);
                    *entry = FollowEntry {
                        origin: if reduced { target } else { current },
                        target,
                        started: now,
                        duration,
                        spec,
                        seen: frame,
                    };
                }
                entry.seen = frame;
                if reduced { target } else { entry.value(now) }
            }
            None => {
                let origin = if reduced {
                    target
                } else {
                    from.unwrap_or(target)
                };
                self.entries.insert(
                    key.to_string(),
                    FollowEntry {
                        origin,
                        target,
                        started: now,
                        duration,
                        spec,
                        seen: frame,
                    },
                );
                origin
            }
        }
    }

    fn active(&self, now: Instant) -> bool {
        self.entries.values().any(|e| !e.settled(now))
    }

    fn tick_at(&mut self, now: Instant) -> bool {
        self.frame += 1;
        let frame = self.frame;
        let mut active = false;
        self.entries.retain(|_, entry| {
            if entry.seen + 1 < frame {
                return false;
            }
            if !entry.settled(now) {
                active = true;
            }
            true
        });
        active
    }
}

/// The eased value of `key` heading to `target` under `spec` (pink
/// `follow`). A new key settles at `target` immediately.
pub fn follow(key: &str, target: f32, spec: MotionSpec) -> f32 {
    let reduced = FRAME_REDUCED.with(|r| r.get());
    FOLLOWERS.with(|f| {
        f.borrow_mut()
            .follow_at(key, target, None, spec, reduced, Instant::now())
    })
}

/// Like [`follow`], but a new key starts at `from` and eases to `target` –
/// for values that should animate in on first mount (progress fills).
pub fn follow_from(key: &str, from: f32, target: f32, spec: MotionSpec) -> f32 {
    let reduced = FRAME_REDUCED.with(|r| r.get());
    FOLLOWERS.with(|f| {
        f.borrow_mut()
            .follow_at(key, target, Some(from), spec, reduced, Instant::now())
    })
}

// ---------------------------------------------------------------------------
// Frame driving
// ---------------------------------------------------------------------------

/// Once per frame, at the top of the root view's render: advances and prunes
/// the hover fades and followers, and requests another frame while any of
/// them is still moving.
pub fn frame(window: &mut Window, cx: &App) {
    note_frame_reduced(reduced_motion(cx));
    if hover_fades_active() {
        window.request_animation_frame();
    }
}

/// The once-per-frame tick behind [`frame`] (pink `hover_fades_active`):
/// advances and prunes the hover fades and followers and reports whether any
/// is still in flight, for hosts that schedule their own frames.
pub fn hover_fades_active() -> bool {
    let now = Instant::now();
    let hovers = HOVER_FADES.with(|fades| fades.borrow_mut().tick_at(now));
    let values = FOLLOWERS.with(|followers| followers.borrow_mut().tick_at(now));
    hovers | values
}

/// Records the window's reduced-motion reading for context-free builders
/// ([`follow`]); [`frame`] calls it (pink `note_frame_reduced`).
pub fn note_frame_reduced(reduced: bool) {
    FRAME_REDUCED.with(|r| r.set(reduced));
}

/// Requests a frame for the current view while a fade or follower is moving.
/// Components call this at the end of their render; it never prunes.
///
/// A view whose children are builders (`RenderOnce`) should use [`driver`]
/// instead: builders render after their parent's `render` returns, so a
/// follower they start would be missed here.
pub fn keep_alive(window: &mut Window) {
    let now = Instant::now();
    let moving =
        HOVER_FADES.with(|f| f.borrow().active(now)) || FOLLOWERS.with(|f| f.borrow().active(now));
    if moving {
        window.request_animation_frame();
    }
}

/// An invisible element that calls [`keep_alive`] after layout, once every
/// child has rendered and registered its fades and followers. Add it as the
/// last child of a view that hosts animated builders.
pub fn driver() -> impl IntoElement {
    gpui::canvas(|_, window, _| keep_alive(window), |_, _, _, _| {})
        .absolute()
        .size_0()
}

// ---------------------------------------------------------------------------
// Reduced motion and speed
// ---------------------------------------------------------------------------

/// Dev knob (`PINK_UI_MOTION_SCALE`, or pink's `PINK_MOTION_SCALE`; default
/// 1, clamped 0.01..100): stretches every timeline, e.g. `10` slows a 150ms
/// fade to 1.5s for frame-by-frame screenshots.
pub fn speed_scale() -> f32 {
    static SCALE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var("PINK_UI_MOTION_SCALE")
            .or_else(|_| std::env::var("PINK_MOTION_SCALE"))
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|s| s.is_finite())
            .map(|s| s.clamp(0.01, 100.0))
            .unwrap_or(1.0)
    })
}

/// Marks the reduced-motion flag as pinned by an explicit caller (pink
/// `ReducedMotionPin`), so a system observer leaves it alone.
struct ReducedMotionPin;

impl Global for ReducedMotionPin {}

/// Turns reduced motion on or off (`prefers-reduced-motion: reduce`) through
/// GPUI's own flag, as pink does: every `with_animation` element snaps
/// (oneshots to their end, loops to their start), the pulse clock parks,
/// hover fades and followers snap, and components skip their own loops.
pub fn set_reduced_motion(cx: &mut App, reduced: bool) {
    cx.set_global(ReducedMotionPin);
    cx.set_reduce_motion(reduced);
    cx.refresh_windows();
}

/// True once [`set_reduced_motion`] pinned the flag (pink
/// `reduced_motion_pinned`).
pub fn reduced_motion_pinned(cx: &App) -> bool {
    cx.has_global::<ReducedMotionPin>()
}

/// The reduced-motion flag (GPUI's `App::reduce_motion`).
pub fn reduced_motion(cx: &App) -> bool {
    cx.reduce_motion()
}

/// Live pulse-clock leases, for loader tests that check a loader really
/// drives the clock when it renders.
#[cfg(test)]
pub(crate) fn pulse_leases(cx: &App) -> usize {
    cx.try_global::<PulseClock>().map_or(0, |c| c.leases.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, tol: f32, what: &str) {
        assert!((a - b).abs() <= tol, "{what}: got {a}, expected {b} ±{tol}");
    }

    #[test]
    fn bezier_endpoints_and_monotonic() {
        for curve in [
            EASE,
            EASE_OUT,
            EASE_IN_OUT,
            EASE_OUT_QUINT,
            EASE_OUT_EXPO,
            EASE_TAILWIND,
            CURVE_STANDARD,
            CURVE_ACCELERATE,
            DISCLOSURE_CURVE,
        ] {
            assert_eq!(curve.eval(0.0), 0.0);
            assert_eq!(curve.eval(1.0), 1.0);
            let mut last = 0.0;
            for i in 0..=10_000 {
                let y = curve.eval(i as f32 / 10_000.0);
                assert!((0.0..=1.0).contains(&y), "escaped [0,1]");
                assert!(y + 1e-4 >= last, "not monotonic");
                last = y;
            }
        }
        assert!((LINEAR.eval(0.3) - 0.3).abs() < 1e-3);
    }

    #[test]
    fn bezier_matches_pink_reference_values() {
        // pink motion.rs `bezier_known_values` (80-step bisection references).
        let cases: [(&str, CubicBezier, [f32; 5]); 3] = [
            (
                "expo",
                EASE_OUT_EXPO,
                [0.494391, 0.825622, 0.971779, 0.997677, 0.999878],
            ),
            (
                "ease-out",
                EASE_OUT,
                [0.160572, 0.378138, 0.684643, 0.906535, 0.982973],
            ),
            (
                "ease",
                EASE,
                [0.094796, 0.408511, 0.802403, 0.960459, 0.994316],
            ),
        ];
        for (name, curve, expected) in cases {
            for (x, want) in [0.1, 0.25, 0.5, 0.75, 0.9].into_iter().zip(expected) {
                close(curve.eval(x), want, 1e-3, name);
            }
        }
    }

    #[test]
    fn catalog_matches_pink() {
        let ms = |spec: MotionSpec| (spec.duration_ms, spec.delay_ms, spec.curve);
        assert_eq!(
            ms(HOVER_FADE),
            (150, 0, CubicBezier::new(0.4, 0.0, 0.2, 1.0))
        );
        assert_eq!(ms(MENU_IN), (180, 0, EASE_OUT_QUINT));
        assert_eq!(ms(MENU_OUT), (120, 0, EASE));
        assert_eq!(
            ms(DISCLOSURE),
            (300, 0, CubicBezier::new(0.32, 0.72, 0.0, 1.0))
        );
        assert_eq!(ms(QUICK), (160, 0, CubicBezier::new(0.2, 0.0, 0.0, 1.0)));
        assert_eq!(ms(SPLASH_OUT), (500, 150, EASE));
        assert_eq!(ms(RESORT), (260, 0, EASE_OUT_QUINT));
        assert_eq!(ms(exit_of(STANDARD)), (180, 0, CURVE_ACCELERATE));
        assert_eq!(ORB_BREATH.duration_ms, 1600);
        assert_eq!(STATUS_SHIMMER.duration_ms, 3400);
        assert_eq!(ms(STATUS_SWAP), (180, 0, EASE_OUT_QUINT));
        assert_eq!(PINK_PULSE.duration_ms, 2400);
        assert_eq!(GRADIENT_SPIN.duration_ms, 750);
        // Aliases follow the pink preset that plays their role.
        assert_eq!(GLIDE, DISCLOSURE);
        assert_eq!(ROW_IN, TOOL_ROW_REVEAL);
        assert_eq!(VIEW_IN, TRANSCRIPT_IN);
    }

    #[test]
    fn spec_progress_holds_through_delay() {
        let spec = MotionSpec::new(100, LINEAR).with_delay(100);
        assert_eq!(spec.progress(0.25), 0.0);
        assert!((spec.progress(0.75) - 0.5).abs() < 1e-3);
        assert_eq!(spec.progress(1.0), 1.0);
        // pink `spec_delay_holds_then_runs`.
        assert_eq!(SPLASH_OUT.total(), Duration::from_millis(650));
        assert_eq!(SPLASH_OUT.progress(0.2), 0.0);
        let mid = SPLASH_OUT.progress(0.65);
        assert!(mid > 0.0 && mid < 1.0);
    }

    #[test]
    fn pulse_math_matches_pink_proto() {
        close(pulse_wave(0.0), 0.0, 1e-5, "trough");
        close(pulse_wave(0.5), 1.0, 1e-5, "crest");
        close(pulse_opacity(0.0), PULSE_MIN_OPACITY, 1e-5, "dim rest");
        close(pulse_scale(0.5), 1.0, 1e-5, "full scale");
        close(
            staggered_phase(0.0, 1, PULSE_STAGGER),
            1.0 - PULSE_STAGGER,
            1e-5,
            "cell 1 trails",
        );
        close(gspin_opacity(0.0, GSPIN_DIM), 1.0, 1e-5, "starts full");
        close(
            gspin_opacity(0.45, GSPIN_DIM),
            GSPIN_DIM,
            1e-5,
            "dim by 45%",
        );
        close(gspin_opacity(0.9, GSPIN_DIM), GSPIN_DIM, 1e-5, "rests dim");
        close(gspin_opacity(1.0, GSPIN_DIM), 1.0, 1e-5, "back to full");
        let mid_rise = gspin_opacity(0.96, GSPIN_DIM);
        assert!(mid_rise > GSPIN_DIM && mid_rise < 1.0);
        close(mark_cell_stagger(0.0, 840.0), 0.0, 1e-5, "head anchors");
        assert!(mark_cell_stagger(720.0, 0.0) > mark_cell_stagger(0.0, 840.0));
    }

    #[test]
    fn pulse_stride_reestablishes_after_each_paint() {
        // pink `pulse_stride_reestablishes_after_each_paint`.
        let now = Instant::now();
        let mut lease = PulseLease {
            until: now + PULSE_LEASE,
            stride: 2,
        };
        assert!(!lease.take_tick(1), "slow loader skips odd ticks");
        assert!(lease.take_tick(2));
        lease.renew(now, 2);
        lease.renew(now, 1);
        assert!(lease.take_tick(3), "fast animation wins while mounted");
        lease.renew(now, 2);
        assert!(lease.take_tick(4));
        lease.renew(now, 2);
        assert!(
            !lease.take_tick(5),
            "retired fast animation leaves no sticky stride"
        );
        assert!(lease.take_tick(6));
        assert!(!lease.take_tick(7), "unpainted view cannot renew itself");
    }

    #[test]
    fn hover_fade_runs_150ms_and_prunes() {
        let mut fades = HoverFades::default();
        let start = Instant::now();
        let ms = |m: u64| start + Duration::from_millis(m);
        fades.set_at("a", true, false, start);
        let mid = fades.value_at("a", ms(75));
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(fades.value_at("a", ms(150)), 1.0, "done at 150ms");
        // A flip mid-flight re-anchors: no jump.
        fades.set_at("b", true, false, start);
        let at_flip = fades.value_at("b", ms(75));
        fades.set_at("b", false, false, ms(75));
        assert!((fades.value_at("b", ms(75)) - at_flip).abs() < 1e-4);
        fades.set_at("a", false, false, ms(400));
        assert!(fades.tick_at(ms(450)));
        // Unread for a whole frame: pruned
        fades.tick_at(ms(900));
        fades.tick_at(ms(950));
        assert!(fades.entries.is_empty());
        assert_eq!(fades.value_at_or("a", ms(960), 1.0), 1.0, "absent fallback");
    }

    #[test]
    fn follower_settles_on_first_sight() {
        let mut f = Followers::default();
        let now = Instant::now();
        assert_eq!(f.follow_at("k", 1.0, None, GLIDE, false, now), 1.0);
        let v = f.follow_at("k", 0.0, None, GLIDE, false, now);
        assert_eq!(v, 1.0);
        let later = f.follow_at(
            "k",
            0.0,
            None,
            GLIDE,
            false,
            now + Duration::from_millis(100),
        );
        assert!(later < 1.0 && later > 0.0);
        assert_eq!(f.follow_at("n", 1.0, Some(0.0), GLIDE, false, now), 0.0);
    }

    #[test]
    fn fold_and_connector_timing() {
        assert_eq!(fold_progress(true, None), (1.0, 1.0));
        assert_eq!(fold_progress(false, None), (0.0, 1.0));
        let (t, alpha) = fold_progress(true, Some(Duration::from_millis(90)));
        assert!(t > 0.0 && t < 1.0 && alpha > 0.0 && alpha < 1.0);
        let (_, gone) = fold_progress(false, Some(Duration::from_millis(180)));
        assert_eq!(gone, 0.0, "closing body is clear by 60% of the fold");
        assert_eq!(tool_connector_parts(0.3, true), (0.0, 0.0));
        assert_eq!(tool_connector_parts(1.0, false), (1.0, 1.0));
        assert_eq!(tool_connector_continuation(Some(0.45)), 1.0);
        assert_eq!(tool_row_arrival_delay(2, true), Duration::from_millis(220));
        assert_eq!(user_resize_duration_ms(5000.0), 850);
        close(
            resize_bounce_offset(ResizeEdge::Max, 0.32),
            5.0,
            1e-4,
            "peak",
        );
        close(
            resize_bounce_offset(ResizeEdge::Min, 1.0),
            0.0,
            1e-4,
            "home",
        );
    }

    #[test]
    fn mix_premultiplied() {
        let clear = Hsla::transparent_black();
        let white = gpui::white();
        let half = mix(clear, white, 0.5);
        assert!((half.l - 1.0).abs() < 1e-3);
        assert!((half.a - 0.5).abs() < 1e-3);
    }

    #[gpui::test]
    fn pulse_clock_ticks_leased_views_and_reduced_motion_parks_it(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let view = cx.update(|cx| {
            let view = cx.new(|_| ()).entity_id();
            let phase = pulse_delta(&GRADIENT_SPIN, view, cx);
            assert!((0.0..1.0).contains(&phase));
            let clock = cx.global::<PulseClock>();
            assert!(clock.running && clock.leases.contains_key(&view));
            view
        });
        // One tick later the clock has notified the leased view.
        cx.executor().advance_clock(PULSE_TICK);
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(cx.global::<PulseClock>().tick >= 1, "the clock ticked");
            set_reduced_motion(cx, true);
            assert!(reduced_motion(cx) && reduced_motion_pinned(cx));
            assert_eq!(pulse_delta(&ORB_BREATH, view, cx), 0.0, "static frame");
        });
    }
}
