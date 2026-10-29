// The Context menu's map of a model's context window (pink-ui Context
// Window, adopted by PINK's pickers.rs `render_context_menu`, novnski/pink
// #257): a field of 100 cells, one per percent of the window, filled column
// by column by the parts of the context, with the reserve as hollow cells at
// the end and a rule where auto-compaction kicks in; under it a breakdown
// whose rows (and the items under a row, such as attached or memory files)
// name each part and its tokens; and a note.
//
// The package draws only. The host decides what fills the window (`cells`
// lays its parts out), resolves every colour (the occupancy tone at each
// part's strength, the dimming of unlit cells, the sweep and wave already
// mixed in, the washes already blended over their hover fade) and the
// fonts. It adds the ids' listeners (which part the pointer lights), the
// item actions with their roles, labels and focus, tabular figures (crates.io
// GPUI 0.2.2 has no font features) and the animation clock.
// Plain `//` comments: the pink-ui gallery compiles this file in place with
// `include!` for its story.

use gpui::{
    AbsoluteLength, AnyElement, Div, ElementId, Hsla, SharedString, Stateful, div, prelude::*,
    rems,
};

/// The field: 20 columns of 5 cells, so each cell is a percent.
pub const COLS: usize = 20;
pub const ROWS: usize = 5;
pub const CELLS: usize = COLS * ROWS;
/// A cell's side, the gap between cells and a cell's corner.
pub const CELL: f32 = 12.0;
pub const CELL_GAP: f32 = 3.0;
pub const CELL_RADIUS: f32 = 3.0;
/// From one cell's edge to the next.
pub const PITCH: f32 = CELL + CELL_GAP;
pub const FIELD_W: f32 = COLS as f32 * PITCH - CELL_GAP;
pub const FIELD_H: f32 = ROWS as f32 * PITCH - CELL_GAP;
/// How far the auto-compaction rule reaches past the field, top and bottom.
pub const MARKER_OVERHANG: f32 = 4.0;
/// The map section's padding and rhythm.
pub const PAD: f32 = 12.0;
pub const GAP: f32 = 8.0;
/// A card that hugs the field (its padding and a hairline each side).
pub const CARD_W: f32 = FIELD_W + 2.0 * PAD + 2.0;
/// A breakdown row (PINK's quiet list row: 28 tall, parts 6 apart).
pub const ROW_H: f32 = 28.0;
pub const ROW_GAP: f32 = 6.0;
pub const ROW_PAD_X: f32 = 8.0;
pub const ROW_RADIUS: f32 = 6.0;
/// A row's swatch: a small cell in the part's colour.
pub const SWATCH: f32 = 8.0;
pub const SWATCH_RADIUS: f32 = 2.0;
/// An item under a row (a file), its name on the labels' column.
pub const ITEM_H: f32 = 24.0;
pub const ITEM_INSET: f32 = ROW_PAD_X + SWATCH + ROW_GAP;
/// Type sizes: labels and counts, meta and item names, the note's line.
pub const TEXT: f32 = 12.0;
pub const TEXT_SMALL: f32 = 11.0;
pub const NOTE_LINE: f32 = 17.0;

/// A design size (pt at the 16 px default) in rems, so the map grows with
/// the host's Interface size.
pub fn ui_rems(design: f32) -> AbsoluteLength {
    rems(design / 16.0).into()
}

/// What one cell holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// Room left in the window.
    Free,
    /// Kept free for the reply or for compaction.
    Reserved,
    /// The host's part `.0`, and when that part lists items, item `.1`.
    Part(usize, Option<usize>),
}

/// One part of the context for [`cells`]: its tokens, and the tokens of its
/// items when it lists them (a part's tokens beyond its items still count).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FillPart {
    pub tokens: u64,
    pub items: Vec<u64>,
}

