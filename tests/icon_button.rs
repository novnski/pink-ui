//! The `pink-icon-button` package's titlebar geometry at Interface sizes 12,
//! 16 and 20. Failure modes: a control under the 24 pt minimum hit at size
//! 12; a control over 32 pt at size 20 (the 38 pt band); the glyph off
//! centre; the three-button cluster's width drifting from the kit's
//! reservation (`layout::cluster_buttons_width`).

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::icon_button::{
        CONTROL_M, ICON_M, IconButtonStyle, RADIUS, icon_button, scaled_control, scaled_icon,
    },
    pink::{Pink, layout},
};

struct Host {
    scale: f32,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let scale = self.scale;
        let button = |id: &'static str| {
            icon_button(
                id,
                svg()
                    .path("icons/settings-page/arrow-left.svg")
                    .debug_selector(move || format!("{id}-glyph"))
                    .size(px(scaled_icon(ICON_M, scale)))
                    .into_any_element(),
                &IconButtonStyle {
                    size: scaled_control(CONTROL_M, scale),
                    radius: RADIUS,
                    wash: t.glass_hover().opacity(0.0),
                    enabled: true,
                },
            )
        };
        div()
            .flex()
            .flex_row()
            .child(button("toggle"))
            .child(div().w(px(layout::TITLEBAR_GROUP_GAP)))
            .child(button("back"))
            .child(div().w(px(layout::TITLEBAR_CONTROL_GAP)))
            .child(button("forward"))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    cx.debug_bounds(Box::leak(name.to_string().into_boxed_str()))
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn titlebar_buttons_scale_within_the_hit_floor_and_band(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for (ui, expected) in [(12.0, 24.0), (16.0, 28.0), (20.0, 32.0)] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| {
            Theme::set_mode(ThemeMode::Dark, cx);
            Theme::set_ui_font_size(ui, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Host { scale });
        cx.run_until_parked();
        for id in ["toggle", "back", "forward"] {
            let button = bounds(cx, id);
            assert_eq!(f32::from(button.size.width), expected, "{id} width at {ui}");
            assert_eq!(
                f32::from(button.size.height),
                expected,
                "{id} height at {ui}"
            );
            let glyph = bounds(cx, &format!("{id}-glyph"));
            let centre = |b: Bounds<Pixels>| {
                (
                    f32::from(b.origin.x + b.size.width / 2.0),
                    f32::from(b.origin.y + b.size.height / 2.0),
                )
            };
            assert_eq!(
                centre(glyph),
                centre(button),
                "{id} glyph off centre at {ui}"
            );
        }
        let left = bounds(cx, "toggle");
        let right = bounds(cx, "forward");
        let width = f32::from(right.origin.x + right.size.width - left.origin.x);
        assert_eq!(
            width,
            layout::cluster_buttons_width(scale),
            "cluster width at {ui}"
        );
    }
}
