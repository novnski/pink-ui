//! The `pink-commit-row` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays out its commit history. Failure modes: a row that grows
//! with the Interface size (the list and the graph are drawn at a fixed
//! 36 px pitch, so lanes would break between rows); ref badges that grow
//! past their 112 px cap or 16 px plate; cells that ignore their column
//! width (columns would stop lining up under the header); a sha pill that
//! drifts off its 24 px plate. The column header and graph cell: a header
//! whose cells stop lining up with the row cells under them, a resize
//! divider off its column's edge, a columns button off the strip's corner,
//! and graph parts (node, HEAD's ring, the fold control) that grow with the
//! Interface size or leave the 36 px row's middle, breaking the lanes the
//! host strokes at a fixed pitch.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::commit_row::{
        AVATAR, AvatarStyle, COLUMNS_BUTTON, ColumnHeaderStyle, FOLD_GAP, FOLD_SIZE, FoldStyle,
        HEAD_RING_PADDING, HEADER_HEIGHT, NODE_RADIUS, REF_BADGE_HEIGHT, REF_BADGE_MAX_WIDTH,
        RESIZE_HANDLE, ROW_HEIGHT, RefBadgeStyle, SHA_PILL_HEIGHT, author_avatar_cell, avatar,
        cells, column_header, columns_button, commit_header, date_cell, fold_control, graph_cell,
        graph_node, head_ring, header_cell, ref_area, ref_badge, resize_handle, row, sha_cell,
        sha_pill, subject, subject_lane,
    },
};

const WIDTH: f32 = 640.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let black = gpui::black();
        div().w(px(WIDTH)).child(
            row("row", gpui::transparent_black())
                .debug_selector(|| "row".into())
                .child(div().w(px(36.0)).h_full())
                .child(
                    div().flex_1().min_w(px(80.0)).overflow_hidden().child(
                        subject_lane(1.0)
                            .child(subject("Document the guide", black))
                            .child(
                                ref_area(240.0).child(
                                    ref_badge(
                                        "ref",
                                        "feature/a-very-long-branch-name-that-overflows",
                                        gpui::svg(),
                                        &RefBadgeStyle {
                                            fill: black,
                                            text: black,
                                            glyph: black,
                                        },
                                    )
                                    .debug_selector(|| "ref".into()),
                                ),
                            ),
                    ),
                )
                .child(
                    cells()
                        .child(
                            author_avatar_cell(72.0, 44.0, 1.0)
                                .debug_selector(|| "author".into())
                                .child(
                                    avatar(
                                        "avatar",
                                        &AvatarStyle {
                                            edge: black,
                                            fill: black,
                                        },
                                    )
                                    .debug_selector(|| "avatar".into()),
                                ),
                        )
                        .child(
                            date_cell(96.0, 68.0, 1.0, "Oct 20, 2026", black)
                                .debug_selector(|| "date".into()),
                        )
                        .child(
                            sha_cell(72.0, 58.0, 1.0).child(
                                sha_pill("sha", "d5c3372", "Menlo".into(), black, black)
                                    .debug_selector(|| "sha".into()),
                            ),
                        ),
                ),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn rows_keep_their_pitch_and_columns(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let row = bounds(cx, "row");
        let badge = bounds(cx, "ref");
        let author = bounds(cx, "author");
        let avatar = bounds(cx, "avatar");
        let date = bounds(cx, "date");
        let sha = bounds(cx, "sha");

        assert!(
            near(f32::from(row.size.height), ROW_HEIGHT),
            "row at {ui}: {row:?}"
        );
        assert!(
            near(f32::from(badge.size.height), REF_BADGE_HEIGHT)
                && f32::from(badge.size.width) <= REF_BADGE_MAX_WIDTH + 0.5,
            "badge at {ui}: {badge:?}"
        );
        assert!(
            near(f32::from(author.size.width), 72.0) && near(f32::from(date.size.width), 96.0),
            "columns at {ui}: {author:?} {date:?}"
        );
        assert!(
            near(f32::from(avatar.size.width), AVATAR)
                && near(
                    f32::from(avatar.origin.x - author.origin.x),
                    (72.0 - AVATAR) / 2.0
                ),
            "avatar centred at {ui}: {avatar:?} in {author:?}"
        );
        assert!(
            near(f32::from(sha.size.height), SHA_PILL_HEIGHT),
            "sha at {ui}: {sha:?}"
        );
    }
}

const GRAPH_WIDTH: f32 = 40.0;
const NODE_X: f32 = 12.0;

struct HeaderHost {
    ui: f32,
}

