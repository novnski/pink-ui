//! OTP Input – The verification code field done right – a gliding focus ring, paste and SMS autofill, a calm shake on errors and a morph into Verified
//!
//! Not ported yet.

use gpui::{AnyView, App, Window};

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    crate::story::view(cx, |_, _| "OTP Input is not ported yet")
}
