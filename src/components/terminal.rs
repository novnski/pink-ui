//! Terminal – a terminal drawer with session tabs, as Pink draws it.
//!
//! Pink: `terminal/panel.rs` `TerminalPanel` (the tab strip, the Commands
//! shelf from `project_commands.rs` and the terminal body), docked at the
//! bottom of the main column by `shell.rs` `render_terminal_container`, and
//! `terminal/view.rs` `TerminalElement`, which paints the grid. Since #176
//! the drawer is the terminal's only home (the right panel has no Terminal
//! place) and it is held to 40% of the window at render time (D52), keeping
//! the saved height. The drawer
//! has a 1 px top edge with a 10 px resize grip floating over it (drag to
//! resize, double-click for the default height) and slides open and shut
//! over `RESIZE`. The strip holds fixed 118 px tabs – a terminal glyph, the
//! session's title (the shell's OSC title when it set one) and a close
//! button on the active tab – that reorder by dragging and close on a
//! middle click, then "+" and the collapse chevron. The strip draws
//! through the `pink-terminal-tabs` package, as Pink's does.
//!
//! The grid is Geist Mono 13/18 inside a 12 px inset on the terminal plate,
//! in Pink's sixteen ANSI colors plus the xterm cube and grey ramp, palette
//! text floored to 4.6:1 the way Pink floors it. A translucent block marks
//! the cursor while the terminal has focus and an outline when it doesn't.
//! Drag selects (double-click a word, triple-click a line) under Pink's
//! selection wash and Cmd+C / Ctrl+Shift+C copies; the wheel scrolls back
//! through history and a thin rail appears while the view moves.
//!
//! There is no PTY: each tab's `output` is the byte stream a PTY would have
//! written (text with ANSI escapes, appended to over time) and keystrokes
//! come back as PTY bytes through `on_input`, so a host can drive a real
//! session or a scripted one.
//!
//! ```ignore
//! Terminal::new("terminal")
//!     .tab(TerminalTab::new("api", "zsh").output("\x1b[32m❯\x1b[0m "))
//!     .on_input(|tab, bytes, _, _| pty_write(tab, bytes))
//! ```

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnimationExt, AnyView, App, BorderStyle, Bounds, ClipboardItem, Context, ElementId, Entity,
    FocusHandle, Font, FontWeight, Hsla, IntoElement, KeyDownEvent, Modifiers, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Render, ScrollDelta,
    ScrollWheelEvent, ShapedLine, SharedString, Task, TextRun, Window, canvas, div, fill, font,
    outline, point, prelude::*, px, size,
};

use super::scroll_rail;
use super::terminal_tabs as tabs;
use crate::{
    motion::{self, RESIZE, TAB_SLIDE},
    pink::{self, ButtonKind, ControlSize, Pink, TextStyle, TypeStyleExt, chrome, layout},
    story,
};

pub const NAME: &str = "Terminal";
pub const DESCRIPTION: &str = "A terminal drawer as Pink draws it: session tabs you can reorder, a monospace grid in Pink's sixteen ANSI colors with a translucent block cursor, drag, word and line selection, scrollback behind a quiet rail, and a Commands shelf for the project's saved tasks";

const ICON_TERMINAL: &str = "icons/terminal/terminal.svg";
const ICON_PLUS: &str = "icons/terminal/plus.svg";
const ICON_CLOSE: &str = "icons/terminal/close.svg";
const ICON_DOWN: &str = "icons/terminal/alt-arrow-down.svg";
const ICON_RIGHT: &str = "icons/terminal/alt-arrow-right.svg";
const ICON_FOLDER: &str = "icons/terminal/folder.svg";
const ICON_PLAY: &str = "icons/terminal/play.svg";

// Pink: terminal/panel.rs; the strip is the `pink-terminal-tabs` package.
/// Fixed tab width, so drag-reorder math stays analytic.
const TAB_WIDTH: f32 = tabs::TAB_WIDTH;
// The strip is the shared header band height (`surface_chrome::HEADER_HEIGHT`).
const _: () = assert!(tabs::BAR_HEIGHT == layout::TITLEBAR_HEIGHT);
const SELECTION_SCROLL_TICK: Duration = Duration::from_millis(24);

// Pink: terminal/view.rs.
const TERM_FONT_SIZE: f32 = 13.0;
const TERM_LINE_HEIGHT: f32 = 18.0;
const TERM_PADDING: f32 = 12.0;
/// Pointer travel before a press becomes a selection.
const SELECTION_DRAG_THRESHOLD: f32 = 2.0;
/// Characters that end a double-click word (alacritty's default).
const SEMANTIC_SEPARATORS: &str = ",│`|:\"' ()[]{}<>\t";

// Pink: shell.rs drawer and settings.rs heights.
const RESIZE_HITBOX_HEIGHT: f32 = 10.0;
const MIN_HEIGHT: f32 = 160.0;
const DEFAULT_HEIGHT: f32 = 280.0;
/// The share of the window the drawer may take (Pink D52,
/// `TERMINAL_MAX_VH`): 40%, applied at render time, so a saved or dragged
/// height is never rewritten.
const DRAWER_MAX_SHARE: f32 = 0.40;
/// The demo's stand-in window: the conversation column the drawer docks in.
const WINDOW_HEIGHT: f32 = 720.0;

/// The tallest the drawer may be in a window `window_height` tall (Pink
/// `clamp_terminal_height`): 40% of it, never under `MIN_HEIGHT`.
fn drawer_max_height(window_height: f32) -> f32 {
    (window_height * DRAWER_MAX_SHARE).max(MIN_HEIGHT)
}

// Pink: popover.rs floating scrollbar rail.
const RAIL_TRACK_INSET: f32 = scroll_rail::TRACK_INSET;
const RAIL_MIN_THUMB: f32 = scroll_rail::MIN_THUMB;
const RAIL_LINGER: Duration = Duration::from_millis(1_400);
const RAIL_FADE: Duration = Duration::from_millis(260);

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// One terminal session.
#[derive(Clone, Debug)]
pub struct TerminalTab {
    pub id: SharedString,
    /// The fallback label; a title the shell sets (OSC 0 / 2) wins.
    pub title: SharedString,
    /// Everything the session has written, ANSI escapes included. Append to
    /// it; a stream that is not an extension of the last one starts over.
    pub output: SharedString,
    /// The process ended with this code: the tab dims and takes no input.
    pub exited: Option<i32>,
}

impl TerminalTab {
    pub fn new(id: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            output: SharedString::default(),
            exited: None,
        }
    }

    pub fn output(mut self, output: impl Into<SharedString>) -> Self {
        self.output = output.into();
        self
    }

    pub fn exited(mut self, code: i32) -> Self {
        self.exited = Some(code);
        self
    }
}

/// A saved project command on the Commands shelf.
#[derive(Clone, Debug)]
pub struct TerminalCommand {
    pub id: SharedString,
    pub name: SharedString,
    /// It comes from the repository's own config (Pink badges it).
    pub repository: bool,
}

impl TerminalCommand {
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            repository: false,
        }
    }

    pub fn repository(mut self, repository: bool) -> Self {
        self.repository = repository;
        self
    }
}

