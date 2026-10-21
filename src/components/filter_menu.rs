//! Filter Menu – Grouped filters with removable chips, collapsible checkbox groups, Clear all and full keyboard control
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Filter Menu";
pub const DESCRIPTION: &str = "Grouped filters with removable chips, collapsible checkbox groups, Clear all and full keyboard control";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Filter Menu is not ported yet")
}
