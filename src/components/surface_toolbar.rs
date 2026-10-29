//! Surface Toolbar: the `pink-surface-toolbar` package
//! (`components/surface-toolbar`), PINK's surface toolbar as one
//! single-GPUI package the app pins (UI rethink decision 54). The gallery
//! compiles the package's own source here, unchanged, against its GPUI;
//! PINK compiles the same file against its patched one. File Tree draws its
//! search toolbar through it and Diff Viewer its header toggles' glyphs; this
//! module adds the story.
//!
//! The row is 38 px tall on the chrome surface with no band of its own: the
//! rounded search field (28 pt, `ink(0.035)`, a 12 pt magnifier) takes the
//! free width, then 28 px `pink-icon-button` toggles, a quiet wash under the
//! pointer and `wash(0.1)` held while latched.

#[allow(dead_code)]
mod package {
    include!("../../components/surface-toolbar/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, ElementId, Hsla, Window, div, prelude::*, px, svg};

use crate::{
    components::icon_button::{IconButtonStyle, icon_button},
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "Surface Toolbar";
pub const DESCRIPTION: &str = "PINK's surface toolbar as a package the app pins: the 38 px control row on the chrome surface, the rounded search field with its magnifier taking the free width, and the toggles' glyphs on 28 px icon buttons, latched or quiet";

const ICON_SEARCH: &str = "icons/file-tree/magnifer.svg";
const ICON_TEXT_SEARCH: &str = "icons/file-tree/file-search.svg";
const ICON_EYE_CLOSED: &str = "icons/file-tree/eye-closed.svg";

/// One toolbar toggle on `pink-icon-button`, latched or at rest.
pub fn toggle(
    id: impl Into<ElementId>,
    glyph: &'static str,
    on: bool,
    t: &Pink,
) -> gpui::Stateful<gpui::Div> {
    let wash: Hsla = if on {
        t.wash(0.1)
    } else {
        t.fill(FillTier::Tertiary).opacity(0.0)
    };
    icon_button(
        id.into().to_string(),
        control_glyph(svg().path(glyph), if on { t.text } else { t.text_muted }).into_any_element(),
        &IconButtonStyle {
            size: CONTROL_SIZE,
            radius: CONTROL_RADIUS,
            wash,
            enabled: true,
        },
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let bar = |query: &'static str, text_mode: bool| {
            toolbar()
                .child(
                    field(t.ink(0.035))
                        .child(field_glyph(svg().path(ICON_SEARCH), t.text_faint))
                        .child(field_input(
                            div()
                                .text_color(if query.is_empty() {
                                    t.text_faint
                                } else {
                                    t.text
                                })
                                .child(if query.is_empty() {
                                    "Search files"
                                } else {
                                    query
                                }),
                        )),
                )
                .child(toggle(
                    format!("surface-toolbar-text-{query}"),
                    ICON_TEXT_SEARCH,
                    text_mode,
                    &t,
                ))
                .child(toggle(
                    format!("surface-toolbar-eye-{query}"),
                    ICON_EYE_CLOSED,
                    false,
                    &t,
                ))
        };
        div()
            .w(px(320.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .bg(t.chrome_surface())
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(bar("", false))
            .child(bar("stripe", true))
    })
}
