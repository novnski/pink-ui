//! Building blocks shared by the components.

pub mod style;
pub mod text_input;

pub use style::*;
pub use text_input::{InputEvent, TextInput};

use gpui::{Hsla, Pixels, Svg, prelude::*, svg};

/// A monochrome SVG icon from `assets/`, tinted with `color`.
///
/// GPUI renders `svg()` as a mask, so `stroke="currentColor"` icons take the
/// color passed here. Multi-colored logos should use `img()` instead.
pub fn icon(path: &'static str, size: Pixels, color: Hsla) -> Svg {
    svg().path(path).size(size).flex_none().text_color(color)
}
