//! Data Table – a small spreadsheet drawn as one of pink's card tables:
//! select ranges, edit in place, copy, paste, undo, sort and resize columns.
//!
//! The chrome is pink's. The card is the Usage page's Breakdown card (a
//! section card whose header carries the controls, the table inset below a
//! hairline); the rows are `usage_table_row` (an 11/14 caption header, 36 tall
//! body rows under `border` hairlines, figures right-aligned in the UI sans);
//! rows wash on hover and columns resize from a 6 px handle the way History's
//! commit table does; the gutter numbers are a diff's line numbers and the
//! footer is the command palette's key legend.
//!
//! Click a cell to select it, drag or Shift-click to select a range, click
//! a row number for the whole row. The active cell wears pink's focus ring
//! and the ring glides between cells as the selection moves. Typing replaces
//! the cell, Enter or a double-click edits it, and Enter, Tab or ↑/↓ commit
//! and move on, the way spreadsheets do. Typed values are checked against
//! the column's kind (numbers, money, ISO dates). Copy and paste use
//! tab-separated text, so ranges move to and from other spreadsheets; Delete
//! clears the selection; every change can be undone. Clicking a header sorts
//! by that column and the rows slide into their new order; dragging the
//! edge between two headers resizes them (a double-click resets the widths).
//! The footer sums numeric selections.
//!
//! Keys (with the grid focused): arrows, Shift+arrows, Tab, Enter/F2,
//! Escape, Delete, ⌘/Ctrl + C, X, V, Z, Shift+Z (or Y) and A.
//!
//! ```ignore
//! DataTable::new("accounts", "Accounts")
//!     .column(TableColumn::new("Account", ColumnKind::Text, 196.))
//!     .column(TableColumn::new("MRR", ColumnKind::Currency, 100.))
//!     .row(["Halcyon Robotics", "18400"])
//!     .on_change(|rows, _, _| println!("{} rows", rows.len()))
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyView, App, ClipboardItem, Context, DragMoveEvent, ElementId, Entity, FocusHandle,
    FontWeight, Hsla, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    SharedString, Subscription, Task, Window, div, prelude::*, px,
};

use crate::{
    HoverFill, motion,
    pink::{
        self, ButtonKind, ControlSize, FillTier, Pink, TextStyle, Tone, TypeStyleExt, glyphs,
        icon_size, layout, radius, space,
    },
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Data Table";
pub const DESCRIPTION: &str = "A spreadsheet in one of pink's card tables – caption headers over hairline rows, ranges under a gliding focus ring, typed in-place edits, tab-separated copy and paste, undo, and columns you sort and resize";

const ICON_UNDO: &str = "icons/data-table/undo.svg";
const ICON_REDO: &str = "icons/data-table/redo.svg";
const ICON_PLUS: &str = "icons/data-table/plus.svg";

/// The caption header row (pink `usage_table_row`, header `min_h 30`).
const HEAD_H: f32 = 30.;
/// A body row (pink `usage_table_row` body `min_h 36`; History's
/// `HISTORY_ROW_HEIGHT`).
const ROW_H: f32 = 36.;
/// The row-number gutter: a diff's line-number column (pink `changes.rs`
/// `diff_line_row`: mono 11, right-aligned, `pr 8`), fitted to two digits.
const GUTTER_W: f32 = 28.;
const GUTTER_PAD: f32 = 8.;
/// Side padding inside a cell, so the selection plate has room around the
/// text. Text starts `CELL_PAD` into its column.
const CELL_PAD: f32 = 8.;
/// The selection plate sits 4 px inside its row: 28 tall, like pink's
/// in-row controls (`ControlSize::Md`, `surface_chrome` toolbar input).
const PLATE_INSET: f32 = 4.;
const PLATE_H: f32 = ROW_H - 2. * PLATE_INSET;
/// The narrowest a resize leaves a column (pink History's `DATE_MIN` 68 /
/// `SHA_MIN` 58).
const MIN_COLUMN: f32 = 64.;
/// History's column resize handle: 6 px, centered on the column edge.
const HANDLE_W: f32 = 6.;
/// A row tag (pink History `render_ref`): capped at 112.
const TAG_MAX_W: f32 = 112.;

type ChangeHandler = Rc<dyn Fn(&[Vec<SharedString>], &mut Window, &mut App)>;

/// What a column holds; it decides alignment, display and input checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnKind {
    Text,
    /// A short label drawn as a tag (see [`TableColumn::tone`]).
    Tag,
    /// A plain number, right-aligned.
    Number,
    /// Money in whole units: stored "18400", shown "$18,400".
    Currency,
    /// An ISO date: stored "2027-02-14", shown "Feb 14, 2027".
    Date,
}

impl ColumnKind {
    fn numeric(self) -> bool {
        matches!(self, ColumnKind::Number | ColumnKind::Currency)
    }
}

#[derive(Clone)]
pub struct TableColumn {
    pub name: SharedString,
    pub kind: ColumnKind,
    pub width: f32,
    /// Tag tones for known [`ColumnKind::Tag`] values.
    pub tones: Vec<(SharedString, Tone)>,
    /// Drawn in the muted tone, like the Usage table's share and token
    /// columns. Dates are always secondary.
    pub secondary: bool,
}

impl TableColumn {
    pub fn new(name: impl Into<SharedString>, kind: ColumnKind, width: f32) -> Self {
        Self {
            name: name.into(),
            kind,
            width,
            tones: Vec::new(),
            secondary: false,
        }
    }

    pub fn tone(mut self, value: impl Into<SharedString>, tone: Tone) -> Self {
        self.tones.push((value.into(), tone));
        self
    }

    /// Draws the column's values in the muted tone (supporting figures).
    pub fn secondary(mut self) -> Self {
        self.secondary = true;
        self
    }
}

#[derive(IntoElement)]
pub struct DataTable {
    id: ElementId,
    title: SharedString,
    columns: Vec<TableColumn>,
    rows: Vec<Vec<SharedString>>,
    today: Option<SharedString>,
    sorted: Option<(usize, bool)>,
    on_change: Option<ChangeHandler>,
}

impl DataTable {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            columns: Vec::new(),
            rows: Vec::new(),
            today: None,
            sorted: None,
            on_change: None,
        }
    }

    pub fn column(mut self, column: TableColumn) -> Self {
        self.columns.push(column);
        self
    }

    /// Initial rows (raw values, as stored). Later edits live in the table.
    pub fn row<S: Into<SharedString>>(mut self, cells: impl IntoIterator<Item = S>) -> Self {
        self.rows.push(cells.into_iter().map(Into::into).collect());
        self
    }

    /// An ISO date; dates within two weeks of it are flagged.
    pub fn today(mut self, date: impl Into<SharedString>) -> Self {
        self.today = Some(date.into());
        self
    }

    /// Starts sorted by column `column` (descending when `descending`).
    pub fn sorted_by(mut self, column: usize, descending: bool) -> Self {
        self.sorted = Some((column, descending));
        self
    }

    /// Called with every row after each change.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[Vec<SharedString>], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// The sheet: data, selection, history. No GPUI in here, so it is unit tested.
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: usize,
    cells: Vec<SharedString>,
}