type InputHandler = Rc<dyn Fn(&SharedString, &[u8], &mut Window, &mut App)>;
type IndexHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type ReorderHandler = Rc<dyn Fn(usize, usize, &mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type CommandHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type ResizeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// The terminal drawer.
#[derive(IntoElement)]
pub struct Terminal {
    id: ElementId,
    tabs: Vec<TerminalTab>,
    active: usize,
    width: f32,
    height: f32,
    max_height: f32,
    open: bool,
    autofocus: bool,
    cwd: Option<SharedString>,
    commands: Vec<TerminalCommand>,
    on_input: Option<InputHandler>,
    on_select: Option<IndexHandler>,
    on_close: Option<IndexHandler>,
    on_new_tab: Option<ActionHandler>,
    on_reorder: Option<ReorderHandler>,
    on_collapse: Option<ActionHandler>,
    on_run_command: Option<CommandHandler>,
    on_resize: Option<ResizeHandler>,
}

impl Terminal {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            active: 0,
            width: 760.0,
            height: DEFAULT_HEIGHT,
            max_height: 520.0,
            open: true,
            autofocus: false,
            cwd: None,
            commands: Vec::new(),
            on_input: None,
            on_select: None,
            on_close: None,
            on_new_tab: None,
            on_reorder: None,
            on_collapse: None,
            on_run_command: None,
            on_resize: None,
        }
    }

    pub fn tab(mut self, tab: TerminalTab) -> Self {
        self.tabs.push(tab);
        self
    }

    pub fn tabs(mut self, tabs: Vec<TerminalTab>) -> Self {
        self.tabs = tabs;
        self
    }

    /// The tab on show.
    pub fn active(mut self, ix: usize) -> Self {
        self.active = ix;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// The drawer's starting height; dragging its top edge changes it.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The tallest the drawer may be dragged.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    /// Holds the drawer to 40% of a window `height` tall (Pink D52), never
    /// under its minimum; sets [`Self::max_height`].
    pub fn window_height(mut self, height: f32) -> Self {
        self.max_height = drawer_max_height(height);
        self
    }

    /// Slides the drawer shut (`false`) or open over Pink's `RESIZE`.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// Focus the terminal when it first shows.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// The project directory the Commands shelf names.
    pub fn cwd(mut self, cwd: impl Into<SharedString>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn commands(mut self, commands: Vec<TerminalCommand>) -> Self {
        self.commands = commands;
        self
    }

    /// Keystrokes and pastes for a tab, encoded as PTY bytes.
    pub fn on_input(
        mut self,
        f: impl Fn(&SharedString, &[u8], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_input = Some(Rc::new(f));
        self
    }

    pub fn on_select(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    pub fn on_close(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    pub fn on_new_tab(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_new_tab = Some(Rc::new(f));
        self
    }

    /// A tab was dragged from one slot to another.
    pub fn on_reorder(mut self, f: impl Fn(usize, usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_reorder = Some(Rc::new(f));
        self
    }

    /// The collapse chevron (Pink: "Hide terminal", ⌘J).
    pub fn on_collapse(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_collapse = Some(Rc::new(f));
        self
    }

    pub fn on_run_command(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run_command = Some(Rc::new(f));
        self
    }

    /// The drawer was resized to this height.
    pub fn on_resize(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_resize = Some(Rc::new(f));
        self
    }
}

// ---------------------------------------------------------------------------
// Emulator: the slice of a VT a scripted session needs
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CellColor {
    Foreground,
    Background,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pen {
    fg: CellColor,
    bg: CellColor,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
    hidden: bool,
}

impl Default for Pen {
    fn default() -> Self {
        Self {
            fg: CellColor::Foreground,
            bg: CellColor::Background,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TermCell {
    ch: char,
    pen: Pen,
}

impl TermCell {
    fn blank() -> Self {
        Self {
            ch: ' ',
            pen: Pen::default(),
        }
    }

    /// Colors after inverse and conceal (Pink `CellSnapshot::display_colors`).
    fn display_colors(&self) -> (CellColor, CellColor) {
        let (fg, bg) = if self.pen.inverse {
            (self.pen.bg, self.pen.fg)
        } else {
            (self.pen.fg, self.pen.bg)
        };
        if self.pen.hidden { (bg, bg) } else { (fg, bg) }
    }
}

#[derive(Clone, Debug, Default)]
enum Parse {
    #[default]
    Ground,
    Escape,
    Csi(String),
    Osc(String),
    OscEscape(String),
}

/// Logical lines (wrapped to the grid at paint), the cursor, the pen, the
/// OSC title. Handles printable text, CR / LF / BS / TAB, SGR (16, 256 and
/// true color), erase in line / display, horizontal cursor moves, cursor
/// visibility and OSC 0 / 2 titles; everything else is ignored.
#[derive(Clone, Debug)]
struct Emulator {
    lines: Vec<Vec<TermCell>>,
    line: usize,
    col: usize,
    pen: Pen,
    title: Option<String>,
    cursor_hidden: bool,
    parse: Parse,
}

impl Emulator {
    fn new() -> Self {
        Self {
            lines: vec![Vec::new()],
            line: 0,
            col: 0,
            pen: Pen::default(),
            title: None,
            cursor_hidden: false,
            parse: Parse::Ground,
        }
    }

    fn feed(&mut self, text: &str) {
        for ch in text.chars() {
            match std::mem::take(&mut self.parse) {
                Parse::Ground => self.ground(ch),
                Parse::Escape => {
                    self.parse = match ch {
                        '[' => Parse::Csi(String::new()),
                        ']' => Parse::Osc(String::new()),
                        'c' => {
                            *self = Self::new();
                            Parse::Ground
                        }
                        _ => Parse::Ground,
                    }
                }
                Parse::Csi(mut params) => {
                    if ('\x40'..='\x7e').contains(&ch) {
                        self.csi(&params, ch);
                    } else {
                        params.push(ch);
                        self.parse = Parse::Csi(params);
                    }
                }
                Parse::Osc(mut body) => match ch {
                    '\x07' => self.osc(&body),
                    '\x1b' => self.parse = Parse::OscEscape(body),
                    _ => {
                        body.push(ch);
                        self.parse = Parse::Osc(body);
                    }
                },
                Parse::OscEscape(body) => self.osc(&body),
            }
        }
    }

    fn ground(&mut self, ch: char) {
        match ch {
            '\x1b' => self.parse = Parse::Escape,
            '\r' => self.col = 0,
            // Output arrives with ONLCR semantics: a line feed also returns.
            '\n' => {
                self.line += 1;
                self.col = 0;
                while self.lines.len() <= self.line {
                    self.lines.push(Vec::new());
                }
            }
            '\x08' => self.col = self.col.saturating_sub(1),
            '\t' => self.col = (self.col / 8 + 1) * 8,
            c if c.is_control() => {}
            c => self.put(c),
        }
    }

    fn put(&mut self, ch: char) {
        let row = &mut self.lines[self.line];
        while row.len() < self.col {
            row.push(TermCell::blank());
        }
        let cell = TermCell { ch, pen: self.pen };
        if self.col < row.len() {
            row[self.col] = cell;
        } else {
            row.push(cell);
        }
        self.col += 1;
    }

    fn csi(&mut self, params: &str, action: char) {
        let private = params.starts_with('?');
        let numbers: Vec<u16> = params
            .trim_start_matches('?')
            .split(';')
            .map(|p| p.split(':').next().unwrap_or("").parse().unwrap_or(0))
            .collect();
        let first = numbers.first().copied().unwrap_or(0);
        let count = first.max(1) as usize;
        match action {
            'm' => self.sgr(&numbers),
            'K' => {
                let row = &mut self.lines[self.line];
                match first {
                    0 => row.truncate(self.col),
                    1 => {
                        for cell in row.iter_mut().take(self.col + 1) {
                            *cell = TermCell::blank();
                        }
                    }
                    _ => row.clear(),
                }
            }
            'J' => match first {
                0 => {
                    self.lines.truncate(self.line + 1);
                    self.lines[self.line].truncate(self.col);
                }
                _ => {
                    self.lines = vec![Vec::new()];
                    self.line = 0;
                    self.col = 0;
                }
            },
            'C' => self.col += count,
            'D' => self.col = self.col.saturating_sub(count),
            'G' => self.col = count - 1,
            'h' | 'l' if private && numbers.contains(&25) => {
                self.cursor_hidden = action == 'l';
            }
            _ => {}
        }
    }

    fn sgr(&mut self, params: &[u16]) {
        let mut i = 0;
        while i < params.len() {
            let p = params[i];
            match p {
                0 => self.pen = Pen::default(),
                1 => self.pen.bold = true,
                2 => self.pen.dim = true,
                3 => self.pen.italic = true,
                4 => self.pen.underline = true,
                7 => self.pen.inverse = true,
                8 => self.pen.hidden = true,
                22 => {
                    self.pen.bold = false;
                    self.pen.dim = false;
                }
                23 => self.pen.italic = false,
                24 => self.pen.underline = false,
                27 => self.pen.inverse = false,
                28 => self.pen.hidden = false,
                30..=37 => self.pen.fg = CellColor::Indexed((p - 30) as u8),
                39 => self.pen.fg = CellColor::Foreground,
                40..=47 => self.pen.bg = CellColor::Indexed((p - 40) as u8),
                49 => self.pen.bg = CellColor::Background,
                90..=97 => self.pen.fg = CellColor::Indexed((p - 90 + 8) as u8),
                100..=107 => self.pen.bg = CellColor::Indexed((p - 100 + 8) as u8),
                38 | 48 => {
                    let color = match params.get(i + 1) {
                        Some(5) => {
                            let ix = params.get(i + 2).copied().unwrap_or(0).min(255) as u8;
                            i += 2;
                            Some(CellColor::Indexed(ix))
                        }
                        Some(2) => {
                            let c =
                                |k: usize| params.get(i + k).copied().unwrap_or(0).min(255) as u8;
                            let color = CellColor::Rgb(c(2), c(3), c(4));
                            i += 4;
                            Some(color)
                        }
                        _ => None,
                    };
                    if let Some(color) = color {
                        if p == 38 {
                            self.pen.fg = color;
                        } else {
                            self.pen.bg = color;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn osc(&mut self, body: &str) {
        if let Some((kind, title)) = body.split_once(';')
            && (kind == "0" || kind == "2")
        {
            self.title = Some(title.to_string());
        }
    }
}

/// One painted row: a slice of a logical line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RowSpan {
    line: usize,
    start: usize,
    end: usize,
    /// It continues the row above (a soft wrap, no newline when copied).
    continued: bool,
}

/// Wraps the logical lines to `cols`, with the cursor's row and column.
fn wrap(emu: &Emulator, cols: usize) -> (Vec<RowSpan>, (usize, usize)) {
    let cols = cols.max(1);
    let mut rows = Vec::new();
    let mut cursor = (0, 0);
    for (ix, line) in emu.lines.iter().enumerate() {
        let n = line.len();
        let mut count = n.div_ceil(cols).max(1);
        let first = rows.len();
        if ix == emu.line {
            let c = emu.col;
            // A cursor just past a full row waits on its last column
            // (pending wrap), as a terminal shows it.
            if c > 0 && c == n && c % cols == 0 {
                cursor = (first + c / cols - 1, cols - 1);
            } else {
                count = count.max(c / cols + 1);
                cursor = (first + c / cols, c % cols);
            }
        }
        for k in 0..count {
            rows.push(RowSpan {
                line: ix,
                start: (k * cols).min(n),
                end: ((k + 1) * cols).min(n),
                continued: k > 0,
            });
        }
    }
    (rows, cursor)
}

// ---------------------------------------------------------------------------
// Colors (Pink: terminal/view.rs)
// ---------------------------------------------------------------------------

const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// The xterm cube as asked; the grey ramp mirrored on light so its faint end
/// stays faint (Pink `extended_indexed_rgb`).
fn extended_indexed_rgb(dark: bool, index: u8) -> (u8, u8, u8) {
    match index {
        16..=231 => {
            let n = index as usize - 16;
            (
                CUBE_LEVELS[n / 36],
                CUBE_LEVELS[(n / 6) % 6],
                CUBE_LEVELS[n % 6],
            )
        }
        232..=255 => {
            let step = index - 232;
            let step = if dark { step } else { 23 - step };
            let v = 8 + 10 * step;
            (v, v, v)
        }
        _ => (0, 0, 0),
    }
}

fn rgb8(r: u8, g: u8, b: u8) -> Hsla {
    gpui::Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
    .into()
}

/// The palette a grid paints with.
#[derive(Clone)]
struct Palette {
    fg: Hsla,
    bg: Hsla,
    ansi: [Hsla; 16],
    selection: Hsla,
    cursor: Hsla,
    /// The opaque color a default-background cell shows (Pink
    /// `terminal_surface`).
    surface: Hsla,
    dark: bool,
}

impl Palette {
    fn new(t: &Pink) -> Self {
        Self {
            fg: t.terminal_fg,
            bg: t.terminal_bg,
            ansi: t.ansi(),
            selection: t.terminal_selection,
            cursor: t.cursor,
            surface: pink::flatten(t.terminal_panel_fill(), t.chrome_surface()),
            dark: t.dark,
        }
    }

    fn resolve(&self, color: CellColor) -> Hsla {
        match color {
            CellColor::Foreground => self.fg,
            CellColor::Background => self.bg,
            CellColor::Indexed(ix @ 0..=15) => self.ansi[ix as usize],
            CellColor::Indexed(ix) => {
                let (r, g, b) = extended_indexed_rgb(self.dark, ix);
                rgb8(r, g, b)
            }
            CellColor::Rgb(r, g, b) => rgb8(r, g, b),
        }
    }
}

fn palette_controlled(color: CellColor) -> bool {
    matches!(
        color,
        CellColor::Foreground | CellColor::Background | CellColor::Indexed(0..=15)
    )
}

/// Box drawing, blocks and braille paint exactly as asked.
fn is_terminal_graphic(ch: char) -> bool {
    matches!(ch, '\u{2500}'..='\u{259F}' | '\u{2800}'..='\u{28FF}')
}

/// The opaque color behind a cell's glyph, selection included.
fn cell_behind(cell: &TermCell, selected: bool, p: &Palette) -> Hsla {
    let (_, bg) = cell.display_colors();
    let base = match bg {
        CellColor::Background => p.surface,
        other => pink::flatten(p.resolve(other), p.surface),
    };
    if selected {
        pink::flatten(p.selection, base)
    } else {
        base
    }
}

/// A cell's painted foreground: dim at 60%, palette pairs floored to 4.6:1
/// toward the terminal foreground; program colors as asked (Pink
/// `terminal_paint_fg`).
fn paint_fg(cell: &TermCell, selected: bool, p: &Palette) -> Hsla {
    let (fg, bg) = cell.display_colors();
    let mut color = p.resolve(fg);
    if cell.pen.hidden {
        return color;
    }
    if cell.pen.dim {
        color.a *= 0.6;
    }
    if is_terminal_graphic(cell.ch) || !palette_controlled(fg) || !palette_controlled(bg) {
        return color;
    }
    floor(color, cell_behind(cell, selected, p), p.fg)
}

/// The glyph redrawn over the focused cursor block (Pink
/// `terminal_cursor_glyph_fg`).
fn cursor_glyph_fg(cell: &TermCell, selected: bool, p: &Palette) -> Hsla {
    let painted = paint_fg(cell, selected, p);
    let (fg, bg) = cell.display_colors();
    if cell.pen.hidden
        || is_terminal_graphic(cell.ch)
        || !palette_controlled(fg)
        || !palette_controlled(bg)
    {
        return painted;
    }
    let under = pink::flatten(p.cursor, cell_behind(cell, selected, p));
    floor(painted, under, p.fg)
}

/// Pink `terminal_floor`, memoized per (color, behind, fallback).
fn floor(color: Hsla, behind: Hsla, fallback: Hsla) -> Hsla {
    type Key = [u32; 12];
    thread_local! {
        static FLOORS: RefCell<HashMap<Key, Hsla>> = RefCell::new(HashMap::new());
    }
    let bits = |c: Hsla| [c.h.to_bits(), c.s.to_bits(), c.l.to_bits(), c.a.to_bits()];
    let mut key = [0u32; 12];
    key[..4].copy_from_slice(&bits(color));
    key[4..8].copy_from_slice(&bits(behind));
    key[8..].copy_from_slice(&bits(fallback));
    if let Some(hit) = FLOORS.with_borrow(|cache| cache.get(&key).copied()) {
        return hit;
    }
    let floored = pink::legible_on(color, fallback, &[behind], pink::TEXT_CONTRAST_TARGET);
    FLOORS.with_borrow_mut(|cache| {
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(key, floored);
    });
    floored
}

// ---------------------------------------------------------------------------
// Pointer → cell, selection
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CellHit {
    row: usize,
    col: usize,
    side: Side,
}

/// A position relative to the first glyph, clamped into the grid; overshoot
/// forces the side so a drag past an edge takes the edge row whole (Pink
/// `cell_at`).
fn cell_at(x: f32, y: f32, cell_w: f32, line_h: f32, cols: usize, rows: usize) -> CellHit {
    let usable = |v: f32| v.is_finite() && v > 0.0;
    if cols == 0 || rows == 0 || !usable(cell_w) || !usable(line_h) {
        return CellHit {
            row: 0,
            col: 0,
            side: Side::Left,
        };
    }
    let x = if x.is_finite() { x } else { 0.0 };
    let y = if y.is_finite() { y } else { 0.0 };
    let raw_col = (x / cell_w).floor();
    let mut side = if x.max(0.0) % cell_w > cell_w / 2.0 {
        Side::Right
    } else {
        Side::Left
    };
    let col = if raw_col > (cols - 1) as f32 {
        side = Side::Right;
        cols - 1
    } else {
        raw_col.max(0.0) as usize
    };
    let raw_row = (y / line_h).floor();
    let row = if raw_row > (rows - 1) as f32 {
        side = Side::Right;
        rows - 1
    } else if raw_row < 0.0 {
        side = Side::Left;
        0
    } else {
        raw_row as usize
    };
    CellHit { row, col, side }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionType {
    Simple,
    Semantic,
    Lines,
}

/// A point on the wrapped grid: an absolute row (history included).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridPoint {
    row: usize,
    col: usize,
    side: Side,
}

#[derive(Clone, Debug, PartialEq)]
struct Selection {
    tab: SharedString,
    /// The grid width it was made at; a reflow drops it.
    cols: usize,
    ty: SelectionType,
    anchor: GridPoint,
    head: GridPoint,
}

fn cell_char(emu: &Emulator, span: &RowSpan, col: usize) -> char {
    emu.lines
        .get(span.line)
        .and_then(|line| line.get(span.start + col))
        .map(|cell| cell.ch)
        .unwrap_or(' ')
}

/// The selected cells as `[start, end)` in row-major cell indices.
fn selection_range(sel: &Selection, emu: &Emulator, rows: &[RowSpan]) -> Option<(usize, usize)> {
    let cols = sel.cols.max(1);
    let (first, last) = {
        let key = |p: &GridPoint| (p.row, p.col, p.side == Side::Right);
        if key(&sel.anchor) <= key(&sel.head) {
            (sel.anchor, sel.head)
        } else {
            (sel.head, sel.anchor)
        }
    };
    match sel.ty {
        SelectionType::Simple => {
            let start = first.row * cols + first.col + usize::from(first.side == Side::Right);
            let end = last.row * cols + last.col + usize::from(last.side == Side::Right);
            (end > start).then_some((start, end))
        }
        SelectionType::Semantic => {
            let is_word = |row: usize, col: usize| {
                rows.get(row)
                    .map(|span| {
                        let ch = cell_char(emu, span, col);
                        ch != ' ' && !SEMANTIC_SEPARATORS.contains(ch)
                    })
                    .unwrap_or(false)
            };
            let mut start = first.col;
            if is_word(first.row, start) {
                while start > 0 && is_word(first.row, start - 1) {
                    start -= 1;
                }
            }
            let mut end = last.col;
            if is_word(last.row, end) {
                while end + 1 < cols && is_word(last.row, end + 1) {
                    end += 1;
                }
            }
            Some((first.row * cols + start, last.row * cols + end + 1))
        }
        SelectionType::Lines => Some((first.row * cols, (last.row + 1) * cols)),
    }
}

/// The selection as text: soft-wrapped rows join, logical lines break, and
/// each line's trailing blanks go.
fn selection_text(sel: &Selection, emu: &Emulator) -> Option<String> {
    let (rows, _) = wrap(emu, sel.cols);
    let (start, end) = selection_range(sel, emu, &rows)?;
    let cols = sel.cols.max(1);
    let mut out = String::new();
    let mut line = String::new();
    for (r, span) in rows.iter().enumerate() {
        let row_start = r * cols;
        let row_end = row_start + cols;
        if row_end <= start || row_start >= end {
            continue;
        }
        if !span.continued && (r * cols) > start {
            out.push_str(line.trim_end());
            out.push('\n');
            line.clear();
        }
        for col in 0..cols {
            let ix = row_start + col;
            if ix >= start && ix < end {
                line.push(cell_char(emu, span, col));
            }
        }
    }
    out.push_str(line.trim_end());
    (!out.is_empty()).then_some(out)
}

// ---------------------------------------------------------------------------
// Keyboard → bytes (Pink: terminal/view.rs `keystroke_bytes`)
// ---------------------------------------------------------------------------

fn keystroke_bytes(key: &str, key_char: Option<&str>, mods: &Modifiers) -> Option<Vec<u8>> {
    if mods.platform {
        return None;
    }
    if mods.alt {
        let inner = keystroke_bytes(
            key,
            key_char,
            &Modifiers {
                alt: false,
                ..*mods
            },
        )?;
        let mut out = vec![0x1b];
        out.extend(inner);
        return Some(out);
    }
    if mods.control {
        return control_bytes(key);
    }
    match key {
        "enter" => Some(b"\r".to_vec()),
        "backspace" => Some(vec![0x7f]),
        "tab" => Some(if mods.shift {
            b"\x1b[Z".to_vec()
        } else {
            b"\t".to_vec()
        }),
        "escape" => Some(vec![0x1b]),
        "space" => Some(b" ".to_vec()),
        "up" => Some(b"\x1b[A".to_vec()),
        "down" => Some(b"\x1b[B".to_vec()),
        "right" => Some(b"\x1b[C".to_vec()),
        "left" => Some(b"\x1b[D".to_vec()),
        "home" => Some(b"\x1b[H".to_vec()),
        "end" => Some(b"\x1b[F".to_vec()),
        "delete" => Some(b"\x1b[3~".to_vec()),
        "pageup" => Some(b"\x1b[5~".to_vec()),
        "pagedown" => Some(b"\x1b[6~".to_vec()),
        _ => {
            let text = key_char
                .filter(|c| !c.is_empty())
                .or((key.chars().count() == 1).then_some(key))?;
            Some(text.as_bytes().to_vec())
        }
    }
}

/// Ctrl-key caret encoding.
fn control_bytes(key: &str) -> Option<Vec<u8>> {
    let mut chars = key.chars();
    let (c, rest) = (chars.next()?, chars.next());
    if rest.is_some() {
        return match key {
            "space" => Some(vec![0x00]),
            "backspace" => Some(vec![0x08]),
            "enter" => Some(b"\r".to_vec()),
            _ => None,
        };
    }
    match c {
        'a'..='z' => Some(vec![c as u8 - b'a' + 1]),
        '@' => Some(vec![0x00]),
        '[' => Some(vec![0x1b]),
        '\\' => Some(vec![0x1c]),
        ']' => Some(vec![0x1d]),
        '^' => Some(vec![0x1e]),
        '_' | '/' => Some(vec![0x1f]),
        '?' => Some(vec![0x7f]),
        _ => None,
    }
}

/// The dimmed trailer an exited session ends with (Pink `exit_message`).
fn exit_message(code: i32) -> String {
    format!("\r\n\x1b[90m[process exited {code}]\x1b[0m\r\n")
}

/// Pink `slide_offset`: tabs between the dragged one and its target shift a
/// slot toward the gap.
fn slide_offset(ix: usize, from: usize, over: usize) -> f32 {
    if from < over && ix > from && ix <= over {
        -1.0
    } else if over < from && ix >= over && ix < from {
        1.0
    } else {
        0.0
    }
}

/// Pink `drop_index`.
fn drop_index(rel_x: f32, tab_w: f32, count: usize) -> usize {
    if count == 0 || tab_w <= 0.0 {
        return 0;
    }
    ((rel_x / tab_w).floor().max(0.0) as usize).min(count - 1)
}

/// Pink `selection_scroll_lines`: lines to scroll while a selection drag
/// sits in an edge band (positive scrolls into history).
fn selection_scroll_lines(geometry: &Geometry, y: f32) -> i32 {
    let grid_height = geometry.line_h * geometry.rows as f32;
    if grid_height <= 0.0 {
        return 0;
    }
    let edge = geometry.line_h.min(grid_height / 3.0);
    let top = f32::from(geometry.origin.y);
    let bottom = top + grid_height;
    let speed = |penetration: f32| {
        let t = (penetration / edge).clamp(0.0, 1.0);
        (1.0 + 2.0 * t * t).round() as i32
    };
    if y < top + edge {
        speed(top + edge - y)
    } else if y > bottom - edge {
        -speed(y - (bottom - edge))
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Where the grid landed last frame, for pointer mapping and the rail.
#[derive(Clone, Copy, Debug)]
struct Geometry {
    bounds: Bounds<Pixels>,
    origin: Point<Pixels>,
    cell_w: f32,
    line_h: f32,
    cols: usize,
    rows: usize,
    total: usize,
    top: usize,
}

/// One tab's emulator and scroll position.
struct Session {
    emu: Rc<RefCell<Emulator>>,
    consumed: SharedString,
    exit_fed: bool,
    /// Lines scrolled back from the live bottom.
    offset: usize,
    /// Wrapped row count last frame, so new output keeps a scrolled-back
    /// view anchored.
    last_total: usize,
}

impl Session {
    fn new() -> Self {
        Self {
            emu: Rc::new(RefCell::new(Emulator::new())),
            consumed: SharedString::default(),
            exit_fed: false,
            offset: 0,
            last_total: 0,
        }
    }

    /// Feeds whatever the stream added since last frame.
    fn sync(&mut self, output: &SharedString, exited: Option<i32>) {
        if *output != self.consumed {
            let appended = output.starts_with(self.consumed.as_ref()) && !self.exit_fed;
            if !appended {
                let offset = self.offset;
                *self = Self::new();
                self.offset = offset;
            }
            let fresh = &output[self.consumed.len()..];
            self.emu.borrow_mut().feed(fresh);
            self.consumed = output.clone();
        }
        if let Some(code) = exited
            && !self.exit_fed
        {
            self.emu.borrow_mut().feed(&exit_message(code));
            self.exit_fed = true;
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SelectionDrag {
    origin: Point<Pixels>,
    position: Point<Pixels>,
    armed: bool,
}

#[derive(Clone, Copy, Debug)]
struct TabDragState {
    from: usize,
    over: usize,
    epoch: usize,
    prev_over: usize,
}

/// The dragged tab (Pink `TabDragPayload`).
#[derive(Clone)]
struct TabDrag {
    from: usize,
    title: SharedString,
}

/// The tab chip that follows the pointer (Pink `TabGhost`, drawn by
/// `pink-terminal-tabs`).
struct TabGhost {
    title: SharedString,
}

impl Render for TabGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        tabs::ghost(
            &tabs::GhostStyle {
                fill: t.surface_raised,
                edge: t.border_strong,
                text: t.text,
            },
            self.title.clone(),
        )
        .font_family(pink::SANS)
    }
}

struct DrawerResize;
struct RailDrag;

struct EmptyGhost;

impl Render for EmptyGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

/// The floating rail's hover, grab and fade clock (Pink
/// `MenuScrollbarState`).
#[derive(Default)]
struct Rail {
    list_hovered: bool,
    bar_hovered: bool,
    grab: Option<f32>,
    last_from_top: Option<usize>,
    last_scroll_at: Option<Instant>,
}

impl Rail {
    fn note(&mut self, from_top: usize) {
        match self.last_from_top {
            None => self.last_from_top = Some(from_top),
            Some(previous) if previous != from_top => {
                self.last_from_top = Some(from_top);
                self.last_scroll_at = Some(Instant::now());
            }
            _ => {}
        }
    }

    fn active(&self) -> bool {
        self.bar_hovered || self.grab.is_some()
    }

    /// 1 while active or lingering, then fading over 260 ms.
    fn fade(&self) -> f32 {
        if self.active() {
            return 1.0;
        }
        let Some(at) = self.last_scroll_at else {
            return 0.0;
        };
        let elapsed = at.elapsed().as_secs_f32() * 1000.0;
        let linger = RAIL_LINGER.as_millis() as f32;
        let fade = RAIL_FADE.as_millis() as f32;
        (1.0 - (elapsed - linger) / fade).clamp(0.0, 1.0)
    }
}

/// The rail's thumb for a viewport over its content (Pink
/// `MenuScrollbarMetrics::from_parts`).
#[derive(Clone, Copy, Debug)]
struct RailMetrics {
    thumb_top: f32,
    thumb_height: f32,
    travel: f32,
    max_scroll: f32,
}

fn rail_metrics(viewport: f32, content: f32, offset: f32) -> Option<RailMetrics> {
    let max_scroll = (content - viewport).max(0.0);
    if viewport <= 0.0 || max_scroll <= 0.0 {
        return None;
    }
    let track = (viewport - RAIL_TRACK_INSET * 2.0).max(0.0);
    if track <= 0.0 {
        return None;
    }
    let thumb_height = (track * viewport / content).max(RAIL_MIN_THUMB).min(track);
    let travel = (track - thumb_height).max(0.0);
    Some(RailMetrics {
        thumb_top: travel * offset.clamp(0.0, max_scroll) / max_scroll,
        thumb_height,
        travel,
        max_scroll,
    })
}

struct TermState {
    focus: FocusHandle,
    sessions: HashMap<SharedString, Session>,
    active_tab: Option<SharedString>,
    geometry: Rc<Cell<Option<Geometry>>>,
    selection: Option<Selection>,
    sel_drag: Option<SelectionDrag>,
    sel_task: Option<Task<()>>,
    scroll_remainder: f32,
    rail: Rail,
    tab_drag: Option<TabDragState>,
    commands_open: bool,
    height: f32,
    resize_anchor: Option<(f32, f32)>,
    on_input: Option<InputHandler>,
}

impl TermState {
    fn session(&mut self) -> Option<&mut Session> {
        let id = self.active_tab.clone()?;
        self.sessions.get_mut(&id)
    }

    /// Scrolls the active session by whole lines (positive: into history).
    fn scroll(&mut self, lines: i32) {
        let Some(geometry) = self.geometry.get() else {
            return;
        };
        let max = geometry.total.saturating_sub(geometry.rows);
        if let Some(session) = self.session() {
            let next = (session.offset as i64 + lines as i64).clamp(0, max as i64) as usize;
            session.offset = next;
        }
    }

    /// Pointer → grid point on this frame's placement.
    fn grid_point_at(&self, position: Point<Pixels>) -> Option<GridPoint> {
        let g = self.geometry.get()?;
        let hit = cell_at(
            f32::from(position.x - g.origin.x),
            f32::from(position.y - g.origin.y),
            g.cell_w,
            g.line_h,
            g.cols,
            g.rows,
        );
        Some(GridPoint {
            row: g.top + hit.row,
            col: hit.col,
            side: hit.side,
        })
    }

    /// The selection's text, for Copy.
    fn copied_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        let session = self.sessions.get(&sel.tab)?;
        selection_text(sel, &session.emu.borrow())
    }

    /// Keeps a selection drag scrolling while it rests in an edge band.
    fn schedule_selection_scroll(&mut self, cx: &mut Context<Self>) {
        if self.sel_task.is_some() {
            return;
        }
        let (Some(drag), Some(geometry)) = (self.sel_drag, self.geometry.get()) else {
            return;
        };
        if !drag.armed || selection_scroll_lines(&geometry, f32::from(drag.position.y)) == 0 {
            return;
        }
        self.sel_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SELECTION_SCROLL_TICK).await;
            this.update(cx, |s, cx| {
                s.sel_task = None;
                s.step_selection_scroll(cx);
            })
            .ok();
        }));
    }

    fn step_selection_scroll(&mut self, cx: &mut Context<Self>) {
        let (Some(drag), Some(geometry)) = (self.sel_drag, self.geometry.get()) else {
            return;
        };
        if !drag.armed {
            return;
        }
        let lines = selection_scroll_lines(&geometry, f32::from(drag.position.y));
        if lines == 0 {
            return;
        }
        self.scroll(lines);
        // The geometry's top moves with the scroll; map with the new top.
        let offset_top = |g: Geometry, lines: i32| {
            let max_top = g.total.saturating_sub(g.rows);
            (g.top as i64 - lines as i64).clamp(0, max_top as i64) as usize
        };
        let top = offset_top(geometry, lines);
        self.geometry.set(Some(Geometry { top, ..geometry }));
        if let Some(point) = self.grid_point_at(drag.position)
            && let Some(sel) = &mut self.selection
        {
            sel.head = point;
        }
        cx.notify();
        self.schedule_selection_scroll(cx);
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl RenderOnce for Terminal {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let autofocus = self.autofocus;
        let initial_height = self.height;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| {
                let focus = cx.focus_handle().tab_stop(true);
                if autofocus {
                    let handle = focus.clone();
                    window.defer(cx, move |window, cx| window.focus(&handle, cx));
                }
                TermState {
                    focus,
                    sessions: HashMap::new(),
                    active_tab: None,
                    geometry: Rc::new(Cell::new(None)),
                    selection: None,
                    sel_drag: None,
                    sel_task: None,
                    scroll_remainder: 0.0,
                    rail: Rail::default(),
                    tab_drag: None,
                    commands_open: false,
                    height: initial_height,
                    resize_anchor: None,
                    on_input: None,
                }
            },
        );

        let active = self.active.min(self.tabs.len().saturating_sub(1));
        let active_tab = self.tabs.get(active).cloned();
        let dragging = cx.has_active_drag();
        let titles: Vec<SharedString> = state.update(cx, |s, _| {
            s.on_input = self.on_input.clone();
            if !dragging {
                s.tab_drag = None;
            }
            for tab in &self.tabs {
                s.sessions
                    .entry(tab.id.clone())
                    .or_insert_with(Session::new)
                    .sync(&tab.output, tab.exited);
            }
            s.sessions
                .retain(|id, _| self.tabs.iter().any(|tab| tab.id == *id));
            let switched = s.active_tab.as_ref() != active_tab.as_ref().map(|t| &t.id);
            if switched {
                s.active_tab = active_tab.as_ref().map(|t| t.id.clone());
                s.rail.last_from_top = None;
                s.rail.last_scroll_at = None;
            }
            // Contextual labels: the shell's OSC title wins (Pink
            // `display_title`).
            self.tabs
                .iter()
                .map(|tab| {
                    s.sessions
                        .get(&tab.id)
                        .and_then(|session| session.emu.borrow().title.clone())
                        .map(|title| title.trim().to_string())
                        .filter(|title| !title.is_empty())
                        .map(SharedString::from)
                        .unwrap_or_else(|| tab.title.clone())
                })
                .collect()
        });

        let focused = state.read(cx).focus.is_focused(window);
        let height = state.read(cx).height.min(self.max_height).max(MIN_HEIGHT);
        // Opening and closing tween the drawer over `RESIZE`; a drag resizes
        // it directly (Pink: `terminal_tween` exists only for a toggle).
        let openness = motion::follow(
            &format!("{key}-drawer"),
            if self.open { 1.0 } else { 0.0 },
            RESIZE,
        );
        let container_h = (height + 1.0) * openness;

        let tab_bar = tab_bar(&key, &t, &state, &self, &titles, active, cx);
        let commands = commands_shelf(&key, &t, &state, &self, cx);
        let body = terminal_body(&key, &t, &state, active_tab.as_ref(), focused, window, cx);
        let handle = resize_handle(
            &key,
            &t,
            &state,
            self.max_height,
            self.on_resize.clone(),
            cx,
        );

        // Pink `render_terminal_container`: a fixed-height inner clipped by
        // the animated container, so nothing reflows while it slides; the
        // grip floats over the top edge.
        let inner = div()
            .h(px(height))
            .w_full()
            .relative()
            .flex()
            .flex_col()
            .bg(t.terminal_panel_fill())
            .font_family(pink::SANS)
            .child(tab_bar)
            .child(commands)
            .child(body)
            .child(handle);

        motion::keep_alive(window);
        // Shut and settled, the drawer leaves nothing behind, edge included.
        let shut = !self.open && container_h < 0.5;
        div()
            .id(self.id.clone())
            .w(px(self.width))
            .flex_none()
            .overflow_hidden()
            .when(!shut, |el| el.border_t_1().border_color(t.border))
            .h(px(if shut { 0.0 } else { container_h }))
            .when(!shut, |el| el.child(inner))
    }
}

/// The 38 px strip: tabs, "+", and the collapse chevron (Pink
/// `render_tab_bar`).
fn tab_bar(
    key: &str,
    t: &Pink,
    state: &Entity<TermState>,
    term: &Terminal,
    titles: &[SharedString],
    active: usize,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let drag = state.read(cx).tab_drag;
    let count = term.tabs.len();
    let move_state = state.clone();
    let drop_state = state.clone();
    let on_reorder = term.on_reorder.clone();
    let tabs = term.tabs.iter().enumerate().map(|(ix, tab)| {
        let selected = ix == active;
        let exited = tab.exited.is_some();
        let title = titles.get(ix).cloned().unwrap_or_else(|| tab.title.clone());
        // Pink tab, drawn by `pink-terminal-tabs`: the active tab sits on an
        // ink(0.08) wash; only it shows the close control, the others keep
        // its room (`invisible`) so labels never shift.
        let on_close = term.on_close.clone();
        let close_key = format!("{key}-tab-close-{}", tab.id);
        let close = tabs::close_button(&tabs::close_style(&close_key, selected, t), ICON_CLOSE)
            .id(SharedString::from(close_key.clone()))
            .on_hover(motion::hover_listener(close_key))
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                if let Some(f) = &on_close {
                    f(ix, window, cx);
                }
            });
        let on_select = term.on_select.clone();
        let middle_close = term.on_close.clone();
        let focus = state.read(cx).focus.clone();
        let hover_key = format!("{key}-tab-{}", tab.id);
        let tab_el = tabs::tab(
            &tabs::tab_style(&hover_key, selected, exited, t),
            ICON_TERMINAL,
            title.clone(),
            close,
        )
        .id(SharedString::from(format!("{key}-tab-el-{}", tab.id)))
        .on_hover(motion::hover_listener(hover_key))
        .on_click(move |_, window, cx| {
            if let Some(f) = &on_select {
                f(ix, window, cx);
            }
            window.focus(&focus, cx);
        })
        .on_mouse_down(MouseButton::Middle, move |_, window, cx| {
            if let Some(f) = &middle_close {
                f(ix, window, cx);
            }
        })
        .on_drag(
            TabDrag {
                from: ix,
                title: title.clone(),
            },
            |payload, _, _, cx| {
                cx.stop_propagation();
                let title = payload.title.clone();
                cx.new(|_| TabGhost { title })
            },
        );
        // While a sibling is dragged over, tabs slide between committed
        // offsets over 150 ms; the dragged tab leaves a spacer.
        match drag {
            Some(d) if ix != d.from => {
                let target = slide_offset(ix, d.from, d.over) * TAB_WIDTH;
                let start = slide_offset(ix, d.from, d.prev_over) * TAB_WIDTH;
                div()
                    .relative()
                    .child(tab_el.with_animation(
                        SharedString::from(format!("{key}-tab-slide-{}-{}", tab.id, d.epoch)),
                        TAB_SLIDE.animation(),
                        move |el, p| el.left(px(motion::lerp(start, target, p))),
                    ))
                    .into_any_element()
            }
            Some(d) if ix == d.from => tabs::drag_spacer().into_any_element(),
            _ => tab_el.into_any_element(),
        }
    });

    let on_new = term.on_new_tab.clone();
    let new_focus = state.read(cx).focus.clone();
    let new_key = format!("{key}-new-tab");
    let new_tab = tabs::add_button(&tabs::button_style(&new_key, 0.6, t), ICON_PLUS)
        .id(SharedString::from(new_key.clone()))
        .on_hover(motion::hover_listener(new_key))
        .on_click(move |_, window, cx| {
            if let Some(f) = &on_new {
                f(window, cx);
            }
            window.focus(&new_focus, cx);
        });

    let on_collapse = term.on_collapse.clone();
    let collapse_key = format!("{key}-collapse");
    let collapse = tabs::collapse_button(&tabs::button_style(&collapse_key, 0.55, t), ICON_DOWN)
        .id(SharedString::from(collapse_key.clone()))
        .on_hover(motion::hover_listener(collapse_key))
        .on_click(move |_, window, cx| {
            if let Some(f) = &on_collapse {
                f(window, cx);
            }
        });

    // Pink: `flex h-10 items-center border-b border-white/[0.07] pl-2
    // pr-1.5` on the terminal plate, no fill of its own.
    tabs::bar(t.hairline(0.07))
        .id(SharedString::from(format!("{key}-tab-bar")))
        .on_drag_move::<TabDrag>(move |ev, _, cx| {
            let from = ev.drag(cx).from;
            let rel_x = f32::from(ev.event.position.x) - f32::from(ev.bounds.left());
            let over = drop_index(rel_x, TAB_WIDTH, count);
            move_state.update(cx, |s, cx| match &mut s.tab_drag {
                Some(d) if d.over != over => {
                    d.prev_over = d.over;
                    d.over = over;
                    d.epoch += 1;
                    cx.notify();
                }
                Some(_) => {}
                None => {
                    s.tab_drag = Some(TabDragState {
                        from,
                        over,
                        epoch: 0,
                        prev_over: from,
                    });
                    cx.notify();
                }
            });
        })
        .on_drop::<TabDrag>(move |payload, window, cx| {
            let to = drop_state
                .read(cx)
                .tab_drag
                .map(|d| d.over)
                .unwrap_or(payload.from);
            drop_state.update(cx, |s, cx| {
                s.tab_drag = None;
                cx.notify();
            });
            if to != payload.from
                && let Some(f) = &on_reorder
            {
                f(payload.from, to, window, cx);
            }
        })
        .children(tabs)
        .child(new_tab)
        .child(tabs::spacer())
        .child(collapse)
}

/// The Commands shelf under the strip: a Ghost disclosure, then the
/// project's directory and its saved commands with Run (Pink:
/// `project_commands.rs` `ProjectCommandPanel`).
fn commands_shelf(
    key: &str,
    t: &Pink,
    state: &Entity<TermState>,
    term: &Terminal,
    cx: &App,
) -> gpui::Div {
    let open = state.read(cx).commands_open;
    let toggle_state = state.clone();
    let toggle = pink::button(
        format!("{key}-commands-toggle"),
        ButtonKind::Ghost,
        ControlSize::Sm,
        t,
    )
    .aria_expanded(open)
    .child(pink::glyph(
        if open { ICON_DOWN } else { ICON_RIGHT },
        ControlSize::Sm,
    ))
    .child("Commands")
    .on_click(move |_, _, cx| {
        toggle_state.update(cx, |s, cx| {
            s.commands_open = !s.commands_open;
            cx.notify();
        })
    });

    let mut shelf = div()
        .flex_none()
        .flex()
        .flex_col()
        .px(px(12.0))
        .py(px(4.0))
        .gap(px(4.0))
        .border_b_1()
        .border_color(t.border)
        .child(div().flex().flex_row().child(toggle));
    if !open {
        return shelf;
    }

    let mut rows = div()
        .id(SharedString::from(format!("{key}-command-list")))
        .max_h(px(180.0))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .pb(px(4.0));
    if let Some(cwd) = &term.cwd {
        rows = rows.child(
            div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(6.0))
                .type_style(TextStyle::Caption)
                .text_color(t.readable_muted(0.8))
                .child(div().flex_none().mt(px(1.0)).child(
                    pink::glyph(ICON_FOLDER, ControlSize::Sm).text_color(t.readable_muted(0.8)),
                ))
                .child(div().min_w_0().child(cwd.clone())),
        );
    }
    if term.commands.is_empty() {
        rows = rows.child(pink::row_description(
            "No saved commands. Add them in Settings → Projects.",
            t,
        ));
    }
    for command in &term.commands {
        let on_run = term.on_run_command.clone();
        let id = command.id.clone();
        let focus = state.read(cx).focus.clone();
        rows = rows.child(
            div()
                .w_full()
                .min_h(px(28.0))
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(120.0))
                        .type_style(TextStyle::Footnote)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(command.name.clone()),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .when(command.repository, |el| {
                            el.child(pink::badge("Repository", pink::Tone::Neutral, t))
                        })
                        .child(
                            pink::button(
                                format!("{key}-run-{}", command.id),
                                ButtonKind::Secondary,
                                ControlSize::Sm,
                                t,
                            )
                            .aria_label(SharedString::from(format!("Run {}", command.name)))
                            .child(pink::glyph(ICON_PLAY, ControlSize::Sm))
                            .child("Run")
                            .on_click(move |_, window, cx| {
                                if let Some(f) = &on_run {
                                    f(&id, window, cx);
                                }
                                window.focus(&focus, cx);
                            }),
                        ),
                ),
        );
    }
    shelf = shelf.child(rows);
    shelf
}

/// The terminal body: the grid, pointer selection, the wheel, keys and the
/// floating rail (Pink `TerminalPanel::render`'s `terminal-body`).
fn terminal_body(
    key: &str,
    t: &Pink,
    state: &Entity<TermState>,
    tab: Option<&TerminalTab>,
    focused: bool,
    window: &mut Window,
    cx: &mut App,
) -> gpui::Stateful<gpui::Div> {
    let s = state.read(cx);
    let focus = s.focus.clone();
    let geometry = s.geometry.clone();
    let tab_id = tab.map(|tab| tab.id.clone());
    let exited = tab.is_some_and(|tab| tab.exited.is_some());

    // A snapshot of the active session for this frame's paint.
    let snapshot = tab_id.as_ref().and_then(|id| {
        s.sessions
            .get(id)
            .map(|session| (session.emu.clone(), session.offset, session.last_total))
    });
    let selection = s
        .selection
        .clone()
        .filter(|sel| Some(&sel.tab) == tab_id.as_ref());

    // The rail reads last frame's placement.
    let rail_el = geometry.get().and_then(|g| {
        let offset = snapshot.as_ref().map(|(_, o, _)| *o).unwrap_or(0);
        let history = g.total.saturating_sub(g.rows);
        let from_top = history.saturating_sub(offset);
        let metrics = rail_metrics(
            g.rows as f32 * g.line_h,
            (history + g.rows) as f32 * g.line_h,
            from_top as f32 * g.line_h,
        )?;
        Some((g, metrics, from_top))
    });
    if let Some((_, _, from_top)) = rail_el {
        state.update(cx, |s, _| s.rail.note(from_top));
    }
    let s = state.read(cx);
    let fade = s.rail.fade();
    let rail_active = s.rail.active();
    if fade > 0.0 && !rail_active {
        window.request_animation_frame();
    }
    let rail = rail_el.filter(|_| fade > 0.0).map(|(g, metrics, _)| {
        let hover_state = state.clone();
        let press_state = state.clone();
        let drag_state = state.clone();
        let up_state = state.clone();
        let track_top = f32::from(g.bounds.top());
        let line_h = g.line_h;
        let apply = move |s: &mut TermState, y: f32| {
            let Some(grab) = s.rail.grab else { return };
            let in_track = y - track_top - RAIL_TRACK_INSET;
            let thumb_top = (in_track - grab).clamp(0.0, metrics.travel);
            let fraction = if metrics.travel <= 0.0 {
                0.0
            } else {
                thumb_top / metrics.travel
            };
            let lines = (((1.0 - fraction) * metrics.max_scroll) / line_h).round() as usize;
            if let Some(session) = s.session() {
                session.offset = lines;
            }
        };
        let apply_drag = apply;
        scroll_rail::rail(
            t.text_faint,
            fade,
            rail_active,
            metrics.thumb_top,
            metrics.thumb_height,
        )
        .id(SharedString::from(format!("{key}-rail")))
        .on_hover(move |hovered, _, cx| {
            hover_state.update(cx, |s, cx| {
                let active = *hovered || s.rail.grab.is_some();
                if s.rail.bar_hovered != active {
                    s.rail.bar_hovered = active;
                    if !active {
                        s.rail.last_scroll_at = Some(Instant::now());
                    }
                    cx.notify();
                }
            })
        })
        .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, _, cx| {
            cx.stop_propagation();
            let y = f32::from(ev.position.y);
            press_state.update(cx, |s, cx| {
                let in_track = y - track_top - RAIL_TRACK_INSET;
                let grab = if (metrics.thumb_top..=metrics.thumb_top + metrics.thumb_height)
                    .contains(&in_track)
                {
                    in_track - metrics.thumb_top
                } else {
                    metrics.thumb_height / 2.0
                };
                s.rail.grab = Some(grab);
                apply(s, y);
                cx.notify();
            })
        })
        .on_drag(RailDrag, |_, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| EmptyGhost)
        })
        .on_drag_move::<RailDrag>(move |ev, _, cx| {
            let y = f32::from(ev.event.position.y);
            drag_state.update(cx, |s, cx| {
                apply_drag(s, y);
                cx.notify();
            })
        })
        .on_mouse_up(MouseButton::Left, move |_: &MouseUpEvent, _, cx| {
            up_state.update(cx, |s, cx| {
                s.rail.grab = None;
                s.rail.last_scroll_at = Some(Instant::now());
                cx.notify();
            })
        })
    });

    let grid = snapshot.map(|(emu, offset, last_total)| {
        let id = tab_id.clone().unwrap_or_default();
        grid_canvas(
            emu,
            offset,
            last_total,
            selection,
            focused,
            Palette::new(t),
            geometry.clone(),
            state.clone(),
            id,
        )
    });

    // -- Input --------------------------------------------------------------
    let key_state = state.clone();
    let key_tab = tab_id.clone();
    let on_key = move |ev: &KeyDownEvent, window: &mut Window, cx: &mut App| {
        let ks = &ev.keystroke;
        let mods = &ks.modifiers;
        let Some(tab) = key_tab.clone() else { return };
        // Paste: Cmd+V / Ctrl+Shift+V.
        if ks.key == "v" && (mods.platform || (mods.control && mods.shift)) {
            if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                let bytes = text.replace("\x1b[201~", "").into_bytes();
                send_input(&key_state, &tab, &bytes, exited, window, cx);
            }
            cx.stop_propagation();
            return;
        }
        // Copy: Cmd+C / Ctrl+Shift+C, swallowed only when it copied.
        if ks.key == "c"
            && (mods.platform || (mods.control && mods.shift))
            && let Some(text) = key_state.read(cx).copied_text()
        {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            cx.stop_propagation();
            return;
        }
        if let Some(bytes) = keystroke_bytes(&ks.key, ks.key_char.as_deref(), mods) {
            send_input(&key_state, &tab, &bytes, exited, window, cx);
            cx.stop_propagation();
        }
    };

    let down_state = state.clone();
    let down_tab = tab_id.clone();
    let down_focus = focus.clone();
    let on_down = move |ev: &MouseDownEvent, window: &mut Window, cx: &mut App| {
        window.focus(&down_focus, cx);
        let Some(tab) = down_tab.clone() else { return };
        down_state.update(cx, |s, cx| {
            let ty = match ev.click_count {
                0 => return,
                1 => SelectionType::Simple,
                2 => SelectionType::Semantic,
                _ => SelectionType::Lines,
            };
            let Some(point) = s.grid_point_at(ev.position) else {
                return;
            };
            let cols = s.geometry.get().map(|g| g.cols).unwrap_or(1);
            if ty == SelectionType::Simple {
                // Shift+click extends; a plain press clears and arms.
                if ev.modifiers.shift
                    && let Some(sel) = &mut s.selection
                    && sel.tab == tab
                {
                    sel.head = point;
                    s.sel_drag = Some(SelectionDrag {
                        origin: ev.position,
                        position: ev.position,
                        armed: true,
                    });
                    cx.notify();
                    return;
                }
                s.selection = None;
                s.sel_drag = Some(SelectionDrag {
                    origin: ev.position,
                    position: ev.position,
                    armed: false,
                });
            } else {
                // Word and line selections are whole on the press.
                s.selection = Some(Selection {
                    tab,
                    cols,
                    ty,
                    anchor: point,
                    head: point,
                });
                s.sel_drag = Some(SelectionDrag {
                    origin: ev.position,
                    position: ev.position,
                    armed: true,
                });
            }
            cx.notify();
        });
    };

    let move_state = state.clone();
    let move_tab = tab_id.clone();
    let on_move = move |ev: &MouseMoveEvent, _: &mut Window, cx: &mut App| {
        if !ev.dragging() {
            return;
        }
        let Some(tab) = move_tab.clone() else { return };
        move_state.update(cx, |s, cx| {
            let Some(mut drag) = s.sel_drag else { return };
            drag.position = ev.position;
            s.sel_drag = Some(drag);
            if !drag.armed {
                let dx = f32::from(ev.position.x - drag.origin.x);
                let dy = f32::from(ev.position.y - drag.origin.y);
                if dx.hypot(dy) < SELECTION_DRAG_THRESHOLD {
                    return;
                }
                // Anchor at the press, so the selection covers the gesture.
                let Some(anchor) = s.grid_point_at(drag.origin) else {
                    return;
                };
                let cols = s.geometry.get().map(|g| g.cols).unwrap_or(1);
                s.selection = Some(Selection {
                    tab,
                    cols,
                    ty: SelectionType::Simple,
                    anchor,
                    head: anchor,
                });
                s.sel_drag = Some(SelectionDrag {
                    armed: true,
                    ..drag
                });
            }
            if let Some(point) = s.grid_point_at(ev.position)
                && let Some(sel) = &mut s.selection
            {
                sel.head = point;
            }
            cx.notify();
            s.schedule_selection_scroll(cx);
        });
    };

    let up_state = state.clone();
    let up_out_state = state.clone();
    let wheel_state = state.clone();
    let hover_state = state.clone();
    div()
        .id(SharedString::from(format!("{key}-body")))
        .relative()
        .flex_1()
        .min_h_0()
        .key_context("Terminal")
        .track_focus(&focus)
        .on_hover(move |hovered, _, cx| {
            hover_state.update(cx, |s, _| s.rail.list_hovered = *hovered)
        })
        .on_key_down(on_key)
        .on_mouse_down(MouseButton::Left, on_down)
        .on_mouse_move(on_move)
        .on_mouse_up(MouseButton::Left, move |_: &MouseUpEvent, _, cx| {
            up_state.update(cx, |s, _| {
                s.sel_drag = None;
                s.sel_task = None;
            })
        })
        .on_mouse_up_out(MouseButton::Left, move |_: &MouseUpEvent, _, cx| {
            up_out_state.update(cx, |s, _| {
                s.sel_drag = None;
                s.sel_task = None;
            })
        })
        .on_scroll_wheel(move |ev: &ScrollWheelEvent, _, cx| {
            wheel_state.update(cx, |s, cx| {
                let line_h = s
                    .geometry
                    .get()
                    .map(|g| g.line_h)
                    .unwrap_or(TERM_LINE_HEIGHT);
                let lines = match ev.delta {
                    ScrollDelta::Lines(delta) => delta.y,
                    ScrollDelta::Pixels(delta) => f32::from(delta.y) / line_h,
                };
                // Fractions accumulate; a reversal starts fresh.
                if (s.scroll_remainder > 0.0) != (lines > 0.0) {
                    s.scroll_remainder = 0.0;
                }
                s.scroll_remainder += lines;
                let step = s.scroll_remainder.trunc();
                s.scroll_remainder -= step;
                if step != 0.0 {
                    s.scroll(step as i32);
                    cx.notify();
                }
            })
        })
        .children(grid)
        .children(rail)
}

/// Keystrokes go to a live session only, and snap the view to the bottom.
fn send_input(
    state: &Entity<TermState>,
    tab: &SharedString,
    bytes: &[u8],
    exited: bool,
    window: &mut Window,
    cx: &mut App,
) {
    if exited {
        return;
    }
    let handler = state.update(cx, |s, cx| {
        if let Some(session) = s.sessions.get_mut(tab)
            && session.offset > 0
        {
            session.offset = 0;
            cx.notify();
        }
        s.on_input.clone()
    });
    if let Some(f) = handler {
        f(tab, bytes, window, cx);
    }
}

/// Everything prepaint measured and shaped for paint.
struct GridPaint {
    bg_quads: Vec<PaintQuad>,
    sel_quads: Vec<PaintQuad>,
    lines: Vec<Vec<(usize, ShapedLine)>>,
    cell_w: Pixels,
    line_h: Pixels,
    cursor: Option<PaintQuad>,
    cursor_glyph: Option<(usize, usize, ShapedLine)>,
}

/// The grid element: measures Geist Mono, wraps the session to the width,
/// and paints backgrounds, the selection wash, column-pinned text and the
/// cursor (Pink `TerminalElement::prepaint` / `paint`).
#[allow(clippy::too_many_arguments)]
fn grid_canvas(
    emu: Rc<RefCell<Emulator>>,
    offset: usize,
    last_total: usize,
    selection: Option<Selection>,
    focused: bool,
    palette: Palette,
    geometry: Rc<Cell<Option<Geometry>>>,
    state: Entity<TermState>,
    tab: SharedString,
) -> impl IntoElement {
    canvas(
        move |bounds, window, cx| {
            let emu = emu.borrow();
            // Ligatures off: the grid needs one cell per character.
            let mut mono = font(pink::MONO);
            mono.features = gpui::FontFeatures(Arc::new(vec![
                ("liga".into(), 0),
                ("calt".into(), 0),
                ("dlig".into(), 0),
            ]));
            let font_size = px(TERM_FONT_SIZE);
            let font_id = window.text_system().resolve_font(&mono);
            let cell_w = window
                .text_system()
                .em_advance(font_id, font_size)
                .unwrap_or(px(TERM_FONT_SIZE * 0.6));
            let line_h = px(TERM_LINE_HEIGHT);
            let inner_w = f32::from(bounds.size.width) - 2.0 * TERM_PADDING;
            let inner_h = f32::from(bounds.size.height) - 2.0 * TERM_PADDING;
            let cols = ((inner_w / f32::from(cell_w)).floor() as i64).clamp(2, 500) as usize;
            let rows = ((inner_h / f32::from(line_h)).floor() as i64).clamp(1, 500) as usize;
            let origin = point(
                bounds.left() + px(TERM_PADDING),
                bounds.top() + px(TERM_PADDING),
            );

            let (spans, cursor_at) = wrap(&emu, cols);
            let total = spans.len();
            let max_offset = total.saturating_sub(rows);
            // New output keeps a scrolled-back view where it was.
            let mut offset = offset;
            if offset > 0 && total > last_total && last_total > 0 {
                offset += total - last_total;
            }
            let offset = offset.min(max_offset);
            let top = total.saturating_sub(rows).saturating_sub(offset);
            state.update(cx, |s, _| {
                if let Some(session) = s.sessions.get_mut(&tab) {
                    session.offset = offset;
                    session.last_total = total;
                }
            });
            geometry.set(Some(Geometry {
                bounds,
                origin,
                cell_w: f32::from(cell_w),
                line_h: f32::from(line_h),
                cols,
                rows,
                total,
                top,
            }));

            let range = selection
                .as_ref()
                .filter(|sel| sel.cols == cols)
                .and_then(|sel| selection_range(sel, &emu, &spans));
            let selected = |row: usize, col: usize| {
                range.is_some_and(|(s, e)| {
                    let ix = row * cols + col;
                    ix >= s && ix < e
                })
            };

            let mut bg_quads = Vec::new();
            let mut sel_quads = Vec::new();
            let mut lines = Vec::with_capacity(rows);
            for r in 0..rows {
                let abs = top + r;
                let Some(span) = spans.get(abs) else { break };
                let y = origin.y + line_h * r as f32;
                let cells: Vec<TermCell> = (0..cols)
                    .map(|col| {
                        emu.lines
                            .get(span.line)
                            .and_then(|line| line.get(span.start + col))
                            .filter(|_| span.start + col < span.end)
                            .copied()
                            .unwrap_or_else(TermCell::blank)
                    })
                    .collect();
                // Selected runs, one quad per span.
                let mut sel_start: Option<usize> = None;
                for col in 0..=cols {
                    let on = col < cols && selected(abs, col);
                    match (sel_start, on) {
                        (None, true) => sel_start = Some(col),
                        (Some(start), false) => {
                            sel_quads.push(fill(
                                Bounds::new(
                                    point(origin.x + cell_w * start as f32, y),
                                    size(cell_w * (col - start) as f32, line_h),
                                ),
                                palette.selection,
                            ));
                            sel_start = None;
                        }
                        _ => {}
                    }
                }
                // Explicit backgrounds, merged into runs.
                let mut run: Option<(usize, Hsla)> = None;
                for col in 0..=cols {
                    let paint = cells
                        .get(col)
                        .and_then(|cell| match cell.display_colors().1 {
                            CellColor::Background => None,
                            other => Some(palette.resolve(other)),
                        });
                    match (run, paint) {
                        (None, Some(color)) => run = Some((col, color)),
                        (Some((start, current)), next) if next != Some(current) => {
                            bg_quads.push(fill(
                                Bounds::new(
                                    point(origin.x + cell_w * start as f32, y),
                                    size(cell_w * (col - start) as f32, line_h),
                                ),
                                current,
                            ));
                            run = next.map(|color| (col, color));
                        }
                        _ => {}
                    }
                }
                lines.push(shape_row(
                    &cells,
                    |col| selected(abs, col),
                    &palette,
                    &mono,
                    font_size,
                    window,
                ));
            }

            let cursor_cell = (!emu.cursor_hidden)
                .then_some(cursor_at)
                .filter(|(row, _)| *row >= top && *row < top + rows)
                .map(|(row, col)| (row - top, col.min(cols - 1)));
            let cursor = cursor_cell.map(|(row, col)| {
                let b = Bounds::new(
                    point(
                        origin.x + cell_w * col as f32,
                        origin.y + line_h * row as f32,
                    ),
                    size(cell_w, line_h),
                );
                if focused {
                    // A translucent block: the glyph under it stays legible.
                    fill(b, palette.cursor)
                } else {
                    outline(b, palette.cursor, BorderStyle::Solid)
                }
            });
            let cursor_glyph = cursor_cell.filter(|_| focused).and_then(|(row, col)| {
                let span = spans.get(top + row)?;
                let cell = emu.lines.get(span.line)?.get(span.start + col)?;
                if cell.pen.hidden || cell.ch == ' ' || span.start + col >= span.end {
                    return None;
                }
                let color = cursor_glyph_fg(cell, selected(top + row, col), &palette);
                let mut glyph_font = mono.clone();
                if cell.pen.bold {
                    glyph_font.weight = FontWeight::BOLD;
                }
                let text = SharedString::from(cell.ch.to_string());
                let shaped = window.text_system().shape_line(
                    text.clone(),
                    font_size,
                    &[TextRun {
                        len: text.len(),
                        font: glyph_font,
                        color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    }],
                    None,
                );
                Some((col, row, shaped))
            });

            GridPaint {
                bg_quads,
                sel_quads,
                lines,
                cell_w,
                line_h,
                cursor,
                cursor_glyph,
            }
        },
        move |bounds, mut paint, window, cx| {
            let origin = point(
                bounds.left() + px(TERM_PADDING),
                bounds.top() + px(TERM_PADDING),
            );
            window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
                for quad in paint.bg_quads.drain(..) {
                    window.paint_quad(quad);
                }
                for quad in paint.sel_quads.drain(..) {
                    window.paint_quad(quad);
                }
                for (ix, segments) in paint.lines.iter().enumerate() {
                    let y = origin.y + paint.line_h * ix as f32;
                    for (col, line) in segments {
                        let _ = line.paint(
                            point(origin.x + paint.cell_w * *col as f32, y),
                            paint.line_h,
                            gpui::TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                }
                if let Some(cursor) = paint.cursor.take() {
                    window.paint_quad(cursor);
                }
                if let Some((col, row, glyph)) = paint.cursor_glyph.take() {
                    let _ = glyph.paint(
                        point(
                            origin.x + paint.cell_w * col as f32,
                            origin.y + paint.line_h * row as f32,
                        ),
                        paint.line_h,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
            });
        },
    )
    .size_full()
}

/// One row as column-pinned segments: ASCII runs shape together, anything
/// that may leave the mono font is its own segment at its own column (Pink
/// `shape_row`).
fn shape_row(
    cells: &[TermCell],
    selected: impl Fn(usize) -> bool,
    palette: &Palette,
    mono: &Font,
    font_size: Pixels,
    window: &Window,
) -> Vec<(usize, ShapedLine)> {
    let mut segments: Vec<(usize, ShapedLine)> = Vec::new();
    let mut text = String::new();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut seg_col = 0usize;
    let flush = |segments: &mut Vec<(usize, ShapedLine)>,
                 text: &mut String,
                 runs: &mut Vec<TextRun>,
                 seg_col: usize| {
        if text.is_empty() {
            return;
        }
        let shaped = window.text_system().shape_line(
            SharedString::from(std::mem::take(text)),
            font_size,
            runs,
            None,
        );
        segments.push((seg_col, shaped));
        runs.clear();
    };
    for (col, cell) in cells.iter().enumerate() {
        let ch = if cell.pen.hidden { ' ' } else { cell.ch };
        let pinned = !ch.is_ascii();
        if pinned {
            flush(&mut segments, &mut text, &mut runs, seg_col);
        }
        if text.is_empty() {
            seg_col = col;
        }
        let color = paint_fg(cell, selected(col), palette);
        let mut cell_font = mono.clone();
        cell_font.weight = if cell.pen.bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        cell_font.style = if cell.pen.italic {
            gpui::FontStyle::Italic
        } else {
            gpui::FontStyle::Normal
        };
        let underline = cell.pen.underline.then_some(gpui::UnderlineStyle {
            color: Some(color),
            thickness: px(1.0),
            wavy: false,
        });
        let len = ch.len_utf8();
        text.push(ch);
        match runs.last_mut() {
            Some(last)
                if last.color == color && last.font == cell_font && last.underline == underline =>
            {
                last.len += len;
            }
            _ => runs.push(TextRun {
                len,
                font: cell_font,
                color,
                background_color: None,
                underline,
                strikethrough: None,
            }),
        }
        if pinned {
            flush(&mut segments, &mut text, &mut runs, seg_col);
        }
    }
    flush(&mut segments, &mut text, &mut runs, seg_col);
    segments
}

/// The 10 px grip over the drawer's top edge: a 1 px line in
/// `border_strong` while hovered or dragged; drag to resize, double-click
/// for the default height (Pink `render_terminal_container`'s handle).
fn resize_handle(
    key: &str,
    t: &Pink,
    state: &Entity<TermState>,
    max_height: f32,
    on_resize: Option<ResizeHandler>,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let handle_key = format!("{key}-resize");
    let active = state.read(cx).resize_anchor.is_some();
    let highlight = if active {
        t.border_strong
    } else {
        motion::hover_blend(&handle_key, t.border_strong.opacity(0.0), t.border_strong)
    };
    let down_state = state.clone();
    let up_state = state.clone();
    let out_state = state.clone();
    let move_state = state.clone();
    let reset_resize = on_resize.clone();
    div()
        .id(SharedString::from(handle_key.clone()))
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(RESIZE_HITBOX_HEIGHT))
        .cursor_row_resize()
        .on_hover(motion::hover_listener(handle_key))
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(1.0))
                .bg(highlight),
        )
        .on_mouse_down(MouseButton::Left, move |ev: &MouseDownEvent, window, cx| {
            window.prevent_default();
            cx.stop_propagation();
            down_state.update(cx, |s, cx| {
                s.resize_anchor = Some((f32::from(ev.position.y), s.height));
                cx.notify();
            })
        })
        .on_drag(DrawerResize, |_, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| EmptyGhost)
        })
        .on_drag_move::<DrawerResize>(move |ev, window, cx| {
            let y = f32::from(ev.event.position.y);
            let height = move_state.update(cx, |s, cx| {
                let (start_y, start_h) = s.resize_anchor?;
                let next = (start_h - (y - start_y)).clamp(MIN_HEIGHT, max_height.max(MIN_HEIGHT));
                if (next - s.height).abs() < f32::EPSILON {
                    return None;
                }
                s.height = next;
                cx.notify();
                Some(next)
            });
            if let (Some(height), Some(f)) = (height, &on_resize) {
                f(height, window, cx);
            }
        })
        .on_mouse_up(MouseButton::Left, move |ev: &MouseUpEvent, window, cx| {
            let reset = ev.click_count == 2;
            up_state.update(cx, |s, cx| {
                if reset {
                    s.height = DEFAULT_HEIGHT;
                }
                s.resize_anchor = None;
                cx.notify();
            });
            if reset && let Some(f) = &reset_resize {
                f(DEFAULT_HEIGHT, window, cx);
            }
        })
        .on_mouse_up_out(MouseButton::Left, move |_: &MouseUpEvent, _, cx| {
            out_state.update(cx, |s, cx| {
                s.resize_anchor = None;
                cx.notify();
            })
        })
}

