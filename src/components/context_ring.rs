//! Context Ring: the `pink-context-ring` package (`components/context-ring`),
//! PINK's composer context-window chip as a single-GPUI package the app
//! pins (UI rethink decision 54). The gallery compiles the package's own
//! source here, unchanged, against its GPUI; PINK compiles the same file
//! against its patched one. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/context-ring/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, SharedString, Window, div, prelude::*, px};

use crate::{pink::Pink, story};

pub const NAME: &str = "Context Ring";
pub const DESCRIPTION: &str = "PINK's composer context-window chip as a package the app pins: a 16 pt ring whose arc fills with the share of the window in use, muted, then warning from 75% and danger from 90%, and the share beside it while the row has room";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let track = t.text_faint.opacity(0.25);
        let chip = |id: &str, fraction: Option<f32>, tone: gpui::Hsla, hovered: bool| {
            let label = fraction.map(|f| SharedString::from(format!("{:.0}%", f * 100.0)));
            context_ring(
                id.to_string(),
                fraction,
                label,
                &ContextRingStyle {
                    text: tone,
                    arc: tone,
                    track,
                    wash: if hovered {
                        t.element_hover
                    } else {
                        gpui::transparent_black()
                    },
                },
            )
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(chip("story-low", Some(0.12), t.text_muted, false))
            .child(chip("story-warm", Some(0.8), t.warning, true))
            .child(chip("story-full", Some(0.94), t.danger, false))
            .child(chip("story-unreported", None, t.text_faint, false))
    })
}
