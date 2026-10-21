//! Button – Primary, secondary, ghost and destructive buttons with icons, sizes and a shimmering loading state
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Button";
pub const DESCRIPTION: &str = "Primary, secondary, ghost and destructive buttons with icons, sizes and a shimmering loading state";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Button is not ported yet")
}
