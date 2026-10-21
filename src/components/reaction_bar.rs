//! Reaction Bar – Like, comment, repost and share in one pill – icons pop and fill, counts roll in digit by digit
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub const NAME: &str = "Reaction Bar";
pub const DESCRIPTION: &str = "Like, comment, repost and share in one pill – icons pop and fill, counts roll in digit by digit";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Reaction Bar is not ported yet")
}
