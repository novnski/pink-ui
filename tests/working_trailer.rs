//! The `pink-working-trailer` package's geometry at Interface sizes 12, 16
//! and 20, as PINK lays out the live line under the last transcript row.
//! Failure modes: a line box that stays fixed while the text grows (the
//! status clips at large sizes); the orb drifting off the status's first
//! line (centred on a wrapped sentence, or pinned to the row's top); a lead
//! or gap that scales when PINK keeps it fixed; a timer or Stop that drops
//! to the second line of a wrapped sentence instead of staying top-aligned;
//! a Stop that scales its lead but not its padding.

use gpui::{
    Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*,
    transparent_black,
};
use pink_ui::{
    Theme, ThemeMode,
    components::working_trailer::{
        GAP, LEAD, LINE_HEIGHT, ORB_BOX, STOP_LEAD, object, orb_slot, status_line, stop, timer,
        trailer, verb,
    },
    pink::Pink,
};

/// A column narrow enough that the sentence wraps (the word row gets twice
/// it).
const COLUMN: f32 = 220.0;

const SENTENCE: &str = "Reconnecting to the provider after the stream dropped, attempt two of five";

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let row = |prefix: &'static str, text: &'static str, sentence: bool| {
            let line = status_line(format!("{prefix}-status"))
                .debug_selector(move || format!("{prefix}-status"))
                .child(verb(text, t.content_muted, sentence))
                .when(!sentence, |el| el.child(object("main.rs", t.text)));
            trailer(prefix)
                .debug_selector(move || format!("{prefix}-row"))
                .child(orb_slot(t.accent, 1.0).debug_selector(move || format!("{prefix}-orb")))
                .child(line)
                .child(
                    timer("12s", t.content_faint).debug_selector(move || format!("{prefix}-timer")),
                )
                .child(
                    stop(
                        format!("{prefix}-stop"),
                        "Stop",
                        t.content_muted,
                        transparent_black(),
                    )
                    .debug_selector(move || format!("{prefix}-stop")),
                )
        };
        div()
            .w(gpui::px(2.0 * COLUMN))
            .flex()
            .flex_col()
            .font_family(pink_ui::pink::SANS)
            .debug_selector(|| "page".into())
            .child(row("word", "Reading", false))
            .child(
                div()
                    .w(gpui::px(COLUMN))
                    .debug_selector(|| "column".into())
                    .child(row("sentence", SENTENCE, true)),
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
fn the_trailer_keeps_its_items_on_the_first_line_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Light, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let page = bounds(cx, "page");
        let column = bounds(cx, "column");
        let row = bounds(cx, "word-row");
        let orb = bounds(cx, "word-orb");
        let status = bounds(cx, "word-status");
        let timer = bounds(cx, "word-timer");
        let stop = bounds(cx, "word-stop");

        // The fixed lead above the row, and one scaled line box.
        assert!(
            near(f32::from(orb.origin.y - page.origin.y), LEAD),
            "fixed lead at {ui}: {:?}",
            orb.origin
        );
        assert!(
            near(f32::from(row.size.height), LEAD + LINE_HEIGHT * scale),
            "one-line row at {ui}: {:?}",
            row.size
        );
        for (name, item) in [
            ("orb", orb),
            ("status", status),
            ("timer", timer),
            ("stop", stop),
        ] {
            assert!(
                near(f32::from(item.origin.y), f32::from(orb.origin.y))
                    && near(f32::from(item.size.height), LINE_HEIGHT * scale),
                "{name} sits on the line box at {ui}: {item:?}"
            );
        }
        // The orb's slot is as wide as the orb; the gap after it is fixed.
        assert!(
            near(f32::from(orb.size.width), ORB_BOX),
            "orb slot width at {ui}: {:?}",
            orb.size
        );
        assert!(
            near(f32::from(status.origin.x - orb.right()), GAP),
            "fixed gap at {ui}"
        );
        // The Stop leads the timer by the fixed gap plus its scaled lead.
        assert!(
            near(
                f32::from(stop.origin.x - timer.right()),
                GAP + STOP_LEAD * scale
            ),
            "stop lead at {ui}"
        );

        // A wrapped sentence keeps the orb, timer and Stop on its first line.
        let s_orb = bounds(cx, "sentence-orb");
        let s_status = bounds(cx, "sentence-status");
        let s_timer = bounds(cx, "sentence-timer");
        let s_stop = bounds(cx, "sentence-stop");
        let lines = f32::from(s_status.size.height) / (LINE_HEIGHT * scale);
        assert!(
            lines >= 2.0 && near(lines, lines.round()),
            "the sentence wraps on whole line boxes at {ui}: {lines}"
        );
        for (name, item) in [("orb", s_orb), ("timer", s_timer), ("stop", s_stop)] {
            assert!(
                near(f32::from(item.origin.y), f32::from(s_status.origin.y)),
                "{name} stays on the first line at {ui}: {item:?}"
            );
        }
        assert!(
            s_stop.right() <= column.right() + gpui::px(0.5),
            "the wrapped row stays in the column at {ui}"
        );
    }
}
