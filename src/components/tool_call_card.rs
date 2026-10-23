//! Tool Call Card – one tool call of an AI agent in a single line: the
//! tool's icon, its name in mono, a short summary and the status (a spinner
//! with a live timer, a check with the duration, or "Failed").
//!
//! Click the line open to see the arguments and the result as highlighted
//! JSON, each with a copy button; long blocks are capped with a fade and
//! "Show all". A failed call opens itself to show the error and a Retry
//! button, and closes again on the next run unless someone toggled it.
//! [`ToolCallGroup`] folds several cards into one quiet line that sums them
//! up: "Used 3 tools · 2.4s", with a light sweeping over "Using tools…"
//! while any of them runs.
//!
//! Clean: one white card with hairline shadows and a plain glyph. Classic:
//! a bordered card with the glyph on a gray tile.
//!
//! Enter or Space toggles a focused line; inside a group, the arrow keys,
//! Home and End move between the lines.
//!
//! ```ignore
//! ToolCallGroup::new("tools")
//!     .child(
//!         ToolCallCard::new("search", "search_web", ToolCallStatus::Success)
//!             .summary("Searched the web for 'CRM pricing 2026'")
//!             .args(Json::object([("query", "CRM pricing 2026".into())]))
//!             .duration(Duration::from_millis(1500)),
//!     )
//! ```

use std::{cell::Cell, f32::consts::PI, ops::Range, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AlignItems, Animation, AnyElement, AnyView, App, AsyncApp, BoxShadow, ClipboardItem, Context,
    DefiniteLength, Div, ElementId, Entity, FocusHandle, FontWeight, HighlightStyle, Hsla,
    IntoElement, KeyDownEvent, MouseButton, Pixels, SharedString, StyledText, Task, Transformation,
    WeakEntity, Window, canvas, div, linear_color_stop, linear_gradient, prelude::*, px, radians,
    relative, size, svg,
};

use crate::{
    ActiveTheme, Look,
    motion::{
        self, AnimationExt as _, CubicBezier, EASE, EASE_OUT_BACK, EASE_OUT_QUINT, MotionSpec,
    },
    primitives::{MONO, SANS, black_a, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Tool Call Card";
pub const DESCRIPTION: &str = "An agent's tool call in one line – live timer, status and summary, folds open to highlighted arguments and result with copy, retry on errors and a group that sums them up";

/// `ui-tcc-in`: the status and the group title settle in after a change.
const SETTLE: MotionSpec = MotionSpec::new(360, EASE_OUT_QUINT);
/// The body growing from `0fr` to `1fr`.
const OPEN: MotionSpec = MotionSpec::new(380, EASE_OUT_QUINT);
/// The body's opacity.
const BODY_FADE: MotionSpec = MotionSpec::new(240, EASE);
/// The chevron turning.
const TURN: MotionSpec = MotionSpec::new(360, EASE_OUT_QUINT);
/// A capped code block opening to its full height.
const CAP: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT);
/// The fade over a capped block.
const CAP_FADE: MotionSpec = MotionSpec::new(240, EASE);
/// The tool icons popping into the group's stack.
const STACK_POP: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT);
/// Copy ↔ check swap: opacity and scale.
const COPY_FADE: MotionSpec = MotionSpec::new(200, EASE);
const COPY_SCALE: MotionSpec = MotionSpec::new(320, EASE_OUT_QUINT);
const COPY_COLOR: MotionSpec = MotionSpec::new(160, EASE);
const SPIN: Duration = Duration::from_millis(800);
const SWEEP: Duration = Duration::from_millis(1800);
/// How long the copy button shows its check.
const COPIED_FOR: Duration = Duration::from_millis(1400);

const WRENCH: &str = "icons/tool-call-card/wrench.svg";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type OpenHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;

thread_local! {
    /// Focus came from the keyboard, so focused lines draw a ring
    /// (`:focus-visible`). A click clears it.
    static KEYBOARD_FOCUS: Cell<bool> = const { Cell::new(false) };
}

fn keyboard_focus() -> bool {
    KEYBOARD_FOCUS.with(|k| k.get())
}

fn set_keyboard_focus(on: bool) {
    KEYBOARD_FOCUS.with(|k| k.set(on));
}

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
        self.highlighted().text
    }

    fn highlighted(&self) -> Highlighted {
        let mut out = Highlighted::default();
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

/// What a code section shows: JSON (pretty-printed and highlighted) or
/// plain text. Strings that hold JSON are parsed.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolPayload {
    Json(Json),
    Text(SharedString),
}

