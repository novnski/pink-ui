//! Tool Call Card – an agent's tool calls drawn the way Pink's transcript
//! draws them (Pink: `transcript.rs` `render_tool_group`, `chip_header_row`,
//! `tool_failure_line`, `tool_history_more_row`, `activity_rail`,
//! `detail_body`; the sentences are `transcript/activity.rs`).
//!
//! Every call and group reads as one truthful, verb-first sentence (UI
//! rethink D63): `Ran pwd`, `Edited 2 files`, `Failed to run cargo test`,
//! `Running git log`. The tense follows the call's own lifecycle, counts name
//! settled successes only, and a failure count or attention state stays a
//! separate fact on the header; no label joins facts with a separator. The
//! sentences, the header and the failure line are drawn by the
//! `pink-tool-activity` package PINK pins ([`super::tool_activity`]).
//!
//! [`ToolCallGroup`] is Pink's tool group: a quiet plate on the 28 pt
//! activity pitch with the kind glyph, the group's sentence in 14/20 (a
//! light sweeps its verb while the run is live), a red `1 failed` when the
//! sentence does not already say so, and a chevron. Open, it hangs one bare
//! row per call off an activity rail on the same pitch: a hairline trunk
//! that bends into each row's glyph (the mini spinner while the call runs, a
//! red danger circle when it failed) and the call's sentence, commands and
//! patterns as inline code and files as Pink's file badge. A failed call adds
//! its failure line: the first line it reported, then Show details and, for a
//! command, Copy command. Past eight settled successes the rest wait behind
//! `Show N more` (D64); failures and running calls never hide and never
//! count. A row with an invocation or an output folds open into Pink's code
//! surface: "Invocation" or "Command" and "Output" segments with copy
//! actions, 24 lines with a counted tail and a "Show full output"
//! affordance. Rows arrive with the rail drawing into them; the group stays
//! open while live and folds itself shut once the run settles, unless
//! someone toggled it. Pitch, text and the failure line scale with the
//! Interface size, as in Pink.
//!
//! A [`ToolCallCard`] outside a group is Pink's chip card: a 30px bordered
//! card with an 18px icon tile that folds open the same way.
//!
//! Tab reaches every header and recovery action; Enter or Space activates.
//!
//! ```ignore
//! ToolCallGroup::new("tools").child(
//!     ToolCallCard::new("test", Verb::RUN, ToolCallStatus::Running)
//!         .icon("icons/tool-call-card/terminal.svg")
//!         .code("cargo test -p cart")
//!         .command("cargo test -p cart"),
//! )
//! ```

use std::{collections::HashMap, f32::consts::FRAC_PI_2, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, AsyncApp, BoxShadow, ClickEvent, ClipboardItem, Context, Div,
    ElementId, Entity, FillOptions, FillRule, IntoElement, PathBuilder, PathStyle, Pixels, Point,
    Role, SharedString, Stateful, Task, Transformation, WeakEntity, Window, canvas, div, img,
    point, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill as _,
    components::{
        code_block, file_badge,
        tool_activity::{self as activity, ActivityState, ActivitySummary, Segment},
    },
    motion,
    pink::{self, FillTier, Pink, loaders, radius},
    story,
};

pub const NAME: &str = "Tool Call Card";
pub const DESCRIPTION: &str = "An agent's tool calls as Pink's verb-first sentences: one quiet group line that shimmers while the run is live and names a failure apart, then one row per call on an activity rail, eight settled calls before Show N more, a failed call's own words with Show details and Copy command, and each row folding open to its command and output with copy";

// ---------------------------------------------------------------------------
// Geometry (Pink `transcript.rs` tool row constants)
// ---------------------------------------------------------------------------

/// A chip card's row: 30px card centered in 38 (Pink `CHIP_HEIGHT`).
const CHIP_HEIGHT: f32 = 38.0;
/// The bordered chip card, border-box (Pink `CHIP_CARD_HEIGHT`).
const CHIP_CARD_HEIGHT: f32 = 30.0;
/// The header inside the bordered card (Pink `CHIP_HEADER_HEIGHT`).
const CHIP_HEADER_HEIGHT: f32 = CHIP_CARD_HEIGHT - 2.0;
/// The rail's gutter, trunk, elbow and glyph (Pink `ACTIVITY_*`).
const ACTIVITY_GUTTER_WIDTH: f32 = 48.0;
const ACTIVITY_TEXT_GAP: f32 = 8.0;
const ACTIVITY_TRUNK_X: f32 = 12.5;
const ACTIVITY_BEND_RADIUS: f32 = 6.0;
const ACTIVITY_BRANCH_END_X: f32 = 28.0;
const ACTIVITY_ICON_LEFT: f32 = 32.0;
const ACTIVITY_ICON_SIZE: f32 = pink::icon_size::SM;
/// Rows and the group sentence: 14/20 at the 16 px default (Pink
/// `TOOL_LABEL_SIZE`, `TOOL_LABEL_LINE_HEIGHT`).
const TOOL_TEXT_SIZE: f32 = activity::TEXT_SIZE;
const TOOL_LINE_HEIGHT: f32 = activity::LINE_HEIGHT;
/// Group headers and rows share the 28 pt activity pitch (D63), scaled with
/// the Interface size; the header's plate is 1 pt in on each side (Pink
/// `TOOL_GROUP_HEADER_HEIGHT`, `TOOL_TREE_ROW_HEIGHT`).
const TOOL_GROUP_HEADER_HEIGHT: f32 = 28.0;
const TOOL_TREE_ROW_HEIGHT: f32 = 28.0;
/// The line under a failed call: what it reported and how to recover, in
/// the 13 pt chrome role (Pink `TOOL_FAILURE_LINE_HEIGHT`,
/// `TOOL_FAILURE_TEXT_SIZE`).
const TOOL_FAILURE_LINE_HEIGHT: f32 = 24.0;
const TOOL_FAILURE_TEXT_SIZE: f32 = 13.0;
/// An open group first shows this many settled calls, then a truthful
/// `Show N more` (D64, Pink `TOOL_HISTORY_BUDGET`).
const TOOL_HISTORY_BUDGET: usize = 8;
/// The failure line's Copy command shares the row's copy feedback, after the
/// invocation (0) and output (1) copies (Pink `TOOL_FAILURE_COPY_IX`).
const FAILURE_COPY_SLOT: usize = 2;
/// Rows and group headers share one chevron (Pink `DISCLOSURE_CHEVRON_SIZE`).
const DISCLOSURE_CHEVRON_SIZE: f32 = pink::icon_size::XS;
/// Space above the first rail row (Pink `CHIPS_TOP_PAD`).
const CHIPS_TOP_PAD: f32 = 2.0;
/// Lines an output segment shows before its counted tail (Pink
/// `OUTPUT_DETAIL_MAX_LINES`).
const OUTPUT_DETAIL_MAX_LINES: usize = 24;
/// The ceiling for an invocation and for a full output (Pink
/// `FULL_OUTPUT_MAX_LINES`).
const FULL_OUTPUT_MAX_LINES: usize = 400;
/// Invocation and output lines soft-wrap here (Pink `CALL_WRAP_COLS`).
const CALL_WRAP_COLS: usize = 80;
/// The code surface header (Pink `OUTPUT_CODE_HEADER_HEIGHT`).
const OUTPUT_CODE_HEADER_HEIGHT: f32 = code_block::HEADER_HEIGHT;
/// Code segments scroll past this (Pink `OUTPUT_VIEWPORT_MAX_HEIGHT`).
const OUTPUT_VIEWPORT_MAX_HEIGHT: f32 = 252.0;
/// One code line: the code size's 12.5/18 rhythm (Pink `OUTPUT_LINE_HEIGHT`).
const OUTPUT_LINE_HEIGHT: f32 = 18.0;
/// The code body's `py 6` × 2 (Pink `OUTPUT_BODY_PAD`).
const OUTPUT_BODY_PAD: f32 = 12.0;
/// The hairline between a row's header and its body (Pink
/// `DETAIL_SEPARATOR`).
const DETAIL_SEPARATOR: f32 = 1.0;
/// The "Show full output" row (Pink `BLOB_AFFORDANCE_HEIGHT`).
const BLOB_AFFORDANCE_HEIGHT: f32 = 24.0;
/// The code surface's side padding (Pink `render::CODE_PADDING_X`).
const CODE_PADDING_X: f32 = 12.0;
/// Copy actions in the code header (Pink `render::CODE_ACTION_SIZE`).
const CODE_ACTION_SIZE: f32 = 22.0;
/// How long a copy action reads "Copied" (Pink `copy_ui_for`).
const COPIED_FOR: Duration = Duration::from_millis(1200);
/// Rows hover their label, detail and chevron together.
const HEADER_GROUP: &str = "tool-header";

const ICONS: &str = "icons/tool-call-card";
const ALT_ARROW_RIGHT: &str = "icons/tool-call-card/alt-arrow-right.svg";
const ALT_ARROW_DOWN: &str = "icons/tool-call-card/alt-arrow-down.svg";
const ARROW_DOWN: &str = "icons/tool-call-card/arrow-down.svg";
const COPY: &str = "icons/tool-call-card/copy.svg";
const COPY_CHECK: &str = "icons/tool-call-card/copy-check.svg";
const DANGER_CIRCLE: &str = "icons/tool-call-card/danger-circle.svg";
const LAYERS: &str = "icons/tool-call-card/layers-minimalistic.svg";
/// Pink's glyph for a tool it has no kind for.
const TOOLBOX: &str = "icons/tool-call-card/toolbox.svg";
/// The polychrome file-type artwork in a file badge's well (Pink
/// `file_icons`, vscode-symbols; dark variants lifted like Pink's
/// `dark_icon_svg`).
const FILE_RUST: &str = "icons/tool-call-card/file-rust.svg";
const FILE_RUST_DARK: &str = "icons/tool-call-card/file-rust-dark.svg";
const FILE_DOCUMENT: &str = "icons/tool-call-card/file-document.svg";
const FILE_DOCUMENT_DARK: &str = "icons/tool-call-card/file-document-dark.svg";

type OpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

/// A call's recorded lifecycle: still running, settled, or failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error,
}

/// What a call does, in each tense its lifecycle needs, and how several of
/// them are counted (Pink `transcript/activity.rs` `verbs`, `counted`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verb {
    base: &'static str,
    past: &'static str,
    gerund: &'static str,
    count: Count,
}

/// The counted object after a group's verb: `2 files`, `once`, `3 times`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Count {
    Noun(&'static str, &'static str),
    Times,
}

impl Verb {
    pub const RUN: Verb = Verb::new("run", "ran", "running", Count::Noun("command", "commands"));
    pub const READ: Verb = Verb::new("read", "read", "reading", Count::Noun("file", "files"));
    pub const EDIT: Verb = Verb::new("edit", "edited", "editing", Count::Noun("file", "files"));
    pub const SEARCH: Verb = Verb::new("search", "searched", "searching", Count::Times);
    pub const FETCH: Verb = Verb::new("fetch", "fetched", "fetching", Count::Noun("page", "pages"));
    pub const CALL: Verb = Verb::new("call", "called", "calling", Count::Noun("tool", "tools"));

    const fn new(
        base: &'static str,
        past: &'static str,
        gerund: &'static str,
        count: Count,
    ) -> Self {
        Self {
            base,
            past,
            gerund,
            count,
        }
    }

    /// Any other verb, counted as `one` or `many`.
    pub const fn custom(
        base: &'static str,
        past: &'static str,
        gerund: &'static str,
        one: &'static str,
        many: &'static str,
    ) -> Self {
        Self::new(base, past, gerund, Count::Noun(one, many))
    }

    /// The leading words for `status`: `Ran`, `Running`, `Failed to run`.
    fn lead(self, status: ToolCallStatus) -> String {
        match status {
            ToolCallStatus::Success => capitalize(self.past),
            ToolCallStatus::Running => capitalize(self.gerund),
            ToolCallStatus::Error => format!("Failed to {}", self.base),
        }
    }

    fn counted(self, n: usize) -> String {
        match self.count {
            Count::Noun(one, many) => format!("{n} {}", if n == 1 { one } else { many }),
            Count::Times if n == 1 => "once".into(),
            Count::Times => format!("{n} times"),
        }
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// `a, b and c`.
fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// A sentence's runs, plus the plain text a screen reader hears (Pink
/// `activity::Sentence`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sentence {
    pub(crate) segments: Vec<Segment>,
    pub(crate) text: String,
}

impl Sentence {
    pub(crate) fn plain(text: impl Into<String>) -> Self {
        Self {
            segments: Vec::new(),
            text: String::new(),
        }
        .words(text)
    }

