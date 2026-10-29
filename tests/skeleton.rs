//! The `pink-skeleton` package's geometry at Interface sizes 12, 16 and 20,
//! and its pulse. Failure modes: rows that stay at their 16 px height while
//! the real rows they stand in for grow; menu rows that keep the list rows'
//! slab height or spacing; a ghost label that does not scale with
//! the chip label it replaces; a stagger that runs upward or escapes 0..1,
//! so rows jump instead of rippling down.

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::skeleton::{
        BAR_HEIGHT, MENU_PAD_Y, MENU_ROW_GAP, MENU_ROW_HEIGHT, OPACITY_REST, OPACITY_SWING,
        ROW_GAP, ROW_HEIGHT, ROWS_PAD_Y, STAGGER, bar, menu_rows, opacity, row_phase, rows,
    },
    pink::Pink,
};

const MENU_W: f32 = 200.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let wash = Pink::get(cx).text_muted;
        let wave = |phase: f32| phase;
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .w(gpui::px(MENU_W))
                    .child(rows(2, wash, 0.0, wave).debug_selector(|| "rows".into())),
            )
            .child(
                div()
                    .w(gpui::px(MENU_W))
                    .child(menu_rows(2, wash, 0.0, wave).debug_selector(|| "menu".into())),
            )
            .child(
                div()
                    .flex()
                    .child(bar(56.0, wash, 0.0, wave).debug_selector(|| "bar".into())),
            )
    }
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn skeletons_grow_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let list = cx.debug_bounds("rows").expect("rows laid out");
        assert!(
            near(
                f32::from(list.size.height),
                (2.0 * ROW_HEIGHT + ROW_GAP + 2.0 * ROWS_PAD_Y) * scale
            ),
            "list rows at {ui}: {:?}",
            list.size
        );

        let menu = cx.debug_bounds("menu").expect("menu rows laid out");
        assert!(
            near(
                f32::from(menu.size.height),
                (2.0 * MENU_ROW_HEIGHT + MENU_ROW_GAP + 2.0 * MENU_PAD_Y) * scale
            ),
            "menu rows at {ui}: {:?}",
            menu.size
        );
        let bar_bounds = cx.debug_bounds("bar").expect("ghost label laid out");
        assert!(
            near(f32::from(bar_bounds.size.width), 56.0 * scale),
            "ghost label width at {ui}"
        );
        assert!(
            near(f32::from(bar_bounds.size.height), BAR_HEIGHT * scale),
            "ghost label height at {ui}"
        );
    }
}

#[test]
fn the_pulse_ripples_down_the_rows_inside_its_range() {
    // Each row trails the one above by `STAGGER`, wrapping into 0..1.
    assert_eq!(row_phase(0.5, 0), 0.5);
    assert!((row_phase(0.5, 1) - (0.5 - STAGGER)).abs() < 1e-6);
    assert!((row_phase(0.02, 1) - (1.0 + 0.02 - STAGGER)).abs() < 1e-6);
    for i in 0..40 {
        let phase = row_phase(0.3, i);
        assert!((0.0..1.0).contains(&phase), "row {i} phase {phase}");
    }
    assert_eq!(opacity(0.0), OPACITY_REST);
    assert!((opacity(1.0) - (OPACITY_REST + OPACITY_SWING)).abs() < 1e-6);
}
