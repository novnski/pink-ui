//! Composer Disc: the `pink-composer-disc` package
//! (`components/composer-disc`), PINK's composer Send, Queue, Steer and
//! Stop buttons as a single-GPUI package the app pins (UI rethink decision
//! 54). The gallery compiles the package's own source here, unchanged,
//! against its GPUI; PINK compiles the same file against its patched one.
//! This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/composer-disc/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Window, div, prelude::*, px, svg};

use crate::{pink::Pink, story};

pub const NAME: &str = "Composer Disc";
pub const DESCRIPTION: &str = "PINK's composer discs as a package the app pins: the round primary button that sends, queues or steers, solid when it can act and recessed when the draft is empty, with its arrow in the accent while the field waits for text, and the quieter Stop beside it while a run is live";

const ICON_ARROW_UP: &str = "icons/chat-composer/arrow-up.svg";
const ICON_QUEUE_SEND: &str = "icons/chat-composer/queue-send.svg";
const ICON_ARROW_UP_RIGHT: &str = "icons/chat-composer/arrow-right-up.svg";

/// The disc's diameter in the composer row, in pt at the 16 px default.
const ROW_DISC: f32 = 28.0;

fn arrow(path: &'static str, color: gpui::Hsla) -> gpui::AnyElement {
    svg()
        .path(path)
        .flex_none()
        .size(ui_rems(GLYPH))
        .text_color(color)
        .into_any_element()
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let recessed = t.text.opacity(0.08);
        let primary = |id: &str, glyph: gpui::AnyElement, fill, actionable| {
            disc(
                id.to_string(),
                glyph,
                &DiscStyle {
                    size: ui_rems(ROW_DISC),
                    fill,
                    glyph: t.bg,
                    actionable,
                    press_dim: true,
                },
            )
        };
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.0))
            .child(primary(
                "story-send",
                arrow(ICON_ARROW_UP, t.bg),
                t.text,
                true,
            ))
            .child(primary(
                "story-rest",
                arrow(ICON_ARROW_UP, t.text.opacity(0.35)),
                recessed,
                false,
            ))
            .child(primary(
                "story-ready",
                arrow(ICON_ARROW_UP, t.readable_tone(t.accent, 1.0)),
                recessed,
                false,
            ))
            .child(primary(
                "story-queue",
                arrow(ICON_QUEUE_SEND, t.bg),
                t.text,
                true,
            ))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(disc(
                        "story-stop",
                        stop_glyph(t.text).into_any_element(),
                        &DiscStyle {
                            size: ui_rems(ROW_DISC),
                            fill: t.text.opacity(0.08),
                            glyph: t.text,
                            actionable: true,
                            press_dim: true,
                        },
                    ))
                    .child(primary(
                        "story-steer",
                        arrow(ICON_ARROW_UP_RIGHT, t.bg),
                        t.text,
                        true,
                    )),
            )
    })
}