#[derive(Clone)]
struct Snapshot {
    rows: Vec<Row>,
    sort: Option<(usize, bool)>,
}

#[derive(Clone)]
struct Sheet {
    kinds: Vec<ColumnKind>,
    rows: Vec<Row>,
    next_id: usize,
    anchor: (usize, usize),
    cursor: (usize, usize),
    /// Column and descending flag of the last sort.
    sort: Option<(usize, bool)>,
    history: Vec<Snapshot>,
    future: Vec<Snapshot>,
}

impl Sheet {
    fn new(kinds: Vec<ColumnKind>, rows: Vec<Vec<SharedString>>) -> Self {
        let cols = kinds.len();
        let rows: Vec<Row> = rows
            .into_iter()
            .enumerate()
            .map(|(id, mut cells)| {
                cells.resize(cols, SharedString::default());
                Row { id, cells }
            })
            .collect();
        Self {
            next_id: rows.len(),
            kinds,
            rows,
            anchor: (0, 0),
            cursor: (0, 0),
            sort: None,
            history: Vec::new(),
            future: Vec::new(),
        }
    }

    fn cols(&self) -> usize {
        self.kinds.len()
    }

    fn cell(&self, r: usize, c: usize) -> SharedString {
        self.rows
            .get(r)
            .and_then(|row| row.cells.get(c))
            .cloned()
            .unwrap_or_default()
    }

    /// Top-left and bottom-right corners of the selection.
    fn range(&self) -> ((usize, usize), (usize, usize)) {
        let (a, b) = (self.anchor, self.cursor);
        ((a.0.min(b.0), a.1.min(b.1)), (a.0.max(b.0), a.1.max(b.1)))
    }

    fn select(&mut self, cell: (usize, usize), extend: bool) {
        let r = cell.0.min(self.rows.len().saturating_sub(1));
        let c = cell.1.min(self.cols().saturating_sub(1));
        self.cursor = (r, c);
        if !extend {
            self.anchor = self.cursor;
        }
    }

    fn step(&mut self, dr: isize, dc: isize, extend: bool) {
        let r = (self.cursor.0 as isize + dr).clamp(0, self.rows.len() as isize - 1);
        let c = (self.cursor.1 as isize + dc).clamp(0, self.cols() as isize - 1);
        self.select((r as usize, c as usize), extend);
    }

    /// Tab order: right, then the next row's first cell.
    fn tab(&mut self, back: bool) {
        let (r, c) = self.cursor;
        let last = self.cols() - 1;
        let next = match (back, c) {
            (false, c) if c < last => (r, c + 1),
            (false, _) => ((r + 1).min(self.rows.len() - 1), 0),
            (true, 0) => (r.saturating_sub(1), if r == 0 { 0 } else { last }),
            (true, c) => (r, c - 1),
        };
        self.select(next, false);
    }

    fn checkpoint(&mut self) {
        self.history.push(Snapshot {
            rows: self.rows.clone(),
            sort: self.sort,
        });
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        self.future.clear();
    }

    fn restore(&mut self, snap: Snapshot) -> Snapshot {
        let current = Snapshot {
            rows: std::mem::replace(&mut self.rows, snap.rows),
            sort: std::mem::replace(&mut self.sort, snap.sort),
        };
        let last = self.rows.len().saturating_sub(1);
        self.cursor.0 = self.cursor.0.min(last);
        self.anchor.0 = self.anchor.0.min(last);
        current
    }

    fn undo(&mut self) -> bool {
        match self.history.pop() {
            Some(snap) => {
                let current = self.restore(snap);
                self.future.push(current);
                true
            }
            None => false,
        }
    }

    fn redo(&mut self) -> bool {
        match self.future.pop() {
            Some(snap) => {
                let current = self.restore(snap);
                self.history.push(current);
                true
            }
            None => false,
        }
    }

    /// Writes a checked value; the caller checkpoints first.
    fn write(&mut self, r: usize, c: usize, raw: &str) -> Result<bool, String> {
        let value = normalize(self.kinds[c], raw)?;
        let Some(cell) = self.rows.get_mut(r).and_then(|row| row.cells.get_mut(c)) else {
            return Ok(false);
        };
        if *cell == value {
            return Ok(false);
        }
        *cell = value;
        if self.sort.is_some_and(|(sc, _)| sc == c) {
            self.sort = None;
        }
        Ok(true)
    }

    /// Commits one edited cell as its own undo step.
    fn set(&mut self, r: usize, c: usize, raw: &str) -> Result<bool, String> {
        let value = normalize(self.kinds[c], raw)?;
        if self.cell(r, c) == value {
            return Ok(false);
        }
        self.checkpoint();
        self.write(r, c, raw)
    }

    fn clear_selection(&mut self) -> usize {
        let ((r0, c0), (r1, c1)) = self.range();
        let filled = (r0..=r1)
            .flat_map(|r| (c0..=c1).map(move |c| (r, c)))
            .filter(|(r, c)| !self.cell(*r, *c).is_empty())
            .count();
        if filled == 0 {
            return 0;
        }
        self.checkpoint();
        for r in r0..=r1 {
            for c in c0..=c1 {
                let _ = self.write(r, c, "");
            }
        }
        filled
    }

