//! The `pink-settings-parts` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its settings rows out. Failure modes: a row tile or
//! its icon that stays at its 16 px size beside text that grows; an option
//! card whose selection ring shifts or resizes its preview, so choosing a
//! card moves it; option cards of unequal width; a strip whose padding or
//! top space stays fixed while its copy grows, or whose icon leaves the
//! first line; a meta line whose fragments overflow a narrow row instead of
//! wrapping inside it.

use gpui::{
    AnyElement, Bounds, Context, IntoElement, Pixels, Render, SharedString, TestAppContext, Window,
    div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::settings_parts::{
        ERROR_STRIP_PAD_Y, ERROR_STRIP_TOP, OPTION_CARD_HEIGHT, OptionCardStyle, RING_GAP,
        RING_WIDTH, STRIP_ICON_TOP, TILE_SIZE, WARNING_STRIP_ICON, WARNING_STRIP_PAD_Y,
        WARNING_STRIP_TOP, error_strip, meta_line, option_card, option_card_row, row_tile,
        strip_style, tile_style, warning_strip,
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