// ---------------------------------------------------------------------------
// Story: a scripted shell
// ---------------------------------------------------------------------------

const CWD: &str = "~/code/billing-api";

fn prompt(ok: bool) -> String {
    format!(
        "\x1b]0;billing-api\x07\x1b[1;36m{CWD}\x1b[0m \x1b[35mmain\x1b[0m \x1b[{}m❯\x1b[0m ",
        if ok { "32" } else { "31" }
    )
}

/// `cargo test -p ledger`, line by line: (delay ms, text).
fn cargo_test_output() -> Vec<(u64, String)> {
    let g = |s: &str| format!("\x1b[1;32m{s:>12}\x1b[0m");
    let mut lines: Vec<(u64, String)> = vec![
        (
            260,
            format!("{} ledger v0.4.2 ({CWD}/crates/ledger)\r\n", g("Compiling")),
        ),
        (
            420,
            "\x1b[1;33mwarning\x1b[0m\x1b[1m: unused import: `std::fmt`\x1b[0m\r\n".into(),
        ),
        (
            40,
            " \x1b[1;34m-->\x1b[0m crates/ledger/src/currency.rs:3:5\r\n".into(),
        ),
        (40, "  \x1b[1;34m|\x1b[0m\r\n".into()),
        (
            40,
            "\x1b[1;34m3\x1b[0m \x1b[1;34m|\x1b[0m use std::fmt;\r\n".into(),
        ),
        (
            40,
            "  \x1b[1;34m|\x1b[0m     \x1b[1;33m^^^^^^^^\x1b[0m\r\n\r\n".into(),
        ),
        (
            620,
            format!(
                "{} `test` profile [unoptimized + debuginfo] target(s) in 3.84s\r\n",
                g("Finished")
            ),
        ),
        (
            80,
            format!(
                "{} unittests src/lib.rs (target/debug/deps/ledger-1f2c3a4b5d6e7f80)\r\n\r\n",
                g("Running")
            ),
        ),
        (60, "running 9 tests\r\n".into()),
    ];
    let tests = [
        ("currency::tests::minor_units_round_trip", true),
        ("currency::tests::rounds_half_to_even", true),
        ("invoice::tests::applies_tax_per_line", true),
        ("invoice::tests::credit_note_matches_invoice", true),
        ("invoice::tests::refund_reverses_ledger_entry", false),
        ("ledger::tests::balances_after_transfer", true),
        ("ledger::tests::rejects_negative_opening_balance", true),
        ("ledger::tests::monthly_view_sums_entries", true),
        ("sync::tests::retries_stop_after_five", true),
    ];
    for (name, ok) in tests {
        let verdict = if ok {
            "\x1b[32mok\x1b[0m"
        } else {
            "\x1b[31mFAILED\x1b[0m"
        };
        lines.push((70, format!("test {name} ... {verdict}\r\n")));
    }
    lines.extend([
        (90, "\r\nfailures:\r\n\r\n".to_string()),
        (
            30,
            "---- invoice::tests::refund_reverses_ledger_entry stdout ----\r\n".into(),
        ),
        (
            30,
            "thread 'invoice::tests::refund_reverses_ledger_entry' panicked at crates/ledger/src/invoice.rs:212:9:\r\n".into(),
        ),
        (30, "assertion `left == right` failed\r\n".into()),
        (30, "  left: -4200\r\n right: -4199\r\n\r\n".into()),
        (
            40,
            "failures:\r\n    invoice::tests::refund_reverses_ledger_entry\r\n\r\n".into(),
        ),
        (
            60,
            "test result: \x1b[31mFAILED\x1b[0m. 8 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s\r\n\r\n".into(),
        ),
        (
            40,
            "\x1b[1;31merror\x1b[0m: test failed, to rerun pass `-p ledger --lib`\r\n".into(),
        ),
    ]);
    lines
}