impl ToolPayload {
    fn highlighted(&self) -> Highlighted {
        match self {
            ToolPayload::Json(value) => value.highlighted(),
            ToolPayload::Text(text) => {
                let trimmed = text.trim();
                if (trimmed.starts_with('{') || trimmed.starts_with('['))
                    && let Some(value) = Json::parse(trimmed)
                {
                    return value.highlighted();
                }
                Highlighted {
                    text: text.to_string(),
                    spans: Vec::new(),
                }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Token {
    Key,
    Str,
    Num,
    Lit,
}

/// Printed text plus the spans to color.
#[derive(Default, Debug)]
struct Highlighted {
    text: String,
    spans: Vec<(Range<usize>, Token)>,
}

impl Highlighted {
    fn push(&mut self, text: &str, token: Option<Token>) {
        let start = self.text.len();
        self.text.push_str(text);
        if let Some(token) = token {
            self.spans.push((start..self.text.len(), token));
        }
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

fn write_json(value: &Json, depth: usize, out: &mut Highlighted) {
    let indent = |depth: usize| "  ".repeat(depth);
    match value {
        Json::Null => out.push("null", Some(Token::Lit)),
        Json::Bool(b) => out.push(if *b { "true" } else { "false" }, Some(Token::Lit)),
        Json::Number(n) if n.is_finite() => out.push(&js_number(*n), Some(Token::Num)),
        Json::Number(_) => out.push("null", Some(Token::Lit)),
        Json::String(s) => out.push(&quote(s), Some(Token::Str)),
        Json::Array(items) if items.is_empty() => out.push("[]", None),
        Json::Array(items) => {
            out.push("[\n", None);
            for (i, item) in items.iter().enumerate() {
                out.push(&indent(depth + 1), None);
                write_json(item, depth + 1, out);
                out.push(if i + 1 < items.len() { ",\n" } else { "\n" }, None);
            }
            out.push(&indent(depth), None);
            out.push("]", None);
        }
        Json::Object(fields) if fields.is_empty() => out.push("{}", None),
        Json::Object(fields) => {
            out.push("{\n", None);
            for (i, (key, item)) in fields.iter().enumerate() {
                out.push(&indent(depth + 1), None);
                out.push(&quote(key), Some(Token::Key));
                out.push(": ", None);
                write_json(item, depth + 1, out);
                out.push(if i + 1 < fields.len() { ",\n" } else { "\n" }, None);
            }
            out.push(&indent(depth), None);
            out.push("}", None);
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

/// 0.4s, 12.3s, 2m 05s – the live timer always shows tenths below a minute.
pub fn format_ms(ms: f64, live: bool) -> String {
    if !live && ms < 1000. {
        return format!("{}ms", ms.round().max(1.));
    }
    if ms < 60_000. {
        return format!("{:.1}s", (ms / 100.).floor() / 10.);
    }
    let s = (ms / 1000.).floor() as u64;
    format!("{}m {:02}s", s / 60, s % 60)
}

/// Live time while running, the final value after. `accumulate` adds up
/// several running stretches (the group); a card restarts on every run.
#[derive(Default)]
struct Stopwatch {
    start: Option<Instant>,
    started_at: Option<Instant>,
    base: Duration,
    value: Option<Duration>,
}

impl Stopwatch {
    fn sync(&mut self, running: bool, started_at: Option<Instant>, accumulate: bool) {
        let now = Instant::now();
        if running {
            if self.start.is_none() || self.started_at != started_at {
                if !accumulate {
                    self.base = Duration::ZERO;
                }
                self.start = Some(started_at.unwrap_or(now));
                self.started_at = started_at;
            }
        } else if let Some(start) = self.start.take() {
            self.base += now.saturating_duration_since(start);
            self.value = Some(self.base);
        }
    }

    fn ms(&self) -> Option<f64> {
        match self.start {
            Some(start) => Some(
                (self.base + Instant::now().saturating_duration_since(start)).as_secs_f64() * 1e3,
            ),
            None => self.value.map(|v| v.as_secs_f64() * 1e3),
        }
    }
}

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

/// The `--ui-tc-*` / `--ui-tcc-*` tokens, resolved to what each part uses.
struct Palette {
    card: Hsla,
    card_shadow: Vec<BoxShadow>,
    /// Classic: `inset 0 0 0 1px` around the card.
    card_edge: Option<Hsla>,
    /// The tool name and the Retry label.
    strong: Hsla,
    ink: Hsla,
    text: Hsla,
    muted: Hsla,
    /// The group's time.
    faint: Hsla,
    /// The tool glyph.
    glyph: Hsla,
    /// Classic: the glyph's tile and its edge.
    tile: Option<(Hsla, Hsla)>,
    /// Classic: the ring that separates stacked tiles.
    ring: Hsla,
    chevron: Hsla,
    chevron_hover: Hsla,
    line: Hsla,
    head_hover: Hsla,
    group_fg: Hsla,
    group_hover: Hsla,
    running_label: Hsla,
    /// Spinner arc, the done check (Clean: its dot).
    accent: Hsla,
    spinner_track: Hsla,
    code: Hsla,
    red: Hsla,
    red_bg: Hsla,
    red_line: Hsla,
    key: Hsla,
    string: Hsla,
    number: Hsla,
    literal: Hsla,
    shine: Hsla,
    /// Clean: the white copy button's fill and shadow.
    copy_plate: Option<(Hsla, Vec<BoxShadow>)>,
    copy_fg: Hsla,
    copy_hover: Hsla,
    copied: Hsla,
    retry_shadow: Vec<BoxShadow>,
    retry_edge: Option<Hsla>,
    retry_icon: Hsla,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, dark) => {
                let (card, strong, ink, text, muted, faint, icon, line, wash, wash_big, code) =
                    if dark {
                        (
                            hex(0x1c1c1e),
                            hex(0xffffff),
                            white_a(0.9),
                            white_a(0.62),
                            white_a(0.46),
                            white_a(0.3),
                            white_a(0.5),
                            white_a(0.08),
                            white_a(0.04),
                            white_a(0.025),
                            hex(0x212123),
                        )
                    } else {
                        (
                            hex(0xffffff),
                            hex(0x000000),
                            black_a(0.88),
                            black_a(0.61),
                            black_a(0.45),
                            black_a(0.27),
                            black_a(0.447),
                            black_a(0.06),
                            black_a(0.03),
                            black_a(0.015),
                            hex(0xfafafa),
                        )
                    };
                let accent = if dark {
                    hex(0x4d8dff)
                } else {
                    hexa(0x0060e2, 0.765)
                };
                let card_shadow = if dark {
                    vec![
                        shadow(0., 0., 0., 0.5, white_a(0.1)),
                        shadow(0., 1., 2., 0., black_a(0.4)),
                    ]
                } else {
                    vec![
                        shadow(0., 0., 0., 0.5, black_a(0.06)),
                        shadow(0., 1., 1., -1., black_a(0.1)),
                        shadow(0., 1., 2., 0., black_a(0.05)),
                    ]
                };
                Palette {
                    card,
                    card_shadow: card_shadow.clone(),
                    card_edge: None,
                    strong,
                    ink,
                    text,
                    muted,
                    faint,
                    glyph: icon,
                    tile: None,
                    ring: card,
                    chevron: icon,
                    chevron_hover: ink,
                    line,
                    head_hover: wash_big,
                    group_fg: ink,
                    group_hover: wash,
                    running_label: muted,
                    accent,
                    spinner_track: line,
                    code,
                    red: if dark { hex(0xff7b72) } else { hex(0xd93a2b) },
                    red_bg: if dark {
                        hexa(0xff6e64, 0.07)
                    } else {
                        hexa(0xd93a2b, 0.04)
                    },
                    red_line: if dark {
                        hexa(0xff6e64, 0.18)
                    } else {
                        hexa(0xd93a2b, 0.14)
                    },
                    key: ink,
                    string: if dark {
                        hex(0x5fc8e8)
                    } else {
                        hexa(0x007bac, 0.9)
                    },
                    number: if dark { hex(0xf2a65a) } else { hex(0xb25b0c) },
                    literal: if dark { hex(0xa5a5ff) } else { hex(0x5b5bd6) },
                    shine: if dark { hex(0xffffff) } else { white_a(0.82) },
                    copy_plate: Some((card, card_shadow.clone())),
                    copy_fg: icon,
                    copy_hover: wash_big,
                    copied: accent,
                    retry_shadow: card_shadow,
                    retry_edge: None,
                    retry_icon: icon,
                    focus: if dark {
                        hexa(0x4d8dff, 0.5)
                    } else {
                        hexa(0x0060e2, 0.35)
                    },
                }
            }
            (Look::Classic, dark) => {
                let (ink, text, muted, bg, line, hover, tile, code) = if dark {
                    (
                        hex(0xededed),
                        hex(0xa1a1a1),
                        hex(0x7c7c7c),
                        hex(0x161618),
                        white_a(0.1),
                        white_a(0.035),
                        hex(0x1d1d20),
                        hex(0x1d1d20),
                    )
                } else {
                    (
                        hex(0x171717),
                        hex(0x6b6b6b),
                        hex(0x8a8a8a),
                        hex(0xffffff),
                        hex(0xe7e7e7),
                        hex(0xfafafa),
                        hex(0xf5f5f5),
                        hex(0xfafafa),
                    )
                };
                let lift = if dark {
                    vec![]
                } else {
                    vec![shadow(0., 1., 2., 0., black_a(0.04))]
                };
                Palette {
                    card: bg,
                    card_shadow: lift,
                    card_edge: Some(line),
                    strong: ink,
                    ink,
                    text,
                    muted,
                    faint: muted,
                    glyph: ink,
                    tile: Some((tile, line)),
                    ring: bg,
                    chevron: muted,
                    chevron_hover: muted,
                    line,
                    head_hover: hover,
                    group_fg: text,
                    group_hover: hover,
                    running_label: text,
                    accent: ink,
                    spinner_track: muted.opacity(0.22),
                    code,
                    red: if dark { hex(0xff7b72) } else { hex(0xc8372d) },
                    red_bg: if dark {
                        hexa(0xff6e64, 0.08)
                    } else {
                        hexa(0xc8372d, 0.06)
                    },
                    red_line: if dark {
                        hexa(0xff6e64, 0.2)
                    } else {
                        hexa(0xc8372d, 0.16)
                    },
                    key: ink,
                    string: if dark { hex(0x5fd4b8) } else { hex(0x0f7b6c) },
                    number: if dark { hex(0xf2a65a) } else { hex(0xb25b0c) },
                    literal: if dark { hex(0xb5a4ff) } else { hex(0x6e56cf) },
                    shine: if dark { white_a(0.95) } else { white_a(0.82) },
                    copy_plate: None,
                    copy_fg: muted,
                    copy_hover: hover,
                    copied: ink,
                    retry_shadow: vec![shadow(0., 1., 2., 0., black_a(0.04))],
                    retry_edge: Some(line),
                    retry_icon: ink,
                    focus: if dark {
                        white_a(0.3)
                    } else {
                        hexa(0x171717, 0.22)
                    },
                }
            }
        }
    }
}

/// Sizes that differ between the two stylesheets.
struct Metrics {
    font_size: Pixels,
    line_height: DefiniteLength,
    radius: Pixels,
    head_pad: [Pixels; 4],
    /// The glyph, and Classic's tile around it.
    glyph: Pixels,
    tile: Option<Pixels>,
    name_size: Pixels,
    status_size: Pixels,
    status_line: DefiniteLength,
    chevron: Pixels,
    chevron_path: &'static str,
    sections_gap: Pixels,
    sections_pad: [Pixels; 4],
    hairline: Pixels,
    section_head: Pixels,
    label_size: Pixels,
    label_line: DefiniteLength,
    small: Pixels,
    small_line: DefiniteLength,
    copy_box: Pixels,
    copy_glyph: Pixels,
    code_line: DefiniteLength,
    focus_offset: Pixels,
}

impl Metrics {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => Metrics {
                font_size: px(13.),
                line_height: px(20.).into(),
                radius: px(10.),
                head_pad: [px(0.), px(14.), px(0.), px(14.)],
                glyph: px(16.),
                tile: None,
                name_size: px(12.),
                status_size: px(12.),
                status_line: px(16.).into(),
                chevron: px(12.),
                chevron_path: "icons/tool-call-card/clean/chevron.svg",
                sections_gap: px(14.),
                sections_pad: [px(12.), px(14.), px(14.), px(14.)],
                hairline: px(0.5),
                section_head: px(28.),
                label_size: px(10.),
                label_line: px(16.).into(),
                small: px(12.),
                small_line: px(16.).into(),
                copy_box: px(28.),
                copy_glyph: px(16.),
                code_line: px(20.).into(),
                focus_offset: px(1.),
            },
            Look::Classic => Metrics {
                font_size: px(13.5),
                line_height: relative(1.4),
                radius: px(12.),
                head_pad: [px(8.), px(10.), px(8.), px(9.)],
                glyph: px(14.),
                tile: Some(px(26.)),
                name_size: px(12.5),
                status_size: px(12.5),
                status_line: relative(1.4),
                chevron: px(14.),
                chevron_path: "icons/tool-call-card/chevron.svg",
                sections_gap: px(12.),
                sections_pad: [px(12.); 4],
                hairline: px(1.),
                section_head: px(24.),
                label_size: px(12.),
                label_line: relative(1.4),
                small: px(12.5),
                small_line: relative(1.4),
                copy_box: px(24.),
                copy_glyph: px(14.),
                code_line: relative(1.6),
                focus_offset: px(2.),
            },
        }
    }
}

fn icon_path(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/tool-call-card/clean/{name}.svg").into(),
        Look::Classic => format!("icons/tool-call-card/{name}.svg").into(),
    }
}

// ---------------------------------------------------------------------------
// Small pieces
// ---------------------------------------------------------------------------

/// `white-space: nowrap; overflow: hidden; text-overflow: ellipsis`.
///
/// GPUI's `truncate()` caches the first (unconstrained) measurement of
/// unwrapped text, so in a flex row it clips without an ellipsis. Wrapping
/// text clamped to one line is measured again once its width is known.
fn one_line(element: Div) -> Div {
    element.whitespace_normal().text_ellipsis().line_clamp(1)
}

/// `align-self` (GPUI has no shorthand for it).
fn align_self<E: Styled>(mut element: E, align: AlignItems) -> E {
    element.style().align_self = Some(align);
    element
}

/// `box-shadow: inset 0 0 0 <width> <color>`: an outline drawn over the
/// background and under the content – add it as the first child.
fn inset_edge(width: Pixels, color: Hsla, radius: Pixels) -> Div {
    div()
        .absolute()
        .inset_0()
        .rounded(radius)
        .border(width)
        .border_color(color)
}

/// `outline: 2px solid; outline-offset: <offset>` for keyboard focus.
fn focus_ring(offset: Pixels, radius: Pixels, color: Hsla) -> Div {
    let o = offset + px(2.);
    div()
        .absolute()
        .top(-o)
        .left(-o)
        .right(-o)
        .bottom(-o)
        .rounded(radius + o)
        .border_2()
        .border_color(color)
}

/// Reports the parent's laid-out height (in the first frame it appears,
/// before paint) so collapsing parts know how far to open.
fn measure<T: 'static>(
    state: Entity<T>,
    write: impl Fn(&mut T, f32) -> bool + 'static,
) -> impl IntoElement {
    canvas(
        move |bounds, _, cx| {
            let height = f32::from(bounds.size.height);
            state.update(cx, |s, cx| {
                if write(s, height) {
                    cx.notify();
                }
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// Writes `value` into `slot`, telling whether it moved.
fn store(slot: &mut f32, value: f32) -> bool {
    let changed = (*slot - value).abs() > 0.1;
    *slot = value;
    changed
}

/// The body under a header: grows from 0 to its natural height and fades in,
/// like the CSS `grid-template-rows: 0fr → 1fr`. It always renders (clipped
/// to nothing when closed), so the cards inside keep their state.
fn reveal(key: &str, open: bool, natural: f32, content: impl IntoElement) -> Div {
    let target = if open { 1. } else { 0. };
    let grow = motion::follow(&format!("{key}-grow"), target, OPEN);
    let fade = motion::follow(&format!("{key}-fade"), target, BODY_FADE);
    div()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .when(!(open && grow >= 1.), |d| d.h(px(natural * grow)))
        .opacity(fade)
        .child(content)
}

/// An unclamped cubic bezier, for curves that overshoot (`ui-tcc-pop`).
fn bezier(curve: CubicBezier, x: f32) -> f32 {
    let axis = |a: f32, b: f32, t: f32| {
        let u = 1. - t;
        3. * u * u * t * a + 3. * u * t * t * b + t * t * t
    };
    let (mut lo, mut hi) = (0f32, 1f32);
    for _ in 0..32 {
        let mid = (lo + hi) / 2.;
        if axis(curve.x1, curve.x2, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    axis(curve.y1, curve.y2, (lo + hi) / 2.)
}

/// How far (in viewBox units) a check drawn along `points` reaches at `f`.
fn drawn_x(points: [(f32, f32); 3], f: f32) -> f32 {
    let dist = |a: (f32, f32), b: (f32, f32)| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let (l1, l2) = (dist(points[0], points[1]), dist(points[1], points[2]));
    let s = f.clamp(0., 1.) * (l1 + l2);
    if s <= l1 {
        motion::lerp(points[0].0, points[1].0, s / l1)
    } else {
        motion::lerp(points[1].0, points[2].0, (s - l1) / l2)
    }
}

/// `top` painted over an opaque `bottom`. GPUI draws a shadow under the whole
/// box, so a control with a shadow needs an opaque fill where CSS would let
/// a translucent hover wash show the panel behind it.
fn over(top: Hsla, bottom: Hsla) -> Hsla {
    motion::mix(bottom, top.alpha(1.), top.a)
}

fn scaled(scale: f32) -> Transformation {
    Transformation::scale(size(scale, scale))
}

/// The check that draws itself (`stroke-dashoffset` → 0), shown as a
/// left-to-right wipe: the path only ever moves right.
fn drawing_check(
    path: SharedString,
    box_size: f32,
    color: Hsla,
    points: [(f32, f32); 3],
    half_stroke: f32,
    drawn: f32,
    scale: f32,
) -> Div {
    let reach = if drawn <= 0. {
        0.
    } else {
        (drawn_x(points, drawn) + half_stroke) * box_size / 16.
    };
    div()
        .absolute()
        .top_0()
        .left_0()
        .h(px(box_size))
        .w(px(reach))
        .overflow_hidden()
        .child(
            svg()
                .path(path)
                .size(px(box_size))
                .text_color(color)
                .with_transformation(scaled(scale)),
        )
}

/// The status glyph: spinner, check or error.
fn status_icon(
    look: Look,
    status: ToolCallStatus,
    p: &Palette,
    reduced: bool,
    ids: &str,
) -> AnyElement {
    let s = px(14.);
    match status {
        ToolCallStatus::Running => {
            let arc = svg()
                .absolute()
                .top_0()
                .left_0()
                .path(icon_path(look, "spinner-arc"))
                .size(s)
                .text_color(p.accent);
            div()
                .relative()
                .flex_none()
                .size(s)
                .child(
                    svg()
                        .path(icon_path(look, "spinner-track"))
                        .size(s)
                        .text_color(p.spinner_track),
                )
                .child(if reduced {
                    arc.into_any_element()
                } else {
                    arc.with_animation(
                        ElementId::Name(format!("{ids}-spin").into()),
                        Animation::new(SPIN).repeat(),
                        |arc, t| {
                            arc.with_transformation(Transformation::rotate(radians(t * 2. * PI)))
                        },
                    )
                    .into_any_element()
                })
                .into_any_element()
        }
        ToolCallStatus::Success => {
            let clean = look == Look::Clean;
            let accent = p.accent;
            // Clean: a blue dot pops in (460ms, overshooting) and its white
            // check draws after 160ms. Classic: the check draws after 80ms.
            let (pop_ms, delay_ms) = if clean { (460., 160.) } else { (0., 80.) };
            let total_ms = (delay_ms + 360f32).max(pop_ms);
            let frame = move |t: f32| {
                let ms = t * total_ms;
                let (scale, alpha) = if clean {
                    let x = (ms / pop_ms).clamp(0., 1.);
                    let s = bezier(EASE_OUT_BACK, x);
                    (motion::lerp(0.6, 1., s), s.clamp(0., 1.))
                } else {
                    (1., 1.)
                };
                let drawn = EASE_OUT_QUINT.eval((ms - delay_ms) / 360.);
                let base = div().relative().flex_none().size(s).opacity(alpha);
                if clean {
                    base.child(
                        svg()
                            .path("icons/tool-call-card/clean/dot.svg")
                            .size(s)
                            .text_color(accent)
                            .with_transformation(scaled(scale)),
                    )
                    .child(drawing_check(
                        "icons/tool-call-card/clean/check.svg".into(),
                        14.,
                        hex(0xffffff),
                        [(5.1, 8.2), (7.1, 10.1), (10.9, 6.)],
                        0.75,
                        drawn,
                        scale,
                    ))
                } else {
                    base.child(drawing_check(
                        "icons/tool-call-card/check.svg".into(),
                        14.,
                        accent,
                        [(3.5, 8.4), (6.6, 11.3), (12.5, 4.9)],
                        0.9,
                        drawn,
                        1.,
                    ))
                }
            };
            if reduced {
                frame(1.).into_any_element()
            } else {
                div()
                    .flex_none()
                    .with_animation(
                        ElementId::Name(format!("{ids}-done").into()),
                        Animation::new(
                            Duration::from_secs_f32(total_ms / 1e3).mul_f32(motion::speed_scale()),
                        ),
                        move |el, t| el.child(frame(t)),
                    )
                    .into_any_element()
            }
        }
        ToolCallStatus::Error => error_icon(look, p, s).into_any_element(),
    }
}

/// Clean: a red dot with a white mark. Classic: a red outlined circle.
fn error_icon(look: Look, p: &Palette, s: Pixels) -> Div {
    match look {
        Look::Clean => div()
            .relative()
            .flex_none()
            .size(s)
            .child(
                svg()
                    .path("icons/tool-call-card/clean/dot.svg")
                    .size(s)
                    .text_color(p.red),
            )
            .child(
                svg()
                    .absolute()
                    .top_0()
                    .left_0()
                    .path("icons/tool-call-card/clean/error-mark.svg")
                    .size(s)
                    .text_color(hex(0xffffff)),
            ),
        Look::Classic => div().flex_none().size(s).child(
            svg()
                .path("icons/tool-call-card/error.svg")
                .size(s)
                .text_color(p.red),
        ),
    }
}

/// The shine sweeping over "Using tools…": a soft band of `shine` crossing
/// the label left to right, tinted per character.
fn shine_text(text: &SharedString, t: f32, base: Hsla, shine: Hsla) -> StyledText {
    let count = text.chars().count().max(1) as f32;
    let center = -0.5 + 2. * t;
    let solid = shine.alpha(1.);
    let highlights: Vec<_> = text
        .char_indices()
        .enumerate()
        .map(|(i, (at, ch))| {
            let u = (i as f32 + 0.5) / count;
            let band = (1. - (u - center).abs() / 0.36).max(0.) * shine.a;
            (
                at..at + ch.len_utf8(),
                HighlightStyle {
                    color: Some(motion::mix(base, solid, band)),
                    ..Default::default()
                },
            )
        })
        .collect();
    StyledText::new(text.clone()).with_highlights(highlights)
}

// ---------------------------------------------------------------------------
// Card
// ---------------------------------------------------------------------------

#[derive(Default)]
struct CodeState {
    /// Natural height of the block, padding included.
    natural: f32,
    full: bool,
    copied: bool,
    reset: Option<Task<()>>,
}

struct CardState {
    open: bool,
    auto_opened: bool,
    prev_status: ToolCallStatus,
    watch: Stopwatch,
    body: f32,
    code: [CodeState; 2],
    retry_pressed: Option<Instant>,
    focus: FocusHandle,
}

#[derive(IntoElement)]
pub struct ToolCallCard {
    id: ElementId,
    name: SharedString,
    summary: Option<SharedString>,
    icon: Option<SharedString>,
    status: ToolCallStatus,
    args: Option<ToolPayload>,
    result: Option<ToolPayload>,
    error: Option<SharedString>,
    started_at: Option<Instant>,
    duration: Option<Duration>,
    default_open: bool,
    open: Option<bool>,
    on_open_change: Option<OpenHandler>,
    open_on_error: bool,
    on_retry: Option<Handler>,
    max_code_height: Pixels,
    look: Option<Look>,
    /// Set by a group: a prefix that keeps motion keys unique, and the
    /// group's focus handle for this line.
    scope: Option<String>,
    focus: Option<FocusHandle>,
}

impl ToolCallCard {
    /// `name` is the tool as the agent calls it, shown in mono – e.g. "search_web".
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        status: ToolCallStatus,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            summary: None,
            icon: None,
            status,
            args: None,
            result: None,
            error: None,
            started_at: None,
            duration: None,
            default_open: false,
            open: None,
            on_open_change: None,
            open_on_error: true,
            on_retry: None,
            max_code_height: px(220.),
            look: None,
            scope: None,
            focus: None,
        }
    }

    /// Short human summary – e.g. "Searched the web for 'CRM pricing 2026'".
    pub fn summary(mut self, summary: impl Into<SharedString>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// A 16×16 stroke icon (SVG path under `assets/`) – defaults to a wrench.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// Arguments the tool was called with.
    pub fn args(mut self, args: impl Into<ToolPayload>) -> Self {
        self.args = Some(args.into());
        self
    }

    /// What the tool returned.
    pub fn result(mut self, result: impl Into<ToolPayload>) -> Self {
        self.result = Some(result.into());
        self
    }

    /// Error message, shown when the status is `Error`.
    pub fn error(mut self, error: impl Into<SharedString>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// When the call started – the live timer counts from here.
    pub fn started_at(mut self, at: Instant) -> Self {
        self.started_at = Some(at);
        self
    }

    /// Final duration – otherwise measured while the card shows `Running`.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Start expanded (uncontrolled).
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
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

    /// Opens the card when the call fails and closes it again on the next
    /// run, unless someone toggled it (default on).
    pub fn open_on_error(mut self, on: bool) -> Self {
        self.open_on_error = on;
        self
    }

    /// Shows a Retry button next to the error.
    pub fn on_retry(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_retry = Some(Rc::new(handler));
        self
    }

    /// Height after which arguments and result are capped with a fade and
    /// "Show all" (default 220px).
    pub fn max_code_height(mut self, height: Pixels) -> Self {
        self.max_code_height = height;
        self
    }

    /// Overrides the global Clean/Classic look for this card.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for ToolCallCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx);
        let m = Metrics::get(look);
        let key = match &self.scope {
            Some(scope) => format!("{scope}/{}", self.id),
            None => self.id.to_string(),
        };
        let reduced = motion::reduced_motion(cx);
        let status = self.status;
        let running = status == ToolCallStatus::Running;
        let failed = status == ToolCallStatus::Error;

        let default_open = self.default_open;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |_, cx| CardState {
                open: default_open,
                auto_opened: false,
                prev_status: status,
                watch: Stopwatch::default(),
                body: 0.,
                code: Default::default(),
                retry_pressed: None,
                focus: cx.focus_handle(),
            },
        );

        // Open on failure, close again on the next run – only while nobody
        // has toggled it by hand
        let controlled = self.open;
        let open_on_error = self.open_on_error;
        let started_at = self.started_at;
        state.update(cx, |s, _| {
            if s.prev_status != status {
                let prev = s.prev_status;
                s.prev_status = status;
                if controlled.is_none() && open_on_error {
                    if failed && !s.open {
                        s.open = true;
                        s.auto_opened = true;
                    } else if prev == ToolCallStatus::Error && s.auto_opened {
                        s.open = false;
                        s.auto_opened = false;
                    }
                }
            }
            s.watch.sync(running, started_at, false);
        });

        let (open, measured, body_h, focus, retry_pressed) = {
            let s = state.read(cx);
            (
                controlled.unwrap_or(s.open),
                s.watch.ms(),
                s.body,
                self.focus.clone().unwrap_or_else(|| s.focus.clone()),
                s.retry_pressed,
            )
        };
        let ms = if running {
            Some(measured.unwrap_or(0.))
        } else {
            self.duration.map(|d| d.as_secs_f64() * 1e3).or(measured)
        };
        let time: Option<SharedString> = if failed {
            Some("Failed".into())
        } else {
            ms.map(|ms| format_ms(ms, running).into())
        };
        if running {
            // The live timer ticks every frame the spinner turns; this
            // keeps it ticking with reduced motion too.
            window.request_animation_frame();
        }

        let toggle: Handler = {
            let state = state.clone();
            let on_change = self.on_open_change.clone();
            Rc::new(move |window, cx| {
                state.update(cx, |s, cx| {
                    s.auto_opened = false;
                    if controlled.is_none() {
                        s.open = !open;
                    }
                    cx.notify();
                });
                if let Some(f) = &on_change {
                    f(!open, window, cx);
                }
            })
        };

        // Header
        let head_key = format!("{key}-head");
        let hover = motion::hover_t(&head_key);
        let turn = motion::follow(&format!("{key}-turn"), if open { 1. } else { 0. }, TURN);
        let body_key = format!("{key}-body");
        let squared = open
            || motion::follow(
                &format!("{body_key}-grow"),
                if open { 1. } else { 0. },
                OPEN,
            ) > 0.;

        let glyph = svg()
            .path(self.icon.clone().unwrap_or(WRENCH.into()))
            .size(m.glyph)
            .flex_none()
            .text_color(p.glyph);
        let tool_icon = match (m.tile, p.tile) {
            (Some(tile), Some((fill, edge))) => div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(tile)
                .rounded(px(7.))
                .bg(fill)
                .border_1()
                .border_color(edge)
                .child(glyph)
                .into_any_element(),
            _ => glyph.into_any_element(),
        };

        let main = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex_shrink_1()
                    .max_w(relative(0.55))
                    .map(one_line)
                    .font_family(MONO)
                    .text_size(m.name_size)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.strong)
                    .child(self.name.clone()),
            )
            .when_some(self.summary.clone(), |d, summary| {
                d.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .map(one_line)
                        .text_color(p.text)
                        .child(summary),
                )
            });

        let status_view = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .flex_none()
            .whitespace_nowrap()
            .text_size(m.status_size)
            .line_height(m.status_line)
            .text_color(if failed { p.red } else { p.muted })
            .when(failed && clean, |d| d.font_weight(FontWeight::MEDIUM))
            .child(status_icon(look, status, &p, reduced, &key))
            .children(time);
        // Keyed on the status, so every change settles in again
        let status_view = if reduced {
            status_view.into_any_element()
        } else {
            status_view
                .with_animation(
                    ElementId::Name(format!("{key}-status-{status:?}").into()),
                    SETTLE.animation(),
                    |el, t| el.relative().top(px(3. * (1. - t))).opacity(t),
                )
                .into_any_element()
        };

        let chevron = svg()
            .path(m.chevron_path)
            .size(m.chevron)
            .flex_none()
            .text_color(motion::mix(p.chevron, p.chevron_hover, hover))
            .with_transformation(Transformation::rotate(radians(PI * turn)));

        let focused = keyboard_focus() && focus.is_focused(window);
        // Enter and Space on the focused line click it (GPUI does that)
        let click = toggle.clone();
        let [pt, pr, pb, pl] = m.head_pad;
        let head = div()
            .id("head")
            .track_focus(&focus)
            .relative()
            .flex()
            .items_center()
            .gap(px(10.))
            .w_full()
            .min_h(px(44.))
            .pt(pt)
            .pr(pr)
            .pb(pb)
            .pl(pl)
            .map(|d| {
                if squared {
                    d.rounded_t(m.radius)
                } else {
                    d.rounded(m.radius)
                }
            })
            .cursor_pointer()
            .bg(motion::mix(p.head_hover.opacity(0.), p.head_hover, hover))
            .on_hover(motion::hover_listener(head_key))
            .on_mouse_down(MouseButton::Left, |_, _, _| set_keyboard_focus(false))
            .on_click(move |_, window, cx| click(window, cx))
            .when(focused, |d| {
                d.child(focus_ring(m.focus_offset, m.radius, p.focus))
            })
            .child(tool_icon)
            .child(main)
            .child(status_view)
            .child(chevron);

        // Body
        let max_code = f32::from(self.max_code_height);
        let mut sections: Vec<AnyElement> = Vec::new();
        if failed {
            sections.push(
                error_panel(
                    &key,
                    look,
                    &p,
                    self.error.clone().unwrap_or("The tool call failed.".into()),
                    self.on_retry.clone(),
                    state.clone(),
                    retry_pressed,
                    window,
                )
                .into_any_element(),
            );
        }
        if let Some(args) = &self.args {
            sections.push(code_section(
                &key,
                0,
                "Arguments",
                args,
                max_code,
                look,
                &p,
                &m,
                &state,
                cx,
            ));
        }
        match &self.result {
            Some(result) => sections.push(code_section(
                &key, 1, "Result", result, max_code, look, &p, &m, &state, cx,
            )),
            None if running => sections.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .min_h(m.section_head)
                            .child(section_label(look, "Result", &p, &m)),
                    )
                    .child(
                        div()
                            .text_size(m.small)
                            .line_height(m.small_line)
                            .text_color(p.muted)
                            .child("Waiting for result…"),
                    )
                    .into_any_element(),
            ),
            None => {}
        }
        if self.args.is_none() && self.result.is_none() && status == ToolCallStatus::Success {
            sections.push(
                div()
                    .text_size(m.small)
                    .line_height(m.small_line)
                    .text_color(p.muted)
                    .child("No arguments or result.")
                    .into_any_element(),
            );
        }
        let [st, sr, sb, sl] = m.sections_pad;
        let sections = div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .w_full()
            .gap(m.sections_gap)
            .pt(st)
            .pr(sr)
            .pb(sb)
            .pl(sl)
            // Browsers draw a 0.5px border as one device pixel at 1x
            .border_t(px(f32::from(m.hairline).max(1. / window.scale_factor())))
            .border_color(p.line)
            .children(sections)
            .child(measure(state.clone(), |s: &mut CardState, h| {
                store(&mut s.body, h)
            }));

        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .rounded(m.radius)
            .bg(p.card)
            .shadow(p.card_shadow.clone())
            .font_family(SANS)
            .text_size(m.font_size)
            .line_height(m.line_height)
            .font_weight(FontWeight::NORMAL)
            .text_color(if clean { p.ink } else { p.text })
            .when_some(p.card_edge, |d, edge| {
                d.child(inset_edge(px(1.), edge, m.radius))
            })
            .child(head)
            .child(reveal(&body_key, open, body_h, sections))
    }
}

/// "Arguments" / "ARGUMENTS".
fn section_label(look: Look, label: &str, p: &Palette, m: &Metrics) -> Div {
    div()
        .text_size(m.label_size)
        .line_height(m.label_line)
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.muted)
        .child(match look {
            Look::Clean => label.to_uppercase(),
            Look::Classic => label.to_string(),
        })
}

/// A labelled code block with a copy button, capped with a fade and
/// "Show all" when it runs long.
#[allow(clippy::too_many_arguments)]
fn code_section(
    key: &str,
    ix: usize,
    label: &str,
    payload: &ToolPayload,
    cap: f32,
    look: Look,
    p: &Palette,
    m: &Metrics,
    state: &Entity<CardState>,
    cx: &App,
) -> AnyElement {
    let printed = payload.highlighted();
    let (natural, full, copied) = {
        let code = &state.read(cx).code[ix];
        (code.natural, code.full, code.copied)
    };
    let capped = natural > cap + 24.;
    let target = if capped && !full { cap } else { natural };
    let height = motion::follow(&format!("{key}-code-{ix}"), target, CAP);
    let fade = motion::follow(
        &format!("{key}-code-{ix}-fade"),
        if capped && !full { 1. } else { 0. },
        CAP_FADE,
    );

    let colors = [p.key, p.string, p.number, p.literal];
    let highlights = printed.spans.iter().map(|(range, token)| {
        let color = match token {
            Token::Key => colors[0],
            Token::Str => colors[1],
            Token::Num => colors[2],
            Token::Lit => colors[3],
        };
        (
            range.clone(),
            HighlightStyle {
                color: Some(color),
                ..Default::default()
            },
        )
    });
    let text =
        StyledText::new(printed.text.clone()).with_highlights(highlights.collect::<Vec<_>>());

    let edge = m.hairline;
    let block = div()
        .relative()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .rounded(px(8.))
        .bg(p.code)
        .when(capped, |d| d.h(px(height)))
        .child(inset_edge(edge, p.line, px(8.)))
        .child(
            div()
                .relative()
                .flex_none()
                .w_full()
                .px(px(12.))
                .py(px(10.))
                .font_family(MONO)
                .text_size(px(12.))
                .line_height(m.code_line)
                .font_weight(FontWeight::NORMAL)
                .text_color(p.muted)
                .child(text)
                .child(measure(state.clone(), move |s: &mut CardState, h| {
                    store(&mut s.code[ix].natural, h)
                })),
        )
        .when(fade > 0., |d| {
            d.child(
                div()
                    .absolute()
                    .left(edge)
                    .right(edge)
                    .bottom(edge)
                    .h(px(56.))
                    .rounded_b(px(8.) - edge)
                    .opacity(fade)
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(p.code.opacity(0.), 0.),
                        linear_color_stop(p.code, 1.),
                    )),
            )
        });

    let copy_key = format!("{key}-copy-{ix}");
    let hover = motion::hover_t(&copy_key);
    let done = if copied { 1. } else { 0. };
    let swap_fade = motion::follow(&format!("{copy_key}-fade"), done, COPY_FADE);
    let swap_scale = motion::follow(&format!("{copy_key}-scale"), done, COPY_SCALE);
    let swap_color = motion::follow(&format!("{copy_key}-color"), done, COPY_COLOR);
    let fg = motion::mix(motion::mix(p.copy_fg, p.ink, hover), p.copied, swap_color);
    let inset = (m.copy_box - m.copy_glyph) / 2.;
    let copy_state = state.clone();
    let copy_text = printed.text;
    let copy = div()
        .id(ElementId::Name(copy_key.clone().into()))
        .relative()
        .flex_none()
        .size(m.copy_box)
        .rounded(px(6.))
        .cursor_pointer()
        .map(|d| match &p.copy_plate {
            Some((plate, plate_shadow)) => d
                .bg(motion::mix(*plate, over(p.copy_hover, *plate), hover))
                .shadow(plate_shadow.clone()),
            None => d.bg(motion::mix(p.copy_hover.opacity(0.), p.copy_hover, hover)),
        })
        .on_hover(motion::hover_listener(copy_key))
        .on_mouse_down(MouseButton::Left, |_, _, _| set_keyboard_focus(false))
        .on_click(move |_, _, cx| {
            cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
            copy_state.update(cx, |s, cx| {
                s.code[ix].copied = true;
                s.code[ix].reset = Some(cx.spawn(
                    async move |this: WeakEntity<CardState>, cx: &mut AsyncApp| {
                        cx.background_executor().timer(COPIED_FOR).await;
                        this.update(cx, |s, cx| {
                            s.code[ix].copied = false;
                            cx.notify();
                        })
                        .ok();
                    },
                ));
                cx.notify();
            });
        })
        .child(
            svg()
                .absolute()
                .top(inset)
                .left(inset)
                .path(icon_path(look, "copy"))
                .size(m.copy_glyph)
                .text_color(fg.opacity(1. - swap_fade))
                .with_transformation(scaled(motion::lerp(1., 0.6, swap_scale))),
        )
        .child(
            svg()
                .absolute()
                .top(inset)
                .left(inset)
                .path("icons/tool-call-card/copied.svg")
                .size(m.copy_glyph)
                .text_color(fg.opacity(swap_fade))
                .with_transformation(scaled(motion::lerp(0.6, 1., swap_scale))),
        );

    let more = capped.then(|| {
        let more_key = format!("{key}-more-{ix}");
        let more_state = state.clone();
        align_self(
            div()
                .id(ElementId::Name(more_key.clone().into()))
                .px(px(4.))
                .py(px(2.))
                .ml(px(-4.))
                .rounded(px(if look == Look::Clean { 6. } else { 5. }))
                .cursor_pointer()
                .text_size(m.small)
                .line_height(m.small_line)
                .font_weight(FontWeight::MEDIUM)
                .text_color(motion::hover_blend(&more_key, p.text, p.ink))
                .on_hover(motion::hover_listener(more_key))
                .on_click(move |_, _, cx| {
                    more_state.update(cx, |s, cx| {
                        s.code[ix].full = !s.code[ix].full;
                        cx.notify();
                    })
                })
                .child(if full { "Show less" } else { "Show all" }),
            AlignItems::FlexStart,
        )
    });

    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .min_w_0()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .min_h(m.section_head)
                .child(section_label(look, label, p, m))
                .child(copy),
        )
        .child(block)
        .children(more)
        .into_any_element()
}

