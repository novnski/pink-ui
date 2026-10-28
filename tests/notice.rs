//! The `pink-notice` package's geometry at Interface sizes 12, 16 and 20, as
//! PINK lays its composer notice out in rems. Failure modes: a notice action
//! that keeps its 24 pt height at every Interface size (PINK's capsule grows
//! with the setting); the Copy message button losing its square or its
//! scale; a plate whose padding stops scaling, so the message no longer
//! spans the plate's inner width; a message that pushes a narrow plate wider
//! than its pane instead of wrapping.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::notice::{
        ACTION_HEIGHT, ActionStyle, COPY_SIZE, NoticeStyle, PLATE_PAD_X, action, copy_button,
        message, plate, spacer, title, title_row,
    },
    pink::Pink,
};

/// The plate's pane width, in px: fixed while the Interface size changes.
const PANE: f32 = 300.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div().flex().flex_col().items_start().child(
            div().w(gpui::px(PANE)).child(
                plate(&NoticeStyle {
                    plate: t.surface,
                    edge: t.border,
                    ink: t.text,
                })
                .debug_selector(|| "plate".into())
                .child(
                    title_row()
                        .child(title("Action failed"))
                        .child(spacer())
                        .child(
                            action(&ActionStyle {
                                border: t.border_strong,
                                text: t.text,
                            })
                            .debug_selector(|| "action".into())
                            .child("Dismiss"),
                        )
                        .child(copy_button().debug_selector(|| "copy".into())),
                )
                .child(
                    message("The worktree has uncommitted changes. ".repeat(12))
                        .debug_selector(|| "message".into()),
                ),
            ),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn notice_parts_scale_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let action = bounds(cx, "action");
        assert_eq!(
            f32::from(action.size.height),
            ACTION_HEIGHT * scale,
            "action height at {ui}"
        );

        let copy = bounds(cx, "copy");
        assert_eq!(
            f32::from(copy.size.width),
            COPY_SIZE * scale,
            "copy width at {ui}"
        );
        assert_eq!(
            f32::from(copy.size.height),
            COPY_SIZE * scale,
            "copy height at {ui}"
        );

        let plate = bounds(cx, "plate");
        assert_eq!(f32::from(plate.size.width), PANE, "plate width at {ui}");
        let message = bounds(cx, "message");
        // A 1 px edge on each side, then the scaled padding.
        let inner = PANE - 2.0 - 2.0 * PLATE_PAD_X * scale;
        assert!(
            (f32::from(message.size.width) - inner).abs() < 0.01,
            "message spans the plate's inner width at {ui}: {} vs {inner}",
            f32::from(message.size.width)
        );
        assert!(
            (f32::from(message.origin.x - plate.origin.x) - (1.0 + PLATE_PAD_X * scale)).abs()
                < 0.01,
            "message inset at {ui}"
        );
    }
}
