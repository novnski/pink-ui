//! Memory Panel – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Memory Panel";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Memory Panel is not built yet")
}