    fn push(mut self, segment: Segment, spoken: &str) -> Self {
        if !self.text.is_empty() {
            self.text.push(' ');
        }
        self.text.push_str(spoken);
        self.segments.push(segment);
        self
    }

    /// Words after words join one run, so their space is the font's own.
    fn words(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        if text.is_empty() {
            return self;
        }
        if let Some(Segment::Text(last)) = self.segments.last_mut() {
            *last = format!("{last} {text}").into();
            self.text.push(' ');
            self.text.push_str(&text);
            return self;
        }
        self.push(Segment::Text(text.clone().into()), &text)
    }
}

/// A JSON value for arguments and results, printed like
/// `JSON.stringify(value, null, 2)`.
#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(SharedString),
    Array(Vec<Json>),
    /// Fields in insertion order.
    Object(Vec<(SharedString, Json)>),
}

impl Json {
    pub fn object<K: Into<SharedString>>(fields: impl IntoIterator<Item = (K, Json)>) -> Self {
        Json::Object(fields.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    pub fn array<V: Into<Json>>(items: impl IntoIterator<Item = V>) -> Self {
        Json::Array(items.into_iter().map(Into::into).collect())
    }

    /// Parses JSON text; `None` when it isn't valid JSON.
    pub fn parse(text: &str) -> Option<Json> {
        let mut parser = Parser { src: text, at: 0 };
        parser.skip_ws();
        let value = parser.value()?;
        parser.skip_ws();
        (parser.at == text.len()).then_some(value)
    }

    /// `JSON.stringify(value, null, 2)`.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        write_json(self, 0, &mut out);
        out
    }
}

impl From<&str> for Json {
    fn from(s: &str) -> Self {
        Json::String(SharedString::from(s.to_string()))
    }
}

impl From<String> for Json {
    fn from(s: String) -> Self {
        Json::String(s.into())
    }
}

impl From<SharedString> for Json {
    fn from(s: SharedString) -> Self {
        Json::String(s)
    }
}

impl From<bool> for Json {
    fn from(b: bool) -> Self {
        Json::Bool(b)
    }
}

macro_rules! json_number {
    ($($t:ty),*) => {$(
        impl From<$t> for Json {
            fn from(n: $t) -> Self {
                Json::Number(n as f64)
            }
        }
    )*};
}
json_number!(i32, i64, u32, u64, usize, f32, f64);

impl<T: Into<Json>> From<Vec<T>> for Json {
    fn from(items: Vec<T>) -> Self {
        Json::array(items)
    }
}

impl<T: Into<Json>> From<Option<T>> for Json {
    fn from(value: Option<T>) -> Self {
        value.map_or(Json::Null, Into::into)
    }
}

/// What a code segment shows, verbatim in mono like Pink's tool output:
/// JSON pretty-printed, or plain text. Text that holds JSON is
/// pretty-printed too.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolPayload {
    Json(Json),
    Text(SharedString),
}

impl ToolPayload {
    fn text(&self) -> String {
        match self {
            ToolPayload::Json(value) => value.pretty(),
            ToolPayload::Text(text) => {
                let trimmed = text.trim();
                if (trimmed.starts_with('{') || trimmed.starts_with('['))
                    && let Some(value) = Json::parse(trimmed)
                {
                    return value.pretty();
                }
                text.to_string()
            }
        }
    }
}

impl From<Json> for ToolPayload {
    fn from(value: Json) -> Self {
        ToolPayload::Json(value)
    }
}

impl From<&str> for ToolPayload {
    fn from(text: &str) -> Self {
        ToolPayload::Text(SharedString::from(text.to_string()))
    }
}

impl From<String> for ToolPayload {
    fn from(text: String) -> Self {
        ToolPayload::Text(text.into())
    }
}

impl From<SharedString> for ToolPayload {
    fn from(text: SharedString) -> Self {
        ToolPayload::Text(text)
    }
}

/// JavaScript's `String(number)` for the values JSON can hold.
fn js_number(n: f64) -> String {
    if n == 0. {
        return "0".into();
    }
    if n.fract() == 0. && n.abs() < 1e21 {
        return format!("{n:.0}");
    }
    format!("{n}")
}

fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_json(value: &Json, depth: usize, out: &mut String) {
    let indent = |depth: usize| "  ".repeat(depth);
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Json::Number(n) if n.is_finite() => out.push_str(&js_number(*n)),
        Json::Number(_) => out.push_str("null"),
        Json::String(s) => out.push_str(&quote(s)),
        Json::Array(items) if items.is_empty() => out.push_str("[]"),
        Json::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&indent(depth + 1));
                write_json(item, depth + 1, out);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&indent(depth));
            out.push(']');
        }
        Json::Object(fields) if fields.is_empty() => out.push_str("{}"),
        Json::Object(fields) => {
            out.push_str("{\n");
            for (i, (key, item)) in fields.iter().enumerate() {
                out.push_str(&indent(depth + 1));
                out.push_str(&quote(key));
                out.push_str(": ");
                write_json(item, depth + 1, out);
                out.push_str(if i + 1 < fields.len() { ",\n" } else { "\n" });
            }
            out.push_str(&indent(depth));
            out.push('}');
        }
    }
}

