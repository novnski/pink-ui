//! The `pink-segmented-control` package's geometry at Interface sizes 12,
//! 16 and 20, as PINK lays its segmented control out in rems. Failure
//! modes: a track that keeps its 16 pt width or height at every Interface
//! size (labels lose their room as the type grows, as the gallery's
//! fixed-pixel tray did); options that stop sharing the track equally; a
//! thumb that misses the chosen option's share or covers the track's inset;
//! a long label pushing its neighbours instead of truncating.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::segmented_control::tray::{self, INSET, Track},
    pink::Pink,
};

/// The control's width and height in design pt, and its corner.
const WIDTH: f32 = 240.0;
const HEIGHT: f32 = 28.0;
const RADIUS: f32 = 8.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let labels = ["Enter".to_string(), "Cmd Enter".into(), "Send ".repeat(40)];
        let options = labels
            .into_iter()
            .enumerate()
            .map(|(ix, text)| {
                tray::option(("option", ix), ix == 1, RADIUS)
                    .debug_selector(move || format!("option-{ix}"))
                    .child(tray::label(text))
                    .into_any_element()
            })
            .collect();
        div().child(
            tray::tray(
                &Track {
                    width: Some(WIDTH),
                    height: HEIGHT,
                    radius: RADIUS,
                    text: 12.0,
                    fill: t.fill(pink_ui::pink::FillTier::Quaternary),
                    edge: t.separator(false),
                },
                Some(tray::thumb(1.0, 3, RADIUS, t.text, None).debug_selector(|| "thumb".into())),
                options,
            )
            .debug_selector(|| "track".into()),
        )
    }
}

const OPTIONS: [&str; 3] = ["option-0", "option-1", "option-2"];

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 0.01, "{what}: {a} != {b}");
}

#[gpui::test]
fn the_tray_grows_with_the_interface_size_and_its_thumb_covers_the_chosen_share(
    cx: &mut TestAppContext,
) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let track = bounds(cx, "track");
        close(
            f32::from(track.size.width),
            WIDTH * scale,
            &format!("track width at {ui}"),
        );
        close(
            f32::from(track.size.height),
            HEIGHT * scale,
            &format!("track height at {ui}"),
        );
        // The content box: inside the 1 px edge and the px inset.
        let inner = f32::from(track.size.width) - 2.0 * (INSET + 1.0);
        let share = inner / 3.0;
        for ix in 0..3 {
            let option = bounds(cx, OPTIONS[ix]);
            close(
                f32::from(option.size.width),
                share,
                &format!("option {ix} share at {ui}"),
            );
            close(
                f32::from(option.left()) - f32::from(track.left()),
                INSET + 1.0 + share * ix as f32,
                &format!("option {ix} place at {ui}"),
            );
        }
        let thumb = bounds(cx, "thumb");
        let chosen = bounds(cx, "option-1");
        close(
            f32::from(thumb.left()),
            f32::from(chosen.left()),
            &format!("thumb left at {ui}"),
        );
        close(
            f32::from(thumb.size.width),
            f32::from(chosen.size.width),
            &format!("thumb width at {ui}"),
        );
        close(
            f32::from(thumb.size.height),
            f32::from(track.size.height) - 2.0 * (INSET + 1.0),
            &format!("thumb height at {ui}"),
        );
    }
}