    /// The selection as tab-separated rows.
    fn copy(&self) -> String {
        let ((r0, c0), (r1, c1)) = self.range();
        (r0..=r1)
            .map(|r| {
                (c0..=c1)
                    .map(|c| self.cell(r, c).to_string())
                    .collect::<Vec<_>>()
                    .join("\t")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Pastes tab-separated text at the selection's top-left corner. Values a
    /// column can't take are skipped. Returns (written, skipped).
    fn paste(&mut self, text: &str) -> (usize, usize) {
        let grid: Vec<Vec<&str>> = text
            .trim_end_matches(['\n', '\r'])
            .lines()
            .map(|l| l.trim_end_matches('\r').split('\t').collect())
            .collect();
        if grid.is_empty() {
            return (0, 0);
        }
        let ((r0, c0), _) = self.range();
        self.checkpoint();
        let (mut written, mut skipped) = (0, 0);
        for (dr, line) in grid.iter().enumerate() {
            for (dc, value) in line.iter().enumerate() {
                let (r, c) = (r0 + dr, c0 + dc);
                if r >= self.rows.len() || c >= self.cols() {
                    continue;
                }
                match self.write(r, c, value) {
                    Ok(true) => written += 1,
                    Ok(false) => {}
                    Err(_) => skipped += 1,
                }
            }
        }
        if written == 0 {
            self.history.pop();
        }
        let r1 = (r0 + grid.len() - 1).min(self.rows.len() - 1);
        let c1 = (c0 + grid.iter().map(Vec::len).max().unwrap_or(1) - 1).min(self.cols() - 1);
        self.anchor = (r0, c0);
        self.cursor = (r1, c1);
        (written, skipped)
    }

    /// Sorts by `col` as an undo step: ascending first, descending when
    /// sorted that way already.
    fn sort_by(&mut self, col: usize) {
        let desc = self.sort == Some((col, false));
        self.checkpoint();
        self.order(col, desc);
    }

    /// Orders the rows by `col`; empty values go last either way.
    fn order(&mut self, col: usize, desc: bool) {
        let kind = self.kinds[col];
        // Keep the selected row selected while it moves.
        let selected = self.rows.get(self.cursor.0).map(|r| r.id);
        self.rows.sort_by(|a, b| {
            let (x, y) = (&a.cells[col], &b.cells[col]);
            let ord = match (x.is_empty(), y.is_empty()) {
                (true, false) => return std::cmp::Ordering::Greater,
                (false, true) => return std::cmp::Ordering::Less,
                _ if kind.numeric() => number(x).total_cmp(&number(y)),
                _ => x.to_lowercase().cmp(&y.to_lowercase()),
            };
            if desc { ord.reverse() } else { ord }
        });
        self.sort = Some((col, desc));
        if let Some(pos) = selected.and_then(|id| self.rows.iter().position(|r| r.id == id)) {
            let c = self.cursor.1;
            self.anchor = (pos, c);
            self.cursor = (pos, c);
        }
    }

    fn add_row(&mut self) -> usize {
        self.checkpoint();
        self.rows.push(Row {
            id: self.next_id,
            cells: vec![SharedString::default(); self.cols()],
        });
        self.next_id += 1;
        self.rows.len() - 1
    }

    fn values(&self) -> Vec<Vec<SharedString>> {
        self.rows.iter().map(|r| r.cells.clone()).collect()
    }
}

/// Checks and canonicalizes typed input for a column.
fn normalize(kind: ColumnKind, raw: &str) -> Result<SharedString, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(SharedString::default());
    }
    match kind {
        ColumnKind::Text | ColumnKind::Tag => Ok(raw.to_string().into()),
        ColumnKind::Number | ColumnKind::Currency => {
            let cleaned: String = raw
                .chars()
                .filter(|c| !matches!(c, '$' | ',' | ' ' | '_'))
                .collect();
            let v: f64 = cleaned
                .parse()
                .map_err(|_| "expects a number".to_string())?;
            Ok(if v.fract() == 0. {
                format!("{}", v as i64)
            } else {
                format!("{v:.2}")
            }
            .into())
        }
        ColumnKind::Date => parse_date(raw)
            .map(|(y, m, d)| format!("{y:04}-{m:02}-{d:02}").into())
            .ok_or_else(|| "expects a date like 2026-11-04".to_string()),
    }
}

fn number(s: &str) -> f64 {
    s.parse().unwrap_or(0.)
}

fn parse_date(s: &str) -> Option<(i32, u32, u32)> {
    let mut parts = s.split('-');
    let y: i32 = parts.next()?.parse().ok()?;
    let m: u32 = parts.next()?.parse().ok()?;
    let d: u32 = parts.next()?.parse().ok()?;
    let days_in = [
        31,
        if y % 4 == 0 { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (parts.next().is_none() && (1..=12).contains(&m) && d >= 1 && d <= days_in[m as usize - 1])
        .then_some((y, m, d))
}

/// Days since 1970-01-01 (Howard Hinnant's civil algorithm).
fn day_number((y, m, d): (i32, u32, u32)) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// How a stored value reads. Dates use History's `%b %-d, %Y`.
fn display(kind: ColumnKind, raw: &str) -> String {
    match kind {
        ColumnKind::Currency if !raw.is_empty() => {
            let v = number(raw);
            if v.fract() == 0. {
                format!("${}", grouped(v as i64))
            } else {
                format!(
                    "${}.{:02}",
                    grouped(v.trunc() as i64),
                    (v.fract() * 100.).round() as i64
                )
            }
        }
        ColumnKind::Number if !raw.is_empty() => {
            let v = number(raw);
            if v.fract() == 0. {
                grouped(v as i64)
            } else {
                raw.to_string()
            }
        }
        ColumnKind::Date => match parse_date(raw) {
            Some((y, m, d)) => {
                const MONTHS: [&str; 12] = [
                    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
                    "Dec",
                ];
                format!("{} {d}, {y}", MONTHS[m as usize - 1])
            }
            None => raw.to_string(),
        },
        _ => raw.to_string(),
    }
}

/// Thousands separators, as pink's Usage page writes large counts.
fn grouped(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// The order a sort reads in, for the card's description (pink's Breakdown
/// card says "largest first" / "newest first").
fn order_words(kind: ColumnKind, desc: bool) -> &'static str {
    match (kind, desc) {
        (ColumnKind::Number | ColumnKind::Currency, false) => "smallest first",
        (ColumnKind::Number | ColumnKind::Currency, true) => "largest first",
        (ColumnKind::Date, false) => "earliest first",
        (ColumnKind::Date, true) => "latest first",
        (_, false) => "A to Z",
        (_, true) => "Z to A",
    }
}

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

// ---------------------------------------------------------------------------
// Column resizing (pink History: the pair of columns either side of a
// divider keeps its total width, so no drag makes the table overflow).
// ---------------------------------------------------------------------------

/// The drag payload of a column resize handle.
struct ColumnResize;

/// The drag's ghost: nothing follows the pointer (pink `HistoryResizeGhost`).
struct ResizeGhost;

impl Render for ResizeGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// Where a resize started: the divider between `left` and `left + 1`.
#[derive(Clone, Copy, Debug)]
struct ResizeAnchor {
    left: usize,
    start_x: f32,
    left_width: f32,
    right_width: f32,
}

/// The pair's widths after moving their divider by `delta` (pink
/// `resized_history_column_widths`).
fn resized(anchor: ResizeAnchor, delta: f32) -> (f32, f32) {
    let total = anchor.left_width + anchor.right_width;
    let floor = MIN_COLUMN.min(total / 2.);
    let left = (anchor.left_width + delta).clamp(floor, total - floor);
    (left, total - left)
}

// ---------------------------------------------------------------------------
// State entity: the sheet plus the in-place editor and transient UI.
// ---------------------------------------------------------------------------

struct State {
    sheet: Sheet,
    names: Vec<SharedString>,
    /// Live column widths (the builder's are the defaults).
    widths: Vec<f32>,
    resize: Option<ResizeAnchor>,
    /// The row Add row made, which folds in.
    added: Option<usize>,
    /// Cell being edited.
    editing: Option<(usize, usize)>,
    input: Entity<TextInput>,
    focus: FocusHandle,
    /// A mouse drag is extending the selection.
    selecting: bool,
    /// A short footer message ("Copied 6 cells"), whether it is an error,
    /// a count that replays its arrival, and the task that clears it.
    flash: Option<(SharedString, bool)>,
    flash_seq: usize,
    flash_task: Option<Task<()>>,
    /// Bumped on every change, so the owner can be told after render.
    revision: usize,
    reported: usize,
    _subscription: Subscription,
}

impl State {
    fn new(
        sheet: Sheet,
        names: Vec<SharedString>,
        widths: Vec<f32>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx));
        let subscription = cx.subscribe_in(&input, window, Self::on_input);
        Self {
            sheet,
            names,
            widths,
            resize: None,
            added: None,
            editing: None,
            input,
            focus: cx.focus_handle(),
            selecting: false,
            flash: None,
            flash_seq: 0,
            flash_task: None,
            revision: 0,
            reported: 0,
            _subscription: subscription,
        }
    }

    fn on_input(
        &mut self,
        _: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.is_none() {
            return;
        }
        let moved = match event {
            InputEvent::Submit(_) => Some((1, 0)),
            InputEvent::MoveDown => Some((1, 0)),
            InputEvent::MoveUp => Some((-1, 0)),
            InputEvent::Tab => Some((0, 1)),
            InputEvent::Escape => {
                self.editing = None;
                window.focus(&self.focus, cx);
                cx.notify();
                return;
            }
            InputEvent::Blur => {
                // Clicking away commits; a value the column can't take is dropped.
                if !self.commit(cx) {
                    self.editing = None;
                }
                cx.notify();
                return;
            }
            _ => None,
        };
        if let Some((dr, dc)) = moved
            && self.commit(cx)
        {
            if dc != 0 {
                self.sheet.tab(false);
            } else {
                self.sheet.step(dr, 0, false);
            }
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// Writes the editor's text into its cell. False (and a message) when the
    /// column rejects it; the editor then stays open.
    fn commit(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((r, c)) = self.editing else {
            return true;
        };
        let text = self.input.read(cx).text();
        match self.sheet.set(r, c, &text) {
            Ok(changed) => {
                if changed {
                    self.revision += 1;
                }
                self.editing = None;
                true
            }
            Err(why) => {
                let name = self.names.get(c).cloned().unwrap_or_default();
                self.say(format!("{name} {why}"), true, cx);
                false
            }
        }
    }

    fn start_edit(&mut self, initial: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let (r, c) = self.sheet.cursor;
        if r >= self.sheet.rows.len() {
            return;
        }
        self.sheet.anchor = self.sheet.cursor;
        let text = initial.unwrap_or_else(|| self.sheet.cell(r, c).to_string());
        self.input.update(cx, |input, cx| input.set_text(text, cx));
        self.editing = Some((r, c));
        TextInput::focus(&self.input, window, cx);
        cx.notify();
    }

    fn say(&mut self, message: impl Into<SharedString>, error: bool, cx: &mut Context<Self>) {
        self.flash = Some((message.into(), error));
        self.flash_seq += 1;
        self.flash_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2200))
                .await;
            let _ = this.update(cx, |s, cx| {
                s.flash = None;
                cx.notify();
            });
        }));
    }

    fn changed(&mut self) {
        self.revision += 1;
    }
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------

