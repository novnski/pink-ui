//! Notification Stack – Notice cards stacked behind each other – dismiss the front one to reveal the next
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Notification Stack is not ported yet")
}