/// A small recursive-descent JSON parser for payloads passed as text.
struct Parser<'a> {
    src: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.at).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, word: &str) -> bool {
        if self.src[self.at..].starts_with(word) {
            self.at += word.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Option<Json> {
        self.skip_ws();
        match self.peek()? {
            b'{' => {
                self.at += 1;
                let mut fields = Vec::new();
                self.skip_ws();
                if self.eat("}") {
                    return Some(Json::Object(fields));
                }
                loop {
                    self.skip_ws();
                    let key = self.string()?;
                    self.skip_ws();
                    if !self.eat(":") {
                        return None;
                    }
                    let value = self.value()?;
                    fields.push((key.into(), value));
                    self.skip_ws();
                    if self.eat(",") {
                        continue;
                    }
                    return self.eat("}").then_some(Json::Object(fields));
                }
            }
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.eat("]") {
                    return Some(Json::Array(items));
                }
                loop {
                    items.push(self.value()?);
                    self.skip_ws();
                    if self.eat(",") {
                        continue;
                    }
                    return self.eat("]").then_some(Json::Array(items));
                }
            }
            b'"' => self.string().map(Json::from),
            b't' => self.eat("true").then_some(Json::Bool(true)),
            b'f' => self.eat("false").then_some(Json::Bool(false)),
            b'n' => self.eat("null").then_some(Json::Null),
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<Json> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        let digits = |p: &mut Self| {
            let from = p.at;
            while matches!(p.peek(), Some(b'0'..=b'9')) {
                p.at += 1;
            }
            p.at > from
        };
        if !digits(self) {
            return None;
        }
        if self.peek() == Some(b'.') {
            self.at += 1;
            if !digits(self) {
                return None;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !digits(self) {
                return None;
            }
        }
        self.src[start..self.at].parse().ok().map(Json::Number)
    }

    fn string(&mut self) -> Option<String> {
        if !self.eat("\"") {
            return None;
        }
        let mut out = String::new();
        let mut chars = self.src[self.at..].char_indices();
        while let Some((i, ch)) = chars.next() {
            match ch {
                '"' => {
                    self.at += i + 1;
                    return Some(out);
                }
                '\\' => {
                    let (_, esc) = chars.next()?;
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hex4 = |chars: &mut std::str::CharIndices| {
                                let mut code = 0u32;
                                for _ in 0..4 {
                                    code = code * 16 + chars.next()?.1.to_digit(16)?;
                                }
                                Some(code)
                            };
                            let high = hex4(&mut chars)?;
                            let code = if (0xd800..0xdc00).contains(&high) {
                                // A surrogate pair: `😀`
                                if chars.next()?.1 != '\\' || chars.next()?.1 != 'u' {
                                    return None;
                                }
                                let low = hex4(&mut chars)?;
                                0x10000 + ((high - 0xd800) << 10) + (low.checked_sub(0xdc00)?)
                            } else {
                                high
                            };
                            out.push(char::from_u32(code)?);
                        }
                        _ => return None,
                    }
                }
                c if (c as u32) < 0x20 => return None,
                c => out.push(c),
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Code segments (Pink `call_block` / `tool_detail` / `detail_body`)
// ---------------------------------------------------------------------------

/// Soft-wraps one raw line into `cols`-char chunks so a long command stays
/// readable instead of ellipsizing (Pink `wrap_cols`).
fn wrap_cols(line: &str, cols: usize) -> Vec<SharedString> {
    if line.chars().count() <= cols {
        return vec![SharedString::from(line.to_owned())];
    }
    line.chars()
        .collect::<Vec<_>>()
        .chunks(cols)
        .map(|chunk| SharedString::from(chunk.iter().collect::<String>()))
        .collect()
}

/// The one-line form of a row's detail (Pink `single_line`).
fn single_line(text: &str) -> SharedString {
    text.split_whitespace().collect::<Vec<_>>().join(" ").into()
}

/// One segment of a row's code surface: what was asked (slot 0) or what
/// came back (slot 1).
#[derive(Clone, Debug)]
struct Section {
    slot: usize,
    label: &'static str,
    copy_label: &'static str,
    lines: Vec<SharedString>,
    truncated_by: usize,
    /// The whole text, for copy.
    raw: SharedString,
}

impl Section {
    /// Pink's output block: wrapped at 80 columns, trailing blank lines
    /// trimmed, the first `cap` lines kept with a counted tail.
    fn new(
        slot: usize,
        label: &'static str,
        copy_label: &'static str,
        text: String,
        cap: usize,
    ) -> Option<Self> {
        let mut lines: Vec<SharedString> = text
            .lines()
            .flat_map(|line| wrap_cols(line, CALL_WRAP_COLS))
            .collect();
        while lines.last().is_some_and(|line| line.trim().is_empty()) {
            lines.pop();
        }
        if lines.is_empty() {
            return None;
        }
        let truncated_by = lines.len().saturating_sub(cap);
        lines.truncate(cap);
        Some(Section {
            slot,
            label,
            copy_label,
            lines,
            truncated_by,
            raw: text.into(),
        })
    }

    /// The scrolling body: lines plus the counted tail, capped at 252.
    fn viewport(&self) -> f32 {
        let rows = self.lines.len() + usize::from(self.truncated_by > 0);
        (rows as f32 * OUTPUT_LINE_HEIGHT + OUTPUT_BODY_PAD).min(OUTPUT_VIEWPORT_MAX_HEIGHT)
    }

    /// A framed body or a segment of a merged frame: its 1px top and bottom
    /// edges (drawn or transparent), the 28px header and the viewport.
    fn height(&self) -> f32 {
        2.0 + OUTPUT_CODE_HEADER_HEIGHT + self.viewport()
    }
}

/// What an open row adds under its header: the separator, the code surface
/// (two segments share one frame split by a hairline) and the "Show full
/// output" row. Analytic, like Pink's fold heights, so nothing is measured.
fn body_height(sections: &[Section], affordance: bool) -> f32 {
    let detail = match sections {
        [one] => DETAIL_SEPARATOR + one.height(),
        [asked, answer] => DETAIL_SEPARATOR + 2.0 + asked.height() + 1.0 + answer.height(),
        _ => 0.0,
    };
    detail
        + if affordance {
            BLOB_AFFORDANCE_HEIGHT
        } else {
            0.0
        }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// One fold's clock (Pink `FoldState`): the user's choice, and the height
/// a tween started from.
#[derive(Clone, Copy, Default)]
struct Fold {
    open: Option<bool>,
    toggled_at: Option<Instant>,
    from: f32,
    rendered: f32,
    rendered_open: Option<bool>,
}

impl Fold {
    /// Any change of the effective open state – a click, or the live
    /// default flipping – tweens from the height last painted.
    fn sync(&mut self, open: bool, now: Instant) {
        if self.rendered_open.is_some_and(|previous| previous != open) {
            self.from = self.rendered;
            self.toggled_at = Some(now);
        }
        self.rendered_open = Some(open);
    }

    /// Time since the toggle while the tween is armed (Pink
    /// `FOLD_TWEEN_WINDOW`).
    fn elapsed(&self, now: Instant, reduced: bool) -> Option<Duration> {
        let at = self.toggled_at.filter(|_| !reduced)?;
        let elapsed = now.saturating_duration_since(at);
        (elapsed < motion::FOLD_TWEEN_WINDOW.mul_f32(motion::speed_scale())).then_some(elapsed)
    }

    /// `(height, disclosure progress, body alpha, animating)` on Pink's one
    /// fold clock.
    fn step(
        &mut self,
        open: bool,
        target: f32,
        now: Instant,
        reduced: bool,
    ) -> (f32, f32, f32, bool) {
        self.sync(open, now);
        let elapsed = self.elapsed(now, reduced);
        let (progress, alpha) = motion::fold_progress(open, elapsed);
        let eased = if open { progress } else { 1.0 - progress };
        let height = match elapsed {
            Some(_) => motion::lerp(self.from, target, eased),
            None => target,
        };
        self.rendered = height;
        (height, progress, alpha, elapsed.is_some())
    }
}

#[derive(Default)]
struct RowState {
    fold: Fold,
    /// "Show full output" was pressed.
    full: bool,
}

/// Everything a card or a group remembers, keyed by its id.
#[derive(Default)]
struct ToolState {
    rows: HashMap<SharedString, RowState>,
    /// `Show N more` was pressed: the group shows every call.
    more_open: bool,
    /// The `(row, segment)` whose copy action reads "Copied".
    copied: Option<(SharedString, usize)>,
    copy_reset: Option<Task<()>>,
    // The group's own fold and arrivals
    fold: Fold,
    mounted: bool,
    reveals: HashMap<SharedString, Option<Instant>>,
    header_started_at: Option<Instant>,
    shimmer_started_at: Option<Instant>,
}

/// A row's fold, resolved for this frame.
struct Geometry {
    /// The row's header: the 28 pt pitch scaled, or the chip header.
    pitch: f32,
    /// The failure line under a failed rail row, else zero.
    failure: f32,
    /// Header plus failure line: the row's height while folded.
    base: f32,
    body: f32,
    open: bool,
    expandable: bool,
    /// The body stays mounted while a close tween shrinks over it.
    mounted: bool,
    animating: bool,
    chevron: f32,
    alpha: f32,
    sections: Vec<Section>,
    affordance: Option<SharedString>,
}

/// Where a row sits on its group's rail, and how far its arrival has drawn.
#[derive(Clone, Copy)]
struct Rail {
    has_predecessor: bool,
    continues: bool,
    reveal: f32,
    connector: f32,
    continuation: f32,
}

/// Handed to a card by its group: the group's state, the row's key and its
/// resolved fold.
struct Slot {
    state: Entity<ToolState>,
    key: SharedString,
    geometry: Geometry,
    rail: Rail,
}

// ---------------------------------------------------------------------------
// Card
// ---------------------------------------------------------------------------

/// One tool call: a rail row inside a [`ToolCallGroup`], Pink's chip card on
/// its own.
#[derive(IntoElement)]
pub struct ToolCallCard {
    id: ElementId,
    verb: Verb,
    /// The sentence's runs after the verb.
    runs: Vec<Segment>,
    file: Option<SharedString>,
    icon: Option<SharedString>,
    status: ToolCallStatus,
    args: Option<ToolPayload>,
    command: bool,
    result: Option<ToolPayload>,
    error: Option<SharedString>,
    default_open: Option<bool>,
    open: Option<bool>,
    on_open_change: Option<OpenHandler>,
    max_lines: usize,
    slot: Option<Slot>,
}

impl ToolCallCard {
    /// `verb` leads the call's sentence in the tense `status` gives it
    /// (`Ran`, `Running`, `Failed to run`); add what it acted on with
    /// [`code`](Self::code), [`object`](Self::object), [`words`](Self::words)
    /// or [`file`](Self::file).
    pub fn new(id: impl Into<ElementId>, verb: Verb, status: ToolCallStatus) -> Self {
        Self {
            id: id.into(),
            verb,
            runs: Vec::new(),
            file: None,
            icon: None,
            status,
            args: None,
            command: false,
            result: None,
            error: None,
            default_open: None,
            open: None,
            on_open_change: None,
            max_lines: OUTPUT_DETAIL_MAX_LINES,
            slot: None,
        }
    }

    /// A command, pattern or tool name, drawn as inline code.
    pub fn code(mut self, text: impl Into<SharedString>) -> Self {
        let text: SharedString = text.into();
        self.runs.push(Segment::Code(single_line(&text)));
        self
    }

    /// A page, path or other noun, in the primary tone.
    pub fn object(mut self, text: impl Into<SharedString>) -> Self {
        let text: SharedString = text.into();
        self.runs.push(Segment::Object(single_line(&text)));
        self
    }

    /// Connecting words ("for", "in").
    pub fn words(mut self, text: impl Into<SharedString>) -> Self {
        self.runs.push(Segment::Text(text.into()));
        self
    }

    /// The file the call reads or writes ("src/cart/total.rs"), ending its
    /// sentence as Pink's file badge: the file-type artwork beside the
    /// file's name. Groups count distinct files.
    pub fn file(mut self, path: impl Into<SharedString>) -> Self {
        self.file = Some(path.into());
        self
    }

    /// The row's 14px glyph (an SVG path under `assets/`); defaults to Pink's
    /// toolbox. A failed call shows a danger circle instead.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// What the tool was called with, shown as "Invocation".
    pub fn args(mut self, args: impl Into<ToolPayload>) -> Self {
        self.args = Some(args.into());
        self.command = false;
        self
    }

    /// A shell command, shown as "Command" (Pink labels exec invocations so).
    pub fn command(mut self, command: impl Into<SharedString>) -> Self {
        self.args = Some(ToolPayload::Text(command.into()));
        self.command = true;
        self
    }

    /// What the tool returned (or has printed so far), shown as "Output".
    pub fn result(mut self, result: impl Into<ToolPayload>) -> Self {
        self.result = Some(result.into());
        self
    }

    /// The failure as the tool reported it: its first line leads the failure
    /// line, and an `Error` call without a result shows it as its output.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Start expanded or folded (uncontrolled). By default a running call
    /// with output is open and a settled one is folded, as in pink.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = Some(open);
        self
    }

    /// Expanded state (controlled) – pair with [`on_open_change`](Self::on_open_change).
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Lines the output shows before "… N more lines" and "Show full
    /// output" (default 24, as in Pink).
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = lines.max(1);
        self
    }

    fn icon_path(&self) -> SharedString {
        self.icon.clone().unwrap_or(TOOLBOX.into())
    }

    /// The call's sentence in its current tense; a file ends it.
    pub(crate) fn sentence(&self) -> Sentence {
        let mut sentence = Sentence::plain(self.verb.lead(self.status));
        for run in &self.runs {
            sentence = match run {
                Segment::Text(words) => sentence.words(words.to_string()),
                Segment::Code(text) | Segment::Object(text) => {
                    sentence.push(run.clone(), text.as_ref())
                }
            };
        }
        if let Some(path) = &self.file {
            let name = file_name(path);
            sentence = sentence.push(Segment::Object(name.to_owned().into()), name);
        }
        sentence
    }

    /// What a failed call reported: the first non-empty line of its error
    /// or output (Pink `activity::failure_text`).
    fn failure_text(&self) -> Option<String> {
        let text = self
            .error
            .as_ref()
            .map(|error| error.to_string())
            .or_else(|| self.result.as_ref().map(ToolPayload::text))?;
        text.lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_owned)
    }

    /// Invocation first (what was asked, whole up to Pink's 400-line
    /// ceiling), then the output (what came back, capped until "Show full
    /// output").
    fn sections(&self, full: bool) -> Vec<Section> {
        let cap = if full {
            FULL_OUTPUT_MAX_LINES
        } else {
            self.max_lines
        };
        let asked = self.args.as_ref().and_then(|args| {
            let (label, copy) = if self.command {
                ("Command", "Copy command")
            } else {
                ("Invocation", "Copy invocation")
            };
            Section::new(0, label, copy, args.text(), FULL_OUTPUT_MAX_LINES)
        });
        let answer = self
            .result
            .as_ref()
            .map(ToolPayload::text)
            .or_else(|| {
                (self.status == ToolCallStatus::Error)
                    .then(|| self.error.as_ref().map(|e| e.to_string()))
                    .flatten()
            })
            .and_then(|text| Section::new(1, "Output", "Copy output", text, cap));
        asked.into_iter().chain(answer).collect()
    }

    /// The row's fold for this frame (Pink: `render_tool_group` row heights).
    /// `failure` is the failure line's height should this row show one.
    fn geometry(
        &self,
        row: &mut RowState,
        pitch: f32,
        failure: f32,
        now: Instant,
        reduced: bool,
    ) -> Geometry {
        let sections = self.sections(row.full);
        let affordance = (!row.full
            && sections
                .iter()
                .any(|section| section.slot == 1 && section.truncated_by > 0))
        .then(|| SharedString::from("Show full output"));
        let expandable = !sections.is_empty();
        // A failed call with something to show adds its failure line.
        let failure = if self.status == ToolCallStatus::Error && expandable {
            failure
        } else {
            0.0
        };
        let base = pitch + failure;
        // A streaming call with output defaults open; a settled one folded
        let default_open = self
            .default_open
            .unwrap_or(self.status == ToolCallStatus::Running && self.result.is_some());
        let open = expandable && self.open.or(row.fold.open).unwrap_or(default_open);
        let target = if open {
            body_height(&sections, affordance.is_some())
        } else {
            0.0
        };
        let (body, chevron, alpha, animating) = row.fold.step(open, target, now, reduced);
        Geometry {
            pitch,
            failure,
            base,
            body,
            open,
            expandable,
            mounted: open || animating,
            animating,
            chevron,
            alpha,
            sections,
            affordance,
        }
    }

    /// The rail glyph: the mini spinner while running, a red danger circle
    /// on failure, else the tool's own glyph (Pink `activity_glyph`).
    fn rail_glyph(&self, key: &str, t: &Pink) -> AnyElement {
        match self.status {
            ToolCallStatus::Running => {
                loaders::mini_glyph_spinner(format!("{key}-running"), 2.0, t.glyph)
                    .into_any_element()
            }
            ToolCallStatus::Error => svg()
                .path(DANGER_CIRCLE)
                .flex_none()
                .size(px(ACTIVITY_ICON_SIZE))
                .text_color(t.danger)
                .into_any_element(),
            ToolCallStatus::Success => svg()
                .path(self.icon_path())
                .flex_none()
                .size(px(ACTIVITY_ICON_SIZE))
                .text_color(t.content_muted)
                .into_any_element(),
        }
    }

    /// Opens or folds the row (its header and the failure line's Show
    /// details share it).
    fn toggle(
        &self,
        state: &Entity<ToolState>,
        key: &SharedString,
        open: bool,
    ) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
        let state = state.clone();
        let key = key.clone();
        let controlled = self.open.is_some();
        let on_change = self.on_open_change.clone();
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            if !controlled {
                state.update(cx, |s, cx| {
                    s.rows.entry(key.clone()).or_default().fold.open = Some(!open);
                    cx.notify();
                });
            }
            if let Some(on_change) = &on_change {
                on_change(!open, window, cx);
            }
        }
    }

    /// The clickable header of an expandable row (Pink: the keyed chip
    /// header button in `render_tool_group`), named by its sentence.
    fn header_button(
        &self,
        state: &Entity<ToolState>,
        key: &SharedString,
        geometry: &Geometry,
        rail: bool,
        t: &Pink,
    ) -> Stateful<Div> {
        let ring = t.focus_ring();
        let open = geometry.open;
        let spoken: String = self.sentence().text.chars().take(160).collect();
        div()
            .id(SharedString::from(format!("{key}-row")))
            .role(Role::Button)
            .aria_label(spoken)
            .aria_expanded(open)
            .tab_index(0)
            .rounded(px(radius::CONTROL))
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .h(px(geometry.pitch))
            .flex_none()
            .flex()
            .items_center()
            .cursor_pointer()
            .on_click(self.toggle(state, key, open))
            .child(chip_header_row(
                self,
                rail,
                Some(geometry.chevron),
                key,
                geometry.pitch,
                t,
            ))
    }

    /// The line under a failed call (D63, Pink `tool_failure_line`): what it
    /// reported, then a route to recover. Show details opens the row's
    /// command and output; a command also offers Copy command so it can be
    /// rerun by hand. Nothing here retries or claims the failure resolved.
    fn failure_line(
        &self,
        state: &Entity<ToolState>,
        key: &SharedString,
        geometry: &Geometry,
        t: &Pink,
        cx: &App,
    ) -> AnyElement {
        let scale = geometry.pitch / TOOL_TREE_ROW_HEIGHT;
        let spoken = self.sentence().text;
        let message = self
            .failure_text()
            .unwrap_or_else(|| "The tool reported a failure without output".into());
        let style = activity::RecoveryStyle {
            scale,
            height: TOOL_LINE_HEIGHT * scale,
            radius: radius::CONTROL,
            text: t.content_muted,
        };
        let ring = t.focus_ring();
        let action = |id: SharedString, label: &'static str, aria: String| {
            activity::recovery_action(id.clone(), label.into(), &style)
                .role(Role::Button)
                .aria_label(aria)
                .tab_index(0)
                .focus_visible(move |style| pink::focus_ring(style, ring))
                .hover_fill_fg(
                    format!("tcc-{id}"),
                    t.fill(FillTier::Quaternary),
                    t.fill(FillTier::Tertiary),
                    t.content_muted,
                    t.text,
                )
        };
        let details = if geometry.open {
            "Hide details"
        } else {
            "Show details"
        };
        let mut actions = vec![
            action(
                format!("{key}-failure-details").into(),
                details,
                format!("{details}, {spoken}"),
            )
            .on_click(self.toggle(state, key, geometry.open))
            .into_any_element(),
        ];
        if let (true, Some(ToolPayload::Text(command))) = (self.command, &self.args) {
            let copied = state.read(cx).copied == Some((key.clone(), FAILURE_COPY_SLOT));
            let on_copy = copy_handler(
                state.clone(),
                key.clone(),
                FAILURE_COPY_SLOT,
                command.clone(),
            );
            actions.push(
                action(
                    format!("{key}-failure-copy").into(),
                    if copied { "Copied" } else { "Copy command" },
                    format!("Copy command, {}", single_line(command)),
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    on_copy(window, cx);
                })
                .into_any_element(),
            );
        }
        let label = format!("{spoken}, {message}");
        activity::failure_line(
            message.into(),
            actions,
            &activity::FailureStyle {
                scale,
                height: geometry.failure,
                text_size: TOOL_FAILURE_TEXT_SIZE * scale,
                line_height: TOOL_LINE_HEIGHT * scale,
                message: t.readable_tone(t.danger, 1.0),
            },
        )
        .id(SharedString::from(format!("{key}-failure")))
        .role(Role::Group)
        .aria_label(label)
        .into_any_element()
    }

    /// The open body: the separator, the code surface and the "Show full
    /// output" affordance, fading in with a 6px settle.
    fn panel(
        &self,
        state: &Entity<ToolState>,
        key: &SharedString,
        geometry: &Geometry,
        rail: bool,
        t: &Pink,
        cx: &App,
    ) -> Div {
        let alpha = geometry.alpha;
        let copied = state
            .read(cx)
            .copied
            .as_ref()
            .filter(|(row, _)| row == key)
            .map(|(_, slot)| *slot);
        let separator = || {
            div()
                .h(px(DETAIL_SEPARATOR))
                .flex_none()
                .when(!rail, |line| line.bg(t.hairline(0.06)))
        };
        let segmented = geometry.sections.len() == 2;
        let mut bodies = geometry.sections.iter().map(|section| {
            let on_copy = copy_handler(
                state.clone(),
                key.clone(),
                section.slot,
                section.raw.clone(),
            );
            detail_body(
                section,
                key,
                copied == Some(section.slot),
                !segmented,
                on_copy,
                t,
            )
        });
        let mut panel = div()
            .flex_none()
            .min_w_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(alpha < 1.0, |panel| {
                panel
                    .relative()
                    .top(px(motion::TOOL_BODY_RISE * (1.0 - alpha)))
                    .opacity(alpha)
            });
        if segmented {
            // What was asked and what came back, as two segments of one
            // code surface split by a single hairline
            let asked = bodies.next();
            let answer = bodies.next();
            panel = panel.child(separator()).child(
                code_surface_frame(t)
                    .children(asked)
                    .child(div().h(px(1.0)).flex_none().bg(t.border))
                    .children(answer),
            );
        } else {
            for body in bodies {
                panel = panel.child(separator()).child(body);
            }
        }
        if let Some(label) = geometry.affordance.clone() {
            panel = panel.child(show_full_row(state.clone(), key.clone(), label, t));
        }
        panel
    }

    /// A row on its group's rail (Pink `render_tool_group` rows and
    /// `tool_chip` in rail mode): the header on the pitch, the failure line
    /// under a failed call, then the open body.
    fn render_rail(self, slot: Slot, t: &Pink, cx: &App) -> AnyElement {
        let Slot {
            state,
            key,
            geometry,
            rail,
        } = slot;
        let pitch = geometry.pitch;
        let row_height = geometry.base + geometry.body;
        let content_reveal = motion::tool_connector_parts(rail.connector, rail.has_predecessor).1;
        let glyph = self.rail_glyph(&key, t);
        let failure =
            (geometry.failure > 0.0).then(|| self.failure_line(&state, &key, &geometry, t, cx));
        let card = div()
            .ml(px(ACTIVITY_TEXT_GAP))
            .h(px(row_height))
            .min_w_0()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            .map(|card| {
                if geometry.expandable {
                    card.child(self.header_button(&state, &key, &geometry, true, t))
                } else {
                    card.child(
                        div()
                            .h(px(pitch))
                            .flex_none()
                            .flex()
                            .items_center()
                            .child(chip_header_row(&self, true, None, &key, pitch, t)),
                    )
                }
            })
            .children(failure)
            .when(geometry.mounted, |card| {
                card.child(self.panel(&state, &key, &geometry, true, t, cx))
            });
        let card = div()
            .min_w_0()
            .flex_1()
            .child(card)
            .when(content_reveal < 1.0, |card| {
                card.relative()
                    .top(px(4.0 * (1.0 - content_reveal)))
                    .opacity(content_reveal)
            });
        let row = div()
            .w_full()
            .h(px(row_height))
            .flex_none()
            .flex()
            .flex_row()
            .child(activity_rail(
                glyph,
                rail.has_predecessor,
                rail.continues,
                rail.connector,
                rail.continuation,
                pitch,
                t,
            ))
            .child(card);
        reveal_tool_row(row.into_any_element(), row_height, rail.reveal)
    }

    /// Pink's chip card (Pink `tool_chip` / `subagent_chip` card mode): a
    /// 30px bordered card, `hairline(0.07)` on `ink(0.03)`, centered in a
    /// 38px row.
    fn render_card(
        self,
        state: Entity<ToolState>,
        key: SharedString,
        geometry: Geometry,
        t: &Pink,
        cx: &App,
    ) -> Div {
        let base = geometry.base;
        let row_height = base + geometry.body;
        let card = div()
            .my(px((base - CHIP_CARD_HEIGHT) / 2.0))
            .min_w_0()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(radius::LG))
            .border_1()
            .border_color(t.hairline(0.07))
            .bg(t.ink(0.03))
            .h(px(row_height - base + CHIP_CARD_HEIGHT));
        let card = if geometry.expandable {
            card.child(self.header_button(&state, &key, &geometry, false, t))
                .when(geometry.mounted, |card| {
                    card.child(self.panel(&state, &key, &geometry, false, t, cx))
                })
        } else {
            card.justify_center().child(chip_header_row(
                &self,
                false,
                None,
                &key,
                CHIP_HEADER_HEIGHT,
                t,
            ))
        };
        div()
            .w_full()
            .h(px(row_height))
            .flex()
            .flex_row()
            .child(div().min_w_0().flex_1().child(card))
    }
}

