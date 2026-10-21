//! Smart Recommendation – Suggestion card with a confidence signal, alternatives to switch to and accept with undo
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Smart Recommendation is not ported yet")
}
