//! Place Tab: the `pink-place-tab` package (`components/place-tab`),
//! PINK's right-pane place tab row as a single-GPUI package the app pins
//! (UI rethink decision 54). The gallery compiles the package's own source
//! here, unchanged, against its GPUI; PINK compiles the same file against
//! its patched one. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/place-tab/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, IntoElement, Window, div, prelude::*, px, svg};

use crate::{
    HoverFill, motion,
    pink::{FillTier, Pink},
    story,
};

pub const NAME: &str = "Place Tab";
pub const DESCRIPTION: &str = "PINK's right-pane place tabs as a package the app pins: a 36 pt row of fixed 112 pt tabs whose glyph turns into the close control under the pointer, a truncating title and the unsaved dot, the Agents place's pinned home tab, the add button and the fades over a scrolled strip";

const ICON_TERMINAL: &str = "icons/terminal/terminal.svg";
const ICON_DOCUMENT: &str = "icons/command-menu/document.svg";
const ICON_BOT: &str = "icons/tab-bar/bot.svg";
const ICON_CLOSE: &str = "icons/tab-bar/close.svg";
const ICON_PLUS: &str = "icons/tab-bar/plus.svg";

fn glyph(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg().path(path).size(px(size)).text_color(color)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let place_tab = |key: &'static str,
                         mark: &'static str,
                         label: &'static str,
                         active: bool,
                         dirty: bool| {
            let tab = tab(&TabStyle {
                fill: active.then(|| t.fill(FillTier::Secondary)),
            })
            .id(key)
            .group(key);
            let tab = if active {
                tab
            } else {
                tab.hover_wash(key, t.fill(FillTier::Tertiary))
            };
            tab.child(
                slot()
                    .child(glyph_layer(key).child(glyph(
                        mark,
                        GLYPH,
                        if active {
                            t.text_muted
                        } else {
                            t.readable_muted(0.7)
                        },
                    )))
                    .child(close_layer(key).child(glyph(ICON_CLOSE, GLYPH, t.text_muted))),
            )
            .child(title(label, if active { t.text } else { t.text_muted }))
            .when(dirty, |tab| tab.child(dirty_dot(t.text_muted)))
        };
        let home = home_tab(&HomeStyle {
            text: t.text_muted,
            fill: None,
        })
        .id("story-place-home")
        .hover_wash("story-place-home", t.fill(FillTier::Tertiary))
        .child(glyph(ICON_BOT, HOME_GLYPH, t.text_muted))
        .child("All agents");
        let add_fade = "story-place-add";
        let add = add_button(motion::hover_blend(
            add_fade,
            gpui::transparent_black(),
            t.fill(FillTier::Secondary),
        ))
        .id(add_fade)
        .on_hover(motion::hover_listener(add_fade))
        .child(glyph(ICON_PLUS, ADD_GLYPH, t.text_muted));
        let region = div()
            .relative()
            .min_w_0()
            .size_full()
            .flex()
            .items_center()
            .child(
                strip()
                    .child(place_tab(
                        "story-place-zsh",
                        ICON_TERMINAL,
                        "zsh",
                        true,
                        false,
                    ))
                    .child(place_tab(
                        "story-place-build",
                        ICON_TERMINAL,
                        "cargo build --locked",
                        false,
                        false,
                    ))
                    .child(place_tab(
                        "story-place-notes",
                        ICON_DOCUMENT,
                        "release-notes.md",
                        false,
                        true,
                    )),
            )
            .child(edge_fade(Edge::Right, t.surface));
        div()
            .w(px(460.0))
            .bg(t.surface)
            .child(
                bar()
                    .child(home)
                    .child(div().flex_1().min_w_0().h_full().child(region))
                    .child(add),
            )
            .into_any_element()
    })
}
