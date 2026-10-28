//! Menu Card: the `pink-menu-card` package (`components/menu-card`),
//! PINK's floating card and its rows as a single-GPUI package the app pins
//! (UI rethink decision 54). The gallery compiles the package's own source
//! here, unchanged, against its GPUI; PINK compiles the same file against
//! its patched one. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/menu-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Window, div, prelude::*, px};

use crate::{pink::Pink, story};

pub const NAME: &str = "Menu Card";
pub const DESCRIPTION: &str = "PINK's floating card as a package the app pins: the bordered, lifted surface every menu, picker and popover opens on, and the one row shape its items share, concentric with the card, washed when selected and a step lighter under the pointer";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let item = |label: &'static str, fill: gpui::Hsla, text: gpui::Hsla| {
            row(&RowStyle { text, fill }).child(label)
        };
        div().w(px(220.0)).child(
            card(&CardStyle {
                border: t.border,
                background: t.surface_raised,
                text: t.text,
                font_family: crate::pink::SANS.into(),
                shadows: None,
            })
            .flex()
            .flex_col()
            .child(item("Sonnet 5.5", t.element_active, t.text))
            .child(item("Opus 5.5", t.element_hover, t.text))
            .child(item(
                "Haiku 5.5",
                gpui::transparent_black(),
                t.text.opacity(0.9),
            )),
        )
    })
}
