//! Switch: the `pink-switch` package (`components/switch`), PINK's toggle
//! switch as a single-GPUI package the app pins (UI rethink decision 54).
//! The gallery compiles the package's own source here, unchanged, against
//! its GPUI; PINK compiles the same file against its patched one. The kit's
//! `pink::toggle_switch` (and so `switch_control`) draws through it. This
//! module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/switch/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, prelude::*, px};

use crate::{
    pink::{FillTier, Pink},
    story,
};

pub const NAME: &str = "Switch";
pub const DESCRIPTION: &str = "PINK's toggle switch as a package the app pins: a macOS-style pill whose white knob slides right and whose track fills with the accent when on, drawn at any point of its travel so the host can ease it";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let off = if t.dark {
            t.fill(FillTier::Primary)
        } else {
            t.text_muted.opacity(0.45)
        };
        let at = |progress: f32| {
            switch(
                &SwitchStyle {
                    track: crate::motion::mix(off, t.accent_strong, progress),
                    knob: gpui::white(),
                },
                progress,
            )
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(16.0))
            .child(at(0.0))
            .child(at(0.5))
            .child(at(1.0))
    })
}
