//! Email Draft – Agent-written email for review – switch recipient and tone, edit inline, remove the attachment, send or discard with undo
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Email Draft is not ported yet")
}
