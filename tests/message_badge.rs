//! The `pink-message-badge` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK draws a sent message's context pills and their hover card in
//! rems. Failure modes: a pill that keeps its 24 pt floor at every Interface
//! size (PINK's composer sizes its badge strip as `BADGE_HEIGHT` times the
//! setting); a hover card that stops scaling its width or inset, so its rows
//! no longer span the card.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::message_badge::{
        BADGE_GLYPH, BADGE_HEIGHT, BadgeStyle, CARD_PAD, CARD_WIDTH, DetailStyle, badge, card,
        detail_row, ui_rems,
    },
    pink::{self, Pink},
};

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let detail = DetailStyle {
            plate: t.ink(0.05),
            bar: t.solid,
            location: t.text_faint,
            tag: t.ink(0.10),
            body: t.text,
            mono: pink::MONO.into(),
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                badge(&BadgeStyle {
                    fill: t.ink(0.06),
                    text: t.text_muted,
                })
                .debug_selector(|| "badge".into())
                .child(div().flex_none().size(ui_rems(BADGE_GLYPH)))
                .child("2 comments"),
            )
            .child(
                card(div().debug_selector(|| "card".into())).child(
                    detail_row(&detail, "src/main.rs:42", Some("R".into()), "early-return")
                        .debug_selector(|| "row".into()),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn badges_and_their_card_scale_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let badge = bounds(cx, "badge");
        assert_eq!(
            f32::from(badge.size.height),
            BADGE_HEIGHT * scale,
            "badge height at {ui}"
        );

        let card = bounds(cx, "card");
        assert_eq!(
            f32::from(card.size.width),
            CARD_WIDTH * scale,
            "card width at {ui}"
        );
        let row = bounds(cx, "row");
        assert!(
            (f32::from(row.size.width) - (CARD_WIDTH - 2.0 * CARD_PAD) * scale).abs() < 0.01,
            "row spans the card's inset at {ui}"
        );
        assert!(
            (f32::from(row.origin.x - card.origin.x) - CARD_PAD * scale).abs() < 0.01,
            "row inset at {ui}"
        );
    }
}
