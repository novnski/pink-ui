//! The `pink-tab-chip` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays its titlebar tabs out. Failure modes: a tab that grows with
//! the Interface size and spills out of the fixed titlebar strip; a close
//! slot that loses its tight trailing inset (or a tab without one that
//! loses its even padding); a title whose chrome line height stops scaling
//! with its text; a Main badge that scales off the 16 pt chip line.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::tab_chip::{
        BADGE_HEIGHT, BadgeStyle, CLOSE_SIZE, Label, PAD_LEFT, PAD_RIGHT, PAD_RIGHT_CLOSABLE,
        PART_GAP, TAB_HEIGHT, TAB_MAX_WIDTH, TAB_MIN_WIDTH, TITLE_LINE_HEIGHT, TabStyle, badge,
        close_slot, tab, title,
    },
    pink::Pink,
};

/// The leading mark's side, in px.
const MARK: f32 = 14.0;

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let style = |label: Label, closable: bool| TabStyle {
            label,
            text: t.text,
            fill: Some(t.element_active),
            closable,
            min_width: TAB_MIN_WIDTH,
            max_width: TAB_MAX_WIDTH,
        };
        let mark = |name: &'static str| {
            div()
                .debug_selector(move || name.into())
                .flex_none()
                .size(gpui::px(MARK))
        };
        div()
            .flex()
            .flex_col()
            .items_start()
            .child(
                tab(&style(Label::Tab, true))
                    .debug_selector(|| "closable".into())
                    .group("closable")
                    .child(mark("closable-mark"))
                    .child(title("Write the migration tests"))
                    .child(close_slot("closable", true).debug_selector(|| "close".into())),
            )
            .child(
                tab(&style(Label::Tab, false))
                    .debug_selector(|| "main".into())
                    .child(mark("main-mark"))
                    .child(
                        badge(&BadgeStyle {
                            fill: t.accent_wash,
                            text: t.accent,
                        })
                        .debug_selector(|| "badge".into())
                        .child("Main"),
                    )
                    .child(title("Retire expired coupon codes"))
                    .child(mark("main-end")),
            )
            .child(
                tab(&style(Label::Title, false))
                    .debug_selector(|| "lone".into())
                    .child(
                        title("Retire expired coupon codes").debug_selector(|| "lone-title".into()),
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
fn tabs_keep_the_strip_height_while_their_text_grows(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let closable = bounds(cx, "closable");
        let close = bounds(cx, "close");
        let mark = bounds(cx, "closable-mark");
        let main = bounds(cx, "main");
        let main_mark = bounds(cx, "main-mark");
        let main_end = bounds(cx, "main-end");
        let badge = bounds(cx, "badge");
        let lone = bounds(cx, "lone");
        let lone_title = bounds(cx, "lone-title");

        // The strip's fixed height at every size.
        for (name, chip) in [("closable", closable), ("main", main), ("lone", lone)] {
            assert!(
                near(f32::from(chip.size.height), TAB_HEIGHT),
                "{name} height at {ui}: {:?}",
                chip.size
            );
            assert!(
                f32::from(chip.size.width) >= TAB_MIN_WIDTH - 0.5
                    && f32::from(chip.size.width) <= TAB_MAX_WIDTH + 0.5,
                "{name} width at {ui}: {:?}",
                chip.size
            );
        }

        // Leading padding, and the close slot's tight trailing inset.
        assert!(
            near(f32::from(mark.origin.x - closable.origin.x), PAD_LEFT),
            "leading pad at {ui}"
        );
        assert!(
            near(
                f32::from(closable.right() - close.right()),
                PAD_RIGHT_CLOSABLE
            ),
            "close inset at {ui}"
        );
        assert!(
            near(f32::from(close.size.width), CLOSE_SIZE)
                && near(f32::from(close.size.height), CLOSE_SIZE),
            "close size at {ui}: {:?}",
            close.size
        );
        // Without a close slot the padding is even.
        assert!(
            near(f32::from(main.right() - main_end.right()), PAD_RIGHT),
            "trailing pad at {ui}"
        );
        assert!(
            near(f32::from(badge.origin.x - main_mark.right()), PART_GAP),
            "part gap at {ui}"
        );
        assert!(
            near(f32::from(badge.size.height), BADGE_HEIGHT),
            "badge height at {ui}: {:?}",
            badge.size
        );

        // A title's chrome line height grows with the Interface size.
        assert!(
            near(f32::from(lone_title.size.height), TITLE_LINE_HEIGHT * scale),
            "title line at {ui}: {:?}",
            lone_title.size
        );
    }
}
