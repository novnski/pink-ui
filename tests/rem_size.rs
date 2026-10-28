//! The gallery's hosts make the window's rem the Interface size, as PINK's
//! root does. Failure mode: the rem-sized packages (chips, rings, menus, the
//! pill) stay at their 16 px size while the theme's Interface size changes,
//! so their text and geometry drift off the scaled rows around them.

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::context_ring::{ContextRingStyle, HEIGHT, context_ring},
    pink::Pink,
};

struct Host;

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        Theme::sync_rem_size(window, cx);
        let t = Pink::get(cx);
        div().flex().child(context_ring(
            "ring",
            Some(0.4),
            None,
            &ContextRingStyle {
                text: t.text_muted,
                arc: t.text_muted,
                track: t.text_faint,
                wash: gpui::transparent_black(),
            },
        ))
    }
}

#[gpui::test]
fn rem_sized_packages_follow_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| {
            Theme::set_mode(ThemeMode::Dark, cx);
            Theme::set_ui_font_size(ui, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Host);
        cx.run_until_parked();
        let ring = cx.debug_bounds("ring").expect("ring laid out");
        assert!(
            (f32::from(ring.size.height) - HEIGHT * ui / 16.0).abs() <= 0.5,
            "chip height at Interface size {ui}: {:?}",
            ring.size
        );
    }
}