impl RenderOnce for ToolCallCard {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        if let Some(slot) = self.slot.take() {
            return self.render_rail(slot, &t, cx);
        }
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| ToolState::default(),
        );
        let key = SharedString::from(self.id.to_string());
        let now = Instant::now();
        let reduced = motion::reduced_motion(cx);
        let geometry = state.update(cx, |s, _| {
            let row = s.rows.entry(key.clone()).or_default();
            let mut geometry = self.geometry(row, CHIP_HEADER_HEIGHT, 0.0, now, reduced);
            // The card centres its header in Pink's taller chip row.
            geometry.base = CHIP_HEIGHT;
            geometry
        });
        if geometry.animating {
            window.request_animation_frame();
        }
        motion::keep_alive(window);
        let id = self.id.clone();
        div()
            .id(id)
            .w_full()
            .min_w_0()
            .font_family(pink::SANS)
            .child(self.render_card(state, key, geometry, &t, cx))
            .into_any_element()
    }
}

/// The row's one line (Pink `chip_header_row`): the call's sentence (Pink
/// `activity::call_sentence`, drawn by the `pink-tool-activity` package)
/// with a file ending it as Pink's file badge. On the rail the verb and
/// connecting words are `content_muted`, brightening to `text` with the
/// row's hover, then a spacer and an 18px tile holding the 12px chevron at
/// half strength until hovered. In a card: an 18px icon tile first and the
/// mini spinner trailing while the call runs. A failure is the sentence's
/// own words ("Failed to run") and one red glyph, never a red row.
fn chip_header_row(
    card: &ToolCallCard,
    rail: bool,
    trail: Option<f32>,
    key: &str,
    height: f32,
    t: &Pink,
) -> Div {
    let running = card.status == ToolCallStatus::Running;
    let hover_text = rail && trail.is_some();
    let scale = if rail {
        height / TOOL_TREE_ROW_HEIGHT
    } else {
        1.0
    };
    let text = t.text;
    let mut segments = card.sentence().segments;
    // A file the call names stays its badge.
    let badge = card.file.as_ref().map(|path| {
        segments.pop();
        file_badge(path, hover_text, t)
    });
    let line =
        activity::sentence(&segments, &activity::sentence_style(t, scale)).text_color(if rail {
            t.content_muted
        } else {
            t.text.opacity(0.85)
        });
    let line = if hover_text {
        line.id("tool-label")
            .group_hover(HEADER_GROUP, move |style| style.text_color(text))
            .into_any_element()
    } else {
        line.into_any_element()
    };
    div()
        .group(HEADER_GROUP)
        .h(px(height))
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(if rail { 0.0 } else { 8.0 }))
        .text_size(px(TOOL_TEXT_SIZE * scale))
        .line_height(px(TOOL_LINE_HEIGHT * scale))
        .when(!rail, |row| {
            row.child(
                div()
                    .size(px(18.0))
                    .flex_none()
                    .rounded(px(radius::SM))
                    .bg(t.ink(0.08))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(card.icon_path())
                            .flex_none()
                            .size(px(12.0))
                            .text_color(t.content_muted),
                    ),
            )
        })
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(activity::SEGMENT_GAP * scale))
                .child(line)
                .children(badge),
        )
        // Trailing accessories share one right-aligned column
        .child(div().flex_1().min_w(px(4.0)))
        .when(running && !rail, |row| {
            row.child(div().flex_none().child(loaders::mini_glyph_spinner(
                format!("{key}-running"),
                2.0,
                t.glyph,
            )))
        })
        .when_some(trail, |row, progress| {
            let mut chevron = svg()
                .path(ALT_ARROW_RIGHT)
                .flex_none()
                .size(px(DISCLOSURE_CHEVRON_SIZE * scale))
                .with_transformation(Transformation::rotate(radians(FRAC_PI_2 * progress)))
                .text_color(t.content_faint);
            if rail {
                chevron = chevron.group_hover(HEADER_GROUP, move |style| style.text_color(text));
            }
            row.child(
                div()
                    .size(px(18.0))
                    .flex_none()
                    .opacity(0.5)
                    .group_hover(HEADER_GROUP, |style| style.opacity(1.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(t.readable_muted(0.8))
                    .child(chevron),
            )
        })
}

/// The file-type artwork for a path (Pink `file_icons::asset_path`, for
/// the kinds this component carries; anything else is Pink's generic
/// document).
fn file_icon(path: &str, dark: bool) -> &'static str {
    let rust = file_name(path)
        .rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("rs"));
    match (rust, dark) {
        (true, true) => FILE_RUST_DARK,
        (true, false) => FILE_RUST,
        (false, true) => FILE_DOCUMENT_DARK,
        (false, false) => FILE_DOCUMENT,
    }
}

/// A badge names the last path component (Pink `file_badge_name`).
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
        .unwrap_or(path)
}

