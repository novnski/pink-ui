//! The `pink-settings-section` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its settings pages out. Failure modes: header or row
//! padding that stays fixed while the text grows (cramped cards at large
//! sizes); a text column that gives up its minimum width and lets the
//! controls squeeze it instead of wrapping under it; a divider or card
//! border that scales or shifts the rows; a footer that drifts off the
//! card's content edge.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::settings_section::{
        CardStyle, FOOTER_GAP, HEADER_PAD_BOTTOM, HEADER_PAD_TOP, HEADER_TITLE_LINE, ROW_GAP_Y,
        ROW_PAD_X, ROW_PAD_Y, ROW_TEXT_MIN, card_row, header_text, header_title, row_controls,
        row_text, row_title, section_card, section_footer, section_header, ui_rems,
    },
    pink::Pink,
};

/// The trailing control's side, in pt at the 16 px default: taller than a
/// row's minimum content so the row's own padding sets its height.
const CONTROL: f32 = 40.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
    width: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div()
            .w(gpui::px(self.width))
            .flex()
            .flex_col()
            .child(
                section_card(&CardStyle {
                    border: t.border,
                    background: t.card_glass_bg(),
                })
                .debug_selector(|| "card".into())
                .child(
                    section_header().debug_selector(|| "header".into()).child(
                        header_text()
                            .debug_selector(|| "header-text".into())
                            .child(header_title(t.text, "Sessions")),
                    ),
                )
                .child(
                    card_row(Some(t.border))
                        .debug_selector(|| "row".into())
                        .child(
                            row_text()
                                .debug_selector(|| "row-text".into())
                                .child(row_title(t.text, "Keep the Mac awake")),
                        )
                        .child(
                            row_controls().child(
                                div()
                                    .debug_selector(|| "control".into())
                                    .flex_none()
                                    .size(ui_rems(CONTROL)),
                            ),
                        ),
                ),
            )
            .child(
                section_footer(t.text_muted, "Saved on this device")
                    .debug_selector(|| "footer".into()),
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
fn header_and_rows_grow_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui, width: 560.0 });
        cx.run_until_parked();

        let card = bounds(cx, "card");
        let header = bounds(cx, "header");
        let header_text = bounds(cx, "header-text");
        let row = bounds(cx, "row");
        let row_text = bounds(cx, "row-text");
        let control = bounds(cx, "control");
        let footer = bounds(cx, "footer");

        // The 1 px card border stays a pixel; the padding inside it scales.
        assert!(
            near(
                f32::from(header_text.origin.x - card.origin.x),
                1.0 + ROW_PAD_X * scale
            ),
            "header inset x at {ui}"
        );
        assert!(
            near(
                f32::from(header_text.origin.y - card.origin.y),
                1.0 + HEADER_PAD_TOP * scale
            ),
            "header inset y at {ui}"
        );
        assert!(
            (f32::from(header.size.height)
                - (HEADER_PAD_TOP + HEADER_TITLE_LINE + HEADER_PAD_BOTTOM) * scale)
                .abs()
                <= 1.0,
            "header height at {ui}: {:?}",
            header.size
        );

        // The row sits under the header behind a 1 px hairline, padded by
        // the scaled row padding, its control at the content's right edge.
        assert!(near(f32::from(row.origin.y), f32::from(header.bottom())));
        assert!(
            near(
                f32::from(control.origin.y - row.origin.y),
                1.0 + ROW_PAD_Y * scale
            ),
            "row padding y at {ui}"
        );
        assert!(
            near(
                f32::from(row.size.height),
                1.0 + (2.0 * ROW_PAD_Y + CONTROL) * scale
            ),
            "row height at {ui}: {:?}",
            row.size
        );
        assert!(
            near(
                f32::from(row_text.origin.x - row.origin.x),
                ROW_PAD_X * scale
            ),
            "row padding x at {ui}"
        );
        assert!(
            near(
                f32::from(card.right() - control.right()),
                1.0 + ROW_PAD_X * scale
            ),
            "controls at the right edge at {ui}"
        );
        assert!(
            near(f32::from(control.origin.y), f32::from(row_text.origin.y))
                || f32::from(control.origin.y) < f32::from(row_text.bottom()),
            "controls beside the text at {ui}"
        );

        // The footer hangs under the card on its content edge.
        assert!(
            near(
                f32::from(footer.origin.y - card.bottom()),
                FOOTER_GAP * scale
            ),
            "footer gap at {ui}"
        );
        // Its text starts where the rows' text does, less the card's 1 px
        // border it doesn't have.
        let footer_text_x = f32::from(footer.origin.x) + ROW_PAD_X * scale;
        assert!(
            near(footer_text_x + 1.0, f32::from(row_text.origin.x)),
            "footer aligned with the rows at {ui}"
        );
    }
}

#[gpui::test]
fn a_narrow_row_wraps_its_controls_under_the_text(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let ui = 20.0_f32;
    let scale = ui / 16.0;
    cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
    let (_, cx) = cx.add_window_view(|_, _| Host { ui, width: 320.0 });
    cx.run_until_parked();

    let row_text = bounds(cx, "row-text");
    let control = bounds(cx, "control");
    assert!(
        f32::from(row_text.size.width) >= ROW_TEXT_MIN * scale - 0.5,
        "the text keeps its minimum width: {:?}",
        row_text.size
    );
    assert!(
        near(
            f32::from(control.origin.y - row_text.bottom()),
            ROW_GAP_Y * scale
        ),
        "the controls wrap a row gap under the text: text {row_text:?}, control {control:?}"
    );
}
