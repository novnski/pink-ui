//! The `pink-agent-row` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK's Agents list and the gallery's Agent Tree lay it out. Failure
//! modes: a row whose height drifts from `ROW_HEIGHT` scaled (the tree's
//! slots and rails assume it, so text clips or rails miss the dot); a meta
//! line that no longer starts under the title; figures that trail the
//! activity at a varying x instead of ending on one edge; a long activity
//! that pushes the figures out, or long figures that overflow a narrow row
//! instead of truncating.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::agent_row::{
        META_INDENT, PAD_X, ROW_HEIGHT, RowStyle, dot, figures, meta_line, row, status_word,
        title_line,
    },
    pink::Pink,
};

struct Host {
    ui: f32,
}

fn style(t: &Pink) -> RowStyle {
    RowStyle {
        title: t.text,
        status: t.text_muted,
        activity: t.text_muted,
        figures: t.text_muted,
    }
}

fn agent(
    width: f32,
    prefix: &'static str,
    activity: &'static str,
    figure_text: &'static str,
    t: &Pink,
) -> impl IntoElement {
    let s = style(t);
    div().w(gpui::px(width)).child(
        row()
            .debug_selector(move || format!("{prefix}-row"))
            .child(title_line(
                s.title,
                dot(t.accent),
                "Query rewriter",
                status_word(s.status, "Running"),
            ))
            .child(
                meta_line(s.activity, activity)
                    .debug_selector(move || format!("{prefix}-meta"))
                    .child(
                        figures(s.figures, figure_text)
                            .debug_selector(move || format!("{prefix}-figures")),
                    ),
            ),
    )
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let figures = "lumen-4 large, 45.8k tokens, 56s";
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(agent(420.0, "short", "Rewrite casts", figures, &t))
            .child(agent(
                420.0,
                "long",
                "Rewrite the 118 raw SQL calls that rely on implicit casts, then rerun \
                 the integration suite against a fresh database",
                figures,
                &t,
            ))
            .child(agent(
                160.0,
                "narrow",
                "Rewrite casts",
                "lumen-4 large, 45,812 in, 3,120 out, 31 tool uses, 56s",
                &t,
            ))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() < 0.05
}

#[gpui::test]
fn agent_rows_keep_their_height_columns_and_edges(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for prefix in ["short", "long", "narrow"] {
            let row = bounds(cx, &format!("{prefix}-row"));
            // Text lines snap to whole layout units: within a pixel.
            assert!(
                (f32::from(row.size.height) - ROW_HEIGHT * scale).abs() < 1.0,
                "{prefix} row is {:?} tall at {ui}, expected {}",
                row.size.height,
                ROW_HEIGHT * scale
            );
            let meta = bounds(cx, &format!("{prefix}-meta"));
            assert!(
                near(meta.origin.x - row.origin.x, PAD_X * scale),
                "{prefix} meta line inside the row's padding at {ui}"
            );
            let figures = bounds(cx, &format!("{prefix}-figures"));
            assert!(
                f32::from(figures.origin.x - meta.origin.x) >= META_INDENT * scale - 0.05,
                "{prefix} figures start after the title indent at {ui}"
            );
            assert!(
                near(
                    row.origin.x + row.size.width - (figures.origin.x + figures.size.width),
                    PAD_X * scale
                ),
                "{prefix} figures end on the row's trailing edge at {ui}"
            );
        }

        // A long activity gives way; the figures keep their width.
        let short = bounds(cx, "short-figures");
        let long = bounds(cx, "long-figures");
        assert!(
            near(long.size.width, f32::from(short.size.width)),
            "a long activity squeezes the figures at {ui}"
        );
    }
}
