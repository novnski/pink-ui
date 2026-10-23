//! Tool Call Card – an agent's tool calls drawn the way pink's transcript
//! draws them (pink: `transcript.rs` `render_tool_group`, `chip_header_row`,
//! `activity_rail`, `detail_body`).
//!
//! [`ToolCallGroup`] is pink's tool group: a quiet 24px plate that sums the
//! calls up ("Ran 2 commands · read 1 file · 1 failed") beside a kind glyph
//! and a chevron, with a light sweeping over the summary while the run is
//! live. Open, it hangs one bare 32px row per call off an activity rail: a
//! hairline trunk that bends into each row's 14px glyph (the mini spinner
//! while the call runs, a red danger circle when it failed), the label and
//! detail in 12/18 muted text that brightens under the pointer, a red
//! "Failed" word and a half-strength chevron; a call that names a file shows
//! pink's file badge for its detail. A row with an invocation or an
//! output folds open into pink's code surface: "Invocation" or "Command" and
//! "Output" segments with copy actions, 24 lines with a counted tail and a
//! "Show full output" affordance. Rows arrive with the rail drawing into
//! them; the group stays open while live and folds itself shut once the run
//! settles, unless someone toggled it.
//!
//! A [`ToolCallCard`] outside a group is pink's chip card: a 30px bordered
//! card with an 18px icon tile that folds open the same way.
//!
//! Tab reaches every header; Enter or Space toggles it.
//!
//! ```ignore
//! ToolCallGroup::new("tools")
//!     .label("Ran 1 command")
//!     .child(
//!         ToolCallCard::new("test", "Run", ToolCallStatus::Running)
//!             .icon("icons/tool-call-card/terminal.svg")
//!             .summary("cargo test -p cart")
//!             .command("cargo test -p cart"),
//!     )
//! ```

use std::{collections::HashMap, f32::consts::FRAC_PI_2, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, AsyncApp, BoxShadow, ClickEvent, ClipboardItem, Context, Div,
    ElementId, Entity, FillOptions, FillRule, FontWeight, IntoElement, PathBuilder, PathStyle,
    Pixels, Point, Role, SharedString, Stateful, Task, Transformation, WeakEntity, Window, canvas,
    div, img, point, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill as _, motion,
    pink::{self, FillTier, Pink, loaders, radius},
    story,
};

pub const NAME: &str = "Tool Call Card";
pub const DESCRIPTION: &str = "An agent's tool calls folded into one quiet line that shimmers while the run is live – open it for one row per call on an activity rail, a spinner while it runs or a red Failed when it didn't, and each row folds open to its command and output with copy";

// ---------------------------------------------------------------------------
// Geometry (pink `transcript.rs` tool row constants)
// ---------------------------------------------------------------------------

