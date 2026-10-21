//! Usage Meter – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Usage Meter";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Usage Meter is not built yet")
}
