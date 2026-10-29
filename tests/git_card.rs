//! The `pink-git-card` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays out the Git card over the diff. Failure modes: buttons that
//! stop growing with their label's type (labels clip at a large Interface
//! size) or that change height between the secondary and the accent plate;
//! a file row whose path stops truncating and pushes its Stage button out of
//! the card; a card that loses its insets.

use gpui::{
    Bounds, Context, FontWeight, IntoElement, Pixels, Render, TestAppContext, Window, div,
    prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::git_card::{
        BUTTON_HEIGHT, ButtonStyle, CARD_PAD_X, CARD_PAD_Y, CardStyle, button, button_row, card,
        file_path, file_row, pick_box,
    },
};

const WIDTH: f32 = 360.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let black = gpui::black();
        let secondary = ButtonStyle {
            fill: gpui::white(),
            border: black,
            text: black,
            weight: None,
        };
        let primary = ButtonStyle {
            fill: black,
            border: gpui::transparent_black(),
            text: gpui::white(),
            weight: Some(FontWeight::MEDIUM),
        };
        div().w(px(WIDTH)).child(
            card(
                "card",
                &CardStyle {
                    border: black,
                    fill: gpui::white(),
                },
            )
            .debug_selector(|| "card".into())
            .child(
                file_row()
                    .debug_selector(|| "file".into())
                    .child(pick_box("pick", true, black))
                    .child(file_path(
                        "a/very/long/path/that/never/fits/in/the/card/at/all/lib.rs",
                        "Menlo".into(),
                        black,
                    ))
                    .child(button("stage", "Stage", &primary).debug_selector(|| "stage".into())),
            )
            .child(
                button_row().child(
                    button("commit", "Commit", &secondary).debug_selector(|| "commit".into()),
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
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn card_buttons_scale_and_rows_stay_inside(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let card = bounds(cx, "card");
        let file = bounds(cx, "file");
        let stage = bounds(cx, "stage");
        let commit = bounds(cx, "commit");

        for button in [stage, commit] {
            assert!(
                near(f32::from(button.size.height), BUTTON_HEIGHT * scale),
                "button at {ui}: {button:?}"
            );
        }
        assert!(
            near(f32::from(file.origin.x - card.origin.x), CARD_PAD_X)
                && near(f32::from(file.origin.y - card.origin.y), CARD_PAD_Y),
            "insets at {ui}: {file:?} in {card:?}"
        );
        assert!(
            f32::from(stage.origin.x + stage.size.width)
                <= f32::from(card.origin.x + card.size.width) - CARD_PAD_X + 0.5,
            "Stage stays in the card at {ui}: {stage:?} in {card:?}"
        );
    }
}
