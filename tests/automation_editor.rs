//! The `pink-automation-editor` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out its automation editor. Failure modes: a setting
//! whose label column shrinks under a long control, or whose control does not
//! wrap under the label in a narrow pane; a field frame or caption that stays
//! at its 16 px size while the text grows; two fields that squeeze below
//! their minimum instead of wrapping onto their own lines; a schedule clock
//! or a review column that ignores the Interface size.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::automation_editor::{
        EditorStyle, FIELD_GAP, FIELD_MIN_WIDE, FRAME_PAD_X, FRAME_PAD_Y, FrameStyle, LABEL_WIDTH,
        SETTING_GAP_X, SUMMARY_ICON, field, frame, review, schedule_line, setting,
    },
    pink::Pink,
};

const ICON: &str = "icons/automation-editor/clock-circle.svg";

struct Host {
    ui: f32,
}

fn style(t: &Pink) -> EditorStyle {
    EditorStyle {
        text: t.text,
        heading: t.text_muted,
        caption: t.text_muted,
        accent: t.accent,
    }
}

fn input(prefix: &'static str, t: &Pink) -> impl IntoElement {
    frame(&FrameStyle {
        border: t.border,
        background: t.surface,
    })
    .debug_selector(move || format!("{prefix}-frame"))
    .child(
        div()
            .debug_selector(move || format!("{prefix}-input"))
            .w_full()
            .child("09:00"),
    )
}

fn control(prefix: &'static str) -> impl IntoElement {
    div()
        .debug_selector(move || format!("{prefix}-control"))
        .w(gpui::px(200.0))
        .h(gpui::px(24.0))
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let s = style(&t);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div().w(gpui::px(640.0)).child(
                    setting(&s, "Project", control("wide")).debug_selector(|| "wide".into()),
                ),
            )
            .child(div().w(gpui::px(200.0)).child(
                setting(&s, "Project", control("narrow")).debug_selector(|| "narrow".into()),
            ))
            .child(
                div()
                    .w(gpui::px(640.0))
                    .flex()
                    .flex_wrap()
                    .child(
                        field(&s, "Time (HH:MM)", FIELD_MIN_WIDE, input("one", &t))
                            .debug_selector(|| "one".into()),
                    )
                    .child(
                        field(&s, "Time zone", FIELD_MIN_WIDE, input("two", &t))
                            .debug_selector(|| "two".into()),
                    ),
            )
            .child(
                div()
                    .w(gpui::px(300.0))
                    .flex()
                    .flex_wrap()
                    .child(
                        field(&s, "Time (HH:MM)", FIELD_MIN_WIDE, input("tight-one", &t))
                            .debug_selector(|| "tight-one".into()),
                    )
                    .child(
                        field(&s, "Time zone", FIELD_MIN_WIDE, input("tight-two", &t))
                            .debug_selector(|| "tight-two".into()),
                    ),
            )
            .child(
                div().w(gpui::px(640.0)).child(
                    schedule_line(&s, ICON, "Weekdays at 09:00 (Europe/Lisbon)")
                        .debug_selector(|| "summary".into()),
                ),
            )
            .child(
                div().w(gpui::px(640.0)).flex().child(
                    review(220.0)
                        .debug_selector(|| "review".into())
                        .child("Review asks the host to check the change."),
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

fn near(a: Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() < 0.01
}

#[gpui::test]
fn the_editor_parts_scale_and_wrap_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // A setting keeps its label column and puts the control beside it,
        // or under it when the pane is too narrow for both.
        let wide = bounds(cx, "wide");
        let wide_control = bounds(cx, "wide-control");
        assert!(
            near(
                wide_control.origin.x - wide.origin.x,
                (LABEL_WIDTH + SETTING_GAP_X) * scale
            ),
            "the control follows the label column at {ui}"
        );
        assert!(
            wide_control.origin.y < wide.origin.y + wide_control.size.height,
            "a wide setting keeps its control on the label's line at {ui}"
        );
        let narrow = bounds(cx, "narrow");
        let narrow_control = bounds(cx, "narrow-control");
        assert!(
            near(narrow_control.origin.x, f32::from(narrow.origin.x)),
            "a narrow setting wraps its control under the label at {ui}"
        );

        // A field: the caption over the frame, the input inset by the
        // frame's scaled padding and 1 px edge.
        let one = bounds(cx, "one");
        let one_frame = bounds(cx, "one-frame");
        let one_input = bounds(cx, "one-input");
        assert!(
            one_frame.origin.y - one.origin.y > gpui::px(FIELD_GAP * scale),
            "the caption sits above the frame at {ui}"
        );
        assert!(
            near(
                one_input.origin.x - one_frame.origin.x,
                1.0 + FRAME_PAD_X * scale
            ),
            "the frame's side padding scales at {ui}"
        );
        assert!(
            near(
                one_input.origin.y - one_frame.origin.y,
                1.0 + FRAME_PAD_Y * scale
            ),
            "the frame's top padding scales at {ui}"
        );

        // Two fields share a wide line and wrap rather than squeeze.
        let two = bounds(cx, "two");
        assert!(
            near(two.origin.y, f32::from(one.origin.y)),
            "two fields share a wide line at {ui}"
        );
        let tight_one = bounds(cx, "tight-one");
        let tight_two = bounds(cx, "tight-two");
        if 2.0 * FIELD_MIN_WIDE * scale > 300.0 {
            assert!(
                tight_two.origin.y >= tight_one.origin.y + tight_one.size.height,
                "fields below their minimum wrap onto their own lines at {ui}"
            );
        }
        assert!(
            f32::from(tight_one.size.width) >= FIELD_MIN_WIDE * scale - 0.01,
            "a field never squeezes below its minimum at {ui}"
        );

        // The clock and the review column grow with the Interface size.
        let summary = bounds(cx, "summary");
        assert!(
            f32::from(summary.size.height) >= SUMMARY_ICON * scale - 0.01,
            "the schedule line holds its scaled clock at {ui}"
        );
        let review = bounds(cx, "review");
        assert!(
            f32::from(review.size.width) >= 220.0 * scale - 0.01,
            "the review column holds its scaled minimum at {ui}"
        );
    }
}
