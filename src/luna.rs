//! luna's design language, for the original components.
//!
//! The affectUI ports keep affectUI's own tokens. The components that are our
//! own designs (inspired by affectUI's Pro items, see `components::originals`)
//! use luna's look instead: Geist type, Tailwind-neutral surfaces, the indigo
//! luna accent, a 4pt spacing grid, the 4/6/8/10/16 radius ladder and the
//! 24/28/32 control sizes. Values come from luna-ui's `theme` and `button`
//! modules.
//!
//! ```ignore
//! let t = Luna::get(cx);
//! div().bg(t.surface_card).border_1().border_color(t.border).rounded(px(radius::LG))
//!     .child(luna::button("deploy", ButtonKind::Primary, ControlSize::Md, &t).child("Deploy"))
//! ```

use gpui::{
    App, BoxShadow, Div, ElementId, FontWeight, Hsla, SharedString, Stateful, Svg, div, hsla,
    prelude::*, px, svg,
};

use crate::{ActiveTheme, HoverFill, motion};

/// luna's interface font (bundled under `assets/fonts`).
pub const SANS: &str = "Geist";
/// luna's code font (bundled under `assets/fonts`).
pub const MONO: &str = "Geist Mono";

/// Corner radius ladder: pick by role, and give a nested surface
/// [`radius::inner`] of its container so corners stay concentric.
pub mod radius {
    /// Inline code, kbd hints, checkboxes, progress tracks.
    pub const XS: f32 = 4.0;
    /// Controls up to 24pt, chips, icon buttons, badges.
    pub const SM: f32 = 6.0;
    /// Controls 28–32pt, menu rows, inputs.
    pub const MD: f32 = 8.0;
    /// Cards, code and output surfaces, popovers, notices.
    pub const LG: f32 = 10.0;
    /// Dialogs, palettes, message bubbles, sheets.
    pub const XL: f32 = 16.0;

    /// Radius of a surface inset `inset` pt inside one with radius `outer`.
    pub fn inner(outer: f32, inset: f32) -> f32 {
        (outer - inset).max(XS)
    }
}

/// Spacing on the 4pt grid. `S6` is the icon-to-label gap, `S10` a control's
/// horizontal padding.
pub mod space {
    pub const S2: f32 = 2.0;
    pub const S4: f32 = 4.0;
    pub const S6: f32 = 6.0;
    pub const S8: f32 = 8.0;
    pub const S10: f32 = 10.0;
    pub const S12: f32 = 12.0;
    pub const S16: f32 = 16.0;
    pub const S20: f32 = 20.0;
    pub const S24: f32 = 24.0;
    pub const S32: f32 = 32.0;
}

/// Type sizes: caption 11, footnote 12, body 13, callout 14, title 16/20.
pub mod text {
    pub const CAPTION: f32 = 11.0;
    pub const FOOTNOTE: f32 = 12.0;
    pub const BODY: f32 = 13.0;
    pub const CALLOUT: f32 = 14.0;
    pub const TITLE: f32 = 16.0;
    pub const LARGE: f32 = 20.0;
}

/// Translucent fill weights for hover, press and selection washes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FillTier {
    /// Resting plates (ghost buttons, wells): dark 4%, light 3%.
    Quaternary,
    /// Hover: dark 8%, light 5%.
    Tertiary,
    /// Pressed, or hover on a filled control: dark 12%, light 8%.
    Secondary,
    /// Selected: dark 16%, light 10%.
    Primary,
}

/// OKLCH → color (CSS Color 4 matrices), as luna writes its palette.
pub fn oklch(l: f32, c: f32, h_deg: f32) -> Hsla {
    let h = h_deg.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_93 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_4 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    let enc = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    gpui::Rgba {
        r: enc(r),
        g: enc(g),
        b: enc(b),
        a: 1.0,
    }
    .into()
}

/// An achromatic tone at OKLCH lightness `l` (Tailwind's neutral scale).
pub fn neutral(l: f32) -> Hsla {
    oklch(l, 0.0, 0.0)
}

fn grey(value: u8) -> Hsla {
    hsla(0.0, 0.0, value as f32 / 255.0, 1.0)
}

/// A shadow with GPUI's own blur semantics (a Gaussian sigma), exactly as
/// luna-ui writes its elevations. `primitives::shadow` converts CSS values
/// instead, which would halve these.
fn shadow(x: f32, y: f32, sigma: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: gpui::point(px(x), px(y)),
        blur_radius: px(sigma),
        spread_radius: px(spread),
    }
}