/// A chip card's row: 30px card centered in 38 (pink `CHIP_HEIGHT`).
const CHIP_HEIGHT: f32 = 38.0;
/// The bordered chip card, border-box (pink `CHIP_CARD_HEIGHT`).
const CHIP_CARD_HEIGHT: f32 = 30.0;
/// The header inside the bordered card (pink `CHIP_HEADER_HEIGHT`).
const CHIP_HEADER_HEIGHT: f32 = CHIP_CARD_HEIGHT - 2.0;
/// The rail's gutter, trunk, elbow and glyph (pink `ACTIVITY_*`).
const ACTIVITY_GUTTER_WIDTH: f32 = 48.0;
const ACTIVITY_TEXT_GAP: f32 = 8.0;
const ACTIVITY_TRUNK_X: f32 = 12.5;
const ACTIVITY_BEND_RADIUS: f32 = 6.0;
const ACTIVITY_BRANCH_END_X: f32 = 28.0;
const ACTIVITY_ICON_LEFT: f32 = 32.0;
const ACTIVITY_ICON_SIZE: f32 = pink::icon_size::SM;
/// Rows and the group summary: 12/18 (pink `TOOL_LABEL_SIZE`).
const TOOL_TEXT_SIZE: f32 = loaders::TOOL_LABEL_SIZE;
const TOOL_LINE_HEIGHT: f32 = loaders::TOOL_LABEL_LINE_HEIGHT;
/// The group header row; its plate is 2px shorter (pink
/// `TOOL_GROUP_HEADER_HEIGHT`).
const TOOL_GROUP_HEADER_HEIGHT: f32 = 26.0;
/// One rail row (pink `TOOL_TREE_ROW_HEIGHT`).
const TOOL_TREE_ROW_HEIGHT: f32 = 32.0;
/// Rows and group headers share one chevron (pink `DISCLOSURE_CHEVRON_SIZE`).
const DISCLOSURE_CHEVRON_SIZE: f32 = pink::icon_size::XS;
/// Space above the first rail row (pink `CHIPS_TOP_PAD`).
const CHIPS_TOP_PAD: f32 = 2.0;
/// Lines an output segment shows before its counted tail (pink
/// `OUTPUT_DETAIL_MAX_LINES`).
const OUTPUT_DETAIL_MAX_LINES: usize = 24;
/// The ceiling for an invocation and for a full output (pink
/// `FULL_OUTPUT_MAX_LINES`).
const FULL_OUTPUT_MAX_LINES: usize = 400;
/// Invocation and output lines soft-wrap here (pink `CALL_WRAP_COLS`).
const CALL_WRAP_COLS: usize = 80;
/// The code surface header (pink `OUTPUT_CODE_HEADER_HEIGHT`).
const OUTPUT_CODE_HEADER_HEIGHT: f32 = 28.0;
/// Code segments scroll past this (pink `OUTPUT_VIEWPORT_MAX_HEIGHT`).
const OUTPUT_VIEWPORT_MAX_HEIGHT: f32 = 252.0;
/// One code line: the code size's 12.5/18 rhythm (pink `OUTPUT_LINE_HEIGHT`).
const OUTPUT_LINE_HEIGHT: f32 = 18.0;
/// The code body's `py 6` × 2 (pink `OUTPUT_BODY_PAD`).
const OUTPUT_BODY_PAD: f32 = 12.0;
/// The hairline between a row's header and its body (pink
/// `DETAIL_SEPARATOR`).
const DETAIL_SEPARATOR: f32 = 1.0;
/// The "Show full output" row (pink `BLOB_AFFORDANCE_HEIGHT`).
const BLOB_AFFORDANCE_HEIGHT: f32 = 24.0;
/// The code surface's side padding (pink `render::CODE_PADDING_X`).
const CODE_PADDING_X: f32 = 12.0;
/// Copy actions in the code header (pink `render::CODE_ACTION_SIZE`).
const CODE_ACTION_SIZE: f32 = 22.0;
/// How long a copy action reads "Copied" (pink `copy_ui_for`).
const COPIED_FOR: Duration = Duration::from_millis(1200);
/// Rows hover their label, detail and chevron together.
const HEADER_GROUP: &str = "tool-header";
/// A file badge: a 22px chip, a 20px well, the 14px file-type artwork (pink
/// `file_badges::render_file_badge`).
const FILE_BADGE_HEIGHT: f32 = 22.0;
const FILE_WELL: f32 = 20.0;

const ICONS: &str = "icons/tool-call-card";
const ALT_ARROW_RIGHT: &str = "icons/tool-call-card/alt-arrow-right.svg";
const ARROW_DOWN: &str = "icons/tool-call-card/arrow-down.svg";
const COPY: &str = "icons/tool-call-card/copy.svg";
const COPY_CHECK: &str = "icons/tool-call-card/copy-check.svg";
const DANGER_CIRCLE: &str = "icons/tool-call-card/danger-circle.svg";
const LAYERS: &str = "icons/tool-call-card/layers-minimalistic.svg";
/// Pink's glyph for a tool it has no kind for.
const TOOLBOX: &str = "icons/tool-call-card/toolbox.svg";
/// The polychrome file-type artwork in a file badge's well (pink
/// `file_icons`, vscode-symbols; dark variants lifted like pink's
/// `dark_icon_svg`).
const FILE_RUST: &str = "icons/tool-call-card/file-rust.svg";
const FILE_RUST_DARK: &str = "icons/tool-call-card/file-rust-dark.svg";
const FILE_DOCUMENT: &str = "icons/tool-call-card/file-document.svg";
const FILE_DOCUMENT_DARK: &str = "icons/tool-call-card/file-document-dark.svg";

type OpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error,
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

/// What a code segment shows, verbatim in mono like pink's tool output:
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
// Code segments (pink `call_block` / `tool_detail` / `detail_body`)
// ---------------------------------------------------------------------------

/// Soft-wraps one raw line into `cols`-char chunks so a long command stays
/// readable instead of ellipsizing (pink `wrap_cols`).
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

