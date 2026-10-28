//! Markdown Message – a Pink component.
//!
//! An assistant reply the way Pink's transcript draws it: bare markdown on the
//! canvas, no bubble (Pink: `markdown/render.rs` `render_tree` /
//! `render_block`, parsed the way `markdown/parser.rs` reads CommonMark with
//! tables, strikethrough and task lists). Blocks stack 12px apart; headings
//! step down quickly from 19/27 to body size and take extra room above so they
//! group with what follows. Prose is Geist 14/22 in `text`, strong runs
//! semibold, links monochrome with a `content_muted` underline, inline code in
//! Geist Mono on a rounded `code_wash`. List markers wear the accent: real 5px
//! discs, tinted numbers, or task checkboxes. Quotes get an accent rail and a
//! whisper of the same hue, tables are frameless with hairline rules, and code
//! fences sit on Pink's code surface with a language header, a Fit-content
//! toggle and Copy, colored by Pink's syntax palette.
//!
//! While a reply streams, newly arrived text dissolves in under Pink's opacity
//! veil (`markdown/veil.rs`) and hanging markers (`**bold`, `` `code ``,
//! `[link](…`) are closed in the display so the line never reflows when the
//! closer lands (`markdown/mend.rs`).
//!
//! ```ignore
//! MarkdownMessage::new("reply", streamed_text)
//!     .streaming(turn_is_live)
//!     .on_task_toggle(|index, checked, _, cx| toggle_task_in_source(index, checked, cx))
//!     .on_link(|url, _, cx| cx.open_url(url))
//! ```

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, AnyElement, AnyView, App, AsyncApp, AvailableSpace, BorderStyle, Bounds, ClickEvent,
    ClipboardItem, Context, DispatchPhase, Div, Element, ElementId, Empty, Entity, FocusHandle,
    Font, FontStyle, FontWeight, GlobalElementId, Hsla, InspectorElementId, IntoElement, LayoutId,
    MouseButton, Pixels, Point, Render, Role, ScrollHandle, ScrollWheelEvent, SharedString,
    StrikethroughStyle, StyledText, Task, TextLayout, TextRun, Toggled, UnderlineStyle, WeakEntity,
    Window, anchored, canvas, deferred, div, point, prelude::*, px, quad, size, svg,
    transparent_black,
};

use crate::{
    HoverFill, motion,
    pink::{self, Pink, Syntax, radius},
    story,
};

pub const NAME: &str = "Markdown Message";
pub const DESCRIPTION: &str = "An AI reply rendered from markdown the way Pink's transcript draws it: headings, accent list markers and task checkboxes, quotes, frameless tables and code fences with syntax colors, Fit and Copy, with streamed text dissolving in as it arrives";

// ---------------------------------------------------------------------------
// Metrics (Pink: `markdown/render.rs`)
// ---------------------------------------------------------------------------

/// Gap between blocks of one message (Pink `MD_BLOCK_GAP`).
const MD_BLOCK_GAP: f32 = 12.;
/// Body text (Pink `MD_TEXT_SIZE` / `MD_LINE_HEIGHT`: 14/22).
const MD_TEXT_SIZE: f32 = 14.;
const MD_LINE_HEIGHT: f32 = 22.;
/// Code fences: 12.5/18 Geist Mono (Pink `CODE_TEXT_SIZE` /
/// `CODE_LINE_HEIGHT`).
const CODE_TEXT_SIZE: f32 = pink::text::CODE;
const CODE_LINE_HEIGHT: f32 = 18.;
/// Pink `CODE_PADDING_X` / `CODE_PADDING_Y`.
const CODE_PADDING_X: f32 = 12.;
const CODE_PADDING_Y: f32 = 10.;
/// The fence header strip and its 22px actions (Pink `CODE_HEADER_HEIGHT`,
/// `CODE_ACTION_SIZE`).
const CODE_HEADER_HEIGHT: f32 = 28.;
const CODE_ACTION_SIZE: f32 = 22.;
/// The horizontal scrollbar under a fence (Pink `CODE_SCROLLBAR_HIT_HEIGHT`
/// and `popover::MENU_SCROLLBAR_*`).
const CODE_SCROLLBAR_HIT_HEIGHT: f32 = 10.;
const SCROLLBAR_TRACK_INSET: f32 = 4.;
const SCROLLBAR_THUMB: f32 = 3.;
const SCROLLBAR_THUMB_ACTIVE: f32 = 5.;
const SCROLLBAR_MIN_THUMB: f32 = 24.;
/// How long a fence's Copy reads "Copied" (Pink `copy_ui_for`).
const COPIED_FOR: Duration = Duration::from_millis(1200);
/// Frameless tables (Pink `TABLE_CELL_PADDING`, `TABLE_DIVIDER`,
/// `TABLE_MIN_COLUMN_CONTENT`, `TABLE_MIN_COLUMN_WIDTH`, `TABLE_HEADER_WEIGHT`).
const TABLE_CELL_PADDING: f32 = 12.;
const TABLE_DIVIDER: f32 = 1.;
const TABLE_MIN_COLUMN_CONTENT: f32 = 48.;
const TABLE_MIN_COLUMN_WIDTH: f32 = 96.;
const TABLE_HEADER_WEIGHT: FontWeight = FontWeight::BOLD;
/// The rounded wash under inline code (Pink `INLINE_CODE_RADIUS`,
/// `INLINE_CODE_PAD_X`, `INLINE_CODE_INSET_Y`).
const INLINE_CODE_RADIUS: f32 = 4.5;
const INLINE_CODE_PAD_X: f32 = 2.;
const INLINE_CODE_INSET_Y: f32 = 2.;
/// Lists: items 4 apart, an 18px marker column 8 from the text, 5px discs.
const LIST_GAP: f32 = 4.;
const LIST_MARKER_MIN_W: f32 = 18.;
const LIST_MARKER_GAP: f32 = 8.;
const LIST_DISC: f32 = 5.;
/// The task checkbox: 16px, r4, a 12px check.
const TASK_BOX: f32 = 16.;
const TASK_CHECK: f32 = 12.;
/// Quotes: children 8 apart.
const QUOTE_GAP: f32 = 8.;
/// The link actions menu (Pink `link_interaction.rs`).
const LINK_MENU_WIDTH: f32 = 260.;

/// Streaming veil (Pink `markdown/veil.rs`, mugen's `FadePainter`).
const VEIL_EMA_SEED_MS: f32 = 160.;
const VEIL_MIN_FADE_MS: f32 = 120.;
const VEIL_MAX_FADE_MS: f32 = 400.;
const VEIL_CURVE_POW: f32 = 1.6;
const VEIL_GAP_CLAMP_MS: f32 = 1000.;

/// A link whose destination is still streaming (Pink `mend::PENDING_LINK_URL`):
/// styled like a link, not clickable yet.
const PENDING_LINK: &str = "pink:pending-link";

const COPY: &str = "icons/markdown-message/copy.svg";
const COPY_CHECK: &str = "icons/markdown-message/copy-check.svg";
const CHECK: &str = "icons/markdown-message/check.svg";
const WRAP_TEXT: &str = "icons/markdown-message/wrap-text.svg";
const ARROW_UP_RIGHT: &str = "icons/markdown-message/arrow-right-up.svg";

/// Pink `heading_metrics`: a tight monochrome scale, H1 19/27 down to 14/22.
fn heading_metrics(level: u8) -> (f32, f32) {
    match level {
        1 => (19., 27.),
        2 => (17., 24.),
        3 => (15., 22.),
        _ => (14., 22.),
    }
}

/// Pink `heading_top_extra`: room above a heading on top of the block gap
/// (H1 28, H2 24, H3 20 in all).
fn heading_top_extra(level: u8) -> f32 {
    match level {
        1 => 16.,
        2 => 12.,
        3 => 8.,
        _ => 4.,
    }
}

// ---------------------------------------------------------------------------
// Model (Pink: `markdown/parser.rs` `Block` / `InlineRun`)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    code: bool,
    strike: bool,
    /// The destination while inside a link ([`PENDING_LINK`] while it streams).
    link: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct Run {
    text: String,
    style: InlineStyle,
}

/// GFM column alignment (`:--`, `:-:`, `--:`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Align {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
struct Item {
    /// A task item's state (`- [ ]` / `- [x]`).
    task: Option<bool>,
    blocks: Vec<Block>,
}

#[derive(Clone, Debug, PartialEq)]
enum Block {
    Paragraph(Vec<Run>),
    Heading(u8, Vec<Run>),
    Code {
        language: Option<String>,
        code: String,
    },
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Table {
        header: Vec<Vec<Run>>,
        rows: Vec<Vec<Vec<Run>>>,
        align: Vec<Align>,
    },
    Rule,
}

// ---------------------------------------------------------------------------
// Block parser: the CommonMark + GFM subset Pink's parser enables (tables,
// strikethrough, task lists), line by line.
// ---------------------------------------------------------------------------

/// Parses `source` into blocks. While `streaming`, the last block's hanging
/// inline markers are closed for display (Pink `mend::close_hanging`).
fn parse(source: &str, streaming: bool) -> Vec<Block> {
    let text = source.replace("\r\n", "\n").replace('\t', "    ");
    let lines: Vec<&str> = text.split('\n').collect();
    // Only the last block can be mid-stream, so only it is mended
    parse_blocks(&lines, streaming)
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// A fence opener: `(marker char, run length, indent, info string)`.
fn fence_open(line: &str) -> Option<(char, usize, usize, &str)> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let n = rest.chars().take_while(|c| *c == ch).count();
    if n < 3 {
        return None;
    }
    let info = rest[n..].trim();
    if ch == '`' && info.contains('`') {
        return None;
    }
    Some((ch, n, indent, info))
}

fn fence_close(line: &str, ch: char, n: usize) -> bool {
    let indent = indent_of(line);
    if indent > 3 {
        return false;
    }
    let rest = line[indent..].trim_end();
    rest.len() >= n && rest.chars().all(|c| c == ch)
}

fn atx_heading(line: &str) -> Option<(u8, &str)> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let level = rest.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let after = &rest[level..];
    if !after.is_empty() && !after.starts_with(' ') {
        return None;
    }
    let mut content = after.trim();
    // A closing run of #s preceded by a space goes away.
    let trimmed = content.trim_end_matches('#');
    if trimmed.len() < content.len() && (trimmed.is_empty() || trimmed.ends_with(' ')) {
        content = trimmed.trim_end();
    }
    Some((level as u8, content))
}

fn is_rule(line: &str) -> bool {
    if indent_of(line) > 3 {
        return false;
    }
    let chars: Vec<char> = line.chars().filter(|c| !c.is_whitespace()).collect();
    chars.len() >= 3 && matches!(chars[0], '-' | '*' | '_') && chars.iter().all(|c| *c == chars[0])
}

fn setext_level(line: &str) -> Option<u8> {
    if indent_of(line) > 3 {
        return None;
    }
    let t = line.trim();
    if !t.is_empty() && t.chars().all(|c| c == '=') {
        Some(1)
    } else if !t.is_empty() && t.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

fn quote_content(line: &str) -> Option<&str> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let rest = line[indent..].strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// A list item marker.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Marker {
    /// The start number of an ordered item.
    ordered: Option<u64>,
    /// `-`, `*`, `+`, or the ordered delimiter `.` / `)`.
    delim: char,
    /// Where the item's content starts.
    content: usize,
    /// Nothing follows the marker.
    empty: bool,
}

fn list_marker(line: &str) -> Option<Marker> {
    let indent = indent_of(line);
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let (ordered, delim, len) = if let Some(c) = rest.chars().next()
        && matches!(c, '-' | '*' | '+')
    {
        (None, c, 1)
    } else {
        let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 || digits > 9 {
            return None;
        }
        let delim = rest[digits..].chars().next()?;
        if delim != '.' && delim != ')' {
            return None;
        }
        (rest[..digits].parse().ok(), delim, digits + 1)
    };
    let after = &rest[len..];
    if after.trim().is_empty() {
        return Some(Marker {
            ordered,
            delim,
            content: indent + len + 1,
            empty: true,
        });
    }
    let spaces = indent_of(after);
    if spaces == 0 {
        return None;
    }
    let spaces = if spaces > 4 { 1 } else { spaces };
    Some(Marker {
        ordered,
        delim,
        content: indent + len + spaces,
        empty: false,
    })
}