/// Lays the window out over the field, column by column: the parts in
/// order (a part with items gives each item its own run), free room, then
/// the reserve's cells at the end. A part with any tokens gets at least
/// one cell; parts that overrun the window stop at the reserve.
pub fn cells(capacity: u64, parts: &[FillPart], reserve: u64) -> Vec<Cell> {
    let per = (capacity as f64 / CELLS as f64).max(1.0);
    let count = |tokens: u64| {
        if tokens == 0 {
            0
        } else {
            ((tokens as f64 / per).round() as usize).max(1)
        }
    };
    let reserved = count(reserve).min(CELLS);
    let room = CELLS - reserved;
    let mut out = Vec::with_capacity(CELLS);
    let mut push = |cell: Cell, n: usize| {
        for _ in 0..n {
            if out.len() < room {
                out.push(cell);
            }
        }
    };
    for (index, part) in parts.iter().enumerate() {
        let mut listed = 0u64;
        for (item, tokens) in part.items.iter().enumerate() {
            listed = listed.saturating_add(*tokens);
            push(Cell::Part(index, Some(item)), count(*tokens));
        }
        push(
            Cell::Part(index, None),
            count(part.tokens.saturating_sub(listed)),
        );
    }
    out.resize(room, Cell::Free);
    out.resize(CELLS, Cell::Reserved);
    out
}

/// A cell's top-left corner in the field, in design pt: the field fills
/// column by column, top to bottom.
pub fn cell_origin(index: usize) -> (f32, f32) {
    let (col, row) = (index / ROWS, index % ROWS);
    (col as f32 * PITCH, row as f32 * PITCH)
}

/// The column the auto-compaction rule stands before, for a threshold at
/// `fraction` of the window.
pub fn marker_column(fraction: f32) -> usize {
    (COLS as f32 * fraction.clamp(0.0, 1.0)).round() as usize
}

/// How one cell paints: a fill, or (for the reserve) an edge only, and its
/// opacity (the host's dimming of unlit cells).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellPaint {
    pub fill: Option<Hsla>,
    pub edge: Option<Hsla>,
    pub opacity: f32,
}

/// One cell, placed in the field. The host chains its hover listener.
pub fn cell(id: impl Into<ElementId>, index: usize, paint: CellPaint) -> Stateful<Div> {
    let (left, top) = cell_origin(index);
    div()
        .id(id)
        .absolute()
        .left(ui_rems(left))
        .top(ui_rems(top))
        .size(ui_rems(CELL))
        .rounded(ui_rems(CELL_RADIUS))
        .opacity(paint.opacity)
        .when_some(paint.fill, |cell, fill| cell.bg(fill))
        .when_some(paint.edge, |cell, edge| cell.border_1().border_color(edge))
}

/// The field: the host's cells and, when the provider compacts on its own,
/// the rule in the gap before `marker`'s column.
pub fn field(
    id: impl Into<SharedString>,
    cells: impl IntoIterator<Item = Stateful<Div>>,
    marker: Option<(usize, Hsla)>,
) -> Div {
    let id: SharedString = id.into();
    let selector = id.to_string();
    let marker_selector = format!("{id}-marker");
    div()
        .debug_selector(move || selector)
        .flex_none()
        .relative()
        .w(ui_rems(FIELD_W))
        .h(ui_rems(FIELD_H))
        .children(cells)
        .when_some(marker, |field, (column, color)| {
            field.child(
                div()
                    .debug_selector(move || marker_selector)
                    .absolute()
                    .left(ui_rems(column as f32 * PITCH - CELL_GAP + 1.0))
                    .top(ui_rems(-MARKER_OVERHANG))
                    .w(ui_rems(1.0))
                    .h(ui_rems(FIELD_H + 2.0 * MARKER_OVERHANG))
                    .bg(color),
            )
        })
}

/// A row's swatch: a small cell in the part's colour, or the reserve's
/// hollow cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Swatch {
    Fill(Hsla),
    Hollow(Hsla),
}

pub fn swatch(swatch: Swatch) -> Div {
    div()
        .flex_none()
        .size(ui_rems(SWATCH))
        .rounded(ui_rems(SWATCH_RADIUS))
        .map(|d| match swatch {
            Swatch::Fill(color) => d.bg(color),
            Swatch::Hollow(edge) => d.border_1().border_color(edge),
        })
}

/// A breakdown row's colours: the wash already blended over its hover fade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowStyle {
    pub wash: Hsla,
    pub label: Hsla,
    pub meta: Hsla,
    pub count: Hsla,
}

