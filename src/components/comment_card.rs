//! Comment Card: the `pink-comment-card` package (`components/comment-card`),
//! PINK's review comment cards as a single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. Diff Viewer, Comment Thread and Comments Panel draw their
//! cards, drafts and adder with it; this module adds only the story.

#[allow(dead_code)]
mod package {
    include!("../../components/comment-card/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, IntoElement, Role, SharedString, Window, div, prelude::*, px, svg};

use crate::{
    motion,
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Comment Card";
pub const DESCRIPTION: &str = "PINK's review comment cards as a package the app pins: the 16px plus a line shows to start a note, the posted card (a flat band with a 3px bar, the cited path:line in mono, edit and remove on hover, and the note), the same card centred under a document column, the fixed-height draft with Cancel and Comment, and the code editor's floating card";

/// The comment glyph (PINK `icons::CHAT_ROUND_LINE`).
pub const ICON_COMMENT: &str = "icons/comment-card/chat-round-line.svg";
/// Remove (PINK `icons::CLOSE_CIRCLE`).
pub const ICON_REMOVE: &str = "icons/comment-card/close-circle.svg";
/// Edit (PINK `icons::PEN`).
pub const ICON_PEN: &str = "icons/comment-card/pen.svg";
/// The adder's plus (PINK `icons::PLUS`).
pub const ICON_PLUS: &str = "icons/comment-card/plus.svg";

/// A posted band's paint (PINK `render_comment_card`).
pub fn posted_style(t: &Pink) -> BandStyle {
    BandStyle {
        background: t.ink(0.05),
        accent: t.solid.opacity(0.35),
    }
}

/// The open draft's paint (PINK `render_comment_draft`).
pub fn draft_style(t: &Pink) -> BandStyle {
    BandStyle {
        background: t.ink(0.08),
        accent: t.solid.opacity(0.7),
    }
}

/// The comment glyph and the mono location heading a card.
pub fn heading(location_text: impl Into<SharedString>, t: &Pink) -> gpui::Div {
    header(svg().path(ICON_COMMENT), t.text_faint).child(location(
        location_text,
        t.text_faint,
        pink::MONO,
    ))
}

/// A draft action's paint: the `solid` plate, or the ghost blended over its
/// hover fade under `key` (pair with `motion::hover_listener(key)`).
pub fn action_style(key: &str, primary: bool, t: &Pink) -> ActionStyle {
    if primary {
        ActionStyle {
            background: t.solid,
            text: t.on_solid,
        }
    } else {
        ActionStyle {
            background: motion::hover_blend(key, t.element_hover.opacity(0.0), t.element_hover),
            text: motion::hover_blend(key, t.text_muted, t.text),
        }
    }
}

/// One posted card as PINK draws it, with edit and remove under its group.
fn posted(
    key: &'static str,
    at: &'static str,
    note: &'static str,
    column: Option<Column>,
    t: &Pink,
) -> impl IntoElement {
    let group = SharedString::from(format!("{key}-group"));
    let lines = note.lines().count().max(1);
    card(&posted_style(t), card_height(lines, 16.0), column)
        .id(key)
        .group(group.clone())
        .child(
            content(column)
                .child(
                    heading(at, t)
                        .child(
                            edit(svg().path(ICON_PEN), t.text_muted, group.clone())
                                .id(SharedString::from(format!("{key}-edit")))
                                .role(Role::Button)
                                .aria_label("Edit comment"),
                        )
                        .child(
                            remove(svg().path(ICON_REMOVE), t.text_muted, group)
                                .id(SharedString::from(format!("{key}-remove")))
                                .role(Role::Button)
                                .aria_label("Remove comment"),
                        ),
                )
                .child(body(note, t.text_dim)),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let cancel = "story-comment-card-cancel";
        div()
            .w(px(520.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(12.0))
                    .text_color(t.text_muted)
                    .child(
                        adder(t.solid, svg().path(ICON_PLUS), t.on_solid)
                            .id("story-comment-card-adder")
                            .role(Role::Button)
                            .aria_label("Add comment"),
                    )
                    .child("A line's adder starts a draft"),
            )
            .child(posted(
                "story-comment-card-posted",
                "src/lib.rs:3",
                "Say what saturation means for callers.",
                None,
                &t,
            ))
            .child(posted(
                "story-comment-card-column",
                "docs/guide.md:3",
                "Say what the probe checks.\nThe second line wraps under the first.",
                Some(Column {
                    max_width: 360.0,
                    gutter: 24.0,
                }),
                &t,
            ))
            .child(
                draft(&draft_style(&t), None).child(
                    content(None)
                        .child(heading("src/lib.rs:5", &t))
                        .child(field(
                            div()
                                .text_color(t.text_faint)
                                .child("Leave a note for the agent"),
                        ))
                        .child(
                            actions()
                                .child(
                                    action(&action_style(cancel, false, &t), "Cancel")
                                        .id(cancel)
                                        .role(Role::Button)
                                        .on_hover(motion::hover_listener(cancel)),
                                )
                                .child(
                                    action(&action_style("", true, &t), "Comment")
                                        .id("story-comment-card-commit")
                                        .role(Role::Button),
                                ),
                        ),
                ),
            )
            .child(
                floating(pink::popover_card_flush(&t))
                    .w(px(360.0))
                    .h(px(card_height(1, 16.0)))
                    .child(heading("src/main.rs:2", &t))
                    .child(body("The code editor floats the same card.", t.text_dim)),
            )
    })
}
