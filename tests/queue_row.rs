//! The `pink-queue-row` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays its message queue out. Failure modes: a row whose height
//! grows with the Interface size, so the tray's drop slots (fixed 36 pt rows)
//! stop lining up with the rows under the pointer; a primary action whose
//! width follows its label, so a Send now to Steer now preview moves the
//! row; a compact row that keeps the wide capsule or gap; action discs that
//! lose their 28 pt circle; a thumbnail that stops matching its frame; and
//! a message line that stays at 12.5 px while the Interface size grows.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::queue_row::{
        ACTION_SIZE, ActionStyle, COMPACT_ROW_GAP, LINE_HEIGHT, MARKER_HEIGHT, MARKER_WIDTH,
        PRIMARY_WIDTH, ROW_GAP, ROW_HEIGHT, RowStyle, THUMB_HEIGHT, THUMB_WIDTH, ThumbnailStyle,
        TrayStyle, action, actions, content, drag_marker, message, message_line, primary, row,
        thumbnail_placeholder, tray,
    },
    pink::Pink,
};

const ICON: &str = "icons/agent-steering/queue-drag-handle.svg";
const TUCKED: f32 = 18.0;

struct Host {
    ui: f32,
}

fn one_row(compact: bool, t: &Pink, prefix: &'static str) -> impl IntoElement {
    let style = ActionStyle {
        fill: t.surface,
        text: t.text,
        enabled: true,
    };
    row(&RowStyle {
        fill: t.surface,
        compact,
        removing: false,
    })
    .debug_selector(move || format!("{prefix}-row"))
    .child(
        drag_marker(ICON, t.text_muted, false).debug_selector(move || format!("{prefix}-marker")),
    )
    .child(
        thumbnail_placeholder(
            &ThumbnailStyle {
                border: t.border,
                background: t.surface,
            },
            ICON,
        )
        .debug_selector(move || format!("{prefix}-thumb")),
    )
    .child(
        content().child(
            message_line(t.text)
                .debug_selector(move || format!("{prefix}-line"))
                .child(message("Also cover the retry path in the webhook tests")),
        ),
    )
    .child(
        actions()
            .child(action(&style).debug_selector(move || format!("{prefix}-action")))
            .child(
                primary(&style, compact)
                    .debug_selector(move || format!("{prefix}-primary"))
                    .when(!compact, |el| el.child("Steer now")),
            ),
    )
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        div().w(gpui::px(520.0)).child(
            tray(&TrayStyle {
                background: t.surface,
                border: t.border,
                shadows: None,
                tucked: TUCKED,
            })
            .debug_selector(|| "tray".into())
            .child(one_row(false, &t, "wide"))
            .child(one_row(true, &t, "compact")),
        )
    }
}

fn bounds(cx: &mut gpui::VisualTestContext, name: &str) -> Bounds<Pixels> {
    // Debug selectors are looked up by a static name.
    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
    cx.debug_bounds(name)
        .unwrap_or_else(|| panic!("{name} not laid out"))
}

#[gpui::test]
fn queue_rows_keep_their_slots_while_text_scales(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let wide = bounds(cx, "wide-row");
        let compact = bounds(cx, "compact-row");
        assert_eq!(
            f32::from(wide.size.height),
            ROW_HEIGHT,
            "row height at {ui}"
        );
        assert_eq!(
            f32::from(compact.size.height),
            ROW_HEIGHT,
            "compact height at {ui}"
        );
        // Rows sit flush: the tray pads only its tucked foot.
        assert_eq!(
            compact.origin.y,
            wide.origin.y + wide.size.height,
            "flush rows at {ui}"
        );
        let tray = bounds(cx, "tray");
        assert!(
            (f32::from(tray.size.height) - (2.0 * ROW_HEIGHT + TUCKED + 2.0)).abs() < 0.01,
            "tray holds two rows, its tucked foot and its edge at {ui}"
        );

        for prefix in ["wide", "compact"] {
            let action = bounds(cx, &format!("{prefix}-action"));
            assert_eq!(
                f32::from(action.size.width),
                ACTION_SIZE,
                "{prefix} disc at {ui}"
            );
            assert_eq!(
                f32::from(action.size.height),
                ACTION_SIZE,
                "{prefix} disc at {ui}"
            );
            let marker = bounds(cx, &format!("{prefix}-marker"));
            assert_eq!(
                f32::from(marker.size.width),
                MARKER_WIDTH,
                "{prefix} marker at {ui}"
            );
            assert_eq!(
                f32::from(marker.size.height),
                MARKER_HEIGHT,
                "{prefix} marker at {ui}"
            );
            let thumb = bounds(cx, &format!("{prefix}-thumb"));
            assert_eq!(
                f32::from(thumb.size.width),
                THUMB_WIDTH,
                "{prefix} thumb at {ui}"
            );
            assert_eq!(
                f32::from(thumb.size.height),
                THUMB_HEIGHT,
                "{prefix} thumb at {ui}"
            );
            let line = bounds(cx, &format!("{prefix}-line"));
            assert!(
                (f32::from(line.size.height) - LINE_HEIGHT * scale).abs() < 0.01,
                "{prefix} message line at {ui}: {}",
                f32::from(line.size.height)
            );
            // The marker and thumbnail sit one gap apart.
            let gap = f32::from(thumb.origin.x - (marker.origin.x + marker.size.width));
            let expected = if prefix == "compact" {
                COMPACT_ROW_GAP
            } else {
                ROW_GAP
            };
            assert!((gap - expected).abs() < 0.01, "{prefix} gap at {ui}: {gap}");
        }
        let wide_primary = bounds(cx, "wide-primary");
        assert_eq!(
            f32::from(wide_primary.size.width),
            PRIMARY_WIDTH,
            "capsule at {ui}"
        );
        let compact_primary = bounds(cx, "compact-primary");
        assert_eq!(
            f32::from(compact_primary.size.width),
            ACTION_SIZE,
            "compact disc at {ui}"
        );
        // Both rows end their actions on the same trailing edge.
        assert_eq!(
            wide_primary.origin.x + wide_primary.size.width,
            compact_primary.origin.x + compact_primary.size.width,
            "trailing edge at {ui}"
        );
    }
}
