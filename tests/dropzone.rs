//! The `pink-dropzone` package's drop veil at Interface sizes 12, 16 and 20,
//! as PINK lays it over its chat. Failure modes: a veil that takes part in
//! layout and pushes the surface's content down (it must float over it); a
//! veil that no longer covers its whole surface, so files dropped near an
//! edge miss it; a label that leaves the centre; a label that stops scaling
//! with the setting.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::dropzone::{VeilStyle, veil},
    pink::Pink,
};

const SURFACE_W: f32 = 360.0;
const SURFACE_H: f32 = 240.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        div().flex().flex_col().items_start().child(
            div()
                .debug_selector(|| "surface".into())
                .relative()
                .w(px(SURFACE_W))
                .h(px(SURFACE_H))
                .flex()
                .flex_col()
                .child(
                    div()
                        .debug_selector(|| "content".into())
                        .h(px(40.0))
                        .child("Transcript"),
                )
                .child(
                    veil(&VeilStyle {
                        scrim: t.scrim(0.4),
                        label: t.text,
                    })
                    .debug_selector(|| "veil".into())
                    .child(
                        div()
                            .debug_selector(|| "label".into())
                            .child("Drop to attach"),
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
    (a - b).abs() < 0.5
}

#[gpui::test]
fn drop_veil_covers_its_surface_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut labels = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let surface = bounds(cx, "surface");
        let veil = bounds(cx, "veil");
        assert_eq!(veil, surface, "veil covers its surface at {ui}");
        let content = bounds(cx, "content");
        assert_eq!(
            content.origin, surface.origin,
            "veil floats over the content at {ui}"
        );

        let label = bounds(cx, "label");
        let (lx, ly) = (
            f32::from(label.origin.x) + f32::from(label.size.width) / 2.0,
            f32::from(label.origin.y) + f32::from(label.size.height) / 2.0,
        );
        assert!(
            near(lx, f32::from(surface.origin.x) + SURFACE_W / 2.0)
                && near(ly, f32::from(surface.origin.y) + SURFACE_H / 2.0),
            "label centred at {ui}"
        );
        labels.push(f32::from(label.size.height));
    }
    assert!(
        labels[0] < labels[1] && labels[1] < labels[2],
        "label grows with the Interface size: {labels:?}"
    );
}