/// luna's tokens for the active light/dark mode.
#[derive(Clone, Debug)]
pub struct Luna {
    pub dark: bool,
    /// Main content panel: #060606 dark, white light.
    pub bg: Hsla,
    /// Shell / sidebar surface: one step away from `bg`.
    pub surface: Hsla,
    /// Opaque pills and chips that sit proud of the panel.
    pub surface_raised: Hsla,
    pub surface_raised_hover: Hsla,
    /// Inline cards resting on the panel.
    pub surface_card: Hsla,
    /// Popovers, menus and palettes – the highest plane.
    pub surface_overlay: Hsla,
    pub element_hover: Hsla,
    pub element_active: Hsla,
    /// Hairline border.
    pub border: Hsla,
    pub border_strong: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_faint: Hsla,
    /// Primary button plate: near-white on dark, near-black on light.
    pub solid: Hsla,
    pub on_solid: Hsla,
    pub accent: Hsla,
    pub accent_strong: Hsla,
    pub accent_wash: Hsla,
    pub on_accent: Hsla,
    pub danger: Hsla,
    pub danger_strong: Hsla,
    pub warning: Hsla,
    pub success: Hsla,
    pub input_bg: Hsla,
    pub selection: Hsla,
}

impl Luna {
    pub fn get(cx: &App) -> Self {
        if cx.is_dark() {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn dark() -> Self {
        let accent = oklch(0.673, 0.182, 276.935);
        let accent_strong = oklch(0.585, 0.233, 277.117);
        Luna {
            dark: true,
            bg: grey(6),
            surface: grey(13),
            surface_raised: neutral(0.235),
            surface_raised_hover: neutral(0.29),
            surface_card: grey(0x0e),
            surface_overlay: grey(0x16),
            element_hover: hsla(0.0, 0.0, 0.92, 0.11),
            element_active: hsla(0.0, 0.0, 0.92, 0.16),
            border: hsla(0.0, 0.0, 1.0, 0.08),
            border_strong: hsla(0.0, 0.0, 1.0, 0.14),
            text: neutral(0.922),
            text_muted: neutral(0.708),
            text_faint: neutral(0.556),
            solid: neutral(0.922),
            on_solid: grey(0x0e),
            accent,
            accent_strong,
            accent_wash: accent_strong.opacity(0.45),
            on_accent: neutral(0.985),
            danger: oklch(0.704, 0.191, 22.216),
            danger_strong: oklch(0.58, 0.16, 25.0),
            warning: oklch(0.828, 0.189, 84.429),
            success: oklch(0.765, 0.177, 163.223),
            input_bg: hsla(0.0, 0.0, 1.0, 0.03),
            selection: accent.opacity(0.35),
        }
    }

    pub fn light() -> Self {
        let accent = oklch(0.511, 0.262, 276.966);
        Luna {
            dark: false,
            bg: grey(0xff),
            surface: neutral(0.968),
            surface_raised: neutral(0.94),
            surface_raised_hover: neutral(0.90),
            surface_card: grey(0xff),
            surface_overlay: grey(0xff),
            element_hover: hsla(0.0, 0.0, 0.10, 0.06),
            element_active: hsla(0.0, 0.0, 0.10, 0.10),
            border: hsla(0.0, 0.0, 0.0, 0.10),
            border_strong: hsla(0.0, 0.0, 0.0, 0.17),
            text: neutral(0.25),
            text_muted: neutral(0.439),
            text_faint: neutral(0.535),
            solid: neutral(0.205),
            on_solid: neutral(0.985),
            accent,
            accent_strong: accent,
            accent_wash: accent.opacity(0.10),
            on_accent: neutral(0.985),
            danger: oklch(0.577, 0.245, 27.325),
            danger_strong: oklch(0.51, 0.20, 25.0),
            warning: oklch(0.555, 0.163, 48.998),
            success: oklch(0.596, 0.145, 163.225),
            input_bg: grey(0xff),
            selection: accent.opacity(0.24),
        }
    }

    /// A translucent wash for hover, press and selection.
    pub fn fill(&self, tier: FillTier) -> Hsla {
        let alpha = match (tier, self.dark) {
            (FillTier::Quaternary, true) => 0.04,
            (FillTier::Tertiary, true) => 0.08,
            (FillTier::Secondary, true) => 0.12,
            (FillTier::Primary, true) => 0.16,
            (FillTier::Quaternary, false) => 0.03,
            (FillTier::Tertiary, false) => 0.05,
            (FillTier::Secondary, false) => 0.08,
            (FillTier::Primary, false) => 0.10,
        };
        if self.dark {
            hsla(0.0, 0.0, 1.0, alpha)
        } else {
            hsla(0.0, 0.0, 0.0, alpha)
        }
    }

    /// Elevation for menus and popovers: a soft ambient drop plus a tight contact shadow.
    pub fn menu_shadow(&self) -> Vec<BoxShadow> {
        let (ambient, contact) = if self.dark {
            (0.45, 0.30)
        } else {
            (0.16, 0.10)
        };
        vec![
            shadow(0., 10., 24., -6., hsla(0., 0., 0., ambient)),
            shadow(0., 1., 2., 0., hsla(0., 0., 0., contact)),
        ]
    }

    /// Elevation for cards resting on the panel.
    pub fn card_shadow(&self) -> Vec<BoxShadow> {
        let alpha = if self.dark { 0.35 } else { 0.05 };
        vec![shadow(0., 1., 2., 0., hsla(0., 0., 0., alpha))]
    }

    /// Tooltip elevation: one step below menus.
    pub fn tooltip_shadow(&self) -> Vec<BoxShadow> {
        let alpha = if self.dark { 0.35 } else { 0.12 };
        vec![shadow(0., 4., 10., 0., hsla(0., 0., 0., alpha))]
    }

    /// Focus ring color: the accent.
    pub fn focus_ring(&self) -> Hsla {
        self.accent.opacity(0.6)
    }
}

// ---------------------------------------------------------------------------
// Control kit (luna-ui `button`): buttons, icon buttons, chips and badges in
// three sizes, so every control on a surface shares one height and corner.
// ---------------------------------------------------------------------------

/// | Size | Height | Radius | Type | Side pad | Glyph |
/// | ---- | ------ | ------ | ---- | -------- | ----- |
/// | `Sm` | 24     | 6      | 11   | 8        | 12    |
/// | `Md` | 28     | 8      | 12   | 10       | 14    |
/// | `Lg` | 32     | 8      | 13   | 12       | 16    |
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSize {
    Sm,
    Md,
    Lg,
}

impl ControlSize {
    pub const fn height(self) -> f32 {
        match self {
            ControlSize::Sm => 24.0,
            ControlSize::Md => 28.0,
            ControlSize::Lg => 32.0,
        }
    }

    pub const fn radius(self) -> f32 {
        match self {
            ControlSize::Sm => radius::SM,
            ControlSize::Md | ControlSize::Lg => radius::MD,
        }
    }

    pub const fn text(self) -> f32 {
        match self {
            ControlSize::Sm => 11.0,
            ControlSize::Md => 12.0,
            ControlSize::Lg => 13.0,
        }
    }

    pub const fn pad_x(self) -> f32 {
        match self {
            ControlSize::Sm => 8.0,
            ControlSize::Md => 10.0,
            ControlSize::Lg => 12.0,
        }
    }

    pub const fn glyph(self) -> f32 {
        match self {
            ControlSize::Sm => 12.0,
            ControlSize::Md => 14.0,
            ControlSize::Lg => 16.0,
        }
    }
}

/// Button emphasis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The one solid action in a group.
    Primary,
    /// A filled, outlined push button.
    Secondary,
    /// A quiet action on a faint resting plate.
    Ghost,
    /// The accent-filled action.
    Accent,
    /// A destructive action.
    Danger,
}

fn frame(id: impl Into<ElementId>, size: ControlSize) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex_none()
        .h(px(size.height()))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(if size == ControlSize::Sm { 4.0 } else { 6.0 }))
        .rounded(px(size.radius()))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_size(px(size.text()))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .whitespace_nowrap()
}

