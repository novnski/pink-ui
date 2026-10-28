//! The `pink-context-ring` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its context chip out in rems. Failure mode: the drawn
//! ring-only chip and `chip_widths` (which a host's controls row budgets
//! with) disagree, so the row gives the share up too late and a chip
//! truncates, or too early; the chip or ring not growing with the
//! Interface size.

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::context_ring::{ContextRingStyle, HEIGHT, chip_widths, context_ring},
    pink::Pink,
};

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div().flex().child(context_ring(
            "ring-only",
            Some(0.4),
            None,
            &ContextRingStyle {
                text: t.text_muted,
                arc: t.text_muted,
                track: t.text_faint,
                wash: gpui::transparent_black(),
            },
        ))
    }
}

#[gpui::test]
fn the_drawn_ring_chip_matches_the_width_a_row_budgets(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let chip = cx.debug_bounds("ring-only").expect("ring-only laid out");
        let (_, ring_only) = chip_widths(0.0, scale);
        // Layout snaps to whole pixels.
        assert!(
            (f32::from(chip.size.width) - ring_only).abs() <= 0.5,
            "width at {ui}: {:?} vs {ring_only}",
            chip.size
        );
        assert!(
            (f32::from(chip.size.height) - HEIGHT * scale).abs() <= 0.5,
            "height at {ui}: {:?}",
            chip.size
        );
    }
}
