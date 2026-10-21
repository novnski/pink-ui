//! Date Range Picker – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Date Range Picker";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Date Range Picker is not built yet")
}
