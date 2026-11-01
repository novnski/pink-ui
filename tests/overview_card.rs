//! The `pink-overview-card` package's geometry at Interface sizes 12, 16 and
//! 20, and the canvas lane it shares with the conversation. Failure modes:
//! a row, its padding or its glyph that stays fixed while the text grows
//! (cramped rows at large sizes), or that drifts from T3's 32/10/16 row
//! grammar; a section inset that stops scaling with the rows; a section
//! rule that stops at the inset; a split control whose options half is not
//! the chevron slot's width. In
//! the canvas: a card that stays inline when the chat beside it would drop
//! below the readable minimum (transcript or composer under the card); a
//! chat that jumps left as soon as the card opens instead of staying
//! centred while it fits; a chat that narrows before it reaches the
//! canvas's padding, or that comes closer to the card than the clearance; a
//! card taller than the canvas leaves room for.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::overview_card::{
        CARD_GAP, CARD_WIDTH, CHAT_CLEARANCE, CHEVRON_SLOT, CardStyle, Density, GLYPH,
        LANE_PADDING, MIN_CARD_HEIGHT, MIN_CHAT_WIDTH, ROW_HEIGHT, ROW_PAD_X, RowStyle,
        SECTION_PAD_X, card, card_placement, chat_lane, density, glyph, label, row, section,
        split_group, split_primary, split_secondary, split_separator,
    },
    pink::Pink,
};

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = RowStyle {
            text: t.text,
            fill: t.element_hover,
        };
        div().w(gpui::px(CARD_WIDTH)).child(
            card(&CardStyle {
                border: t.border,
                background: t.surface,
                text: t.text,
                font_family: pink_ui::pink::SANS.into(),
                shadows: None,
            })
            .debug_selector(|| "card".into())
            .child(
                section(None).child(
                    row(&style)
                        .debug_selector(|| "row".into())
                        .child(
                            glyph("icons/right-panel/git-branch.svg", t.text_muted)
                                .debug_selector(|| "glyph".into()),
                        )
                        .child(label("main")),
                ),
            )
            .child(
                section(Some(t.border))
                    .debug_selector(|| "ruled".into())
                    .child(row(&style).child(label("Agents")))
                    .child(
                        split_group("split")
                            .debug_selector(|| "split".into())
                            .child(split_primary(&style).child(label("Commit")))
                            .child(split_separator("split", t.border))
                            .child(
                                split_secondary(&style).debug_selector(|| "split-options".into()),
                            ),
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
fn rows_keep_t3s_grammar_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let c = bounds(cx, "card");
        let r = bounds(cx, "row");
        let g = bounds(cx, "glyph");
        let ruled = bounds(cx, "ruled");

        let split = bounds(cx, "split");
        let options = bounds(cx, "split-options");

        // The 1 px border plus the section's inset, scaled with the rows.
        let inset = 1.0 + SECTION_PAD_X * scale;
        assert!(
            near(f32::from(r.origin.x - c.origin.x), inset),
            "row inset at {ui}"
        );
        assert!(
            near(f32::from(c.right() - r.right()), inset),
            "row fills the section at {ui}"
        );
        // The row, its padding and its glyph grow with the Interface size.
        assert!(
            near(f32::from(r.size.height), ROW_HEIGHT * scale),
            "row height at {ui}: {:?}",
            r.size
        );
        // A split control is a row's shape, its options half the chevron slot.
        assert!(
            near(f32::from(split.size.height), ROW_HEIGHT * scale)
                && near(f32::from(split.size.width), f32::from(r.size.width)),
            "split control at {ui}"
        );
        assert!(
            near(f32::from(options.size.width), CHEVRON_SLOT * scale)
                && near(f32::from(options.right()), f32::from(split.right())),
            "split options at {ui}"
        );
        assert!(
            near(f32::from(g.origin.x - r.origin.x), ROW_PAD_X * scale),
            "row padding at {ui}"
        );
        assert!(
            near(f32::from(g.size.width), GLYPH * scale),
            "glyph at {ui}"
        );
        // A ruled section runs border to border.
        assert!(
            near(f32::from(ruled.origin.x - c.origin.x), 1.0)
                && near(f32::from(c.right() - ruled.right()), 1.0),
            "section rule at {ui}"
        );
    }
}

#[test]
fn the_card_sits_inline_only_beside_a_readable_chat() {
    let height = 800.0;
    let preferred = 608.0;
    let min_chat = MIN_CHAT_WIDTH.min(preferred);
    // The narrowest canvas that still leaves the minimum chat beside it.
    let narrowest = min_chat + LANE_PADDING + CHAT_CLEARANCE + CARD_WIDTH + CARD_GAP;
    for width in [narrowest, 1000.0, 1100.0, 1400.0, 2200.0] {
        let card = card_placement(width, height, min_chat, 0.0)
            .unwrap_or_else(|| panic!("inline at {width}"));
        assert!(near(card.left + card.width + card.right, width));
        assert!(near(card.top, CARD_GAP) && near(card.height, height - 2.0 * CARD_GAP));
        let chat = chat_lane(width, preferred, Some(card.left));
        assert!(
            chat.left + 0.01 >= LANE_PADDING,
            "chat inside the padding at {width}"
        );
        assert!(
            chat.left + chat.width <= card.left - CHAT_CLEARANCE + 0.01,
            "chat clear of the card at {width}"
        );
        assert!(
            chat.width + 0.01 >= min_chat,
            "chat stays readable at {width}"
        );
        // Centred while it fits; moved only as far as the clearance needs.
        let centred = (width - chat.width) / 2.0;
        if centred + chat.width <= card.left - CHAT_CLEARANCE {
            assert!(near(chat.left, centred), "chat stays centred at {width}");
        } else {
            assert!(
                near(chat.left + chat.width, card.left - CHAT_CLEARANCE)
                    || near(chat.left, LANE_PADDING),
                "chat moves only as far as it must at {width}"
            );
        }
        // It narrows only once it reaches the padding.
        if chat.width < preferred - 0.01 {
            assert!(near(chat.left, LANE_PADDING), "narrowed early at {width}");
        }
    }
    // One pixel narrower, or too short: the popover takes over.
    assert_eq!(card_placement(narrowest - 1.0, height, min_chat, 0.0), None);
    assert_eq!(
        card_placement(
            1400.0,
            MIN_CARD_HEIGHT + 2.0 * CARD_GAP - 1.0,
            min_chat,
            0.0
        ),
        None
    );
    // Chrome above the card moves it down and shortens it.
    let lowered = card_placement(1400.0, height, min_chat, 40.0).unwrap();
    assert!(near(lowered.top, CARD_GAP + 40.0));
    assert!(near(lowered.height, height - CARD_GAP * 2.0 - 40.0));
}

#[test]
fn density_drops_only_when_the_content_does_not_fit() {
    // Unmeasured densities are tried so they get measured.
    assert_eq!(density(300.0, 0.0, 0.0), Density::Full);
    assert_eq!(density(300.0, 280.0, 200.0), Density::Full);
    assert_eq!(density(300.0, 420.0, 0.0), Density::Compact);
    assert_eq!(density(300.0, 420.0, 260.0), Density::Compact);
    assert_eq!(density(300.0, 420.0, 340.0), Density::Essential);
}
