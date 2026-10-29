//! The composer's primary controls at Interface sizes 12, 16 and 20 (Pink
//! composer.rs `send_button_mode`, `render_stop_button`, `stop_reserve`,
//! UI rethink D70), measured from the laid-out elements. Failure modes:
//! Stop under 28 pt at a small size; Stop overlapping the submit disc; a gap
//! other than `ICON_LABEL` scaled; Stop missing while a draft is typed during
//! a live run; Stop shown with no live run; Stop alone without the separate
//! Stop's width reserved, so the row jumps when typing starts.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::chat_composer::{Attachment, ChatComposer, FollowUp},
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

struct Staged;

impl Render for Staged {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().pt(px(80.)).pl(px(40.)).child(
            ChatComposer::new("cc")
                .width(480.0)
                .attachments(vec![Attachment::image("chart.png", "PNG image, 1 KB")]),
        )
    }
}

/// A staged attachment's remove button keeps the 24 pt minimum target
/// (`pink::chrome::MIN_HIT`) around the package's 20 pt disc, centred where
/// the disc overhangs the square's corner by 7 pt, at Interface sizes 12, 16
/// and 20. Failure mode: the click target shrinks to the drawn disc, so a
/// press in the outer ring of the old target misses the hover-revealed
/// action, or the disc drifts off the corner.
#[gpui::test]
fn remove_attachment_keeps_its_minimum_target(cx: &mut TestAppContext) {
    for ui in [12.0, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(pink_ui::init);
        cx.update(|cx| {
            Theme::set_mode(ThemeMode::Light, cx);
            Theme::set_ui_font_size(ui, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Staged);
        cx.run_until_parked();
        let thumb = cx.debug_bounds("cc-thumb-0").expect("the staged square");
        let target = cx.debug_bounds("cc-thumb-x-0").expect("the remove target");
        let (w, h) = (f32::from(target.size.width), f32::from(target.size.height));
        assert!(w >= 24.0 && h >= 24.0, "remove target {w}x{h} at {ui}");
        // The 20 pt disc overhangs the corner by 7 pt: its centre sits 3 pt
        // inside the square's top right corner.
        let centre_x = f32::from(target.origin.x) + w / 2.0;
        let centre_y = f32::from(target.origin.y) + h / 2.0;
        let corner_x = f32::from(thumb.origin.x + thumb.size.width);
        let corner_y = f32::from(thumb.origin.y);
        assert!((centre_x - (corner_x - 3.0)).abs() < 0.5, "x at {ui}");
        assert!((centre_y - (corner_y + 3.0)).abs() < 0.5, "y at {ui}");
    }
}