/// Splits a table row on unescaped pipes outside code spans.
fn split_row(line: &str) -> Vec<String> {
    let mut t = line.trim();
    if let Some(s) = t.strip_prefix('|') {
        t = s;
    }
    if t.ends_with('|') && !t.ends_with("\\|") {
        t = &t[..t.len() - 1];
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut in_code = false;
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '`' => {
                in_code = !in_code;
                cur.push(c);
            }
            '|' if !in_code => cells.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

fn table_delimiter(line: &str) -> Option<Vec<Align>> {
    if !line.contains('-') {
        return None;
    }
    let cells = split_row(line);
    let mut align = Vec::with_capacity(cells.len());
    for cell in cells {
        let c = cell.trim();
        let left = c.starts_with(':');
        let right = c.ends_with(':');
        let dashes = c.trim_matches(':');
        if dashes.is_empty() || !dashes.chars().all(|ch| ch == '-') {
            return None;
        }
        align.push(match (left, right) {
            (true, true) => Align::Center,
            (false, true) => Align::Right,
            _ => Align::Left,
        });
    }
    Some(align)
}

/// Does `line` open a block that interrupts a paragraph?
fn interrupts_paragraph(line: &str) -> bool {
    fence_open(line).is_some()
        || atx_heading(line).is_some()
        || quote_content(line).is_some()
        || is_rule(line)
        || list_marker(line).is_some_and(|m| !m.empty && m.ordered.is_none_or(|n| n == 1))
}

fn parse_blocks(lines: &[&str], mend: bool) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut i = 0;
    let n = lines.len();
    while i < n {
        let line = lines[i];
        if is_blank(line) {
            i += 1;
            continue;
        }
        let last = |end: usize| mend && lines[end..].iter().all(|l| is_blank(l));
        // Fenced code (an unclosed fence runs to the end, as in CommonMark)
        if let Some((ch, len, indent, info)) = fence_open(line) {
            let language = info.split_whitespace().next().map(str::to_string);
            let mut code = Vec::new();
            i += 1;
            while i < n && !fence_close(lines[i], ch, len) {
                let l = lines[i];
                let strip = indent_of(l).min(indent);
                code.push(&l[strip..]);
                i += 1;
            }
            i += 1;
            blocks.push(Block::Code {
                language,
                code: code.join("\n"),
            });
            continue;
        }
        if let Some((level, content)) = atx_heading(line) {
            i += 1;
            blocks.push(Block::Heading(level, parse_inline(content, last(i))));
            continue;
        }
        if is_rule(line) {
            i += 1;
            blocks.push(Block::Rule);
            continue;
        }
        if quote_content(line).is_some() {
            let mut inner = Vec::new();
            while i < n {
                if let Some(content) = quote_content(lines[i]) {
                    inner.push(content);
                } else if !is_blank(lines[i])
                    && !interrupts_paragraph(lines[i])
                    && inner.last().is_some_and(|l: &&str| !is_blank(l))
                {
                    // A lazy continuation line of the quoted paragraph
                    inner.push(lines[i].trim_start());
                } else {
                    break;
                }
                i += 1;
            }
            blocks.push(Block::Quote(parse_blocks(&inner, last(i))));
            continue;
        }
        if let Some(first) = list_marker(line) {
            let (list, next) = parse_list(lines, i, first, mend);
            i = next;
            blocks.push(list);
            continue;
        }
        // Indented code
        if indent_of(line) >= 4 {
            let mut code = Vec::new();
            while i < n && (indent_of(lines[i]) >= 4 || is_blank(lines[i])) {
                code.push(lines[i].get(4..).unwrap_or(""));
                i += 1;
            }
            while code.last().is_some_and(|l| l.trim().is_empty()) {
                code.pop();
            }
            blocks.push(Block::Code {
                language: None,
                code: code.join("\n"),
            });
            continue;
        }
        // A table: a header row, then the delimiter row
        if line.contains('|')
            && let Some(align) = lines.get(i + 1).and_then(|l| table_delimiter(l))
            && split_row(line).len() == align.len()
        {
            let header = split_row(line);
            i += 2;
            let mut rows = Vec::new();
            while i < n
                && !is_blank(lines[i])
                && !interrupts_paragraph(lines[i])
                && lines[i].contains('|')
            {
                rows.push(split_row(lines[i]));
                i += 1;
            }
            let mend_cells = last(i);
            let cell = |c: &String| parse_inline(c, mend_cells);
            blocks.push(Block::Table {
                header: header.iter().map(cell).collect(),
                rows: rows
                    .iter()
                    .map(|row| {
                        let mut cells: Vec<Vec<Run>> = row.iter().map(cell).collect();
                        cells.truncate(align.len());
                        cells
                    })
                    .collect(),
                align,
            });
            continue;
        }
        // A paragraph, up to a blank line or a block that interrupts it
        let mut para = vec![line.trim_start()];
        i += 1;
        let mut heading = None;
        while i < n && !is_blank(lines[i]) {
            if let Some(level) = setext_level(lines[i]) {
                heading = Some(level);
                i += 1;
                break;
            }
            if interrupts_paragraph(lines[i]) {
                break;
            }
            para.push(lines[i].trim_start());
            i += 1;
        }
        let text = para.join("\n");
        let runs = parse_inline(&text, last(i));
        blocks.push(match heading {
            Some(level) => Block::Heading(level, runs),
            None => Block::Paragraph(runs),
        });
    }
    blocks
}

/// One list from `start`: its items, each parsed as a nested document.
fn parse_list(lines: &[&str], start: usize, first: Marker, mend: bool) -> (Block, usize) {
    let n = lines.len();
    let same_kind =
        |m: &Marker| m.ordered.is_some() == first.ordered.is_some() && m.delim == first.delim;
    let mut items = Vec::new();
    let mut i = start;
    while i < n {
        let Some(marker) = list_marker(lines[i]).filter(same_kind) else {
            break;
        };
        let offset = marker.content;
        let mut item_lines: Vec<&str> = vec![if marker.empty {
            ""
        } else {
            &lines[i][offset.min(lines[i].len())..]
        }];
        i += 1;
        while i < n {
            let l = lines[i];
            if is_blank(l) {
                // A blank line continues the item only if indented content follows
                let next = lines[i..].iter().position(|l| !is_blank(l)).map(|p| i + p);
                match next {
                    Some(j) if indent_of(lines[j]) >= offset => {
                        item_lines.extend(std::iter::repeat_n("", j - i));
                        i = j;
                        continue;
                    }
                    _ => break,
                }
            }
            if indent_of(l) >= offset {
                item_lines.push(&l[offset..]);
            } else if list_marker(l).is_none()
                && !interrupts_paragraph(l)
                && item_lines.last().is_some_and(|last| !is_blank(last))
            {
                // A lazy continuation of the item's paragraph
                item_lines.push(l.trim_start());
            } else {
                break;
            }
            i += 1;
        }
        // GFM task marker
        let mut task = None;
        if let Some(first_line) = item_lines.first_mut() {
            for (prefix, checked) in [("[ ] ", false), ("[x] ", true), ("[X] ", true)] {
                if let Some(rest) = first_line.strip_prefix(prefix) {
                    task = Some(checked);
                    *first_line = rest;
                    break;
                }
            }
        }
        let tail = mend && lines[i..].iter().all(|l| is_blank(l));
        items.push(Item {
            task,
            blocks: parse_blocks(&item_lines, tail),
        });
        // Blank lines between sibling items
        let next = lines[i..].iter().position(|l| !is_blank(l)).map(|p| i + p);
        match next {
            Some(j) if list_marker(lines[j]).is_some_and(|m| same_kind(&m)) => i = j,
            _ => break,
        }
    }
    (
        Block::List {
            start: first.ordered,
            items,
        },
        i,
    )
}

// ---------------------------------------------------------------------------
// Inline parser (Pink: pulldown-cmark's inline events + `mend.rs` while
// streaming + `autolink_runs`)
// ---------------------------------------------------------------------------

fn parse_inline(text: &str, mend: bool) -> Vec<Run> {
    let mut out = Vec::new();
    inline(text, &InlineStyle::default(), mend, &mut out);
    autolink_runs(out)
}

fn push_run(out: &mut Vec<Run>, text: &str, style: &InlineStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = out.last_mut()
        && last.style == *style
    {
        last.text.push_str(text);
        return;
    }
    out.push(Run {
        text: text.to_string(),
        style: style.clone(),
    });
}

fn flush(buf: &mut String, style: &InlineStyle, out: &mut Vec<Run>) {
    push_run(out, buf, style);
    buf.clear();
}

fn run_len(b: &[u8], i: usize, c: u8) -> usize {
    b[i..].iter().take_while(|x| **x == c).count()
}

/// The start of the closing backtick run of exactly `n` ticks.
fn find_backticks(s: &str, from: usize, n: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut j = from;
    while j < b.len() {
        if b[j] == b'`' {
            let m = run_len(b, j, b'`');
            if m == n {
                return Some(j);
            }
            j += m;
        } else {
            j += 1;
        }
    }
    None
}

/// CommonMark code span content: newlines read as spaces, and one space on
/// each side goes when both are there.
fn code_span(content: &str) -> String {
    let c = content.replace('\n', " ");
    if c.len() >= 2 && c.starts_with(' ') && c.ends_with(' ') && !c.trim().is_empty() {
        c[1..c.len() - 1].to_string()
    } else {
        c
    }
}

/// The closer for an emphasis delimiter of length `k` of `c`, searching from
/// `from`: a right-flanking run of the same length (or a longer combined run,
/// closing at its end). Code spans and escapes are skipped.
fn find_closer(s: &str, from: usize, c: u8, k: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut j = from;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'`' => {
                let m = run_len(b, j, b'`');
                j = find_backticks(s, j + m, m).map_or(j + m, |close| close + m);
            }
            x if x == c => {
                let m = run_len(b, j, c);
                let prev = s[..j].chars().next_back();
                let after = s[j + m..].chars().next();
                let right_flanking = j > from && prev.is_some_and(|p| !p.is_whitespace());
                let underscore_ok = c != b'_' || !after.is_some_and(|a| a.is_alphanumeric());
                if right_flanking && underscore_ok && (m == k || (m == 3 && k < 3)) {
                    return Some(j + m - k);
                }
                j += m;
            }
            _ => j += 1,
        }
    }
    None
}

