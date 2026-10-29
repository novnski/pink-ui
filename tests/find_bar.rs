//! The `pink-find-bar` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK floats its conversation Find bar. Failure modes: a plate that
//! stops growing with the Interface size (its status and buttons clip); a
//! bar that drifts from `BAR_RIGHT` off the conversation's right edge; a
//! magnifier that stops scaling with the query beside it; a query field
//! that stops taking the room the status and buttons leave.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::find_bar::chrome::{
        BAR_RIGHT, BAR_WIDTH, GLYPH, anchor, field, glyph, plate, status,
    },
};

const WIDTH: f32 = 700.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let black = gpui::black();
        div()
            .w(px(WIDTH))
            .h(px(200.0))
            .relative()
            .debug_selector(|| "conversation".into())
            .child(
                anchor(8.0).child(
                    plate(div(), gpui::white())
                        .debug_selector(|| "plate".into())
                        .child(glyph(gpui::svg(), black).debug_selector(|| "glyph".into()))
                        .child(field(div().h(px(18.0))).debug_selector(|| "field".into()))
                        .child(status("3 of 7", black).debug_selector(|| "status".into())),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn find_bar_scales_and_stays_anchored_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let conversation = bounds(cx, "conversation");
        let plate = bounds(cx, "plate");
        let glyph = bounds(cx, "glyph");
        let field = bounds(cx, "field");
        let status = bounds(cx, "status");

        assert!(
            near(f32::from(plate.size.width), BAR_WIDTH * scale),
            "plate width at {ui}: {plate:?}"
        );
        assert!(
            near(
                f32::from(
                    conversation.origin.x + conversation.size.width
                        - (plate.origin.x + plate.size.width)
                ),
                BAR_RIGHT
            ),
            "anchor at {ui}: {plate:?} in {conversation:?}"
        );
        assert!(
            near(f32::from(glyph.size.width), GLYPH * scale),
            "glyph at {ui}: {glyph:?}"
        );
        // The field fills the room between the glyph and the status.
        assert!(
            f32::from(field.size.width) > 0.0
                && f32::from(field.origin.x) > f32::from(glyph.origin.x + glyph.size.width)
                && f32::from(field.origin.x + field.size.width) < f32::from(status.origin.x),
            "field at {ui}: {field:?} between {glyph:?} and {status:?}"
        );
    }
}
