//! Billing Schedule – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Billing Schedule";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Billing Schedule is not built yet")
}
