// PINK's pull request badge (change_requests.rs `pull_request_badge`,
// `task_pull_request_badge` and `composer_task_pull_request_chip`): the
// compact badge a sidebar row carries, its number in mono and toned by the
// pull request's state, and the larger composer chip that leads with the
// pull request glyph and may end with the word Draft. UI rethink decision
// 54: drawn here, consumed by PINK through a pinned revision.
//
// The package draws only. The host supplies the state's tone already
// resolved for legibility, the plate and label colours already blended over
// its hover fade, the glyph, the mono family and the number (plain or with
// a search highlight); it adds the id, the role, accessible name and focus
// ring, the cursor, the tooltip and the click. Plain `//` comments: the
// pink-ui gallery compiles this file in place with `include!` for its
// story.

use gpui::{
    AbsoluteLength, Div, FontWeight, Hsla, SharedString, Svg, div, prelude::*, px, rems, svg,
};

/// The sidebar badge: its height and label size in pt at the 16 px default,
/// its horizontal padding and corner radius in pixels.
pub const SIDEBAR_HEIGHT: f32 = 16.0;
pub const SIDEBAR_TEXT: f32 = 10.0;
pub const SIDEBAR_PAD_X: f32 = 4.0;
pub const SIDEBAR_RADIUS: f32 = 4.0;
/// The composer chip: its height and label size in pt at the 16 px default,
/// its horizontal padding, part gap and corner radius in pixels.
pub const COMPOSER_HEIGHT: f32 = 20.0;
pub const COMPOSER_TEXT: f32 = 11.0;
pub const COMPOSER_PAD_X: f32 = 7.0;
pub const COMPOSER_GAP: f32 = 5.0;
pub const COMPOSER_RADIUS: f32 = 6.0;
/// The pull request glyph a composer chip leads with, in pt at the 16 px
/// default.
pub const GLYPH: f32 = 11.0;

/// A design size (pt at the 16 px default) in rems, so it grows with the
/// host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// Where the badge sits, which sets its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// A sidebar row or list: the compact badge, the number alone.
    Sidebar,
    /// The composer's context row: the taller chip with the glyph.
    Composer,
}

impl Surface {
    /// The badge's height, in pt at the 16 px default.
    pub fn height(self) -> f32 {
        match self {
            Self::Sidebar => SIDEBAR_HEIGHT,
            Self::Composer => COMPOSER_HEIGHT,
        }
    }

    /// The label's size, in pt at the 16 px default.
    pub fn text(self) -> f32 {
        match self {
            Self::Sidebar => SIDEBAR_TEXT,
            Self::Composer => COMPOSER_TEXT,
        }
    }

    /// The horizontal padding, in pixels.
    pub fn pad_x(self) -> f32 {
        match self {
            Self::Sidebar => SIDEBAR_PAD_X,
            Self::Composer => COMPOSER_PAD_X,
        }
    }

    /// The gap between the parts, in pixels.
    pub fn gap(self) -> f32 {
        match self {
            Self::Sidebar => 0.0,
            Self::Composer => COMPOSER_GAP,
        }
    }

    /// The corner radius, in pixels.
    pub fn radius(self) -> f32 {
        match self {
            Self::Sidebar => SIDEBAR_RADIUS,
            Self::Composer => COMPOSER_RADIUS,
        }
    }
}

/// What a badge draws.
pub struct BadgeStyle {
    /// The plate (the state's tone washed, already blended over the host's
    /// hover fade); `None` draws no plate.
    pub fill: Option<Hsla>,
    /// The label's colour, likewise blended.
    pub text: Hsla,
}

/// The badge's frame: the surface's height, padding, corners and medium
/// label, its parts in a centred line. The host adds the id, everything
/// interactive and the parts: `glyph`, `number` and any plain word (Draft).
pub fn badge(surface: Surface, style: &BadgeStyle) -> Div {
    let badge = div()
        .h(ui_rems(surface.height()))
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(surface.gap()))
        .px(px(surface.pad_x()))
        .rounded(px(surface.radius()))
        .text_size(ui_rems(surface.text()))
        .font_weight(FontWeight::MEDIUM);
    let badge = match style.fill {
        Some(fill) => badge.bg(fill),
        None => badge,
    };
    badge.text_color(style.text)
}

/// The pull request glyph a composer chip leads with, in `color`.
pub fn glyph(path: impl Into<SharedString>, color: Hsla) -> Svg {
    svg()
        .path(path)
        .flex_none()
        .size(ui_rems(GLYPH))
        .text_color(color)
}

/// The number in the host's `mono` family, so its width stays stable as
/// numbers change. `number` is its text, plain or with a search highlight.
pub fn number(mono: impl Into<SharedString>, number: impl IntoElement) -> Div {
    div().font_family(mono).child(number)
}