/// Pink's file badge (`file_badges::render_file_badge`, no open route),
/// drawn by the `pink-file-badge` package: a 22px r6 chip on `ink(0.06)`
/// holding the file-type artwork in a 20px well and the file name in `text`
/// at 85%, brightening with the row's hover.
fn file_badge(path: &str, hover_text: bool, t: &Pink) -> AnyElement {
    let text = t.text;
    // Pink `file_icons::well_bg`: a counter-shade seat for the artwork
    let well = pink::grey(if t.dark { 0 } else { 255 }).opacity(0.16);
    let chip = file_badge::badge(&file_badge::BadgeStyle {
        background: t.ink(0.06),
        text: t.text.opacity(0.85),
    })
    .child(file_badge::well(
        well,
        img(file_icon(path, t.dark))
            .flex_none()
            .size(px(file_badge::GLYPH)),
    ))
    .child(file_badge::name(file_name(path).to_owned()));
    if hover_text {
        chip.id("tool-file-badge")
            .group_hover(HEADER_GROUP, move |style| style.text_color(text))
            .into_any_element()
    } else {
        chip.into_any_element()
    }
}

/// The tree beside a row (Pink `activity_rail`): a 1px `hairline(0.12)`
/// ribbon down the trunk at x 12.5, bending on a 6px elbow into a branch
/// that ends at the 14px glyph at x 32. A new row draws its incoming trunk,
/// then the elbow, then shows its glyph; the trunk continues past the row
/// (alongside its open body) as the next row arrives. One fill paints every
/// segment, so joints are not darkened twice.
fn activity_rail(
    glyph: AnyElement,
    has_predecessor: bool,
    continues: bool,
    reveal: f32,
    continuation_reveal: f32,
    row_height: f32,
    t: &Pink,
) -> Div {
    let color = t.hairline(0.12);
    let (incoming_reveal, branch_reveal) = motion::tool_connector_parts(reveal, has_predecessor);
    div()
        .relative()
        .w(px(ACTIVITY_GUTTER_WIDTH))
        .flex_none()
        .child(
            canvas(
                move |_, _, _| (),
                move |bounds, _, window, _| {
                    let x = bounds.origin.x + px(ACTIVITY_TRUNK_X);
                    let branch_y = bounds.origin.y + px(row_height / 2.0);
                    let bend_y = branch_y - px(ACTIVITY_BEND_RADIUS);
                    // Union the ribbons before painting: stroke tessellation
                    // blends intersections twice, even in a single path
                    let mut tree = PathBuilder::fill().with_style(PathStyle::Fill(
                        FillOptions::default().with_fill_rule(FillRule::NonZero),
                    ));
                    if incoming_reveal > 0.0 {
                        let mut bottom = point(
                            x,
                            bounds.origin.y
                                + px((row_height / 2.0 - ACTIVITY_BEND_RADIUS) * incoming_reveal),
                        );
                        if incoming_reveal >= 1.0 && continues && continuation_reveal > 0.0 {
                            let continuation_height = (f32::from(bounds.size.height)
                                - (row_height / 2.0 - ACTIVITY_BEND_RADIUS))
                                .max(0.0);
                            bottom =
                                point(x, bend_y + px(continuation_height * continuation_reveal));
                        }
                        activity_ribbon(&mut tree, &[point(x, bounds.origin.y), bottom]);
                    }
                    if branch_reveal > 0.0 {
                        let points: Vec<_> = activity_branch_points(branch_reveal)
                            .into_iter()
                            .map(|p| point(x + px(p.x), bend_y + px(p.y)))
                            .collect();
                        activity_ribbon(&mut tree, &points);
                    }
                    if let Ok(path) = tree.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(
            div()
                .absolute()
                .left(px(ACTIVITY_ICON_LEFT))
                .top(px(row_height / 2.0 - ACTIVITY_ICON_SIZE / 2.0))
                .size(px(ACTIVITY_ICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .opacity(branch_reveal)
                .child(glyph),
        )
}

/// The elbow and branch, revealed by distance along the path so the speed
/// doesn't jump at their junction (Pink `activity_branch_points`).
fn activity_branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<Point<f32>> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.0;
            point(
                ACTIVITY_BEND_RADIUS * t * t,
                ACTIVITY_BEND_RADIUS * (2.0 * t - t * t),
            )
        })
        .collect();
    path.push(point(
        ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X,
        ACTIVITY_BEND_RADIUS,
    ));
    if progress >= 1.0 {
        return path;
    }
    let lengths: Vec<f32> = path
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .collect();
    let mut remaining = lengths.iter().sum::<f32>() * progress.clamp(0.0, 1.0);
    let mut visible = vec![path[0]];
    for (pair, length) in path.windows(2).zip(lengths) {
        if remaining <= 0.0 {
            break;
        }
        let t = (remaining / length).min(1.0);
        visible.push(point(
            motion::lerp(pair[0].x, pair[1].x, t),
            motion::lerp(pair[0].y, pair[1].y, t),
        ));
        remaining -= length;
    }
    visible
}

/// A 1px ribbon contour along `points`; the nonzero fill unions crossing
/// contours (Pink `activity_ribbon`).
fn activity_ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
    if points.len() < 2 {
        return;
    }
    let mut left = Vec::with_capacity(points.len());
    let mut right = Vec::with_capacity(points.len());
    for (ix, p) in points.iter().enumerate() {
        let a = points[ix.saturating_sub(1)];
        let b = points[(ix + 1).min(points.len() - 1)];
        let dx = f32::from(b.x - a.x);
        let dy = f32::from(b.y - a.y);
        let length = dx.hypot(dy).max(0.0001);
        let normal = point(px(-dy / length * 0.5), px(dx / length * 0.5));
        left.push(*p + normal);
        right.push(*p - normal);
    }
    path.move_to(left[0]);
    for p in left.iter().skip(1).chain(right.iter().rev()) {
        path.line_to(*p);
    }
    path.close();
}

/// Clips a newly arrived row to its committed height while it reveals
/// (Pink `reveal_tool_row`); the fade and lift ride the row's content, so the
/// rail stays at full contrast while it draws.
fn reveal_tool_row(row: AnyElement, height: f32, progress: f32) -> AnyElement {
    if progress >= 1.0 {
        return row;
    }
    div()
        .w_full()
        .h(px(height * progress))
        .flex_none()
        .overflow_hidden()
        .child(row)
        .into_any_element()
}

/// The code surface frame (Pink `code_surface_frame`): the kit's code
/// surface, r10 with a 1px edge on `ink(0.035)`.
fn code_surface_frame(t: &Pink) -> Div {
    pink::code_surface(t)
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .overflow_hidden()
}

/// One code segment (Pink `detail_body`, output case): the header with its
/// copy action, then verbatim 12.5/18 mono lines in `text` that scroll past
/// 252px, and the counted tail. A standalone segment draws its own frame; a
/// segment of a merged frame keeps the same box with transparent edges.
fn detail_body(
    section: &Section,
    key: &str,
    copied: bool,
    framed: bool,
    on_copy: impl Fn(&mut Window, &mut App) + 'static,
    t: &Pink,
) -> Div {
    let body = div().w_full().min_w_0().flex().flex_col().overflow_hidden();
    let body = if framed {
        body.rounded(px(radius::LG))
            .border_1()
            .border_color(t.border)
            .bg(t.ink(0.035))
    } else {
        body.border_y_1().border_color(gpui::transparent_black())
    };
    let text = t.text;
    // Pink `code_surface_header`, drawn by the `pink-code-block` package:
    // 28px, an 11px medium `content_muted` label, the copy action trailing,
    // a barely raised `ink(0.02)` over the body.
    body.child(
        code_block::surface_header(
            section.label,
            vec![
                copy_action(
                    SharedString::from(format!("{key}-copy-{}", section.slot)),
                    section.copy_label,
                    copied,
                    on_copy,
                    t,
                )
                .into_any_element(),
            ],
            code_block::frame_style(t),
        )
        .font_family(pink::SANS),
    )
    .child(
        // Long lines scroll sideways instead of ending in an ellipsis
        div()
            .id(SharedString::from(format!("{key}-scroll-{}", section.slot)))
            .h(px(section.viewport()))
            .overflow_scroll()
            .py(px(OUTPUT_BODY_PAD / 2.0))
            .px(px(CODE_PADDING_X))
            .flex()
            .flex_col()
            .items_start()
            .font_family(pink::MONO)
            .text_size(px(pink::text::CODE))
            .line_height(px(OUTPUT_LINE_HEIGHT))
            .children(section.lines.iter().map(move |line| {
                div()
                    .h(px(OUTPUT_LINE_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .whitespace_nowrap()
                    .text_color(text)
                    .child(line.clone())
            }))
            .when(section.truncated_by > 0, |block| {
                block.child(more_lines_row(section.truncated_by, t))
            }),
    )
}

/// The counted tail under a capped segment (Pink `more_lines_row`).
fn more_lines_row(truncated_by: usize, t: &Pink) -> Div {
    div()
        .h(px(OUTPUT_LINE_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .text_size(px(pink::text::CODE))
        .text_color(t.content_faint)
        .child(SharedString::from(match truncated_by {
            1 => "… 1 more line".to_string(),
            n => format!("… {n} more lines"),
        }))
}

/// A code header action (Pink `render::copy_action`): 22px, r6, a 12px copy
/// glyph in `content_muted` on a transparent → `ink(0.08)` hover; after a
/// click the glyph checks and "Copied" shows for 1.2s.
fn copy_action(
    id: SharedString,
    label: &'static str,
    copied: bool,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    let fade_key = format!("tcc-{id}");
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .h(px(CODE_ACTION_SIZE))
        .px(px(6.0))
        .rounded(px(radius::SM))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .cursor_pointer()
        .hover_fill(fade_key, gpui::transparent_black(), t.ink(0.08))
        .text_size(px(pink::text::CAPTION))
        .text_color(t.content_muted)
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            on_click(window, cx);
        })
        .child(
            svg()
                .path(if copied { COPY_CHECK } else { COPY })
                .flex_none()
                .size(px(12.0))
                .text_color(t.content_muted),
        )
        .when(copied, |el| el.child("Copied"))
}

/// Writes a segment to the clipboard and lets its action read "Copied"
/// for a moment (Pink `copy_ui_for`).
fn copy_handler(
    state: Entity<ToolState>,
    key: SharedString,
    slot: usize,
    text: SharedString,
) -> impl Fn(&mut Window, &mut App) + 'static {
    move |_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        let key = key.clone();
        state.update(cx, |s, cx| {
            s.copied = Some((key, slot));
            s.copy_reset = Some(cx.spawn(
                async move |this: WeakEntity<ToolState>, cx: &mut AsyncApp| {
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
    }
}

/// The "Show full output" row under a capped segment (Pink's blob
/// affordance): a 20px ghost button, r6, 12px `content_muted` with a 12px
/// arrow, `ink(0.05)` → `ink(0.09)` under the pointer. It lifts the line
/// cap; the segment then scrolls inside its 252px viewport.
fn show_full_row(
    state: Entity<ToolState>,
    key: SharedString,
    label: SharedString,
    t: &Pink,
) -> Div {
    let ring = t.focus_ring();
    let fade_key = format!("tcc-{key}-full");
    div()
        .h(px(BLOB_AFFORDANCE_HEIGHT))
        .flex_none()
        .flex()
        .items_end()
        .child(
            div()
                .id(SharedString::from(format!("{key}-full")))
                .role(Role::Button)
                .aria_label(label.clone())
                .tab_index(0)
                .focus_visible(move |style| pink::focus_ring(style, ring))
                .h(px(BLOB_AFFORDANCE_HEIGHT - 4.0))
                .px(px(8.0))
                .rounded(px(radius::SM))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .cursor_pointer()
                .text_size(px(TOOL_TEXT_SIZE))
                .text_color(t.content_muted)
                .hover_fill(fade_key, t.ink(0.05), t.ink(0.09))
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    state.update(cx, |s, cx| {
                        s.rows.entry(key.clone()).or_default().full = true;
                        cx.notify();
                    });
                })
                .child(
                    svg()
                        .path(ARROW_DOWN)
                        .flex_none()
                        .size(px(12.0))
                        .text_color(t.content_muted),
                )
                .child(label),
        )
}

// ---------------------------------------------------------------------------
// Group
// ---------------------------------------------------------------------------

/// Several calls folded behind one sentence, on Pink's activity rail (Pink
/// `render_tool_group`).
#[derive(IntoElement)]
pub struct ToolCallGroup {
    id: ElementId,
    cards: Vec<ToolCallCard>,
    label: Option<SharedString>,
    live: Option<bool>,
    default_open: Option<bool>,
    open: Option<bool>,
    on_open_change: Option<OpenHandler>,
}

impl ToolCallGroup {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            cards: Vec::new(),
            label: None,
            live: None,
            default_open: None,
            open: None,
            on_open_change: None,
        }
    }

    pub fn child(mut self, card: ToolCallCard) -> Self {
        self.cards.push(card);
        self
    }

    pub fn children(mut self, cards: impl IntoIterator<Item = ToolCallCard>) -> Self {
        self.cards.extend(cards);
        self
    }

    /// Replaces the group's sentence, which by default follows the calls
    /// (Pink `activity::group_sentence`). Write it verb first, and never
    /// join facts with a separator.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The group is the run's live tail: its verb shimmers and it stays
    /// open. Defaults to "any call is running".
    pub fn live(mut self, live: bool) -> Self {
        self.live = Some(live);
        self
    }

    /// Open or folded when nobody has toggled it (uncontrolled). By default
    /// it is open while live and folds once the run settles, as in pink.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = Some(open);
        self
    }

    /// Expanded state (controlled) – pair with [`on_open_change`](Self::on_open_change).
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    pub fn on_open_change(
        mut self,
        handler: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }
}

/// A group's sentence and the failures it does not already name (Pink
/// `activity::GroupSentence`).
pub(crate) struct GroupSentence {
    pub(crate) sentence: Sentence,
    pub(crate) failures: usize,
}

/// Calls of one verb in first-seen order; verbs that name a file count
/// distinct files, so reading one file twice is `Read 1 file`.
fn tally(cards: &[&ToolCallCard]) -> Vec<(Verb, usize)> {
    let mut verbs: Vec<(Verb, usize)> = Vec::new();
    let mut files: Vec<(Verb, &str)> = Vec::new();
    for card in cards {
        if let Some(path) = card.file.as_deref() {
            if files.contains(&(card.verb, path)) {
                continue;
            }
            files.push((card.verb, path));
        }
        match verbs.iter_mut().find(|(seen, _)| *seen == card.verb) {
            Some((_, n)) => *n += 1,
            None => verbs.push((card.verb, 1)),
        }
    }
    verbs
}

/// Several calls sharing one status: `Failed to run 2 commands`, `Running 3
/// commands`, or `Running 4 tools` across verbs.
fn shared_status(cards: &[&ToolCallCard], status: ToolCallStatus) -> Sentence {
    if let [card] = cards {
        return card.sentence();
    }
    match tally(cards).as_slice() {
        [(verb, n)] => Sentence::plain(format!("{} {}", verb.lead(status), verb.counted(*n))),
        _ => Sentence::plain(format!(
            "{} {}",
            Verb::RUN.lead(status),
            Verb::CALL.counted(cards.len())
        )),
    }
}

/// The most kinds a group sentence lists before summing up the rest.
const GROUP_SEGMENTS: usize = 3;

/// A group's sentence (Pink `activity::group_sentence`). While any call
/// runs it names only what is running. Settled, it counts the successes;
/// when nothing succeeded it names the failures.
pub(crate) fn group_sentence(cards: &[ToolCallCard]) -> GroupSentence {
    let failures = cards
        .iter()
        .filter(|card| card.status == ToolCallStatus::Error)
        .count();
    let with = |wanted: ToolCallStatus| -> Vec<&ToolCallCard> {
        cards.iter().filter(|card| card.status == wanted).collect()
    };
    let named = |sentence: Sentence, named: ToolCallStatus| GroupSentence {
        failures: if named == ToolCallStatus::Error {
            0
        } else {
            failures
        },
        sentence,
    };
    if cards.is_empty() {
        return named(Sentence::plain("Called no tools"), ToolCallStatus::Success);
    }
    let running = with(ToolCallStatus::Running);
    if !running.is_empty() {
        return named(
            shared_status(&running, ToolCallStatus::Running),
            ToolCallStatus::Running,
        );
    }
    if let [card] = cards {
        return named(card.sentence(), card.status);
    }
    let succeeded = with(ToolCallStatus::Success);
    if let [card] = succeeded.as_slice() {
        // One success among failures still reads as itself.
        return named(card.sentence(), ToolCallStatus::Success);
    }
    if !succeeded.is_empty() {
        let verbs = tally(&succeeded);
        let listed = if verbs.len() > GROUP_SEGMENTS {
            GROUP_SEGMENTS - 1
        } else {
            verbs.len()
        };
        let mut parts: Vec<String> = verbs[..listed]
            .iter()
            .map(|(verb, n)| format!("{} {}", verb.past, verb.counted(*n)))
            .collect();
        let rest: usize = verbs[listed..].iter().map(|(_, n)| n).sum();
        if rest > 0 {
            parts.push(format!(
                "{rest} more {}",
                if rest == 1 { "call" } else { "calls" }
            ));
        }
        return named(
            Sentence::plain(capitalize(&join(&parts))),
            ToolCallStatus::Success,
        );
    }
    named(
        shared_status(&with(ToolCallStatus::Error), ToolCallStatus::Error),
        ToolCallStatus::Error,
    )
}

/// Which calls an open group shows under the row budget (D64, Pink
/// `HistoryBudget`).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HistoryBudget {
    pub(crate) visible: Vec<bool>,
    /// Calls behind `Show N more`: settled successes only.
    pub(crate) hidden: usize,
    /// The `Show N more` row sits where the first hidden call would.
    pub(crate) more_at: Option<usize>,
}

