//! Command Row: the `pink-command-row` package (`components/command-row`),
//! PINK's command palette rows as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. Each row's base is the Menu Card package's row, as in PINK;
//! this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/command-row/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, IntoElement, Window, div, prelude::*, px, svg};

use super::menu_card;
use crate::{
    motion,
    pink::{Pink, SANS},
    story,
};

pub const NAME: &str = "Command Row";
pub const DESCRIPTION: &str = "PINK's command palette rows as a package the app pins: an action's glyph, label and shortcut, an extension command's name beside its label, a matching thread's summary over its attribution, the centred show more row, the quiet section headings, the status line and the footer's key hints";

const ICON_NEW: &str = "icons/command-menu/pen-new-square.svg";
const ICON_SETTINGS: &str = "icons/command-menu/settings-minimalistic.svg";
const ICON_WIDGET: &str = "icons/command-menu/widget.svg";
const ICON_THREAD: &str = "icons/command-menu/chat-round-line.svg";

/// The story's kbd hint (PINK's `popover::kbd_hint`).
fn kbd(t: &Pink, keys: &'static str) -> gpui::Div {
    div()
        .flex_none()
        .px(ui_rems(5.0))
        .rounded(px(4.0))
        .bg(t.code_wash)
        .text_size(ui_rems(11.0))
        .text_color(t.text_muted)
        .child(keys)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let base = |key: &'static str, selected: bool| -> gpui::Div {
            if selected {
                menu_card::row(&menu_card::RowStyle {
                    text: t.text,
                    fill: t.element_active,
                })
            } else {
                let mut base = menu_card::row(&menu_card::RowStyle {
                    text: motion::hover_blend(key, t.text.opacity(0.9), t.text),
                    fill: motion::hover_blend(key, gpui::transparent_black(), t.element_hover),
                });
                base.interactivity().on_hover(motion::hover_listener(key));
                base
            }
        };
        let icon = |path: &'static str, color: Hsla| glyph(svg().path(path), color);
        let action = |key: &'static str,
                      path: &'static str,
                      text: &'static str,
                      keys: Option<&'static str>,
                      selected: bool| {
            row(base(key, selected), RowKind::Action)
                .id(key)
                .child(icon(path, t.text_muted))
                .child(label().child(text))
                .when_some(keys, |row, keys| row.child(kbd(&t, keys)))
        };
        let list = div()
            .flex()
            .flex_col()
            .child(
                slot(true, false)
                    .child(heading(t.text_faint).child("Suggested"))
                    .child(inset(action(
                        "story-command-new",
                        ICON_NEW,
                        "New chat",
                        Some("⌘N"),
                        true,
                    ))),
            )
            .child(slot(false, false).child(inset(action(
                "story-command-settings",
                ICON_SETTINGS,
                "Settings",
                Some("⌘,"),
                false,
            ))))
            .child(
                slot(false, false).child(inset(
                    row(base("story-command-extension", false), RowKind::Action)
                        .id("story-command-extension")
                        .child(icon(ICON_WIDGET, t.text_muted))
                        .child(label().child("Open the deploy board"))
                        .child(note("Release tools", t.text_faint)),
                )),
            )
            .child(
                slot(false, false)
                    .child(separator(div().h(px(1.0)).bg(t.separator)))
                    .child(heading(t.text_faint).child("Matching threads"))
                    .child(inset(
                        row(base("story-command-thread", false), RowKind::Detail)
                            .id("story-command-thread")
                            .child(icon(ICON_THREAD, t.text_muted))
                            .child(detail(
                                "Retire the expired coupon codes before Friday",
                                "pink, 3 hours ago",
                                &DetailStyle {
                                    summary: t.text,
                                    meta: t.text_faint,
                                },
                            )),
                    )),
            )
            .child(
                slot(false, true).child(inset(
                    row(base("story-command-more", false), RowKind::More)
                        .id("story-command-more")
                        .text_color(t.text_muted)
                        .child("Show more results"),
                )),
            )
            .child(status(t.text_faint).child("Searching recorded content…"));
        let footer = div()
            .px(ui_rems(16.0))
            .py(ui_rems(7.0))
            .border_t_1()
            .border_color(t.separator)
            .flex()
            .items_center()
            .gap(ui_rems(12.0))
            .child(key_hint(kbd(&t, "↑ ↓"), "Navigate", t.text_muted))
            .child(key_hint(kbd(&t, "↵"), "Select", t.text_muted))
            .child(key_hint(kbd(&t, "Esc"), "Close", t.text_muted));
        div()
            .w(px(460.0))
            .rounded(px(16.0))
            .border_1()
            .border_color(t.border)
            .bg(t.surface_raised)
            .font_family(SANS)
            .text_color(t.text)
            .flex()
            .flex_col()
            .child(list)
            .child(footer)
            .into_any_element()
    })
}
