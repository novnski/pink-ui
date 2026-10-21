//! Tips Panel – Grid of short tips with one action each – taken tips turn into a done state
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Tips Panel is not ported yet")
}
