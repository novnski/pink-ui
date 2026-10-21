//! Thinking Loader – A small working pill for AI replies – an eight-ray spinner and a shimmering label that rolls through phases and settles on a check
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Thinking Loader is not ported yet")
}
