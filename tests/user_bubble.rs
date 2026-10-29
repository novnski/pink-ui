//! The `pink-user-bubble` package's geometry at Interface sizes 12, 16 and
//! 20, as PINK lays a sent prompt out. Failure modes: padding that stays
//! fixed while the text grows (a cramped plate at large sizes); a line box
//! that doesn't follow the text size (clipped or airy lines); a long prompt
//! that overflows the width cap, or pushes a right-aligned row off the
//! column's left edge because the plate can't shrink below its unwrapped
//! text; a short prompt that stretches to the cap instead of hugging it;
//! a long prompt in a narrow pane (a column narrower than the cap, which
//! PINK derives from the reading width) stopping short of the full column.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::user_bubble::{LINE_HEIGHT, MAX_WIDTH_FRACTION, PAD_X, PAD_Y, plate},
    pink::Pink,
};

/// The test column's width.
const COLUMN: f32 = 400.0;
/// A reading width wider than the column: a narrow pane.
const READING: f32 = 560.0;

const LONG: &str = "The payment helper polls with a fixed two-second timeout, but the runners take up to three seconds to boot the mock, so the first assertion sometimes runs before the server is listening.";

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let row = |name: &'static str, reading: f32, text: &'static str| {
            div().w_full().flex().justify_end().child(
                plate(reading, &t)
                    .debug_selector(move || name.into())
                    .child(
                        div()
                            .debug_selector(move || format!("{name}-text"))
                            .child(text),
                    ),
            )
        };
        div()
            .w(gpui::px(COLUMN))
            .flex()
            .flex_col()
            .font_family(pink_ui::pink::SANS)
            .debug_selector(|| "column".into())
            .child(row("short", COLUMN, "Why?"))
            .child(row("long", COLUMN, LONG))
            .child(row("narrow", READING, LONG))
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
fn the_plate_pads_and_wraps_its_prompt_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Light, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let column = bounds(cx, "column");
        let short = bounds(cx, "short");
        let short_text = bounds(cx, "short-text");
        let long = bounds(cx, "long");
        let long_text = bounds(cx, "long-text");

        // One line: the padding and line box grow with the Interface size.
        assert!(
            near(
                f32::from(short.size.height),
                (LINE_HEIGHT + 2.0 * PAD_Y) * scale
            ),
            "one-line plate height at {ui}: {:?}",
            short.size
        );
        assert!(
            near(
                f32::from(short_text.origin.x - short.origin.x),
                PAD_X * scale
            ) && near(
                f32::from(short_text.origin.y - short.origin.y),
                PAD_Y * scale
            ),
            "plate padding at {ui}"
        );
        // A short prompt hugs its text, right-aligned.
        assert!(
            f32::from(short.size.width) < COLUMN * MAX_WIDTH_FRACTION / 2.0,
            "a short prompt stretches at {ui}: {:?}",
            short.size
        );
        assert!(
            near(f32::from(column.right() - short.right()), 0.0),
            "right-aligned at {ui}"
        );

        // A long prompt wraps inside the cap instead of overflowing it.
        assert!(
            near(f32::from(long.size.width), COLUMN * MAX_WIDTH_FRACTION),
            "long plate at the cap at {ui}: {:?}",
            long.size
        );
        assert!(
            long.origin.x >= column.origin.x && near(f32::from(column.right() - long.right()), 0.0),
            "long plate stays in the column at {ui}"
        );
        let lines = f32::from(long_text.size.height) / (LINE_HEIGHT * scale);
        assert!(
            lines >= 2.0 && near(lines, lines.round()),
            "long prompt wraps on whole line boxes at {ui}: {lines}"
        );

        // A narrow pane: the column bounds the plate before the cap does,
        // so the long prompt takes the whole column, never folded.
        let narrow = bounds(cx, "narrow");
        let narrow_text = bounds(cx, "narrow-text");
        assert!(
            near(f32::from(narrow.size.width), COLUMN)
                && near(f32::from(narrow.left()), f32::from(column.left())),
            "a long prompt in a narrow pane uses the full column at {ui}: {:?}",
            narrow
        );
        assert!(
            near(
                f32::from(narrow.size.height),
                f32::from(narrow_text.size.height) + 2.0 * PAD_Y * scale
            ),
            "the narrow plate shows its whole prompt at {ui}"
        );
    }
}