/// The matching `]` for a `[` at `open`.
fn find_bracket(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    let mut j = open;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 1,
            b'`' => {
                let m = run_len(b, j, b'`');
                j = find_backticks(s, j + m, m).map_or(j + m, |close| close + m) - 1;
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// The matching `)` for a `(` at `open`.
fn find_paren(s: &str, open: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut depth = 0usize;
    let mut j = open;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 1,
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            b' ' | b'\n' if depth == 1 && !s[j..].trim_start().starts_with(['"', '\'', '(']) => {
                // Only a title may follow a space inside the destination
                if !s[j..].trim_start().starts_with(')') {
                    return None;
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// The URL of a link destination `(url "title")`.
fn link_destination(inner: &str) -> String {
    let t = inner.trim();
    if let Some(rest) = t.strip_prefix('<')
        && let Some(end) = rest.find('>')
    {
        return rest[..end].to_string();
    }
    t.split_whitespace().next().unwrap_or("").to_string()
}

fn entity(s: &str) -> Option<(&'static str, usize)> {
    for (name, ch) in [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
        ("&nbsp;", "\u{a0}"),
        // joiners: allow HTML entities decoded into the message's own text
        ("&mdash;", "—"),
        // joiners: allow HTML entities decoded into the message's own text
        ("&ndash;", "–"),
        ("&hellip;", "…"),
    ] {
        if s.starts_with(name) {
            return Some((ch, name.len()));
        }
    }
    None
}

fn inline(s: &str, style: &InlineStyle, mend: bool, out: &mut Vec<Run>) {
    let b = s.as_bytes();
    let mut buf = String::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\\' => match b.get(i + 1) {
                Some(b'\n') => {
                    buf.push('\n');
                    i += 2;
                }
                Some(n) if n.is_ascii_punctuation() => {
                    buf.push(*n as char);
                    i += 2;
                }
                _ => {
                    buf.push('\\');
                    i += 1;
                }
            },
            b'\n' => {
                // Two trailing spaces make a hard break; else a soft one
                let hard = buf.ends_with("  ");
                let trimmed = buf.trim_end_matches(' ').len();
                buf.truncate(trimmed);
                buf.push(if hard { '\n' } else { ' ' });
                i += 1;
            }
            b'`' if !style.code => {
                let n = run_len(b, i, b'`');
                if let Some(close) = find_backticks(s, i + n, n) {
                    flush(&mut buf, style, out);
                    let mut code = style.clone();
                    code.code = true;
                    push_run(out, &code_span(&s[i + n..close]), &code);
                    i = close + n;
                } else if mend && !s[i + n..].trim().is_empty() {
                    flush(&mut buf, style, out);
                    let mut code = style.clone();
                    code.code = true;
                    push_run(out, &s[i + n..].replace('\n', " "), &code);
                    return;
                } else {
                    buf.push_str(&s[i..i + n]);
                    i += n;
                }
            }
            c @ (b'*' | b'_' | b'~') => {
                let n = run_len(b, i, c);
                let next = s[i + n..].chars().next();
                let prev = s[..i].chars().next_back();
                let left_flanking = next.is_some_and(|ch| !ch.is_whitespace());
                let intraword = c == b'_' && prev.is_some_and(|p| p.is_alphanumeric());
                let usable = left_flanking && !intraword && (c != b'~' || n == 2);
                let mut handled = false;
                if usable {
                    let ks: &[usize] = if c == b'~' {
                        &[2]
                    } else {
                        match n {
                            1 => &[1],
                            2 => &[2, 1],
                            _ => &[3, 2, 1],
                        }
                    };
                    for &k in ks {
                        if let Some(close) = find_closer(s, i + k, c, k) {
                            flush(&mut buf, style, out);
                            let inner_style = emphasis(style, c, k);
                            inline(&s[i + k..close], &inner_style, false, out);
                            i = close + k;
                            handled = true;
                            break;
                        }
                    }
                    if !handled && mend && !s[i + n..].trim().is_empty() {
                        // Pink `mend`: a hanging opener styles what follows
                        flush(&mut buf, style, out);
                        let k = if c == b'~' { 2 } else { n.min(3) };
                        inline(&s[i + k..], &emphasis(style, c, k), true, out);
                        return;
                    }
                }
                if !handled {
                    buf.push_str(&s[i..i + n]);
                    i += n;
                }
            }
            b'!' if b.get(i + 1) == Some(&b'[') => {
                // An image: its alt text, linked to the source (Pink's parser)
                if let Some(close) = find_bracket(s, i + 1)
                    && s[close + 1..].starts_with('(')
                    && let Some(paren) = find_paren(s, close + 1)
                {
                    flush(&mut buf, style, out);
                    let mut linked = style.clone();
                    linked.link = Some(link_destination(&s[close + 2..paren]));
                    inline(&s[i + 2..close], &linked, false, out);
                    i = paren + 1;
                } else {
                    buf.push('!');
                    i += 1;
                }
            }
            b'[' if style.link.is_none() => {
                let close = find_bracket(s, i);
                match close {
                    Some(close) if s[close + 1..].starts_with('(') => {
                        if let Some(paren) = find_paren(s, close + 1) {
                            flush(&mut buf, style, out);
                            let mut linked = style.clone();
                            linked.link = Some(link_destination(&s[close + 2..paren]));
                            inline(&s[i + 1..close], &linked, false, out);
                            i = paren + 1;
                        } else if mend {
                            // `[text](partial`: the link text shows styled now
                            flush(&mut buf, style, out);
                            let mut linked = style.clone();
                            linked.link = Some(PENDING_LINK.into());
                            inline(&s[i + 1..close], &linked, false, out);
                            return;
                        } else {
                            buf.push('[');
                            i += 1;
                        }
                    }
                    None if mend && !s[i + 1..].trim().is_empty() => {
                        flush(&mut buf, style, out);
                        let mut linked = style.clone();
                        linked.link = Some(PENDING_LINK.into());
                        inline(&s[i + 1..], &linked, true, out);
                        return;
                    }
                    _ => {
                        buf.push('[');
                        i += 1;
                    }
                }
            }
            b'<' => {
                // An autolink, `<https://…>`
                let end = s[i + 1..].find('>').map(|e| i + 1 + e);
                match end {
                    Some(end) if s[i + 1..end].contains("://") && !s[i + 1..end].contains(' ') => {
                        flush(&mut buf, style, out);
                        let mut linked = style.clone();
                        linked.link = Some(s[i + 1..end].to_string());
                        push_run(out, &s[i + 1..end], &linked);
                        i = end + 1;
                    }
                    _ => {
                        buf.push('<');
                        i += 1;
                    }
                }
            }
            b'&' => match entity(&s[i..]) {
                Some((ch, len)) => {
                    buf.push_str(ch);
                    i += len;
                }
                None => {
                    buf.push('&');
                    i += 1;
                }
            },
            _ => {
                let ch = s[i..].chars().next().unwrap_or(' ');
                buf.push(ch);
                i += ch.len_utf8();
            }
        }
    }
    flush(&mut buf, style, out);
}

fn emphasis(style: &InlineStyle, c: u8, k: usize) -> InlineStyle {
    let mut s = style.clone();
    if c == b'~' {
        s.strike = true;
    } else {
        s.italic |= k != 2;
        s.bold |= k >= 2;
    }
    s
}

/// Pink `autolink_runs`: bare `http(s)://` URLs become links, minus the
/// trailing punctuation GFM leaves out.
fn autolink_runs(runs: Vec<Run>) -> Vec<Run> {
    let mut out = Vec::with_capacity(runs.len());
    for run in runs {
        if run.style.link.is_some() || run.style.code {
            out.push(run);
            continue;
        }
        let mut rest = run.text.as_str();
        while let Some(at) = find_url_start(rest) {
            let from = &rest[at..];
            let scheme = if from.starts_with("https://") { 8 } else { 7 };
            let len = bare_url_len(from);
            if len <= scheme {
                push_run(&mut out, &rest[..at + scheme], &run.style);
                rest = &from[scheme..];
                continue;
            }
            push_run(&mut out, &rest[..at], &run.style);
            let mut linked = run.style.clone();
            linked.link = Some(from[..len].to_string());
            push_run(&mut out, &from[..len], &linked);
            rest = &from[len..];
        }
        push_run(&mut out, rest, &run.style);
    }
    out
}

fn find_url_start(text: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = text[from..].find("http") {
        let at = from + rel;
        let after = &text[at..];
        let is_scheme = after.starts_with("http://") || after.starts_with("https://");
        let boundary = text[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_alphanumeric());
        if is_scheme && boundary {
            return Some(at);
        }
        from = at + 4;
    }
    None
}

fn bare_url_len(text: &str) -> usize {
    let end = text
        .char_indices()
        .find(|(_, c)| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '`'))
        .map_or(text.len(), |(i, _)| i);
    let mut url = &text[..end];
    while let Some(last) = url.chars().next_back() {
        let trim = match last {
            '.' | ',' | ';' | ':' | '!' | '?' | '*' | '_' | '~' => true,
            ')' => url.matches('(').count() < url.matches(')').count(),
            _ => false,
        };
        if !trim {
            break;
        }
        url = &url[..url.len() - last.len_utf8()];
    }
    url.len()
}

/// Rewrites the `index`th task marker (in document order) of `source` to
/// `checked`, the way Pink's markdown preview toggles a task in its editor.
/// Returns the source unchanged when there is no such task.
pub fn toggle_task(source: &str, index: usize, checked: bool) -> String {
    let mut seen = 0;
    let mut fence: Option<(char, usize)> = None;
    let mut out = String::with_capacity(source.len());
    for (ix, line) in source.split('\n').enumerate() {
        if ix > 0 {
            out.push('\n');
        }
        if let Some((ch, n)) = fence {
            if fence_close(line, ch, n) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if let Some((ch, n, _, _)) = fence_open(line) {
            fence = Some((ch, n));
            out.push_str(line);
            continue;
        }
        // A task marker follows a list marker, possibly behind quote markers
        let mut prefix_len = 0;
        let mut rest = line;
        while let Some(inner) = quote_content(rest) {
            prefix_len += rest.len() - inner.len();
            rest = inner;
        }
        let task_at = list_marker(rest).filter(|m| !m.empty).and_then(|m| {
            let body = &rest[m.content..];
            ["[ ] ", "[x] ", "[X] "]
                .iter()
                .any(|p| body.starts_with(p))
                .then_some(prefix_len + m.content)
        });
        match task_at {
            Some(at) => {
                if seen == index {
                    out.push_str(&line[..at]);
                    out.push_str(if checked { "[x]" } else { "[ ]" });
                    out.push_str(&line[at + 3..]);
                } else {
                    out.push_str(line);
                }
                seen += 1;
            }
            None => out.push_str(line),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Syntax colors (Pink: tree-sitter highlights painted with `Theme::syntax`;
// here a small per-language lexer emitting the same token classes)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tok {
    Comment,
    Keyword,
    String,
    Escape,
    Number,
    Type,
    Function,
    Macro,
    Property,
    Attribute,
    Operator,
    Punctuation,
    Variable,
}

/// Pink `token_color` through the kit's grouped syntax palette.
fn tok_color(tok: Tok, syn: &Syntax) -> Hsla {
    match tok {
        Tok::Comment => syn.comment,
        Tok::Keyword | Tok::Macro | Tok::Operator => syn.keyword,
        Tok::String => syn.string,
        Tok::Escape | Tok::Attribute => syn.string_special,
        Tok::Number => syn.number,
        Tok::Type => syn.type_name,
        Tok::Function => syn.function,
        Tok::Property => syn.property,
        Tok::Punctuation => syn.punctuation,
        Tok::Variable => syn.variable,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lang {
    Rust,
    Script,
    Python,
    Shell,
    Json,
    Toml,
    Plain,
}

impl Lang {
    fn of(info: Option<&str>) -> Self {
        match info.map(str::to_ascii_lowercase).as_deref() {
            Some("rust" | "rs") => Lang::Rust,
            Some(
                "ts" | "tsx" | "typescript" | "js" | "jsx" | "javascript" | "mjs" | "go" | "swift"
                | "java" | "kotlin" | "c" | "cpp" | "c++" | "cs",
            ) => Lang::Script,
            Some("py" | "python") => Lang::Python,
            Some("sh" | "bash" | "zsh" | "shell" | "console" | "fish") => Lang::Shell,
            Some("json" | "jsonc") => Lang::Json,
            Some("toml" | "yaml" | "yml" | "ini") => Lang::Toml,
            _ => Lang::Plain,
        }
    }

    fn keywords(self) -> &'static [&'static str] {
        match self {
            Lang::Rust => &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
                "enum", "extern", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
                "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super",
                "trait", "type", "unsafe", "use", "where", "while",
            ],
            Lang::Script => &[
                "async",
                "await",
                "break",
                "case",
                "catch",
                "class",
                "const",
                "continue",
                "default",
                "delete",
                "do",
                "else",
                "export",
                "extends",
                "finally",
                "for",
                "from",
                "function",
                "if",
                "import",
                "in",
                "instanceof",
                "interface",
                "let",
                "new",
                "of",
                "return",
                "static",
                "switch",
                "this",
                "throw",
                "try",
                "type",
                "typeof",
                "var",
                "void",
                "while",
                "yield",
                "func",
                "package",
                "struct",
                "go",
                "defer",
                "fn",
            ],
            Lang::Python => &[
                "and", "as", "assert", "async", "await", "break", "class", "continue", "def",
                "del", "elif", "else", "except", "finally", "for", "from", "global", "if",
                "import", "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return",
                "try", "while", "with", "yield", "self",
            ],
            Lang::Shell => &[
                "if", "then", "else", "elif", "fi", "for", "in", "do", "done", "while", "case",
                "esac", "function", "export", "local", "return",
            ],
            Lang::Json | Lang::Toml | Lang::Plain => &[],
        }
    }

    fn constants(self) -> &'static [&'static str] {
        &[
            "true",
            "false",
            "null",
            "None",
            "True",
            "False",
            "undefined",
            "nil",
        ]
    }

    fn line_comment(self) -> &'static [&'static str] {
        match self {
            Lang::Rust | Lang::Script | Lang::Json => &["//"],
            Lang::Python | Lang::Shell | Lang::Toml => &["#"],
            Lang::Plain => &[],
        }
    }

    fn block_comment(self) -> bool {
        matches!(self, Lang::Rust | Lang::Script)
    }
}

/// The length of a shell variable name after its `$` (`HOME`, `{HOME}`).
fn shell_var_len(s: &str) -> usize {
    s.bytes()
        .take_while(|x| x.is_ascii_alphanumeric() || matches!(*x, b'_' | b'{' | b'}'))
        .count()
}

/// Token spans for one line; `in_comment` carries a block comment across lines.
fn lex_line(lang: Lang, line: &str, in_comment: &mut bool) -> Vec<(Range<usize>, Tok)> {
    let mut spans = Vec::new();
    if lang == Lang::Plain {
        return spans;
    }
    let b = line.as_bytes();
    let mut i = 0;
    let mut prev_word: Option<&str> = None;
    let mut command_pos = true;
    fn push(spans: &mut Vec<(Range<usize>, Tok)>, r: Range<usize>, t: Tok) {
        if !r.is_empty() {
            spans.push((r, t));
        }
    }
    if *in_comment {
        match line.find("*/") {
            Some(end) => {
                push(&mut spans, 0..end + 2, Tok::Comment);
                i = end + 2;
                *in_comment = false;
            }
            None => {
                push(&mut spans, 0..line.len(), Tok::Comment);
                return spans;
            }
        }
    }
    while i < b.len() {
        let c = b[i];
        let rest = &line[i..];
        if c == b' ' {
            i += 1;
            continue;
        }
        if lang.line_comment().iter().any(|p| rest.starts_with(p))
            && !(lang == Lang::Shell && i > 0 && b[i - 1] != b' ')
        {
            push(&mut spans, i..line.len(), Tok::Comment);
            break;
        }
        if lang.block_comment() && rest.starts_with("/*") {
            match rest[2..].find("*/") {
                Some(end) => {
                    push(&mut spans, i..i + 2 + end + 2, Tok::Comment);
                    i += end + 4;
                }
                None => {
                    push(&mut spans, i..line.len(), Tok::Comment);
                    *in_comment = true;
                    break;
                }
            }
            continue;
        }
        if lang == Lang::Rust && (rest.starts_with("#[") || rest.starts_with("#![")) {
            let end = rest.find(']').map_or(line.len(), |e| i + e + 1);
            push(&mut spans, i..end, Tok::Attribute);
            i = end;
            continue;
        }
        if lang == Lang::Rust && c == b'\'' {
            // A lifetime or a char literal
            let lifetime = rest.len() > 2
                && (rest.as_bytes()[1].is_ascii_alphabetic() || rest.as_bytes()[1] == b'_')
                && rest.as_bytes().get(2) != Some(&b'\'');
            if lifetime {
                let end = i
                    + 1
                    + rest[1..]
                        .bytes()
                        .take_while(|x| x.is_ascii_alphanumeric() || *x == b'_')
                        .count();
                push(&mut spans, i..end, Tok::Function);
                i = end;
                continue;
            }
        }
        if c == b'"' || c == b'\'' || (c == b'`' && lang == Lang::Script) {
            let start = i;
            let mut j = i + 1;
            // The current plain-string segment, opening quote included
            let mut seg = i;
            while j < b.len() && b[j] != c {
                if b[j] == b'\\' && j + 1 < b.len() {
                    push(&mut spans, seg..j, Tok::String);
                    let esc_end = (j + 1 + line[j + 1..].chars().next().map_or(1, char::len_utf8))
                        .min(b.len());
                    push(&mut spans, j..esc_end, Tok::Escape);
                    j = esc_end;
                    seg = j;
                    continue;
                }
                if lang == Lang::Shell && b[j] == b'$' {
                    push(&mut spans, seg..j, Tok::String);
                    let end = j + 1 + shell_var_len(&line[j + 1..]);
                    push(&mut spans, j..end, Tok::Keyword);
                    j = end;
                    seg = j;
                    continue;
                }
                j += 1;
            }
            let end = (j + 1).min(b.len());
            push(&mut spans, seg..end, Tok::String);
            // JSON / TOML keys are properties
            let after = line[end..].trim_start();
            if matches!(lang, Lang::Json | Lang::Toml)
                && (after.starts_with(':') || after.starts_with('='))
            {
                for span in spans
                    .iter_mut()
                    .filter(|(r, _)| r.start >= start && r.end <= end)
                {
                    span.1 = Tok::Property;
                }
            }
            i = end;
            command_pos = false;
            continue;
        }
        if c.is_ascii_digit() {
            let end = i + rest
                .bytes()
                .take_while(|x| x.is_ascii_alphanumeric() || *x == b'.' || *x == b'_')
                .count();
            push(&mut spans, i..end, Tok::Number);
            i = end;
            command_pos = false;
            continue;
        }
        if lang == Lang::Shell && c == b'$' {
            let end = i + 1 + shell_var_len(&rest[1..]);
            push(&mut spans, i..end, Tok::Keyword);
            i = end;
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' || (lang == Lang::Shell && c == b'-' && command_pos)
        {
            let len = rest
                .bytes()
                .take_while(|x| {
                    x.is_ascii_alphanumeric()
                        || *x == b'_'
                        || (lang == Lang::Shell && matches!(*x, b'-' | b'.' | b'/'))
                })
                .count();
            let end = i + len;
            let word = &line[i..end];
            let after = line[end..].trim_start();
            let before = line[..i].trim_end();
            let tok = if lang == Lang::Rust
                && line[end..].starts_with('!')
                && !line[end..].starts_with("!=")
            {
                push(&mut spans, i..end + 1, Tok::Macro);
                i = end + 1;
                prev_word = Some(word);
                continue;
            } else if lang.keywords().contains(&word) {
                Tok::Keyword
            } else if lang.constants().contains(&word) {
                Tok::Number
            } else if matches!(prev_word, Some("fn" | "def" | "function" | "func")) {
                Tok::Function
            } else if lang == Lang::Shell {
                if command_pos {
                    Tok::Function
                } else {
                    Tok::Variable
                }
            } else if lang == Lang::Toml {
                if before.is_empty() && (after.starts_with('=') || after.starts_with('.')) {
                    Tok::Property
                } else {
                    Tok::Variable
                }
            } else if after.starts_with('(') || (lang == Lang::Rust && after.starts_with("::<")) {
                Tok::Function
            } else if before.ends_with('.') {
                Tok::Property
            } else if word.len() > 1
                && word
                    .bytes()
                    .all(|x| x.is_ascii_uppercase() || x == b'_' || x.is_ascii_digit())
            {
                Tok::Number
            } else if word.as_bytes()[0].is_ascii_uppercase() {
                Tok::Type
            } else {
                Tok::Variable
            };
            push(&mut spans, i..end, tok);
            prev_word = Some(word);
            command_pos = false;
            i = end;
            continue;
        }
        let ch_len = rest.chars().next().map_or(1, char::len_utf8);
        let tok = match c {
            b'(' | b')' | b'{' | b'}' | b'[' | b']' | b';' | b',' | b'.' => Tok::Punctuation,
            b':' => {
                if lang == Lang::Rust {
                    Tok::Punctuation
                } else {
                    Tok::Operator
                }
            }
            b'=' | b'+' | b'-' | b'*' | b'/' | b'%' | b'<' | b'>' | b'!' | b'&' | b'|' | b'^'
            | b'?' => Tok::Operator,
            _ => Tok::Variable,
        };
        if lang == Lang::Shell && matches!(c, b'|' | b';' | b'&') {
            command_pos = true;
        }
        if tok != Tok::Variable {
            push(&mut spans, i..i + ch_len, tok);
        }
        i += ch_len;
    }
    spans.sort_by_key(|(r, _)| r.start);
    // Merge overlapping spans: keep the first claim of each byte
    let mut merged: Vec<(Range<usize>, Tok)> = Vec::with_capacity(spans.len());
    let mut at = 0;
    for (r, t) in spans {
        let start = r.start.max(at);
        if start < r.end {
            merged.push((start..r.end, t));
            at = r.end;
        }
    }
    merged
}

/// Pink `runs_for_syntax_line`: one exact-cover run list in the mono font,
/// recolored per token; layout never depends on the colors.
fn syntax_runs(
    line: &str,
    spans: &[(Range<usize>, Tok)],
    plain: Hsla,
    syn: &Syntax,
) -> Vec<TextRun> {
    let mono = gpui::font(pink::MONO);
    let run = |len: usize, color: Hsla| TextRun {
        len,
        font: mono.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let mut runs = Vec::new();
    let mut at = 0;
    for (r, tok) in spans {
        if r.start > at {
            runs.push(run(r.start - at, plain));
        }
        let color = if *tok == Tok::Variable {
            plain
        } else {
            tok_color(*tok, syn)
        };
        runs.push(run(r.len(), color));
        at = r.end;
    }
    if at < line.len() {
        runs.push(run(line.len() - at, plain));
    }
    runs.retain(|r| r.len > 0);
    runs
}

// ---------------------------------------------------------------------------
// Streaming veil (Pink: `markdown/veil.rs`)
// ---------------------------------------------------------------------------

type VeilSpan = (Range<usize>, f32);

/// Pink `veil_opacity`: the veil dissolves as `(1 − p)^1.6`.
fn veil_opacity(p: f32) -> f32 {
    1. - (1. - p.clamp(0., 1.)).powf(VEIL_CURVE_POW)
}

struct Chunk {
    range: Range<usize>,
    started: Instant,
    duration_ms: f32,
}

/// Pink `ElemVeil`: fades every newly appended suffix of one element exactly
/// once, over `clamp(ema × 3, 120, 400)` ms of the stream's cadence.
struct ElemVeil {
    prev: String,
    chunks: Vec<Chunk>,
    ema_ms: f32,
    last_append: Option<Instant>,
}

impl Default for ElemVeil {
    fn default() -> Self {
        Self {
            prev: String::new(),
            chunks: Vec::new(),
            ema_ms: VEIL_EMA_SEED_MS,
            last_append: None,
        }
    }
}

fn common_prefix(a: &str, b: &str) -> usize {
    let mut p = a
        .as_bytes()
        .iter()
        .zip(b.as_bytes())
        .take_while(|(x, y)| x == y)
        .count();
    while p > 0 && !b.is_char_boundary(p) {
        p -= 1;
    }
    p
}

impl ElemVeil {
    fn advance(&mut self, text: &str, now: Instant) -> Vec<VeilSpan> {
        if text != self.prev {
            let p = common_prefix(&self.prev, text);
            self.chunks.retain_mut(|c| {
                c.range.end = c.range.end.min(p);
                c.range.start < c.range.end
            });
            if text.len() > p {
                if let Some(last) = self.last_append {
                    let gap = now.saturating_duration_since(last).as_secs_f32() * 1000.;
                    self.ema_ms = self.ema_ms * 0.7 + gap.min(VEIL_GAP_CLAMP_MS) * 0.3;
                }
                self.last_append = Some(now);
                self.chunks.push(Chunk {
                    range: p..text.len(),
                    started: now,
                    duration_ms: (self.ema_ms * 3.).clamp(VEIL_MIN_FADE_MS, VEIL_MAX_FADE_MS),
                });
            }
            self.prev.clear();
            self.prev.push_str(text);
        }
        let boost = |active: usize| 1. + 0.3 * active.saturating_sub(2) as f32;
        let elapsed = |c: &Chunk| now.saturating_duration_since(c.started).as_secs_f32() * 1000.;
        let b = boost(self.chunks.len());
        self.chunks.retain(|c| elapsed(c) * b < c.duration_ms);
        let b = boost(self.chunks.len());
        self.chunks
            .iter()
            .map(|c| {
                let progress = (elapsed(c) * b / c.duration_ms).clamp(0., 1.);
                (c.range.clone(), veil_opacity(progress))
            })
            .collect()
    }
}

/// Pink `RowVeil`: the veils of one streaming reply, keyed per text element.
/// While `seeding`, an element seen for the first time adopts its text
/// without fading (content already on screen when the reply mounts).
#[derive(Default)]
struct RowVeil {
    elems: HashMap<usize, ElemVeil>,
    seeding: bool,
}

impl RowVeil {
    fn advance(&mut self, elem: usize, text: &str, now: Instant) -> Vec<VeilSpan> {
        if self.seeding && !self.elems.contains_key(&elem) {
            self.elems.insert(
                elem,
                ElemVeil {
                    prev: text.to_string(),
                    ..ElemVeil::default()
                },
            );
            return Vec::new();
        }
        self.elems.entry(elem).or_default().advance(text, now)
    }

    fn is_fading(&self) -> bool {
        self.elems.values().any(|e| !e.chunks.is_empty())
    }
}

/// Pink `slice_spans`: spans of a whole code text, cut to one line.
fn slice_spans(spans: &[VeilSpan], start: usize, end: usize) -> Vec<VeilSpan> {
    spans
        .iter()
        .filter_map(|(r, a)| {
            let s = r.start.max(start);
            let e = r.end.min(end);
            (s < e).then(|| (s - start..e - start, *a))
        })
        .collect()
}

/// Pink `apply_veil`: multiplies the veil into the runs' colors, splitting
/// runs at span edges. Paint only: text, fonts and lengths are untouched.
fn apply_veil(runs: Vec<TextRun>, spans: &[VeilSpan]) -> Vec<TextRun> {
    if spans.is_empty() || spans.iter().all(|(_, a)| *a >= 1.) {
        return runs;
    }
    let mut out = Vec::with_capacity(runs.len() + spans.len() * 2);
    let mut pos = 0usize;
    for run in runs {
        let (start, end) = (pos, pos + run.len);
        pos = end;
        let mut cuts = vec![start, end];
        for (r, _) in spans {
            if r.start > start && r.start < end {
                cuts.push(r.start);
            }
            if r.end > start && r.end < end {
                cuts.push(r.end);
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for w in cuts.windows(2) {
            let (s, e) = (w[0], w[1]);
            let mut piece = run.clone();
            piece.len = e - s;
            if let Some(alpha) = spans
                .iter()
                .find(|(r, _)| r.start <= s && e <= r.end)
                .map(|(_, a)| *a)
                && alpha < 1.
            {
                piece.color = piece.color.opacity(alpha);
                if let Some(u) = piece.underline.as_mut() {
                    u.color = u.color.map(|c| c.opacity(alpha));
                }
            }
            out.push(piece);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Flattened text (Pink: `FlatText` / `flatten_runs_weighted`)
// ---------------------------------------------------------------------------

struct FlatText {
    text: SharedString,
    runs: Vec<TextRun>,
    /// Clickable link ranges and their destinations.
    links: Vec<(Range<usize>, SharedString)>,
    /// Inline code ranges (their rounded washes paint under the text).
    code: Vec<Range<usize>>,
}

/// Pink `flatten_runs_weighted`: Geist at the block's weight (strong runs
/// never below semibold), italic where emphasized, Geist Mono in `code_text`
/// for inline code, everything else monochrome `text`. Links keep the text
/// color with a 1px `content_muted` underline; strikethrough is 1px
/// `content_muted`.
fn flatten(runs: &[Run], base: FontWeight, t: &Pink) -> FlatText {
    let mut text = String::new();
    let mut out: Vec<TextRun> = Vec::with_capacity(runs.len());
    let mut links: Vec<(Range<usize>, SharedString)> = Vec::new();
    let mut code: Vec<Range<usize>> = Vec::new();
    for run in runs {
        if run.text.is_empty() {
            continue;
        }
        let start = text.len();
        text.push_str(&run.text);
        let family = if run.style.code {
            pink::MONO
        } else {
            pink::SANS
        };
        let weight = if run.style.bold && base.0 < FontWeight::SEMIBOLD.0 {
            FontWeight::SEMIBOLD
        } else {
            base
        };
        let font = Font {
            weight,
            style: if run.style.italic {
                FontStyle::Italic
            } else {
                FontStyle::Normal
            },
            ..gpui::font(family)
        };
        if run.style.code {
            match code.last_mut() {
                Some(range) if range.end == start => range.end = text.len(),
                _ => code.push(start..text.len()),
            }
        }
        if let Some(url) = &run.style.link
            && url != PENDING_LINK
        {
            match links.last_mut() {
                Some((range, last)) if range.end == start && last.as_ref() == url => {
                    range.end = text.len()
                }
                _ => links.push((start..text.len(), url.clone().into())),
            }
        }
        out.push(TextRun {
            len: run.text.len(),
            font,
            color: if run.style.code { t.code_text } else { t.text },
            background_color: None,
            underline: run.style.link.is_some().then_some(UnderlineStyle {
                color: Some(t.content_muted),
                thickness: px(1.),
                wavy: false,
            }),
            strikethrough: run.style.strike.then_some(StrikethroughStyle {
                thickness: px(1.),
                color: Some(t.content_muted),
            }),
        });
    }
    FlatText {
        text: text.into(),
        runs: out,
        links,
        code,
    }
}

/// The rectangles a byte range covers on its shaped lines, one per visual row
/// (Pink: `markdown/render.rs` `range_rects`).
fn range_rects(
    layout: &TextLayout,
    range: &Range<usize>,
    pad_x: f32,
    inset_y: f32,
) -> Vec<Bounds<Pixels>> {
    let bounds = layout.bounds();
    let line_height = layout.line_height();
    let position = |ix: usize| layout.position_for_index(ix);
    let mut rects = Vec::new();
    let mut cur = range.start;
    let mut guard = 0;
    while cur < range.end && guard < 256 {
        guard += 1;
        let Some(mut p1) = position(cur) else {
            break;
        };
        // A soft-wrap boundary reports the end of the row above; start the
        // wash at the left of the row it really opens.
        if let Some(after) = position(cur.saturating_add(1))
            && after.y > p1.y
        {
            p1 = point(bounds.left(), after.y);
        }
        let (seg_end, next) = match position(range.end) {
            Some(pe) if pe.y == p1.y => (range.end, range.end),
            _ => {
                let (mut lo, mut hi) = (cur, range.end);
                while hi - lo > 1 {
                    let mid = lo + (hi - lo) / 2;
                    match position(mid) {
                        Some(pm) if pm.y == p1.y => lo = mid,
                        _ => hi = mid,
                    }
                }
                (lo, lo)
            }
        };
        if let Some(p2) = position(seg_end)
            && p2.x > p1.x
        {
            rects.push(Bounds::new(
                point(p1.x - px(pad_x), p1.y + px(inset_y)),
                size(p2.x - p1.x + px(2. * pad_x), line_height - px(2. * inset_y)),
            ));
        }
        if next <= cur {
            break;
        }
        cur = next;
    }
    rects
}

// ---------------------------------------------------------------------------
// Links (Pink: `markdown/link_interaction.rs` `LinkRanges`,
// `link_destination.rs`)
// ---------------------------------------------------------------------------

type LinkHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// Pink `link_destination::destination_card`: the link's full destination as
/// local text – never a fetched preview – on the tooltip surface, r4,
/// `border_strong`, 11/14, sized to the URL up to 360 wide and scrolling past
/// 160 tall.
fn destination_card(url: &str, window: &Window, t: &Pink) -> AnyElement {
    use unicode_segmentation::UnicodeSegmentation;
    let viewport = window.viewport_size();
    let max_w = (f32::from(viewport.width) - 24.).clamp(1., 360.);
    let max_h = (f32::from(viewport.height) - 80.).clamp(1., 160.);
    // Invisible break opportunities let even one long path segment wrap
    let text: SharedString = url
        .graphemes(true)
        .collect::<Vec<_>>()
        .join("\u{200b}")
        .into();
    let run = TextRun {
        len: text.len(),
        font: gpui::font(pink::SANS),
        color: t.text,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let natural = window
        .text_system()
        .shape_line(text.clone(), px(11.), &[run], None)
        .width();
    let width = (f32::from(natural).ceil() + 14.).min(max_w);
    div()
        .id("md-link-destination")
        .w(px(width))
        .max_w(px(width))
        .px(px(6.))
        .py(px(4.))
        .rounded(px(radius::XS))
        .border_1()
        .border_color(t.border_strong)
        .bg(t.tooltip_surface())
        .shadow(pink::shadow_sm())
        .font_family(pink::SANS)
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(t.text)
        .flex()
        .flex_col()
        .child(
            div()
                .id("md-link-destination-scroll")
                .max_h(px(max_h))
                .overflow_y_scroll()
                .child(text),
        )
        .into_any_element()
}

struct Destination(SharedString);

impl Render for Destination {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        destination_card(&self.0, window, &t)
    }
}

/// Pink `click_is_activation`: a plain left click that did not drag, or a
/// keyboard / touch activation.
fn click_is_activation(event: &ClickEvent) -> bool {
    match event {
        ClickEvent::Mouse(event) => {
            event.down.button == MouseButton::Left
                && event.down.click_count == 1
                && (event.up.position - event.down.position).magnitude() <= 4.
        }
        ClickEvent::Keyboard(_) | ClickEvent::Touch(_) => true,
    }
}

/// Opens a link: the host's handler, or the system browser.
fn open_link(url: &SharedString, on_link: Option<&LinkHandler>, window: &mut Window, cx: &mut App) {
    match on_link {
        Some(handler) => handler(url, window, cx),
        None => cx.open_url(url),
    }
}

/// Per-element link interaction state (Pink `Interaction`).
struct LinkInteraction {
    urls: Vec<SharedString>,
    focus: Vec<FocusHandle>,
    menu_focus: [FocusHandle; 2],
    menu_focus_pending: Rc<Cell<bool>>,
    menu: Rc<RefCell<Option<(usize, Point<Pixels>)>>>,
    bounds: Bounds<Pixels>,
    epoch: Rc<Cell<u64>>,
    dismissed: Rc<Cell<bool>>,
    focused: Option<usize>,
}

/// Pink `LinkRanges`: hit targets laid over a text's links, sharing its
/// shaped geometry so wrapped links work. A link opens on click, shows its
/// destination under the pointer, takes Tab focus (selection wash and an
/// accent edge, with the destination card below), and opens a small actions
/// menu on right click or Shift+F10.
struct LinkText {
    id: ElementId,
    child: AnyElement,
    layout: TextLayout,
    links: Vec<(Range<usize>, SharedString)>,
    labels: Vec<SharedString>,
    on_link: Option<LinkHandler>,
    t: Pink,
}

impl IntoElement for LinkText {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

type LinkPaint = (
    Vec<AnyElement>,
    Rc<RefCell<Option<(usize, Point<Pixels>)>>>,
    Rc<Cell<u64>>,
    Rc<Cell<bool>>,
);

impl Element for LinkText {
    type RequestLayoutState = ();
    type PrepaintState = LinkPaint;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> LinkPaint {
        self.child.prepaint(window, cx);
        let Some(id) = id else {
            return (Vec::new(), Rc::default(), Rc::default(), Rc::default());
        };
        window.with_element_state::<LinkInteraction, _>(id, |previous, window| {
            let urls: Vec<SharedString> = self.links.iter().map(|(_, u)| u.clone()).collect();
            let mut state =
                previous
                    .filter(|s| s.urls == urls)
                    .unwrap_or_else(|| LinkInteraction {
                        focus: urls
                            .iter()
                            .map(|_| cx.focus_handle().tab_stop(true))
                            .collect(),
                        urls,
                        menu_focus: std::array::from_fn(|_| cx.focus_handle().tab_stop(true)),
                        menu_focus_pending: Rc::default(),
                        menu: Rc::default(),
                        bounds,
                        epoch: Rc::default(),
                        dismissed: Rc::default(),
                        focused: None,
                    });
            if state.bounds != bounds {
                state.menu.borrow_mut().take();
                state.epoch.set(state.epoch.get().wrapping_add(1));
                state.dismissed.set(true);
            }
            let focused = state.focus.iter().position(|f| f.is_focused(window));
            if focused != state.focused {
                state.dismissed.set(false);
            }
            state.focused = focused;
            state.bounds = bounds;
            let t = self.t.clone();
            let mut overlays = Vec::new();
            for (index, (range, url)) in self.links.iter().enumerate() {
                for (part, rect) in range_rects(&self.layout, range, 0., 0.)
                    .into_iter()
                    .enumerate()
                {
                    let menu = state.menu.clone();
                    let key_menu = menu.clone();
                    let focus = state.focus[index].clone();
                    let destination = url.clone();
                    let click_url = url.clone();
                    let on_link = self.on_link.clone();
                    let pending = state.menu_focus_pending.clone();
                    let key_pending = pending.clone();
                    let dismissed = state.dismissed.clone();
                    let epoch = state.epoch.clone();
                    let selection = t.selection;
                    let accent = t.accent;
                    let hit = div()
                        .id(SharedString::from(format!(
                            "md-link-{index}-{part}-{}",
                            state.epoch.get()
                        )))
                        // No hover card while the actions menu owns input
                        .when(state.menu.borrow().is_none(), |hit| {
                            hit.hoverable_tooltip(move |_, cx| {
                                cx.new(|_| Destination(destination.clone())).into()
                            })
                        })
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .cursor_pointer()
                        .role(Role::Link)
                        .aria_label(self.labels[index].clone())
                        .when(part == 0, |el| el.track_focus(&focus))
                        .focus_visible(move |s| s.bg(selection).border_1().border_color(accent))
                        .on_click(move |event, window, cx| {
                            if click_is_activation(event) {
                                open_link(&click_url, on_link.as_ref(), window, cx);
                            }
                        })
                        .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                            *menu.borrow_mut() = Some((index, event.position));
                            pending.set(true);
                            cx.stop_propagation();
                            window.refresh();
                        })
                        .on_key_down(move |event, window, cx| {
                            match event.keystroke.key.as_str() {
                                "f10" if event.keystroke.modifiers.shift => {
                                    *key_menu.borrow_mut() = Some((index, rect.bottom_left()));
                                    key_pending.set(true);
                                    window.refresh();
                                }
                                "escape" => {
                                    key_menu.borrow_mut().take();
                                    dismissed.set(true);
                                    epoch.set(epoch.get().wrapping_add(1));
                                    window.refresh();
                                }
                                _ => {
                                    cx.propagate();
                                    return;
                                }
                            }
                            cx.stop_propagation();
                        });
                    let mut hit = hit.into_any_element();
                    hit.prepaint_as_root(
                        rect.origin,
                        rect.size.map(AvailableSpace::Definite),
                        window,
                        cx,
                    );
                    overlays.push(hit);
                }
            }
            // The focused link discloses its destination below it
            if state.menu.borrow().is_none()
                && !state.dismissed.get()
                && let Some(index) = focused
                && let Some(rect) = range_rects(&self.layout, &self.links[index].0, 0., 0.).first()
            {
                let card = destination_card(&state.urls[index], window, &t);
                let dismissed = state.dismissed.clone();
                let epoch = state.epoch.clone();
                let mut popup = deferred(
                    anchored()
                        .position(rect.bottom_left())
                        .anchor(Anchor::TopLeft)
                        .snap_to_window_with_margin(px(8.))
                        .child(
                            div()
                                .id("md-focused-link-destination")
                                .occlude()
                                .on_mouse_down_out(move |_, window, _| {
                                    dismissed.set(true);
                                    epoch.set(epoch.get().wrapping_add(1));
                                    window.refresh();
                                })
                                .child(card),
                        ),
                )
                .priority(1)
                .into_any_element();
                popup.prepaint_as_root(
                    bounds.origin,
                    window.viewport_size().map(AvailableSpace::Definite),
                    window,
                    cx,
                );
                overlays.push(popup);
            }
            if let Some((index, position)) = *state.menu.borrow() {
                let mut popup = link_menu(index, position, &state, self.on_link.clone(), &t);
                popup.prepaint_as_root(
                    bounds.origin,
                    window.viewport_size().map(AvailableSpace::Definite),
                    window,
                    cx,
                );
                overlays.push(popup);
            }
            (
                (
                    overlays,
                    state.menu.clone(),
                    state.epoch.clone(),
                    state.dismissed.clone(),
                ),
                state,
            )
        })
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        paint: &mut LinkPaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Scrolling away closes the menu and the focused disclosure
        let (menu, epoch, dismissed) = (paint.1.clone(), paint.2.clone(), paint.3.clone());
        window.on_mouse_event(move |_: &ScrollWheelEvent, phase, window, _| {
            if phase == DispatchPhase::Capture && (menu.borrow().is_some() || !dismissed.get()) {
                menu.borrow_mut().take();
                epoch.set(epoch.get().wrapping_add(1));
                dismissed.set(true);
                window.refresh();
            }
        });
        self.child.paint(window, cx);
        for overlay in &mut paint.0 {
            overlay.paint(window, cx);
        }
    }
}

/// Pink's link actions menu: a 260-wide popover card at the pointer with
/// "Open in external browser" and "Copy link address", arrowed with Tab or
/// Up/Down and closed with Escape or a click outside.
fn link_menu(
    index: usize,
    position: Point<Pixels>,
    state: &LinkInteraction,
    on_link: Option<LinkHandler>,
    t: &Pink,
) -> AnyElement {
    let menu = state.menu.clone();
    let dismiss = state.menu.clone();
    let return_focus = state.focus[index].clone();
    let menu_focus = state.menu_focus.clone();
    let pending = state.menu_focus_pending.clone();
    let initial = state.menu_focus[0].clone();
    let url = state.urls[index].clone();
    let mut card = pink::popover_card(t)
        .child(
            canvas(
                move |_, window, cx| {
                    // Claim focus once the menu mounts
                    if pending.replace(false) {
                        window.focus(&initial, cx);
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_0(),
        )
        .id("md-link-actions")
        .w(px(LINK_MENU_WIDTH))
        .flex()
        .flex_col()
        .on_key_down(move |event, window, cx| {
            match event.keystroke.key.as_str() {
                "escape" => {
                    dismiss.borrow_mut().take();
                    window.focus(&return_focus, cx);
                    window.refresh();
                }
                "tab" | "down" | "up" => {
                    let current = menu_focus
                        .iter()
                        .position(|f| f.is_focused(window))
                        .unwrap_or(0);
                    let back = event.keystroke.key == "up"
                        || (event.keystroke.key == "tab" && event.keystroke.modifiers.shift);
                    let next = if back {
                        (current + menu_focus.len() - 1) % menu_focus.len()
                    } else {
                        (current + 1) % menu_focus.len()
                    };
                    window.focus(&menu_focus[next], cx);
                }
                _ => return,
            }
            cx.stop_propagation();
        })
        .on_mouse_down_out({
            let menu = menu.clone();
            move |_, window, _| {
                menu.borrow_mut().take();
                window.refresh();
            }
        });
    let selected = t.card_selected_bg();
    for (ix, (label, icon)) in [
        ("Open in external browser", ARROW_UP_RIGHT),
        ("Copy link address", COPY),
    ]
    .into_iter()
    .enumerate()
    {
        let menu = menu.clone();
        let url = url.clone();
        let on_link = on_link.clone();
        card = card.child(
            pink::menu_row(format!("md-link-{index}-action-{ix}"), false, t)
                .child(
                    svg()
                        .path(icon)
                        .flex_none()
                        .size(px(16.))
                        .text_color(t.text_muted),
                )
                .child(label)
                .track_focus(&state.menu_focus[ix])
                .role(Role::Button)
                .aria_label(label)
                .focus_visible(move |s| s.bg(selected))
                .on_click(move |_, window, cx| {
                    if ix == 0 {
                        open_link(&url, on_link.as_ref(), window, cx);
                    } else {
                        cx.write_to_clipboard(ClipboardItem::new_string(url.to_string()));
                    }
                    menu.borrow_mut().take();
                    window.refresh();
                }),
        );
    }
    deferred(
        anchored()
            .position(position)
            .anchor(Anchor::TopLeft)
            .snap_to_window_with_margin(px(8.))
            .child(motion::menu_in(
                "md-link-actions-in",
                div().occlude().child(card),
            )),
    )
    .priority(1)
    .into_any_element()
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// One fence's horizontal viewport (Pink `CodeFenceRuntime` +
/// `popover::HorizontalScrollbarState`).
#[derive(Default)]
struct CodeRuntime {
    scroll: ScrollHandle,
    viewport_hovered: bool,
    bar_hovered: bool,
    grab: Option<f32>,
}

/// Pink `HorizontalScrollbarMetrics`.
#[derive(Clone, Copy)]
struct ScrollbarMetrics {
    thumb_left: f32,
    thumb_width: f32,
    track_width: f32,
    max_scroll: f32,
}

impl ScrollbarMetrics {
    fn travel(&self) -> f32 {
        (self.track_width - self.thumb_width).max(0.)
    }
}

impl CodeRuntime {
    fn metrics(&self) -> Option<ScrollbarMetrics> {
        let viewport = f32::from(self.scroll.bounds().size.width);
        let max_scroll = f32::from(self.scroll.max_offset().x).max(0.);
        if viewport <= 0. || max_scroll <= 0. {
            return None;
        }
        let track_width = (viewport - SCROLLBAR_TRACK_INSET * 2.).max(0.);
        if track_width <= 0. {
            return None;
        }
        let thumb_width = (track_width * viewport / (viewport + max_scroll))
            .max(SCROLLBAR_MIN_THUMB)
            .min(track_width);
        let current = (-f32::from(self.scroll.offset().x)).clamp(0., max_scroll);
        let travel = (track_width - thumb_width).max(0.);
        Some(ScrollbarMetrics {
            thumb_left: travel * current / max_scroll,
            thumb_width,
            track_width,
            max_scroll,
        })
    }

    fn visible(&self) -> bool {
        self.viewport_hovered || self.grab.is_some()
    }

    fn active(&self) -> bool {
        self.bar_hovered || self.grab.is_some()
    }

    fn set_viewport_hovered(&mut self, hovered: bool) -> bool {
        if self.viewport_hovered == hovered {
            return false;
        }
        self.viewport_hovered = hovered;
        if !hovered && self.grab.is_none() {
            self.bar_hovered = false;
        }
        true
    }

    fn set_bar_hovered(&mut self, hovered: bool) -> bool {
        let active = hovered || self.grab.is_some();
        if self.bar_hovered == active {
            return false;
        }
        self.bar_hovered = active;
        true
    }

    fn pointer_in_track(&self, x: Pixels) -> f32 {
        f32::from(x - self.scroll.bounds().left()) - SCROLLBAR_TRACK_INSET
    }

    fn begin_press(&mut self, x: Pixels) -> bool {
        let Some(m) = self.metrics() else {
            return false;
        };
        let p = self.pointer_in_track(x);
        let grab = if (m.thumb_left..=m.thumb_left + m.thumb_width).contains(&p) {
            p - m.thumb_left
        } else {
            m.thumb_width / 2.
        };
        self.grab = Some(grab);
        self.drag_to(x);
        true
    }

    fn drag_to(&self, x: Pixels) -> bool {
        let (Some(grab), Some(m)) = (self.grab, self.metrics()) else {
            return false;
        };
        let left = (self.pointer_in_track(x) - grab).clamp(0., m.travel());
        let to = if m.travel() <= 0. {
            0.
        } else {
            left / m.travel() * m.max_scroll
        };
        let offset = self.scroll.offset();
        self.scroll.set_offset(point(px(-to), offset.y));
        true
    }

    fn end_press(&mut self) -> bool {
        self.grab = None;
        if !self.viewport_hovered && self.bar_hovered {
            self.bar_hovered = false;
            return true;
        }
        false
    }
}

#[derive(Default)]
struct State {
    veil: RowVeil,
    seen: bool,
    /// The fence whose Copy reads "Copied".
    copied: Option<usize>,
    copy_reset: Option<Task<()>>,
    /// The reader's Fit-content choice, once made.
    fit: Option<bool>,
    code: Vec<CodeRuntime>,
}

/// A drag on a fence's scrollbar.
struct ScrollbarDrag {
    key: SharedString,
    ix: usize,
}

struct DragGhost;

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

/// Pink's code tooltip (`CodeBlockTooltip`): the tooltip card at `px 8 /
/// py 6`, 11px `text`.
struct CodeTooltip(&'static str);

impl Render for CodeTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        pink::tooltip_card(&t)
            .px(px(8.))
            .py(px(6.))
            .text_size(px(11.))
            .text_color(t.text)
            .child(self.0)
    }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

type TaskHandler = Rc<dyn Fn(usize, bool, &mut Window, &mut App)>;
type CopyHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// An assistant reply rendered from markdown (Pink: `markdown/render.rs`).
#[derive(IntoElement)]
pub struct MarkdownMessage {
    id: ElementId,
    source: SharedString,
    streaming: bool,
    tasks: bool,
    fit_code: bool,
    on_task_toggle: Option<TaskHandler>,
    on_link: Option<LinkHandler>,
    on_copy: Option<CopyHandler>,
}

impl MarkdownMessage {
    pub fn new(id: impl Into<ElementId>, source: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            source: source.into(),
            streaming: false,
            tasks: true,
            fit_code: false,
            on_task_toggle: None,
            on_link: None,
            on_copy: None,
        }
    }

    /// The reply is still arriving: newly appended text dissolves in under
    /// Pink's veil and hanging markers are closed for display.
    pub fn streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// Task items (`- [ ]`) as checkboxes (default), as in Pink's markdown
    /// preview; `false` shows them the way Pink's transcript does, as a
    /// bullet with the literal marker.
    pub fn task_checkboxes(mut self, on: bool) -> Self {
        self.tasks = on;
        self
    }

    /// Code fences start wrapped to the width (Pink's "Fit content") instead
    /// of scrolling sideways. The reader can switch it on each fence.
    pub fn fit_code(mut self, fit: bool) -> Self {
        self.fit_code = fit;
        self
    }

    /// Makes task checkboxes clickable: the task's index in document order
    /// and its new state. Rewrite your source with [`toggle_task`]. Without
    /// it, checkboxes show at half strength and do not toggle.
    pub fn on_task_toggle(
        mut self,
        handler: impl Fn(usize, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_task_toggle = Some(Rc::new(handler));
        self
    }

    /// Opens links yourself (default: the system browser).
    pub fn on_link(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }

    /// Called with a fence's code after Copy put it on the clipboard.
    pub fn on_copy(
        mut self,
        handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_copy = Some(Rc::new(handler));
        self
    }
}

/// Counts the code fences in document order, nested ones included.
fn count_code(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|b| match b {
            Block::Code { .. } => 1,
            Block::Quote(children) => count_code(children),
            Block::List { items, .. } => items.iter().map(|i| count_code(&i.blocks)).sum(),
            _ => 0,
        })
        .sum()
}

/// Walks the blocks into elements, numbering text elements, fences and
/// tasks in document order (the veil's, Copy's and the toggles' keys).
struct Renderer<'a> {
    t: Pink,
    key: SharedString,
    state: Entity<State>,
    window: &'a Window,
    veil: RowVeil,
    streaming: bool,
    now: Instant,
    seen: HashSet<usize>,
    elem: usize,
    code_ix: usize,
    task_ix: usize,
    copied: Option<usize>,
    fit: bool,
    tasks: bool,
    /// Per fence: (scroll handle, scrollbar metrics while shown, active).
    code: Vec<(ScrollHandle, Option<ScrollbarMetrics>, bool)>,
    on_task: Option<TaskHandler>,
    on_link: Option<LinkHandler>,
    on_copy: Option<CopyHandler>,
}

impl Renderer<'_> {
    fn veil_spans(&mut self, text: &str) -> (usize, Vec<VeilSpan>) {
        let elem = self.elem;
        self.elem += 1;
        if !self.streaming {
            return (elem, Vec::new());
        }
        self.seen.insert(elem);
        (elem, self.veil.advance(elem, text, self.now))
    }

    fn block(&mut self, block: &Block) -> AnyElement {
        match block {
            Block::Paragraph(runs) => {
                self.text(runs, MD_TEXT_SIZE, MD_LINE_HEIGHT, FontWeight::NORMAL)
            }
            Block::Heading(level, runs) => {
                let (size, line) = heading_metrics(*level);
                self.text(runs, size, line, FontWeight::SEMIBOLD)
            }
            Block::Code { language, code } => self.code(language.as_deref(), code),
            Block::Quote(children) => self.quote(children),
            Block::List { start, items } => self.list(*start, items),
            Block::Table {
                header,
                rows,
                align,
            } => self.table(header, rows, align),
            // Pink: a 1px rule in `border`
            Block::Rule => div()
                .h(px(1.))
                .w_full()
                .bg(self.t.border)
                .into_any_element(),
        }
    }

    /// Pink `text_element` + `flat_text_presented_element`: veiled runs over
    /// a canvas painting inline code's rounded washes, with link hit targets
    /// on top.
    fn text(
        &mut self,
        runs: &[Run],
        size: f32,
        line_height: f32,
        weight: FontWeight,
    ) -> AnyElement {
        let flat = flatten(runs, weight, &self.t);
        let (elem, spans) = self.veil_spans(&flat.text);
        let element = self.flat_element(flat, elem, &spans);
        div()
            .text_size(px(size))
            .line_height(px(line_height))
            .child(element)
            .into_any_element()
    }

    fn flat_element(&mut self, flat: FlatText, elem: usize, spans: &[VeilSpan]) -> AnyElement {
        let runs = apply_veil(flat.runs, spans);
        let styled = StyledText::new(flat.text.clone()).with_runs(runs);
        let layout = styled.layout().clone();
        let wash_layout = layout.clone();
        let wash = self.t.code_wash;
        let code = flat.code;
        // Inline code washes paint under the glyphs: a run background could
        // only be a square box
        let underlay = canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                for range in &code {
                    for rect in
                        range_rects(&wash_layout, range, INLINE_CODE_PAD_X, INLINE_CODE_INSET_Y)
                    {
                        window.paint_quad(quad(
                            rect,
                            px(INLINE_CODE_RADIUS),
                            wash,
                            px(0.),
                            transparent_black(),
                            BorderStyle::default(),
                        ));
                    }
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let child = div()
            .relative()
            .child(underlay)
            .child(styled)
            .into_any_element();
        if flat.links.is_empty() {
            return child;
        }
        let labels = flat
            .links
            .iter()
            .map(|(r, _)| SharedString::from(flat.text[r.clone()].to_string()))
            .collect();
        LinkText {
            id: ElementId::Name(format!("{}-links-{elem}", self.key).into()),
            child,
            layout,
            links: flat.links,
            labels,
            on_link: self.on_link.clone(),
            t: self.t.clone(),
        }
        .into_any_element()
    }

    /// Pink's quote: a 2px accent rail at 0.6, a 0.05 accent whisper behind,
    /// r6 on the right, `pl 12 pr 10 py 6`, children 8 apart.
    fn quote(&mut self, children: &[Block]) -> AnyElement {
        let mut el = div()
            .border_l_2()
            .border_color(self.t.accent.opacity(0.6))
            .bg(self.t.accent.opacity(0.05))
            .rounded_tr(px(radius::SM))
            .rounded_br(px(radius::SM))
            .pl(px(12.))
            .pr(px(10.))
            .py(px(6.))
            .flex()
            .flex_col()
            .gap(px(QUOTE_GAP))
            // Pink sets the quote's tone, but its runs paint `text`
            .text_color(self.t.content_muted);
        for child in children {
            el = el.child(self.block(child));
        }
        el.into_any_element()
    }

    /// Pink's list: items 4 apart, each a row of an 18px marker column and
    /// the item's blocks 8 to its right. Markers wear the accent: tinted
    /// numbers, a real 5px disc centered on the first line, or a checkbox.
    fn list(&mut self, start: Option<u64>, items: &[Item]) -> AnyElement {
        let mut list = div().flex().flex_col().gap(px(LIST_GAP));
        for (ix, item) in items.iter().enumerate() {
            let checkbox = item.task.filter(|_| self.tasks);
            let marker = match (checkbox, start) {
                (Some(checked), _) => self.task_marker(checked, item),
                (None, Some(start)) => div()
                    .flex_none()
                    .min_w(px(LIST_MARKER_MIN_W))
                    .text_size(px(MD_TEXT_SIZE))
                    .line_height(px(MD_LINE_HEIGHT))
                    .text_color(self.t.readable_tone(self.t.accent, 1.))
                    .child(SharedString::from(format!("{}.", start + ix as u64)))
                    .into_any_element(),
                (None, None) => div()
                    .flex_none()
                    .min_w(px(LIST_MARKER_MIN_W))
                    .h(px(MD_LINE_HEIGHT))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .ml(px(1.))
                            .size(px(LIST_DISC))
                            .rounded_full()
                            .bg(self.t.accent),
                    )
                    .into_any_element(),
            };
            let mut body = div().flex_1().min_w_0().flex().flex_col().gap(px(LIST_GAP));
            for (ci, child) in item.blocks.iter().enumerate() {
                // Without checkboxes, Pink's transcript shows the marker text
                let literal = match (ci, item.task, checkbox, child) {
                    (0, Some(done), None, Block::Paragraph(runs)) => {
                        let mut runs = runs.clone();
                        runs.insert(
                            0,
                            Run {
                                text: if done { "[x] " } else { "[ ] " }.into(),
                                style: InlineStyle::default(),
                            },
                        );
                        Some(Block::Paragraph(runs))
                    }
                    _ => None,
                };
                body = body.child(self.block(literal.as_ref().unwrap_or(child)));
            }
            list = list.child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(LIST_MARKER_GAP))
                    .child(marker)
                    .child(body),
            );
        }
        list.into_any_element()
    }

    /// Pink's task checkbox: 16px, r4, 1px edge – `border` when open, an
    /// accent plate with a 12px check in `bg` when done. It toggles when the
    /// host can rewrite the source, and otherwise rests at half strength.
    /// Focus brightens its edge to `text`.
    fn task_marker(&mut self, checked: bool, item: &Item) -> AnyElement {
        let ix = self.task_ix;
        self.task_ix += 1;
        let t = &self.t;
        let label: String = match item.blocks.first() {
            Some(Block::Paragraph(runs)) => runs.iter().map(|r| r.text.as_str()).collect(),
            _ => String::new(),
        };
        let edge = if checked { t.accent } else { t.border };
        let text = t.text;
        let toggle = self.on_task.clone();
        let mut check = div()
            .id(SharedString::from(format!("{}-task-{ix}", self.key)))
            .role(Role::CheckBox)
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .aria_label(label)
            .flex_none()
            .size(px(TASK_BOX))
            .border_1()
            .rounded(px(radius::XS))
            .border_color(edge)
            .bg(if checked {
                t.accent
            } else {
                transparent_black()
            })
            .flex()
            .items_center()
            .justify_center()
            .when(checked, |el| {
                el.child(svg().path(CHECK).size(px(TASK_CHECK)).text_color(t.bg))
            });
        check = match toggle {
            Some(toggle) => check
                .tab_index(0)
                .cursor_pointer()
                .focus_visible(move |s| s.border_color(text))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    toggle(ix, !checked, window, cx);
                }),
            None => check.opacity(0.5),
        };
        div()
            .flex_none()
            .min_w(px(LIST_MARKER_MIN_W))
            .h(px(MD_LINE_HEIGHT))
            .flex()
            .items_center()
            .child(check)
            .into_any_element()
    }

    /// Pink `render_table`: frameless – only 1px `separator(false)` rules
    /// under the header and between rows. Cells are 14/22 with 12px padding,
    /// the header bold. Columns share the width in proportion to their
    /// content, never below `min(content, 96)`; past that the table scrolls.
    fn table(
        &mut self,
        header: &[Vec<Run>],
        rows: &[Vec<Vec<Run>>],
        align: &[Align],
    ) -> AnyElement {
        let all: Vec<&[Vec<Run>]> = std::iter::once(header)
            .filter(|h| !h.is_empty())
            .map(|h| h as &[Vec<Run>])
            .chain(rows.iter().map(|r| r.as_slice()))
            .collect();
        let cols = all.iter().map(|r| r.len()).max().unwrap_or(0);
        if cols == 0 {
            return Empty.into_any_element();
        }
        let has_header = !header.is_empty();
        let mut flats: Vec<Vec<Option<(FlatText, usize, Vec<VeilSpan>)>>> = Vec::new();
        let mut content = vec![0f32; cols];
        for (r, row) in all.iter().enumerate() {
            let weight = if has_header && r == 0 {
                TABLE_HEADER_WEIGHT
            } else {
                FontWeight::NORMAL
            };
            let mut out = Vec::with_capacity(cols);
            for (c, natural) in content.iter_mut().enumerate() {
                let Some(runs) = row.get(c) else {
                    out.push(None);
                    continue;
                };
                let flat = flatten(runs, weight, &self.t);
                if !flat.text.is_empty() {
                    let line: SharedString = flat.text.replace('\n', " ").into();
                    let width = f32::from(
                        self.window
                            .text_system()
                            .shape_line(line, px(MD_TEXT_SIZE), &flat.runs, None)
                            .width(),
                    );
                    *natural = natural.max(width);
                }
                let (elem, spans) = self.veil_spans(&flat.text);
                out.push(Some((flat, elem, spans)));
            }
            flats.push(out);
        }
        let naturals: Vec<f32> = content
            .iter()
            .map(|w| w.max(TABLE_MIN_COLUMN_CONTENT) + 2. * TABLE_CELL_PADDING)
            .collect();
        let minimums: Vec<f32> = naturals
            .iter()
            .map(|n| n.min(TABLE_MIN_COLUMN_WIDTH))
            .collect();
        let min_table_width: f32 = minimums.iter().sum();
        let hairline = self.t.separator(false);
        let mut inner = div().flex().flex_col().w_full().min_w(px(min_table_width));
        for (r, row) in flats.into_iter().enumerate() {
            if r > 0 {
                inner = inner.child(div().flex_none().h(px(TABLE_DIVIDER)).w_full().bg(hairline));
            }
            let mut row_el = div().flex().flex_row();
            for (c, cell_flat) in row.into_iter().enumerate() {
                let mut cell = div()
                    .flex_grow(naturals[c])
                    .flex_shrink(naturals[c])
                    .flex_basis(px(0.))
                    .min_w(px(minimums[c]))
                    .p(px(TABLE_CELL_PADDING))
                    .text_size(px(MD_TEXT_SIZE))
                    .line_height(px(MD_LINE_HEIGHT));
                cell = match align.get(c).copied().unwrap_or_default() {
                    Align::Left => cell,
                    Align::Center => cell.text_center(),
                    Align::Right => cell.text_right(),
                };
                if let Some((flat, elem, spans)) = cell_flat {
                    cell = cell.child(self.flat_element(flat, elem, &spans));
                }
                row_el = row_el.child(cell);
            }
            inner = inner.child(row_el);
        }
        div()
            .id(SharedString::from(format!(
                "{}-table-{}",
                self.key, self.elem
            )))
            .w_full()
            .overflow_x_scroll()
            .child(inner)
            .into_any_element()
    }

    /// Pink `render_code_block_source`: the code surface (r10, 1px `border`,
    /// `ink(0.035)`), a 28px header on `ink(0.02)` naming the language in
    /// 11px `content_muted` with Fit and Copy trailing, then 12.5/18 mono
    /// lines in Pink's syntax colors at `px 12 / py 10`. Long lines scroll
    /// sideways under a thin scrollbar that shows while the pointer is over
    /// the fence – or wrap, with Fit content on.
    fn code(&mut self, language: Option<&str>, code: &str) -> AnyElement {
        let ix = self.code_ix;
        self.code_ix += 1;
        let t = self.t.clone();
        let key = self.key.clone();
        let (_, spans) = self.veil_spans(code);
        let fit = self.fit;
        let (scroll, metrics, active) =
            self.code
                .get(ix)
                .cloned()
                .unwrap_or((ScrollHandle::new(), None, false));

        // Pink's Fit-content toggle: a 22px square, r6; `ink(0.09)` while on
        let fit_key = format!("{key}-fit-{ix}");
        let (base, hover) = if fit {
            (t.ink(0.09), t.ink(0.13))
        } else {
            (transparent_black(), t.ink(0.08))
        };
        let label = if fit {
            "Use horizontal scrolling"
        } else {
            "Fit content"
        };
        let state = self.state.clone();
        let ring = t.focus_ring();
        let fit_button = div()
            .id(SharedString::from(fit_key.clone()))
            .role(Role::Button)
            .aria_label(label)
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .size(px(CODE_ACTION_SIZE))
            .rounded(px(radius::SM))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover_fill(fit_key, base, hover)
            .on_click(move |_, _, cx| {
                cx.stop_propagation();
                state.update(cx, |s, cx| {
                    s.fit = Some(!fit);
                    cx.notify();
                });
            })
            .tooltip(move |_, cx| cx.new(|_| CodeTooltip(label)).into())
            .child(
                svg()
                    .path(WRAP_TEXT)
                    .size(px(13.))
                    .text_color(t.content_muted),
            );

        // Pink `code_copy_button`
        let copied = self.copied == Some(ix);
        let copy_key = format!("{key}-copy-{ix}");
        let state = self.state.clone();
        let code_text: SharedString = code.to_string().into();
        let on_copy = self.on_copy.clone();
        let copy_button = div()
            .id(SharedString::from(copy_key.clone()))
            .role(Role::Button)
            .aria_label("Copy code")
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .h(px(CODE_ACTION_SIZE))
            .px(px(6.))
            .rounded(px(radius::SM))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .cursor_pointer()
            .hover_fill(copy_key, transparent_black(), t.ink(0.08))
            .text_size(px(11.))
            .text_color(t.content_muted)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                cx.write_to_clipboard(ClipboardItem::new_string(code_text.to_string()));
                state.update(cx, |s, cx| {
                    s.copied = Some(ix);
                    s.copy_reset = Some(cx.spawn(
                        async move |this: WeakEntity<State>, cx: &mut AsyncApp| {
                            cx.background_executor().timer(COPIED_FOR).await;
                            this.update(cx, |s, cx| {
                                s.copied = None;
                                cx.notify();
                            })
                            .ok();
                        },
                    ));
                    cx.notify();
                });
                if let Some(on_copy) = &on_copy {
                    on_copy(&code_text, window, cx);
                }
            })
            .child(
                svg()
                    .path(if copied { COPY_CHECK } else { COPY })
                    .flex_none()
                    .size(px(12.))
                    .text_color(t.content_muted),
            )
            .when(copied, |el| el.child("Copied"));

        // The lines, colored per token
        let lang = Lang::of(language);
        let syn = t.syntax();
        let mut in_comment = false;
        let mut offset = 0;
        let line_el = |el: Div| {
            if fit {
                el.w_full().min_w_0().min_h(px(CODE_LINE_HEIGHT))
            } else {
                el.h(px(CODE_LINE_HEIGHT)).flex_none()
            }
        };
        let mut lines = div()
            .map(|el| {
                if fit {
                    el.w_full().min_w_0()
                } else {
                    el.min_w_full().flex_none()
                }
            })
            .px(px(CODE_PADDING_X))
            .py(px(CODE_PADDING_Y))
            .font_family(pink::MONO)
            .text_size(px(CODE_TEXT_SIZE))
            .line_height(px(CODE_LINE_HEIGHT))
            .map(|el| {
                if fit {
                    el.whitespace_normal()
                } else {
                    el.whitespace_nowrap()
                }
            })
            .flex()
            .flex_col();
        for line in code.split('\n') {
            let start = offset;
            offset += line.len() + 1;
            let tokens = lex_line(lang, line, &mut in_comment);
            let runs = syntax_runs(line, &tokens, t.text, &syn);
            let runs = apply_veil(runs, &slice_spans(&spans, start, start + line.len()));
            lines = lines.child(
                line_el(div())
                    .child(StyledText::new(SharedString::from(line.to_string())).with_runs(runs)),
            );
        }

        let body = if fit {
            div()
                .w_full()
                .min_w_0()
                .overflow_hidden()
                .child(lines)
                .into_any_element()
        } else {
            div()
                .id(SharedString::from(format!("{key}-code-{ix}")))
                .w_full()
                .min_w_0()
                .flex()
                .overflow_x_scroll()
                .track_scroll(&scroll)
                // A vertical wheel over code keeps moving the transcript
                .restrict_scroll_to_axis()
                .child(lines)
                .into_any_element()
        };

        // Pink's code scrollbar: a 10px hit strip along the bottom; a 3px
        // thumb (5px while pointed at or dragged) in `text_faint` at 0.5
        // (0.68 active), 2px off the bottom edge
        let scrollbar = metrics.filter(|_| !fit).map(|m| {
            let thumb = if active {
                SCROLLBAR_THUMB_ACTIVE
            } else {
                SCROLLBAR_THUMB
            };
            let hover_state = self.state.clone();
            let press_state = self.state.clone();
            let up_state = self.state.clone();
            let out_state = self.state.clone();
            let drag_key = key.clone();
            div()
                .id(SharedString::from(format!("{key}-scrollbar-{ix}")))
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(CODE_SCROLLBAR_HIT_HEIGHT))
                .on_hover(move |hovered, _, cx| {
                    let hovered = *hovered;
                    hover_state.update(cx, |s, cx| {
                        if s.code
                            .get_mut(ix)
                            .is_some_and(|c| c.set_bar_hovered(hovered))
                        {
                            cx.notify();
                        }
                    });
                })
                .on_mouse_down(MouseButton::Left, move |event, _, cx| {
                    let x = event.position.x;
                    press_state.update(cx, |s, cx| {
                        if s.code.get_mut(ix).is_some_and(|c| c.begin_press(x)) {
                            cx.stop_propagation();
                            cx.notify();
                        }
                    });
                })
                .on_drag(ScrollbarDrag { key: drag_key, ix }, |_, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| DragGhost)
                })
                .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                    up_state.update(cx, |s, cx| {
                        if let Some(c) = s.code.get_mut(ix) {
                            c.end_press();
                        }
                        cx.notify();
                    });
                })
                .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                    out_state.update(cx, |s, cx| {
                        if let Some(c) = s.code.get_mut(ix) {
                            c.end_press();
                        }
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .absolute()
                        .left(px(SCROLLBAR_TRACK_INSET + m.thumb_left))
                        .bottom(px(2.))
                        .w(px(m.thumb_width))
                        .h(px(thumb))
                        .rounded(px(thumb / 2.))
                        .bg(t.text_faint.opacity(if active { 0.68 } else { 0.5 })),
                )
        });

        // Pink `code_block_header`: language left, actions right, gap 2
        let header = div()
            .h(px(CODE_HEADER_HEIGHT))
            .flex_none()
            .pl(px(CODE_PADDING_X))
            .pr(px(5.))
            .border_b_1()
            .border_color(t.border)
            .bg(t.ink(0.02))
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .min_w_0()
                    .text_size(px(11.))
                    .text_color(t.content_muted)
                    .children(language.map(|l| SharedString::from(l.to_string()))),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .child(fit_button)
                    .child(copy_button),
            );

        // Pink `code_block_frame`
        let hover_state = self.state.clone();
        let move_state = self.state.clone();
        let frame_key = key.clone();
        div()
            .id(SharedString::from(format!("{key}-code-frame-{ix}")))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded(px(radius::LG))
            .bg(t.ink(0.035))
            .border_1()
            .border_color(t.border)
            .overflow_hidden()
            .relative()
            .child(header)
            .child(div().w_full().relative().child(body).children(scrollbar))
            .on_hover(move |hovered, _, cx| {
                let hovered = *hovered;
                hover_state.update(cx, |s, cx| {
                    if s.code
                        .get_mut(ix)
                        .is_some_and(|c| c.set_viewport_hovered(hovered))
                    {
                        cx.notify();
                    }
                });
            })
            .on_drag_move(move |event: &gpui::DragMoveEvent<ScrollbarDrag>, _, cx| {
                let Some(drag) = event.dragged_item().downcast_ref::<ScrollbarDrag>() else {
                    return;
                };
                if drag.key != frame_key || drag.ix != ix {
                    return;
                }
                let x = event.event.position.x;
                move_state.update(cx, |s, cx| {
                    if s.code.get(ix).is_some_and(|c| c.drag_to(x)) {
                        cx.notify();
                    }
                });
            })
            .into_any_element()
    }
}

