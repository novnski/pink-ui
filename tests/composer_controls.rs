//! The composer's primary controls at Interface sizes 12, 16 and 20 (Pink
//! composer.rs `send_button_mode`, `render_stop_button`, `stop_reserve`,
//! UI rethink D70), measured from the laid-out elements. Failure modes:
//! Stop under 28 pt at a small size; Stop overlapping the submit disc; a gap
//! other than `ICON_LABEL` scaled; Stop missing while a draft is typed during
//! a live run; Stop shown with no live run; Stop alone without the separate
//! Stop's width reserved, so the row jumps when typing starts.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::chat_composer::{ChatComposer, FollowUp},
};

struct Host {
    running: bool,
    draft: Option<&'static str>,
    steer: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let composer = ChatComposer::new("cc")
            .width(480.0)
            .running(self.running)
            .follow_up(FollowUp::Steer)
            .steer_available(self.steer);
        div().size_full().child(match self.draft {
            Some(text) => composer.draft(text),
            None => composer,
        })
    }
}

fn layout(
    cx: &mut TestAppContext,
    ui: f32,
    host: Host,
) -> (Option<Bounds<Pixels>>, Option<Bounds<Pixels>>) {
    cx.update(pink_ui::init);
    cx.update(|cx| {
        Theme::set_mode(ThemeMode::Light, cx);
        Theme::set_ui_font_size(ui, cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| host);
    // The draft lands on the first frame; the controls follow on the next.
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    (cx.debug_bounds("cc-stop"), cx.debug_bounds("cc-send"))
}

#[gpui::test]
fn stop_sits_beside_queue_and_steer(cx: &mut TestAppContext) {
    for ui in [12.0, 16.0, 20.0] {
        for steer in [false, true] {
            let mut cx = cx.clone();
            let host = Host {
                running: true,
                draft: Some("Also check the refund rate"),
                steer,
            };
            let (stop, send) = layout(&mut cx, ui, host);
            let (stop, send) = (stop.expect("Stop beside a draft"), send.expect("submit"));
            let scale = ui / 16.0;
            let size = f32::from(stop.size.width);
            assert!(size >= 28.0, "Stop {size} at {ui}");
            assert_eq!(size, (28.0 * scale).max(28.0), "Stop size at {ui}");
            assert_eq!(f32::from(stop.size.height), size);
            let gap = f32::from(send.origin.x) - f32::from(stop.origin.x + stop.size.width);
            assert!(gap >= 0.0, "Stop overlaps submit at {ui}: {gap}");
            assert!((gap - 6.0 * scale).abs() < 0.01, "gap {gap} at {ui}");
        }
    }
}

#[gpui::test]
fn stop_alone_keeps_its_room_and_idle_has_none(cx: &mut TestAppContext) {
    for ui in [12.0, 20.0] {
        let mut running = cx.clone();
        let (stop, send) = layout(
            &mut running,
            ui,
            Host {
                running: true,
                draft: None,
                steer: false,
            },
        );
        assert!(stop.is_none(), "an empty draft shows Stop alone");
        let send = send.expect("the stop disc");
        let mut drafted = cx.clone();
        let (stop_beside, _) = layout(
            &mut drafted,
            ui,
            Host {
                running: true,
                draft: Some("next"),
                steer: false,
            },
        );
        // Typing a draft brings the separate Stop into the room that was
        // already reserved: the submit disc does not move.
        let stop_beside = stop_beside.expect("Stop beside a draft");
        assert!(stop_beside.origin.x < send.origin.x);
        let mut idle = cx.clone();
        let (stop, send_idle) = layout(
            &mut idle,
            ui,
            Host {
                running: false,
                draft: Some("next"),
                steer: false,
            },
        );
        assert!(stop.is_none(), "no Stop without a live run");
        assert_eq!(
            send_idle.unwrap().origin.x,
            send.origin.x,
            "submit stays put"
        );
    }
}
