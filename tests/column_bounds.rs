//! The pane's shared conversation column (Pink column_bounds.rs
//! `ColumnBounds`, composer.rs `set_available_width`, transcript.rs
//! `transcript_row_gutter`; PINK 0.31.2, D43, D44), measured from the
//! laid-out composer at Interface sizes 12 and 20, the way
//! scripts/qa-column-bounds.py measures PINK's pill at about 1200, 832 and
//! 440 pt panes and a 600 pt Conversation width. Failure modes: the pill
//! keeping its old fixed 736 pt width in a pane narrower than the column;
//! the old 16 pt composer gutter instead of the column's 24; the narrow
//! 16 pt gutter applied from 480 pt up, or not below it; the Conversation
//! width ignored; the pill off-centre in the pane; an unmeasured pane
//! getting the narrow gutter.

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*, px};
use pink_ui::{
    Theme, ThemeMode,
    components::chat_composer::ChatComposer,
    pink::layout::{self, ColumnBounds},
};

struct Host {
    pane: f32,
    reading: f32,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pane = self.pane;
        let selector = "pane";
        div().size_full().child(
            div()
                .debug_selector(move || selector.into())
                .w(px(pane))
                .flex()
                .flex_col()
                .child(
                    ChatComposer::new("cc")
                        .in_thread(true)
                        .pane(pane)
                        .reading_width(self.reading),
                ),
        )
    }
}

#[gpui::test]
fn the_pill_takes_the_panes_column(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        for (pane, reading) in [
            (1200.0, 736.0),
            (832.0, 736.0),
            (440.0, 736.0),
            (1200.0, 600.0),
        ] {
            let mut cx = cx.clone();
            cx.update(pink_ui::init);
            cx.update(|cx| {
                Theme::set_mode(ThemeMode::Light, cx);
                Theme::set_ui_font_size(ui, cx);
            });
            let (_, cx) = cx.add_window_view(|_, _| Host { pane, reading });
            cx.run_until_parked();
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
            let frame = cx.debug_bounds("pane").expect("the pane");
            let pill = cx.debug_bounds("cc-pill").expect("the pill");
            // The setting steps by 16 from 560: 600 reads as 608, in PINK too.
            let column =
                ColumnBounds::for_pane(pane, layout::normalize_conversation_width(reading));
            let left = f32::from(pill.origin.x - frame.origin.x);
            let right = f32::from(frame.right() - pill.right());
            let width = f32::from(pill.size.width);
            assert!(
                (width - column.width).abs() <= 1.0,
                "pill {width} for column {} in a {pane} pane at {ui}",
                column.width
            );
            assert!(
                (left - right).abs() <= 1.0,
                "pill off-centre ({left}, {right}) in a {pane} pane"
            );
            assert!(
                left + 1.0 >= column.gutter,
                "pill inside the {} gutter in a {pane} pane",
                column.gutter
            );
        }
    }
}

#[test]
fn the_column_matches_pinks_bounds() {
    // The default Conversation width keeps the wide pane's 736 pt pill.
    let wide = ColumnBounds::for_pane(1200.0, layout::CONVERSATION_WIDTH_DEFAULT);
    assert_eq!((wide.gutter, wide.width), (24.0, 736.0));
    assert_eq!(wide.outer_width(), 784.0);
    // A pane narrower than the column plus gutters shrinks the column.
    assert_eq!(ColumnBounds::for_pane(700.0, 736.0).width, 652.0);
    // The narrow gutter starts below 480 pt.
    assert_eq!(layout::column_gutter(480.0), 24.0);
    assert_eq!(layout::column_gutter(479.0), 16.0);
    assert_eq!(ColumnBounds::for_pane(440.0, 736.0).width, 408.0);
    // An unmeasured pane gets the wide gutter and the full reading width.
    assert_eq!(ColumnBounds::for_pane(0.0, 736.0), wide);
    // The Conversation width setting's range and steps (Pink settings.rs).
    assert_eq!(layout::normalize_conversation_width(100.0), 560.0);
    assert_eq!(layout::normalize_conversation_width(2000.0), 1200.0);
    assert_eq!(layout::normalize_conversation_width(745.0), 752.0);
    assert_eq!(layout::normalize_conversation_width(f32::NAN), 736.0);
}