/// A push button. Add the label (and an optional leading [`glyph`]) and the
/// click handler: `button("save", ButtonKind::Primary, ControlSize::Md, &t).child("Save")`.
pub fn button(
    id: impl Into<SharedString>,
    kind: ButtonKind,
    size: ControlSize,
    t: &Luna,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let (rest, hover, fg, border) = match kind {
        ButtonKind::Primary => (t.solid, t.solid.opacity(0.86), t.on_solid, None),
        ButtonKind::Secondary => (
            t.fill(FillTier::Quaternary),
            t.fill(FillTier::Tertiary),
            t.text,
            Some(t.border),
        ),
        ButtonKind::Ghost => (
            t.fill(FillTier::Quaternary),
            t.fill(FillTier::Tertiary),
            t.text,
            None,
        ),
        ButtonKind::Accent => (
            t.accent_strong,
            t.accent_strong.opacity(0.86),
            t.on_accent,
            None,
        ),
        ButtonKind::Danger => (
            t.danger_strong,
            t.danger_strong.opacity(0.86),
            t.on_accent,
            None,
        ),
    };
    frame(id.clone(), size)
        .px(px(size.pad_x()))
        .when_some(border, |el, b| el.border_color(b))
        .text_color(fg)
        .hover_fill(format!("luna-btn-{id}"), rest, hover)
}

