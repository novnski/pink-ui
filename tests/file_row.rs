//! The `pink-file-row` package's geometry at Interface sizes 12, 16 and 20,
//! as PINK lays out its Files pane. Failure modes: a row whose height grows
//! with the Interface size (PINK's virtualized tree and search lists measure
//! rows at a fixed `ROW_HEIGHT`, so rows would overlap or gap); a tree pill
//! that loses its side inset or its per-level indent; a disclosure slot or
//! artwork that stops scaling with the name beside it; a status row that is
//! no longer one level deeper than its folder's entries; a text match that
//! loses its lead.

use gpui::{Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::file_row::{
        INDENT, PILL_INSET, ROW_HEIGHT, SLOT, StatusKind, TEXT_MATCH_LEAD, TextRowKind, artwork,
        disclosure, lead, line_number, name, search_entry, status_label, status_row, text_row,
        tree_entry,
    },
};

const PANE: f32 = 300.0;

struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let black = gpui::black();
        let slot = || disclosure(None).debug_selector(|| "slot".into());
        div()
            .w(gpui::px(PANE))
            .flex()
            .flex_col()
            .debug_selector(|| "pane".into())
            .child(
                tree_entry(div(), 2, false)
                    .debug_selector(|| "entry".into())
                    .child(slot())
                    .child(artwork(div()).debug_selector(|| "art".into()))
                    .child(name("lib.rs", black, "Helvetica".into())),
            )
            .child(
                search_entry(div(), 2)
                    .debug_selector(|| "search".into())
                    .child(disclosure(None).debug_selector(|| "search-slot".into()))
                    .child(name("lib.rs", black, "Helvetica".into())),
            )
            .child(
                status_row(div(), 2, StatusKind::Note)
                    .debug_selector(|| "status".into())
                    .child(
                        status_label("Loading…", black).debug_selector(|| "status-label".into()),
                    ),
            )
            .child(
                text_row(div(), TextRowKind::Match, "Helvetica".into())
                    .debug_selector(|| "match".into())
                    .child(line_number("12", black).debug_selector(|| "line".into())),
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
fn file_rows_keep_their_height_and_indent_at_every_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();

        let pane = bounds(cx, "pane");
        let entry = bounds(cx, "entry");
        let slot = bounds(cx, "slot");
        let art = bounds(cx, "art");
        let search = bounds(cx, "search");
        let search_slot = bounds(cx, "search-slot");
        let status = bounds(cx, "status");
        let label = bounds(cx, "status-label");
        let line = bounds(cx, "line");
        let row_match = bounds(cx, "match");

        for (what, row) in [("entry", entry), ("search", search), ("status", status)] {
            assert!(
                near(f32::from(row.size.height), ROW_HEIGHT),
                "{what} height at {ui}: {:?}",
                row.size
            );
        }
        // The pill sits PILL_INSET inside each side of the pane.
        assert!(
            near(f32::from(entry.origin.x - pane.origin.x), PILL_INSET)
                && near(f32::from(entry.size.width), PANE - 2.0 * PILL_INSET),
            "pill inset at {ui}: {entry:?} in {pane:?}"
        );
        // The slot starts at the level's lead from the pane, in the pill or
        // on a full-width row alike.
        assert!(
            near(f32::from(slot.origin.x - pane.origin.x), lead(2))
                && near(f32::from(search_slot.origin.x - pane.origin.x), lead(2)),
            "lead at {ui}: {slot:?}, {search_slot:?}"
        );
        assert!(
            near(f32::from(slot.size.width), SLOT * scale)
                && near(f32::from(art.size.width), SLOT * scale),
            "slot and artwork at {ui}: {slot:?}, {art:?}"
        );
        // A status row is one level deeper than its folder's entries.
        assert!(
            near(f32::from(label.origin.x - pane.origin.x), lead(2) + INDENT),
            "status lead at {ui}: {label:?}"
        );
        assert!(
            near(
                f32::from(line.origin.x - row_match.origin.x),
                TEXT_MATCH_LEAD * scale
            ),
            "match lead at {ui}: {line:?} in {row_match:?}"
        );
    }
}
