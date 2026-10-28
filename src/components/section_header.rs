//! Section Header: the `pink-section-header` package
//! (`components/section-header`), PINK's sidebar section and shelf headers
//! as a single-GPUI package the app pins (UI rethink decision 54). The
//! gallery compiles the package's own source here, unchanged, against its
//! GPUI; PINK compiles the same file against its patched one. The Sidebar
//! draws its group and shelf headers with it; this module adds only the
//! story.

#[allow(dead_code)]
mod package {
    include!("../../components/section-header/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, IntoElement, Window, div, prelude::*, px, svg};

use crate::{
    HoverFill, motion,
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "Section Header";
pub const DESCRIPTION: &str = "PINK's sidebar section headers as a package the app pins: the quiet semibold group label whose chevron waits under the pointer, and a bottom shelf's header with its glyph on the title column, its count and the chevron ending on the time column";

const ICON_CHEVRON: &str = "icons/sidebar/alt-arrow-right.svg";
const ICON_CHECK: &str = "icons/sidebar/check.svg";
const ICON_ARCHIVE: &str = "icons/sidebar/archive-minimalistic.svg";

fn glyph(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(ui_rems(size))
        .flex_none()
        .text_color(color)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let scale = pink::ui_scale(cx);
        let chevron = || {
            glyph(
                ICON_CHEVRON,
                CHEVRON,
                t.readable_icon(t.readable_muted(0.5)),
            )
        };
        let group = disclosure_header(scale)
            .id("story-section-pinned")
            .group("story-section-pinned-hdr")
            .hover_wash("story-section-pinned", t.fill(FillTier::Tertiary))
            .child(disclosure_label(t.readable_faint(0.9), "Pinned"))
            .child(div().flex_1())
            .child(reveal_on_hover("story-section-pinned-hdr").child(chevron()));
        let shelf = |key: &'static str, label: &'static str, mark: &'static str, total: usize| {
            shelf_header(scale)
                .id(key)
                .hover_fill_fg(
                    key,
                    gpui::transparent_black(),
                    t.glass_hover(),
                    t.readable_muted(0.68),
                    t.text,
                )
                .child(mark_slot().child(glyph(
                    mark,
                    SHELF_GLYPH,
                    t.readable_icon(motion::hover_blend(
                        key,
                        t.readable_muted(0.5),
                        t.text_muted,
                    )),
                )))
                .child(shelf_label(label))
                .child(
                    shelf_count(t.readable_faint(0.9))
                        .font_features(pink::tabular_figures())
                        .child(total.to_string()),
                )
                .child(trailing_chevron().child(chevron()))
        };
        div()
            .w(px(260.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(group)
            .child(shelf("story-shelf-inactive", "Inactive", ICON_CHECK, 12))
            .child(shelf("story-shelf-archived", "Archived", ICON_ARCHIVE, 48))
            .into_any_element()
    })
}