/// A tag inside a row (pink `history.rs` `render_ref`): 16 tall, `px 5`,
/// radius 4, the tone at 0.07 behind 11 px copy in `readable_tone(tone,
/// 0.9)`, capped at 112 and truncating.
fn row_tag(label: impl Into<SharedString>, tone: Hsla, t: &Pink) -> gpui::Div {
    div()
        .h(px(16.))
        .max_w(px(TAG_MAX_W))
        .px(px(5.))
        .flex_none()
        .flex()
        .items_center()
        .rounded(px(radius::XS))
        .bg(tone.opacity(0.07))
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(t.readable_tone(tone, 0.9))
        .child(div().min_w_0().truncate().child(label.into()))
}

/// One entry of the footer legend (pink `command_palette.rs`
/// `command_key_hint`): a `kbd_hint` and its 11 px verb in `text_muted`,
/// gap 5.
fn key_legend(keys: impl Into<SharedString>, label: &'static str, t: &Pink) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .child(pink::kbd_hint(keys, t))
        .child(
            div()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child(label),
        )
}

impl RenderOnce for DataTable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let kinds: Vec<ColumnKind> = self.columns.iter().map(|c| c.kind).collect();
        let names: Vec<SharedString> = self.columns.iter().map(|c| c.name.clone()).collect();
        let defaults: Vec<f32> = self.columns.iter().map(|c| c.width).collect();
        let initial = self.rows.clone();
        let sorted = self.sorted.filter(|(c, _)| *c < kinds.len());
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            {
                let (names, defaults) = (names.clone(), defaults.clone());
                move |window, cx| {
                    let mut sheet = Sheet::new(kinds, initial);
                    if let Some((c, desc)) = sorted {
                        sheet.order(c, desc);
                    }
                    State::new(sheet, names, defaults, window, cx)
                }
            },
        );
        if self.columns.is_empty() {
            return pink::card(&t).id(self.id.clone());
        }
        // Keep the live widths in step with the columns the owner passes.
        if state.read(cx).widths.len() != defaults.len() {
            state.update(cx, |s, _| {
                s.widths = defaults.clone();
                s.names = names.clone();
            });
        }

        // Tell the owner about changes made since the last render.
        if let Some(handler) = self.on_change.clone() {
            let (revision, reported, values) = {
                let s = state.read(cx);
                (s.revision, s.reported, s.sheet.values())
            };
            if revision != reported {
                state.update(cx, |s, _| s.reported = revision);
                handler(&values, window, cx);
            }
        }

        let s = state.read(cx);
        let sheet = s.sheet.clone();
        let widths = s.widths.clone();
        let editing = s.editing;
        let added = s.added;
        let input = s.input.clone();
        let focus = s.focus.clone();
        let flash = s.flash.clone();
        let flash_seq = s.flash_seq;
        // The editor's own focus counts as the grid's: it sits in the active cell.
        let focused = focus.contains_focused(window, cx) || input.read(cx).is_focused(window);
        let columns = self.columns.clone();
        // Column left edges inside a row, after the gutter.
        let offsets: Vec<f32> = widths
            .iter()
            .scan(GUTTER_W, |x, w| {
                let at = *x;
                *x += w;
                Some(at)
            })
            .collect();
        let table_w = GUTTER_W + widths.iter().sum::<f32>();
        let ((r0, c0), (r1, c1)) = sheet.range();
        let today = self.today.as_deref().and_then(parse_date).map(day_number);
        let muted = t.readable_muted(0.85);

        // --- Section header -------------------------------------------------
        // pink `widgets::section_header`: the title, a description that
        // follows the sort (the Breakdown card's "largest first"), and the
        // controls on the title's line.
        let description = match sheet.sort {
            Some((c, desc)) => format!(
                "{} · sorted by {}, {}",
                plural(sheet.rows.len(), "row"),
                columns[c].name,
                order_words(columns[c].kind, desc)
            ),
            None => plural(sheet.rows.len(), "row"),
        };
        let history_button = |id: &str, icon: &'static str, enabled: bool, redo: bool| {
            let state = state.clone();
            let focus = focus.clone();
            pink::icon_button_enabled(
                SharedString::from(format!("{key}-{id}")),
                icon,
                ControlSize::Sm,
                enabled,
                &t,
            )
            .tooltip(pink::text_tooltip(if redo { "Redo" } else { "Undo" }))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .when(enabled, |b| {
                b.on_click(move |_, window, cx| {
                    window.focus(&focus, cx);
                    state.update(cx, |s, cx| {
                        let done = if redo { s.sheet.redo() } else { s.sheet.undo() };
                        if done {
                            s.changed();
                            s.say(if redo { "Redone" } else { "Undone" }, false, cx);
                        }
                        cx.notify();
                    })
                })
            })
        };
        let add_button = {
            let state = state.clone();
            let focus = focus.clone();
            pink::button(
                SharedString::from(format!("{key}-add")),
                ButtonKind::Secondary,
                ControlSize::Sm,
                &t,
            )
            .child(pink::glyph(ICON_PLUS, ControlSize::Sm))
            .child("Add row")
            .on_click(move |_, window, cx| {
                window.focus(&focus, cx);
                state.update(cx, |s, cx| {
                    if s.commit(cx) {
                        let r = s.sheet.add_row();
                        s.added = s.sheet.rows.get(r).map(|row| row.id);
                        s.sheet.select((r, 0), false);
                        s.changed();
                        s.start_edit(None, window, cx);
                    }
                    cx.notify();
                })
            })
        };
        let section_header = pink::section_header(self.title.clone(), Some(description.into()), &t)
            .child(
                pink::row_controls()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S2))
                            .child(history_button(
                                "undo",
                                ICON_UNDO,
                                !sheet.history.is_empty(),
                                false,
                            ))
                            .child(history_button(
                                "redo",
                                ICON_REDO,
                                !sheet.future.is_empty(),
                                true,
                            )),
                    )
                    .child(add_button),
            );

        // --- Header row -------------------------------------------------------
        // pink `usage_table_row(header)`: 30 tall, 11/14 in
        // `readable_muted(0.85)`, no fill and no rule of its own. A header
        // brightens under the pointer (it sorts), and stays bright while its
        // column is sorted or selected.
        let header_cells = columns.iter().enumerate().map(|(c, col)| {
            let hkey = format!("{key}-head-{c}");
            let sort_state = state.clone();
            let focus = focus.clone();
            let lit = focused && (c0..=c1).contains(&c);
            let sorted = sheet.sort.filter(|(sc, _)| *sc == c).map(|(_, desc)| desc);
            let rest = if lit || sorted.is_some() {
                t.text
            } else {
                muted
            };
            let tone = motion::hover_blend(&hkey, rest, t.text);
            div()
                .id(SharedString::from(hkey.clone()))
                .flex_none()
                .w(px(widths[c]))
                .h_full()
                .px(px(CELL_PAD))
                .flex()
                .items_center()
                .gap(px(space::S4))
                .overflow_hidden()
                // Numeric headers sit over their right-aligned figures, the
                // sort glyph leading.
                .when(col.kind.numeric(), |d| d.flex_row_reverse())
                .cursor_pointer()
                .text_color(tone)
                .on_hover(motion::hover_listener(hkey))
                .on_click(move |_, window, cx| {
                    window.focus(&focus, cx);
                    sort_state.update(cx, |s, cx| {
                        if s.commit(cx) {
                            s.sheet.sort_by(c);
                            s.changed();
                        }
                        cx.notify();
                    })
                })
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .child(col.name.clone()),
                )
                .when_some(sorted, |d, desc| {
                    d.child(pink::line_glyph(
                        if desc {
                            glyphs::ALT_ARROW_DOWN
                        } else {
                            glyphs::ALT_ARROW_UP
                        },
                        icon_size::XS,
                        tone,
                    ))
                })
        });
        // pink History `column_resize_handle`: 6 px centered on the divider,
        // washing to `border_strong @ 0.7` under the pointer. Painted after
        // the headers so its narrow hitbox wins over the header's click.
        let handles = (1..columns.len()).map(|c| {
            let hkey = SharedString::from(format!("{key}-resize-{c}"));
            let hover = t.border_strong;
            let down_state = state.clone();
            let up_state = state.clone();
            let reset = defaults.clone();
            div()
                .id(hkey.clone())
                .absolute()
                .left(px(offsets[c] - HANDLE_W / 2.))
                .top_0()
                .bottom_0()
                .w(px(HANDLE_W))
                .cursor_col_resize()
                .hover_fill(hkey, hover.opacity(0.), hover.opacity(0.7))
                .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                    down_state.update(cx, |s, _| {
                        s.resize = Some(ResizeAnchor {
                            left: c - 1,
                            start_x: f32::from(ev.position.x),
                            left_width: s.widths[c - 1],
                            right_width: s.widths[c],
                        });
                    })
                })
                .on_drag(ColumnResize, |_, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| ResizeGhost)
                })
                .on_mouse_up(MouseButton::Left, move |ev: &MouseUpEvent, _, cx| {
                    up_state.update(cx, |s, cx| {
                        if ev.click_count == 2 {
                            s.widths = reset.clone();
                            cx.notify();
                        }
                        s.resize = None;
                    })
                })
        });
        let header_row = div()
            .relative()
            .h(px(HEAD_H))
            .w(px(table_w))
            .flex()
            .items_center()
            .type_style(TextStyle::Caption)
            .child(div().flex_none().w(px(GUTTER_W)))
            .children(header_cells)
            .children(handles);

        // --- Rows ---------------------------------------------------------------
        // pink `usage_table_row`: 36 tall under a `border` hairline, 13/18,
        // primary columns in `text`, supporting ones muted. History's hover:
        // the row blends to `fill(Quaternary)`. Rows are opaque on the card so
        // they cover each other as they slide into a new sort order.
        let card_bg = t.card_glass_bg();
        let row_wash = t.fill(FillTier::Quaternary);
        let rows = sheet.rows.iter().enumerate().map(|(r, row)| {
            let rkey = format!("{key}-row-{}", row.id);
            let top = motion::follow(&format!("{rkey}-top"), r as f32 * ROW_H, motion::RESORT);
            // A new row folds in like History's entering rows (`COLLAPSE`,
            // opacity 0.35 → 1).
            let arrival = if added == Some(row.id) {
                motion::follow_from(&format!("{rkey}-in"), 0., 1., motion::COLLAPSE)
            } else {
                1.
            };
            let row_lit = focused && (r0..=r1).contains(&r);
            let gutter_state = state.clone();
            let gutter_focus = focus.clone();
            let last_col = columns.len() - 1;
            let gutter = div()
                .id(SharedString::from(format!("{key}-gutter-{}", row.id)))
                .flex_none()
                .w(px(GUTTER_W))
                .h_full()
                .pr(px(GUTTER_PAD))
                .flex()
                .items_center()
                .justify_end()
                .cursor_pointer()
                .font_family(pink::MONO)
                .text_size(px(11.))
                .text_color(if row_lit {
                    t.text
                } else {
                    t.readable_faint(0.8)
                })
                .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                    window.focus(&gutter_focus, cx);
                    gutter_state.update(cx, |s, cx| {
                        if ev.modifiers.shift {
                            s.sheet.cursor = (r, last_col);
                        } else {
                            s.sheet.anchor = (r, 0);
                            s.sheet.cursor = (r, last_col);
                        }
                        cx.notify();
                    })
                })
                .child(format!("{}", r + 1));
            let cells = columns.iter().enumerate().map(|(c, col)| {
                let raw = row.cells.get(c).cloned().unwrap_or_default();
                let down_state = state.clone();
                let move_state = state.clone();
                let cell_focus = focus.clone();
                let cell = div()
                    .id(SharedString::from(format!("{key}-cell-{}-{c}", row.id)))
                    .flex_none()
                    .w(px(widths[c]))
                    .h_full()
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                        if down_state.read(cx).editing == Some((r, c)) {
                            return;
                        }
                        window.focus(&cell_focus, cx);
                        down_state.update(cx, |s, cx| {
                            s.sheet.select((r, c), ev.modifiers.shift);
                            s.selecting = !ev.modifiers.shift;
                            if ev.click_count >= 2 {
                                s.selecting = false;
                                s.start_edit(None, window, cx);
                            }
                            cx.notify();
                        })
                    })
                    .on_mouse_move(move |ev: &MouseMoveEvent, _, cx| {
                        if ev.pressed_button != Some(MouseButton::Left) {
                            return;
                        }
                        move_state.update(cx, |s, cx| {
                            if s.selecting && s.sheet.cursor != (r, c) {
                                s.sheet.cursor = (r, c);
                                cx.notify();
                            }
                        })
                    });
                if editing == Some((r, c)) {
                    // The editor: the composer's input plate on the cell's
                    // selection plate, text where the value was. The focus
                    // ring is the active-cell ring drawn over it.
                    return cell.child(
                        div()
                            .w_full()
                            .h(px(PLATE_H))
                            .px(px(CELL_PAD))
                            .flex()
                            .items_center()
                            .rounded(px(radius::CONTROL))
                            .bg(t.input_glass_bg())
                            .text_color(t.text)
                            .child(div().flex_1().min_w_0().child(input.clone())),
                    );
                }
                let cell = cell.px(px(CELL_PAD)).when(col.kind.numeric(), |d| {
                    d.justify_end().font_features(pink::tabular_figures())
                });
                let tone = if col.secondary || col.kind == ColumnKind::Date {
                    muted
                } else {
                    t.text
                };
                let shown = display(col.kind, &raw);
                match col.kind {
                    ColumnKind::Tag if !raw.is_empty() => {
                        let tag_tone = col
                            .tones
                            .iter()
                            .find(|(v, _)| v.eq_ignore_ascii_case(&raw))
                            .map_or(Tone::Neutral, |(_, tone)| *tone);
                        cell.child(row_tag(shown, tag_tone.color(&t), &t))
                    }
                    ColumnKind::Date => {
                        // A renewal inside two weeks: the date stays quiet and
                        // one warning word says when (pink shows status as one
                        // tinted word, never a tinted row).
                        let soon = today.zip(parse_date(&raw)).and_then(|(now, d)| {
                            let days = day_number(d) - now;
                            (0..=14).contains(&days).then_some(days)
                        });
                        cell.gap(px(space::S6))
                            .text_color(tone)
                            .child(div().min_w_0().truncate().child(shown))
                            .when_some(soon, |d, days| {
                                d.child(
                                    div()
                                        .flex_none()
                                        .text_size(px(11.))
                                        .line_height(px(14.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(t.readable_tone(t.warning, 0.9))
                                        .child(match days {
                                            0 => "today".to_string(),
                                            1 => "tomorrow".to_string(),
                                            n => format!("in {n} days"),
                                        }),
                                )
                            })
                    }
                    _ => cell
                        .text_color(tone)
                        .child(div().min_w_0().truncate().child(shown)),
                }
            });
            div()
                .id(SharedString::from(rkey.clone()))
                .absolute()
                .left_0()
                .top(px(top))
                .h(px(ROW_H))
                .w(px(table_w))
                .flex()
                .items_center()
                .border_t_1()
                .border_color(t.border)
                .bg(pink::flatten(
                    motion::hover_blend(&rkey, row_wash.opacity(0.), row_wash),
                    card_bg,
                ))
                .on_hover(motion::hover_listener(rkey))
                .opacity(0.35 + 0.65 * arrival)
                .child(gutter)
                .children(cells)
        });

        // The selection, shown while the grid has focus: a wash over the
        // range and pink's 2 px inset focus ring on the active cell, both on
        // a plate 4 px inside the rows, gliding between cells (`QUICK`, the
        // segmented thumb's slide).
        let glide =
            |name: &str, v: f32| motion::follow(&format!("{key}-sel-{name}"), v, motion::QUICK);
        let range_box = (focused && (r0 != r1 || c0 != c1)).then(|| {
            let x = glide("rx", offsets[c0]);
            let y = glide("ry", r0 as f32 * ROW_H + PLATE_INSET);
            let w = glide("rw", offsets[c1] + widths[c1] - offsets[c0]);
            let h = glide("rh", (r1 - r0) as f32 * ROW_H + PLATE_H);
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h))
                .rounded(px(radius::CONTROL))
                .bg(t.tab_focused_fill())
        });
        let (cr, cc) = sheet.cursor;
        let ring = (focused && !sheet.rows.is_empty()).then(|| {
            let x = glide("cx", offsets[cc]);
            let y = glide("cy", cr as f32 * ROW_H + PLATE_INSET);
            let w = glide("cw", widths[cc]);
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(PLATE_H))
                .rounded(px(radius::CONTROL))
                .shadow(t.focus_ring_shadows())
        });

        let body = {
            let up_state = state.clone();
            let out_state = state.clone();
            let body_h = motion::follow(
                &format!("{key}-body-h"),
                ROW_H * sheet.rows.len() as f32,
                motion::COLLAPSE,
            );
            div()
                .id(SharedString::from(format!("{key}-body")))
                .relative()
                .w(px(table_w))
                .h(px(body_h))
                .type_style(TextStyle::Body)
                .children(rows)
                .children(range_box)
                .children(ring)
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    up_state.update(cx, |s, _| s.selecting = false)
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    out_state.update(cx, |s, _| s.selecting = false)
                })
        };

        // pink `render_usage_page` `table`: inset to the rows' edge (16),
        // below a `border` hairline, `pt 4 / pb 8`.
        let table = div()
            .w_full()
            .border_t_1()
            .border_color(t.border)
            .px(px(layout::ROW_PAD_X))
            .pt(px(space::S4))
            .pb(px(space::S8))
            .flex()
            .flex_col()
            .child(header_row)
            .child(body);

        // --- Footer -------------------------------------------------------------
        // pink's command palette footer (`px 16 / py 7`, a `hairline(0.06)`
        // rule, legends gap 12), led by a readout of the selection, or of the
        // numeric columns' totals while the grid isn't focused.
        let numeric_total = |c: usize| -> String {
            let sum: f64 = sheet.rows.iter().map(|row| number(&row.cells[c])).sum();
            display(columns[c].kind, &format!("{}", sum.round() as i64))
        };
        let summary: SharedString = if sheet.rows.is_empty() {
            "No rows".into()
        } else if !focused {
            let totals: Vec<String> = (0..columns.len())
                .filter(|c| columns[*c].kind.numeric())
                .map(|c| format!("{} {}", columns[c].name, numeric_total(c)))
                .collect();
            if totals.is_empty() {
                plural(sheet.rows.len(), "row").into()
            } else {
                format!("Totals · {}", totals.join(" · ")).into()
            }
        } else if r0 == r1 && c0 == c1 {
            format!("{} · row {}", columns[c0].name, r0 + 1).into()
        } else {
            let cells = (r1 - r0 + 1) * (c1 - c0 + 1);
            let mut text = plural(cells, "cell");
            let numeric: Vec<(ColumnKind, f64)> = (r0..=r1)
                .flat_map(|r| (c0..=c1).map(move |c| (r, c)))
                .filter(|(_, c)| columns[*c].kind.numeric())
                .filter_map(|(r, c)| {
                    let raw = sheet.cell(r, c);
                    (!raw.is_empty()).then(|| (columns[c].kind, number(&raw)))
                })
                .collect();
            if !numeric.is_empty() {
                let sum: f64 = numeric.iter().map(|(_, v)| v).sum();
                let avg = sum / numeric.len() as f64;
                let kind = numeric[0].0;
                let same = numeric.iter().all(|(k, _)| *k == kind);
                let fmt = |v: f64| {
                    let rounded = format!("{}", v.round() as i64);
                    if same {
                        display(kind, &rounded)
                    } else {
                        grouped(v.round() as i64)
                    }
                };
                text.push_str(&format!(" · Sum {} · Avg {}", fmt(sum), fmt(avg)));
            }
            text.into()
        };
        let readout = match &flash {
            // A message arrives like pink's working-line status: a fade and
            // a 2 px settle (`STATUS_SWAP`).
            Some((message, error)) => motion::status_swap(
                pink::loaders::label_id(&format!("{key}-flash"), &format!("{flash_seq}")),
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(if *error {
                        t.readable_tone(t.danger, 0.9)
                    } else {
                        t.text
                    })
                    .child(message.clone()),
            )
            .into_any_element(),
            None => div()
                .min_w_0()
                .truncate()
                .text_color(muted)
                .child(summary)
                .into_any_element(),
        };
        let mod_key = if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Ctrl "
        };
        let footer = div()
            .flex_none()
            .px(px(layout::ROW_PAD_X))
            .py(px(7.))
            .border_t_1()
            .border_color(t.hairline(0.06))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(space::S12))
            .line_height(px(16.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .text_size(px(12.))
                    .font_features(pink::tabular_figures())
                    .child(readout),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S12))
                    .child(key_legend("↵", "Edit", &t))
                    .child(key_legend(format!("{mod_key}C"), "Copy", &t))
                    .child(key_legend(format!("{mod_key}Z"), "Undo", &t)),
            );

        // --- Keyboard -----------------------------------------------------------
        let key_state = state.clone();
        let on_key = move |ev: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if key_state.read(cx).editing.is_some() {
                return;
            }
            let ks = &ev.keystroke;
            let m = ks.modifiers;
            let shift = m.shift;
            let secondary = m.secondary();
            let k = ks.key.as_str();
            let handled = key_state.update(cx, |s, cx| {
                let sheet = &mut s.sheet;
                if sheet.rows.is_empty() {
                    return false;
                }
                let last_row = sheet.rows.len() - 1;
                let last_col = sheet.cols() - 1;
                match k {
                    "up" if secondary => sheet.select((0, sheet.cursor.1), shift),
                    "down" if secondary => sheet.select((last_row, sheet.cursor.1), shift),
                    "left" if secondary => sheet.select((sheet.cursor.0, 0), shift),
                    "right" if secondary => sheet.select((sheet.cursor.0, last_col), shift),
                    "up" => sheet.step(-1, 0, shift),
                    "down" => sheet.step(1, 0, shift),
                    "left" => sheet.step(0, -1, shift),
                    "right" => sheet.step(0, 1, shift),
                    "home" => sheet.select((sheet.cursor.0, 0), shift),
                    "end" => sheet.select((sheet.cursor.0, last_col), shift),
                    "tab" => sheet.tab(shift),
                    "enter" | "f2" => s.start_edit(None, window, cx),
                    "escape" => sheet.anchor = sheet.cursor,
                    "backspace" | "delete" => {
                        let n = sheet.clear_selection();
                        if n > 0 {
                            s.changed();
                            s.say(format!("Cleared {}", plural(n, "cell")), false, cx);
                        }
                    }
                    "a" if secondary => {
                        sheet.anchor = (0, 0);
                        sheet.cursor = (last_row, last_col);
                    }
                    "c" | "x" if secondary => {
                        let text = sheet.copy();
                        let ((r0, c0), (r1, c1)) = sheet.range();
                        let n = (r1 - r0 + 1) * (c1 - c0 + 1);
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                        if k == "x" {
                            sheet.clear_selection();
                            s.changed();
                            s.say(format!("Cut {}", plural(n, "cell")), false, cx);
                        } else {
                            s.say(format!("Copied {}", plural(n, "cell")), false, cx);
                        }
                    }
                    "v" if secondary => {
                        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else {
                            return true;
                        };
                        let (written, skipped) = s.sheet.paste(&text);
                        if written > 0 {
                            s.changed();
                        }
                        let mut msg = format!("Pasted {}", plural(written, "cell"));
                        if skipped > 0 {
                            msg.push_str(&format!(" · skipped {skipped} that didn't fit"));
                        }
                        s.say(msg, skipped > 0 && written == 0, cx);
                    }
                    "z" if secondary => {
                        let done = if shift { sheet.redo() } else { sheet.undo() };
                        if done {
                            s.changed();
                            s.say(if shift { "Redone" } else { "Undone" }, false, cx);
                        }
                    }
                    "y" if secondary => {
                        if sheet.redo() {
                            s.changed();
                            s.say("Redone", false, cx);
                        }
                    }
                    _ => {
                        // Typing replaces the cell, like a spreadsheet.
                        let typed = ks.key_char.clone().filter(|ch| {
                            !m.control
                                && !m.platform
                                && !m.function
                                && !ch.is_empty()
                                && ch.chars().all(|c| !c.is_control())
                        });
                        match typed {
                            Some(ch) => s.start_edit(Some(ch), window, cx),
                            None => return false,
                        }
                    }
                }
                cx.notify();
                true
            });
            if handled {
                cx.stop_propagation();
            }
        };

        // A resize drag moves the divider it started on.
        let drag_state = state.clone();
        let on_resize = move |ev: &DragMoveEvent<ColumnResize>, _: &mut Window, cx: &mut App| {
            drag_state.update(cx, |s, cx| {
                let Some(anchor) = s.resize else {
                    return;
                };
                let (left, right) =
                    resized(anchor, f32::from(ev.event.position.x) - anchor.start_x);
                if s.widths.get(anchor.left) != Some(&left) {
                    s.widths[anchor.left] = left;
                    s.widths[anchor.left + 1] = right;
                    cx.notify();
                }
            })
        };

        motion::keep_alive(window);
        // pink `widgets::section_card`'s surface (without the page's section
        // margin): radius 12, a `border` edge, `card_glass_bg()`, clipped.
        pink::card(&t)
            .id(self.id.clone())
            .w(px(table_w + 2. * layout::ROW_PAD_X + 2.))
            .overflow_hidden()
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .track_focus(&focus)
            .on_key_down(on_key)
            .on_drag_move(on_resize)
            .child(section_header)
            .child(table)
            .child(footer)
    }
}

