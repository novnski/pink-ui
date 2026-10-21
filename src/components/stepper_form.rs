//! Stepper Form – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Stepper Form";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Stepper Form is not built yet")
}
