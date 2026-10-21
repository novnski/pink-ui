//! Small helpers for translating affectUI's CSS into GPUI styles.

use gpui::{BoxShadow, Hsla, Rgba, point, px};

/// Inter, bundled under `assets/fonts` – the family every component uses.
pub const SANS: &str = "Inter";
/// JetBrains Mono, bundled under `assets/fonts` – for code and tabular bits.
pub const MONO: &str = "JetBrains Mono";

/// `#rrggbb` → color.
pub fn hex(rgb: u32) -> Hsla {
    gpui::rgb(rgb).into()
}

/// `#rrggbb` with an alpha, like `rgba(r, g, b, a)` in CSS.
pub fn hexa(rgb: u32, alpha: f32) -> Hsla {
    hex(rgb).opacity(alpha)
}

/// Black or white at an alpha – the `rgba(0, 0, 0, .05)` tints the CSS uses everywhere.
pub fn black_a(alpha: f32) -> Hsla {
    hexa(0x000000, alpha)
}

pub fn white_a(alpha: f32) -> Hsla {
    hexa(0xffffff, alpha)
}

/// `color-mix(in srgb, a <weight>, b)`: `weight` is the share of `a`, from 0 to 1.
///
/// For transitions between two states use [`crate::motion::mix`] instead.
pub fn color_mix(a: Hsla, b: Hsla, weight: f32) -> Hsla {
    let a: Rgba = a.into();
    let b: Rgba = b.into();
    let w = weight.clamp(0., 1.);
    let lerp = |x: f32, y: f32| x * w + y * (1. - w);
    Rgba {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    }
    .into()
}

/// `box-shadow: x y blur spread color`.
pub fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(x), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
    }
}

/// `box-shadow: 0 0 0 <width> color` – a ring drawn outside the border box.
pub fn ring(width: f32, color: Hsla) -> BoxShadow {
    shadow(0., 0., 0., width, color)
}
