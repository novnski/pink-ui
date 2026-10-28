//! The `pink-toast` package's geometry at Interface sizes 12, 16 and 20, as
//! PINK floats its feedback toast above the composer. Failure modes: a
//! capsule that grows with the Interface size (PINK's toast stays 34 px; only
//! its copy scales); a long message pushing the capsule past its 520 px cap
//! instead of truncating; a Dismiss control that loses its 24 px circle; a
//! lane that no longer sits `TOAST_HEIGHT + ANCHOR_GAP` above its anchor or
//! stops centering the capsule over it.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::toaster::package::{
        ANCHOR_GAP, DISMISS_SIZE, MAX_WIDTH, TOAST_HEIGHT, ToastStyle, capsule, dismiss, lane,
        message,
    },
    pink::{self, Pink},
};

/// The anchor's top edge and width, in px.
const ANCHOR_TOP: f32 = 200.0;
const ANCHOR_WIDTH: f32 = 900.0;

struct Host {
    ui: f32,
    long: bool,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let long = self.long;
        div().pt(gpui::px(ANCHOR_TOP)).child(
            div()
                .debug_selector(|| "anchor".into())
                .relative()
                .w(gpui::px(ANCHOR_WIDTH))
                .h(gpui::px(80.0))
                .child(
                    lane().child(
                        capsule(&ToastStyle {
                            surface: t.tooltip_surface(),
                            edge: t.hairline(0.10),
                            text: t.text,
                            shadows: t.menu_shadows(),
                            font: pink::SANS.into(),
                        })
                        .debug_selector(|| "capsule".into())
                        .child(
                            message("PINK task link copied")
                                // A message wider than the cap, whatever the
                                // test platform's text measures.
                                .when(long, |el| el.child(div().w(gpui::px(2000.0)))),
                        )
                        .child(dismiss().debug_selector(|| "dismiss".into())),
                    ),
                ),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn the_toast_keeps_its_height_and_cap_and_floats_over_its_anchor(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        for long in [false, true] {
            let mut cx = cx.clone();
            cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
            let (_, cx) = cx.add_window_view(|_, _| Host { ui, long });
            cx.run_until_parked();

            let capsule = bounds(cx, "capsule");
            assert_eq!(
                f32::from(capsule.size.height),
                TOAST_HEIGHT,
                "height at {ui}"
            );
            if long {
                assert_eq!(f32::from(capsule.size.width), MAX_WIDTH, "cap at {ui}");
            } else {
                assert!(f32::from(capsule.size.width) < MAX_WIDTH, "short at {ui}");
            }

            let dismiss = bounds(cx, "dismiss");
            assert_eq!(
                f32::from(dismiss.size.width),
                DISMISS_SIZE,
                "dismiss at {ui}"
            );
            assert_eq!(
                f32::from(dismiss.size.height),
                DISMISS_SIZE,
                "dismiss at {ui}"
            );

            let anchor = bounds(cx, "anchor");
            assert_eq!(
                f32::from(anchor.origin.y - capsule.origin.y),
                TOAST_HEIGHT + ANCHOR_GAP,
                "lane above the anchor at {ui}"
            );
            let center = |b: Bounds<Pixels>| f32::from(b.origin.x) + f32::from(b.size.width) / 2.0;
            assert!(
                (center(anchor) - center(capsule)).abs() < 0.5,
                "centered at {ui}"
            );
        }
    }
}
