//! The `pink-trigger-chip` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays its composer chips out in rems. Failure modes: a Rems
//! chip that keeps its 28 pt height at every Interface size (PINK's chips
//! stop growing with the setting); the icon-only form losing its square; a
//! long model name pushing a labelled chip past its 280 pt cap instead of
//! truncating.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, SharedString, TestAppContext, Window, div,
    prelude::*, svg,
};
use pink_ui::{
    Theme, ThemeMode,
    components::trigger_chip::{
        ChipLabel, HEIGHT, ICON, MAX_WIDTH, TriggerChipContent, TriggerChipStyle, Units,
        trigger_chip,
    },
    pink::Pink,
};

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = TriggerChipStyle {
            units: Units::Rems,
            label: t.text,
            suffix: t.text_muted,
            wash: gpui::transparent_black(),
        };
        let icon = || {
            Some(
                svg()
                    .path("icons/chat-composer/brain.svg")
                    .size(Units::Rems.len(ICON))
                    .into_any_element(),
            )
        };
        div()
            .flex()
            .flex_col()
            .w(gpui::px(2000.0))
            .child(div().flex().child(trigger_chip(
                "short",
                TriggerChipContent {
                    icon: icon(),
                    icon_only: true,
                    ..Default::default()
                },
                &style,
            )))
            .child(div().flex().child(trigger_chip(
                "long",
                TriggerChipContent {
                    icon: icon(),
                    label: Some(ChipLabel::Text(SharedString::from("Opus ".repeat(80)))),
                    suffix: Some("Medium".into()),
                    ..Default::default()
                },
                &style,
            )))
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn rem_chips_grow_with_the_interface_size_and_cap_long_labels(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let short = bounds(cx, "short");
        assert_eq!(
            f32::from(short.size.height),
            HEIGHT * scale,
            "height at {ui}"
        );
        assert_eq!(
            f32::from(short.size.width),
            f32::from(short.size.height),
            "icon-only chip is square at {ui}"
        );

        let long = bounds(cx, "long");
        assert_eq!(
            f32::from(long.size.height),
            HEIGHT * scale,
            "height at {ui}"
        );
        assert_eq!(f32::from(long.size.width), MAX_WIDTH * scale, "cap at {ui}");
    }
}
