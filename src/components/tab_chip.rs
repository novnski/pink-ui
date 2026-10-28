//! Tab Chip: the `pink-tab-chip` package (`components/tab-chip`), PINK's
//! titlebar tab as a single-GPUI package the app pins (UI rethink decision
//! 54). The gallery compiles the package's own source here, unchanged,
//! against its GPUI; PINK compiles the same file against its patched one.
//! The Tab Bar's strips draw their tabs with it; this module adds only the
//! story.

#[allow(dead_code)]
mod package {
    include!("../../components/tab-chip/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, IntoElement, Window, div, prelude::*, px, svg};

use super::tab_bar::{on_tab_badge, role_icon_color, selected_fill, tab_title_color};
use crate::{
    HoverFill,
    pink::{self, FillTier, Pink},
    story,
};

pub const NAME: &str = "Tab Chip";
pub const DESCRIPTION: &str = "PINK's titlebar tab as a package the app pins: a 28 pt chip with the agent's mark, a Main badge or the role glyph, a truncating title, the status and a close slot that waits under the pointer, and the same slot as a lone chat's semibold title with its menu chevron";

const ICON_HOME: &str = "icons/tab-bar/home.svg";
const ICON_FORK: &str = "icons/tab-bar/arrow-fork.svg";
const ICON_BOT: &str = "icons/tab-bar/bot.svg";
const ICON_CLOSE: &str = "icons/tab-bar/close.svg";
const ICON_CLAUDE: &str = "icons/tab-bar/claude-mark.svg";
const ICON_CHEVRON: &str = "icons/sidebar/alt-arrow-down.svg";
/// Pink: icons.rs `claude_brand`.
const CLAUDE_BRAND: u32 = 0xD97757;

fn glyph(path: &'static str, size: f32, color: Hsla) -> gpui::Svg {
    svg()
        .path(path)
        .size(px(size))
        .flex_none()
        .text_color(color)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let claude: Hsla = gpui::rgb(CLAUDE_BRAND).into();
        let main = tab(&TabStyle {
            label: Label::Tab,
            text: tab_title_color(&t, false, true),
            fill: None,
            closable: false,
            min_width: TAB_MIN_WIDTH,
            max_width: TAB_MAX_WIDTH,
        })
        .id("story-tab-main")
        .hover_wash("story-tab-main", t.fill(FillTier::Tertiary))
        .child(glyph(ICON_CLAUDE, HARNESS_MARK, claude))
        .child(
            badge(&BadgeStyle {
                fill: t.accent_wash,
                text: on_tab_badge(&t, pink::TEXT_CONTRAST_TARGET),
            })
            .child(glyph(
                ICON_HOME,
                BADGE_GLYPH,
                on_tab_badge(&t, pink::NON_TEXT_CONTRAST_TARGET),
            ))
            .child("Main"),
        )
        .child(title("Retire expired coupon codes"));
        let active = tab(&TabStyle {
            label: Label::Tab,
            text: tab_title_color(&t, true, true),
            fill: Some(selected_fill(&t, true)),
            closable: true,
            min_width: TAB_MIN_WIDTH,
            max_width: TAB_MAX_WIDTH,
        })
        .id("story-tab-active")
        .group("story-tab-active")
        .child(glyph(ICON_CLAUDE, HARNESS_MARK, claude))
        .child(glyph(ICON_FORK, ROLE_GLYPH, role_icon_color(&t, true)))
        .child(title("Write the migration tests"))
        .child(close_slot("story-tab-active", true).child(glyph(
            ICON_CLOSE,
            CLOSE_GLYPH,
            t.text_muted,
        )));
        let unread = tab(&TabStyle {
            label: Label::Tab,
            text: tab_title_color(&t, false, true),
            fill: None,
            closable: true,
            min_width: TAB_MIN_WIDTH,
            max_width: TAB_MAX_WIDTH,
        })
        .id("story-tab-unread")
        .group("story-tab-unread")
        .hover_wash("story-tab-unread", t.fill(FillTier::Tertiary))
        .child(glyph(ICON_CLAUDE, HARNESS_MARK, claude))
        .child(glyph(ICON_BOT, ROLE_GLYPH, role_icon_color(&t, false)))
        .child(title("Audit the webhook retries"))
        .child(unread_dot(super::tab_bar::unread_dot(&t)))
        .child(close_slot("story-tab-unread", false).child(glyph(
            ICON_CLOSE,
            CLOSE_GLYPH,
            t.text_muted,
        )));
        let lone = tab(&TabStyle {
            label: Label::Title,
            text: t.text,
            fill: None,
            closable: false,
            min_width: TAB_MIN_WIDTH,
            max_width: 4.0 * TAB_MAX_WIDTH,
        })
        .id("story-title")
        .hover_wash("story-title", t.fill(FillTier::Tertiary))
        .child(glyph(ICON_CLAUDE, HARNESS_MARK, claude))
        .child(title("Retire expired coupon codes"))
        .child(
            title_chevron(t.text_muted)
                .child(glyph(ICON_CHEVRON, CHEVRON_GLYPH, t.text_muted))
                .child(div().text_size(ui_rems(12.0)).child("3")),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .child(main)
                    .child(active)
                    .child(unread),
            )
            .child(lone)
            .into_any_element()
    })
}
