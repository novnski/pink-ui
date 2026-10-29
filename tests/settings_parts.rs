//! The `pink-settings-parts` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its settings rows out. Failure modes: a row tile or
//! its icon that stays at its 16 px size beside text that grows; an option
//! card whose selection ring shifts or resizes its preview, so choosing a
//! card moves it; option cards of unequal width; a strip whose padding or
//! top space stays fixed while its copy grows, or whose icon leaves the
//! first line; a meta line whose fragments overflow a narrow row instead of
//! wrapping inside it; a page title or its count gap that stays fixed
//! while the Interface size changes; a status badge that shrinks and wraps
//! its label in a narrow controls column.

use gpui::{
    AnyElement, Bounds, Context, IntoElement, Pixels, Render, SharedString, TestAppContext, Window,
    div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::settings_parts::{
        BADGE_PAD_Y, BADGE_TEXT, BadgeStyle, ERROR_STRIP_PAD_Y, ERROR_STRIP_TOP,
        OPTION_CARD_HEIGHT, OptionCardStyle, PAGE_COUNT_GAP, PAGE_COUNT_SIZE, PAGE_TITLE_LINE,
        PageHeaderStyle, RING_GAP, RING_WIDTH, STRIP_ICON_TOP, TILE_SIZE, WARNING_STRIP_ICON,
        WARNING_STRIP_PAD_Y, WARNING_STRIP_TOP, badge, error_strip, meta_line, option_card,
        option_card_row, page_header, row_tile, strip_style, tile_style, ui_rems, warning_strip,
    },
    pink::Pink,
};

const ICON: &str = "icons/settings-page/monitor.svg";
const DANGER: &str = "icons/settings-page/danger-triangle.svg";

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
    width: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = OptionCardStyle {
            ring: t.accent,
            border: t.border,
            text: t.text,
            text_muted: t.text_muted,
        };
        let fragment = |copy: &'static str| SharedString::from(copy).into_any_element();
        let fragments: Vec<AnyElement> = vec![
            fragment("This Mac"),
            fragment("Paired since the first of October"),
            fragment("Seen a moment ago"),
        ];
        div()
            .w(px(self.width))
            .flex()
            .flex_col()
            .child(row_tile(&tile_style(&t), ICON).debug_selector(|| "tile".into()))
            .child(
                div()
                    .w_full()
                    .debug_selector(|| "meta-box".into())
                    .child(meta_line(t.text_muted, fragments).debug_selector(|| "meta".into())),
            )
            .child(
                option_card_row()
                    .mt(px(0.0))
                    .child(
                        option_card(
                            &style,
                            ICON,
                            "Light",
                            true,
                            div().size_full().debug_selector(|| "chosen-preview".into()),
                        )
                        .debug_selector(|| "chosen".into()),
                    )
                    .child(
                        option_card(
                            &style,
                            ICON,
                            "Dark",
                            false,
                            div().size_full().debug_selector(|| "other-preview".into()),
                        )
                        .debug_selector(|| "other".into()),
                    ),
            )
            .child(
                error_strip(
                    &strip_style(t.danger, t.danger_muted, &t),
                    DANGER,
                    "Pairing failed",
                )
                .debug_selector(|| "error".into()),
            )
            .child(
                warning_strip(
                    &strip_style(t.warning, t.warning_muted, &t),
                    DANGER,
                    "Not synced this week",
                )
                .debug_selector(|| "warning".into()),
            )
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
fn parts_grow_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui, width: 560.0 });
        cx.run_until_parked();

        // The tile scales with the text beside it.
        let tile = bounds(cx, "tile");
        assert!(
            near(f32::from(tile.size.width), TILE_SIZE * scale)
                && near(f32::from(tile.size.height), TILE_SIZE * scale),
            "tile size at {ui}: {:?}",
            tile.size
        );

        // Both preview frames are the same scaled height and the same
        // place: the transparent ring keeps the unchosen card's geometry.
        let chosen = bounds(cx, "chosen");
        let other = bounds(cx, "other");
        let chosen_preview = bounds(cx, "chosen-preview");
        let other_preview = bounds(cx, "other-preview");
        assert!(
            near(f32::from(chosen.size.width), f32::from(other.size.width)),
            "equal cards at {ui}: {chosen:?} {other:?}"
        );
        assert!(
            near(
                f32::from(chosen_preview.origin.y),
                f32::from(other_preview.origin.y)
            ) && near(
                f32::from(chosen_preview.size.height),
                f32::from(other_preview.size.height)
            ),
            "selection moves the preview at {ui}"
        );
        // The frame's own 1 px border sits inside the scaled height.
        assert!(
            near(
                f32::from(chosen_preview.size.height),
                OPTION_CARD_HEIGHT * scale - 2.0
            ),
            "preview height at {ui}: {:?}",
            chosen_preview.size
        );
        // The ring (pixels) sits outside the frame.
        assert!(
            near(
                f32::from(chosen_preview.origin.x - chosen.origin.x),
                RING_WIDTH + RING_GAP + 1.0
            ),
            "ring inset at {ui}"
        );

        // The strips' space above and padding scale; the 1 px edge doesn't.
        let options_bottom = f32::from(chosen.bottom().max(other.bottom()));
        let error = bounds(cx, "error");
        let warning = bounds(cx, "warning");
        assert!(
            near(
                f32::from(error.origin.y) - options_bottom,
                ERROR_STRIP_TOP * scale
            ),
            "error strip top at {ui}"
        );
        assert!(
            near(
                f32::from(warning.origin.y - error.bottom()),
                WARNING_STRIP_TOP * scale
            ),
            "warning strip top at {ui}"
        );
        assert!(
            f32::from(error.size.height) >= 2.0 + 2.0 * ERROR_STRIP_PAD_Y * scale,
            "error strip padding at {ui}: {:?}",
            error.size
        );
        // A one-line warning strip is its padding plus the taller of its
        // icon (dropped STRIP_ICON_TOP) and its line.
        let icon_box = (STRIP_ICON_TOP + WARNING_STRIP_ICON) * scale;
        assert!(
            f32::from(warning.size.height)
                >= 2.0 + 2.0 * WARNING_STRIP_PAD_Y * scale + icon_box - 1.0,
            "warning strip height at {ui}: {:?}",
            warning.size
        );
    }
}