impl Render for HeaderHost {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let black = gpui::black();
        div()
            .w(px(WIDTH))
            .child(
                column_header(
                    "header",
                    "header",
                    &ColumnHeaderStyle {
                        rule: black,
                        text: black,
                    },
                )
                .debug_selector(|| "header".into())
                .child(div().w(px(GRAPH_WIDTH)).flex_none())
                .child(commit_header("Commit"))
                .child(
                    cells()
                        .child(
                            header_cell("author-head", 72.0, 44.0, true)
                                .debug_selector(|| "author-head".into())
                                .child("Author")
                                .child(
                                    resize_handle("divider", black)
                                        .debug_selector(|| "divider".into()),
                                ),
                        )
                        .child(
                            header_cell("date-head", 96.0, 68.0, false)
                                .debug_selector(|| "date-head".into())
                                .child("Date"),
                        ),
                )
                .child(
                    columns_button("columns", "header", true, black)
                        .debug_selector(|| "columns".into()),
                ),
            )
            .child(
                row("row", gpui::transparent_black())
                    .child(
                        graph_cell("graph", "graph", GRAPH_WIDTH)
                            .debug_selector(|| "graph".into())
                            .child(
                                head_ring(NODE_X, NODE_RADIUS, black, black)
                                    .debug_selector(|| "ring".into()),
                            )
                            .child(
                                graph_node(NODE_X, NODE_RADIUS, black)
                                    .debug_selector(|| "node".into()),
                            )
                            .child(
                                fold_control(
                                    "fold",
                                    "graph",
                                    NODE_X,
                                    true,
                                    &FoldStyle {
                                        edge: black,
                                        fill: black,
                                    },
                                )
                                .debug_selector(|| "fold".into()),
                            ),
                    )
                    .child(div().flex_1().min_w(px(80.0)))
                    .child(
                        cells()
                            .child(
                                author_avatar_cell(72.0, 44.0, 1.0)
                                    .debug_selector(|| "author-cell".into()),
                            )
                            .child(
                                date_cell(96.0, 68.0, 1.0, "Oct 20, 2026", black)
                                    .debug_selector(|| "date-cell".into()),
                            ),
                    ),
            )
    }
}

#[gpui::test]
fn header_lines_up_and_graph_parts_keep_their_pitch(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| HeaderHost { ui });
        cx.run_until_parked();

        let header = bounds(cx, "header");
        let author_head = bounds(cx, "author-head");
        let date_head = bounds(cx, "date-head");
        let divider = bounds(cx, "divider");
        let columns = bounds(cx, "columns");
        let author_cell = bounds(cx, "author-cell");
        let date_cell = bounds(cx, "date-cell");
        let graph = bounds(cx, "graph");
        let ring = bounds(cx, "ring");
        let node = bounds(cx, "node");
        let fold = bounds(cx, "fold");

        assert!(
            near(f32::from(header.size.height), HEADER_HEIGHT),
            "header at {ui}: {header:?}"
        );
        for (head, cell) in [(author_head, author_cell), (date_head, date_cell)] {
            assert!(
                near(f32::from(head.origin.x), f32::from(cell.origin.x))
                    && near(f32::from(head.size.width), f32::from(cell.size.width)),
                "column at {ui}: {head:?} over {cell:?}"
            );
        }
        assert!(
            near(f32::from(divider.size.width), RESIZE_HANDLE)
                && near(
                    f32::from(divider.origin.x + divider.size.width / 2.0),
                    f32::from(author_head.origin.x)
                ),
            "divider at {ui}: {divider:?} on {author_head:?}"
        );
        assert!(
            near(f32::from(columns.size.width), COLUMNS_BUTTON)
                && near(
                    f32::from(
                        header.origin.x + header.size.width
                            - (columns.origin.x + columns.size.width)
                    ),
                    3.0
                ),
            "columns button at {ui}: {columns:?} in {header:?}"
        );
        let middle = f32::from(graph.origin.y) + ROW_HEIGHT / 2.0;
        assert!(
            near(f32::from(graph.size.height), ROW_HEIGHT)
                && near(f32::from(graph.size.width), GRAPH_WIDTH),
            "graph cell at {ui}: {graph:?}"
        );
        for (part, side) in [
            (node, NODE_RADIUS * 2.0),
            (ring, (NODE_RADIUS + HEAD_RING_PADDING) * 2.0),
            (fold, FOLD_SIZE),
        ] {
            assert!(
                near(f32::from(part.size.width), side)
                    && near(f32::from(part.origin.y + part.size.height / 2.0), middle),
                "graph part at {ui}: {part:?} in {graph:?}"
            );
        }
        for part in [node, ring] {
            assert!(
                near(
                    f32::from(part.origin.x + part.size.width / 2.0 - graph.origin.x),
                    NODE_X
                ),
                "centred on the lane at {ui}: {part:?}"
            );
        }
        assert!(
            near(
                f32::from(fold.origin.x - graph.origin.x),
                NODE_X + NODE_RADIUS + FOLD_GAP
            ),
            "fold after the node at {ui}: {fold:?}"
        );
    }
}
