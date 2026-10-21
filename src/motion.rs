//! Motion kit: CSS timing curves, transition specs and eased hover fades.
//!
//! Adapted from luna-ui's `motion` module so components animate the way
//! affectUI's CSS does:
//!
//! - [`CubicBezier`] evaluates CSS `cubic-bezier()` exactly; the common
//!   affectUI curves are constants ([`EASE`], [`EASE_OUT_QUINT`], ...).
//! - [`MotionSpec`] is one transition (duration + delay + curve) and turns
//!   into a GPUI [`Animation`] with the delay folded in.
//! - Element helpers ([`fade_in`], [`rise_in`], [`pop_in`]) cover the
//!   entrance keyframes the components share.
//! - Hover fades: GPUI's `.hover()` styles snap, while affectUI puts
//!   `transition: background-color 160ms ease` on every control. [`hover_t`]
//!   and [`hover_listener`] (wrapped by [`crate::interaction::HoverFill`])
//!   ease a per-element progress instead.
//! - [`follow`] eases a value toward a changing target (switch knobs,
//!   segmented thumbs, progress bars) without replaying on remount.
//!
//! Frames: call [`frame`] once at the top of your root view's render (the
//! gallery does). It advances the fades and keeps frames coming while any
//! fade is moving. Components also call [`keep_alive`], so fades still
//! finish in an app that skips [`frame`]; only stale-entry cleanup needs it.
//!
//! translateY is a relative `top` inset: taffy applies it after layout, so
//! like a CSS transform it never moves siblings. GPUI has no scale transform
//! for divs, so scale keyframes are approximated with fade + translate.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui::{
    Animation, AnimationElement, App, ElementId, Global, Hsla, IntoElement, Rgba, SharedString,
    Styled, Window, px,
};

pub use gpui::AnimationExt;

// ---------------------------------------------------------------------------
// Cubic bezier
// ---------------------------------------------------------------------------

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function. Solves x(t) = input
/// by Newton iteration with a bisection fallback.
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

    /// Eased output for progress `x ∈ [0, 1]` (clamped – GPUI's animation
    /// element asserts its delta stays in range).
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

/// CSS `ease`.
pub const EASE: CubicBezier = CubicBezier::new(0.25, 0.1, 0.25, 1.0);
/// CSS `ease-out`.
pub const EASE_OUT: CubicBezier = CubicBezier::new(0.0, 0.0, 0.58, 1.0);
/// CSS `ease-in-out`.
pub const EASE_IN_OUT: CubicBezier = CubicBezier::new(0.42, 0.0, 0.58, 1.0);
/// CSS `ease-in`.
pub const EASE_IN: CubicBezier = CubicBezier::new(0.42, 0.0, 1.0, 1.0);
/// `cubic-bezier(0.22, 1, 0.36, 1)` – affectUI's `--ui-*-ease`, used for
/// nearly every glide, reveal and spring in the library.
pub const EASE_OUT_QUINT: CubicBezier = CubicBezier::new(0.22, 1.0, 0.36, 1.0);
/// `cubic-bezier(0.16, 1, 0.3, 1)`.
pub const EASE_OUT_EXPO: CubicBezier = CubicBezier::new(0.16, 1.0, 0.3, 1.0);
/// `cubic-bezier(0.34, 1.56, 0.64, 1)` – a small overshoot for pops.
pub const EASE_OUT_BACK: CubicBezier = CubicBezier::new(0.34, 1.56, 0.64, 1.0);
/// Linear.
pub const LINEAR: CubicBezier = CubicBezier::new(0.0, 0.0, 1.0, 1.0);

// ---------------------------------------------------------------------------
// Motion specs
// ---------------------------------------------------------------------------

/// One transition: duration + optional delay + curve. GPUI's `Animation`
/// has no delay, so the animation runs for `delay + duration` and
/// [`progress`](Self::progress) holds 0 until the delay has passed.
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

    /// A one-shot GPUI [`Animation`] for this spec (delay folded in, eased).
    pub fn animation(&self) -> Animation {
        let spec = *self;
        Animation::new(spec.total().mul_f32(speed_scale())).with_easing(move |d| spec.progress(d))
    }

    /// A repeating [`Animation`] with linear easing over the raw period, for
    /// loops whose easing happens in the animator.
    pub fn repeating(&self) -> Animation {
        Animation::new(self.total().mul_f32(speed_scale())).repeat()
    }
}

/// Color transitions on controls: `160ms ease`.
pub const HOVER_FADE: MotionSpec = MotionSpec::new(160, EASE);
/// Quick fades: `150ms ease`.
pub const FADE_QUICK: MotionSpec = MotionSpec::new(150, EASE);
/// Popovers and menus opening: `180ms` springy ease-out.
pub const MENU_IN: MotionSpec = MotionSpec::new(180, EASE_OUT_QUINT);
/// Content views swapping in (`ui-*-view` keyframes): `380ms`.
pub const VIEW_IN: MotionSpec = MotionSpec::new(380, EASE_OUT_QUINT);
/// Rows and items entering a list: `420ms`.
pub const ROW_IN: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT);
/// Heights, widths and positions gliding: `420ms`.
pub const GLIDE: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT);
/// Progress fills: `900ms` after a `150ms` beat.
pub const FILL: MotionSpec = MotionSpec::new(900, EASE_OUT_QUINT).with_delay(150);

// ---------------------------------------------------------------------------
// Element helpers
// ---------------------------------------------------------------------------

