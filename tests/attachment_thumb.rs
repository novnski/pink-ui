//! The `pink-attachment-thumb` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out the tiles above a sent message and the squares
//! staged above its composer. Failure modes: a tile or square that scales
//! with the Interface size (PINK keeps thumbnails at fixed px); an image
//! that is not inset inside the 1 px edge or grows past its frame; a
//! sending veil that no longer covers the whole image inside the edge; a
//! sent strip that stops right-aligning beside the user's bubble, or that
//! runs its tiles off its edge instead of wrapping them into rows 8 px
//! apart; a remove disc that changes size or stops overhanging its square's
//! top right corner.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::attachment_thumb::{
        Kind, PlateStyle, REMOVE_OVERHANG, REMOVE_SIZE, RemoveStyle, SENT_HEIGHT, SENT_STRIP_GAP,
        SENT_STRIP_PAD_X, SENT_WIDTH, STAGED_SIZE, image, missing, plate, remove_disc, sending,
        sent_strip, skeleton, staged_strip,
    },
    pink::Pink,
};

/// Wide enough for three sent tiles in a row, not four.
const STRIP_W: f32 = 400.0;
const IMAGE: &str = "images/image-lightbox/p95-latency.png";

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = PlateStyle {
            edge: t.hairline(0.11),
            fill: t.ink(0.035),
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div().w(px(STRIP_W)).child(
                    sent_strip()
                        .debug_selector(|| "strip".into())
                        .child(
                            plate(Kind::Sent, &style)
                                .debug_selector(|| "sent".into())
                                .child(
                                    image(Kind::Sent, IMAGE).debug_selector(|| "sent-image".into()),
                                )
                                .child(sending(0.5).debug_selector(|| "veil".into())),
                        )
                        .child(missing(Kind::Sent, &style))
                        .child(skeleton(Kind::Sent, &style, 0.5))
                        .child(plate(Kind::Sent, &style).debug_selector(|| "wrapped".into())),
                ),
            )
            .child(
                div().w(px(STRIP_W)).child(
                    staged_strip().child(
                        div()
                            .flex_none()
                            .relative()
                            .child(
                                plate(Kind::Staged, &style)
                                    .debug_selector(|| "staged".into())
                                    .child(image(Kind::Staged, IMAGE)),
                            )
                            .child(
                                remove_disc(&RemoveStyle {
                                    fill: t.surface_raised,
                                    edge: t.border_strong,
                                })
                                .debug_selector(|| "remove".into()),
                            ),
                    ),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

fn size_of(b: Bounds<Pixels>) -> (f32, f32) {
    (f32::from(b.size.width), f32::from(b.size.height))
}

#[gpui::test]
fn thumbnails_keep_their_size_and_strips_their_layout(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let sent = bounds(cx, "sent");
        assert_eq!(
            size_of(sent),
            (SENT_WIDTH, SENT_HEIGHT),
            "sent tile at {ui}"
        );
        let picture = bounds(cx, "sent-image");
        assert_eq!(
            size_of(picture),
            (SENT_WIDTH - 2.0, SENT_HEIGHT - 2.0),
            "image fills inside the edge at {ui}"
        );
        assert!(
            near(f32::from(picture.origin.x - sent.origin.x), 1.0)
                && near(f32::from(picture.origin.y - sent.origin.y), 1.0),
            "image inset by the 1 px edge at {ui}"
        );
        assert_eq!(
            bounds(cx, "veil"),
            picture,
            "veil covers the tile inside its edge, like the image, at {ui}"
        );

        // Three tiles fill the first row; the fourth wraps under the first
        // row's right edge, since the strip right-aligns.
        let strip = bounds(cx, "strip");
        let wrapped = bounds(cx, "wrapped");
        assert!(
            near(
                f32::from(wrapped.origin.y),
                f32::from(sent.origin.y) + SENT_HEIGHT + SENT_STRIP_GAP
            ),
            "the fourth tile wraps {SENT_STRIP_GAP} px under the first row at {ui}"
        );
        assert!(
            near(
                f32::from(wrapped.origin.x + wrapped.size.width),
                f32::from(strip.origin.x + strip.size.width) - SENT_STRIP_PAD_X
            ),
            "the strip right-aligns its tiles at {ui}"
        );

        let staged = bounds(cx, "staged");
        assert_eq!(
            size_of(staged),
            (STAGED_SIZE, STAGED_SIZE),
            "staged square at {ui}"
        );
        let remove = bounds(cx, "remove");
        assert_eq!(
            size_of(remove),
            (REMOVE_SIZE, REMOVE_SIZE),
            "remove disc at {ui}"
        );
        assert!(
            near(
                f32::from(remove.origin.y),
                f32::from(staged.origin.y) - REMOVE_OVERHANG
            ) && near(
                f32::from(remove.origin.x + remove.size.width),
                f32::from(staged.origin.x + staged.size.width) + REMOVE_OVERHANG
            ),
            "remove disc overhangs the top right corner by {REMOVE_OVERHANG} px at {ui}"
        );
    }
}
