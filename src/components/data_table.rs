//! Data Table – a small spreadsheet: select ranges, edit in place, copy,
//! paste, undo and sort.
//!
//! Click a cell to select it, drag or Shift-click to select a range, click
//! a row number for the whole row. A selection box glides between cells as
//! the selection moves. Typing replaces the cell, Enter or a double-click
//! edits it, and Enter, Tab or ↑/↓ commit and move on, the way spreadsheets
//! do. Typed values are checked against the column's kind (numbers, money,
//! ISO dates). Copy and paste use tab-separated text, so ranges move to and
//! from other spreadsheets; Delete clears the selection; every change can be
//! undone. Clicking a header sorts by that column and the rows slide into
//! their new order. The status bar sums numeric selections.
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

use std::{rc::Rc, time::Duration};

use gpui::{
    AnyView, App, ClipboardItem, Context, ElementId, Entity, FocusHandle, FontWeight, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, SharedString, Subscription, Task, Window, div,
    prelude::*, px, svg,
};

use crate::{
    HoverFill,
    luna::{self, ControlSize, FillTier, Luna, Tone, space, text},
    motion::{self, MotionSpec},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Data Table";
pub const DESCRIPTION: &str = "A compact spreadsheet: range selection with a gliding cursor, typed in-place editing, tab-separated copy and paste, undo and redo, and sorting that slides the rows";

const ICON_GRID: &str = "icons/data-table/grid.svg";
const ICON_UNDO: &str = "icons/data-table/undo.svg";
const ICON_REDO: &str = "icons/data-table/redo.svg";
const ICON_PLUS: &str = "icons/data-table/plus.svg";
const ICON_SORT_UP: &str = "icons/data-table/sort-up.svg";
const ICON_SORT_DOWN: &str = "icons/data-table/sort-down.svg";

const ROW_H: f32 = 34.;
const HEAD_H: f32 = 32.;
const GUTTER_W: f32 = 40.;
/// The selection box moving between cells.
const CURSOR: MotionSpec = MotionSpec::new(140, motion::CubicBezier::new(0.2, 0.0, 0.0, 1.0));
/// Rows sliding into a new sort order.
const REORDER: MotionSpec = MotionSpec::new(420, motion::EASE_OUT_QUINT);

type ChangeHandler = Rc<dyn Fn(&[Vec<SharedString>], &mut Window, &mut App)>;

/// What a column holds; it decides alignment, display and input checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnKind {
    Text,
    /// A short label drawn as a badge (see [`TableColumn::tone`]).
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
    /// Badge tones for known [`ColumnKind::Tag`] values.
    pub tones: Vec<(SharedString, Tone)>,
}

impl TableColumn {
    pub fn new(name: impl Into<SharedString>, kind: ColumnKind, width: f32) -> Self {
        Self {
            name: name.into(),
            kind,
            width,
            tones: Vec::new(),
        }
    }

