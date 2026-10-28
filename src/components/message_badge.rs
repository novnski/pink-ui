//! Message Badge: the `pink-message-badge` package
//! (`components/message-badge`), PINK's message badge and its hover card's
//! rows as a single-GPUI package the app pins (UI rethink decision 54). The
//! gallery compiles the package's own source here, unchanged, against its
//! GPUI; PINK compiles the same file against its patched one. The kit's
//! `message_badge` draws with it; this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/message-badge/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Window, div, prelude::*, px, svg};

use crate::{
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Message Badge";
pub const DESCRIPTION: &str = "PINK's message badge as a package the app pins: the quiet pill a sent message draws in place of the context it carries, a glyph and a medium label, and the hover card's rows, each a tinted bar beside a mono location, its tag and the cited body";

const ICON_COMMENTS: &str = "icons/toaster/chat-round-line.svg";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let pill = |label: &'static str| {
            badge(&BadgeStyle {
                fill: t.ink(0.06),
                text: t.text_muted,
            })
            .child(
                svg()
                    .path(ICON_COMMENTS)
                    .size(ui_rems(BADGE_GLYPH))
                    .flex_none()
                    .text_color(t.readable_muted(0.7)),
            )
            .child(label)
        };
        let detail = DetailStyle {
            plate: t.ink(0.05),
            bar: t.solid.opacity(0.35),
            location: t.text_faint,
            tag: t.ink(0.10),
            body: t.text,
            mono: pink::MONO.into(),
        };
        let popup = t.for_popup();
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(12.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(pill("2 comments"))
                    .child(pill("1 comment")),
            )
            .child(
                card(pink::popover_card(&popup).bg(popup.tooltip_surface()))
                    .child(detail_row(
                        &detail,
                        "src/main.rs:42",
                        Some("R".into()),
                        "early-return here",
                    ))
                    .child(detail_row(
                        &detail,
                        "src/lib.rs:7",
                        Some("L".into()),
                        "why was this dropped?",
                    )),
            )
            .into_any_element()
    })
}
