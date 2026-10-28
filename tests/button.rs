//! The `pink-button` package's control geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its controls out. Failure modes: a height, pad or gap
//! fixed in pixels while the label's type grows (clipped labels at size 20,
//! loose controls at 12); the 1 px edge shifting content; a glyph square
//! that is not square; a pop-up button whose chevron side is not 2 pt
//! tighter than its label side; a disabled control changing size.

use gpui::{
    Bounds, Context, FontWeight, IntoElement, Pixels, Render, TestAppContext, Window, div,
    prelude::*,
};
use pink_ui::{
    Theme, ThemeMode,
    components::button::{
        ControlSize, ControlStyle, POPUP_GAP, POPUP_TRAIL_TIGHTEN, button, popup, square, ui_rems,
    },
    pink::Pink,
};

const SIZES: [(ControlSize, &str); 3] = [
    (ControlSize::Sm, "sm"),
    (ControlSize::Md, "md"),
    (ControlSize::Lg, "lg"),
];

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

fn mark(name: String, side: f32) -> gpui::Div {
    div()
        .debug_selector(move || name)
        .flex_none()
        .size(ui_rems(side))
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = |size, enabled| ControlStyle {
            size,
            fill: Some(t.text),
            text: t.on_solid,
            border: t.separator(true),
            weight: FontWeight::MEDIUM,
            enabled,
        };
        let mut column = div().flex().flex_col().items_start().gap(gpui::px(4.0));
        for (size, n) in SIZES {
            let glyph = size.glyph();
            column = column
                .child(
                    button(format!("{n}-button"), &style(size, true))
                        .debug_selector(move || format!("{n}-button"))
                        .child(mark(format!("{n}-glyph"), glyph))
                        .child(mark(format!("{n}-label"), 40.0)),
                )
                .child(
                    button(format!("{n}-disabled"), &style(size, false))
                        .debug_selector(move || format!("{n}-disabled"))
                        .child(mark(format!("{n}-disabled-glyph"), glyph))
                        .child(mark(format!("{n}-disabled-label"), 40.0)),
                )
                .child(
                    square(format!("{n}-square"), &style(size, true))
                        .debug_selector(move || format!("{n}-square"))
                        .child(mark(format!("{n}-square-glyph"), glyph)),
                )
                .child(
                    popup(format!("{n}-popup"), &style(size, true))
                        .debug_selector(move || format!("{n}-popup"))
                        .w(ui_rems(160.0))
                        .child(
                            div()
                                .debug_selector(move || format!("{n}-choice"))
                                .flex_1()
                                .h(ui_rems(16.0)),
                        )
                        .child(mark(format!("{n}-chevron"), 14.0)),
                );
        }
        column
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: String) -> Bounds<Pixels> {
    cx.debug_bounds(Box::leak(name.clone().into_boxed_str()))
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: Pixels, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (f32::from(a) - b).abs() <= 0.5
}

#[gpui::test]
fn controls_grow_with_the_interface_size_inside_a_fixed_edge(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for (size, n) in SIZES {
            let at = format!("{n} at {ui}");
            let height = size.height() * scale;
            let b = bounds(cx, format!("{n}-button"));
            let glyph = bounds(cx, format!("{n}-glyph"));
            let label = bounds(cx, format!("{n}-label"));
            assert!(near(b.size.height, height), "button height {at}");
            // The edge is a fixed hairline; the side pad grows.
            assert!(
                near(glyph.origin.x - b.origin.x, 1.0 + size.pad_x() * scale),
                "leading pad {at}"
            );
            assert!(
                near(b.right() - label.right(), 1.0 + size.pad_x() * scale),
                "trailing pad {at}"
            );
            assert!(
                near(label.origin.x - glyph.right(), size.gap() * scale),
                "glyph gap {at}"
            );
            assert!(
                near(
                    glyph.origin.y - b.origin.y,
                    (height - size.glyph() * scale) / 2.0
                ),
                "glyph centred {at}"
            );

            let disabled = bounds(cx, format!("{n}-disabled"));
            assert!(
                near(disabled.size.width, f32::from(b.size.width))
                    && near(disabled.size.height, height),
                "disabled keeps its size {at}"
            );

            let sq = bounds(cx, format!("{n}-square"));
            let sq_glyph = bounds(cx, format!("{n}-square-glyph"));
            assert!(
                near(sq.size.width, height) && near(sq.size.height, height),
                "square {at}: {:?}",
                sq.size
            );
            assert!(
                near(
                    sq_glyph.origin.x - sq.origin.x,
                    (height - size.glyph() * scale) / 2.0
                ),
                "square glyph centred {at}"
            );

            let pop = bounds(cx, format!("{n}-popup"));
            let choice = bounds(cx, format!("{n}-choice"));
            let chevron = bounds(cx, format!("{n}-chevron"));
            assert!(near(pop.size.height, height), "popup height {at}");
            assert!(
                near(choice.origin.x - pop.origin.x, 1.0 + size.pad_x() * scale),
                "popup leading pad {at}"
            );
            assert!(
                near(
                    pop.right() - chevron.right(),
                    1.0 + (size.pad_x() - POPUP_TRAIL_TIGHTEN) * scale
                ),
                "popup chevron side {at}"
            );
            assert!(
                near(chevron.origin.x - choice.right(), POPUP_GAP * scale),
                "popup gap {at}"
            );
        }
    }
}