/// The one-line form of a row's detail (pink `single_line`).
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
/// output" row. Analytic, like pink's fold heights, so nothing is measured.
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

/// One fold's clock (pink `FoldState`): the user's choice, and the height
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

    /// Time since the toggle while the tween is armed (pink
    /// `FOLD_TWEEN_WINDOW`).
    fn elapsed(&self, now: Instant, reduced: bool) -> Option<Duration> {
        let at = self.toggled_at.filter(|_| !reduced)?;
        let elapsed = now.saturating_duration_since(at);
        (elapsed < motion::FOLD_TWEEN_WINDOW.mul_f32(motion::speed_scale())).then_some(elapsed)
    }

    /// `(height, disclosure progress, body alpha, animating)` on pink's one
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

/// One tool call: a rail row inside a [`ToolCallGroup`], pink's chip card on
/// its own.
#[derive(IntoElement)]
pub struct ToolCallCard {
    id: ElementId,
    name: SharedString,
    summary: Option<SharedString>,
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
    /// `name` is the row's label: the kind of call as pink names it ("Run",
    /// "Read", "Search", "Web") or the tool's own name.
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        status: ToolCallStatus,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            summary: None,
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

    /// The detail beside the label, on one line – the command, path, query
    /// or URL ("cargo test -p cart").
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// The file the call reads or writes ("src/cart/total.rs"): its detail
    /// becomes pink's file badge, the file-type artwork beside the file's
    /// name. Takes the place of [`summary`](Self::summary).
    pub fn file(mut self, path: impl Into<SharedString>) -> Self {
        self.file = Some(path.into());
        self
    }

    /// The row's 14px glyph (an SVG path under `assets/`); defaults to pink's
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

    /// A shell command, shown as "Command" (pink labels exec invocations so).
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

    /// The failure, shown as the output of an `Error` call without a result.
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
    /// output" (default 24, as in pink).
    pub fn max_lines(mut self, lines: usize) -> Self {
        self.max_lines = lines.max(1);
        self
    }

    fn icon_path(&self) -> SharedString {
        self.icon.clone().unwrap_or(TOOLBOX.into())
    }

    fn detail(&self) -> SharedString {
        self.file
            .as_ref()
            .or(self.summary.as_ref())
            .map(|detail| single_line(detail))
            .unwrap_or_default()
    }

    /// Invocation first (what was asked, whole up to pink's 400-line
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

    /// The row's fold for this frame (pink: `render_tool_group` row heights).
    fn geometry(&self, row: &mut RowState, base: f32, now: Instant, reduced: bool) -> Geometry {
        let sections = self.sections(row.full);
        let affordance = (!row.full
            && sections
                .iter()
                .any(|section| section.slot == 1 && section.truncated_by > 0))
        .then(|| SharedString::from("Show full output"));
        let expandable = !sections.is_empty();
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
    /// on failure, else the tool's own glyph (pink `activity_glyph`).
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

    /// The clickable header of an expandable row (pink: the keyed chip
    /// header button in `render_tool_group`).
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
        let detail = self.detail();
        let accessible = format!(
            "{}: {}",
            self.name,
            detail.chars().take(120).collect::<String>()
        );
        let toggle = {
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
        };
        div()
            .id(SharedString::from(format!("{key}-row")))
            .role(Role::Button)
            .aria_label(accessible)
            .aria_expanded(open)
            .tab_index(0)
            .rounded(px(radius::CONTROL))
            .focus_visible(move |style| pink::focus_ring(style, ring))
            .h(px(if rail {
                CHIP_CARD_HEIGHT
            } else {
                CHIP_HEADER_HEIGHT
            }))
            .flex_none()
            .flex()
            .items_center()
            .cursor_pointer()
            .on_click(toggle)
            .child(chip_header_row(self, rail, Some(geometry.chevron), key, t))
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

    /// A row on its group's rail (pink `render_tool_group` rows and
    /// `tool_chip` in rail mode).
    fn render_rail(self, slot: Slot, t: &Pink, cx: &App) -> AnyElement {
        let Slot {
            state,
            key,
            geometry,
            rail,
        } = slot;
        let base = geometry.base;
        let row_height = base + geometry.body;
        let content_reveal = motion::tool_connector_parts(rail.connector, rail.has_predecessor).1;
        let glyph = self.rail_glyph(&key, t);
        let card = if geometry.expandable {
            div()
                .my(px((base - CHIP_CARD_HEIGHT) / 2.0))
                .ml(px(ACTIVITY_TEXT_GAP))
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .overflow_hidden()
                .child(self.header_button(&state, &key, &geometry, true, t))
                .when(geometry.mounted, |card| {
                    card.child(self.panel(&state, &key, &geometry, true, t, cx))
                })
                .h(px(row_height - base + CHIP_CARD_HEIGHT))
        } else {
            div()
                .ml(px(ACTIVITY_TEXT_GAP))
                .my(px((base - CHIP_CARD_HEIGHT) / 2.0))
                .h(px(CHIP_CARD_HEIGHT))
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .overflow_hidden()
                .child(chip_header_row(&self, true, None, &key, t))
        };
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
                base,
                t,
            ))
            .child(card);
        reveal_tool_row(row.into_any_element(), row_height, rail.reveal)
    }

    /// Pink's chip card (pink `tool_chip` / `subagent_chip` card mode): a
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
            card.justify_center()
                .child(chip_header_row(&self, false, None, &key, t))
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
            self.geometry(row, CHIP_HEIGHT, now, reduced)
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

