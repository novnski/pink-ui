//! Icon Button: the `pink-icon-button` package (`components/icon-button`),
//! PINK's titlebar icon button as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/icon-button/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Window, div, prelude::*, px, svg};

use crate::{
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Icon Button";
pub const DESCRIPTION: &str = "PINK's titlebar icon button as a package the app pins: a square that grows with the Interface size between the 24 pt minimum hit and 32 pt, the control radius, a hover wash and a centred glyph";

const ICON_SIDEBAR: &str = "icons/settings-page/sidebar-minimalistic-left.svg";
const ICON_BACK: &str = "icons/settings-page/arrow-left.svg";
const ICON_FORWARD: &str = "icons/settings-page/arrow-right.svg";

/// One titlebar button at Interface `scale`, drawn by the package with the
/// kit's tokens (`hovered` shows the wash a pointer would fade in).
pub fn titlebar_button(
    id: String,
    glyph: &'static str,
    hovered: bool,
    enabled: bool,
    scale: f32,
    t: &Pink,
) -> gpui::Stateful<gpui::Div> {
    let glyph = svg()
        .path(glyph)
        .size(px(scaled_icon(ICON_M, scale)))
        .text_color(if enabled {
            t.text_muted
        } else {
            t.disabled_glyph()
        })
        .into_any_element();
    icon_button(
        id,
        glyph,
        &IconButtonStyle {
            size: scaled_control(CONTROL_M, scale),
            radius: RADIUS,
            wash: if hovered && enabled {
                t.glass_hover()
            } else {
                t.glass_hover().opacity(0.0)
            },
            enabled,
        },
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let scale = pink::ui_scale(cx);
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(pink::layout::TITLEBAR_CONTROL_GAP))
            .child(titlebar_button(
                "story-sidebar".into(),
                ICON_SIDEBAR,
                false,
                true,
                scale,
                &t,
            ))
            .child(titlebar_button(
                "story-back".into(),
                ICON_BACK,
                true,
                true,
                scale,
                &t,
            ))
            .child(titlebar_button(
                "story-forward".into(),
                ICON_FORWARD,
                false,
                false,
                scale,
                &t,
            ))
    })
}
