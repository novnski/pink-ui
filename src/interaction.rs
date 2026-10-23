//! Eased hover feedback, ported from the Pink app's `interaction` module.
//!
//! GPUI's `.hover(|s| s.bg(..))` applies on the frame the pointer enters, so
//! it snaps. Pink eases every wash instead, on the shared
//! [`motion::HOVER_FADE`] (Tailwind `transition-colors`: 150ms
//! `cubic-bezier(0.4, 0, 0.2, 1)`), through [`HoverFill`]. Colors come from
//! the caller (pink's washes are its fill tiers, `crate::pink::Pink::fill`).
//!
//! Keys must be unique among the elements on screen: two elements sharing a
//! key share one fade. Derive the key from the element id, prefixed by the
//! component (`"pm-item-settings"`).
//!
//! ```ignore
//! div()
//!     .id("save")
//!     .hover_fill_fg("save", p.surface, p.surface_hover, p.muted, p.ink)
//!     .child("Save")
//! ```

use gpui::{Hsla, SharedString, StatefulInteractiveElement, Styled};

use crate::motion;

/// Background, border and foreground of an outlined control in one state.
#[derive(Clone, Copy, Debug)]
pub struct HoverColors {
    pub bg: Hsla,
    pub border: Hsla,
    pub fg: Hsla,
}

/// Eased hover feedback for a stateful (`.id(..)`) element.
pub trait HoverFill: Sized {
    /// Background eases `rest → hover` while the pointer is over the element.
    fn hover_fill(self, key: impl Into<SharedString>, rest: Hsla, hover: Hsla) -> Self;

    /// Pink's standard quiet hover (pink `hover_wash`): transparent at rest,
    /// `wash` under the pointer. Pink passes its Tertiary fill
    /// (`Pink::fill(FillTier::Tertiary)`); the rest color is the wash at
    /// zero alpha, so the premultiplied blend never greys mid-fade.
    fn hover_wash(self, key: impl Into<SharedString>, wash: Hsla) -> Self {
        self.hover_fill(key, wash.opacity(0.0), wash)
    }

    /// Background and foreground ease together (a label or glyph that
    /// brightens as its plate fills in).
    fn hover_fill_fg(
        self,
        key: impl Into<SharedString>,
        rest: Hsla,
        hover: Hsla,
        rest_fg: Hsla,
        hover_fg: Hsla,
    ) -> Self;

    /// Only the foreground eases (text links, bare glyph buttons).
    fn hover_fg(self, key: impl Into<SharedString>, rest_fg: Hsla, hover_fg: Hsla) -> Self;

    /// Only the border eases (outlined chips and cards).
    fn hover_border(self, key: impl Into<SharedString>, rest: Hsla, hover: Hsla) -> Self;

    /// Opacity eases from `rest` to `hover` (image and swatch buttons).
    fn hover_opacity(self, key: impl Into<SharedString>, rest: f32, hover: f32) -> Self;

    /// Fill, edge and label ease together (outlined push buttons).
    fn hover_colors(
        self,
        key: impl Into<SharedString>,
        rest: HoverColors,
        hover: HoverColors,
    ) -> Self;
}

impl<E: StatefulInteractiveElement + Styled> HoverFill for E {
    fn hover_fill(self, key: impl Into<SharedString>, rest: Hsla, hover: Hsla) -> Self {
        let key = key.into();
        self.bg(motion::hover_blend(&key, rest, hover))
            .on_hover(motion::hover_listener(key))
    }

    fn hover_fill_fg(
        self,
        key: impl Into<SharedString>,
        rest: Hsla,
        hover: Hsla,
        rest_fg: Hsla,
        hover_fg: Hsla,
    ) -> Self {
        let key = key.into();
        let t = motion::hover_t(&key);
        self.bg(motion::mix(rest, hover, t))
            .text_color(motion::mix(rest_fg, hover_fg, t))
            .on_hover(motion::hover_listener(key))
    }

    fn hover_fg(self, key: impl Into<SharedString>, rest_fg: Hsla, hover_fg: Hsla) -> Self {
        let key = key.into();
        self.text_color(motion::hover_blend(&key, rest_fg, hover_fg))
            .on_hover(motion::hover_listener(key))
    }

    fn hover_border(self, key: impl Into<SharedString>, rest: Hsla, hover: Hsla) -> Self {
        let key = key.into();
        self.border_color(motion::hover_blend(&key, rest, hover))
            .on_hover(motion::hover_listener(key))
    }

    fn hover_opacity(self, key: impl Into<SharedString>, rest: f32, hover: f32) -> Self {
        let key = key.into();
        let t = motion::hover_t(&key);
        self.opacity(motion::lerp(rest, hover, t))
            .on_hover(motion::hover_listener(key))
    }

    fn hover_colors(
        self,
        key: impl Into<SharedString>,
        rest: HoverColors,
        hover: HoverColors,
    ) -> Self {
        let key = key.into();
        let t = motion::hover_t(&key);
        self.bg(motion::mix(rest.bg, hover.bg, t))
            .border_color(motion::mix(rest.border, hover.border, t))
            .text_color(motion::mix(rest.fg, hover.fg, t))
            .on_hover(motion::hover_listener(key))
    }
}
