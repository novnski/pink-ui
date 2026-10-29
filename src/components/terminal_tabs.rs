//! Terminal Tabs: the `pink-terminal-tabs` package
//! (`components/terminal-tabs`), PINK's terminal tab strip as a
//! single-GPUI package the app pins (UI rethink decision 54). The gallery
//! compiles the package's own source here, unchanged, against its GPUI;
//! PINK compiles the same file against its patched one. The Terminal
//! drawer's tab strip draws through it. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/terminal-tabs/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, SharedString, Window, div, prelude::*, px};

use crate::{
    motion,
    pink::{FillTier, Pink},
    story,
};

pub const NAME: &str = "Terminal Tabs";
pub const DESCRIPTION: &str = "PINK's terminal tab strip as a package the app pins: a 38 pt band of fixed 118 pt session tabs with the terminal glyph, a truncating title and the close control on the selected tab, a dimmed tab for a session that exited, the add button and the collapse chevron pinned right";

pub const ICON_TERMINAL: &str = "icons/terminal/terminal.svg";
pub const ICON_CLOSE: &str = "icons/terminal/close.svg";
pub const ICON_PLUS: &str = "icons/terminal/plus.svg";
pub const ICON_DOWN: &str = "icons/terminal/alt-arrow-down.svg";

/// The host's resolved tab colours (Pink `render_tab_bar`): the selected
/// tab on an ink(0.08) wash in `text`, a resting one clear in
/// `readable_muted(0.6)`, both easing to `element_hover` under the pointer;
/// the glyph at 0.8 / 0.6 of the label on a dark appearance.
pub fn tab_style(key: &str, selected: bool, exited: bool, t: &Pink) -> TabStyle {
    let (text, rest, glyph_alpha) = if selected {
        (t.text, t.ink(0.08), 0.8)
    } else {
        (t.readable_muted(0.6), gpui::transparent_black(), 0.6)
    };
    TabStyle {
        fill: motion::hover_blend(key, rest, t.element_hover),
        text,
        glyph: if t.dark {
            text.opacity(glyph_alpha)
        } else {
            text
        },
        opacity: exited.then_some(if t.dark {
            EXITED_OPACITY_DARK
        } else {
            EXITED_OPACITY_LIGHT
        }),
    }
}

/// The host's resolved close colours: clear easing to the secondary fill,
/// the glyph in `readable_muted(0.8)`.
pub fn close_style(key: &str, visible: bool, t: &Pink) -> CloseStyle {
    CloseStyle {
        fill: motion::hover_blend(key, gpui::transparent_black(), t.fill(FillTier::Secondary)),
        glyph: t.readable_muted(0.8),
        visible,
    }
}

/// The host's resolved add and collapse colours: clear easing to
/// ink(0.05), the glyph in `readable_muted(alpha)`.
pub fn button_style(key: &str, glyph_alpha: f32, t: &Pink) -> ButtonStyle {
    ButtonStyle {
        fill: motion::hover_blend(key, gpui::transparent_black(), t.ink(0.05)),
        glyph: t.readable_muted(glyph_alpha),
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let session = |ix: usize, title: &'static str, selected: bool, exited: bool| {
            let key = format!("story-term-tab-{ix}");
            let close_key = format!("story-term-close-{ix}");
            let close = close_button(&close_style(&close_key, selected, &t), ICON_CLOSE)
                .id(SharedString::from(close_key.clone()))
                .on_hover(motion::hover_listener(close_key));
            tab(
                &tab_style(&key, selected, exited, &t),
                ICON_TERMINAL,
                title,
                close,
            )
            .id(SharedString::from(key.clone()))
            .on_hover(motion::hover_listener(key))
        };
        let add = add_button(&button_style("story-term-add", 0.6, &t), ICON_PLUS)
            .id("story-term-add")
            .on_hover(motion::hover_listener("story-term-add"));
        let collapse = collapse_button(&button_style("story-term-collapse", 0.55, &t), ICON_DOWN)
            .id("story-term-collapse")
            .on_hover(motion::hover_listener("story-term-collapse"));
        div()
            .w(px(560.0))
            .bg(t.surface)
            .child(
                bar(t.hairline(0.07))
                    .child(session(0, "~/code/pink", true, false))
                    .child(session(1, "cargo build --locked", false, false))
                    .child(session(2, "Terminal 3", false, true))
                    .child(add)
                    .child(spacer())
                    .child(collapse),
            )
            .child(div().p(px(12.0)).child(ghost(
                &GhostStyle {
                    fill: t.surface_raised,
                    edge: t.border_strong,
                    text: t.text,
                },
                "cargo build --locked",
            )))
            .into_any_element()
    })
}
