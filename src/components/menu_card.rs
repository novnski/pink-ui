//! Menu Card: the `pink-menu-card` package (`components/menu-card`),
//! PINK's floating card, its rows and the chrome around them as a
//! single-GPUI package the app pins (UI rethink decision 54). The gallery
//! compiles the package's own source here, unchanged, against its GPUI;
//! PINK compiles the same file against its patched one. The kit's
//! `pink::menu_heading`, `menu_separator`, `menu_section`,
//! `search_input_frame`, `palette_card`, `kbd_hint`, `key_cap` and
//! `key_hint*` draw with it; this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/menu-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, prelude::*, px};

use crate::{
    pink::{FillTier, Pink},
    story,
};

pub const NAME: &str = "Menu Card";
pub const DESCRIPTION: &str = "PINK's floating card as a package the app pins: the bordered, lifted surface every menu, picker and popover opens on, the one row shape its items share, concentric with the card, washed when selected and a step lighter under the pointer, and the chrome around them: the search field, small tracked headings, edge-to-edge rules, a trailing section, accelerator chips and the key legends in a picker's footer. Below it, the completion popup stacked above the composer, its rows rounded to the popup's larger corner";

const ICON_ARROW_UP: &str = "icons/combobox/arrow-up.svg";
const ICON_ARROW_DOWN: &str = "icons/combobox/arrow-down.svg";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let popup = t.for_popup();
        let keys = KeyHintStyle {
            cap: t.fill(FillTier::Quaternary),
            text: popup.text_muted,
            divider: t.separator(false),
            mono: crate::pink::MONO.into(),
        };
        let item = |label: &'static str, fill: gpui::Hsla, text: gpui::Hsla| {
            row(&RowStyle { text, fill }).child(div().flex_1().child(label))
        };
        let completion = |label: &'static str, cursor: bool| {
            let (fill, text) = if cursor {
                (t.element_active, t.text)
            } else {
                (gpui::transparent_black(), t.text.opacity(0.9))
            };
            stack_row(&RowStyle { text, fill }).child(div().flex_1().child(label))
        };
        let completions = crate::components::overlay_card::stack_popup(
            card(&CardStyle {
                border: t.border,
                background: t.surface_raised,
                text: t.text,
                font_family: crate::pink::SANS.into(),
                shadows: None,
            })
            .flex()
            .flex_col(),
        )
        .child(completion("/compact", true))
        .child(completion("/clear", false))
        .child(completion("/model", false));
        let menu = div().w(px(260.0)).child(
            card(&CardStyle {
                border: t.border,
                background: t.surface_raised,
                text: t.text,
                font_family: crate::pink::SANS.into(),
                shadows: None,
            })
            .flex()
            .flex_col()
            .child(
                search_frame(t.fill(FillTier::Quaternary))
                    .text_color(popup.text_faint)
                    .child("Search models"),
            )
            .child(heading(popup.text_muted, "Anthropic"))
            .child(item("Sonnet 5.5", t.element_active, t.text).child(kbd_hint(&keys, "⌘1")))
            .child(item("Opus 5.5", t.element_hover, t.text).child(kbd_hint(&keys, "⌘2")))
            .child(separator(t.separator(false)))
            .child(heading(popup.text_muted, "Local"))
            .child(item(
                "Haiku 5.5",
                gpui::transparent_black(),
                t.text.opacity(0.9),
            ))
            .child(section(t.separator(false)).child(item(
                "Manage models",
                gpui::transparent_black(),
                t.text.opacity(0.9),
            )))
            .child(
                div()
                    .pt(px(6.0))
                    .px(px(4.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.0))
                    .child(key_hint_pair(
                        &keys,
                        ICON_ARROW_UP,
                        ICON_ARROW_DOWN,
                        "Navigate",
                    ))
                    .child(key_hint_text(&keys, "esc", "Close")),
            ),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(menu)
            .child(div().w(px(360.0)).child(completions))
    })
}
