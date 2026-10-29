//! Commit Row: the `pink-commit-row` package (`components/commit-row`),
//! PINK's commit history rows as one single-GPUI package the app pins (UI
//! rethink decision 54). The gallery compiles the package's own source here,
//! unchanged, against its GPUI; PINK compiles the same file against its
//! patched one. Commit History draws its rows, ref badges, author, date and
//! sha cells, Load more, the column header and each row's graph cell through
//! it; this module adds the story.
//!
//! A row is 36 px tall in 11 pt type: the host's graph cell, then the 12 pt
//! subject with its ref badges (16 px plates in the ref's colour at 0.07,
//! a 10 px glyph, at most 112 px; past the room a `+N`), then the author
//! (a 20 px avatar or the name), the date and the short sha on a pill that
//! copies it. Over the rows a 24 px header names the columns (a 6 px
//! divider on each optional column's edge, a checklist button at the end
//! while pointed at); each row's graph cell draws its 3 px node, HEAD's ring
//! 2 px outside it and a branch tip's 16 px fold control.

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
pub const DESCRIPTION: &str = "PINK's commit history rows as a package the app pins: the column header, each row's graph node, HEAD ring and fold control, the subject with branch, remote and tag badges and an overflow count, the author as an avatar or a name, the date, the short sha on a pill that copies it, and Load more";

const BRANCH: &str = "icons/commit-history/git-branch.svg";
const TAG: &str = "icons/commit-history/tag.svg";
const DOWN: &str = "icons/commit-history/alt-arrow-down.svg";
const EXPAND: &str = "icons/commit-history/maximize.svg";
const COLUMNS: &str = "icons/commit-history/checklist.svg";
const GRAPH: f32 = 36.0;
const LANE: f32 = 12.0;

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
            let group = format!("commit-row-graph-{ix}");
            row(("commit-row", ix), gpui::transparent_black())
                .child(
                    graph_cell(("commit-row-graph", ix), group.clone(), GRAPH)
                        .when(ix == 0, |cell| {
                            cell.child(head_ring(LANE, NODE_RADIUS, t.accent, t.bg))
                        })
                        .child(graph_node(LANE, NODE_RADIUS, t.accent))
                        .when(refs, |cell| {
                            cell.child(
                                fold_control(
                                    ("commit-row-fold", ix),
                                    group,
                                    LANE,
                                    true,
                                    &FoldStyle {
                                        edge: t.accent.opacity(0.32),
                                        fill: t.bg.opacity(0.96),
                                    },
                                )
                                .child(fold_glyph(
                                    svg().path(EXPAND),
                                    t.readable_tone(t.accent, 0.9),
                                )),
                            )
                        }),
                )
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
            .child(
                column_header(
                    "commit-row-header",
                    "commit-row-header",
                    &ColumnHeaderStyle {
                        rule: t.hairline(0.06),
                        text: t.text_faint,
                    },
                )
                .child(div().w(px(GRAPH)).flex_none())
                .child(commit_header("Commit"))
                .child(
                    cells()
                        .child(
                            header_cell("commit-row-author-head", 72.0, 44.0, true).child("Author"),
                        )
                        .child(header_cell("commit-row-date-head", 96.0, 68.0, false).child("Date"))
                        .child(header_cell("commit-row-sha-head", 72.0, 58.0, false).child("SHA")),
                )
                .child(
                    columns_button(
                        "commit-row-columns",
                        "commit-row-header",
                        true,
                        gpui::transparent_black(),
                    )
                    .child(columns_glyph(svg().path(COLUMNS), t.text_muted)),
                ),
            )
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
