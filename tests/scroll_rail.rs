//! The `pink-scroll-rail` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays a menu's rail out. Failure modes: a strip that does not
//! hug the viewport's right edge or full height (the hover target drifts off
//! the thumb); a thumb placed from the viewport's top instead of the track's
//! inset; a thumb that grows with the Interface size or widens outside its
//! fixed strip (rows reflow on hover).

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::scroll_rail::{
        ACTIVE_THUMB_WIDTH, HIT_WIDTH, THUMB_INSET, THUMB_WIDTH, TRACK_INSET, rail,
    },
    pink::Pink,
};

const VIEW_W: f32 = 220.0;
const VIEW_H: f32 = 180.0;
const THUMB_TOP: f32 = 18.0;
const THUMB_H: f32 = 64.0;

struct Host {
    ui: f32,
    active: bool,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let faint = Pink::get(cx).text_faint;
        div().child(
            div()
                .debug_selector(|| "viewport".into())
                .relative()
                .w(gpui::px(VIEW_W))
                .h(gpui::px(VIEW_H))
                .child(
                    rail(faint, 1.0, self.active, THUMB_TOP, THUMB_H)
                        .debug_selector(|| "rail".into()),
                ),
        )
    }
}

fn near(a: f32, b: f32) -> bool {
    // Layout snaps to whole pixels.
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn the_rail_hugs_the_right_edge_at_a_fixed_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        for active in [false, true] {
            let mut cx = cx.clone();
            cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
            let (_, cx) = cx.add_window_view(|_, _| Host { ui, active });
            cx.run_until_parked();

            let view = cx.debug_bounds("viewport").expect("viewport laid out");
            let strip = cx.debug_bounds("rail").expect("rail laid out");
            assert!(
                near(f32::from(view.right() - strip.right()), 0.0)
                    && near(f32::from(strip.size.width), HIT_WIDTH),
                "strip on the right edge at {ui}: {strip:?} in {view:?}"
            );
            assert!(
                near(f32::from(strip.origin.y), f32::from(view.origin.y))
                    && near(f32::from(strip.size.height), VIEW_H),
                "strip spans the viewport at {ui}"
            );

            let thumb = cx.debug_bounds("scrollbar-thumb").expect("thumb laid out");
            let width = if active {
                ACTIVE_THUMB_WIDTH
            } else {
                THUMB_WIDTH
            };
            assert!(
                near(
                    f32::from(thumb.origin.y - view.origin.y),
                    TRACK_INSET + THUMB_TOP
                ),
                "thumb top at {ui}"
            );
            assert!(
                near(f32::from(thumb.size.height), THUMB_H)
                    && near(f32::from(thumb.size.width), width),
                "thumb size at {ui}, active {active}: {thumb:?}"
            );
            assert!(
                near(f32::from(view.right() - thumb.right()), THUMB_INSET),
                "thumb inset at {ui}"
            );
        }
    }
}
