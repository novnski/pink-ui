//! The `pink-place-tab` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays its right-pane place tabs out. Failure modes: a tab that
//! stops being the fixed width the strip's drag maths assume (drop indexes
//! and slide offsets land on the wrong tab); a row or tab that grows with
//! the Interface size; a glyph slot that loses its leading inset or its
//! size; a title that stops truncating inside the tab.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::place_tab::{
        ADD_SIZE, BAR_HEIGHT, HomeStyle, PAD_LEFT, SLOT_SIZE, TAB_HEIGHT, TAB_SLOT, TAB_WIDTH,
        TabStyle, add_button, bar, home_tab, slot, strip, tab, title,
    },
    pink::Pink,
};

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let place = |name: &'static str, slot_name: &'static str, label: &'static str| {
            tab(&TabStyle {
                fill: Some(t.element_active),
            })
            .debug_selector(move || name.into())
            .child(slot().debug_selector(move || slot_name.into()))
            .child(title(label, t.text))
        };
        div().w(gpui::px(520.0)).child(
            bar()
                .debug_selector(|| "bar".into())
                .child(
                    home_tab(&HomeStyle {
                        text: t.text,
                        fill: None,
                    })
                    .debug_selector(|| "home".into())
                    .child("All agents"),
                )
                .child(
                    div().flex_1().min_w_0().child(
                        strip()
                            .child(place("first", "first-slot", "zsh"))
                            .child(place(
                                "second",
                                "second-slot",
                                "a very long terminal title that cannot fit",
                            )),
                    ),
                )
                .child(add_button(t.element_hover).debug_selector(|| "add".into())),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.5
}

#[gpui::test]
fn place_tabs_keep_their_fixed_slots_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let row = bounds(cx, "bar");
        let first = bounds(cx, "first");
        let second = bounds(cx, "second");
        let first_slot = bounds(cx, "first-slot");
        let home = bounds(cx, "home");
        let add = bounds(cx, "add");

        assert!(near(f32::from(row.size.height), BAR_HEIGHT), "bar at {ui}");
        for (name, chip) in [("first", first), ("second", second)] {
            assert!(
                near(f32::from(chip.size.width), TAB_WIDTH)
                    && near(f32::from(chip.size.height), TAB_HEIGHT),
                "{name} at {ui}: {:?}",
                chip.size
            );
        }
        // The drag maths: one slot per tab, a long title never widens it.
        assert!(
            near(f32::from(second.origin.x - first.origin.x), TAB_SLOT),
            "slot at {ui}"
        );
        assert!(
            near(f32::from(first_slot.origin.x - first.origin.x), PAD_LEFT)
                && near(f32::from(first_slot.size.width), SLOT_SIZE)
                && near(f32::from(first_slot.size.height), SLOT_SIZE),
            "glyph slot at {ui}: {first_slot:?}"
        );
        assert!(
            near(f32::from(home.size.height), TAB_HEIGHT),
            "home at {ui}"
        );
        assert!(
            near(f32::from(add.size.width), ADD_SIZE) && near(f32::from(add.size.height), ADD_SIZE),
            "add at {ui}"
        );
    }
}
