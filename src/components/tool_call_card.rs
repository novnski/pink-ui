//! Tool Call Card – An agent's tool call in one line – live timer, status and summary, folds open to highlighted arguments and result with copy, retry on errors and a group that sums them up
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Tool Call Card";
pub const DESCRIPTION: &str = "An agent's tool call in one line – live timer, status and summary, folds open to highlighted arguments and result with copy, retry on errors and a group that sums them up";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Tool Call Card is not ported yet")
}