impl RenderOnce for MarkdownMessage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let now = Instant::now();
        // Pink: only a live row wears the veil; reduced motion renders none
        let streaming = self.streaming && !motion::reduced_motion(cx);
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| State::default(),
        );
        let blocks = parse(&self.source, self.streaming);
        let fences = count_code(&blocks);
        let (veil, copied, fit, code) = state.update(cx, |s, _| {
            if !s.seen {
                s.veil.seeding = true;
            }
            s.seen = true;
            s.code
                .resize_with(fences.max(s.code.len()), CodeRuntime::default);
            let code = s
                .code
                .iter()
                .map(|c| {
                    let shown = c.visible().then(|| c.metrics()).flatten();
                    (c.scroll.clone(), shown, c.active())
                })
                .collect();
            (
                std::mem::take(&mut s.veil),
                s.copied,
                s.fit.unwrap_or(self.fit_code),
                code,
            )
        });
        let key = SharedString::from(self.id.to_string());
        let mut r = Renderer {
            t: t.clone(),
            key,
            state: state.clone(),
            window,
            veil,
            streaming,
            now,
            seen: HashSet::new(),
            elem: 0,
            code_ix: 0,
            task_ix: 0,
            copied,
            fit,
            tasks: self.tasks,
            code,
            on_task: self.on_task_toggle.clone(),
            on_link: self.on_link.clone(),
            on_copy: self.on_copy.clone(),
        };
        let mut children = Vec::with_capacity(blocks.len());
        for (ix, block) in blocks.iter().enumerate() {
            let element = r.block(block);
            // Pink `heading_top_extra`: headings group with what follows
            children.push(match block {
                Block::Heading(level, _) if ix > 0 => div()
                    .pt(px(heading_top_extra(*level)))
                    .child(element)
                    .into_any_element(),
                _ => element,
            });
        }
        let mut veil = r.veil;
        let seen = r.seen;
        if streaming {
            veil.elems.retain(|elem, _| seen.contains(elem));
        } else {
            veil.elems.clear();
        }
        veil.seeding = false;
        let fading = veil.is_fading();
        state.update(cx, |s, _| s.veil = veil);
        if fading {
            window.request_animation_frame();
        }
        motion::keep_alive(window);

        div()
            .id(self.id)
            .w_full()
            .min_w_0()
            .font_family(pink::SANS)
            .text_color(t.text)
            .text_size(px(MD_TEXT_SIZE))
            .line_height(px(MD_LINE_HEIGHT))
            .flex()
            .flex_col()
            .gap(px(MD_BLOCK_GAP))
            .children(children)
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

