//! Comment Thread – One comment with replies – an assistant reports what it did, folds open details and answers @mentions
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Comment Thread is not ported yet")
}