/// The message on a faint red panel, with an optional Retry button.
#[allow(clippy::too_many_arguments)]
fn error_panel(
    key: &str,
    look: Look,
    p: &Palette,
    message: SharedString,
    on_retry: Option<Handler>,
    state: Entity<CardState>,
    pressed: Option<Instant>,
    window: &mut Window,
) -> Div {
    let clean = look == Look::Clean;
    let icon = if clean {
        align_self(
            error_icon(look, p, px(16.)).mt(px(6.)),
            AlignItems::FlexStart,
        )
    } else {
        error_icon(look, p, px(14.)).mt(px(2.))
    };
    let retry = on_retry.map(|on_retry| {
        let retry_key = format!("{key}-retry");
        let hover = motion::hover_t(&retry_key);
        // `:active svg { rotate(-60deg) }`, easing in while pressed
        let turn = match pressed {
            Some(at) => {
                let t = at.elapsed().as_secs_f32() / 0.2 / motion::speed_scale();
                if t < 1. {
                    window.request_animation_frame();
                }
                EASE_OUT_QUINT.eval(t)
            }
            None => 0.,
        };
        let down = state.clone();
        let up = state.clone();
        let up_out = state.clone();
        let release = |s: &mut CardState, cx: &mut Context<CardState>| {
            s.retry_pressed = None;
            cx.notify();
        };
        div()
            .id(ElementId::Name(retry_key.clone().into()))
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(if clean { 6. } else { 5. }))
            .h(px(if clean { 28. } else { 26. }))
            .when(!clean, |d| d.my(px(-3.)))
            .pl(px(if clean { 6. } else { 8. }))
            .pr(px(10.))
            .rounded(px(if clean { 6. } else { 7. }))
            .bg(motion::mix(
                p.card,
                over(p.head_hover, over(p.red_bg, p.card)),
                hover,
            ))
            .shadow(p.retry_shadow.clone())
            .text_color(p.strong)
            .text_size(px(if clean { 13. } else { 12.5 }))
            .line_height(if clean { px(20.).into() } else { relative(1.4) })
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .on_hover(motion::hover_listener(retry_key))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                set_keyboard_focus(false);
                down.update(cx, |s, cx| {
                    s.retry_pressed = Some(Instant::now());
                    cx.notify();
                });
            })
            .on_mouse_up(MouseButton::Left, move |_, _, cx| up.update(cx, release))
            .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                up_out.update(cx, release)
            })
            .on_click(move |_, window, cx| on_retry(window, cx))
            .when_some(p.retry_edge, |d, edge| {
                d.child(inset_edge(px(1.), edge, px(7.)))
            })
            .child(
                svg()
                    .path("icons/tool-call-card/retry.svg")
                    .size(px(if clean { 16. } else { 13. }))
                    .flex_none()
                    .text_color(p.retry_icon)
                    .with_transformation(Transformation::rotate(radians(-PI / 3. * turn))),
            )
            .child("Retry")
    });

    let [pt, pr, pb, pl] = if clean {
        [px(8.), px(8.), px(8.), px(12.)]
    } else {
        [px(9.), px(10.), px(9.), px(11.)]
    };
    div()
        .relative()
        .flex()
        .map(|d| {
            if clean {
                d.items_center()
            } else {
                d.items_start()
            }
        })
        .gap(px(8.))
        .when(clean, |d| d.min_h(px(44.)))
        .pt(pt)
        .pr(pr)
        .pb(pb)
        .pl(pl)
        .rounded(px(8.))
        .bg(p.red_bg)
        .child(inset_edge(
            if clean { px(0.5) } else { px(1.) },
            p.red_line,
            px(8.),
        ))
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.))
                .line_height(if clean {
                    px(20.).into()
                } else {
                    relative(1.45)
                })
                .text_color(p.red)
                .child(message),
        )
        .children(retry)
}