const DEMO_WIDTH: f32 = 640.;
/// The settled reply's height: the stage holds still while it streams.
const MESSAGE_HEIGHT: f32 = 975.;
const DEMO_QUESTION: &str = "How do I stop our request logs from printing session tokens?";

/// The tasks at the end of the demo reply, toggled by the reader.
const DEMO_TASKS: [&str; 3] = [
    "Wrap the headers in `Redacted`",
    "Rotate the session token that already shipped",
    "Add a test that fails if `Bearer` reaches the log",
];

/// The demo reply up to its task list.
// joiners: allow a sample markdown reply, rendered as prose
const DEMO_REPLY: &str = r#"## Redact sensitive headers before logging

The leak comes from `RequestLogger`, which prints the header map verbatim. Wrap it in a **redacting view** so `Authorization` and `Cookie` never reach the formatter – the rest stay readable. The [`tracing` field docs](https://docs.rs/tracing/latest/tracing/field/index.html) cover the same idea for spans.

1. Add the `Redacted` wrapper below.
2. Log `Redacted(&headers)` instead of `headers` in *log_request*.
3. Rotate anything that already reached ~~stdout~~ the log files.

```rust
const SENSITIVE: [&str; 2] = ["authorization", "cookie"];
pub struct Redacted<'a>(pub &'a HeaderMap);

impl fmt::Debug for Redacted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut map = f.debug_map();
        for (name, value) in self.0 {
            let shown = if SENSITIVE.contains(&name.as_str()) { "[redacted]" } else { value.to_str().unwrap_or("…") };
            map.entry(&name.as_str(), &shown);
        }
        map.finish()
    }
}
```

| Header | Before | After |
| --- | --- | --- |
| `Authorization` | `Bearer eyJhbGciOi…` | `[redacted]` |
| `Cookie` | `sid=4f9c2e…` | `[redacted]` |
| `User-Agent` | `pink/0.4.2` | `pink/0.4.2` |

Two more places the same values can slip out:

- `Set-Cookie` on responses goes through the same logger.
- Proxies may add `X-Forwarded-Authorization`, so match on the suffix too.

> The token in yesterday's `api.log` stays valid until it expires on Friday, so rotate it now rather than waiting.

"#;

fn demo_source(tasks: [bool; 3]) -> String {
    let mut s = String::from(DEMO_REPLY);
    for (task, done) in DEMO_TASKS.iter().zip(tasks) {
        s.push_str(&format!("- [{}] {task}\n", if done { "x" } else { " " }));
    }
    s
}

/// The demo turn: the reply streams in a few words at a time, settles,
/// holds, then streams again.
struct Demo {
    run: u64,
    /// Bytes of the reply shown so far.
    shown: usize,
    tasks: [bool; 3],
    _timer: Task<()>,
}

/// How long one streamed chunk takes, and the pause after the reply lands.
const STREAM_TICK: Duration = Duration::from_millis(45);
const HOLD: Duration = Duration::from_millis(9000);

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            run: 0,
            shown: 0,
            tasks: [true, false, false],
            _timer: Self::play(cx),
        }
    }

    fn play(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |demo, cx| {
                    let total = demo_source(demo.tasks).len();
                    if demo.shown >= total {
                        // Start over after the hold
                        demo.shown = 0;
                        demo.run += 1;
                        demo.tasks = [true, false, false];
                        cx.notify();
                        return STREAM_TICK;
                    }
                    demo.shown = next_chunk(&demo_source(demo.tasks), demo.shown);
                    cx.notify();
                    if demo.shown >= total {
                        HOLD
                    } else {
                        STREAM_TICK
                    }
                }) else {
                    break;
                };
                cx.background_executor().timer(wait).await;
            }
        })
    }
}

