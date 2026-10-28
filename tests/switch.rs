//! The `pink-switch` package's geometry at Interface sizes 12, 16 and 20.
//! Failure modes: a track or knob that stays fixed while the row's text
//! grows (a tiny switch at large sizes); a knob that leaves the track's
//! centre line or overshoots its inset at either end of its travel.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::switch::{KNOB, KNOB_INSET, SwitchStyle, TRACK_H, TRACK_W, knob, track},
    pink::Pink,
};

/// The progresses laid out, each with its track and knob selectors.
const AT: [(f32, &str, &str); 3] = [
    (0.0, "off", "off-knob"),
    (0.5, "half", "half-knob"),
    (1.0, "on", "on-knob"),
];

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`), and composes each switch as `switch` does.
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = SwitchStyle {
            track: t.accent_strong,
            knob: gpui::white(),
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .gap(gpui::px(8.0))
            .children(AT.map(|(progress, name, knob_name)| {
                track(&style)
                    .debug_selector(move || name.into())
                    .child(knob(&style, progress).debug_selector(move || knob_name.into()))
            }))
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
fn the_switch_grows_with_the_interface_size_and_travels_inside_its_inset(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        for (progress, name, knob_name) in AT {
            let track = bounds(cx, name);
            let knob = bounds(cx, knob_name);
            assert!(
                near(f32::from(track.size.width), TRACK_W * scale)
                    && near(f32::from(track.size.height), TRACK_H * scale),
                "{name} track at {ui}: {:?}",
                track.size
            );
            assert!(
                near(f32::from(knob.size.width), KNOB * scale)
                    && near(f32::from(knob.size.height), KNOB * scale),
                "{name} knob at {ui}: {:?}",
                knob.size
            );
            // Centred on the track's line at every point of its travel.
            assert!(
                near(
                    f32::from(knob.origin.y - track.origin.y),
                    (TRACK_H - KNOB) / 2.0 * scale
                ),
                "{name} knob centred at {ui}"
            );
            let travel = TRACK_W - KNOB - 2.0 * KNOB_INSET;
            assert!(
                near(
                    f32::from(knob.origin.x - track.origin.x),
                    (KNOB_INSET + travel * progress) * scale
                ),
                "{name} knob position at {ui}: knob {knob:?} track {track:?}"
            );
        }
    }
}