/// The row's one line (pink `chip_header_row`). On the rail: the label and
/// detail in 12/18 `content_muted`, brightening to `text` with the row's
/// hover, a spacer, a red "Failed", then an 18px tile holding the 12px
/// chevron at half strength until hovered. In a card: an 18px icon tile,
/// a medium label and the detail in `text` at 0.85, with the mini spinner
/// trailing while the call runs. A failure is one red glyph and one red
/// word, never a red row.
fn chip_header_row(
    card: &ToolCallCard,
    rail: bool,
    trail: Option<f32>,
    key: &str,
    t: &Pink,
) -> Div {
    let failed = card.status == ToolCallStatus::Error;
    let running = card.status == ToolCallStatus::Running;
    let hover_text = rail && trail.is_some();
    let detail = card.detail();
    let text = t.text;
    div()
        .group(HEADER_GROUP)
        .h(px(if rail {
            CHIP_CARD_HEIGHT
        } else {
            CHIP_HEADER_HEIGHT
        }))
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.0))
        .px(px(if rail { 0.0 } else { 8.0 }))
        .text_size(px(TOOL_TEXT_SIZE))
        .line_height(px(TOOL_LINE_HEIGHT))
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
                .flex_none()
                .h(px(TOOL_LINE_HEIGHT))
                .flex()
                .items_center()
                .when(!rail, |label| label.font_weight(FontWeight::MEDIUM))
                .text_color(t.content_muted)
                .child(card.name.clone())
                .map(|label| {
                    if hover_text {
                        label
                            .id("tool-label")
                            .group_hover(HEADER_GROUP, move |style| style.text_color(text))
                            .into_any_element()
                    } else {
                        label.into_any_element()
                    }
                }),
        )
        .when(!rail || !detail.is_empty(), |row| {
            row.child(
                div()
                    .when(!rail, |detail| detail.flex_1())
                    .min_w_0()
                    .h(px(if card.file.is_some() {
                        FILE_BADGE_HEIGHT
                    } else {
                        TOOL_LINE_HEIGHT
                    }))
                    .flex()
                    .items_center()
                    .truncate()
                    .text_color(if rail {
                        t.content_muted
                    } else {
                        t.text.opacity(0.85)
                    })
                    .child(match &card.file {
                        Some(path) => file_badge(path, hover_text, t),
                        None => div().min_w_0().truncate().child(detail).into_any_element(),
                    })
                    .map(|detail| {
                        if hover_text {
                            detail
                                .id("tool-detail")
                                .group_hover(HEADER_GROUP, move |style| style.text_color(text))
                                .into_any_element()
                        } else {
                            detail.into_any_element()
                        }
                    }),
            )
        })
        // Trailing accessories share one right-aligned column
        .child(div().flex_1().min_w(px(4.0)))
        .when(failed, |row| {
            row.child(
                div()
                    .flex_none()
                    .text_size(px(pink::text::CAPTION))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.readable_tone(t.danger, 1.0))
                    .child("Failed"),
            )
        })
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
                .size(px(DISCLOSURE_CHEVRON_SIZE))
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

/// The file-type artwork for a path (pink `file_icons::asset_path`, for
/// the kinds this component carries; anything else is pink's generic
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