/// What the scripted shell answers for a command line.
fn run_command(line: &str) -> (Vec<(u64, String)>, bool) {
    let line = line.trim();
    match line {
        "" => (Vec::new(), true),
        "cargo test -p ledger" | "cargo test" => (cargo_test_output(), false),
        "git status -sb" | "git status --short" | "git status" => (
            vec![(
                120,
                "## \x1b[32mmain\x1b[0m...\x1b[31morigin/main\x1b[0m [ahead \x1b[32m1\x1b[0m]\r\n \x1b[31mM\x1b[0m crates/ledger/src/invoice.rs\r\n\x1b[31m??\x1b[0m crates/ledger/tests/refunds.rs\r\n".into(),
            )],
            true,
        ),
        "ls" => (
            vec![(
                60,
                "Cargo.lock  Cargo.toml  \x1b[1;34mcrates\x1b[0m  README.md  \x1b[1;34mscripts\x1b[0m  \x1b[1;34mtarget\x1b[0m\r\n".into(),
            )],
            true,
        ),
        "cargo clippy --workspace" => (
            vec![
                (
                    400,
                    "\x1b[1;32m    Checking\x1b[0m ledger v0.4.2 (~/code/billing-api/crates/ledger)\r\n".into(),
                ),
                (
                    700,
                    "\x1b[1;32m    Finished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 2.10s\r\n".into(),
                ),
            ],
            true,
        ),
        "clear" => (vec![(0, "\x1b[H\x1b[2J".into())], true),
        other if other.starts_with("echo ") => (
            vec![(20, format!("{}\r\n", &other[5..]))],
            true,
        ),
        other => (
            vec![(
                20,
                format!(
                    "zsh: command not found: {}\r\n",
                    other.split_whitespace().next().unwrap_or(other)
                ),
            )],
            false,
        ),
    }
}

