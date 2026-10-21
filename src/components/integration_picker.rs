//! Integration Picker – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Integration Picker";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Integration Picker is not built yet")
}