/// A badge names the last path component (pink `file_badge_name`).
fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .find(|component| !component.is_empty())
        .unwrap_or(path)
}

/// Pink's file badge (`file_badges::render_file_badge`, no open route): a
/// 22px r6 chip on `ink(0.06)` holding the file-type artwork in a 20px well
/// and the file name in `text` at 85%, brightening with the row's hover.
fn file_badge(path: &str, hover_text: bool, t: &Pink) -> AnyElement {
    let text = t.text;
    // pink `file_icons::well_bg`: a counter-shade seat for the artwork
    let well = pink::grey(if t.dark { 0 } else { 255 }).opacity(0.16);
    let chip = div()
        .min_w_0()
        .h(px(FILE_BADGE_HEIGHT))
        .flex()
        .items_center()
        .overflow_hidden()
        .gap(px(6.0))
        .rounded(px(radius::SM))
        .bg(t.ink(0.06))
        .pl(px(1.0))
        .pr(px(6.0))
        .text_color(t.text.opacity(0.85))
        .child(
            div()
                .size(px(FILE_WELL))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::XS))
                .bg(well)
                .child(
                    img(file_icon(path, t.dark))
                        .flex_none()
                        .size(px(ACTIVITY_ICON_SIZE)),
                ),
        )
        .child(
            div()
                .min_w_0()
                .truncate()
                .child(SharedString::from(file_name(path).to_owned())),
        );
    if hover_text {
        chip.id("tool-file-badge")
            .group_hover(HEADER_GROUP, move |style| style.text_color(text))
            .into_any_element()
    } else {
        chip.into_any_element()
    }
}

/// The tree beside a row (pink `activity_rail`): a 1px `hairline(0.12)`
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
/// doesn't jump at their junction (pink `activity_branch_points`).
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
/// contours (pink `activity_ribbon`).
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
/// (pink `reveal_tool_row`); the fade and lift ride the row's content, so the
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

/// The code surface frame (pink `code_surface_frame`): the kit's code
/// surface, r10 with a 1px edge on `ink(0.035)`.
fn code_surface_frame(t: &Pink) -> Div {
    pink::code_surface(t)
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .overflow_hidden()
}

