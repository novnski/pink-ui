//! The `pink-project-row` package's layout at Interface sizes 12, 16 and 20,
//! as PINK lays its project picker rows out. Failure modes: a long device
//! tag that pushes the project's name out of its line (the tag must stop at
//! half the line); counts that shrink and clip instead of holding their
//! width; a footer divider that stops short of the card's edges.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::project_row::{
        DIVIDER_BLEED, TAG_MAX_SHARE, count, counts, divider, name_line, subtitle, tag, text_column,
    },
    pink::Pink,
};

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div()
            .debug_selector(|| "card".into())
            .w(gpui::px(260.0))
            .p(gpui::px(DIVIDER_BLEED))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(
                        text_column()
                            .child(
                                name_line("pink")
                                    .debug_selector(|| "name-line".into())
                                    .child(
                                        tag(
                                            "a very long device name that would fill the line",
                                            t.text_muted,
                                        )
                                        .debug_selector(|| "tag".into()),
                                    ),
                            )
                            .child(subtitle("~/workspace/pink", t.text_faint, None)),
                    )
                    .child(counts().debug_selector(|| "counts".into()).child(count(
                        div().size(gpui::px(6.0)),
                        12,
                        t.warning,
                    ))),
            )
            .child(divider(t.border).debug_selector(|| "divider".into()))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn project_rows_hold_their_name_counts_and_divider(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let card = bounds(cx, "card");
        let line = bounds(cx, "name-line");
        let tag = bounds(cx, "tag");
        let counts = bounds(cx, "counts");
        let divider = bounds(cx, "divider");

        assert!(
            f32::from(tag.size.width) <= f32::from(line.size.width) * TAG_MAX_SHARE + 0.5,
            "tag at {ui}: {tag:?} in {line:?}"
        );
        assert!(
            f32::from(counts.size.width) > 0.0 && counts.right() <= card.right(),
            "counts at {ui}: {counts:?}"
        );
        assert!(
            (f32::from(divider.origin.x - card.origin.x)).abs() <= 0.5
                && (f32::from(card.right() - divider.right())).abs() <= 0.5,
            "divider at {ui}: {divider:?} in {card:?}"
        );
    }
}
