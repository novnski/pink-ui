//! The `pink-loaders` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays its loaders out. Failure modes: spinner cells or gaps that
//! scale with the Interface size when PINK keeps them fixed pixels (a
//! running row's glyph outgrowing its 14 pt slot at large sizes); a mini
//! spinner that draws outside the box its host's cached view reserves
//! (`mini_spinner_box`), clipping or shifting the row; an upload ring that
//! stays fixed while the thumbnail and its percent grow; a splash layer
//! that stops covering the window it fades over.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, hsla, prelude::*,
};
use pink_ui::components::loaders::{
    gradient_spinner, mini_spinner, mini_spinner_box, splash_overlay, upload_progress_ring,
};

const MATRIX_CELL: f32 = 3.0;
const MINI_CELL: f32 = 2.0;
const RING: f32 = 34.0;
const SPLASH_W: f32 = 320.0;
const SPLASH_H: f32 = 200.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let tint = hsla(0.69, 0.88, 0.73, 1.0);
        let tints = [tint; 3];
        let pulse = |t: f32| t.rem_euclid(1.0);
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                gradient_spinner(MATRIX_CELL, tints, 0.25, pulse)
                    .debug_selector(|| "matrix".into()),
            )
            .child(mini_spinner(MINI_CELL, tints, 0.25, pulse).debug_selector(|| "mini".into()))
            .child(upload_progress_ring(64, RING).debug_selector(|| "ring".into()))
            .child(
                div()
                    .relative()
                    .w(gpui::px(SPLASH_W))
                    .h(gpui::px(SPLASH_H))
                    .debug_selector(|| "splash-host".into())
                    .child(
                        splash_overlay(
                            tint,
                            tints,
                            0.25,
                            pulse,
                            "Setting up PINK environment",
                            tint,
                        )
                        .debug_selector(|| "splash".into()),
                    ),
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
fn loaders_keep_pinks_proportions_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // The matrix: three fixed cells and two half-cell gaps a side.
        let matrix = bounds(cx, "matrix");
        for (axis, len) in [("width", matrix.size.width), ("height", matrix.size.height)] {
            assert!(
                near(f32::from(len), 4.0 * MATRIX_CELL),
                "fixed matrix {axis} at {ui}: {:?}",
                matrix.size
            );
        }

        // The mini spinner fills exactly the box its host reserves.
        let mini = bounds(cx, "mini");
        let (w, h) = mini_spinner_box(MINI_CELL);
        assert!(
            near(f32::from(mini.size.width), w) && near(f32::from(mini.size.height), h),
            "mini spinner fills its {w}x{h} box at {ui}: {:?}",
            mini.size
        );

        // The ring grows with the Interface size.
        let ring = bounds(cx, "ring");
        for (axis, len) in [("width", ring.size.width), ("height", ring.size.height)] {
            assert!(
                near(f32::from(len), RING * scale),
                "scaled ring {axis} at {ui}: {:?}",
                ring.size
            );
        }

        // The splash covers the window it sits in.
        let host = bounds(cx, "splash-host");
        let splash = bounds(cx, "splash");
        assert!(
            near(f32::from(splash.origin.x), f32::from(host.origin.x))
                && near(f32::from(splash.origin.y), f32::from(host.origin.y))
                && near(f32::from(splash.size.width), SPLASH_W)
                && near(f32::from(splash.size.height), SPLASH_H),
            "splash covers its host at {ui}: {splash:?} in {host:?}"
        );
    }
}