/// The code surface header (pink `code_surface_header`): 28px, an 11px
/// medium `content_muted` label, actions trailing, a barely raised
/// `ink(0.02)` over the body.
fn code_surface_header(t: &Pink) -> Div {
    div()
        .h(px(OUTPUT_CODE_HEADER_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .pl(px(CODE_PADDING_X))
        .pr(px(5.0))
        .border_b_1()
        .border_color(t.border)
        .bg(t.ink(0.02))
        .font_family(pink::SANS)
        .text_size(px(pink::text::CAPTION))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.content_muted)
}

/// One code segment (pink `detail_body`, output case): the header with its
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
    body.child(
        code_surface_header(t)
            .child(section.label)
            .child(copy_action(
                SharedString::from(format!("{key}-copy-{}", section.slot)),
                section.copy_label,
                copied,
                on_copy,
                t,
            )),
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

/// The counted tail under a capped segment (pink `more_lines_row`).
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

/// A code header action (pink `render::copy_action`): 22px, r6, a 12px copy
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
/// for a moment (pink `copy_ui_for`).
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

/// The "Show full output" row under a capped segment (pink's blob
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

/// Several calls folded behind one summary plate, on pink's activity rail
/// (pink `render_tool_group`).
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

    /// What the calls did, pink style – "Ran 2 commands · read 1 file";
    /// defaults to "Called 3 tools". The group adds "· 1 failed".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// The group is the run's live tail: its summary shimmers and it stays
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

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The summary line (pink `tool_group_summary`): the caller's segments, or
/// pink's wording for tools it has no kind for, then "N failed".
fn group_summary(label: Option<&str>, count: usize, failed: usize) -> String {
    let mut summary = match label {
        Some(label) => label.to_string(),
        None => format!("Called {}", plural(count, "tool", "tools")),
    };
    if failed > 0 {
        summary.push_str(&format!(" · {failed} failed"));
    }
    summary
}

/// The summary's glyph (pink `tool_group_glyph`): the calls' own kind when
/// they share one, else the generic layers mark.
fn group_glyph(cards: &[ToolCallCard]) -> SharedString {
    let mut kinds = cards.iter().map(ToolCallCard::icon_path);
    match kinds.next() {
        None => TOOLBOX.into(),
        Some(first) if kinds.all(|kind| kind == first) => first,
        Some(_) => LAYERS.into(),
    }
}

/// The summary, with pink's light sweep laid over it while live (pink
/// `tool_group_title`); settled, it takes the plate's hover-eased color.
fn group_title(text: SharedString, phase: Option<f32>, t: &Pink) -> AnyElement {
    let Some(phase) = phase else {
        return text.into_any_element();
    };
    div()
        .relative()
        .h_full()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .child(text.clone())
        .child(loaders::shimmer_overlay(
            text,
            TOOL_TEXT_SIZE,
            TOOL_LINE_HEIGHT,
            t.content_muted,
            t.text,
            phase,
        ))
        .into_any_element()
}

impl RenderOnce for ToolCallGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let now = Instant::now();
        let reduced = motion::reduced_motion(cx);
        let speed = motion::speed_scale();
        let gid = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| ToolState::default(),
        );
        let live = self.live.unwrap_or_else(|| {
            self.cards
                .iter()
                .any(|card| card.status == ToolCallStatus::Running)
        });
        let failed = self
            .cards
            .iter()
            .filter(|card| card.status == ToolCallStatus::Error)
            .count();
        let summary: SharedString =
            group_summary(self.label.as_deref(), self.cards.len(), failed).into();
        let kind_glyph = group_glyph(&self.cards);
        let keys: Vec<SharedString> = self
            .cards
            .iter()
            .map(|card| SharedString::from(format!("{gid}/{}", card.id)))
            .collect();

        let (open, geometries, starts, header_started_at, shimmer_started_at, fold) =
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
                        card.geometry(row, TOOL_TREE_ROW_HEIGHT, now, reduced)
                    })
                    .collect();
                (
                    open,
                    geometries,
                    starts,
                    s.header_started_at,
                    s.shimmer_started_at,
                    s.fold,
                )
            });

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
        let header_reveal =
            motion::reveal_progress(motion::TOOL_ROW_REVEAL, header_started_at, now, reduced);
        let revealed = CHIPS_TOP_PAD
            + geometries
                .iter()
                .zip(&reveal)
                .map(|(geometry, progress)| (geometry.base + geometry.body) * progress)
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

        // The summary plate: a resting fill that brightens under the
        // pointer, the kind glyph on the rail's trunk, the summary and a
        // chevron that turns as the group opens. Quiet even when calls
        // failed – the rows and "· N failed" say so.
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
        let header = div()
            .h(px(TOOL_GROUP_HEADER_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .child(
                div()
                    .id(SharedString::from(format!("{gid}-hdr")))
                    .rounded(px(radius::CONTROL))
                    .focus_visible(move |style| pink::focus_ring(style, ring))
                    .role(Role::Button)
                    .aria_label(format!("Tool activity: {summary}"))
                    .aria_expanded(open)
                    .tab_index(0)
                    .flex_none()
                    .max_w_full()
                    .min_w_0()
                    .h(px(TOOL_GROUP_HEADER_HEIGHT - 2.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .pl(px(ACTIVITY_TRUNK_X - ACTIVITY_ICON_SIZE / 2.0))
                    .pr(px(8.0))
                    .cursor_pointer()
                    .text_size(px(TOOL_TEXT_SIZE))
                    .line_height(px(TOOL_LINE_HEIGHT))
                    .hover_fill_fg(
                        hover_key.clone(),
                        t.fill(FillTier::Quaternary),
                        t.fill(FillTier::Tertiary),
                        t.content_muted,
                        t.text,
                    )
                    .on_click(toggle)
                    .child(
                        svg()
                            .path(kind_glyph)
                            .flex_none()
                            .size(px(ACTIVITY_ICON_SIZE))
                            .text_color(motion::hover_blend(&hover_key, t.content_muted, t.text)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .h(px(TOOL_LINE_HEIGHT))
                            .flex()
                            .items_center()
                            .truncate()
                            .child(group_title(summary.clone(), shimmer_phase, &t)),
                    )
                    .child(
                        svg()
                            .path(ALT_ARROW_RIGHT)
                            .flex_none()
                            .size(px(DISCLOSURE_CHEVRON_SIZE))
                            .with_transformation(Transformation::rotate(radians(
                                FRAC_PI_2 * disclosure,
                            )))
                            .text_color(t.content_faint),
                    ),
            );

        let count = self.cards.len();
        let rows = self
            .cards
            .into_iter()
            .zip(keys)
            .zip(geometries)
            .enumerate()
            .map(|(ix, ((mut card, key), geometry))| {
                card.slot = Some(Slot {
                    state: state.clone(),
                    key,
                    geometry,
                    rail: Rail {
                        has_predecessor: ix > 0,
                        continues: ix + 1 < count,
                        reveal: reveal[ix],
                        connector: connector[ix],
                        continuation: motion::tool_connector_continuation(
                            connector.get(ix + 1).copied(),
                        ),
                    },
                });
                card
            });
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
                TOOL_GROUP_HEADER_HEIGHT,
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

const LOG: &str = "git log -3 --format='%h %an %s' -- src/cart/total.rs";
const LOG_OUT: &str = "a41c9e2 Mara Lindqvist Apply tax before coupons in checkout_total
7d02b5f Jonas Weber Round line totals to the cent
19e6a3c Mara Lindqvist Split cart totals into their own module";

const ASK: &str = "The checkout total test is failing on main. Can you find out why?";
const REPLY: &str = "It broke in a41c9e2, which moved add_tax ahead of apply_discount. The $10 coupon now comes off the taxed total, so the test's $100 cart checks out at $98.00 instead of $97.20. Putting the coupon back before tax fixes it – want me to make that change?";

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

/// Pink's summary wording for the demo's calls.
fn demo_summary(commands: usize, reads: usize, searches: usize) -> String {
    let mut segments = Vec::new();
    if commands > 0 {
        segments.push(format!("ran {}", plural(commands, "command", "commands")));
    }
    if reads > 0 {
        segments.push(format!("read {}", plural(reads, "file", "files")));
    }
    if searches > 0 {
        segments.push(format!("searched {}", plural(searches, "time", "times")));
    }
    let mut summary = segments.join(" · ");
    if let Some(first) = summary.get(0..1) {
        let upper = first.to_uppercase();
        summary.replace_range(0..1, &upper);
    }
    summary
}

/// A user bubble (pink transcript): right-aligned, at most 80% wide,
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
            ToolCallCard::new("read", "Read", done(1))
                .icon(icon("file-text"))
                .file("src/cart/total.rs")
                .args("src/cart/total.rs")
                .when(step >= 1, |c| c.result(TOTAL_RS)),
        );
    if step >= 1 {
        group = group.child(
            ToolCallCard::new("search", "Search", done(2))
                .icon(icon("file-search"))
                .summary("apply_discount in src")
                .args("apply_discount in src")
                .when(step >= 2, |c| c.result(GREP)),
        );
    }
    if step >= 2 {
        let streamed = step.saturating_sub(2).min(TEST_OUT.len());
        group = group.child(
            ToolCallCard::new("test", "Run", if step >= 6 { Error } else { Running })
                .icon(icon("terminal"))
                .summary(TEST)
                .command(TEST)
                .when(streamed > 0, |c| c.result(TEST_OUT[..streamed].join("\n"))),
        );
    }
    if step >= 6 {
        group = group.child(
            ToolCallCard::new("log", "Run", done(8))
                .icon(icon("terminal"))
                .summary(LOG)
                .command(LOG)
                .when(step >= 7, |c| c.result(LOG_OUT)),
        );
    }
    let commands = usize::from(step >= 2) + usize::from(step >= 6);
    let group = group.label(demo_summary(commands, 1, usize::from(step >= 1)));

    // The working line under a live run (pink's working trailer)
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
    fn summary_follows_pink() {
        assert_eq!(group_summary(None, 3, 0), "Called 3 tools");
        assert_eq!(group_summary(None, 1, 1), "Called 1 tool · 1 failed");
        assert_eq!(
            group_summary(Some("Ran 2 commands"), 2, 1),
            "Ran 2 commands · 1 failed"
        );
        assert_eq!(
            demo_summary(2, 1, 1),
            "Ran 2 commands · read 1 file · searched 1 time"
        );
        assert_eq!(single_line("a\n  b\tc"), "a b c");
        assert_eq!(file_name("src/cart/total.rs"), "total.rs");
        assert_eq!(file_icon("src/cart/total.rs", true), FILE_RUST_DARK);
        assert_eq!(file_icon("Cargo.lock", false), FILE_DOCUMENT);
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
