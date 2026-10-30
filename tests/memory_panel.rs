//! The `pink-memory-panel` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK's Memory card and the gallery's Memory Panel lay it out.
//! Failure modes: a row whose height drifts from `ROW_H` scaled (the fold
//! animation and the list's rhythm assume it); a long memory that pushes the
//! corner actions out of the row instead of fading under them; corner actions
//! that stop being `ACTION` square hit targets; a meta line that no longer
//! starts on the title's edge; a heading off `HEADING_H`.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::memory_panel::{
        ACTION, HEADING_H, ROW_H, ROW_PAD_X, RowStyle, action, corner, heading, meta_line, row,
        title_line,
    },
    pink::Pink,
};

struct Host {
    ui: f32,
}

fn memory(prefix: &'static str, text: &'static str, t: &Pink) -> impl IntoElement {
    let style = RowStyle {
        title: t.text,
        meta: t.text_muted,
        wash: t.fill(pink_ui::pink::FillTier::Tertiary),
        fade_to: t.text_muted,
    };
    let actions = corner(1.0)
        .child(
            action("icons/memory-panel/pen.svg", t.text_muted)
                .debug_selector(move || format!("{prefix}-edit")),
        )
        .child(
            action(
                "icons/memory-panel/trash-bin-minimalistic.svg",
                t.text_muted,
            )
            .debug_selector(move || format!("{prefix}-forget")),
        );
    row(&style)
        .debug_selector(move || format!("{prefix}-row"))
        .child(title_line(
            style.fade_to,
            text,
            Some(actions.into_any_element()),
        ))
        .child(
            meta_line(style.meta, None, "Project instructions, CLAUDE.md")
                .debug_selector(move || format!("{prefix}-meta")),
        )
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div()
            .w(gpui::px(360.0))
            .flex()
            .flex_col()
            .child(
                heading(t.text_muted, t.text_muted, "Instructions", 2, 0.0)
                    .debug_selector(|| "heading".into()),
            )
            .child(memory("short", "Answer first", &t))
            .child(memory(
                "long",
                "Prefers a short RFC with the decision up top over a long thread, and wants \
                 every migration to ship behind a flag with its rollback written down first",
                &t,
            ))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn memory_rows_keep_height_actions_and_edges(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let heading = bounds(cx, "heading");
        assert!(
            (f32::from(heading.size.height) - HEADING_H * scale).abs() < 1.0,
            "heading is {:?} tall at {ui}",
            heading.size.height
        );
        for prefix in ["short", "long"] {
            let row = bounds(cx, &format!("{prefix}-row"));
            assert!(
                (f32::from(row.size.height) - ROW_H * scale).abs() < 1.0,
                "{prefix} row is {:?} tall at {ui}, expected {}",
                row.size.height,
                ROW_H * scale
            );
            let meta = bounds(cx, &format!("{prefix}-meta"));
            assert!(
                (f32::from(meta.origin.x - row.origin.x) - ROW_PAD_X * scale).abs() < 0.05,
                "{prefix} meta line inside the row's padding at {ui}"
            );
            for name in ["edit", "forget"] {
                let tile = bounds(cx, &format!("{prefix}-{name}"));
                assert!(
                    (f32::from(tile.size.width) - ACTION * scale).abs() < 0.05
                        && (f32::from(tile.size.height) - ACTION * scale).abs() < 0.05,
                    "{prefix} {name} is not an {ACTION} square at {ui}: {:?}",
                    tile.size
                );
                assert!(
                    tile.origin.x + tile.size.width <= row.origin.x + row.size.width,
                    "{prefix} {name} overflows the row at {ui}"
                );
            }
        }
        // A long memory fades under the actions; it never moves them.
        let short = bounds(cx, "short-forget");
        let long = bounds(cx, "long-forget");
        assert!(
            (f32::from(short.origin.x) - f32::from(long.origin.x)).abs() < 0.05,
            "a long memory pushes its actions at {ui}"
        );
    }
}