    pub fn tone(mut self, value: impl Into<SharedString>, tone: Tone) -> Self {
        self.tones.push((value.into(), tone));
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

    /// Sorts by `col`: ascending first, descending when sorted that way already.
    fn sort_by(&mut self, col: usize) {
        let desc = self.sort == Some((col, false));
        self.checkpoint();
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

/// "C" for the third column.
fn column_letter(c: usize) -> char {
    (b'A' + (c % 26) as u8) as char
}

// ---------------------------------------------------------------------------
// State entity: the sheet plus the in-place editor and transient UI.
// ---------------------------------------------------------------------------

struct State {
    sheet: Sheet,
    /// Cell being edited.
    editing: Option<(usize, usize)>,
    input: Entity<TextInput>,
    focus: FocusHandle,
    /// A mouse drag is extending the selection.
    selecting: bool,
    /// A short status message ("Copied 6 cells") and the task that clears it.
    flash: Option<(SharedString, bool)>,
    flash_task: Option<Task<()>>,
    /// Bumped on every change, so the owner can be told after render.
    revision: usize,
    reported: usize,
    _subscription: Subscription,
}

impl State {
    fn new(sheet: Sheet, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx));
        let subscription = cx.subscribe_in(&input, window, Self::on_input);
        Self {
            sheet,
            editing: None,
            input,
            focus: cx.focus_handle(),
            selecting: false,
            flash: None,
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
                window.focus(&self.focus);
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
            window.focus(&self.focus);
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
                self.say(format!("{} {why}", column_letter(c)), true, cx);
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
        self.input.read(cx).focus(window);
        cx.notify();
    }

    fn say(&mut self, message: impl Into<SharedString>, error: bool, cx: &mut Context<Self>) {
        self.flash = Some((message.into(), error));
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

fn plural(n: usize, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

impl RenderOnce for DataTable {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let kinds: Vec<ColumnKind> = self.columns.iter().map(|c| c.kind).collect();
        let initial = self.rows.clone();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| State::new(Sheet::new(kinds, initial), window, cx),
        );
        if self.columns.is_empty() {
            return luna::card(&t).id(self.id.clone());
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
        let editing = s.editing;
        let input = s.input.clone();
        let focus = s.focus.clone();
        let flash = s.flash.clone();
        // The editor's own focus counts as the grid's: it sits in the active cell.
        let focused = focus.contains_focused(window, cx) || input.read(cx).is_focused(window);
        let columns = self.columns.clone();
        let offsets: Vec<f32> = columns
            .iter()
            .scan(GUTTER_W, |x, c| {
                let at = *x;
                *x += c.width;
                Some(at)
            })
            .collect();
        let width = GUTTER_W + columns.iter().map(|c| c.width).sum::<f32>();
        let ((r0, c0), (r1, c1)) = sheet.range();
        let today = self
            .today
            .as_deref()
            .and_then(|d| parse_date(d))
            .map(day_number);

        // --- Toolbar ------------------------------------------------------
        let history_button = |id: &str, icon: &'static str, enabled: bool, redo: bool| {
            let state = state.clone();
            let focus = focus.clone();
            luna::icon_button(
                SharedString::from(format!("{key}-{id}")),
                icon,
                ControlSize::Sm,
                &t,
            )
            .when(!enabled, |b| b.opacity(0.35).cursor_default())
            .on_click(move |_, window, cx| {
                window.focus(&focus);
                state.update(cx, |s, cx| {
                    let done = if redo { s.sheet.redo() } else { s.sheet.undo() };
                    if done {
                        s.changed();
                        s.say(if redo { "Redone" } else { "Undone" }, false, cx);
                    }
                    cx.notify();
                })
            })
        };
        let toolbar = div()
            .h(px(48.))
            .px(px(space::S16))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .child(svg().path(ICON_GRID).size(px(16.)).text_color(if t.dark {
                        t.text_muted
                    } else {
                        t.accent
                    }))
                    .child(
                        div()
                            .text_size(px(text::CALLOUT))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(self.title.clone()),
                    )
                    .child(luna::badge(
                        plural(sheet.rows.len(), "row"),
                        Tone::Neutral,
                        &t,
                    )),
            )
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
            );

        // --- Header row ---------------------------------------------------
        let header = div()
            .h(px(HEAD_H))
            .flex()
            .bg(t.fill(FillTier::Quaternary))
            .border_y_1()
            .border_color(t.border)
            .child(
                div()
                    .flex_none()
                    .w(px(GUTTER_W))
                    .h_full()
                    .border_r_1()
                    .border_color(t.border),
            )
            .children(columns.iter().enumerate().map(|(c, col)| {
                let hkey = format!("{key}-head-{c}");
                let sort_state = state.clone();
                let focus = focus.clone();
                let lit = focused && c >= c0 && c <= c1;
                let sorted = sheet.sort.filter(|(sc, _)| *sc == c);
                let last = c + 1 == columns.len();
                div()
                    .id(SharedString::from(hkey.clone()))
                    .flex_none()
                    .w(px(col.width))
                    .h_full()
                    .px(px(space::S10))
                    .flex()
                    .items_center()
                    .gap(px(space::S4))
                    .when(col.kind.numeric(), |d| d.flex_row_reverse())
                    .when(!last, |d| d.border_r_1())
                    .border_color(t.border)
                    .cursor_pointer()
                    .text_size(px(text::FOOTNOTE))
                    .font_weight(FontWeight::MEDIUM)
                    .hover_fill_fg(
                        hkey,
                        t.fill(FillTier::Tertiary).opacity(0.),
                        t.fill(FillTier::Tertiary),
                        if lit || sorted.is_some() {
                            t.text
                        } else {
                            t.text_muted
                        },
                        t.text,
                    )
                    .on_click(move |_, window, cx| {
                        window.focus(&focus);
                        sort_state.update(cx, |s, cx| {
                            if s.commit(cx) {
                                s.sheet.sort_by(c);
                                s.changed();
                            }
                            cx.notify();
                        })
                    })
                    .child(col.name.clone())
                    .when_some(sorted, |d, (_, desc)| {
                        d.child(
                            svg()
                                .path(if desc { ICON_SORT_DOWN } else { ICON_SORT_UP })
                                .size(px(12.))
                                .text_color(t.accent),
                        )
                    })
            }));

        // --- Rows ---------------------------------------------------------
        let rows = sheet.rows.iter().enumerate().map(|(r, row)| {
            let top = motion::follow(&format!("{key}-row-{}", row.id), r as f32 * ROW_H, REORDER);
            let row_lit = focused && r >= r0 && r <= r1;
            let gutter_state = state.clone();
            let gutter_focus = focus.clone();
            let last_col = columns.len() - 1;
            div()
                .absolute()
                .left_0()
                .top(px(top))
                .h(px(ROW_H))
                .w(px(width))
                .flex()
                .border_b_1()
                .border_color(t.border)
                .bg(t.surface_card)
                .child(
                    div()
                        .id(SharedString::from(format!("{key}-gutter-{}", row.id)))
                        .flex_none()
                        .w(px(GUTTER_W))
                        .h_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_r_1()
                        .border_color(t.border)
                        .bg(if row_lit {
                            t.fill(FillTier::Tertiary)
                        } else {
                            t.fill(FillTier::Quaternary)
                        })
                        .cursor_pointer()
                        .font_family(luna::MONO)
                        .text_size(px(text::CAPTION))
                        .text_color(if row_lit { t.text } else { t.text_faint })
                        .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                            window.focus(&gutter_focus);
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
                        .child(format!("{}", r + 1)),
                )
                .children(columns.iter().enumerate().map(|(c, col)| {
                    let raw = row.cells.get(c).cloned().unwrap_or_default();
                    let is_editing = editing == Some((r, c));
                    let down_state = state.clone();
                    let move_state = state.clone();
                    let cell_focus = focus.clone();
                    let cell = div()
                        .id(SharedString::from(format!("{key}-cell-{}-{c}", row.id)))
                        .flex_none()
                        .w(px(col.width))
                        .h_full()
                        .px(px(space::S10))
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .when(c != last_col, |d| d.border_r_1())
                        .border_color(t.border)
                        .text_size(px(text::BODY))
                        .when(col.kind.numeric(), |d| {
                            d.justify_end()
                                .font_family(luna::MONO)
                                .text_size(px(text::FOOTNOTE))
                        })
                        .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
                            if down_state.read(cx).editing == Some((r, c)) {
                                return;
                            }
                            window.focus(&cell_focus);
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
                    if is_editing {
                        return cell
                            .justify_start()
                            .bg(t.input_bg)
                            .text_color(t.text)
                            .child(div().flex_1().child(input.clone()));
                    }
                    let shown = display(col.kind, &raw);
                    match col.kind {
                        ColumnKind::Tag if !raw.is_empty() => {
                            let tone = col
                                .tones
                                .iter()
                                .find(|(v, _)| v.eq_ignore_ascii_case(&raw))
                                .map_or(Tone::Neutral, |(_, tone)| *tone);
                            cell.child(luna::badge(shown, tone, &t))
                        }
                        ColumnKind::Date => {
                            let soon = today.zip(parse_date(&raw)).and_then(|(now, d)| {
                                let days = day_number(d) - now;
                                (0..=14).contains(&days).then_some(days)
                            });
                            cell.gap(px(space::S6))
                                .text_color(if soon.is_some() {
                                    t.warning
                                } else {
                                    t.text_muted
                                })
                                .child(div().truncate().child(shown))
                                .when_some(soon, |d, days| {
                                    d.child(
                                        div()
                                            .text_size(px(text::CAPTION))
                                            .text_color(t.warning)
                                            .child(match days {
                                                0 => "today".to_string(),
                                                1 => "1 day".to_string(),
                                                n => format!("{n} days"),
                                            }),
                                    )
                                })
                        }
                        _ => cell.text_color(t.text).child(div().truncate().child(shown)),
                    }
                }))
        });

        // The selection: a translucent range and a ring on the active cell,
        // both gliding between cells.
        let glide = |name: &str, v: f32| motion::follow(&format!("{key}-sel-{name}"), v, CURSOR);
        let range_box = (focused && (r0 != r1 || c0 != c1)).then(|| {
            let x = glide("rx", offsets[c0]);
            let y = glide("ry", r0 as f32 * ROW_H);
            let w = glide("rw", offsets[c1] + columns[c1].width - offsets[c0]);
            let h = glide("rh", (r1 - r0 + 1) as f32 * ROW_H);
            div()
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(w))
                .h(px(h))
                .bg(t.accent.opacity(if t.dark { 0.12 } else { 0.07 }))
                .border_1()
                .border_color(t.accent.opacity(0.5))
        });
        let (cr, cc) = sheet.cursor;
        let ring = (!sheet.rows.is_empty()).then(|| {
            let x = glide("cx", offsets[cc]);
            let y = glide("cy", cr as f32 * ROW_H);
            let w = glide("cw", columns[cc].width);
            div()
                .absolute()
                .left(px(x - 1.))
                .top(px(y - 1.))
                .w(px(w + 1.))
                .h(px(ROW_H + 1.))
                .rounded(px(2.))
                .border_2()
                .border_color(if focused { t.accent } else { t.border_strong })
        });

        let body = {
            let up_state = state.clone();
            let out_state = state.clone();
            div()
                .id(SharedString::from(format!("{key}-body")))
                .relative()
                .w(px(width))
                .h(px(ROW_H * sheet.rows.len() as f32))
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

        // --- Status bar -----------------------------------------------------
        let cell_ref = |r: usize, c: usize| format!("{}{}", column_letter(c), r + 1);
        let summary: SharedString = if sheet.rows.is_empty() {
            "Empty".into()
        } else if r0 == r1 && c0 == c1 {
            format!("{} · {}", cell_ref(r0, c0), columns[c0].name).into()
        } else {
            let cells = (r1 - r0 + 1) * (c1 - c0 + 1);
            let mut text = format!(
                "{}:{} · {} cells",
                cell_ref(r0, c0),
                cell_ref(r1, c1),
                cells
            );
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
        let mac = cfg!(target_os = "macos");
        let mod_key = if mac { "⌘" } else { "Ctrl+" };
        let hint = |keys: String, label: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(space::S4))
                .child(luna::kbd(keys, &t))
                .child(label)
        };
        let add_state = state.clone();
        let add_focus = focus.clone();
        let status = div()
            .h(px(40.))
            .px(px(space::S8))
            .flex()
            .items_center()
            .gap(px(space::S12))
            .text_size(px(text::FOOTNOTE))
            .text_color(t.text_faint)
            .child(
                luna::button(
                    SharedString::from(format!("{key}-add")),
                    luna::ButtonKind::Ghost,
                    ControlSize::Sm,
                    &t,
                )
                .child(luna::glyph(ICON_PLUS, ControlSize::Sm).text_color(t.text_muted))
                .child("Add row")
                .on_click(move |_, window, cx| {
                    window.focus(&add_focus);
                    add_state.update(cx, |s, cx| {
                        if s.commit(cx) {
                            let r = s.sheet.add_row();
                            s.sheet.select((r, 0), false);
                            s.changed();
                            s.start_edit(None, window, cx);
                        }
                        cx.notify();
                    })
                }),
            )
            .child(div().w(px(1.)).h(px(16.)).bg(t.border))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(luna::MONO)
                    .text_size(px(text::CAPTION))
                    .child(match &flash {
                        Some((message, error)) => motion::fade_in(
                            SharedString::from(format!("{key}-flash-{message}")),
                            div()
                                .text_color(if *error { t.danger } else { t.success })
                                .child(message.clone()),
                        )
                        .into_any_element(),
                        None => div()
                            .text_color(t.text_muted)
                            .child(summary)
                            .into_any_element(),
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S12))
                    .text_size(px(text::CAPTION))
                    .child(hint("↵".into(), "edit"))
                    .child(hint(format!("{mod_key}C"), "copy"))
                    .child(hint(format!("{mod_key}Z"), "undo")),
            );