// ---------------------------------------------------------------------------
// Group
// ---------------------------------------------------------------------------

struct GroupState {
    open: bool,
    watch: Stopwatch,
    list: f32,
    head_focus: FocusHandle,
    card_focus: Vec<FocusHandle>,
}

/// Several cards folded into one line: their icons, "Used 3 tools", how many
/// failed and the time spent.
#[derive(IntoElement)]
pub struct ToolCallGroup {
    id: ElementId,
    cards: Vec<ToolCallCard>,
    label: Option<SharedString>,
    running_label: SharedString,
    duration: Option<Duration>,
    default_open: bool,
    open: Option<bool>,
    on_open_change: Option<OpenHandler>,
    look: Option<Look>,
}

impl ToolCallGroup {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            cards: Vec::new(),
            label: None,
            running_label: "Using tools…".into(),
            duration: None,
            default_open: false,
            open: None,
            on_open_change: None,
            look: None,
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

    /// Label once all calls are done – defaults to "Used 3 tools".
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Label while any call is running (default "Using tools…").
    pub fn running_label(mut self, label: impl Into<SharedString>) -> Self {
        self.running_label = label.into();
        self
    }

    /// Total time – otherwise the sum of the cards' durations, or the time
    /// measured while they ran.
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
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

    /// Overrides the global Clean/Classic look for the group and its cards.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for ToolCallGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx);
        let m = Metrics::get(look);
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);

        // Read the cards' props to sum them up in the header
        let calls: Vec<(Option<SharedString>, ToolCallStatus, Option<Duration>)> = self
            .cards
            .iter()
            .map(|c| (c.icon.clone(), c.status, c.duration))
            .collect();
        let running = calls.iter().any(|c| c.1 == ToolCallStatus::Running);
        let failed = calls
            .iter()
            .filter(|c| c.1 == ToolCallStatus::Error)
            .count();

        let default_open = self.default_open;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |_, cx| GroupState {
                open: default_open,
                watch: Stopwatch::default(),
                list: 0.,
                head_focus: cx.focus_handle(),
                card_focus: Vec::new(),
            },
        );
        let count = calls.len();
        state.update(cx, |s, cx| {
            // Time spent with tools running, added up across stretches
            s.watch.sync(running, None, true);
            while s.card_focus.len() < count {
                s.card_focus.push(cx.focus_handle());
            }
        });
        let (stored_open, measured, list_h, head_focus, card_focus) = {
            let s = state.read(cx);
            (
                s.open,
                s.watch.ms(),
                s.list,
                s.head_focus.clone(),
                s.card_focus[..count].to_vec(),
            )
        };
        let controlled = self.open;
        let open = controlled.unwrap_or(stored_open);

        let sum = (count > 0 && calls.iter().all(|c| c.2.is_some()))
            .then(|| calls.iter().filter_map(|c| c.2).sum::<Duration>());
        let ms = if running {
            measured
        } else {
            self.duration
                .or(sum)
                .map(|d| d.as_secs_f64() * 1e3)
                .or(measured)
        };
        let used = format!("Used {count} {}", if count == 1 { "tool" } else { "tools" });
        let title: SharedString = if running {
            self.running_label.clone()
        } else {
            self.label.clone().unwrap_or(used.into())
        };
        if running {
            window.request_animation_frame();
        }

        let toggle: Handler = {
            let state = state.clone();
            let on_change = self.on_open_change.clone();
            Rc::new(move |window, cx| {
                if controlled.is_none() {
                    state.update(cx, |s, cx| {
                        s.open = !open;
                        cx.notify();
                    });
                }
                if let Some(f) = &on_change {
                    f(!open, window, cx);
                }
            })
        };

        // The last three tools, on small tiles
        let shown = calls.len().saturating_sub(3);
        let (tile, tile_radius, overlap, glyph) = if clean {
            (22., 6., 4., 12.)
        } else {
            (24., 7., 5., 13.)
        };
        let tiles = (calls.len() - shown) as f32;
        let stack = div()
            .flex()
            .flex_none()
            .when(tiles > 0., |d| {
                d.w(px(tiles * tile - (tiles - 1.) * overlap))
            })
            .when(clean, |d| {
                d.p(px(0.5))
                    .w(px(tiles * tile - (tiles - 1.) * overlap + 1.))
            })
            .children(calls[shown..].iter().enumerate().map(|(i, call)| {
                let ix = shown + i;
                let path = call.0.clone().unwrap_or(WRENCH.into());
                let fill = p.tile.map_or(p.card, |t| t.0);
                let edge = p.tile.map(|t| t.1);
                let tile_shadow = if clean {
                    p.card_shadow.clone()
                } else {
                    vec![shadow(0., 0., 0., 2., p.ring)]
                };
                let glyph_color = p.glyph;
                let draw = move |t: f32| {
                    // `ui-tcc-pop`: scale .6 → 1 and fade in, from the center
                    let s = motion::lerp(0.6, 1., t);
                    let side = tile * s;
                    div()
                        .relative()
                        .flex_none()
                        .size(px(side))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(tile_radius * s))
                        .bg(fill)
                        .shadow(tile_shadow.clone())
                        .when_some(edge, |d, edge| {
                            d.child(inset_edge(px(1.), edge, px(tile_radius * s)))
                        })
                        .opacity(t)
                        .child(
                            svg()
                                .path(path.clone())
                                .size(px(glyph))
                                .flex_none()
                                .text_color(glyph_color)
                                .with_transformation(scaled(s)),
                        )
                };
                let slot = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(tile))
                    .when(i > 0, |d| d.ml(px(-overlap)));
                if reduced {
                    slot.child(draw(1.)).into_any_element()
                } else {
                    slot.with_animation(
                        ElementId::Name(format!("{key}-tile-{ix}").into()),
                        STACK_POP.animation(),
                        move |el, t| el.child(draw(t)),
                    )
                    .into_any_element()
                }
            }));

        let label_color = if running { p.running_label } else { p.group_fg };
        let label = div()
            .relative()
            .min_w_0()
            .map(one_line)
            .font_weight(FontWeight::MEDIUM);
        let label = if running && !reduced {
            let (shine, text) = (p.shine, title.clone());
            label
                .text_color(label_color)
                .with_animation(
                    ElementId::Name(format!("{key}-shine").into()),
                    Animation::new(SWEEP.mul_f32(motion::speed_scale())).repeat(),
                    move |el, t| el.child(shine_text(&text, t, label_color, shine)),
                )
                .into_any_element()
        } else if running {
            label
                .text_color(label_color)
                .child(title.clone())
                .into_any_element()
        } else {
            // Done: the label takes the head's (hover-eased) color
            label.child(title.clone()).into_any_element()
        };
        let title_view = div()
            .flex()
            .items_center()
            .gap(px(if clean { 6. } else { 5. }))
            .min_w_0()
            .whitespace_nowrap()
            .child(label)
            .when(!running && failed > 0, |d| {
                d.child(
                    div()
                        .flex_none()
                        .text_color(p.red)
                        .child(format!("· {failed} failed")),
                )
            })
            .when_some(ms, |d, ms| {
                d.child(
                    div()
                        .flex_none()
                        .text_color(p.faint)
                        .child(format!("· {}", format_ms(ms, running))),
                )
            });
        let title_view = if reduced {
            title_view.into_any_element()
        } else {
            title_view
                .with_animation(
                    ElementId::Name(
                        format!("{key}-title-{}", if running { "busy" } else { "done" }).into(),
                    ),
                    SETTLE.animation(),
                    |el, t| el.relative().top(px(3. * (1. - t))).opacity(t),
                )
                .into_any_element()
        };

        let head_key = format!("{key}-head");
        let hover = motion::hover_t(&head_key);
        let turn = motion::follow(&format!("{key}-turn"), if open { 1. } else { 0. }, TURN);
        let focused = keyboard_focus() && head_focus.is_focused(window);
        // Enter and Space on the focused line click it (GPUI does that)
        let click = toggle.clone();
        let radius = px(if clean { 8. } else { 10. });
        let head = align_self(
            div()
                .id("group-head")
                .track_focus(&head_focus)
                .relative()
                .flex()
                .items_center()
                .gap(px(if clean { 8. } else { 9. }))
                .max_w_full()
                .min_h(px(if clean { 32. } else { 36. }))
                .when(clean, |d| d.ml(px(-6.)))
                .py(px(4.))
                .pr(px(10.))
                .pl(px(if clean { 6. } else { 5. }))
                .rounded(radius)
                .cursor_pointer()
                .bg(motion::mix(p.group_hover.opacity(0.), p.group_hover, hover))
                .text_color(motion::mix(p.group_fg, p.ink, hover))
                .on_hover(motion::hover_listener(head_key))
                .on_mouse_down(MouseButton::Left, |_, _, _| set_keyboard_focus(false))
                .on_click(move |_, window, cx| click(window, cx))
                .when(focused, |d| {
                    d.child(focus_ring(m.focus_offset, radius, p.focus))
                })
                .child(stack)
                .child(title_view)
                .child(
                    svg()
                        .path(m.chevron_path)
                        .size(m.chevron)
                        .flex_none()
                        .text_color(motion::mix(p.chevron, p.chevron_hover, hover))
                        .with_transformation(Transformation::rotate(radians(PI * turn))),
                ),
            AlignItems::FlexStart,
        );

        // Arrow keys move between the group's line and its cards' lines
        let mut lines = vec![head_focus.clone()];
        if open {
            lines.extend(card_focus.iter().cloned());
        }
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let len = lines.len();
            let current = lines.iter().position(|f| f.is_focused(window));
            let next = match event.keystroke.key.as_str() {
                "down" => current.map_or(0, |i| (i + 1) % len),
                "up" => current.map_or(len - 1, |i| (i + len - 1) % len),
                "home" => 0,
                "end" => len - 1,
                _ => return,
            };
            set_keyboard_focus(true);
            window.focus(&lines[next], cx);
            window.refresh();
            cx.stop_propagation();
        };

        let scope = key.clone();
        let list = div()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .w_full()
            .gap(px(8.))
            .pt(px(if clean { 6. } else { 8. }))
            .when(clean, |d| d.pb(px(2.)))
            .children(
                self.cards
                    .into_iter()
                    .zip(card_focus)
                    .map(|(mut card, focus)| {
                        card.look = card.look.or(Some(look));
                        card.scope = Some(scope.clone());
                        card.focus = Some(focus);
                        card
                    }),
            )
            .child(measure(state.clone(), |s: &mut GroupState, h| {
                store(&mut s.list, h)
            }));

        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .flex()
            .flex_col()
            .w_full()
            .min_w_0()
            .font_family(SANS)
            .text_size(m.font_size)
            .line_height(m.line_height)
            .font_weight(FontWeight::NORMAL)
            .on_key_down(on_key)
            .child(head)
            .child(reveal(&format!("{key}-list"), open, list_h, list))
    }
}

