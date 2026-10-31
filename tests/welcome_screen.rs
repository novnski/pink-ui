//! The `pink-welcome-screen` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its first-run canvas out. Failure modes: a field
//! heart that grows or shrinks with the Interface size instead of the space
//! it has, or that pushes the column apart; a question,
//! line or hint that stays put while the rest of the interface scales, or
//! whose space above it scales; an action whose label stays at 14 px, or
//! whose padding and the space above it drift with the text.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::welcome_screen::package::{
        ACTION_GAP, ACTION_PAD_Y, ACTION_TEXT, ActionStyle, FieldStyle, HEADING_GAP, HERO_SIDE,
        HINT_GAP, HintStyle, SUBHEADING_GAP, SUBHEADING_LINE, action, field_cells, field_layout,
        heading, hero, hint, split_keys, subheading, welcome,
    },
    pink::Pink,
};

/// A 1200 x 1000 room: the field at full scale.
const ROOM: (f32, f32) = (1200.0, 1000.0);

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = FieldStyle {
            shades: [
                0xfbcfe8, 0xf9a8d4, 0xf472b6, 0xec4899, 0xdb2777, 0xbe185d, 0x9d174d,
            ]
            .into_iter()
            .map(|hex| gpui::rgb(hex).into())
            .collect(),
            highlight: gpui::rgb(0xfdf2f8).into(),
        };
        let layout = field_layout(ROOM.0, ROOM.1);
        let hint_style = HintStyle {
            text: t.content_faint,
            cell: t.border,
            cell_edge: t.border,
            cell_text: t.text_muted,
            mono: pink_ui::pink::MONO.into(),
        };
        let action_style = ActionStyle {
            top: gpui::rgb(0xf472b6).into(),
            bottom: gpui::rgb(0xdb2777).into(),
            edge: gpui::rgb(0xbe185d).into(),
            text: gpui::white(),
        };
        welcome()
            .child(
                hero(&layout, field_cells(&layout, None, None), &style)
                    .debug_selector(|| "hero".into()),
            )
            .child(heading(t.text, "What will you build?").debug_selector(|| "heading".into()))
            .child(
                subheading(t.content_muted, "Add a project folder to start a thread.")
                    .debug_selector(|| "subheading".into()),
            )
            .child(
                action(div().child("Add project"), &action_style)
                    .line_height(gpui::relative(1.0))
                    .debug_selector(|| "action".into()),
            )
            .child(
                hint(&hint_style, "or press", split_keys("⇧⌘N")).debug_selector(|| "hint".into()),
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
fn the_welcome_follows_the_interface_size_but_the_field_its_space(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // The field's heart takes its side in the column at every Interface
        // size; the field around it spills over without moving anything.
        let mark = bounds(cx, "hero");
        assert!(
            near(f32::from(mark.size.width), HERO_SIDE)
                && near(f32::from(mark.size.height), HERO_SIDE),
            "hero at {ui}: {:?}",
            mark.size
        );

        // The question sits HEADING_GAP px under the heart; its line grows
        // with the Interface size.
        let heading = bounds(cx, "heading");
        assert!(
            near(f32::from(heading.origin.y - mark.bottom()), HEADING_GAP),
            "heading gap at {ui}"
        );
        let base_line = f32::from(heading.size.height) / scale;
        assert!(
            (40.0..=54.0).contains(&base_line),
            "heading line at {ui}: {:?}",
            heading.size
        );

        // The line sits SUBHEADING_GAP px under the question, one line of
        // SUBHEADING_LINE rems.
        let subheading = bounds(cx, "subheading");
        assert!(
            near(
                f32::from(subheading.origin.y - heading.bottom()),
                SUBHEADING_GAP
            ),
            "subheading gap at {ui}"
        );
        assert!(
            near(f32::from(subheading.size.height), SUBHEADING_LINE * scale),
            "subheading line at {ui}: {:?}",
            subheading.size
        );

        // The action sits ACTION_GAP px under the line; its 15 rem label
        // (line height 1) grows inside fixed pixel padding and its 1 px edge.
        let action = bounds(cx, "action");
        assert!(
            near(f32::from(action.origin.y - subheading.bottom()), ACTION_GAP),
            "action gap at {ui}"
        );
        assert!(
            near(
                f32::from(action.size.height),
                ACTION_TEXT * scale + 2.0 * ACTION_PAD_Y + 2.0
            ),
            "action height at {ui}: {:?}",
            action.size
        );

        // The shortcut sits HINT_GAP px under the action and grows with the
        // text.
        let hint = bounds(cx, "hint");
        assert!(
            near(f32::from(hint.origin.y - action.bottom()), HINT_GAP),
            "hint gap at {ui}"
        );
        let hint_line = f32::from(hint.size.height) / scale;
        assert!(
            (22.0..=30.0).contains(&hint_line),
            "hint line at {ui}: {:?}",
            hint.size
        );
    }
    // The field follows the window, never the Interface size.
    assert!(near(field_layout(1200.0, 1000.0).side, HERO_SIDE));
    assert!(near(field_layout(200.0, 300.0).side, HERO_SIDE * 0.6));
}
