//! Message Actions – The quiet bar under an AI message – copy turns into a check, regenerate spins and unfolds a version switcher, thumbs pop and ask why, read aloud becomes a tiny equalizer
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "Message Actions is not ported yet")
}