struct DemoTab {
    id: SharedString,
    title: SharedString,
    output: String,
    input: String,
    /// A command is printing; typed keys wait.
    busy: bool,
    exited: Option<i32>,
    /// A dev server owns the tab: only Ctrl+C reaches it.
    server: bool,
}

impl DemoTab {
    fn shell(id: &str) -> Self {
        Self {
            id: id.to_string().into(),
            title: "zsh".into(),
            output: prompt(true),
            input: String::new(),
            busy: false,
            exited: None,
            server: false,
        }
    }
}

struct Demo {
    tabs: Vec<DemoTab>,
    active: usize,
    open: bool,
    next_id: usize,
    tasks: Vec<Task<()>>,
}

impl Demo {
    fn tab_mut(&mut self, id: &str) -> Option<&mut DemoTab> {
        self.tabs.iter_mut().find(|tab| tab.id.as_ref() == id)
    }

    /// Streams `chunks` into a tab, then a prompt.
    fn play(
        &mut self,
        id: SharedString,
        chunks: Vec<(u64, String)>,
        ok: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tab_mut(&id) {
            tab.busy = true;
        }
        self.tasks.push(cx.spawn(async move |this, cx| {
            for (delay, chunk) in chunks {
                cx.background_executor()
                    .timer(Duration::from_millis(delay))
                    .await;
                let alive = this
                    .update(cx, |d, cx| {
                        if let Some(tab) = d.tab_mut(&id) {
                            tab.output.push_str(&chunk);
                        }
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    return;
                }
            }
            this.update(cx, |d, cx| {
                if let Some(tab) = d.tab_mut(&id) {
                    tab.output.push_str(&prompt(ok));
                    tab.busy = false;
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// Runs a command line in a tab as if it had been typed and entered.
    fn submit(&mut self, id: SharedString, cx: &mut Context<Self>) {
        let Some(tab) = self.tab_mut(&id) else { return };
        let line = std::mem::take(&mut tab.input);
        tab.output.push_str("\r\n");
        let (chunks, ok) = run_command(&line);
        if line.trim().starts_with("cargo") {
            tab.output.push_str("\x1b]0;cargo\x07");
        }
        self.play(id, chunks, ok, cx);
    }

    /// The scripted shell's line discipline: echo, erase, enter, ^C.
    fn input(&mut self, id: &SharedString, bytes: &[u8], cx: &mut Context<Self>) {
        let text = String::from_utf8_lossy(bytes).to_string();
        let Some(tab) = self.tab_mut(id) else { return };
        if tab.exited.is_some() {
            return;
        }
        if tab.server {
            if bytes == [0x03] {
                tab.output.push_str("^C\r\n");
                tab.output.push_str(&prompt(false));
                tab.server = false;
                tab.title = "zsh".into();
                cx.notify();
            }
            return;
        }
        if tab.busy {
            return;
        }
        for ch in text.chars() {
            let Some(tab) = self.tab_mut(id) else { return };
            match ch {
                '\r' => {
                    self.submit(id.clone(), cx);
                    return;
                }
                '\u{7f}' | '\u{8}' => {
                    if tab.input.pop().is_some() {
                        tab.output.push_str("\x08 \x08");
                    }
                }
                '\u{3}' => {
                    tab.input.clear();
                    tab.output.push_str("^C\r\n");
                    tab.output.push_str(&prompt(true));
                }
                '\u{c}' => {
                    tab.output.push_str("\x1b[H\x1b[2J");
                    tab.output.push_str(&prompt(true));
                    tab.output.push_str(&tab.input.clone());
                }
                c if !c.is_control() => {
                    tab.input.push(c);
                    tab.output.push(c);
                }
                _ => {}
            }
        }
        cx.notify();
    }

    /// Types a command at human speed, then runs it.
    fn type_command(&mut self, id: SharedString, command: &'static str, cx: &mut Context<Self>) {
        if self.tab_mut(&id).is_none_or(|tab| tab.busy || tab.server) {
            return;
        }
        if let Some(tab) = self.tab_mut(&id) {
            tab.busy = true;
        }
        self.tasks.push(cx.spawn(async move |this, cx| {
            for ch in command.chars() {
                cx.background_executor()
                    .timer(Duration::from_millis(38))
                    .await;
                let alive = this
                    .update(cx, |d, cx| {
                        if let Some(tab) = d.tab_mut(&id) {
                            tab.input.push(ch);
                            tab.output.push(ch);
                        }
                        cx.notify();
                    })
                    .is_ok();
                if !alive {
                    return;
                }
            }
            cx.background_executor()
                .timer(Duration::from_millis(260))
                .await;
            this.update(cx, |d, cx| {
                if let Some(tab) = d.tab_mut(&id) {
                    tab.busy = false;
                }
                d.submit(id.clone(), cx);
            })
            .ok();
        }));
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.next_id += 1;
        self.tabs
            .push(DemoTab::shell(&format!("shell-{}", self.next_id)));
        self.active = self.tabs.len() - 1;
        cx.notify();
    }
}

fn dev_server_output() -> String {
    format!(
        "\x1b]0;pnpm dev\x07\x1b[1;36m~/code/billing-web\x1b[0m \x1b[35mmain\x1b[0m \x1b[32m❯\x1b[0m pnpm dev\r\n\r\n\
         > billing-web@0.9.0 dev ~/code/billing-web\r\n> vite\r\n\r\n\
         \x20 \x1b[1;32mVITE\x1b[0m \x1b[32mv5.4.10\x1b[0m  ready in \x1b[1m412\x1b[0m ms\r\n\r\n\
         \x20 \x1b[32m➜\x1b[0m  \x1b[1mLocal\x1b[0m:   \x1b[36mhttp://localhost:\x1b[1m5173\x1b[0m\x1b[36m/\x1b[0m\r\n\
         \x20 \x1b[2;32m➜\x1b[0m  \x1b[2mNetwork: use \x1b[0m\x1b[1m--host\x1b[0m\x1b[2m to expose\x1b[0m\r\n\
         \x20 \x1b[2;32m➜\x1b[0m  \x1b[2mpress \x1b[0m\x1b[1mh + enter\x1b[0m\x1b[2m to show help\x1b[0m\r\n\r\n\
         \x1b[2m10:41:07\x1b[0m \x1b[1;36m[vite]\x1b[0m \x1b[32mhmr update \x1b[0m\x1b[2m/src/pages/Invoices.tsx\x1b[0m\r\n"
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("terminal-demo", cx, |_, cx: &mut Context<Demo>| {
            let mut api = DemoTab::shell("api");
            api.title = "zsh".into();
            let web = DemoTab {
                id: "web".into(),
                title: "pnpm dev".into(),
                output: dev_server_output(),
                input: String::new(),
                busy: false,
                exited: None,
                server: true,
            };
            let psql = DemoTab {
                id: "psql".into(),
                title: "psql".into(),
                output: "\x1b]0;psql\x07psql (16.4)\r\nType \"help\" for help.\r\n\r\nledger=# select count(*) from refunds where reversed_at is null;\r\n count \r\n-------\r\n     3\r\n(1 row)\r\n\r\nledger=# \\q\r\n".into(),
                input: String::new(),
                busy: false,
                exited: Some(0),
                server: false,
            };
            // The session plays itself: tests run (one fails), then a look
            // at what changed.
            let script = cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(700))
                    .await;
                this.update(cx, |d, cx| {
                    d.type_command("api".into(), "cargo test -p ledger", cx)
                })
                .ok();
                cx.background_executor()
                    .timer(Duration::from_millis(4_300))
                    .await;
                this.update(cx, |d, cx| d.type_command("api".into(), "git status -sb", cx))
                    .ok();
            });
            Demo {
                tabs: vec![api, web, psql],
                active: 0,
                open: true,
                next_id: 0,
                tasks: vec![script],
            }
        });

        let d = demo.read(cx);
        let tabs: Vec<TerminalTab> = d
            .tabs
            .iter()
            .map(|tab| {
                let mut out = TerminalTab::new(tab.id.clone(), tab.title.clone())
                    .output(SharedString::from(tab.output.clone()));
                out.exited = tab.exited;
                out
            })
            .collect();
        let (active, open) = (d.active, d.open);

        let input = demo.clone();
        let select = demo.clone();
        let close = demo.clone();
        let new_tab = demo.clone();
        let reorder = demo.clone();
        let collapse = demo.clone();
        let run = demo.clone();
        let toggle = demo.clone();
        // The saved height (400) is taller than D52 allows in this window;
        // the drawer draws at 40% of it and keeps the saved value.
        let terminal = Terminal::new("terminal")
            .width(780.0)
            .height(400.0)
            .window_height(WINDOW_HEIGHT)
            .autofocus(true)
            .open(open)
            .tabs(tabs)
            .active(active)
            .cwd(CWD)
            .commands(vec![
                TerminalCommand::new("test", "Test ledger").repository(true),
                TerminalCommand::new("lint", "Lint workspace").repository(true),
                TerminalCommand::new("status", "Git status"),
            ])
            .on_input(move |tab, bytes, _, cx| input.update(cx, |d, cx| d.input(tab, bytes, cx)))
            .on_select(move |ix, _, cx| {
                select.update(cx, |d, cx| {
                    d.active = ix;
                    cx.notify();
                })
            })
            .on_close(move |ix, _, cx| {
                close.update(cx, |d, cx| {
                    if ix < d.tabs.len() {
                        d.tabs.remove(ix);
                    }
                    let len = d.tabs.len();
                    let shifted = if ix < d.active {
                        d.active - 1
                    } else {
                        d.active
                    };
                    d.active = shifted.min(len.saturating_sub(1));
                    // Closing the last terminal closes the drawer too (Pink).
                    if len == 0 {
                        d.open = false;
                    }
                    cx.notify();
                })
            })
            .on_new_tab(move |_, cx| new_tab.update(cx, |d, cx| d.new_tab(cx)))
            .on_reorder(move |from, to, _, cx| {
                reorder.update(cx, |d, cx| {
                    if from < d.tabs.len() && to < d.tabs.len() {
                        let tab = d.tabs.remove(from);
                        d.tabs.insert(to, tab);
                        let a = d.active;
                        d.active = if a == from {
                            to
                        } else if from < a && to >= a {
                            a - 1
                        } else if from > a && to <= a {
                            a + 1
                        } else {
                            a
                        };
                    }
                    cx.notify();
                })
            })
            .on_collapse(move |_, cx| {
                collapse.update(cx, |d, cx| {
                    d.open = false;
                    cx.notify();
                })
            })
            .on_run_command(move |id, _, cx| {
                run.update(cx, |d, cx| {
                    let Some(tab) = d.tabs.get(d.active).map(|tab| tab.id.clone()) else {
                        return;
                    };
                    let command = match id.as_ref() {
                        "test" => "cargo test -p ledger",
                        "lint" => "cargo clippy --workspace",
                        _ => "git status -sb",
                    };
                    d.type_command(tab, command, cx);
                })
            });

        // A stand-in for Pink's main column: the content panel with the
        // titlebar's terminal toggle; the drawer docks at its foot.
        let toggle_button =
            pink::icon_button("terminal-toggle", ICON_TERMINAL, ControlSize::Md, &t)
                .on_click(move |_, _, cx| {
                    toggle.update(cx, |d, cx| {
                        d.open = !d.open;
                        if d.open && d.tabs.is_empty() {
                            d.new_tab(cx);
                        }
                        cx.notify();
                    })
                })
                .tooltip(pink::text_tooltip("Toggle terminal ⌘J"))
                .tooltip_show_delay(layout::TOOLTIP_DELAY);
        div()
            .w(px(780.0))
            .h(px(WINDOW_HEIGHT))
            .flex()
            .flex_col()
            .rounded(px(pink::radius::PANEL))
            .border_1()
            .border_color(t.border)
            .overflow_hidden()
            .bg(t.bg)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .items_start()
                    .p(px(chrome::EDGE_INSET))
                    .child(toggle_button),
            )
            .child(terminal)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D52 (#176): the drawer is held to 40% of its window (render and drag
    /// both clamp to `max_height`), never under its minimum, and the saved
    /// height is kept. Catches a return to Pink's old 55% share or a cap
    /// that drops below the floor in a short window.
    #[test]
    fn the_drawer_is_held_to_forty_percent_of_its_window() {
        let drawer = Terminal::new("t")
            .height(400.0)
            .window_height(WINDOW_HEIGHT);
        assert_eq!(drawer.max_height, 288.0);
        // The saved height is kept; render draws min(saved, cap).
        assert_eq!(drawer.height, 400.0);
        let short = Terminal::new("t").window_height(300.0);
        assert_eq!(short.max_height, MIN_HEIGHT);
    }

    fn text_of(emu: &Emulator, line: usize) -> String {
        emu.lines[line].iter().map(|c| c.ch).collect()
    }

    #[test]
    fn crlf_moves_lines_and_cr_returns_to_column_zero() {
        let mut emu = Emulator::new();
        emu.feed("hello\r\nworld\rW");
        assert_eq!(text_of(&emu, 0), "hello");
        assert_eq!(text_of(&emu, 1), "World");
        assert_eq!((emu.line, emu.col), (1, 1));
    }

    #[test]
    fn sgr_sets_16_256_and_true_colors_and_resets() {
        let mut emu = Emulator::new();
        emu.feed("\x1b[1;31mR\x1b[38;5;208mO\x1b[48;2;1;2;3mT\x1b[0mN\x1b[92mB");
        let cells = &emu.lines[0];
        assert_eq!(cells[0].pen.fg, CellColor::Indexed(1));
        assert!(cells[0].pen.bold);
        assert_eq!(cells[1].pen.fg, CellColor::Indexed(208));
        assert_eq!(cells[2].pen.bg, CellColor::Rgb(1, 2, 3));
        assert_eq!(cells[3].pen, Pen::default());
        assert_eq!(cells[4].pen.fg, CellColor::Indexed(10));
    }

    #[test]
    fn backspace_erase_and_clear() {
        let mut emu = Emulator::new();
        emu.feed("lsx\x08 \x08");
        assert_eq!(text_of(&emu, 0).trim_end(), "ls");
        emu.feed("\r\nabc\x1b[2D\x1b[K");
        assert_eq!(text_of(&emu, 1), "a");
        emu.feed("\x1b[H\x1b[2J$ ");
        assert_eq!(emu.lines.len(), 1);
        assert_eq!(text_of(&emu, 0), "$ ");
    }

    #[test]
    fn osc_titles_and_cursor_visibility() {
        let mut emu = Emulator::new();
        emu.feed("\x1b]0;cargo test\x07\x1b[?25l");
        assert_eq!(emu.title.as_deref(), Some("cargo test"));
        assert!(emu.cursor_hidden);
        emu.feed("\x1b]2;zsh\x1b\\\x1b[?25h");
        assert_eq!(emu.title.as_deref(), Some("zsh"));
        assert!(!emu.cursor_hidden);
    }

    #[test]
    fn long_lines_wrap_and_the_cursor_follows() {
        let mut emu = Emulator::new();
        emu.feed("abcdefghij\r\nxy");
        let (rows, cursor) = wrap(&emu, 4);
        assert_eq!(rows.len(), 4);
        assert!(rows[1].continued && !rows[3].continued);
        assert_eq!(cursor, (3, 2));
        // A full row keeps the cursor on its last column (pending wrap).
        let mut emu = Emulator::new();
        emu.feed("abcd");
        assert_eq!(wrap(&emu, 4).1, (0, 3));
    }

    #[test]
    fn selections_copy_words_lines_and_spans() {
        let mut emu = Emulator::new();
        emu.feed("cargo test -p ledger\r\nok");
        let point = |row, col, side| GridPoint { row, col, side };
        let sel = |ty, a, b| Selection {
            tab: "t".into(),
            cols: 40,
            ty,
            anchor: a,
            head: b,
        };
        let word = sel(
            SelectionType::Semantic,
            point(0, 7, Side::Left),
            point(0, 7, Side::Left),
        );
        assert_eq!(selection_text(&word, &emu).as_deref(), Some("test"));
        let line = sel(
            SelectionType::Lines,
            point(1, 0, Side::Left),
            point(1, 0, Side::Left),
        );
        assert_eq!(selection_text(&line, &emu).as_deref(), Some("ok"));
        let span = sel(
            SelectionType::Simple,
            point(0, 14, Side::Left),
            point(1, 1, Side::Right),
        );
        assert_eq!(selection_text(&span, &emu).as_deref(), Some("ledger\nok"));
        let click = sel(
            SelectionType::Simple,
            point(0, 3, Side::Left),
            point(0, 3, Side::Left),
        );
        assert_eq!(selection_text(&click, &emu), None);
    }

    #[test]
    fn pointer_maps_to_cells_and_overshoot_clamps() {
        let hit = |x, y| cell_at(x, y, 10.0, 20.0, 8, 4);
        assert_eq!(
            hit(25.0, 45.0),
            CellHit {
                row: 2,
                col: 2,
                side: Side::Left
            }
        );
        assert_eq!(hit(26.0, 0.0).side, Side::Right);
        assert_eq!(hit(9_999.0, 10.0).side, Side::Right);
        assert_eq!(hit(1.0, 9_999.0).row, 3);
        assert_eq!(hit(75.0, -50.0).side, Side::Left);
    }

    #[test]
    fn keystrokes_encode_like_pink() {
        let mods = Modifiers::default();
        assert_eq!(keystroke_bytes("enter", None, &mods), Some(b"\r".to_vec()));
        assert_eq!(keystroke_bytes("backspace", None, &mods), Some(vec![0x7f]));
        assert_eq!(keystroke_bytes("a", Some("A"), &mods), Some(b"A".to_vec()));
        let ctrl = Modifiers {
            control: true,
            ..Modifiers::default()
        };
        assert_eq!(keystroke_bytes("c", Some("c"), &ctrl), Some(vec![0x03]));
        let alt = Modifiers {
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(
            keystroke_bytes("b", Some("b"), &alt),
            Some(vec![0x1b, b'b'])
        );
        let cmd = Modifiers {
            platform: true,
            ..Modifiers::default()
        };
        assert_eq!(keystroke_bytes("j", Some("j"), &cmd), None);
    }

    #[test]
    fn grey_ramp_mirrors_in_light_and_the_cube_does_not() {
        assert_eq!(extended_indexed_rgb(true, 232), (8, 8, 8));
        assert_eq!(extended_indexed_rgb(false, 232), (238, 238, 238));
        assert_eq!(extended_indexed_rgb(true, 196), (255, 0, 0));
        assert_eq!(extended_indexed_rgb(false, 196), (255, 0, 0));
    }

    #[test]
    fn tabs_slide_toward_the_gap() {
        assert_eq!(slide_offset(2, 1, 3), -1.0);
        assert_eq!(slide_offset(0, 2, 0), 1.0);
        assert_eq!(slide_offset(3, 0, 1), 0.0);
        assert_eq!(drop_index(250.0, TAB_WIDTH, 3), 2);
        assert_eq!(drop_index(-5.0, TAB_WIDTH, 3), 0);
    }

    #[test]
    fn sessions_feed_appended_output_and_the_exit_trailer() {
        let mut session = Session::new();
        session.sync(&"one\r\n".into(), None);
        session.sync(&"one\r\ntwo".into(), Some(0));
        assert_eq!(text_of(&session.emu.borrow(), 1), "two");
        assert!(
            session.emu.borrow().lines.iter().any(|line| line
                .iter()
                .map(|c| c.ch)
                .collect::<String>()
                == "[process exited 0]")
        );
    }
}
