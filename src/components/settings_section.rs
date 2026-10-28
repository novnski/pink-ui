//! Settings Section: the `pink-settings-section` package
//! (`components/settings-section`), PINK's settings section card, header and
//! rows as a single-GPUI package the app pins (UI rethink decision 54). The
//! gallery compiles the package's own source here, unchanged, against its
//! GPUI; PINK compiles the same file against its patched one. The kit's
//! `pink::section_card`, `card_row`, `row_text` and their parts draw
//! through it, so the Settings Page and every settings-style story share
//! this one source. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/settings-section/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, prelude::*, px};

use crate::{pink, pink::Pink, story};

pub const NAME: &str = "Settings Section";
pub const DESCRIPTION: &str = "PINK's settings section as a package the app pins: one rounded card per section with its title inside, rows divided by hairlines, each a wrapping title and description beside a trailing control group that drops under the text when it no longer fits";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let muted = t.readable_muted(0.85);
        let row = |first: bool, title: &'static str, copy: &'static str| {
            card_row((!first).then_some(t.border)).child(
                row_text()
                    .child(row_title(t.text, title))
                    .child(row_description(muted, copy)),
            )
        };
        div()
            .w(px(560.0))
            .flex()
            .flex_col()
            .child(
                section_card(&CardStyle {
                    border: t.border,
                    background: t.card_glass_bg(),
                })
                .mt(px(0.0))
                .child(
                    section_header().child(
                        header_text().child(header_title(t.text, "Sessions")).child(
                            row_description(muted, "How new chats start and what they keep"),
                        ),
                    ),
                )
                .child(
                    row(false, "Keep the Mac awake", "While an agent is working").child(
                        row_controls().child(pink::switch_control(
                            "settings-section-awake",
                            "Keep the Mac awake",
                            true,
                            &t,
                        )),
                    ),
                )
                .child(
                    row(
                        false,
                        "Restore open chats",
                        "Reopen the chats that were open when PINK quit",
                    )
                    .child(row_controls().child(pink::switch_control(
                        "settings-section-restore",
                        "Restore open chats",
                        false,
                        &t,
                    ))),
                ),
            )
            .child(section_footer(
                t.readable_muted(0.8),
                "Saved on this device; other devices keep their own",
            ))
    })
}
