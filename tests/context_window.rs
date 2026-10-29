//! The `pink-context-window` package: how a window's parts fill the field,
//! and the map's geometry at Interface sizes 12, 16 and 20, as PINK lays its
//! Context menu out in rems. Failure modes: the field filled row by row
//! instead of column by column, or the reserve placed before free room; a
//! part with a few tokens vanishing instead of keeping one cell; an item's
//! run merged into its part (so hovering a file can't light only its
//! cells), or a part's tokens beyond its items dropped; parts that overrun
//! the window eating the reserve; the field, its cells, the auto-compaction
//! rule or the breakdown rows keeping their 16 px size at other Interface
//! sizes, so the map drifts from the card PINK sizes in rems; a removed item
//! that keeps its height instead of folding away.

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*};
use pink_ui::{
    Theme, ThemeMode,
    components::context_window::package::{
        CELL, CELLS, Cell, CellPaint, FIELD_H, FIELD_W, FillPart, ITEM_H, ItemStyle, PITCH, ROW_H,
        RowStyle, Swatch, breakdown, cell, cells, field, item_row, marker_column, part_row,
    },
    pink::Pink,
};

fn part(tokens: u64) -> FillPart {
    FillPart {
        tokens,
        items: Vec::new(),
    }
}

#[test]
fn parts_fill_column_by_column_with_the_reserve_last() {
    // 200k window: 2k tokens a cell.
    let field = cells(
        200_000,
        &[
            part(3_000),
            FillPart {
                tokens: 15_000,
                items: vec![10_000, 4_000],
            },
            part(20_000),
            part(300),
        ],
        8_000,
    );
    assert_eq!(field.len(), CELLS);
    assert_eq!(field[0], Cell::Part(0, None));
    assert_eq!(field[1], Cell::Part(0, None));
    // Each item its own run, then the part's tokens beyond its items.
    assert_eq!(field[2], Cell::Part(1, Some(0)));
    assert_eq!(field[6], Cell::Part(1, Some(0)));
    assert_eq!(field[7], Cell::Part(1, Some(1)));
    assert_eq!(field[8], Cell::Part(1, Some(1)));
    assert_eq!(field[9], Cell::Part(1, None));
    assert_eq!(field[10], Cell::Part(2, None));
    // A sliver still shows.
    assert_eq!(field[20], Cell::Part(3, None));
    assert_eq!(field[21], Cell::Free);
    assert!(field[96..].iter().all(|c| *c == Cell::Reserved));
    assert_eq!(field[95], Cell::Free);
}

#[test]
fn an_overfull_window_stops_at_the_reserve() {
    let field = cells(100_000, &[part(150_000)], 3_000);
    assert!(field[..97].iter().all(|c| *c == Cell::Part(0, None)));
    assert!(field[97..].iter().all(|c| *c == Cell::Reserved));
    assert_eq!(marker_column(0.9), 18);
    assert_eq!(marker_column(0.967), 19);
}

/// A host that sets the window's rem to its Interface size, as PINK's root
/// does (`window.set_rem_size`).
struct Host {
    ui: f32,
}

impl Render for Host {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(gpui::px(self.ui));
        let t = Pink::get(cx);
        let paint = CellPaint {
            fill: Some(t.text_muted),
            edge: None,
            opacity: 1.0,
        };
        let row = RowStyle {
            wash: gpui::transparent_black(),
            label: t.text,
            meta: t.text_faint,
            count: t.text_muted,
        };
        let item = ItemStyle {
            wash: gpui::transparent_black(),
            name: t.text_muted,
            count: t.text_faint,
            font: pink_ui::pink::MONO.into(),
        };
        div()
            .flex()
            .flex_col()
            .child(field(
                "map",
                (0..CELLS).map(|i| {
                    let selector = format!("cell-{i}");
                    cell(format!("cell-{i}"), i, paint).debug_selector(move || selector)
                }),
                Some((18, t.border_strong)),
            ))
            .child(breakdown([
                part_row(
                    "row",
                    Swatch::Fill(t.text_muted),
                    "Chat history",
                    Some("12 turns".into()),
                    "96,300",
                    row,
                )
                .into_any_element(),
                item_row(
                    "item",
                    "billing_rules.md",
                    "18,400",
                    1.0,
                    None,
                    item.clone(),
                )
                .into_any_element(),
                item_row("gone", "old.md", "1,200", 0.0, None, item).into_any_element(),
            ]))
    }
}

#[gpui::test]
fn the_map_grows_with_the_interface_size(cx: &mut TestAppContext) {
    cx.update(pink_ui::init);
    for ui in [12.0_f32, 16.0, 20.0] {
        let scale = ui / 16.0;
        let mut cx = cx.clone();
        cx.update(|cx| Theme::set_mode(ThemeMode::Dark, cx));
        let (_, cx) = cx.add_window_view(|_, _| Host { ui });
        cx.run_until_parked();
        let near = |a: f32, b: f32| (a - b).abs() <= 0.5;
        let map = cx.debug_bounds("map").expect("the field");
        assert!(
            near(f32::from(map.size.width), FIELD_W * scale),
            "field at {ui}: {map:?}"
        );
        assert!(
            near(f32::from(map.size.height), FIELD_H * scale),
            "field at {ui}: {map:?}"
        );
        // Cell 7 is column 1, row 2: the field fills column by column.
        let seventh = cx.debug_bounds("cell-7").expect("cell 7");
        assert!(
            near(f32::from(seventh.origin.x - map.origin.x), PITCH * scale),
            "cell 7 x at {ui}"
        );
        assert!(
            near(
                f32::from(seventh.origin.y - map.origin.y),
                2.0 * PITCH * scale
            ),
            "cell 7 y at {ui}"
        );
        assert!(
            near(f32::from(seventh.size.width), CELL * scale),
            "cell size at {ui}"
        );
        let last = cx.debug_bounds("cell-99").expect("the last cell");
        assert!(
            near(
                f32::from(last.origin.x + last.size.width),
                f32::from(map.origin.x + map.size.width)
            ),
            "last cell ends the field at {ui}"
        );
        // The rule stands in the gap before column 18.
        let marker = cx
            .debug_bounds("map-marker")
            .expect("the auto-compaction rule");
        let gap_start = map.origin.x + gpui::px((18.0 * PITCH - 3.0) * scale);
        assert!(
            marker.origin.x > gap_start
                && marker.origin.x < map.origin.x + gpui::px(18.0 * PITCH * scale),
            "rule at {ui}: {marker:?}"
        );
        let row = cx.debug_bounds("row").expect("a part row");
        assert!(
            near(f32::from(row.size.height), ROW_H * scale),
            "row at {ui}: {row:?}"
        );
        let item = cx.debug_bounds("item").expect("an item");
        assert!(
            near(f32::from(item.size.height), ITEM_H * scale),
            "item at {ui}: {item:?}"
        );
        let gone = cx.debug_bounds("gone").expect("a removed item");
        assert!(
            f32::from(gone.size.height) < 0.5,
            "a removed item folds away at {ui}"
        );
    }
}