fn demo() -> DataTable {
    let accounts = [
        [
            "Halcyon Robotics",
            "Active",
            "240",
            "18400",
            "2027-02-14",
            "Maya Okafor",
        ],
        [
            "Brightline Health",
            "Past due",
            "86",
            "5160",
            "2026-11-03",
            "Theo Lindqvist",
        ],
        [
            "Parcelwise",
            "Active",
            "24",
            "1150",
            "2026-12-19",
            "Maya Okafor",
        ],
        [
            "Northwind Labs",
            "Active",
            "112",
            "6720",
            "2027-01-08",
            "Ana Ruiz",
        ],
        [
            "Quillstack",
            "Trial",
            "6",
            "90",
            "2026-10-30",
            "Theo Lindqvist",
        ],
        [
            "Meridian Freight",
            "Active",
            "410",
            "29520",
            "2027-06-30",
            "Ana Ruiz",
        ],
        [
            "Copperleaf Studio",
            "Paused",
            "18",
            "860",
            "2026-11-21",
            "Jun Park",
        ],
        [
            "Tidepool Analytics",
            "Trial",
            "64",
            "3840",
            "2027-03-02",
            "Jun Park",
        ],
    ];
    accounts.iter().fold(
        DataTable::new("data-table", "Accounts")
            .today("2026-10-21")
            .column(TableColumn::new("Account", ColumnKind::Text, 184.))
            .column(
                TableColumn::new("Status", ColumnKind::Tag, 100.)
                    .tone("Active", Tone::Success)
                    .tone("Trial", Tone::Accent)
                    .tone("Past due", Tone::Warning),
            )
            .column(TableColumn::new("Seats", ColumnKind::Number, 76.).secondary())
            .column(TableColumn::new("MRR", ColumnKind::Currency, 100.))
            .column(TableColumn::new("Renews", ColumnKind::Date, 176.))
            .column(TableColumn::new("Owner", ColumnKind::Text, 136.).secondary())
            .sorted_by(3, true),
        |table, row| table.row(row.iter().copied()),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| demo())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet() -> Sheet {
        let rows = vec![
            vec!["b".into(), "20".into(), "2026-11-03".into()],
            vec!["a".into(), "5".into(), "2027-01-08".into()],
            vec!["c".into(), "110".into(), "".into()],
        ];
        Sheet::new(
            vec![ColumnKind::Text, ColumnKind::Currency, ColumnKind::Date],
            rows,
        )
    }

    #[test]
    fn normalizes_by_kind() {
        assert_eq!(normalize(ColumnKind::Currency, "$1,250").unwrap(), "1250");
        assert_eq!(normalize(ColumnKind::Number, " 12.5 ").unwrap(), "12.50");
        assert!(normalize(ColumnKind::Number, "twelve").is_err());
        assert_eq!(
            normalize(ColumnKind::Date, "2026-2-3").unwrap(),
            "2026-02-03"
        );
        assert!(normalize(ColumnKind::Date, "2026-02-30").is_err());
        assert_eq!(display(ColumnKind::Currency, "18400"), "$18,400");
        assert_eq!(display(ColumnKind::Date, "2027-02-14"), "Feb 14, 2027");
    }

    #[test]
    fn edit_undo_redo() {
        let mut s = sheet();
        assert_eq!(s.set(0, 1, "$30"), Ok(true));
        assert_eq!(s.cell(0, 1), "30");
        assert!(s.set(0, 1, "oops").is_err());
        assert!(s.undo());
        assert_eq!(s.cell(0, 1), "20");
        assert!(s.redo());
        assert_eq!(s.cell(0, 1), "30");
        assert!(!s.redo());
    }

    #[test]
    fn copy_and_paste_round_trip() {
        let mut s = sheet();
        s.select((0, 0), false);
        s.select((1, 1), true);
        assert_eq!(s.copy(), "b\t20\na\t5");
        s.select((1, 0), false);
        let (written, skipped) = s.paste("x\t7\ny\tnot money");
        assert_eq!((written, skipped), (3, 1));
        assert_eq!(s.cell(1, 0), "x");
        assert_eq!(s.cell(2, 1), "110");
        assert_eq!(s.range(), ((1, 0), (2, 1)));
        assert!(s.undo());
        assert_eq!(s.cell(1, 0), "a");
    }

    #[test]
    fn sort_keeps_selection_and_toggles() {
        let mut s = sheet();
        s.select((2, 1), false);
        s.sort_by(1);
        let money: Vec<_> = s.rows.iter().map(|r| r.cells[1].to_string()).collect();
        assert_eq!(money, ["5", "20", "110"]);
        assert_eq!(s.cursor, (2, 1));
        s.sort_by(1);
        assert_eq!(s.rows[0].cells[1], "110");
        // Empty values sort last either way.
        s.sort_by(2);
        assert_eq!(s.rows[2].cells[2], "");
        assert!(s.undo());
        assert_eq!(s.sort, Some((1, true)));
    }

    #[test]
    fn initial_order_is_not_an_undo_step() {
        let mut s = sheet();
        s.order(1, true);
        assert_eq!(s.rows[0].cells[1], "110");
        assert_eq!(s.sort, Some((1, true)));
        assert!(!s.undo());
    }

    #[test]
    fn clear_and_tab() {
        let mut s = sheet();
        s.select((0, 0), false);
        s.select((0, 2), true);
        assert_eq!(s.clear_selection(), 3);
        assert!(s.rows[0].cells.iter().all(|c| c.is_empty()));
        s.select((0, 2), false);
        s.tab(false);
        assert_eq!(s.cursor, (1, 0));
        s.tab(true);
        assert_eq!(s.cursor, (0, 2));
    }

    #[test]
    fn resizing_keeps_the_pair_width() {
        let anchor = ResizeAnchor {
            left: 0,
            start_x: 0.,
            left_width: 120.,
            right_width: 100.,
        };
        assert_eq!(resized(anchor, 30.), (150., 70.));
        // Neither side goes under the floor.
        assert_eq!(resized(anchor, 200.), (220. - MIN_COLUMN, MIN_COLUMN));
        assert_eq!(resized(anchor, -200.), (MIN_COLUMN, 220. - MIN_COLUMN));
    }

    #[test]
    fn day_numbers() {
        assert_eq!(day_number((1970, 1, 1)), 0);
        assert_eq!(day_number((2026, 10, 30)) - day_number((2026, 10, 21)), 9);
    }
}