/// The first eight settled successes show; later ones hide unless the
/// group was expanded or the reader has that call open. As in Pink, an open
/// call counts as one of the eight, and past them it stays in view without
/// counting toward `Show N more`. Anything else (`settled` false: running
/// and failed calls, thoughts) never hides and never counts (Pink
/// `tool_history_budget`).
pub(crate) fn history_budget(
    settled: &[bool],
    inspected: &[bool],
    expanded: bool,
) -> HistoryBudget {
    let mut shown = 0;
    let mut hidden = 0;
    let mut more_at = None;
    let visible = settled
        .iter()
        .enumerate()
        .map(|(ix, settled)| {
            if !settled {
                return true;
            }
            shown += 1;
            if expanded
                || shown <= TOOL_HISTORY_BUDGET
                || inspected.get(ix).copied().unwrap_or(false)
            {
                return true;
            }
            hidden += 1;
            more_at.get_or_insert(ix);
            false
        })
        .collect();
    HistoryBudget {
        visible,
        hidden,
        more_at,
    }
}

/// The group's glyph (Pink `tool_group_glyph`): the calls' own kind when
/// they share one, else the generic layers mark.
fn group_glyph(cards: &[ToolCallCard]) -> SharedString {
    let mut kinds = cards.iter().map(ToolCallCard::icon_path);
    match kinds.next() {
        None => TOOLBOX.into(),
        Some(first) if kinds.all(|kind| kind == first) => first,
        Some(_) => LAYERS.into(),
    }
}

/// The group's sentence, with Pink's light sweeping its leading verb while
/// live (Pink `tool_group_title`); inline code keeps its own face. Settled,
/// it takes the plate's hover-eased colour.
pub(crate) fn group_title(
    sentence: &Sentence,
    phase: Option<f32>,
    scale: f32,
    t: &Pink,
) -> AnyElement {
    let style = activity::sentence_style(t, scale);
    let (Some(phase), Some((Segment::Text(lead), rest))) = (phase, sentence.segments.split_first())
    else {
        return activity::sentence(&sentence.segments, &style).into_any_element();
    };
    div()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(activity::SEGMENT_GAP * scale))
        .child(
            div()
                .relative()
                .flex_none()
                .h(px(TOOL_LINE_HEIGHT * scale))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(lead.clone())
                .child(loaders::shimmer_overlay(
                    lead.clone(),
                    TOOL_TEXT_SIZE * scale,
                    TOOL_LINE_HEIGHT * scale,
                    t.content_muted,
                    t.text,
                    phase,
                )),
        )
        .when(!rest.is_empty(), |title| {
            title.child(activity::sentence(rest, &style))
        })
        .into_any_element()
}

/// `Show N more` (D64, Pink `tool_history_more_row`): the label naming the
/// exact count of settled calls the budget holds back, as a focusable text
/// action. The host adds the click.
pub(crate) fn more_action(id: SharedString, hidden: usize, pitch: f32, t: &Pink) -> Stateful<Div> {
    let scale = pitch / TOOL_TREE_ROW_HEIGHT;
    let label: SharedString = format!("Show {hidden} more").into();
    let ring = t.focus_ring();
    activity::recovery_action(
        id.clone(),
        label.clone(),
        &activity::RecoveryStyle {
            scale,
            height: pitch - 4.0 * scale,
            radius: radius::CONTROL,
            text: t.content_muted,
        },
    )
    .role(Role::Button)
    .aria_label(label)
    .tab_index(0)
    .focus_visible(move |style| pink::focus_ring(style, ring))
    .hover_fill_fg(
        format!("tcc-{id}"),
        t.fill(FillTier::Tertiary).opacity(0.0),
        t.fill(FillTier::Tertiary),
        t.content_muted,
        t.text,
    )
}

/// The `Show N more` row: the action on the rail in the first hidden
/// call's place, under the down chevron. Pressing it shows every call and
/// keeps the group open.
fn more_row(
    state: &Entity<ToolState>,
    gid: &str,
    hidden: usize,
    rail: Rail,
    pitch: f32,
    t: &Pink,
) -> AnyElement {
    let scale = pitch / TOOL_TREE_ROW_HEIGHT;
    let action = more_action(format!("{gid}-more").into(), hidden, pitch, t).on_click({
        let state = state.clone();
        move |_, _, cx| {
            cx.stop_propagation();
            state.update(cx, |s, cx| {
                s.more_open = true;
                s.fold.open = Some(true);
                cx.notify();
            });
        }
    });
    let glyph = svg()
        .path(ALT_ARROW_DOWN)
        .flex_none()
        .size(px(ACTIVITY_ICON_SIZE))
        .text_color(t.content_faint)
        .into_any_element();
    let row = div()
        .w_full()
        .h(px(pitch))
        .flex_none()
        .flex()
        .flex_row()
        .child(activity_rail(
            glyph,
            rail.has_predecessor,
            rail.continues,
            rail.connector,
            rail.continuation,
            pitch,
            t,
        ))
        .child(
            // The label's text, not its padding, starts on the sentences'
            // column.
            div()
                .ml(px(ACTIVITY_TEXT_GAP - 6.0 * scale))
                .min_w_0()
                .flex_1()
                .h(px(pitch))
                .flex()
                .items_center()
                .text_size(px(TOOL_TEXT_SIZE * scale))
                .line_height(px(TOOL_LINE_HEIGHT * scale))
                .child(action),
        );
    reveal_tool_row(row.into_any_element(), pitch, rail.reveal)
}

/// One entry on the open group's rail.
enum RailItem {
    Call(usize),
    More,
}

