//! The `pink-composer-pill` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its composer out. Failure modes: a hairline that scales
//! with the Interface size and throws off PINK's fixed `BORDER_V` height
//! budget; content not inset by exactly the hairline; corners that don't
//! tighten as the pill docks.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, rems,
};
use pink_ui::{
    Theme, ThemeMode,
    components::composer_pill::{BORDER_V, DOCK_TIGHTEN, PillStyle, RADIUS, pill, radius},
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
        // Sized to its content, as PINK's composer column places it.
        div().flex().items_start().child(
            pill(&PillStyle {
                radius: radius(0.0),
                border: t.border,
                background: t.surface_raised,
                lifted: true,
            })
            .debug_selector(|| "pill".into())
            .child(
                div()
                    .debug_selector(|| "content".into())
                    .w(rems(20.0))
                    .h(rems(5.0)),
            ),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn the_hairline_stays_one_pixel_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let p = bounds(cx, "pill");
        let c = bounds(cx, "content");
        assert_eq!(
            f32::from(p.size.height - c.size.height),
            BORDER_V,
            "hairline height at {ui}"
        );
        assert_eq!(
            f32::from(p.size.width - c.size.width),
            BORDER_V,
            "hairline width at {ui}"
        );
        assert_eq!(f32::from(c.origin.x - p.origin.x), 1.0, "inset at {ui}");
    }
}

#[test]
fn corners_tighten_as_the_pill_docks() {
    assert_eq!(radius(0.0), RADIUS);
    assert_eq!(radius(0.5), RADIUS - DOCK_TIGHTEN / 2.0);
    assert_eq!(radius(1.0), RADIUS - DOCK_TIGHTEN);
}
