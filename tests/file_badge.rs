//! The `pink-file-badge` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays out a tool header's file badge. Failure modes: a chip or
//! well that grows with the Interface size, so the badge no longer sits in
//! the tool header's fixed 22 pt detail slot; artwork that leaves the well's
//! centre; a long name that pushes the badge out of a narrow header instead
//! of truncating. For a Markdown file link, the opposite: a well, artwork or
//! gap that stays fixed while the paragraph's text grows with the Interface
//! size.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::file_badge::{
        BadgeStyle, GLYPH, HEIGHT, LINK_GAP, PAD_LEFT, WELL, badge, file_link, name, scaled_well,
        ui_rems, well,
    },
    pink::Pink,
};

const ART: &str = "icons/tool-call-card/file-rust.svg";
const NARROW: f32 = 120.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let chip = |prefix: &'static str, label: &'static str| {
            badge(&BadgeStyle {
                background: t.surface,
                text: t.text,
            })
            .debug_selector(move || format!("{prefix}-badge"))
            .child(
                well(
                    t.border,
                    gpui::img(ART)
                        .flex_none()
                        .size(gpui::px(GLYPH))
                        .debug_selector(move || format!("{prefix}-art")),
                )
                .debug_selector(move || format!("{prefix}-well")),
            )
            .child(name(label))
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(chip("short", "lib.rs"))
            .child(
                div()
                    .w(gpui::px(NARROW))
                    .flex()
                    .child(chip("long", "an-unusually-long-file-name.rs")),
            )
            .child(
                file_link(
                    scaled_well(
                        t.border,
                        gpui::img(ART)
                            .flex_none()
                            .size(ui_rems(GLYPH))
                            .debug_selector(|| "link-art".into()),
                    )
                    .debug_selector(|| "link-well".into()),
                    div()
                        .debug_selector(|| "link-text".into())
                        .child("the library source"),
                )
                .debug_selector(|| "link".into()),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn file_badges_hold_the_header_slot_and_truncate(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        for prefix in ["short", "long"] {
            let chip = bounds(cx, &format!("{prefix}-badge"));
            assert_eq!(f32::from(chip.size.height), HEIGHT, "{prefix} chip at {ui}");
            let well = bounds(cx, &format!("{prefix}-well"));
            assert_eq!(f32::from(well.size.width), WELL, "{prefix} well at {ui}");
            assert_eq!(f32::from(well.size.height), WELL, "{prefix} well at {ui}");
            assert_eq!(
                f32::from(well.origin.x - chip.origin.x),
                PAD_LEFT,
                "{prefix} well inset at {ui}"
            );
            let art = bounds(cx, &format!("{prefix}-art"));
            assert_eq!(
                f32::from(art.origin.x - well.origin.x),
                (WELL - GLYPH) / 2.0,
                "{prefix} artwork centred at {ui}"
            );
        }
        // Scaled sizes land on whole pixels, so allow the layout's rounding.
        let scale = ui / 16.0;
        let close = |a: Pixels, b: f32| (f32::from(a) - b).abs() <= 0.5;
        let link_well = bounds(cx, "link-well");
        assert!(
            close(link_well.size.width, WELL * scale),
            "link well at {ui}: {:?}",
            link_well.size.width
        );
        let link_art = bounds(cx, "link-art");
        assert!(
            close(
                link_art.origin.x - link_well.origin.x,
                (WELL - GLYPH) / 2.0 * scale
            ),
            "link artwork centred at {ui}"
        );
        let link_text = bounds(cx, "link-text");
        assert!(
            close(link_text.origin.x - link_well.right(), LINK_GAP * scale),
            "link gap at {ui}"
        );
        let long = bounds(cx, "long-badge");
        assert!(
            f32::from(long.size.width) <= NARROW,
            "a long name truncates inside its header at {ui}: {}",
            f32::from(long.size.width)
        );
    }
}