impl RenderOnce for ToolCallGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let scale = pink::ui_scale(cx);
        let pitch = TOOL_TREE_ROW_HEIGHT * scale;
        let header_height = TOOL_GROUP_HEADER_HEIGHT * scale;
        let failure_height = TOOL_FAILURE_LINE_HEIGHT * scale;
        let now = Instant::now();
        let reduced = motion::reduced_motion(cx);
        let speed = motion::speed_scale();
        let gid = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| ToolState::default(),
        );
        let running = self
            .cards
            .iter()
            .any(|card| card.status == ToolCallStatus::Running);
        let live = self.live.unwrap_or(running);
        let group = group_sentence(&self.cards);
        let sentence = match &self.label {
            Some(label) => Sentence::plain(label.to_string()),
            None => group.sentence,
        };
        let summary = ActivitySummary {
            state: if running {
                ActivityState::Running
            } else {
                ActivityState::Recorded
            },
            failures: group.failures,
        };
        let kind_glyph = group_glyph(&self.cards);
        let keys: Vec<SharedString> = self
            .cards
            .iter()
            .map(|card| SharedString::from(format!("{gid}/{}", card.id)))
            .collect();
        let settled: Vec<bool> = self
            .cards
            .iter()
            .map(|card| card.status == ToolCallStatus::Success)
            .collect();

        let (open, geometries, starts, header_started_at, shimmer_started_at, fold, more_open) =
            state.update(cx, |s, _| {
                // Rows found after the group mounted (or in a group that
                // arrives live) reveal: the first 90ms in, then 65ms apart
                let new_group = !s.mounted;
                if new_group {
                    s.mounted = true;
                    s.header_started_at = live.then_some(now);
                }
                let mut arrivals = 0;
                for key in &keys {
                    s.reveals.entry(key.clone()).or_insert_with(|| {
                        if new_group && !live {
                            return None;
                        }
                        let delay = motion::tool_row_arrival_delay(arrivals, new_group);
                        arrivals += 1;
                        Some(now + delay.mul_f32(speed))
                    });
                }
                let starts: Vec<Option<Instant>> = keys
                    .iter()
                    .map(|key| s.reveals.get(key).copied().flatten())
                    .collect();
                // A row still drawing in keeps the group open
                let connector_span = motion::TOOL_CONNECTOR_REVEAL.total().mul_f32(speed);
                let arrival_pending = !reduced
                    && starts
                        .iter()
                        .flatten()
                        .any(|start| now.saturating_duration_since(*start) < connector_span);
                let auto_open = self.default_open.unwrap_or(live) || arrival_pending;
                let open = self.open.or(s.fold.open).unwrap_or(auto_open);
                if live {
                    s.shimmer_started_at.get_or_insert(now);
                } else {
                    s.shimmer_started_at = None;
                }
                let geometries: Vec<Geometry> = self
                    .cards
                    .iter()
                    .zip(&keys)
                    .map(|(card, key)| {
                        let row = s.rows.entry(key.clone()).or_default();
                        card.geometry(row, pitch, failure_height, now, reduced)
                    })
                    .collect();
                (
                    open,
                    geometries,
                    starts,
                    s.header_started_at,
                    s.shimmer_started_at,
                    s.fold,
                    s.more_open,
                )
            });

        // The row budget: an open call counts as inspected.
        let inspected: Vec<bool> = geometries.iter().map(|geometry| geometry.open).collect();
        let budget = history_budget(&settled, &inspected, more_open);
        let mut items = Vec::new();
        for ix in 0..self.cards.len() {
            if budget.more_at == Some(ix) {
                items.push(RailItem::More);
            }
            if budget.visible[ix] {
                items.push(RailItem::Call(ix));
            }
        }

        let reveal: Vec<f32> = starts
            .iter()
            .map(|start| motion::reveal_progress(motion::TOOL_ROW_REVEAL, *start, now, reduced))
            .collect();
        let connector: Vec<f32> = starts
            .iter()
            .map(|start| {
                motion::reveal_progress(motion::TOOL_CONNECTOR_REVEAL, *start, now, reduced)
            })
            .collect();
        // The more row draws in with the first call it stands for.
        let item_ix = |item: &RailItem| match item {
            RailItem::Call(ix) => *ix,
            RailItem::More => budget.more_at.unwrap_or(0),
        };
        let header_reveal =
            motion::reveal_progress(motion::TOOL_ROW_REVEAL, header_started_at, now, reduced);
        let revealed = CHIPS_TOP_PAD
            + items
                .iter()
                .map(|item| {
                    let height = match item {
                        RailItem::Call(ix) => geometries[*ix].base + geometries[*ix].body,
                        RailItem::More => pitch,
                    };
                    height * reveal[item_ix(item)]
                })
                .sum::<f32>();
        let target = if open { revealed } else { 0.0 };
        // The group folds on the same clock as its rows
        let mut fold = fold;
        let (body_height, disclosure, _, folding) = fold.step(open, target, now, reduced);
        state.update(cx, |s, _| s.fold = fold);

        let motion_active = folding
            || header_reveal < 1.0
            || reveal
                .iter()
                .chain(&connector)
                .any(|progress| *progress < 1.0)
            || geometries.iter().any(|geometry| geometry.animating);
        if motion_active {
            window.request_animation_frame();
        }
        let shimmer_phase = shimmer_started_at
            .filter(|_| live && !reduced)
            .map(|start| {
                motion::pulse_keep(window, cx);
                loaders::shimmer_phase_since(start, now)
            });

        // The header: the package's plate on the pitch, the kind glyph on
        // the rail's trunk, the sentence, a failure count only when the
        // sentence does not say it, and a chevron that turns as it opens.
        let toggle = {
            let state = state.clone();
            let controlled = self.open.is_some();
            let on_change = self.on_open_change.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                if !controlled {
                    state.update(cx, |s, cx| {
                        s.fold.open = Some(!open);
                        cx.notify();
                    });
                }
                if let Some(on_change) = &on_change {
                    on_change(!open, window, cx);
                }
            }
        };
        let ring = t.focus_ring();
        let hover_key = format!("tcc-{gid}-hdr");
        let glyph_size = ACTIVITY_ICON_SIZE * scale;
        let header = div()
            .h(px(header_height))
            .flex()
            .flex_row()
            .items_center()
            .child(
                activity::activity_header(
                    SharedString::from(format!("{gid}-hdr")),
                    group_title(&sentence, shimmer_phase, scale, &t),
                    svg()
                        .path(kind_glyph)
                        .flex_none()
                        .size(px(glyph_size))
                        .text_color(motion::hover_blend(&hover_key, t.content_muted, t.text))
                        .into_any_element(),
                    svg()
                        .path(ALT_ARROW_RIGHT)
                        .flex_none()
                        .size(px(DISCLOSURE_CHEVRON_SIZE * scale))
                        .with_transformation(Transformation::rotate(radians(
                            FRAC_PI_2 * disclosure,
                        )))
                        .text_color(t.content_faint)
                        .into_any_element(),
                    summary,
                    activity::HeaderStyle {
                        scale,
                        height: header_height - 2.0 * scale,
                        radius: radius::CONTROL,
                        left_inset: ACTIVITY_TRUNK_X - glyph_size / 2.0,
                        text_size: TOOL_TEXT_SIZE * scale,
                        line_height: TOOL_LINE_HEIGHT * scale,
                        foreground: t.content_muted,
                        muted: t.content_faint,
                        active: t.text,
                        danger: t.readable_tone(t.danger, 1.0),
                        warning: t.warning,
                    },
                )
                .focus_visible(move |style| pink::focus_ring(style, ring))
                .role(Role::Button)
                .aria_label(summary.description(&sentence.text))
                .aria_expanded(open)
                .tab_index(0)
                .hover_fill_fg(
                    hover_key.clone(),
                    t.fill(FillTier::Quaternary),
                    t.fill(FillTier::Tertiary),
                    t.content_muted,
                    t.text,
                )
                .on_click(toggle),
            );

        let shown = items.len();
        let mut cards: Vec<Option<(ToolCallCard, SharedString, Geometry)>> = self
            .cards
            .into_iter()
            .zip(keys)
            .zip(geometries)
            .map(|((card, key), geometry)| Some((card, key, geometry)))
            .collect();
        let rows: Vec<AnyElement> = items
            .iter()
            .enumerate()
            .map(|(pos, item)| {
                let ix = item_ix(item);
                let rail = Rail {
                    has_predecessor: pos > 0,
                    continues: pos + 1 < shown,
                    reveal: reveal[ix],
                    connector: connector[ix],
                    continuation: motion::tool_connector_continuation(
                        items.get(pos + 1).map(|next| connector[item_ix(next)]),
                    ),
                };
                match item {
                    RailItem::More => more_row(&state, &gid, budget.hidden, rail, pitch, &t),
                    RailItem::Call(ix) => {
                        let (mut card, key, geometry) =
                            cards[*ix].take().expect("each call is drawn once");
                        card.slot = Some(Slot {
                            state: state.clone(),
                            key,
                            geometry,
                            rail,
                        });
                        card.into_any_element()
                    }
                }
            })
            .collect();
        let chips = div().pt(px(CHIPS_TOP_PAD)).flex().flex_col().children(rows);

        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .child(reveal_tool_row(
                header.into_any_element(),
                header_height,
                header_reveal,
            ))
            .child(div().overflow_hidden().h(px(body_height)).child(chips))
    }
}

// ---------------------------------------------------------------------------
// Story: the demo
// ---------------------------------------------------------------------------

/// When each step of the demo lands (ms), and when it starts over.
const STEPS: [u64; 10] = [0, 1000, 1800, 2400, 3200, 4000, 5200, 5800, 8200, 8500];
const LOOP: u64 = 16000;

const TOTAL_RS: &str = r#"use crate::{discount::apply_discount, tax::add_tax, Cart, Money};

/// What the customer pays at checkout, in cents.
pub fn checkout_total(cart: &Cart) -> Money {
    let subtotal: Money = cart.lines.iter().map(|line| line.total()).sum();
    let taxed = add_tax(subtotal, cart.region);
    let discounted = apply_discount(taxed, &cart.coupon);
    discounted.round_to_cent()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coupon, Region};

    #[test]
    fn checkout_total_applies_coupon_before_tax() {
        let cart = Cart::with_lines([(10_000, 1)])
            .region(Region::Eu)
            .coupon(Coupon::amount_off(1_000));
        assert_eq!(checkout_total(&cart), 9_720);
    }

    #[test]
    fn checkout_total_without_coupon() {
        let cart = Cart::with_lines([(2_500, 2)]).region(Region::Eu);
        assert_eq!(checkout_total(&cart), 5_400);
    }
}
"#;

const GREP: &str =
    "src/cart/total.rs:1:use crate::{discount::apply_discount, tax::add_tax, Cart, Money};
src/cart/total.rs:7:    let discounted = apply_discount(taxed, &cart.coupon);
src/cart/discount.rs:12:pub fn apply_discount(amount: Money, coupon: &Coupon) -> Money {";

const TEST: &str = "cargo test -p cart checkout_total";
/// `cargo test` output, as it streams in.
const TEST_OUT: [&str; 4] = [
    "   Compiling cart v0.4.1 (/work/shop/crates/cart)",
    "    Finished `test` profile [unoptimized + debuginfo] target(s) in 2.84s
     Running unittests src/lib.rs (target/debug/deps/cart-3f9c1d2e)",
    "
running 1 test
test total::tests::checkout_total_applies_coupon_before_tax ... FAILED",
    "
failures:

---- total::tests::checkout_total_applies_coupon_before_tax stdout ----
thread 'total::tests::checkout_total_applies_coupon_before_tax' panicked at src/cart/total.rs:21:9:
assertion `left == right` failed
  left: 9800
 right: 9720

failures:
    total::tests::checkout_total_applies_coupon_before_tax

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 13 filtered out
error: test failed, to rerun pass `-p cart --lib`",
];

/// What the failed test reported first: its failure line shows this.
const TEST_ERROR: &str = "assertion `left == right` failed: left 9800, right 9720";

// joiners: allow a shell command's path separator
const LOG: &str = "git log -3 --format='%h %an %s' -- src/cart/total.rs";
const LOG_OUT: &str = "a41c9e2 Mara Lindqvist Apply tax before coupons in checkout_total
7d02b5f Jonas Weber Round line totals to the cent
19e6a3c Mara Lindqvist Split cart totals into their own module";

const ASK: &str = "The checkout total test is failing on main. Can you find out why?";
const REPLY: &str = "It broke in a41c9e2, which moved add_tax ahead of apply_discount. The $10 coupon now comes off the taxed total, so the test's $100 cart checks out at $98.00 instead of $97.20. Putting the coupon back before tax fixes it. Want me to make that change?";

struct Demo {
    step: usize,
    run: u64,
    start: Instant,
    /// Someone toggled the group: stop replaying it.
    touched: bool,
    _timeline: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let start = Instant::now();
        Demo {
            step: 0,
            run: 0,
            start,
            touched: false,
            _timeline: cx.spawn(async move |this, cx| Demo::play(this, cx, start).await),
        }
    }

    async fn play(this: WeakEntity<Demo>, cx: &mut AsyncApp, mut start: Instant) {
        loop {
            let executor = cx.background_executor().clone();
            let wait = |ms: u64| {
                let deadline = start + Duration::from_millis(ms);
                executor.timer(deadline.saturating_duration_since(Instant::now()))
            };
            for (step, &at) in STEPS.iter().enumerate().skip(1) {
                wait(at).await;
                let landed = this.update(cx, |d, cx| {
                    d.step = step;
                    cx.notify();
                });
                if landed.is_err() {
                    return;
                }
            }
            wait(LOOP).await;
            let now = Instant::now();
            let again = this.update(cx, |d, cx| {
                if d.touched {
                    return false;
                }
                d.run += 1;
                d.step = 0;
                d.start = now;
                cx.notify();
                true
            });
            if !matches!(again, Ok(true)) {
                return;
            }
            start = now;
        }
    }
}

