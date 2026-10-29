//! The `pink-commit-row` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays out its commit history. Failure modes: a row that grows
//! with the Interface size (the list and the graph are drawn at a fixed
//! 36 px pitch, so lanes would break between rows); ref badges that grow
//! past their 112 px cap or 16 px plate; cells that ignore their column
//! width (columns would stop lining up under the header); a sha pill that
//! drifts off its 24 px plate.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::commit_row::{
        AVATAR, AvatarStyle, REF_BADGE_HEIGHT, REF_BADGE_MAX_WIDTH, ROW_HEIGHT, RefBadgeStyle,
        SHA_PILL_HEIGHT, author_avatar_cell, avatar, cells, date_cell, ref_area, ref_badge, row,
        sha_cell, sha_pill, subject, subject_lane,
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
