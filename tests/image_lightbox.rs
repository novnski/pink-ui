//! The `pink-image-lightbox` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its lightbox over the window. Failure modes: an
//! image frame that grows with the Interface size instead of keeping 90% by
//! 85% of the window (the image would overflow a small window); a frame that
//! is no longer centred, or a caption that leaves its 12 px gap under it; a
//! long file name that pushes the caption wider than the frame instead of
//! cutting it to one line; a caption that stops scaling with the setting; a
//! close capsule that scales or drifts from its 16 px top right inset.
//!
//! The view geometry PINK's viewer and the story's place the image with:
//! a fit that upscales a small image or distorts a large one; a zoom that
//! lets the image slide out from under the pointer; a zoom or pan that runs
//! past its bounds (32x, the 0.01 floor, the image's overhang) or takes a
//! NaN; a resize that loses a zoom, or refits an image the user zoomed.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, point, prelude::*,
    size,
};
use pink_ui::{
    Theme, ThemeMode,
    components::image_lightbox::{
        CLOSE_INSET, CLOSE_SIZE, FRAME_HEIGHT, FRAME_WIDTH, GAP, LightboxStyle, MAX_ZOOM, MIN_ZOOM,
        ViewGeometry, caption, close_button, frame, layer, place,
    },
    pink::Pink,
};

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let viewport = window.viewport_size();
        let style = LightboxStyle {
            scrim: t.scrim(0.7),
            caption: t.ink(0.7),
        };
        div().size_full().child(place(
            layer(viewport, &style)
                .debug_selector(|| "layer".into())
                .child(frame(viewport).debug_selector(|| "frame".into()))
                .child(
                    caption(viewport, &style)
                        .debug_selector(|| "caption".into())
                        .child("checkout-coupon-error-".repeat(40) + ".png"),
                )
                .child(close_button().debug_selector(|| "close".into())),
        ))
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
fn lightbox_frame_follows_the_window_and_the_caption_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    let mut caption_heights = Vec::new();
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let layer = bounds(cx, "layer");
        let (w, h) = (f32::from(layer.size.width), f32::from(layer.size.height));
        assert!(w > 0.0 && h > 0.0, "layer covers the window at {ui}");

        let frame = bounds(cx, "frame");
        assert!(
            near(f32::from(frame.size.width), w * FRAME_WIDTH)
                && near(f32::from(frame.size.height), h * FRAME_HEIGHT),
            "frame is 90% by 85% of the window at {ui}: {frame:?} in {w}x{h}"
        );
        let frame_mid = f32::from(frame.origin.x) + f32::from(frame.size.width) / 2.0;
        assert!(near(frame_mid, w / 2.0), "frame centred at {ui}");

        let caption = bounds(cx, "caption");
        assert!(
            f32::from(caption.size.width) <= f32::from(frame.size.width) + 0.5,
            "a long name stays within the frame's width at {ui}: {caption:?}"
        );
        assert!(
            near(
                f32::from(caption.origin.y),
                f32::from(frame.origin.y + frame.size.height) + GAP
            ),
            "caption sits {GAP} px under the frame at {ui}"
        );
        caption_heights.push(f32::from(caption.size.height));

        let close = bounds(cx, "close");
        assert_eq!(
            (f32::from(close.size.width), f32::from(close.size.height)),
            (CLOSE_SIZE, CLOSE_SIZE),
            "close capsule keeps its size at {ui}"
        );
        assert!(
            near(f32::from(close.origin.y - layer.origin.y), CLOSE_INSET)
                && near(
                    f32::from(layer.origin.x + layer.size.width)
                        - f32::from(close.origin.x + close.size.width),
                    CLOSE_INSET
                ),
            "close capsule sits {CLOSE_INSET} px in from the top right at {ui}: {close:?}"
        );
    }
    assert!(
        caption_heights[0] < caption_heights[1] && caption_heights[1] < caption_heights[2],
        "the caption is one line that grows with the Interface size: {caption_heights:?}"
    );
}

/// A 1000x500 image in a 500x300 viewer.
fn geometry() -> ViewGeometry {
    let mut g = ViewGeometry::default();
    g.resize(size(1000.0, 500.0), size(500.0, 300.0));
    g
}

#[test]
fn view_fit_keeps_the_aspect_ratio_and_never_upscales() {
    let mut g = geometry();
    assert_eq!(g.scale, 0.5);
    assert_eq!(g.image_origin(), point(0.0, 25.0));
    assert!(g.contains(point(250.0, 150.0)) && !g.contains(point(250.0, 10.0)));
    g.resize(size(100.0, 50.0), size(500.0, 300.0));
    assert_eq!(g.scale, 1.0);
    assert_eq!(g.image_origin(), point(200.0, 125.0));
}

#[test]
fn view_zoom_keeps_the_pointer_over_the_same_image_point() {
    let mut g = geometry();
    let anchor = point(100.0, 150.0);
    let before = (anchor - g.image_origin()) / g.scale;
    g.zoom(2.0, anchor);
    let after = (anchor - g.image_origin()) / g.scale;
    assert!((before.x - after.x).abs() < 0.001);
    assert!((before.y - after.y).abs() < 0.001);
}

#[test]
fn view_pan_zoom_and_resize_stay_bounded() {
    let mut g = geometry();
    g.zoom(1e9, point(250.0, 150.0));
    assert_eq!(g.scale, MAX_ZOOM);
    g.pan_by(point(1e9, -1e9));
    assert_eq!(g.pan, point(15750.0, -7850.0));
    assert!(
        !g.pan_by(point(1.0, -1.0)),
        "a pan at the edge does not move"
    );
    g.zoom(1e-9, point(250.0, 150.0));
    assert_eq!(g.scale, MIN_ZOOM);
    assert_eq!(g.pan, point(0.0, 0.0));
    g.zoom(f32::NAN, point(0.0, 0.0));
    assert_eq!(g.scale, MIN_ZOOM);
    g.fit();
    g.resize(size(1000.0, 500.0), size(250.0, 200.0));
    assert_eq!(g.scale, 0.25, "a fitted image refits");
    g.zoom(1.0, point(125.0, 100.0));
    g.resize(size(1000.0, 500.0), size(300.0, 200.0));
    assert_eq!(g.scale, 1.0, "a zoomed image keeps its zoom");
}
