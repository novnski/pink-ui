//! Scroll Rail: the `pink-scroll-rail` package (`components/scroll-rail`),
//! PINK's floating scroll rail as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The settings page, file tree, terminal and diff viewer
//! stories draw their rails with it; this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/scroll-rail/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, SharedString, Window, div, prelude::*, px};

use crate::{
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Scroll Rail";
pub const DESCRIPTION: &str = "PINK's floating scroll rail as a package the app pins: a quiet thumb at the right edge of a menu while it scrolls, widening and brightening when the pointer reaches the rail, inside a fixed strip so the rows never shift, and the same rail along the bottom of a code fence that scrolls sideways";

/// A menu's height and its rows, long enough to scroll.
const MENU_H: f32 = 180.0;
const ROWS: usize = 12;

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = &Pink::get(cx);
        let menu = |key: &'static str, active: bool| {
            pink::popover_card(t)
                .w(px(220.0))
                .h(px(MENU_H))
                .relative()
                .flex()
                .flex_col()
                .children((0..ROWS).map(move |ix| {
                    pink::menu_row(SharedString::from(format!("{key}-{ix}")), false, t)
                        .child(format!("Chat {}", ix + 1))
                }))
                .child(
                    rail(t.text_faint, 1.0, active, 18.0, 64.0)
                        .id(SharedString::from(format!("{key}-rail"))),
                )
        };
        let fence = |key: &'static str, active: bool| {
            pink::popover_card(t)
                .w(px(220.0))
                .h(px(72.0))
                .relative()
                .overflow_hidden()
                .font_family(pink::MONO)
                .text_color(t.text_muted)
                .child("let rail = horizontal_rail(faint, 1.0, active, left, length);")
                .child(
                    horizontal_rail(t.text_faint, 1.0, active, 28.0, 96.0)
                        .id(SharedString::from(format!("{key}-fence-rail"))),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(16.0))
                    .child(menu("resting", false))
                    .child(menu("active", true)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(16.0))
                    .child(fence("resting", false))
                    .child(fence("active", true)),
            )
    })
}
