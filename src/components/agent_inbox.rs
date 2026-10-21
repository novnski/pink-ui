//! Agent Inbox – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Agent Inbox";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Agent Inbox is not built yet")
}
