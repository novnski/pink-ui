//! The `pink-integration-picker` package at Interface sizes 12, 16 and 20, as
//! PINK's Settings, Extensions page and the gallery's Integration Picker draw
//! with it. Failure modes: a search field, tile or review that stops scaling
//! with the Interface size (fixed pixels clip at 20 or float at 12); a long
//! permission label that pushes its switch out of the review instead of
//! wrapping; a search that hides a section's rows by the wrong field, puts an
//! available row under Connected, or keeps an empty section's heading.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::integration_picker::{
        REVIEW_W, SEARCH_H, SEARCH_W, SearchStyle, TILE, TileStyle, arrange, matches, monogram,
        monogram_tile, permission_detail, permission_list, permission_row, search_field,
    },
    pink::Pink,
};

const LONG: &str =
    "Read and write every file in the task's checkout, including ignored and generated files";

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let search = SearchStyle {
            fill: t.ink(0.035),
            glyph: t.text_faint,
        };
        let tile = TileStyle {
            border: t.border,
            background: t.ink(0.045),
            text: t.text_muted,
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                search_field(
                    &search,
                    "icons/integration-picker/magnifer.svg",
                    div().child("Search extensions"),
                )
                .debug_selector(|| "search".to_owned()),
            )
            .child(monogram_tile(&tile, "Notes").debug_selector(|| "tile".to_owned()))
            .child(
                div()
                    .w(pink_ui::components::integration_picker::ui_rems(REVIEW_W))
                    .debug_selector(|| "review".to_owned())
                    .child(
                        permission_list().child(permission_row(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(LONG)
                                .debug_selector(|| "permission-text".to_owned()),
                            div()
                                .flex_none()
                                .w(gpui::px(32.0))
                                .h(gpui::px(18.0))
                                .debug_selector(|| "switch".to_owned()),
                        )),
                    ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn picker_pieces_scale_with_the_interface_and_keep_switches_in_the_review(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let search = bounds(cx, "search");
        assert!(
            (f32::from(search.size.width) - SEARCH_W * scale).abs() < 0.5
                && (f32::from(search.size.height) - SEARCH_H * scale).abs() < 0.5,
            "search is {:?} at {ui}",
            search.size
        );
        let tile = bounds(cx, "tile");
        assert!(
            (f32::from(tile.size.width) - TILE * scale).abs() < 0.5,
            "tile is {:?} at {ui}",
            tile.size
        );
        let review = bounds(cx, "review");
        assert!((f32::from(review.size.width) - REVIEW_W * scale).abs() < 0.5);
        // A long permission wraps; its switch stays inside the review.
        let switch = bounds(cx, "switch");
        assert!(
            switch.origin.x + switch.size.width
                <= review.origin.x + review.size.width + gpui::px(0.5),
            "the switch left the review at {ui}: {switch:?} in {review:?}"
        );
        let text = bounds(cx, "permission-text");
        assert!(
            f32::from(text.size.height) > 12.0 * scale * 1.5,
            "a long permission did not wrap at {ui}: {:?}",
            text.size
        );
    }
}

#[test]
fn search_and_sections_follow_the_rows_not_their_headings() {
    // (name, category, connected)
    let rows = [
        ("Notes", "Data only", true),
        ("Linter", "Sandboxed code", false),
        ("Shell", "Native code", false),
    ];

    let all = arrange(&rows, "", |row| row.2, |row| vec![row.0, row.1]);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].0.title(), "Connected");
    assert_eq!(
        all[0].1.iter().map(|row| row.0).collect::<Vec<_>>(),
        ["Notes"]
    );
    assert_eq!(all[1].0.title(), "Available");
    assert_eq!(
        all[1].1.iter().map(|row| row.0).collect::<Vec<_>>(),
        ["Linter", "Shell"]
    );

    // A category match keeps only its section; the empty Connected goes.
    let native = arrange(&rows, "  NATIVE ", |row| row.2, |row| vec![row.0, row.1]);
    assert_eq!(native.len(), 1);
    assert_eq!(native[0].0.title(), "Available");
    assert_eq!(native[0].1[0].0, "Shell");

    assert!(
        arrange(
            &rows,
            "nothing like it",
            |row| row.2,
            |row| vec![row.0, row.1]
        )
        .is_empty()
    );
    assert!(matches("", &[]));
    assert!(!matches("x", &[]));
    assert_eq!(monogram("  notes"), "N");
    assert_eq!(monogram("…"), "");
    assert_eq!(permission_detail("Network", true), "Network, required");
    assert_eq!(permission_detail("", true), "Required");
    assert_eq!(permission_detail("Network", false), "Network");
}