/// A part's row: its swatch, label, a quiet meta line that truncates first,
/// and its token count. The host chains the hover listener that lights the
/// part's cells, and sets tabular figures on the breakdown.
pub fn part_row(
    id: impl Into<SharedString>,
    swatch_paint: Swatch,
    label: impl Into<SharedString>,
    meta: Option<SharedString>,
    count: impl Into<SharedString>,
    style: RowStyle,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let selector = id.to_string();
    div()
        .id(id)
        .debug_selector(move || selector)
        .flex_none()
        .h(ui_rems(ROW_H))
        .px(ui_rems(ROW_PAD_X))
        .flex()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .rounded(ui_rems(ROW_RADIUS))
        .bg(style.wash)
        .child(swatch(swatch_paint))
        .child(
            div()
                .flex_none()
                .text_size(ui_rems(TEXT))
                .text_color(style.label)
                .child(label.into()),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(ui_rems(TEXT_SMALL))
                .text_color(style.meta)
                .child(meta.unwrap_or_default()),
        )
        .child(token_count(count, style.count))
}

/// A token count.
pub fn token_count(count: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .text_size(ui_rems(TEXT))
        .text_color(color)
        .child(count.into())
}

/// An item's colours and the family its name is set in (PINK keeps mono
/// for paths).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemStyle {
    pub wash: Hsla,
    pub name: Hsla,
    pub count: Hsla,
    pub font: SharedString,
}

/// The hover group an item's action and count follow.
pub fn item_group(id: &str) -> SharedString {
    SharedString::from(format!("{id}-group"))
}

/// An item under a part's row: its name on the labels' column and its
/// count. With an `action` (the host's button, which shows on the item's
/// [`item_group`] hover) the count gives way to it under the pointer.
/// `open` (0..=1) folds a removed item away.
pub fn item_row(
    id: impl Into<SharedString>,
    name: impl Into<SharedString>,
    count: impl Into<SharedString>,
    open: f32,
    action: Option<AnyElement>,
    style: ItemStyle,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let selector = id.to_string();
    let group = item_group(&id);
    let has_action = action.is_some();
    let open = open.clamp(0.0, 1.0);
    div()
        .id(id)
        .debug_selector(move || selector)
        .group(group.clone())
        .flex_none()
        .h(ui_rems(ITEM_H * open))
        .opacity(open)
        .overflow_hidden()
        .pl(ui_rems(ITEM_INSET))
        .pr(ui_rems(ROW_PAD_X))
        .flex()
        .items_center()
        .gap(ui_rems(ROW_GAP))
        .rounded(ui_rems(ROW_RADIUS))
        .bg(style.wash)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .font_family(style.font)
                .text_size(ui_rems(TEXT_SMALL))
                .text_color(style.name)
                .child(name.into()),
        )
        .child(
            div()
                .flex_none()
                .relative()
                .h_full()
                .flex()
                .items_center()
                .child(
                    token_count(count, style.count)
                        .when(has_action, |count| count.group_hover(group, |s| s.opacity(0.0))),
                )
                .when_some(action, |slot, action| {
                    slot.child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right(ui_rems(-4.0))
                            .flex()
                            .items_center()
                            .child(action),
                    )
                }),
        )
}

/// The breakdown: its rows, reaching into the section's padding so their
/// washes line up with the field's edge.
pub fn breakdown(rows: impl IntoIterator<Item = AnyElement>) -> Div {
    div()
        .mx(ui_rems(-ROW_PAD_X))
        .flex()
        .flex_col()
        .children(rows)
}

/// A quiet note under the breakdown, broken only at its own newlines.
pub fn note(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .text_size(ui_rems(TEXT_SMALL))
        .line_height(ui_rems(NOTE_LINE))
        .text_color(color)
        .child(text.into())
}

/// The map's section of the card: the field, its breakdown and the note.
pub fn section() -> Div {
    div()
        .px(ui_rems(PAD))
        .pt(ui_rems(GAP))
        .pb(ui_rems(PAD))
        .flex()
        .flex_col()
        .gap(ui_rems(GAP))
}
