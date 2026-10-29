//! Composer Pill: the `pink-composer-pill` package
//! (`components/composer-pill`), PINK's composer surface as a single-GPUI
//! package the app pins (UI rethink decision 54). The gallery compiles the
//! package's own source here, unchanged, against its GPUI; PINK compiles the
//! same file against its patched one. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/composer-pill/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, prelude::*, px};

use crate::{pink::Pink, story};

pub const NAME: &str = "Composer Pill";
pub const DESCRIPTION: &str = "PINK's composer surface as a package the app pins: the rounded, hairline-bordered pill the composer, the question wizard and the queue tray share, lifted on an opaque window and tightening its corners as it docks to the pane's edge";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let surface = |dock: f32, placeholder: &'static str| {
            pill(&PillStyle {
                radius: radius(dock),
                border: t.border,
                background: t.surface_raised,
                lifted: true,
            })
            .w(px(360.0))
            .h(px(124.0))
            .p(px(16.0))
            .text_color(t.text_faint)
            .child(placeholder)
        };
        div()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .child(surface(0.0, "Message this thread"))
            .child(surface(1.0, "Docked to the pane"))
    })
}
