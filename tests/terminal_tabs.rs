//! The `pink-terminal-tabs` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its terminal drawer's strip out. Failure modes: a
//! tab that stops being the fixed width the strip's drag maths assume (a
//! long title widens it, so drop indexes and slide offsets land on the
//! wrong tab); a band that grows with the Interface size and leaves the
//! shared header height; an unselected tab that drops its hidden close
//! control's room, so a title shifts when its tab is selected; the collapse
//! button that stops being pinned to the band's trailing edge.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::terminal_tabs::{
        BAR_HEIGHT, BAR_PAD_LEFT, BAR_PAD_RIGHT, BUTTON_SIZE, CloseStyle, ICON_CLOSE, ICON_DOWN,
        ICON_PLUS, ICON_TERMINAL, TAB_HEIGHT, TAB_WIDTH, add_button, bar, button_style,
        close_button, collapse_button, spacer, tab, tab_style,
    },
    pink::Pink,
};

const WIDTH: f32 = 600.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let session = |name: &'static str, label: &'static str, selected: bool| {
            let close = close_button(
                &CloseStyle {
                    fill: t.element_hover,
                    glyph: t.text_muted,
                    visible: selected,
                },
                ICON_CLOSE,
            )
            .debug_selector(move || format!("{name}-close"));
            tab(
                &tab_style(name, selected, false, &t),
                ICON_TERMINAL,
                label,
                close,
            )
            .debug_selector(move || name.into())
        };
        div().w(px(WIDTH)).child(
            bar(t.hairline(0.07))
                .debug_selector(|| "bar".into())
                .child(session("selected", "zsh", true))
                .child(session(
                    "long",
                    "a very long terminal title that cannot fit in the tab",
                    false,
                ))
                .child(session("resting", "zsh", false))
                .child(
                    add_button(&button_style("add", 0.6, &t), ICON_PLUS)
                        .debug_selector(|| "add".into()),
                )
                .child(spacer())
                .child(
                    collapse_button(&button_style("collapse", 0.55, &t), ICON_DOWN)
                        .debug_selector(|| "collapse".into()),
                ),
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
fn terminal_tabs_keep_their_fixed_slots_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // The band is the shared header height at every Interface size.
        let band = bounds(cx, "bar");
        assert!(
            near(f32::from(band.size.height), BAR_HEIGHT),
            "band height at {ui}: {:?}",
            band.size
        );

        // Every tab is the fixed width and height, whatever its title.
        for name in ["selected", "long", "resting"] {
            let tab = bounds(cx, name);
            assert!(
                near(f32::from(tab.size.width), TAB_WIDTH)
                    && near(f32::from(tab.size.height), TAB_HEIGHT),
                "{name} tab size at {ui}: {:?}",
                tab.size
            );
        }
        let selected = bounds(cx, "selected");
        assert!(
            near(f32::from(selected.origin.x - band.origin.x), BAR_PAD_LEFT),
            "first tab inset at {ui}"
        );

        // The hidden close control keeps its room: it sits in the same
        // place in a resting tab as in the selected one.
        let selected_close = bounds(cx, "selected-close");
        let resting = bounds(cx, "resting");
        let resting_close = bounds(cx, "resting-close");
        assert!(
            near(
                f32::from(selected_close.origin.x - selected.origin.x),
                f32::from(resting_close.origin.x - resting.origin.x)
            ),
            "close room at {ui}: {selected_close:?} in {selected:?}, {resting_close:?} in {resting:?}"
        );
        assert!(
            f32::from(bounds(cx, "long-close").right()) <= f32::from(bounds(cx, "long").right()),
            "a long title pushes the close control out of its tab at {ui}"
        );

        // The add button follows the tabs; the collapse button is pinned to
        // the trailing edge.
        let add = bounds(cx, "add");
        let collapse = bounds(cx, "collapse");
        assert!(
            near(f32::from(add.size.width), BUTTON_SIZE)
                && near(f32::from(collapse.size.width), BUTTON_SIZE),
            "button sizes at {ui}: {add:?} {collapse:?}"
        );
        assert!(
            near(f32::from(band.right() - collapse.right()), BAR_PAD_RIGHT),
            "collapse pinned right at {ui}: {collapse:?} in {band:?}"
        );
    }
}
