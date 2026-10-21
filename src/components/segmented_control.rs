//! Segmented Control – Switch between two to six options – a raised pill or underline that glides, stretches a little and can be dragged across, with icons, counts and badges
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Segmented Control";
pub const DESCRIPTION: &str = "Switch between two to six options – a raised pill or underline that glides, stretches a little and can be dragged across, with icons, counts and badges";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Segmented Control is not ported yet")
}
