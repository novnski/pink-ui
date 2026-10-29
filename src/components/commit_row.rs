//! Commit Row: the `pink-commit-row` package (`components/commit-row`),
//! PINK's commit history rows as one single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. Commit History draws its rows, ref badges, author, date and
//! sha cells and Load more through it; this module adds the story.
//!
//! A row is 36 px tall in 11 pt type: the host's graph cell, then the 12 pt
//! subject with its ref badges (16 px plates in the ref's colour at 0.07,
//! a 10 px glyph, at most 112 px; past the room a `+N`), then the author
//! (a 20 px avatar or the name), the date and the short sha on a pill that
//! copies it.

#[allow(dead_code)]
mod package {
    include!("../../components/commit-row/src/lib.rs");
}
pub use package::*;

use gpui::{AnyView, App, Window, div, prelude::*, px, svg};

use crate::{
    pink::{self, Pink},
    story,
};

pub const NAME: &str = "Commit Row";
pub const DESCRIPTION: &str = "PINK's commit history rows as a package the app pins: the subject with branch, remote and tag badges and an overflow count, the author as an avatar or a name, the date, the short sha on a pill that copies it, and Load more";

const BRANCH: &str = "icons/commit-history/git-branch.svg";
const TAG: &str = "icons/commit-history/tag.svg";
const DOWN: &str = "icons/commit-history/alt-arrow-down.svg";

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        let mono = gpui::SharedString::from(pink::MONO);
        let badge = |key: &str, label: &'static str, glyph: &'static str, color: gpui::Hsla| {
            ref_badge(
                format!("commit-row-ref-{key}"),
                label,
                svg().path(glyph),
                &RefBadgeStyle {
                    fill: color.opacity(0.07),
                    text: t.readable_tone(color, 0.9),
                    glyph: t.readable_tone(color, 0.78),
                },
            )
        };
        let commit = |ix: usize, subject_text: &'static str, avatar_cell: bool, refs: bool| {
            row(("commit-row", ix), gpui::transparent_black())
                .child(div().w(px(36.)).h_full())
                .child(
                    div()
                        .flex_1()
                        .min_w(px(SUBJECT_MIN_WIDTH))
                        .overflow_hidden()
                        .child(subject_lane(1.0).child(subject(subject_text, t.text)).when(
                            refs,
                            |lane| {
                                lane.child(
                                    ref_area(180.0)
                                        .child(badge("main", "main", BRANCH, t.accent))
                                        .child(badge("tag", "v0.1.0", TAG, t.warning))
                                        .child(ref_overflow(
                                            ("commit-row-more", ix),
                                            "+2",
                                            t.text_faint,
                                        )),
                                )
                            },
                        )),
                )
                .child(
                    cells()
                        .child(if avatar_cell {
                            author_avatar_cell(72.0, 44.0, 1.0).child(
                                avatar(
                                    ("commit-row-avatar", ix),
                                    &AvatarStyle {
                                        edge: t.hairline(0.12),
                                        fill: t.wash(0.08),
                                    },
                                )
                                .child(avatar_initial(
                                    "P",
                                    pink::SANS.into(),
                                    t.text_faint,
                                )),
                            )
                        } else {
                            author_name_cell(72.0, 44.0, 1.0, "PINK QA", t.text_muted)
                        })
                        .child(date_cell(96.0, 68.0, 1.0, "Oct 20, 2026", t.text_muted))
                        .child(sha_cell(72.0, 58.0, 1.0).child(sha_pill(
                            ("commit-row-sha", ix),
                            "d5c3372",
                            mono.clone(),
                            gpui::transparent_black(),
                            t.text_muted,
                        ))),
                )
        };
        div()
            .w(px(560.))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(commit(0, "Greet from main", true, false))
            .child(commit(1, "Document the guide", true, true))
            .child(commit(2, "Probe base", false, false))
            .child(load_more_row().child(load_more(
                "commit-row-load-more",
                "Load more",
                svg().path(DOWN),
                &LoadMoreStyle {
                    fill: t.surface_raised.opacity(0.72),
                    border: t.border.opacity(0.85),
                    text: t.text_muted,
                    glyph: t.text_faint,
                    enabled: true,
                },
            )))
    })
}
