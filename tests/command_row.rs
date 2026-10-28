//! The `pink-command-row` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its command palette rows out. Failure modes: an action
//! or thread row that stops growing with the Interface size (its label
//! clips); a thread's attribution that overlaps its summary; a glyph that
//! stops scaling with the label beside it; a result inset or end room that
//! no longer scales.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::command_row::{
        ACTION_MIN_HEIGHT, DETAIL_MIN_HEIGHT, DetailStyle, GLYPH, RESULTS_INSET, RowKind, detail,
        inset, label, row, slot,
    },
    components::menu_card,
    pink::Pink,
};

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let base = || {
            menu_card::row(&menu_card::RowStyle {
                text: t.text,
                fill: t.element_active,
            })
        };
        div()
            .w(gpui::px(480.0))
            .flex()
            .flex_col()
            .child(
                slot(true, false)
                    .debug_selector(|| "first".into())
                    .child(inset(
                        row(base(), RowKind::Action)
                            .debug_selector(|| "action".into())
                            .child(
                                div()
                                    .debug_selector(|| "glyph".into())
                                    .flex_none()
                                    .size(pink_ui::components::command_row::ui_rems(GLYPH)),
                            )
                            .child(label().child("New chat")),
                    )),
            )
            .child(
                slot(false, true).child(inset(
                    row(base(), RowKind::Detail)
                        .debug_selector(|| "detail".into())
                        .child(detail(
                            "Retire the expired coupon codes",
                            "pink, 3 hours ago",
                            &DetailStyle {
                                summary: t.text,
                                meta: t.text_faint,
                            },
                        )),
                )),
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
fn palette_rows_grow_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let first = bounds(cx, "first");
        let action = bounds(cx, "action");
        let glyph = bounds(cx, "glyph");
        let detail = bounds(cx, "detail");

        assert!(
            f32::from(action.size.height) >= ACTION_MIN_HEIGHT * scale - 0.5,
            "action at {ui}: {:?}",
            action.size
        );
        assert!(
            f32::from(detail.size.height) >= DETAIL_MIN_HEIGHT * scale - 0.5,
            "detail at {ui}: {:?}",
            detail.size
        );
        assert!(
            near(f32::from(glyph.size.width), GLYPH * scale),
            "glyph at {ui}"
        );
        // The first result's room above and the inset both scale.
        assert!(
            near(
                f32::from(action.origin.y - first.origin.y),
                RESULTS_INSET * scale
            ) && near(
                f32::from(action.origin.x - first.origin.x),
                RESULTS_INSET * scale
            ),
            "inset at {ui}: {action:?} in {first:?}"
        );
    }
}
