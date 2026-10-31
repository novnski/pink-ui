//! The `pink-welcome-screen` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays its first-run canvas out. Failure modes: a mark
//! that grows or shrinks with the Interface size instead of the space it
//! has, or whose cells overflow or underfill its square; a question,
//! line or hint that stays put while the rest of the interface scales, or
//! whose space above it scales; an action whose label stays at 14 px, or
//! whose padding and the space above it drift with the text.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*, px,
};
use pink_ui::{
    Theme, ThemeMode,
    components::welcome_screen::package::{
        ACTION_GAP, ACTION_PAD_Y, GRID, HEADING_GAP, HINT_GAP, HintStyle, MarkStyle,
        SUBHEADING_GAP, SUBHEADING_LINE, action, heading, hint, mark, mark_frame, mark_side,
        subheading, welcome,
    },
    pink::Pink,
};

const SIDE: f32 = 160.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(px(self.ui));
        let t = Pink::get(cx);
        let style = MarkStyle {
            tile: gpui::rgb(0x831843).into(),
            shades: [
                0xfbcfe8, 0xf9a8d4, 0xf472b6, 0xec4899, 0xdb2777, 0xbe185d, 0x9d174d,
            ]
            .into_iter()
            .map(|hex| gpui::rgb(hex).into())
            .collect(),
            highlight: gpui::rgb(0xfdf2f8).into(),
        };
        let hint_style = HintStyle {
            text: t.content_faint,
            keycap: t.border,
            keycap_text: t.text_muted,
            mono: pink_ui::pink::MONO.into(),
        };
        welcome()
            .child(mark(mark_frame(None, None), SIDE, &style).debug_selector(|| "mark".into()))
            .child(heading(t.text, "What will you build?").debug_selector(|| "heading".into()))
            .child(
                subheading(t.content_muted, "Add a project folder to start a thread.")
                    .debug_selector(|| "subheading".into()),
            )
            .child(
                action(div().child("Add project"))
                    .line_height(gpui::relative(1.0))
                    .debug_selector(|| "action".into()),
            )
            .child(hint(&hint_style, "or press", "⌘⇧N").debug_selector(|| "hint".into()))
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
fn the_welcome_follows_the_interface_size_but_the_mark_its_space(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        // The mark is its side at every Interface size, one cell per grid
        // position.
        let mark = bounds(cx, "mark");
        assert!(
            near(f32::from(mark.size.width), SIDE) && near(f32::from(mark.size.height), SIDE),
            "mark at {ui}: {:?}",
            mark.size
        );
        assert_eq!(mark_frame(None, None).cells.len(), GRID * GRID);

        // The question sits HEADING_GAP px under the mark; its line grows
        // with the Interface size.
        let heading = bounds(cx, "heading");
        assert!(
            near(f32::from(heading.origin.y - mark.bottom()), HEADING_GAP),
            "heading gap at {ui}"
        );
        let base_line = f32::from(heading.size.height) / scale;
        assert!(
            (28.0..=38.0).contains(&base_line),
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

        // The action sits ACTION_GAP px under the line; its 14 rem label
        // (line height 1) grows inside fixed pixel padding.
        let action = bounds(cx, "action");
        assert!(
            near(f32::from(action.origin.y - subheading.bottom()), ACTION_GAP),
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

        // The shortcut sits HINT_GAP px under the action and grows with the
        // text.
        let hint = bounds(cx, "hint");
        assert!(
            near(f32::from(hint.origin.y - action.bottom()), HINT_GAP),
            "hint gap at {ui}"
        );
        let hint_line = f32::from(hint.size.height) / scale;
        assert!(
            (14.0..=22.0).contains(&hint_line),
            "hint line at {ui}: {:?}",
            hint.size
        );
    }
    // The mark's side follows the window, never the Interface size.
    assert!(near(mark_side(1200.0, 1000.0), 168.0));
    assert!(near(mark_side(200.0, 300.0), 168.0 * 0.6));
}
