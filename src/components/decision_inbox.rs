//! Decision Inbox – Open decisions with two quick answers each – answered ones confirm and fold away
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Decision Inbox";
pub const DESCRIPTION: &str =
    "Open decisions with two quick answers each – answered ones confirm and fold away";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Decision Inbox is not ported yet")
}
