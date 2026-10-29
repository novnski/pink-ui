//! The `pink-model-picker` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its model menu out in rems. Failure modes: a strip, tab,
//! row or tray row that keeps its 16 pt size at every Interface size (the
//! card stops growing with the setting, as the gallery's fixed-pixel card
//! did); the viewed tab's bar dropping a fixed 4 px, so it floats above the
//! hairline at 20 and hangs below the card's strip at 12; a long model name
//! pushing the key cap and star out of the row instead of truncating; a
//! two-line row losing its provider line.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::model_picker::menu::{
        self, CARD_W, ModelRow, ModelRowStyle, ProviderLine, ROW_PAD_X, ROW_PAD_Y_COMPACT, STAR,
        STRIP_H, TAB, TRAY_ROW_H, ui_rems,
    },
    pink::Pink,
};

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

fn glyph(side: f32) -> gpui::AnyElement {
    div().size(ui_rems(side)).flex_none().into_any_element()
}

fn row(name: &str, provider: bool, t: &Pink) -> ModelRow {
    ModelRow {
        name: name.to_string().into(),
        badge: Some(menu::new_badge("New", t.accent, t.accent.opacity(0.14)).into_any_element()),
        attribution: None,
        provider: provider.then(|| ProviderLine {
            mark: glyph(menu::META_MARK),
            name: "Claude Code".into(),
        }),
        shortcut: Some(div().w(ui_rems(24.0)).into_any_element()),
        star: menu::star_button("star", glyph(menu::STAR_GLYPH))
            .debug_selector(move || format!("star-{provider}"))
            .into_any_element(),
    }
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = ModelRowStyle {
            text: t.text,
            muted: t.text_muted,
            faint: t.text_faint,
            fill: None,
            shadows: Vec::new(),
        };
        let long = "Opus ".repeat(60);
        div()
            .flex()
            .flex_col()
            .w(ui_rems(CARD_W))
            .child(
                menu::tab_strip("strip", t.hairline(0.08))
                    .debug_selector(|| "strip".into())
                    .child(
                        // The viewed tab, with its bar named for the test.
                        menu::tab("tab", glyph(menu::PROVIDER_MARK), false, t.accent)
                            .debug_selector(|| "tab".into())
                            .child(menu::tab_bar(t.accent).debug_selector(|| "bar".into())),
                    ),
            )
            .child(
                menu::model_row("compact", row(&long, false, &t), &style)
                    .debug_selector(|| "compact".into()),
            )
            .child(
                menu::model_row("two-line", row("Opus 5.5", true, &t), &style)
                    .debug_selector(|| "two-line".into()),
            )
            .child(
                menu::tray("tray", t.hairline(0.08)).child(
                    menu::tray_rows().child(
                        div()
                            .h(ui_rems(TRAY_ROW_H))
                            .debug_selector(|| "tray-row".into()),
                    ),
                ),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn close(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() < 0.01, "{what}: {a} != {b}");
}

#[gpui::test]
fn the_menu_grows_with_the_interface_size_and_keeps_its_bar_on_the_hairline(
    cx: &mut TestAppContext,
) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let strip = bounds(cx, "strip");
        close(
            f32::from(strip.size.height),
            STRIP_H * scale,
            &format!("strip at {ui}"),
        );
        close(
            f32::from(strip.size.width),
            CARD_W * scale,
            &format!("card at {ui}"),
        );
        let tab = bounds(cx, "tab");
        close(
            f32::from(tab.size.height),
            TAB * scale,
            &format!("tab at {ui}"),
        );
        // The hairline is the strip's bottom pixel; the bar ends inside it.
        let bar = bounds(cx, "bar");
        let bottom = f32::from(bar.bottom());
        let hairline = f32::from(strip.bottom());
        assert!(
            bottom > hairline - 1.0 && bottom <= hairline,
            "bar ends at {bottom}, off the hairline {}..{hairline} at {ui}",
            hairline - 1.0
        );

        // A long name truncates: the star keeps its size and its inset.
        let compact = bounds(cx, "compact");
        close(
            f32::from(compact.size.height),
            2.0 * ROW_PAD_Y_COMPACT + STAR * scale,
            &format!("compact row at {ui}"),
        );
        let star = bounds(cx, "star-false");
        close(
            f32::from(star.size.width),
            STAR * scale,
            &format!("star at {ui}"),
        );
        assert!(
            f32::from(star.right()) <= f32::from(compact.right()) - ROW_PAD_X * scale + 0.01,
            "a long name pushed the star out at {ui}"
        );

        let two_line = bounds(cx, "two-line");
        assert!(
            two_line.size.height > compact.size.height,
            "the two-line row lost its provider line at {ui}"
        );
        let tray_row = bounds(cx, "tray-row");
        close(
            f32::from(tray_row.size.height),
            TRAY_ROW_H * scale,
            &format!("tray row at {ui}"),
        );
    }
}
