//! The `pink-diff-rows` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays out its diff rows. Failure modes: a hunk band, notice or
//! line whose height follows the Interface size (PINK computes each file's
//! body height from these fixed numbers, so folding and the sticky header
//! would drift); gutters or a marker column that no longer add up to where
//! the code starts; a split row whose halves are unequal or lose the
//! hairline between them; a note that is not indented past the columns.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::diff_rows::{
        ACCENT_BAR_WIDTH, GUTTER_WIDTH, HUNK_HEADER_HEIGHT, LINE_HEIGHT, MARKER_WIDTH,
        NOTICE_HEIGHT, SPLIT_DIVIDER_WIDTH, accent_bar, diff_style, gutter, hunk_header_row,
        marker, meta_line_row, notice_row, split_filler, split_half, split_row, unified_meta_pad,
        unified_row,
    },
    pink::Pink,
};

const WIDTH: f32 = 600.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = diff_style(&t);
        let code = || {
            div()
                .flex_1()
                .debug_selector(|| "code".into())
                .child("a + b")
        };
        div()
            .w(px(WIDTH))
            .flex()
            .flex_col()
            .debug_selector(|| "body".into())
            .child(notice_row("New file", &style).debug_selector(|| "notice".into()))
            .child(hunk_header_row("@@ -1 +1 @@", &style).debug_selector(|| "hunk".into()))
            .child(
                unified_row(Some(t.diff_add), false, &style)
                    .debug_selector(|| "line".into())
                    .child(accent_bar(None))
                    .child(gutter(Some(1), t.text, GUTTER_WIDTH, &style))
                    .child(gutter(Some(1), t.text, GUTTER_WIDTH, &style))
                    .child(marker("+", t.text, MARKER_WIDTH, &style))
                    .child(code()),
            )
            .child(
                meta_line_row("", unified_meta_pad(GUTTER_WIDTH, 12.0), &style)
                    .debug_selector(|| "meta".into())
                    .child(div().debug_selector(|| "meta-text".into()).child("x")),
            )
            .child(split_row(
                split_half(None)
                    .debug_selector(|| "left".into())
                    .into_any_element(),
                split_filler(&style)
                    .debug_selector(|| "right".into())
                    .into_any_element(),
                false,
                &style,
            ))
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
fn diff_rows_keep_fixed_heights_and_columns_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let body = bounds(cx, "body");
        let notice = bounds(cx, "notice");
        let hunk = bounds(cx, "hunk");
        let line = bounds(cx, "line");
        let code = bounds(cx, "code");
        let meta_text = bounds(cx, "meta-text");
        let left = bounds(cx, "left");
        let right = bounds(cx, "right");

        assert!(
            near(f32::from(notice.size.height), NOTICE_HEIGHT)
                && near(f32::from(hunk.size.height), HUNK_HEADER_HEIGHT)
                && near(f32::from(line.size.height), LINE_HEIGHT),
            "heights at {ui}: {notice:?} {hunk:?} {line:?}"
        );
        // The code starts after the bar, both gutters and the marker.
        assert!(
            near(
                f32::from(code.origin.x - body.origin.x),
                ACCENT_BAR_WIDTH + 2.0 * GUTTER_WIDTH + MARKER_WIDTH
            ),
            "code column at {ui}: {code:?}"
        );
        // A note clears the columns and the code's lead.
        assert!(
            near(
                f32::from(meta_text.origin.x - body.origin.x),
                unified_meta_pad(GUTTER_WIDTH, 12.0)
            ),
            "note indent at {ui}: {meta_text:?}"
        );
        // The halves split the row evenly around the hairline.
        assert!(
            near(f32::from(left.size.width), f32::from(right.size.width))
                && near(
                    f32::from(right.origin.x - (left.origin.x + left.size.width)),
                    SPLIT_DIVIDER_WIDTH
                ),
            "split halves at {ui}: {left:?} {right:?}"
        );
    }
}
