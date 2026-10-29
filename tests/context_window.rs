//! The `pink-context-window` package: how a window's parts fill the field,
//! and the map's geometry at Interface sizes 12, 16 and 20, as PINK lays its
//! Context menu out in rems. Failure modes: the field filled row by row
//! instead of column by column, or the reserve placed before free room; a
//! part with a few tokens vanishing instead of keeping one cell; an item's
//! run merged into its part (so hovering a file can't light only its
//! cells), or a part's tokens beyond its items dropped; an overfull window
//! hiding a later part or keeping hollow cells; the field, its cells, the
//! auto-compaction
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
    // 38,300 tokens are 19% (19.15) of the window: 19 cells, shared out by
    // largest remainder although each run alone would round up to 21.
    assert_eq!(occupied(&field), 19);
    assert_eq!(field[0], Cell::Part(0, None));
    // Each item its own run, then the part's tokens beyond its items.
    assert_eq!(field[1], Cell::Part(1, Some(0)));
    assert_eq!(field[5], Cell::Part(1, Some(0)));
    assert_eq!(field[6], Cell::Part(1, Some(1)));
    assert_eq!(field[7], Cell::Part(1, Some(1)));
    assert_eq!(field[8], Cell::Part(1, None));
    assert_eq!(field[9], Cell::Part(2, None));
    assert_eq!(field[17], Cell::Part(2, None));
    // A sliver still shows.
    assert_eq!(field[18], Cell::Part(3, None));
    assert_eq!(field[19], Cell::Free);
    assert!(field[96..].iter().all(|c| *c == Cell::Reserved));
    assert_eq!(field[95], Cell::Free);
}

/// Claude Code 2.1.312's recorded parts in a 200k window (instructions
/// 3,275, tool definitions 10,722, one 2,413-token memory file; the parts
/// in PINK's fill order with the history sized to the share) and its 33,000
/// autocompact buffer as the reserve, 17 cells from cell 83 on.
fn claude_at(percent: u64) -> Vec<Cell> {
    let history = 200_000 * percent / 100 - (3_275 + 10_722 + 2_413);
    cells(
        200_000,
        &[
            part(3_275),
            part(10_722),
            part(history),
            FillPart {
                tokens: 2_413,
                items: vec![2_413],
            },
        ],
        33_000,
    )
}

fn occupied(field: &[Cell]) -> usize {
    field.iter().filter(|c| matches!(c, Cell::Part(..))).count()
}

/// novnski/pink#277: the ring said 95% while the map filled 83 cells,
/// because the parts stopped where the reserve began. Failure modes: usage
/// that runs into the reserve left undrawn (the map understating a nearly
/// full window); rounding each part on its own (or a sliver's one cell)
/// adding up to more or fewer cells than the share; the reserve drawn over
/// occupied cells, or its hollow cells dropped where usage hasn't reached
/// them; occupied cells out of fill order or broken up by free room.
#[test]
fn filled_cells_equal_the_rounded_share() {
    for (percent, hollow) in [(25, 17), (80, 17), (95, 5)] {
        let field = claude_at(percent);
        assert_eq!(field.len(), CELLS);
        let filled = occupied(&field);
        assert_eq!(filled, percent as usize, "filled cells at {percent}%");
        // Occupied cells first, then free room, then the reserve's hollow
        // cells only where usage hasn't reached it.
        assert!(field[..filled].iter().all(|c| matches!(c, Cell::Part(..))));
        let reserved = field.iter().filter(|c| **c == Cell::Reserved).count();
        assert_eq!(reserved, hollow, "hollow reserve at {percent}%");
        assert!(field[CELLS - hollow..].iter().all(|c| *c == Cell::Reserved));
        assert!(
            field[filled..CELLS - hollow]
                .iter()
                .all(|c| *c == Cell::Free)
        );
    }
    // Above `100 - reserve` the parts keep their fill order into the
    // reserve: the history and then the memory file own the overlap.
    let full = claude_at(95);
    assert_eq!(full[0], Cell::Part(0, None));
    assert!(full[83..93].iter().all(|c| *c == Cell::Part(2, None)));
    assert_eq!(full[94], Cell::Part(3, Some(0)));
    assert_eq!(full[95], Cell::Reserved);
}

#[test]
fn an_overfull_window_fills_every_cell() {
    // 150% of the window: every cell occupied, the reserve wholly overlapped,
    // and the later part still drawn at its share.
    let field = cells(100_000, &[part(30_000), part(120_000)], 3_000);
    assert!(field[..20].iter().all(|c| *c == Cell::Part(0, None)));
    assert!(field[20..].iter().all(|c| *c == Cell::Part(1, None)));
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