// ---------------------------------------------------------------------------
// Story: the demo
// ---------------------------------------------------------------------------

/// When each step of the demo lands (ms), and when it starts over.
const STEPS: [u64; 7] = [0, 1500, 2700, 4300, 5400, 6700, 8000];
const LOOP: u64 = 11500;

struct Demo {
    step: usize,
    open: bool,
    /// Someone toggled the group: stop closing and replaying it.
    touched: bool,
    run: u64,
    _timeline: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        Demo {
            step: 0,
            open: true,
            touched: false,
            run: 0,
            _timeline: {
                // The cards start their clocks on this frame, so does the script
                let start = Instant::now();
                cx.spawn(async move |this, cx| Demo::play(this, cx, start).await)
            },
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
                    d.step = d.step.max(step);
                    if step == STEPS.len() - 1 && !d.touched {
                        d.open = false;
                    }
                    cx.notify();
                });
                if landed.is_err() {
                    return;
                }
            }
            wait(LOOP).await;
            let again = this.update(cx, |d, cx| {
                if d.touched {
                    return false;
                }
                d.run += 1;
                d.step = 0;
                d.open = true;
                cx.notify();
                true
            });
            if !matches!(again, Ok(true)) {
                return;
            }
            start = Instant::now();
        }
    }
}

