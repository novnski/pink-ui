//! The `pink-menu-card` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays its menus out. Failure modes: row padding that stays fixed
//! while the text grows (cramped rows at large sizes); a card inset or row
//! gap that scales and pulls rows off the card's concentric corners; a
//! border that shifts the rows.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::menu_card::{
        CARD_INSET, CardStyle, MENU_GAP, ROW_PAD_X, ROW_PAD_Y, RowStyle, card, row, ui_rems,
    },
    pink::Pink,
};

/// The mark's side inside each row, in pt at the 16 px default.
const MARK: f32 = 16.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let item = |n: &'static str, mark: &'static str| {
            row(&RowStyle {
                text: t.text,
                fill: t.element_active,
            })
            .debug_selector(move || n.into())
            .child(
                div()
                    .debug_selector(move || mark.into())
                    .flex_none()
                    .size(ui_rems(MARK)),
            )
        };
        div().w(gpui::px(240.0)).child(
            card(&CardStyle {
                border: t.border,
                background: t.surface_raised,
                text: t.text,
                font_family: pink_ui::pink::SANS.into(),
                shadows: None,
            })
            .debug_selector(|| "card".into())
            .flex()
            .flex_col()
            .child(item("first", "first-mark"))
            .child(item("second", "second-mark")),
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
fn rows_grow_with_the_interface_size_inside_a_fixed_card_inset(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let c = bounds(cx, "card");
        let first = bounds(cx, "first");
        let second = bounds(cx, "second");
        let mark = bounds(cx, "first-mark");

        // The 1 px border plus the inset, at every size.
        let inset = 1.0 + CARD_INSET;
        assert!(
            near(f32::from(first.origin.x - c.origin.x), inset),
            "row inset x at {ui}"
        );
        assert!(
            near(f32::from(first.origin.y - c.origin.y), inset),
            "row inset y at {ui}"
        );
        assert!(
            near(f32::from(c.right() - first.right()), inset),
            "row fills the card at {ui}"
        );
        assert!(
            near(f32::from(second.origin.y - first.bottom()), MENU_GAP),
            "row gap at {ui}"
        );

        // The row's padding grows with the Interface size.
        assert!(
            near(
                f32::from(first.size.height),
                (MARK + 2.0 * ROW_PAD_Y) * scale
            ),
            "row height at {ui}: {:?}",
            first.size
        );
        assert!(
            near(f32::from(mark.origin.x - first.origin.x), ROW_PAD_X * scale),
            "row padding at {ui}"
        );
    }
}
