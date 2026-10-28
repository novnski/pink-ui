//! The `pink-message-actions` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out the hover lane under a transcript entry. Failure
//! modes: a lane whose height stays fixed while the text grows (the strip
//! clips or the next turn crowds it); actions that stay 24 px at 20 px
//! (too small a target beside larger text); the strip floating at the
//! lane's top instead of its bottom `ACTION_SIZE`; a user lane that starts
//! at the column's left instead of ending under the bubble's right edge;
//! a fixed 2 px gap between actions that scales and drifts them apart.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*,
    transparent_black,
};
use pink_ui::{
    Theme, ThemeMode,
    components::message_actions::{
        ACTION_ICON, ACTION_SIZE, LANE_HEIGHT, STRIP_GAP, STRIP_LEAD, action, lane, strip,
        timestamp, ui_rems,
    },
    pink::Pink,
};

/// The test column's width.
const COLUMN: f32 = 400.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let glyph = |name: String| {
            div()
                .debug_selector(move || name)
                .flex_none()
                .size(ui_rems(ACTION_ICON))
                .into_any_element()
        };
        let entry = |align_end: bool, prefix: &'static str| {
            lane(align_end)
                .debug_selector(move || format!("{prefix}-lane"))
                .child(
                    strip()
                        .child(
                            timestamp("Oct 23, 3:41 PM", t.content_faint)
                                .debug_selector(move || format!("{prefix}-time")),
                        )
                        .child(
                            action(
                                format!("{prefix}-fork"),
                                glyph(format!("{prefix}-fork-glyph")),
                                transparent_black(),
                            )
                            .debug_selector(move || format!("{prefix}-fork")),
                        )
                        .child(
                            action(
                                format!("{prefix}-copy"),
                                glyph(format!("{prefix}-copy-glyph")),
                                transparent_black(),
                            )
                            .debug_selector(move || format!("{prefix}-copy")),
                        ),
                )
        };
        div()
            .w(gpui::px(COLUMN))
            .flex()
            .flex_col()
            .debug_selector(|| "column".into())
            .child(entry(false, "reply"))
            .child(entry(true, "user"))
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
fn the_lane_and_its_actions_grow_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let column = bounds(cx, "column");
        let lane = bounds(cx, "reply-lane");
        let time = bounds(cx, "reply-time");
        let fork = bounds(cx, "reply-fork");
        let copy = bounds(cx, "reply-copy");

        assert!(
            near(f32::from(lane.size.height), LANE_HEIGHT * scale),
            "lane height at {ui}: {:?}",
            lane.size
        );
        assert!(
            near(f32::from(lane.size.width), COLUMN),
            "the lane spans the column at {ui}"
        );
        for (name, b) in [("fork", fork), ("copy", copy)] {
            assert!(
                near(f32::from(b.size.width), ACTION_SIZE * scale)
                    && near(f32::from(b.size.height), ACTION_SIZE * scale),
                "{name} square at {ui}: {:?}",
                b.size
            );
            // The strip sits on the lane's bottom `ACTION_SIZE`.
            assert!(
                near(f32::from(lane.bottom() - b.bottom()), 0.0),
                "{name} sits on the lane's bottom at {ui}"
            );
        }
        let glyph = bounds(cx, "reply-fork-glyph");
        assert!(
            near(f32::from(glyph.size.width), ACTION_ICON * scale),
            "glyph at {ui}"
        );
        assert!(
            near(f32::from(glyph.center().x - fork.center().x), 0.0)
                && near(f32::from(glyph.center().y - fork.center().y), 0.0),
            "glyph centred at {ui}"
        );

        // A reply's strip starts flush with the column: the lead, then the
        // timestamp, then the actions a fixed `STRIP_GAP` apart.
        assert!(
            near(f32::from(lane.origin.x - column.origin.x), 0.0),
            "reply lane flush at {ui}"
        );
        assert!(
            near(
                f32::from(time.origin.x - lane.origin.x),
                STRIP_LEAD * scale + STRIP_GAP
            ),
            "timestamp lead at {ui}: {}",
            f32::from(time.origin.x - lane.origin.x)
        );
        assert!(
            near(f32::from(fork.origin.x - time.right()), STRIP_GAP),
            "timestamp to first action at {ui}"
        );
        assert!(
            near(f32::from(copy.origin.x - fork.right()), STRIP_GAP),
            "action gap at {ui}"
        );

        // A user lane ends under the bubble's right edge.
        let user_copy = bounds(cx, "user-copy");
        assert!(
            near(f32::from(column.right() - user_copy.right()), 0.0),
            "user lane ends flush at {ui}"
        );
    }
}
