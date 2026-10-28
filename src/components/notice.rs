//! Notice: the `pink-notice` package (`components/notice`), PINK's
//! in-context notice as a single-GPUI package the app pins (UI rethink
//! decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. The Chat Composer's notice and the kit's `notice_action`
//! draw with it; this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/notice/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Hsla, IntoElement, Role, SharedString, Window, div, prelude::*, px, svg};

use crate::{
    HoverFill,
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Notice";
pub const DESCRIPTION: &str = "PINK's in-context notice as a package the app pins: an opaque plate in the danger or warning tone on the stack ladder's notice step, a title row with the tone glyph, the medium title, capsule actions and a Copy message button that wraps under the title in a narrow pane, and the message on its own line";

const ICON_DANGER: &str = "icons/chat-composer/danger-triangle.svg";
const ICON_COPY: &str = "icons/chat-composer/copy.svg";

/// One notice as PINK's composer draws it (notice.rs `notice_chip`).
fn notice(
    key: &'static str,
    warning: bool,
    title_text: &'static str,
    message_text: &'static str,
    action_label: &'static str,
    t: &Pink,
) -> impl IntoElement {
    let (accent, muted) = if warning {
        (t.warning, t.warning_muted)
    } else {
        (t.danger, t.danger_muted)
    };
    let ink: Hsla = t.readable_tone(muted, 0.9);
    let copy_key = format!("{key}-copy");
    plate(&NoticeStyle {
        plate: pink::flatten(accent.opacity(0.08), t.canvas()),
        edge: accent.opacity(0.22),
        ink,
    })
    .child(
        title_row()
            .child(
                svg()
                    .path(ICON_DANGER)
                    .size(ui_rems(GLYPH))
                    .flex_none()
                    .text_color(ink),
            )
            .child(title(title_text))
            .child(spacer())
            .child(
                pink::notice_action(format!("{key}-action"), t)
                    .aria_label(action_label)
                    .child(action_label),
            )
            .child(
                copy_button()
                    .id(SharedString::from(copy_key.clone()))
                    .role(Role::Button)
                    .aria_label("Copy message")
                    .hover_fill(copy_key, accent.opacity(0.0), accent.opacity(0.12))
                    .child(
                        svg()
                            .path(ICON_COPY)
                            .size(ui_rems(COPY_GLYPH))
                            .text_color(t.readable_tone(muted, 0.8)),
                    ),
            ),
    )
    .child(
        message(message_text)
            .id(SharedString::from(format!("{key}-message")))
            .role(Role::Alert)
            .aria_label(SharedString::from(format!("{title_text}: {message_text}"))),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(div().w(px(520.0)).child(notice(
                "story-notice-failed",
                false,
                "Action failed",
                "The worktree has uncommitted changes. Commit or stash them, then retry.",
                "Dismiss",
                &t,
            )))
            .child(div().w(px(520.0)).child(notice(
                "story-notice-draft",
                true,
                "Draft not saved",
                "The draft couldn't be written to disk.",
                "Retry",
                &t,
            )))
            .child(div().w(px(240.0)).child(notice(
                "story-notice-narrow",
                false,
                "Can't attach",
                "notes.pdf isn't an image. Only images can be attached for now.",
                "Dismiss",
                &t,
            )))
            .into_any_element()
    })
}
