//! The `pink-row-card` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays out its agent and automation cards. Failure modes: an agent
//! card whose actions squeeze the title instead of wrapping under the text
//! when the card is narrow, or wrap even when there is room (a long preview
//! that refuses to truncate pushes them under); wrapped
//! actions that drift off the trailing edge; padding or a tile that stays at
//! its 16 px size while the text grows; an inline card that wraps.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::{
        row_card::{
            AgentCardStyle, AgentCardText, BadgeStyle, CardStyle, Layout, PAD_X, PAD_Y, actions,
            agent_actions, agent_card, card, detail, text_column, title,
        },
        settings_parts::{TILE_SIZE, row_tile, tile_style},
    },
    pink::Pink,
};

const ICON: &str = "icons/subagent-cards/helix.svg";

struct Host {
    ui: f32,
}

fn agent(width: f32, prefix: &'static str, t: &Pink) -> impl IntoElement {
    div().w(gpui::px(width)).child(
        agent_card(
            &AgentCardStyle {
                card: CardStyle {
                    border: t.border,
                    background: t.surface,
                },
                badge: BadgeStyle {
                    background: t.surface,
                    text: t.text_muted,
                },
                title: t.text,
                meta: t.text_muted,
                status_dot: t.accent,
                status_tone: None,
                preview: t.text_muted,
            },
            row_tile(&tile_style(t), ICON).debug_selector(move || format!("{prefix}-tile")),
            AgentCardText {
                title: "Audit webhook retries".into(),
                badge: "Fresh side chat".into(),
                status: "Running".into(),
                duration: Some("1m 12s".into()),
                detail: Some("Helix 4 Pro High".into()),
                // Long enough to overflow any card here: only the preview
                // gives way, so the actions stay beside a wide card's text.
                preview: "List every webhook handler that is not idempotent, then \
                          group them by the queue that retries them and say which \
                          ones can double-charge a customer."
                    .into(),
            },
            agent_actions()
                .debug_selector(move || format!("{prefix}-actions"))
                .child(div().w(gpui::px(96.0)).h(gpui::px(24.0))),
        )
        .debug_selector(move || format!("{prefix}-card")),
    )
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(agent(640.0, "wide", &t))
            .child(agent(260.0, "narrow", &t))
            .child(
                div().w(gpui::px(300.0)).child(
                    card(
                        Layout::Inline,
                        &CardStyle {
                            border: t.border,
                            background: t.surface,
                        },
                    )
                    .debug_selector(|| "inline-card".into())
                    .child(row_tile(&tile_style(&t), ICON))
                    .child(
                        text_column(Layout::Inline)
                            .debug_selector(|| "inline-text".into())
                            .child(title(t.text, "Morning dependency check"))
                            .child(detail(t.text_muted, "Weekdays at 9:00, next run tomorrow")),
                    )
                    .child(
                        actions(Layout::Inline)
                            .debug_selector(|| "inline-actions".into())
                            .child(div().w(gpui::px(96.0)).h(gpui::px(24.0))),
                    ),
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

#[gpui::test]
fn row_cards_wrap_their_actions_only_when_narrow(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for prefix in ["wide", "narrow"] {
            let card = bounds(cx, &format!("{prefix}-card"));
            let tile = bounds(cx, &format!("{prefix}-tile"));
            assert!(
                (f32::from(tile.size.width) - TILE_SIZE * scale).abs() < 0.01,
                "{prefix} tile at {ui}"
            );
            // A 1 px edge, then the scaled padding.
            assert!(
                (f32::from(tile.origin.x - card.origin.x) - (1.0 + PAD_X * scale)).abs() < 0.01,
                "{prefix} leading inset at {ui}"
            );
            let actions = bounds(cx, &format!("{prefix}-actions"));
            assert!(
                (f32::from(
                    card.origin.x + card.size.width - actions.origin.x - actions.size.width
                ) - (1.0 + PAD_X * scale))
                    .abs()
                    < 0.01,
                "{prefix} actions on the trailing edge at {ui}"
            );
        }

        // The tile centres on the text line, so actions beside the text
        // start above the tile's bottom and wrapped ones below it.
        let wide_tile = bounds(cx, "wide-tile");
        let wide_actions = bounds(cx, "wide-actions");
        assert!(
            wide_actions.origin.y < wide_tile.origin.y + wide_tile.size.height,
            "a wide card keeps its actions beside the text at {ui}"
        );
        let narrow_card = bounds(cx, "narrow-card");
        let narrow_tile = bounds(cx, "narrow-tile");
        let narrow_actions = bounds(cx, "narrow-actions");
        assert!(
            narrow_actions.origin.y >= narrow_tile.origin.y + narrow_tile.size.height,
            "a narrow card wraps its actions under the text at {ui}"
        );
        assert!(
            (f32::from(
                narrow_card.origin.y + narrow_card.size.height
                    - narrow_actions.origin.y
                    - narrow_actions.size.height
            ) - (1.0 + PAD_Y * scale))
                .abs()
                < 0.01,
            "wrapped actions keep the bottom padding at {ui}"
        );

        let inline_text = bounds(cx, "inline-text");
        let inline_actions = bounds(cx, "inline-actions");
        assert!(
            inline_actions.origin.y < inline_text.origin.y + inline_text.size.height,
            "an inline card never wraps at {ui}"
        );
    }
}
