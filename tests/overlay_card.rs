//! The `pink-overlay-card` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its tooltips, dialogs and composer popups out. Failure
//! modes: tooltip padding or the wrap width that stays fixed while the
//! caption grows (cramped or early-wrapping tooltips at large sizes); a
//! short label forced out to the wrap width; a dialog whose width or padding
//! ignores the Interface size; a popup inset that scales and pulls rows off
//! the stack ladder's concentric corners; an above-the-trigger root that no
//! longer centres its card over the anchor or lets it cover the anchor.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::overlay_card::{
        DIALOG_PAD, DIALOG_WIDTH, STACK_POPUP_INSET, TOOLTIP_MAX_WIDTH, TOOLTIP_PAD_Y,
        TOOLTIP_WRAP_CHARS, above, dialog_card, dialog_style, stack_popup, text_tooltip,
        tooltip_style, ui_rems,
    },
    pink::Pink,
};

/// The caption's line height, in pt at the 16 px default.
const CAPTION_LINE: f32 = 14.0;
/// Where the above-the-trigger root is anchored, in px.
const ANCHOR_X: f32 = 300.0;
const ANCHOR_Y: f32 = 120.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let tooltip = |name: &'static str, label: String| {
            text_tooltip(&tooltip_style(&t), t.text, label)
                .text_size(ui_rems(11.0))
                .line_height(ui_rems(CAPTION_LINE))
                .debug_selector(move || name.into())
        };
        // The host's menu card: a 1 px edge and its own 4 px inset, which
        // the popup step replaces.
        let menu_card = div().border_1().border_color(t.border).p(px(4.0));
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(px(24.0))
            .child(tooltip("short", "Copy".into()))
            .child(tooltip("long", "x".repeat(TOOLTIP_WRAP_CHARS + 1)))
            .child(
                dialog_card(&dialog_style(&t))
                    .debug_selector(|| "dialog".into())
                    .child(div().h(px(10.0)).debug_selector(|| "dialog-body".into())),
            )
            .child(
                stack_popup(menu_card)
                    .w(px(400.0))
                    .debug_selector(|| "popup".into())
                    .child(div().h(px(20.0)).debug_selector(|| "popup-row".into())),
            )
            .child(
                div().relative().w(px(600.0)).h(px(200.0)).child(
                    div().absolute().left(px(ANCHOR_X)).top(px(ANCHOR_Y)).child(
                        above(
                            div()
                                .w(px(80.0))
                                .h(px(30.0))
                                .debug_selector(|| "above-card".into()),
                        )
                        .debug_selector(|| "anchor".into()),
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
fn overlay_cards_scale_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // A short label stays on one line inside its scaled padding.
        let short = bounds(cx, "short");
        assert!(
            near(
                f32::from(short.size.height),
                (CAPTION_LINE + 2.0 * TOOLTIP_PAD_Y) * scale + 2.0
            ),
            "short tooltip height at {ui}: {:?}",
            short.size
        );
        assert!(
            f32::from(short.size.width) < TOOLTIP_MAX_WIDTH * scale,
            "short tooltip forced to the wrap width at {ui}"
        );
        // A long one wraps at the scaled maximum.
        let long = bounds(cx, "long");
        assert!(
            near(f32::from(long.size.width), TOOLTIP_MAX_WIDTH * scale),
            "long tooltip width at {ui}: {:?}",
            long.size
        );

        let dialog = bounds(cx, "dialog");
        let body = bounds(cx, "dialog-body");
        assert!(
            near(f32::from(dialog.size.width), DIALOG_WIDTH * scale),
            "dialog width at {ui}"
        );
        assert!(
            near(
                f32::from(body.origin.x - dialog.origin.x),
                1.0 + DIALOG_PAD * scale
            ),
            "dialog padding x at {ui}"
        );
        assert!(
            near(
                f32::from(body.origin.y - dialog.origin.y),
                1.0 + DIALOG_PAD * scale
            ),
            "dialog padding y at {ui}"
        );

        // The popup's inset stays on the ladder at every size.
        let popup = bounds(cx, "popup");
        let row = bounds(cx, "popup-row");
        assert!(
            near(
                f32::from(row.origin.x - popup.origin.x),
                1.0 + STACK_POPUP_INSET
            ),
            "popup inset x at {ui}"
        );
        assert!(
            near(
                f32::from(row.origin.y - popup.origin.y),
                1.0 + STACK_POPUP_INSET
            ),
            "popup inset y at {ui}"
        );
        assert!(
            near(
                f32::from(popup.right() - row.right()),
                1.0 + STACK_POPUP_INSET
            ),
            "popup row fills the card at {ui}"
        );
    }
}

#[gpui::test]
fn the_above_root_centres_its_card_over_the_anchor(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
    let (_, cx) = cx.add_window_view(|_, _| Host { ui: 16.0 });
    cx.run_until_parked();

    let anchor = bounds(cx, "anchor");
    let card = bounds(cx, "above-card");
    assert_eq!(anchor.size, gpui::size(px(0.0), px(0.0)), "zero-size root");
    assert!(
        near(f32::from(card.bottom()), f32::from(anchor.origin.y)),
        "card ends at the anchor: {card:?} over {anchor:?}"
    );
    let center = f32::from(card.origin.x) + f32::from(card.size.width) / 2.0;
    assert!(
        near(center, f32::from(anchor.origin.x)),
        "card centred on the anchor: {card:?} over {anchor:?}"
    );
}