fn icon(name: &str) -> SharedString {
    format!("{ICONS}/{name}.svg").into()
}

/// The rest of the cart module, read once `total.rs` is in: enough settled
/// calls to cross the group's eight-row budget.
const MODULE: [&str; 9] = [
    "src/cart/discount.rs",
    "src/cart/tax.rs",
    "src/cart/money.rs",
    "src/cart/coupon.rs",
    "src/cart/line.rs",
    "src/cart/rounding.rs",
    "src/cart/currency.rs",
    "src/cart/mod.rs",
    "tests/checkout.rs",
];

/// A user bubble (Pink transcript): right-aligned, at most 80% wide,
/// `user_bubble_bg` with its inset edge, r16, 14/22.
fn user_bubble(text: &'static str, t: &Pink) -> Div {
    div().flex().justify_end().child(
        div()
            .max_w(gpui::relative(0.8))
            .bg(t.user_bubble_bg())
            .when_some(t.user_bubble_edge(), |el, edge| {
                el.shadow(vec![BoxShadow {
                    color: edge,
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(1.),
                    inset: true,
                }])
            })
            .rounded(px(radius::BUBBLE))
            .px(px(16.))
            .py(px(10.))
            .text_size(px(pink::text::PROSE))
            .line_height(px(22.))
            .text_color(t.text)
            .child(text),
    )
}

fn demo_view(demo: &Entity<Demo>, cx: &App) -> Div {
    use ToolCallStatus::{Error, Running, Success};
    let t = Pink::get(cx);
    let (step, run, start) = {
        let d = demo.read(cx);
        (d.step, d.run, d.start)
    };
    let done = |at: usize| if step >= at { Success } else { Running };

    let mut group = ToolCallGroup::new(ElementId::NamedInteger("tool-calls".into(), run))
        .on_open_change({
            let demo = demo.clone();
            move |_, _, cx| demo.update(cx, |d, _| d.touched = true)
        })
        .child(
            ToolCallCard::new("read", Verb::READ, done(1))
                .icon(icon("file-text"))
                .file("src/cart/total.rs")
                .args("src/cart/total.rs")
                .when(step >= 1, |c| c.result(TOTAL_RS)),
        );
    if step >= 1 {
        group = group.children(MODULE.iter().enumerate().map(|(ix, path)| {
            ToolCallCard::new(
                ElementId::NamedInteger("module".into(), ix as u64),
                Verb::READ,
                Success,
            )
            .icon(icon("file-text"))
            .file(*path)
            .args(*path)
            .result(format!("// {path}"))
        }));
        group = group.child(
            ToolCallCard::new("search", Verb::SEARCH, done(2))
                .icon(icon("file-search"))
                .words("for")
                .code("apply_discount")
                .words("in")
                .object("src")
                .args("apply_discount in src")
                .when(step >= 2, |c| c.result(GREP)),
        );
    }
    if step >= 2 {
        let streamed = step.saturating_sub(2).min(TEST_OUT.len());
        group = group.child(
            ToolCallCard::new("test", Verb::RUN, if step >= 6 { Error } else { Running })
                .icon(icon("terminal"))
                .code(TEST)
                .command(TEST)
                .when(streamed > 0, |c| c.result(TEST_OUT[..streamed].join("\n")))
                .when(step >= 6, |c| c.error(TEST_ERROR)),
        );
    }
    if step >= 6 {
        group = group.child(
            ToolCallCard::new("log", Verb::RUN, done(8))
                .icon(icon("terminal"))
                .code(LOG)
                .command(LOG)
                .when(step >= 7, |c| c.result(LOG_OUT)),
        );
    }

    // The working line under a live run (Pink's working trailer)
    let (verb, object) = match step {
        0 => ("Reading", "total.rs"),
        1 => ("Searching", "apply_discount"),
        2..=5 => ("Running", "cargo test"),
        _ => ("Running", "git log"),
    };
    let trailer = (step < 8).then(|| {
        div()
            .pt(px(16.))
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.))
            .child(loaders::orb_slot(t.accent))
            .child(loaders::working_status(verb, t.content_muted, t.text).object(object))
            .child(loaders::working_timer(
                start.elapsed().as_secs() as i64,
                t.content_faint,
            ))
    });
    let reply = (step >= 9).then(|| {
        motion::rise_in(
            ElementId::NamedInteger("tool-call-reply".into(), run),
            motion::TRANSCRIPT_IN,
            motion::TRANSCRIPT_IN_RISE,
            div()
                .pt(px(12.))
                .text_size(px(pink::text::PROSE))
                .line_height(px(22.))
                .text_color(t.text)
                .child(REPLY),
        )
    });

    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(48.))
        .font_family(pink::SANS)
        .child(
            div()
                // Room for the code surface's 80 columns beside the rail
                .w(px(700.))
                .max_w_full()
                .flex()
                .flex_col()
                .child(user_bubble(ASK, &t))
                .child(div().pt(px(16.)).child(group))
                .children(trailer)
                .children(reply),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("tool-call-demo", cx, |_, cx| Demo::new(cx));
        demo_view(&demo, cx)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_prints_like_json_stringify() {
        let value = Json::object([
            ("query", "CRM \"pricing\"".into()),
            ("max_results", 5.into()),
            ("ratio", 0.25.into()),
            ("tags", Json::array(Vec::<Json>::new())),
            (
                "deep",
                Json::object([("ok", true.into()), ("none", Json::Null)]),
            ),
        ]);
        assert_eq!(
            value.pretty(),
            "{\n  \"query\": \"CRM \\\"pricing\\\"\",\n  \"max_results\": 5,\n  \"ratio\": 0.25,\n  \"tags\": [],\n  \"deep\": {\n    \"ok\": true,\n    \"none\": null\n  }\n}"
        );
    }

    #[test]
    fn parses_json_text() {
        let parsed = Json::parse(r#" {"a": [1, -2.5e1, "x\nyé"], "b": null} "#).unwrap();
        assert_eq!(
            parsed,
            Json::object([
                (
                    "a",
                    Json::array([Json::Number(1.), Json::Number(-25.), "x\nyé".into()])
                ),
                ("b", Json::Null),
            ])
        );
        assert_eq!(Json::parse("{"), None);
        assert_eq!(Json::parse("[1,]"), None);
        // Text payloads that aren't JSON stay verbatim; JSON text pretty-prints
        assert_eq!(ToolPayload::from("{not json").text(), "{not json");
        assert_eq!(ToolPayload::from("[1]").text(), "[\n  1\n]");
    }

    #[test]
    fn segments_wrap_trim_and_cap_like_pink() {
        let long = "x".repeat(CALL_WRAP_COLS * 2 + 10);
        let section = Section::new(1, "Output", "Copy output", format!("{long}\n\n"), 24).unwrap();
        assert_eq!(section.lines.len(), 3);
        assert_eq!(section.truncated_by, 0);
        let many = (0..40)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let capped = Section::new(1, "Output", "Copy output", many, 24).unwrap();
        assert_eq!(capped.lines.len(), 24);
        assert_eq!(capped.truncated_by, 16);
        // 24 lines + the tail overflow the 252px viewport
        assert_eq!(capped.viewport(), OUTPUT_VIEWPORT_MAX_HEIGHT);
        assert!(Section::new(0, "Command", "Copy command", "\n".into(), 24).is_none());
    }

    #[test]
    fn open_bodies_add_up_analytically() {
        let one = Section::new(0, "Command", "Copy command", "ls".into(), 24).unwrap();
        // 1 line: 18 + 12 viewport, 28 header, 2 edges, 1 separator
        assert_eq!(body_height(std::slice::from_ref(&one), false), 61.0);
        let two = [
            one.clone(),
            Section::new(1, "Output", "Copy output", "a\nb".into(), 24).unwrap(),
        ];
        // One frame: separator, frame edges, both segments and the hairline
        assert_eq!(
            body_height(&two, true),
            1.0 + 2.0 + 60.0 + 1.0 + 78.0 + 24.0
        );
    }

    #[test]
    fn sentences_follow_the_recorded_lifecycle() {
        use ToolCallStatus::{Error, Running, Success};
        let run = |status| ToolCallCard::new("t", Verb::RUN, status).code("cargo test");
        let read = |id: &'static str, path: &'static str| {
            ToolCallCard::new(id, Verb::READ, Success).file(path)
        };
        let search = || {
            ToolCallCard::new("s", Verb::SEARCH, Success)
                .words("for")
                .code("apply_discount")
        };
        // The tense is the call's own: never "Ran" while it runs or failed.
        assert_eq!(run(Success).sentence().text, "Ran cargo test");
        assert_eq!(run(Running).sentence().text, "Running cargo test");
        assert_eq!(run(Error).sentence().text, "Failed to run cargo test");
        assert_eq!(search().sentence().text, "Searched for apply_discount");
        // While a call runs the group names only what runs; the failure
        // stays its own count beside the sentence.
        let group = group_sentence(&[read("a", "src/a.rs"), run(Error), run(Running)]);
        assert_eq!(
            (group.sentence.text.as_str(), group.failures),
            ("Running cargo test", 1)
        );
        // Settled, successes are counted (one file read twice is one file)
        // and a failed call is never among them.
        let group = group_sentence(&[
            read("a", "src/a.rs"),
            read("b", "src/a.rs"),
            read("c", "src/b.rs"),
            search(),
            run(Error),
        ]);
        assert_eq!(
            (group.sentence.text.as_str(), group.failures),
            ("Read 2 files and searched once", 1)
        );
        // Nothing succeeded: the failure is the sentence, not a second count.
        let group = group_sentence(&[run(Error), run(Error)]);
        assert_eq!(
            (group.sentence.text.as_str(), group.failures),
            ("Failed to run 2 commands", 0)
        );
        // Past three kinds the rest sum up.
        let group = group_sentence(&[
            read("a", "src/a.rs"),
            search(),
            ToolCallCard::new("e", Verb::EDIT, Success).file("src/c.rs"),
            ToolCallCard::new("f", Verb::FETCH, Success).object("docs.rs"),
        ]);
        assert_eq!(
            group.sentence.text,
            "Read 1 file, searched once and 2 more calls"
        );
        assert_eq!(Verb::SEARCH.counted(3), "3 times");
        assert_eq!(Verb::RUN.counted(1), "1 command");
        assert_eq!(single_line("a\n  b\tc"), "a b c");
        assert_eq!(file_name("src/cart/total.rs"), "total.rs");
        assert_eq!(file_icon("src/cart/total.rs", true), FILE_RUST_DARK);
        assert_eq!(file_icon("Cargo.lock", false), FILE_DOCUMENT);
    }

    #[test]
    fn the_row_budget_hides_only_settled_successes() {
        use ToolCallStatus::{Error, Running, Success};
        // Nine successes, a failure, a tenth success and a running call.
        let mut statuses = vec![Success; 9];
        statuses.extend([Error, Success, Running]);
        let statuses: Vec<bool> = statuses.iter().map(|status| *status == Success).collect();
        let closed = vec![false; statuses.len()];
        let budget = history_budget(&statuses, &closed, false);
        // Eight successes show; the ninth and tenth wait behind the row,
        // which sits where the first of them would.
        assert_eq!((budget.hidden, budget.more_at), (2, Some(8)));
        assert!(budget.visible[9] && budget.visible[11]);
        assert!(!budget.visible[8] && !budget.visible[10]);
        // An open call past the eight stays in view and is not counted.
        let mut inspected = closed.clone();
        inspected[10] = true;
        let budget = history_budget(&statuses, &inspected, false);
        assert_eq!(budget.hidden, 1);
        assert!(budget.visible[10]);
        // Expanded, nothing hides; under the budget there is no row.
        assert_eq!(history_budget(&statuses, &closed, true).hidden, 0);
        assert_eq!(
            history_budget(&statuses[..8], &closed[..8], false).more_at,
            None
        );
    }

    #[test]
    fn branch_reveals_along_its_length() {
        let full = activity_branch_points(1.0);
        assert_eq!(
            full.last().copied(),
            Some(point(
                ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X,
                ACTIVITY_BEND_RADIUS
            ))
        );
        let half = activity_branch_points(0.5);
        assert!(half.len() < full.len());
        assert_eq!(half[0], full[0]);
    }
}
