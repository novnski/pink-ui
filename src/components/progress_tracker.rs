//! Progress Tracker – Onboarding checklist with an animated progress bar, ticks that draw themselves and auto play
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Progress Tracker";
pub const DESCRIPTION: &str =
    "Onboarding checklist with an animated progress bar, ticks that draw themselves and auto play";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Progress Tracker is not ported yet")
}