/// The end of the next streamed chunk: about three words on.
fn next_chunk(source: &str, from: usize) -> usize {
    let mut at = from;
    for _ in 0..3 {
        let rest = &source[at..];
        let skip = rest.len() - rest.trim_start().len();
        let word = rest[skip..]
            .find(char::is_whitespace)
            .map_or(rest.len(), |w| skip + w);
        at += word;
        if at >= source.len() {
            return source.len();
        }
    }
    at
}

/// Pink: the user message bubble (`transcript.rs` user row), drawn by the
/// `pink-user-bubble` package – right-aligned, at most 80% wide.
fn user_bubble(text: &'static str, t: &Pink) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .justify_end()
        .child(crate::components::user_bubble::plate(DEMO_WIDTH, t).child(text))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("markdown-message-demo", cx, |_, cx| Demo::new(cx));
        let (run, shown, tasks) = {
            let d = demo.read(cx);
            (d.run, d.shown, d.tasks)
        };
        let source = demo_source(tasks);
        let shown = shown.min(source.len());
        let live = shown < source.len();
        let message = MarkdownMessage::new(
            ElementId::NamedInteger("reply".into(), run),
            source[..shown].to_string(),
        )
        .streaming(live)
        .on_task_toggle({
            let demo = demo.clone();
            move |index, checked, _, cx| {
                demo.update(cx, |d, cx| {
                    if let Some(task) = d.tasks.get_mut(index) {
                        *task = checked;
                    }
                    cx.notify();
                })
            }
        });
        div()
            .w(px(DEMO_WIDTH))
            .flex()
            .flex_col()
            .gap(px(16.))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(user_bubble(DEMO_QUESTION, &t))
            // Room for the whole reply, so the stage holds still
            .child(div().min_h(px(MESSAGE_HEIGHT)).child(message))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(runs: &[Run]) -> String {
        runs.iter().map(|r| r.text.as_str()).collect()
    }

    #[test]
    fn parses_the_blocks_pink_supports() {
        let blocks = parse(
            // joiners: allow markdown table syntax under test
            "# Title\n\nSome *em* and **strong** text.\n\n- one\n- [x] two\n\n3. three\n4. four\n\n> quoted\n\n---\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|:-|-:|\n| 1 | 2 |\n",
            false,
        );
        assert!(matches!(&blocks[0], Block::Heading(1, runs) if text(runs) == "Title"));
        assert!(
            matches!(&blocks[1], Block::Paragraph(runs) if text(runs) == "Some em and strong text.")
        );
        let Block::List { start: None, items } = &blocks[2] else {
            panic!("bullets")
        };
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].task, Some(true));
        assert!(matches!(&blocks[3], Block::List { start: Some(3), items } if items.len() == 2));
        assert!(matches!(&blocks[4], Block::Quote(children) if children.len() == 1));
        assert_eq!(blocks[5], Block::Rule);
        assert!(
            matches!(&blocks[6], Block::Code { language: Some(l), code } if l == "rust" && code == "fn main() {}")
        );
        let Block::Table {
            header,
            rows,
            align,
        } = &blocks[7]
        else {
            panic!("table")
        };
        assert_eq!(header.len(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(align, &vec![Align::Left, Align::Right]);
    }

    #[test]
    fn inline_styles_nest_and_link() {
        let runs = parse_inline(
            "a **b *c*** `d` [e](https://x.dev) ~~f~~ https://y.dev.",
            false,
        );
        let find = |s: &str| runs.iter().find(|r| r.text == s).expect(s).style.clone();
        assert!(find("b ").bold && !find("b ").italic);
        assert!(find("c").bold && find("c").italic);
        assert!(find("d").code);
        assert_eq!(find("e").link.as_deref(), Some("https://x.dev"));
        assert!(find("f").strike);
        assert_eq!(find("https://y.dev").link.as_deref(), Some("https://y.dev"));
        assert!(runs.last().is_some_and(|r| r.text == "."));
    }

    #[test]
    fn streaming_closes_hanging_markers() {
        let settled = parse_inline("a **bold", false);
        assert_eq!(text(&settled), "a **bold");
        let mended = parse_inline("a **bold", true);
        assert!(mended.iter().any(|r| r.text == "bold" && r.style.bold));
        let link = parse_inline("see [the docs](https://exa", true);
        assert!(
            link.iter()
                .any(|r| r.text == "the docs" && r.style.link.as_deref() == Some(PENDING_LINK))
        );
        let code = parse_inline("run `cargo te", true);
        assert!(code.iter().any(|r| r.text == "cargo te" && r.style.code));
    }

    #[test]
    fn toggles_the_nth_task_only() {
        let src = "- [ ] a\n```\n- [ ] not a task\n```\n- [x] b\n> - [ ] c\n";
        let out = toggle_task(src, 1, false);
        assert_eq!(
            out,
            "- [ ] a\n```\n- [ ] not a task\n```\n- [ ] b\n> - [ ] c\n"
        );
        let out = toggle_task(src, 2, true);
        assert!(out.ends_with("> - [x] c\n"));
        assert_eq!(toggle_task(src, 9, true), src);
    }

    #[test]
    fn lexer_colors_rust_tokens() {
        let mut in_comment = false;
        let line = "let x = format!(\"{}\\n\", y.len()); // done";
        let spans = lex_line(Lang::Rust, line, &mut in_comment);
        let kind = |s: &str| {
            let at = line.find(s).unwrap();
            spans.iter().find(|(r, _)| r.start == at).map(|(_, k)| *k)
        };
        assert_eq!(kind("let"), Some(Tok::Keyword));
        assert_eq!(kind("format!"), Some(Tok::Macro));
        assert_eq!(kind("\\n"), Some(Tok::Escape));
        assert_eq!(kind("len"), Some(Tok::Function));
        assert_eq!(kind("// done"), Some(Tok::Comment));
        // Spans never overlap and stay inside the line
        let mut at = 0;
        for (r, _) in &spans {
            assert!(r.start >= at && r.end <= line.len());
            at = r.end;
        }
    }

    #[test]
    fn veil_fades_each_append_once() {
        let mut veil = ElemVeil::default();
        let t0 = Instant::now();
        assert_eq!(veil.advance("Hello", t0).len(), 1);
        let later = t0 + Duration::from_millis(500);
        assert!(veil.advance("Hello", later).is_empty());
        let spans = veil.advance("Hello there", later);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].0, 5..11);
        assert_eq!(veil_opacity(1.), 1.);
    }

    #[test]
    fn demo_streams_to_the_end() {
        let source = demo_source([true, false, false]);
        let mut at = 0;
        let mut steps = 0;
        while at < source.len() {
            at = next_chunk(&source, at);
            assert!(source.is_char_boundary(at));
            steps += 1;
        }
        assert!(steps > 20);
        let blocks = parse(&source, false);
        assert_eq!(count_code(&blocks), 1);
    }
}
