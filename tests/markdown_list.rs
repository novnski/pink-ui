//! The `pink-markdown-list` package's geometry at Interface sizes 12, 16
//! and 20. Failure modes: a task box, disc or marker column that stays fixed
//! while the body text grows (a tiny box beside large text); a box or disc
//! that leaves the first line's centre, so it floats above or below the
//! text it marks; a task item taller than a bullet item with the same text
//! (the box pushing the line); text that starts at a different x beside a
//! box than beside a disc, so a mixed list loses its left edge.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::markdown_list::{
        MARKER_GAP, MARKER_MIN_W, TASK_BOX, body, disc, item, list, marker_slot, task_box,
        task_style, ui_rems,
    },
    pink::Pink,
};

/// PINK's body text (`MD_TEXT_SIZE` / `MD_LINE_HEIGHT`).
const TEXT: f32 = 15.0;
const LINE: f32 = 24.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`), and lays out a task item, a done read-only
/// task item and a bullet item, each with one line of text.
struct Host {
    ui: f32,
}

fn text(name: &'static str) -> impl IntoElement {
    body().child(
        div()
            .text_size(ui_rems(TEXT))
            .line_height(ui_rems(LINE))
            .debug_selector(move || format!("{name}-text"))
            .child("Ship it"),
    )
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = task_style(&t);
        let task = |name: &'static str, checked: bool, toggleable: bool| {
            item(
                marker_slot(LINE)
                    .debug_selector(move || format!("{name}-slot"))
                    .child(
                        task_box(&style, checked, toggleable, svg())
                            .debug_selector(move || format!("{name}-box")),
                    ),
                text(name),
            )
            .debug_selector(move || format!("{name}-item"))
        };
        div().w(gpui::px(400.0)).child(
            list()
                .child(task("open", false, true))
                .child(task("done", true, false))
                .child(
                    item(
                        disc(t.accent, LINE).debug_selector(|| "bullet-slot".into()),
                        text("bullet"),
                    )
                    .debug_selector(|| "bullet-item".into()),
                ),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    cx.debug_bounds(Box::leak(name.to_owned().into_boxed_str()))
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn task_boxes_scale_with_the_interface_and_sit_on_the_first_line(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let bullet_item = bounds(cx, "bullet-item");
        let bullet_text = bounds(cx, "bullet-text");
        for name in ["open", "done"] {
            let slot = bounds(cx, &format!("{name}-slot"));
            let check = bounds(cx, &format!("{name}-box"));
            let row = bounds(cx, &format!("{name}-item"));
            let text = bounds(cx, &format!("{name}-text"));
            assert!(
                near(f32::from(check.size.width), TASK_BOX * scale)
                    && near(f32::from(check.size.height), TASK_BOX * scale),
                "{name} box at {ui}: {:?}",
                check.size
            );
            assert!(
                near(f32::from(slot.size.width), MARKER_MIN_W * scale)
                    && near(f32::from(slot.size.height), LINE * scale),
                "{name} slot at {ui}: {:?}",
                slot.size
            );
            // Centred on the first line of the text it marks.
            assert!(
                near(
                    f32::from(check.center().y - text.origin.y),
                    LINE * scale / 2.0
                ),
                "{name} box centred on the first line at {ui}: box {check:?} text {text:?}"
            );
            // Same height and text edge as a bullet item.
            assert!(
                near(
                    f32::from(row.size.height),
                    f32::from(bullet_item.size.height)
                ),
                "{name} item height at {ui}: {row:?} vs bullet {bullet_item:?}"
            );
            assert!(
                near(f32::from(text.origin.x), f32::from(bullet_text.origin.x))
                    && near(
                        f32::from(text.origin.x - slot.origin.x),
                        (MARKER_MIN_W + MARKER_GAP) * scale
                    ),
                "{name} text edge at {ui}: {text:?} vs bullet {bullet_text:?}"
            );
        }
        // The disc scales too, centred on the same line.
        let slot = bounds(cx, "bullet-slot");
        assert!(
            near(f32::from(slot.size.height), LINE * scale),
            "bullet slot at {ui}: {:?}",
            slot.size
        );
    }
}
