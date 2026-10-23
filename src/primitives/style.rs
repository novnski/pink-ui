//! Small helpers for writing CSS values (hex colors, box shadows) as GPUI styles.

use gpui::{BoxShadow, Hsla, Pixels, Rgba, Window, point, px};

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

/// `box-shadow: x y blur spread color`, with CSS semantics.
///
/// GPUI's `blur_radius` is a Gaussian sigma, while a CSS blur radius is about
/// two sigmas, so the blur is halved. A sigma of 0 draws nothing in GPUI's
/// shader, so it is kept just above that: crisp rings stay visible. A
/// sub-pixel ring (`0 0 0 0.5px`) becomes one pixel at proportional alpha,
/// which is how browsers draw it at 1x.
///
/// GPUI paints a list of shadows last-on-top, the reverse of CSS; build lists
/// with [`shadows`] to keep the CSS order.
pub fn shadow(x: f32, y: f32, blur: f32, spread: f32, color: Hsla) -> BoxShadow {
    let (spread, color) = if blur == 0. && spread > 0. && spread < 1. {
        (1., color.opacity(spread))
    } else {
        (spread, color)
    };
    BoxShadow {
        color,
        offset: point(px(x), px(y)),
        blur_radius: px((blur / 2.).max(0.35)),
        spread_radius: px(spread),
        inset: false,
    }
}

/// `box-shadow: 0 0 0 <width> color` – a ring drawn outside the border box.
pub fn ring(width: f32, color: Hsla) -> BoxShadow {
    shadow(0., 0., 0., width, color)
}

/// A `box-shadow` list in CSS order (first on top), reordered for GPUI.
pub fn shadows(css_order: impl IntoIterator<Item = BoxShadow>) -> Vec<BoxShadow> {
    let mut list: Vec<BoxShadow> = css_order.into_iter().collect();
    list.reverse();
    list
}

/// The thinnest visible line: one device pixel, never under 0.5px.
///
/// GPUI clips each border side to its width, so a `0.5px` border can vanish
/// on a 1x screen; browsers draw it as one device pixel. Use this for CSS
/// `0.5px` hairlines: `.border_b(hairline(window))`.
pub fn hairline(window: &Window) -> Pixels {
    px((1.0 / window.scale_factor()).max(0.5))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_follows_css() {
        let s = shadow(0., 4., 12., 0., black_a(0.2));
        assert_eq!(s.blur_radius, px(6.));
        // A crisp ring keeps a drawable sigma
        assert!(ring(1., black_a(0.1)).blur_radius > px(0.));
        // A half-pixel ring becomes one pixel at half the alpha
        let hair = ring(0.5, black_a(0.2));
        assert_eq!(hair.spread_radius, px(1.));
        assert!((hair.color.a - 0.1).abs() < 1e-4);
    }

    #[test]
    fn shadows_keep_css_order() {
        let list = shadows([
            ring(1., black_a(0.1)),
            shadow(0., 8., 24., 0., black_a(0.2)),
        ]);
        assert_eq!(list[1].spread_radius, px(1.));
    }
}
