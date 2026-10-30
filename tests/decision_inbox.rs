//! The `pink-decision-inbox` package's geometry at Interface sizes 12, 16
//! and 20, as PINK's Activity and the gallery's Decision Inbox list waiting
//! decisions with it. Failure modes: a card that stops scaling with the
//! Interface size (fixed pixels clip its text at 20 or float it at 12); a long
//! question that pushes the status out of a narrow sidebar card instead of
//! ending in an ellipsis, in a narrow card or a wide one; fact lines that stop
//! wrapping, so a refusal reason runs off the edge.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::decision_inbox::{CARD_H, CardStyle, TILE, card, fact, facts, ui_rems},
    pink::Pink,
};

const LONG: &str = "Should the migration rewrite every saved workspace layout before the next release ships to the team?";

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = CardStyle {
            border: t.accent,
            background: t.ink(0.045),
            tile: t.ink(0.09),
            icon: t.content_muted,
            title: t.text,
            status: t.accent,
        };
        let row = |name: &'static str, width: f32, title: &'static str| {
            div().w(gpui::px(width)).child(
                card(
                    &style,
                    "icons/decision-inbox/chat-round-line.svg",
                    title,
                    "Open",
                )
                .debug_selector(move || format!("{name}-card"))
                .child(div().size_0().debug_selector(move || format!("{name}-end"))),
            )
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(row("narrow", 240.0, LONG))
            .child(row("wide", 900.0, LONG))
            .child(div().w(gpui::px(200.0)).child(
                facts().child(fact(t.text_muted, LONG).debug_selector(|| "fact".to_owned())),
            ))
            .child(
                div()
                    .w(ui_rems(TILE))
                    .h_1()
                    .debug_selector(|| "tile".to_owned()),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn cards_scale_with_the_interface_and_keep_their_status_in_reach(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for name in ["narrow", "wide"] {
            let card = bounds(cx, &format!("{name}-card"));
            assert!(
                (f32::from(card.size.height) - CARD_H * scale).abs() < 0.5,
                "{name} card is {:?} tall at {ui}, expected {}",
                card.size.height,
                CARD_H * scale
            );
            // The last child sits inside the card: a long question never
            // pushes the status past the card's edge.
            let end = bounds(cx, &format!("{name}-end"));
            assert!(
                end.origin.x <= card.origin.x + card.size.width,
                "{name} status pushed out at {ui}: {:?} past {:?}",
                end.origin.x,
                card.origin.x + card.size.width
            );
        }
        let tile = bounds(cx, "tile");
        assert!((f32::from(tile.size.width) - TILE * scale).abs() < 0.05);
        // A refusal reason wraps inside its column rather than running off.
        let fact = bounds(cx, "fact");
        assert!(f32::from(fact.size.width) <= 200.0 + 0.5);
        assert!(
            f32::from(fact.size.height) > 11.5 * scale * 2.0,
            "a long fact did not wrap at {ui}: {:?}",
            fact.size.height
        );
    }
}
