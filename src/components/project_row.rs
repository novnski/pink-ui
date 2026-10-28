//! Project Row: the `pink-project-row` package (`components/project-row`),
//! PINK's project picker rows as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. Each row's base is the Menu Card package's row, as in PINK;
//! this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/project-row/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, Window, div, prelude::*, px, svg};

use super::menu_card;
use crate::{
    pink::{self, MONO, Pink, SANS},
    story,
};

pub const NAME: &str = "Project Row";
pub const DESCRIPTION: &str = "PINK's project picker rows as a package the app pins: a project's monogram tile, its name with the device tag and the disconnected glyph over its folder, its approvals waiting and tasks running or when it was last active, and the pinned footer's actions under a full-bleed divider";

const ICON_FOLDER: &str = "icons/sidebar/folder-with-files.svg";
const ICON_SHIELD: &str = "icons/sidebar/shield-warning.svg";
const ICON_PLUS: &str = "icons/sidebar/plus.svg";
const ICON_SETTINGS: &str = "icons/sidebar/settings.svg";

/// The story's monogram tile (PINK's `project_tile`).
fn monogram(tint: Hsla, letter: &'static str, t: &Pink) -> gpui::Div {
    div()
        .size(ui_rems(TILE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(ui_rems((TILE * 0.28).round()))
        .bg(tint.opacity(0.2))
        .text_size(ui_rems((TILE * 0.5).round()))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(t.readable_tone(tint, 1.0))
        .child(letter)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let base = |highlighted: bool| {
            menu_card::row(&menu_card::RowStyle {
                text: if highlighted {
                    t.text
                } else {
                    t.text.opacity(0.9)
                },
                fill: if highlighted {
                    t.element_active
                } else {
                    gpui::transparent_black()
                },
            })
        };
        let all = base(false)
            .child(
                div()
                    .size(ui_rems(TILE))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(ICON_FOLDER)
                            .size(ui_rems(ALL_GLYPH))
                            .text_color(t.text_muted),
                    ),
            )
            .child(
                text_column()
                    .child(name_line("All projects"))
                    .child(subtitle(
                        "Every chat on every device",
                        t.readable_faint(0.9),
                        None,
                    )),
            );
        let pink_row = base(true)
            .child(monogram(t.accent, "P", &t))
            .child(
                text_column()
                    .child(name_line("pink").child(tag("studio", t.text_muted)))
                    .child(subtitle(
                        "~/workspace/code/personal/pink",
                        t.readable_faint(0.9),
                        Some(MONO.into()),
                    )),
            )
            .child(
                counts()
                    .font_features(pink::tabular_figures())
                    .child(count(
                        svg()
                            .path(ICON_SHIELD)
                            .size(ui_rems(GLYPH))
                            .text_color(t.readable_icon(t.warning)),
                        2,
                        t.readable_tone(t.warning, 1.0),
                    ))
                    .child(count(working_mark(t.busy), 1, t.readable_tone(t.busy, 1.0))),
            );
        let site = base(false)
            .child(monogram(t.success, "S", &t))
            .child(
                text_column()
                    .child(name_line("site").child(tag("laptop", t.text_muted)))
                    .child(subtitle(
                        "~/workspace/site",
                        t.readable_faint(0.9),
                        Some(MONO.into()),
                    )),
            )
            .child(counts().child(last_active("3d", t.readable_faint(0.9))));
        let action = |path: &'static str, label: &'static str| {
            base(false)
                .child(glyph(svg().path(path), t.text_muted))
                .child(action_label(label))
        };
        div().w(px(300.0)).child(
            menu_card::card(&menu_card::CardStyle {
                border: t.border,
                background: t.surface_raised,
                text: t.text,
                font_family: SANS.into(),
                shadows: None,
            })
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(LIST_GAP))
                    .child(all)
                    .child(pink_row)
                    .child(site),
            )
            .child(divider(t.border.opacity(0.6)))
            .child(action(ICON_PLUS, "New project…"))
            .child(action(ICON_SETTINGS, "Manage projects…")),
        )
    })
}