/// A square glyph-only button: transparent at rest, the hover wash under the pointer.
pub fn icon_button(
    id: impl Into<SharedString>,
    glyph_path: impl Into<SharedString>,
    size: ControlSize,
    t: &Luna,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let key = format!("luna-icon-{id}");
    let tone = motion::hover_blend(&key, t.text_muted, t.text);
    frame(id, size)
        .w(px(size.height()))
        .child(glyph(glyph_path, size).text_color(tone))
        .hover_fill(
            key,
            t.fill(FillTier::Tertiary).opacity(0.),
            t.fill(FillTier::Tertiary),
        )
}

/// A leading glyph sized for `size`. GPUI paints an SVG only with its own
/// text color – it never inherits the button's – so chain `.text_color(..)`.
pub fn glyph(path: impl Into<SharedString>, size: ControlSize) -> Svg {
    svg().path(path).size(px(size.glyph())).flex_none()
}

/// A selectable chip: a quiet filled control that takes the accent wash when chosen.
pub fn chip(
    id: impl Into<SharedString>,
    selected: bool,
    size: ControlSize,
    t: &Luna,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let base = frame(id.clone(), size)
        .px(px(size.pad_x()))
        .font_weight(FontWeight::NORMAL);
    if selected {
        base.bg(t.accent_wash)
            .border_color(t.accent.opacity(0.35))
            .text_color(if t.dark { t.text } else { t.accent })
    } else {
        base.text_color(t.text_muted).hover_fill_fg(
            format!("luna-chip-{id}"),
            t.fill(FillTier::Quaternary),
            t.fill(FillTier::Tertiary),
            t.text_muted,
            t.text,
        )
    }
}

/// Tone of a [`badge`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

impl Tone {
    pub fn color(self, t: &Luna) -> Hsla {
        match self {
            Tone::Neutral => t.text_muted,
            Tone::Accent => t.accent,
            Tone::Success => t.success,
            Tone::Warning => t.warning,
            Tone::Danger => t.danger,
        }
    }
}

/// A small status pill: tinted wash, colored label.
pub fn badge(label: impl Into<SharedString>, tone: Tone, t: &Luna) -> Div {
    let color = tone.color(t);
    let wash = if tone == Tone::Neutral {
        t.fill(FillTier::Tertiary)
    } else {
        color.opacity(if t.dark { 0.16 } else { 0.10 })
    };
    div()
        .flex_none()
        .h(px(20.))
        .px(px(6.))
        .flex()
        .items_center()
        .rounded(px(radius::SM))
        .bg(wash)
        .text_size(px(text::CAPTION))
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .child(label.into())
}

/// A keyboard hint: `kbd("⌘K", &t)`.
pub fn kbd(keys: impl Into<SharedString>, t: &Luna) -> Div {
    div()
        .flex_none()
        .h(px(18.))
        .px(px(5.))
        .flex()
        .items_center()
        .rounded(px(radius::XS))
        .bg(t.fill(FillTier::Tertiary))
        .text_size(px(text::CAPTION))
        .font_family(MONO)
        .text_color(t.text_muted)
        .child(keys.into())
}

/// A card resting on the panel: card surface, hairline border, `radius::LG`.
pub fn card(t: &Luna) -> Div {
    div()
        .bg(t.surface_card)
        .border_1()
        .border_color(t.border)
        .rounded(px(radius::LG))
        .shadow(t.card_shadow())
}

/// A floating popover / menu surface.
pub fn overlay(t: &Luna) -> Div {
    div()
        .bg(t.surface_overlay)
        .border_1()
        .border_color(t.border)
        .rounded(px(radius::LG))
        .shadow(t.menu_shadow())
}

/// A hairline divider.
pub fn divider(t: &Luna) -> Div {
    div().h(px(1.)).w_full().bg(t.border)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_scale_matches_tailwind() {
        let to_u8 = |c: Hsla| {
            let rgb: gpui::Rgba = c.into();
            (rgb.r * 255.0).round() as u8
        };
        // neutral-950 is #0a0a0a, neutral-200 is #e5e5e5
        assert_eq!(to_u8(neutral(0.145)), 10);
        assert_eq!(to_u8(neutral(0.922)), 229);
    }
}
