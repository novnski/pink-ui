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
//! pointer and `wash(0.1)` held while latched. The file editor header (PINK
//! files/preview.rs `render_breadcrumb`) puts the file's artwork and its
//! path before its controls: the leaf in 12 pt medium keeps its width while
//! the 11 pt faint folders shrink, quiet chevrons between, then text
//! controls such as Preview or a save status. Commit History's toolbar uses
//! the compact search field that grows out of a magnifier and the labelled
//! Fetch all control.

#[allow(dead_code)]
mod package {
    include!("../../components/surface-toolbar/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, ElementId, Hsla, Window, div, img, prelude::*, px, svg};

use crate::{
    components::icon_button::{IconButtonStyle, icon_button},
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "Surface Toolbar";
pub const DESCRIPTION: &str = "PINK's surface toolbar as a package the app pins: the 38 px control row on the chrome surface, the rounded search field with its magnifier taking the free width, the toggles' glyphs on 28 px icon buttons, latched or quiet, the file editor header's breadcrumb and text controls, and History's compact search and Fetch all";

const ICON_SEARCH: &str = "icons/file-tree/magnifer.svg";
const ICON_TEXT_SEARCH: &str = "icons/file-tree/file-search.svg";
const ICON_EYE_CLOSED: &str = "icons/file-tree/eye-closed.svg";
const ICON_CHEVRON: &str = "icons/file-tree/alt-arrow-right.svg";
const ICON_FOLDER: &str = "icons/file-tree/file-folder-dark.svg";
const ICON_CODE: &str = "icons/file-tree/file-code-orange.svg";
const ICON_CLOUD: &str = "icons/commit-history/cloud.svg";
const ICON_CLOSE: &str = "icons/commit-history/close.svg";
const ICON_MAGNIFER: &str = "icons/commit-history/magnifer.svg";

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
        // The file editor header over a path, with a save status.
        let editor = |path: &'static str, status: Option<&'static str>| {
            let parts: Vec<&str> = path.split('/').collect();
            let style = CrumbStyle {
                sans: pink::SANS.into(),
                leaf: t.text,
                parent: t.text_faint,
            };
            let mut crumbs = breadcrumb(format!("surface-toolbar-crumbs-{path}"));
            for (index, part) in parts.iter().enumerate() {
                if index > 0 {
                    crumbs = crumbs.child(crumb_separator(
                        svg().path(ICON_CHEVRON),
                        t.text_faint.opacity(0.65),
                    ));
                }
                crumbs = crumbs.child(crumb(part.to_string(), index + 1 == parts.len(), &style));
            }
            editor_header()
                .child(file_artwork(img(ICON_CODE)))
                .child(crumbs)
                .children(status.map(|status| {
                    text_control(
                        format!("surface-toolbar-status-{path}"),
                        &TextControlStyle {
                            sans: pink::SANS.into(),
                            text: t.warning_muted,
                            wash: gpui::transparent_black(),
                            enabled: true,
                        },
                    )
                    .child(status)
                }))
                .child(toggle(
                    format!("surface-toolbar-reveal-{path}"),
                    ICON_FOLDER,
                    false,
                    &t,
                ))
        };
        // History's compact search and Fetch all.
        let history = toolbar()
            .child(div().flex_1())
            .child(
                compact_field("surface-toolbar-history-search", 196.0, t.ink(0.035))
                    .child(compact_field_status(compact_field_glyph(
                        svg().path(ICON_MAGNIFER),
                        t.text_faint,
                    )))
                    .child(compact_field_input(
                        div().text_color(t.text_faint).child("Search commits"),
                    ))
                    .child(
                        compact_close("surface-toolbar-history-close", gpui::transparent_black())
                            .child(compact_close_glyph(svg().path(ICON_CLOSE), t.text_faint)),
                    ),
            )
            .child(
                labeled_control(
                    "surface-toolbar-history-fetch",
                    &LabeledControlStyle {
                        wash: gpui::transparent_black(),
                        enabled: true,
                    },
                )
                .child(control_glyph(
                    svg().path(ICON_CLOUD),
                    t.readable_muted(0.75),
                ))
                .child(control_label("Fetch all", t.text_muted)),
            );
        div()
            .w(px(420.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .bg(t.chrome_surface())
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(bar("", false))
            .child(bar("stripe", true))
            .child(editor("crates/ui/src/files/preview.rs", None))
            .child(editor("src/lib.rs", Some("Deleted on disk")))
            .child(history)
    })
}
