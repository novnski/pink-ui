//! Trigger Chip: the `pink-trigger-chip` package (`components/trigger-chip`),
//! PINK's composer controls-row chip as a single-GPUI package the app pins
//! (UI rethink decision 54). The gallery compiles the package's own source
//! here, unchanged, against its GPUI; PINK compiles the same file against
//! its patched one. This module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/trigger-chip/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, SharedString, Window, div, prelude::*, px, svg};

use crate::{pink::Pink, story};

pub const NAME: &str = "Trigger Chip";
pub const DESCRIPTION: &str = "PINK's composer controls-row chip as a package the app pins: a quiet pill with an icon, the label, a badge and a muted suffix, a corner dot for a change still to apply, and a square icon-only form for a short row";

const ICON_DEEP: &str = "icons/chat-composer/brain.svg";
const ICON_BOLT: &str = "icons/chat-composer/bolt-fast.svg";
const ICON_SHIELD_CHECK: &str = "icons/chat-composer/shield-check.svg";
const ICON_UNLOCKED: &str = "icons/chat-composer/lock-keyhole-unlocked.svg";

/// One chip drawn by the package with the kit's tokens (`hovered` shows the
/// wash a pointer would fade in).
#[allow(clippy::too_many_arguments)]
pub fn chip(
    id: &str,
    glyph: &'static str,
    tint: Option<gpui::Hsla>,
    label: Option<&str>,
    suffix: Option<&str>,
    fast: bool,
    attention: bool,
    hovered: bool,
    t: &Pink,
) -> gpui::Stateful<gpui::Div> {
    let units = Units::Pixels;
    let text = if hovered { t.text } else { t.text.opacity(0.9) };
    trigger_chip(
        SharedString::from(id.to_string()),
        TriggerChipContent {
            icon: Some(
                svg()
                    .path(glyph)
                    .flex_none()
                    .size(units.len(ICON))
                    .text_color(tint.unwrap_or(t.text_muted))
                    .into_any_element(),
            ),
            attention: attention.then_some(t.warning),
            label: label.map(|l| ChipLabel::Text(SharedString::from(l.to_string()))),
            badge: fast.then(|| {
                svg()
                    .path(ICON_BOLT)
                    .flex_none()
                    .size(units.len(BADGE))
                    .text_color(text)
                    .into_any_element()
            }),
            suffix: suffix.map(|s| SharedString::from(s.to_string())),
            icon_only: label.is_none(),
        },
        &TriggerChipStyle {
            units,
            label: text,
            suffix: t.readable_muted(0.7),
            wash: if hovered {
                t.element_hover
            } else {
                gpui::transparent_black()
            },
        },
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(chip(
                "story-model",
                ICON_DEEP,
                None,
                Some("Opus 5.5"),
                Some("Medium"),
                true,
                false,
                false,
                &t,
            ))
            .child(chip(
                "story-access",
                ICON_SHIELD_CHECK,
                None,
                Some("Approve for me"),
                None,
                false,
                false,
                true,
                &t,
            ))
            .child(chip(
                "story-access-short",
                ICON_UNLOCKED,
                Some(t.warning),
                None,
                None,
                false,
                true,
                false,
                &t,
            ))
    })
}