/// Opacity 0 → 1 over [`FADE_QUICK`].
pub fn fade_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    element.with_animation(id, FADE_QUICK.animation(), |el, t| el.opacity(t))
}

/// Opacity 0 → 1 while rising `rise` px into place – the `translateY(4px)`
/// entrance most affectUI views use. Pass a negative `rise` to drop in.
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

/// Popover entrance: fade + `translateY(-4px)` → 0 over [`MENU_IN`] (stands
/// in for `scale(0.98)`).
pub fn pop_in<E>(id: impl Into<ElementId>, element: E) -> AnimationElement<E>
where
    E: Styled + IntoElement + 'static,
{
    rise_in(id, MENU_IN, -4.0, element)
}

/// Linear interpolation.
pub fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

// ---------------------------------------------------------------------------
// Hover fades (CSS `transition` parity)
// ---------------------------------------------------------------------------
//
// A per-element-key hover progress, advanced from wall time on every read.
// The store is a main-thread thread_local rather than a GPUI global so
// builders can blend colors without threading `cx` everywhere. Entries are
// stamped with the frame counter on every read; [`frame`] prunes entries
// that went a whole frame unread (their element unmounted mid-hover and will
// never get a leave event) and entries that settled back at rest.

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
        let frame = self.frame;
        match self.entries.get_mut(key) {
            Some(entry) => {
                entry.seen = frame;
                entry.value(now, Self::duration())
            }
            None => 0.0,
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
        window.refresh();
    }
}

/// `rest → hover` at `key`'s current hover progress.
pub fn hover_blend(key: &str, rest: Hsla, hover: Hsla) -> Hsla {
    mix(rest, hover, hover_t(key))
}

/// Blends two colors by `t` the way browsers transition them: sRGB with
/// premultiplied alpha, so a wash fading in from transparent never goes grey.
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
// A value whose target changes eases from where it was to where it now is.
// The first time a key is seen it settles immediately, so mounting never
// replays the motion (the failure `with_animation` has on remount).

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
        // Holds through the spec's delay, then eases, like a CSS transition.
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

/// The eased value of `key` heading to `target` under `spec`. A new key
/// settles at `target` immediately.
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
    FRAME_REDUCED.with(|r| r.set(reduced_motion(cx)));
    let now = Instant::now();
    let hovers = HOVER_FADES.with(|fades| fades.borrow_mut().tick_at(now));
    let values = FOLLOWERS.with(|followers| followers.borrow_mut().tick_at(now));
    if hovers || values {
        window.request_animation_frame();
    }
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
// Reduced motion
// ---------------------------------------------------------------------------

/// Dev knob (`AFFECTUI_MOTION_SCALE`, default 1): stretches every timeline,
/// e.g. `10` slows a 160ms fade to 1.6s for frame-by-frame screenshots.
pub fn speed_scale() -> f32 {
    static SCALE: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *SCALE.get_or_init(|| {
        std::env::var("AFFECTUI_MOTION_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|s| s.is_finite())
            .map(|s| s.clamp(0.01, 100.0))
            .unwrap_or(1.0)
    })
}

#[derive(Default, Clone, Copy)]
struct ReducedMotion(bool);

impl Global for ReducedMotion {}

/// Turns reduced motion on or off (`prefers-reduced-motion: reduce`):
/// hover fades and followers snap, and components skip their loops.
pub fn set_reduced_motion(cx: &mut App, reduced: bool) {
    cx.set_global(ReducedMotion(reduced));
    cx.refresh_windows();
}

pub fn reduced_motion(cx: &App) -> bool {
    cx.try_global::<ReducedMotion>().is_some_and(|r| r.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_endpoints_and_monotonic() {
        for curve in [EASE, EASE_OUT, EASE_IN_OUT, EASE_OUT_QUINT, EASE_OUT_EXPO] {
            assert_eq!(curve.eval(0.0), 0.0);
            assert_eq!(curve.eval(1.0), 1.0);
            let mut last = 0.0;
            for i in 1..=100 {
                let y = curve.eval(i as f32 / 100.0);
                assert!(y + 1e-4 >= last, "not monotonic");
                last = y;
            }
        }
        assert!((LINEAR.eval(0.3) - 0.3).abs() < 1e-3);
    }

    #[test]
    fn spec_progress_holds_through_delay() {
        let spec = MotionSpec::new(100, LINEAR).with_delay(100);
        assert_eq!(spec.progress(0.25), 0.0);
        assert!((spec.progress(0.75) - 0.5).abs() < 1e-3);
        assert_eq!(spec.progress(1.0), 1.0);
    }

    #[test]
    fn hover_fade_runs_and_prunes() {
        let mut fades = HoverFades::default();
        let start = Instant::now();
        fades.set_at("a", true, false, start);
        let mid = fades.value_at("a", start + Duration::from_millis(80));
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(fades.value_at("a", start + Duration::from_millis(400)), 1.0);
        fades.set_at("a", false, false, start + Duration::from_millis(400));
        assert!(fades.tick_at(start + Duration::from_millis(450)));
        // Unread for a whole frame: pruned
        fades.tick_at(start + Duration::from_millis(900));
        fades.tick_at(start + Duration::from_millis(950));
        assert!(fades.entries.is_empty());
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
    fn mix_premultiplied() {
        let clear = Hsla::transparent_black();
        let white = gpui::white();
        let half = mix(clear, white, 0.5);
        assert!((half.l - 1.0).abs() < 1e-3);
        assert!((half.a - 0.5).abs() < 1e-3);
    }
}
