//! Change Summary – What moved since last time – one metric per row with a colored trend
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Change Summary";
pub const DESCRIPTION: &str =
    "What moved since last time – one metric per row with a colored trend";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Change Summary is not ported yet")
}
