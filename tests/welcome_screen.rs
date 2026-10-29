//! The `pink-welcome-screen` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its first-run canvas out. Failure modes: a moon
//! that grows or shrinks with the Interface size instead of the space it
//! has, or whose glyph grid overflows or underfills its square; a question
//! that stays at 20 px while the rest of the interface scales, or whose
//! space above it scales; an action whose label stays at 14 px, or whose
//! padding and the space above it drift with the text.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::welcome_screen::package::{
        ACTION_GAP, ACTION_PAD_Y, HEADING_GAP, MoonStyle, ROWS, action, heading, moon,
        moon_diameter, moon_ink, moon_rows, welcome,
    },
    pink::Pink,
};

const DIAMETER: f32 = 240.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = MoonStyle {
            text: moon_ink(t.text, 0.0, false),
            font_family: pink_ui::pink::MONO.into(),
            advance: 0.6,
        };
        welcome()
            .child(moon(moon_rows(0.0, false), DIAMETER, &style).debug_selector(|| "moon".into()))
            .child(heading(t.text, "What will you build?").debug_selector(|| "heading".into()))
            .child(
                action(div().child("Add project"))
                    .line_height(gpui::relative(1.0))
                    .debug_selector(|| "action".into()),
            )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} was not drawn"))
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 0.75
}

#[gpui::test]
fn the_welcome_follows_the_interface_size_but_the_moon_its_space(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // The moon is its diameter at every Interface size, one line per
        // grid row.
        let moon = bounds(cx, "moon");
        assert!(
            near(f32::from(moon.size.width), DIAMETER)
                && near(f32::from(moon.size.height), DIAMETER),
            "moon at {ui}: {:?}",
            moon.size
        );
        assert_eq!(moon_rows(0.0, false).len(), ROWS);

        // The question sits HEADING_GAP px under the moon; its line grows
        // with the Interface size.
        let heading = bounds(cx, "heading");
        assert!(
            near(f32::from(heading.origin.y - moon.bottom()), HEADING_GAP),
            "heading gap at {ui}"
        );
        let base_line = f32::from(heading.size.height) / scale;
        assert!(
            (26.0..=34.0).contains(&base_line),
            "heading line at {ui}: {:?}",
            heading.size
        );

        // The action sits ACTION_GAP px under the question; its 14 rem label
        // (line height 1) grows inside fixed pixel padding.
        let action = bounds(cx, "action");
        assert!(
            near(f32::from(action.origin.y - heading.bottom()), ACTION_GAP),
            "action gap at {ui}"
        );
        assert!(
            near(
                f32::from(action.size.height),
                14.0 * scale + 2.0 * ACTION_PAD_Y
            ),
            "action height at {ui}: {:?}",
            action.size
        );
    }
    // The moon's diameter follows the window, never the Interface size.
    assert!(near(moon_diameter(1200.0, 1000.0), 320.0));
    assert!(near(moon_diameter(200.0, 300.0), 320.0 * 0.58));
}
