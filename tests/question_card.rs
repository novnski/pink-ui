//! The `pink-question-card` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its question card out. Failure modes: padding, gaps or
//! the number chip that stay fixed while the text grows (a cramped card at
//! large sizes); an answer field whose inset or padding drifts from the
//! composer's; a footer that loses the primary action's trailing edge.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, SharedString, TestAppContext, Window, div,
    prelude::*,
};
use pink_ui::{
    Theme, ThemeMode,
    components::question_card::{
        ANSWER_INSET, ANSWER_PAD_X, ANSWER_PAD_Y, COUNTER_HEIGHT, FOOTER_TOP, NUMBER_SIZE,
        OPTION_PAD_Y, PAD, answer, body, footer, header, notes, option, option_style, options,
        palette, ui_rems,
    },
    pink::Pink,
};

/// The host's line height, in pt at the 16 px default: shorter than the
/// counter and the number chip, so they set their rows' heights.
const LINE: f32 = 16.0;
/// The host's answer field and primary action, in pt at the 16 px default.
const FIELD: f32 = 20.0;
const ACTION: f32 = 24.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let p = palette(&t);
        div().w(gpui::px(480.0)).line_height(ui_rems(LINE)).child(
            div()
                .debug_selector(|| "card".into())
                .flex()
                .flex_col()
                .child(
                    body()
                        .child(
                            header(&p, "APPROVAL", Some(SharedString::from("1 of 2")))
                                .debug_selector(|| "header".into()),
                        )
                        .child(
                            options("options").child(
                                option(
                                    &option_style(&t, true, t.ink(0.025)),
                                    "Allow once",
                                    [],
                                    Some(1),
                                )
                                .debug_selector(|| "option".into()),
                            ),
                        )
                        .child(
                            answer(
                                div().debug_selector(|| "surface".into()),
                                div()
                                    .debug_selector(|| "field".into())
                                    .flex_1()
                                    .h(ui_rems(FIELD)),
                            )
                            .debug_selector(|| "answer".into()),
                        ),
                )
                .child(
                    footer()
                        .debug_selector(|| "footer".into())
                        .child(notes())
                        .child(
                            div()
                                .debug_selector(|| "action".into())
                                .flex_none()
                                .size(ui_rems(ACTION)),
                        ),
                ),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn the_card_grows_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let card = bounds(cx, "card");
        let header = bounds(cx, "header");
        let option = bounds(cx, "option");
        let answer = bounds(cx, "answer");
        let surface = bounds(cx, "surface");
        let field = bounds(cx, "field");
        let footer = bounds(cx, "footer");
        let action = bounds(cx, "action");

        // The body's inset and the counter-high header.
        assert!(
            near(f32::from(header.origin.x - card.origin.x), PAD * scale),
            "body inset x at {ui}"
        );
        assert!(
            near(f32::from(header.origin.y - card.origin.y), PAD * scale),
            "body inset y at {ui}"
        );
        assert!(
            near(f32::from(header.size.height), COUNTER_HEIGHT * scale),
            "header height at {ui}: {:?}",
            header.size
        );

        // An option row is its number chip plus its padding, both scaled,
        // inside its 1 px edge.
        assert!(
            near(
                f32::from(option.size.height),
                (NUMBER_SIZE + 2.0 * OPTION_PAD_Y) * scale + 2.0
            ),
            "option height at {ui}: {:?}",
            option.size
        );

        // The answer field sits inset in the host's surface, padded inside.
        assert!(
            near(
                f32::from(surface.origin.x - answer.origin.x),
                ANSWER_INSET * scale
            ),
            "answer inset x at {ui}"
        );
        assert!(
            near(
                f32::from(answer.bottom() - surface.bottom()),
                ANSWER_INSET * scale
            ),
            "answer inset bottom at {ui}"
        );
        assert!(
            near(
                f32::from(field.origin.x - surface.origin.x),
                ANSWER_PAD_X * scale
            ),
            "field padding x at {ui}"
        );
        assert!(
            near(
                f32::from(field.origin.y - surface.origin.y),
                ANSWER_PAD_Y * scale
            ),
            "field padding y at {ui}"
        );

        // The primary action keeps the footer's trailing edge.
        assert!(
            near(f32::from(footer.right() - action.right()), PAD * scale),
            "action trailing edge at {ui}"
        );
        assert!(
            near(
                f32::from(action.origin.y - footer.origin.y),
                FOOTER_TOP * scale
            ),
            "footer top at {ui}"
        );
        assert!(
            near(f32::from(footer.bottom() - action.bottom()), PAD * scale),
            "footer bottom at {ui}"
        );
    }
}
