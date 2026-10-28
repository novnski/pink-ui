//! The `pink-composer-disc` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its composer discs out in rems. Failure modes: a
//! disc that keeps one size at every Interface size, or stops being round
//! (a square Send); the glyph off centre; the stop square not growing with
//! the disc.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::composer_disc::{DiscStyle, GLYPH, STOP_GLYPH, disc, stop_glyph, ui_rems},
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
        let style = |fill| DiscStyle {
            size: ui_rems(28.0),
            fill,
            glyph: t.text,
            actionable: true,
        };
        div()
            .flex()
            .flex_row()
            .child(disc(
                "send",
                svg()
                    .path("icons/chat-composer/arrow-up.svg")
                    .debug_selector(|| "send-glyph".into())
                    .size(ui_rems(GLYPH))
                    .into_any_element(),
                &style(t.text),
            ))
            .child(disc(
                "stop",
                stop_glyph(t.text)
                    .debug_selector(|| "stop-glyph".into())
                    .into_any_element(),
                &style(t.text.opacity(0.08)),
            ))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn centre(b: Bounds<Pixels>) -> (f32, f32) {
    (
        f32::from(b.origin.x) + f32::from(b.size.width) / 2.0,
        f32::from(b.origin.y) + f32::from(b.size.height) / 2.0,
    )
}

#[gpui::test]
fn discs_grow_with_the_interface_size_and_centre_their_glyphs(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for (id, glyph, side) in [
            ("send", "send-glyph", GLYPH),
            ("stop", "stop-glyph", STOP_GLYPH),
        ] {
            let d = bounds(cx, id);
            assert_eq!(f32::from(d.size.width), 28.0 * scale, "{id} width at {ui}");
            assert_eq!(
                f32::from(d.size.height),
                28.0 * scale,
                "{id} height at {ui}"
            );
            let g = bounds(cx, glyph);
            // Layout snaps to whole pixels (11 pt at size 12 is 8.25).
            assert!(
                (f32::from(g.size.width) - side * scale).abs() <= 0.5,
                "{glyph} at {ui}: {:?}",
                g.size
            );
            let (dx, dy) = centre(d);
            let (gx, gy) = centre(g);
            assert!(
                (dx - gx).abs() < 0.51 && (dy - gy).abs() < 0.51,
                "{glyph} centred at {ui}"
            );
        }
    }
}
