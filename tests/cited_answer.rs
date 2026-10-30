//! The `pink-cited-answer` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK's transcript and the gallery's Cited Answer draw a reply's
//! sources with it. Failure modes: a header, tile or heading that stops
//! scaling with the Interface size (fixed pixels clip the two-line header at
//! 20 or float it at 12); a long page title that pushes the chevron out of a
//! narrow source column instead of ending in an ellipsis; a quoted passage or
//! preview snippet that stops wrapping and runs off its card; several
//! citation numbers that lose their thin space and read as one number.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::cited_answer::{
        CITE_GAP, CardStyle, HEADER_H, HEADING_H, HeadingStyle, PREVIEW_W, PreviewStyle,
        QuoteStyle, TILE, Tile, card, header, heading, marker_text, preview, quote, ui_rems,
    },
    pink::Pink,
};

const LONG: &str = "Refunds and cancellations for annual subscriptions, add-ons and invoiced customers in every region";

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = CardStyle {
            border: t.hairline(0.07),
            background: t.ink(0.03),
            tile: t.ink(0.08),
            tile_text: t.content_muted,
            title: t.text,
            meta: t.content_faint,
            chevron: t.content_faint,
        };
        let source = |name: &'static str, width: f32, tile: Tile| {
            div().w(gpui::px(width)).child(
                card(&style)
                    .debug_selector(move || format!("{name}-card"))
                    .child(
                        header(
                            &style,
                            tile,
                            LONG,
                            "Help Center",
                            Some((
                                "icons/cited-answer/alt-arrow-right.svg".into(),
                                gpui::radians(0.0),
                            )),
                        )
                        .debug_selector(move || format!("{name}-header"))
                        .child(div().size_0().debug_selector(move || format!("{name}-end"))),
                    ),
            )
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                div().w(gpui::px(240.0)).child(
                    heading(
                        &HeadingStyle {
                            title: t.text,
                            count: t.content_faint,
                        },
                        "Sources",
                        "3",
                    )
                    .debug_selector(|| "heading".to_owned()),
                ),
            )
            .child(source("narrow", 220.0, Tile::Number("2".into())))
            .child(source(
                "wide",
                900.0,
                Tile::Icon("icons/cited-answer/global.svg".into()),
            ))
            .child(
                div().w(gpui::px(220.0)).child(
                    quote(
                        &QuoteStyle {
                            rule: t.hairline(0.06),
                            rail: t.accent,
                            background: t.ink(0.03),
                            text: t.content_muted,
                        },
                        div().child(LONG),
                    )
                    .debug_selector(|| "quote".to_owned()),
                ),
            )
            .child(
                div().w(ui_rems(PREVIEW_W)).child(
                    preview(
                        &PreviewStyle {
                            row: t.ink(0.05),
                            bar: t.accent,
                            site: t.content_faint,
                            tag: t.ink(0.1),
                            title: t.text,
                            snippet: t.content_muted,
                        },
                        "Geist Mono",
                        Some("1".into()),
                        "developer.mozilla.org",
                        LONG,
                        Some(LONG.into()),
                    )
                    .debug_selector(|| "preview".to_owned()),
                ),
            )
            .child(
                div()
                    .w(ui_rems(TILE))
                    .h_1()
                    .debug_selector(|| "tile".to_owned()),
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
fn sources_scale_with_the_interface_and_keep_their_chevron_in_reach(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let heading = bounds(cx, "heading");
        assert!(
            (f32::from(heading.size.height) - HEADING_H * scale).abs() < 0.5,
            "heading is {:?} tall at {ui}",
            heading.size.height
        );
        for name in ["narrow", "wide"] {
            let header = bounds(cx, &format!("{name}-header"));
            assert!(
                (f32::from(header.size.height) - HEADER_H * scale).abs() < 0.5,
                "{name} header is {:?} tall at {ui}, expected {}",
                header.size.height,
                HEADER_H * scale
            );
            // The chevron is the header's last child: a long title ends in
            // an ellipsis instead of pushing it past the card's edge.
            let card = bounds(cx, &format!("{name}-card"));
            let end = bounds(cx, &format!("{name}-end"));
            assert!(
                end.origin.x <= card.origin.x + card.size.width,
                "{name} chevron pushed out at {ui}: {:?} past {:?}",
                end.origin.x,
                card.origin.x + card.size.width
            );
        }
        let tile = bounds(cx, "tile");
        assert!((f32::from(tile.size.width) - TILE * scale).abs() < 0.05);

        // A long passage wraps inside the narrow column's quote.
        let quote = bounds(cx, "quote");
        assert!(f32::from(quote.size.width) <= 220.0 + 0.5);
        assert!(
            f32::from(quote.size.height) > 18.0 * scale * 2.0,
            "a long passage did not wrap at {ui}: {:?}",
            quote.size.height
        );
        // The preview row fills its popover and wraps a long snippet.
        let preview = bounds(cx, "preview");
        assert!(
            (f32::from(preview.size.width) - PREVIEW_W * scale).abs() < 0.5,
            "preview is {:?} wide at {ui}",
            preview.size.width
        );
    }
}

#[test]
fn several_citation_numbers_stay_apart() {
    assert_eq!(marker_text(&[1]), "1");
    assert_eq!(marker_text(&[1, 3]), format!("1{CITE_GAP}3"));
    assert_eq!(marker_text(&[]), "");
}