fn search_results() -> Json {
    let hit = |title: &str, url: &str, price: i64| {
        Json::object([
            ("title", title.into()),
            ("url", url.into()),
            ("price_per_seat", price.into()),
        ])
    };
    Json::object([
        (
            "results",
            Json::array([
                hit(
                    "HubSpot Sales Hub pricing",
                    "https://www.hubspot.com/pricing/sales",
                    90,
                ),
                hit(
                    "Pipedrive plans and pricing",
                    "https://www.pipedrive.com/en/pricing",
                    49,
                ),
                hit("Attio pricing", "https://attio.com/pricing", 59),
                hit(
                    "Salesforce Starter Suite",
                    "https://www.salesforce.com/sales/pricing",
                    25,
                ),
                hit("Close CRM pricing", "https://www.close.com/pricing", 49),
            ]),
        ),
        ("total", 5.into()),
        ("cached", false.into()),
    ])
}

fn demo_view(demo: &Entity<Demo>, cx: &App) -> Div {
    let dark = cx.is_dark();
    let (step, open, run) = {
        let d = demo.read(cx);
        (d.step, d.open, d.run)
    };
    let fetch = if step == 2 {
        ToolCallStatus::Error
    } else if step >= 4 {
        ToolCallStatus::Success
    } else {
        ToolCallStatus::Running
    };

    let mut group = ToolCallGroup::new(ElementId::NamedInteger("tool-calls".into(), run))
        .open(open)
        .on_open_change({
            let demo = demo.clone();
            move |open, _, cx| {
                demo.update(cx, |d, cx| {
                    d.touched = true;
                    d.open = open;
                    cx.notify();
                })
            }
        })
        .child(
            ToolCallCard::new(
                "search_web",
                "search_web",
                if step >= 1 {
                    ToolCallStatus::Success
                } else {
                    ToolCallStatus::Running
                },
            )
            .icon("icons/tool-call-card/search.svg")
            .summary(if step >= 1 {
                "Searched the web for 'CRM pricing 2026'"
            } else {
                "Searching the web for 'CRM pricing 2026'"
            })
            .args(Json::object([
                ("query", "CRM pricing 2026 small sales team".into()),
                ("max_results", 5.into()),
            ]))
            .when(step >= 1, |c| c.result(search_results())),
        );
    if step >= 1 {
        let retry = demo.clone();
        group = group.child(
            ToolCallCard::new("fetch_page", "fetch_page", fetch)
                .icon("icons/tool-call-card/page.svg")
                .summary(if fetch == ToolCallStatus::Success {
                    "Read hubspot.com/pricing"
                } else {
                    "Reading hubspot.com/pricing"
                })
                .args(Json::object([
                    ("url", "https://www.hubspot.com/pricing/sales".into()),
                    ("timeout_ms", 8000.into()),
                ]))
                .when(fetch == ToolCallStatus::Success, |c| {
                    c.result(Json::object([
                        ("status", 200.into()),
                        ("title", "Sales Hub Pricing".into()),
                        ("words", 2140.into()),
                    ]))
                })
                .error("Request timed out after 8s – hubspot.com didn't respond.")
                .on_retry(move |_, cx| {
                    retry.update(cx, |d, cx| {
                        d.step = d.step.max(3);
                        cx.notify();
                    })
                }),
        );
    }
    if step >= 4 {
        group = group.child(
            ToolCallCard::new(
                "crm.list_deals",
                "crm.list_deals",
                if step >= 5 {
                    ToolCallStatus::Success
                } else {
                    ToolCallStatus::Running
                },
            )
            .icon("icons/tool-call-card/database.svg")
            .summary(if step >= 5 {
                "Found 4 open deals in negotiation"
            } else {
                "Looking up open deals"
            })
            .args(Json::object([
                ("stage", "negotiation".into()),
                ("owner", "me".into()),
                ("limit", 10.into()),
            ]))
            .when(step >= 5, |c| {
                c.result(Json::object([
                    ("count", 4.into()),
                    (
                        "deals",
                        Json::array([
                            Json::object([("name", "Northwind".into()), ("value", 18400.into())]),
                            Json::object([("name", "Globex".into()), ("value", 9600.into())]),
                        ]),
                    ),
                ]))
            }),
        );
    }

    // `.tc-demo`: a 460px chat column, the request bubble above the tools
    let bubble = div()
        // 88% in the CSS; GPUI rounds the measured line up, so give the
        // first line the same room it has in the browser
        .max_w(relative(0.885))
        .px(px(14.))
        .py(px(10.))
        .rounded_tl(px(18.))
        .rounded_tr(px(18.))
        .rounded_br(px(6.))
        .rounded_bl(px(18.))
        .bg(if dark { hex(0x262628) } else { hex(0xf0f0f0) })
        .text_color(if dark { hex(0xededed) } else { hex(0x171717) })
        .text_size(px(14.))
        .line_height(relative(1.45))
        .child("Compare CRM pricing for our 10-person team and check open deals.");

    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .pt(px(56.))
        .font_family(SANS)
        .child(
            div()
                .w(px(460.))
                .max_w_full()
                .flex()
                .flex_col()
                .gap(px(18.))
                .child(div().flex().justify_end().child(bubble))
                .child(group),
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
    fn highlights_keys_strings_numbers_literals() {
        let printed =
            Json::object([("a", "b".into()), ("n", 1.into()), ("t", false.into())]).highlighted();
        let tokens: Vec<_> = printed
            .spans
            .iter()
            .map(|(r, t)| (&printed.text[r.clone()], *t))
            .collect();
        assert_eq!(
            tokens,
            [
                ("\"a\"", Token::Key),
                ("\"b\"", Token::Str),
                ("\"n\"", Token::Key),
                ("1", Token::Num),
                ("\"t\"", Token::Key),
                ("false", Token::Lit),
            ]
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
        // Text payloads that aren't JSON stay text
        let text = ToolPayload::from("{not json").highlighted();
        assert_eq!(text.text, "{not json");
        assert!(text.spans.is_empty());
        assert_eq!(ToolPayload::from("[1]").highlighted().text, "[\n  1\n]");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_ms(0.4, false), "1ms");
        assert_eq!(format_ms(240., false), "240ms");
        assert_eq!(format_ms(240., true), "0.2s");
        assert_eq!(format_ms(2_749., false), "2.7s");
        assert_eq!(format_ms(12_300., true), "12.3s");
        assert_eq!(format_ms(125_000., false), "2m 05s");
    }

    #[test]
    fn check_draws_left_to_right() {
        let points = [(5.1, 8.2), (7.1, 10.1), (10.9, 6.)];
        assert!((drawn_x(points, 0.) - 5.1).abs() < 1e-4);
        assert!((drawn_x(points, 1.) - 10.9).abs() < 1e-4);
        assert!(drawn_x(points, 0.5) > drawn_x(points, 0.3));
        assert!(bezier(EASE_OUT_BACK, 0.6) > 1.);
    }
}
