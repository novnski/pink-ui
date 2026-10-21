//! Thinking State – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Thinking State";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Thinking State is not built yet")
}