        // --- Keyboard -------------------------------------------------------
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

        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .w(px(width + 2.))
            .overflow_hidden()
            .font_family(luna::SANS)
            .track_focus(&focus)
            .on_key_down(on_key)
            .child(toolbar)
            .child(header)
            .child(body)
            .child(status)
    }
}

fn demo() -> DataTable {
    let accounts = [
        [
            "Halcyon Robotics",
            "Enterprise",
            "240",
            "18400",
            "2027-02-14",
            "Maya Okafor",
        ],
        [
            "Brightline Health",
            "Business",
            "86",
            "5160",
            "2026-11-03",
            "Theo Lindqvist",
        ],
        [
            "Parcelwise",
            "Team",
            "24",
            "1150",
            "2026-12-19",
            "Maya Okafor",
        ],
        [
            "Northwind Labs",
            "Business",
            "112",
            "6720",
            "2027-01-08",
            "Ana Ruiz",
        ],
        [
            "Quillstack",
            "Starter",
            "6",
            "90",
            "2026-10-30",
            "Theo Lindqvist",
        ],
        [
            "Meridian Freight",
            "Enterprise",
            "410",
            "29520",
            "2027-06-30",
            "Ana Ruiz",
        ],
        [
            "Copperleaf Studio",
            "Team",
            "18",
            "860",
            "2026-11-21",
            "Jun Park",
        ],
        [
            "Tidepool Analytics",
            "Business",
            "64",
            "3840",
            "2027-03-02",
            "Jun Park",
        ],
    ];
    accounts.iter().fold(
        DataTable::new("data-table", "Accounts")
            .today("2026-10-21")
            .column(TableColumn::new("Account", ColumnKind::Text, 188.))
            .column(
                TableColumn::new("Plan", ColumnKind::Tag, 112.)
                    .tone("Enterprise", Tone::Accent)
                    .tone("Business", Tone::Success)
                    .tone("Team", Tone::Warning),
            )
            .column(TableColumn::new("Seats", ColumnKind::Number, 76.))
            .column(TableColumn::new("MRR", ColumnKind::Currency, 100.))
            .column(TableColumn::new("Renews", ColumnKind::Date, 152.))
            .column(TableColumn::new("Owner", ColumnKind::Text, 132.)),
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
    fn day_numbers() {
        assert_eq!(day_number((1970, 1, 1)), 0);
        assert_eq!(day_number((2026, 10, 30)) - day_number((2026, 10, 21)), 9);
    }
}
