//! Compact Breakdown – an original component, not built yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Compact Breakdown";
pub const DESCRIPTION: &str = "Not built yet.";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Compact Breakdown is not built yet")
}