#[gpui::test]
fn a_narrow_meta_line_wraps_inside_its_row(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
    let (_, cx) = cx.add_window_view(|_, _| Host {
        ui: 20.0,
        width: 200.0,
    });
    cx.run_until_parked();

    let meta_box = bounds(cx, "meta-box");
    let meta = bounds(cx, "meta");
    assert!(
        f32::from(meta.right()) <= f32::from(meta_box.right()) + 0.5,
        "the meta line overflows its row: {meta:?} in {meta_box:?}"
    );
    assert!(
        f32::from(meta.size.height) > 20.0 * 1.5,
        "the fragments wrap onto more lines: {meta:?}"
    );
}

/// A page header over a narrow controls column holding a badge, as a
/// settings page lays them out.
struct HeaderHost {
    ui: f32,
    controls: f32,
}

impl Render for HeaderHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = PageHeaderStyle {
            title: t.text,
            count: t.text_muted,
        };
        div()
            .w(px(560.0))
            .flex()
            .flex_col()
            .child(div().flex().flex_row().child(
                page_header(&style, "Projects", None).debug_selector(|| "header-alone".into()),
            ))
            .child(
                div().flex().flex_row().child(
                    page_header(&style, "Projects", Some(SharedString::from("14")))
                        .debug_selector(|| "header".into()),
                ),
            )
            .child(
                div().flex().flex_row().child(
                    div()
                        .text_size(ui_rems(PAGE_COUNT_SIZE))
                        .child("14")
                        .debug_selector(|| "count-text".into()),
                ),
            )
            .child(
                div().w(px(self.controls)).flex().flex_row().child(
                    badge(
                        &BadgeStyle {
                            edge: Some(t.border),
                            fill: None,
                            text: t.text_muted,
                        },
                        "Overridden on this device",
                    )
                    .debug_selector(|| "badge".into()),
                ),
            )
    }
}

#[gpui::test]
fn the_page_header_and_badges_grow_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| HeaderHost {
            ui,
            controls: 400.0,
        });
        cx.run_until_parked();

        // The title's line follows the Interface size, and its count sits
        // the scaled gap after it.
        let alone = bounds(cx, "header-alone");
        let header = bounds(cx, "header");
        let count = bounds(cx, "count-text");
        assert!(
            near(f32::from(alone.size.height), PAGE_TITLE_LINE * scale),
            "title line at {ui}: {:?}",
            alone.size
        );
        assert!(
            near(
                f32::from(header.size.width) - f32::from(alone.size.width),
                PAGE_COUNT_GAP * scale + f32::from(count.size.width)
            ),
            "count gap at {ui}: {header:?} vs {alone:?} and a {:?} count",
            count.size
        );

        // A badge's padding grows with its label.
        let badge = bounds(cx, "badge");
        assert!(
            f32::from(badge.size.height) >= 2.0 * BADGE_PAD_Y * scale + BADGE_TEXT * scale,
            "badge height at {ui}: {:?}",
            badge.size
        );
    }
}

#[gpui::test]
fn a_narrow_controls_column_does_not_shrink_a_badge(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
    let (_, roomy) = cx.add_window_view(|_, _| HeaderHost {
        ui: 16.0,
        controls: 400.0,
    });
    roomy.run_until_parked();
    let natural = bounds(roomy, "badge");

    let (_, narrow) = cx.add_window_view(|_, _| HeaderHost {
        ui: 16.0,
        controls: 48.0,
    });
    narrow.run_until_parked();
    let squeezed = bounds(narrow, "badge");
    assert!(
        near(
            f32::from(squeezed.size.width),
            f32::from(natural.size.width)
        ) && near(
            f32::from(squeezed.size.height),
            f32::from(natural.size.height)
        ),
        "the badge shrank or wrapped in a 48 px column: {squeezed:?} vs {natural:?}"
    );
}
