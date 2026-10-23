//! AI Answer – a pink component.
//!
//! One assistant turn the way pink's transcript draws it (pink:
//! `transcript.rs` `render_row`, `render_tool_group`, `render_working_trailer`
//! and `markdown/render.rs`). There is no card. The question sits in the
//! right-aligned user bubble. The web search and the pages the answer read
//! form a tool group: a quiet 24px plate summarising "Searched 1 time ·
//! fetched 4 pages" that unfolds into rows on the activity rail, open while
//! the search runs and folding shut once the reply starts. The reply is bare
//! markdown at 14/22 whose newly streamed words dissolve in under pink's
//! opacity veil. While the turn runs, the working line under it names what it
//! is doing ("Searching the web for …", "Fetching …", "Writing") beside a
//! timer and a Stop that shows on hover. Once it settles, a reserved lane
//! under each entry shows its timestamp, Retry and Copy while the pointer is
//! over it.
//!
//! Citations are the one part pink has no twin for. They are superscript
//! numbers in pink's list-marker tone that preview their source in pink's
//! quoted-passage hover card and open it on click.
//!
//! The component renders whatever it is given, so a caller streams by passing
//! the blocks received so far (or the whole answer plus `.reveal(words)`):
//!
//! ```ignore
//! AiAnswer::new("answer", "Is it safe to keep refresh tokens in localStorage?")
//!     .query("refresh token localStorage XSS")
//!     .sources(sources)
//!     .blocks(vec![Block::paragraph(vec![
//!         Span::strong("Not if you can avoid it."), Span::text(" Any script can read it"),
//!         Span::cite(0), Span::text("."),
//!     ])])
//!     .status(AnswerStatus::Writing)
//!     .elapsed(turn_started.elapsed())
//!     .on_stop(|_, cx| stop_turn(cx))
//! ```

use std::{
    collections::{HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    ops::Range,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, BoxShadow, ClickEvent, ClipboardItem, Context, Div, ElementId,
    Entity, FillOptions, FillRule, Font, FontFeatures, FontWeight, Hsla, InteractiveText,
    IntoElement, MouseMoveEvent, PathBuilder, PathStyle, Pixels, Point, Render, Role, SharedString,
    Stateful, StyledText, Task, TextRun, Transformation, Window, canvas, div, point, prelude::*,
    px, radians, svg,
};

use crate::{
    HoverFill,
    motion::{self, DISCLOSURE, TOOL_CONNECTOR_REVEAL, TOOL_ROW_REVEAL},
    pink::{
        self, FillTier, Pink,
        loaders::{
            LIVE_STATUS_OBJECT_CHARS, TOOL_LABEL_LINE_HEIGHT, TOOL_LABEL_SIZE,
            WORKING_STATUS_LINE_HEIGHT, WORKING_TIMER_SIZE, ellipsize_chars, fnv1a,
            mini_glyph_spinner, orb_slot, shimmer_text, working_status, working_timer,
        },
        radius, space,
    },
    story,
};

pub const NAME: &str = "AI Answer";
pub const DESCRIPTION: &str = "One assistant turn as pink's transcript draws it – the question in a bubble, the searches and pages behind the reply folded into a tool group, an answer that fades in as it streams with citations that preview their source, and Retry and Copy on hover once it settles";

const ICON_COPY: &str = "icons/ai-answer/copy.svg";
const ICON_COPY_CHECK: &str = "icons/ai-answer/copy-check.svg";
const ICON_RESTART: &str = "icons/ai-answer/restart.svg";
const ICON_GLOBAL: &str = "icons/ai-answer/global.svg";
const ICON_CHEVRON: &str = "icons/ai-answer/alt-arrow-right.svg";
const ICON_OPEN: &str = "icons/ai-answer/arrow-right-up.svg";

/// pink: `markdown/render.rs` `MD_TEXT_SIZE`, `MD_LINE_HEIGHT`, `MD_BLOCK_GAP`.
const MD_TEXT_SIZE: f32 = 14.;
const MD_LINE_HEIGHT: f32 = 22.;
const MD_BLOCK_GAP: f32 = 12.;
/// pink: `render_block` `Block::List` – an 18px marker column holding a 5px
/// accent disc, 8px to the text, 4px between items.
const LIST_MARKER_MIN_W: f32 = 18.;
const LIST_DISC: f32 = 5.;
const LIST_MARKER_GAP: f32 = 8.;
const LIST_ITEM_GAP: f32 = 4.;
/// pink: `transcript.rs` `USER_LINE_HEIGHT`; the bubble caps at 80% of the
/// column.
const USER_LINE_HEIGHT: f32 = 22.;
const USER_BUBBLE_SHARE: f32 = 0.8;
/// pink: `META_LANE_HEIGHT` – the reserved lane holds 24px actions under a
/// 4px lead.
const META_LANE_HEIGHT: f32 = 28.;
const META_ACTION_SIZE: f32 = 24.;
/// pink: `copy_message` keeps the "Copied" check this long.
const COPIED_FOR: Duration = Duration::from_millis(1200);
/// pink: the tool group header, its rows and the activity rail.
const TOOL_GROUP_HEADER_HEIGHT: f32 = 26.;
const TOOL_TREE_ROW_HEIGHT: f32 = 32.;
const CHIP_CARD_HEIGHT: f32 = 30.;
const CHIPS_TOP_PAD: f32 = 2.;
const ACTIVITY_GUTTER_WIDTH: f32 = 48.;
const ACTIVITY_TEXT_GAP: f32 = 8.;
const ACTIVITY_TRUNK_X: f32 = 12.5;
const ACTIVITY_BEND_RADIUS: f32 = 6.;
const ACTIVITY_BRANCH_END_X: f32 = 28.;
const ACTIVITY_ICON_LEFT: f32 = 32.;
const ACTIVITY_ICON_SIZE: f32 = 14.;
const DISCLOSURE_CHEVRON_SIZE: f32 = 12.;
/// pink: `chip_header_row`'s trailing tile.
const TRAIL_TILE: f32 = 18.;
/// pink: `badges.rs` `BadgeCard`.
const CARD_WIDTH: f32 = 320.;
/// pink: `markdown/veil.rs`.
const VEIL_EMA_SEED_MS: f32 = 160.;
const VEIL_MIN_FADE_MS: f32 = 120.;
const VEIL_MAX_FADE_MS: f32 = 400.;
const VEIL_CURVE_POW: f32 = 1.6;
const VEIL_GAP_CLAMP_MS: f32 = 1000.;
/// A thin space between adjacent citation numbers, so ¹ ² never reads as ¹².
const CITE_GAP: &str = "\u{2009}";

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type SourceHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A page the answer read: a row of the tool group, and what a citation of it
/// previews.
#[derive(Clone)]
pub struct Source {
    pub title: SharedString,
    /// The host it was read from ("developer.mozilla.org").
    pub site: SharedString,
    /// The address fetched; the site when unset.
    pub url: Option<SharedString>,
    /// A passage shown in the citation preview.
    pub snippet: SharedString,
}

impl Source {
    pub fn new(title: impl Into<SharedString>, site: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            site: site.into(),
            url: None,
            snippet: SharedString::default(),
        }
    }

    pub fn url(mut self, url: impl Into<SharedString>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn snippet(mut self, snippet: impl Into<SharedString>) -> Self {
        self.snippet = snippet.into();
        self
    }

    /// What the fetch row names: the url, else the site.
    pub fn address(&self) -> SharedString {
        self.url.clone().unwrap_or_else(|| self.site.clone())
    }
}

/// A run of answer text.
#[derive(Clone)]
pub enum Span {
    Text(SharedString),
    Strong(SharedString),
    /// A citation of `sources[ix]`.
    Cite(usize),
}

impl Span {
    pub fn text(s: impl Into<SharedString>) -> Self {
        Span::Text(s.into())
    }

    pub fn strong(s: impl Into<SharedString>) -> Self {
        Span::Strong(s.into())
    }

    pub fn cite(source: usize) -> Self {
        Span::Cite(source)
    }
}

/// A paragraph or a bulleted list.
#[derive(Clone)]
pub enum Block {
    Paragraph(Vec<Span>),
    Bullets(Vec<Vec<Span>>),
}

impl Block {
    pub fn paragraph(spans: Vec<Span>) -> Self {
        Block::Paragraph(spans)
    }

    pub fn bullets(items: Vec<Vec<Span>>) -> Self {
        Block::Bullets(items)
    }
}

/// Where the turn is. Searching and Writing show the working line; a settled
/// turn (Done, or Stopped part way) shows the hover lane instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerStatus {
    Searching,
    Writing,
    Done,
    Stopped,
}

impl AnswerStatus {
    fn busy(self) -> bool {
        matches!(self, AnswerStatus::Searching | AnswerStatus::Writing)
    }
}

/// Counts the words and citations an answer streams, for `.reveal`.
pub fn piece_count(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(spans) => pieces(spans).len(),
            Block::Bullets(items) => items.iter().map(|s| pieces(s).len()).sum(),
        })
        .sum()
}

/// The answer as plain text, citations as `[n]`.
pub fn plain_text(blocks: &[Block]) -> String {
    plain_text_upto(blocks, usize::MAX)
}

/// [`plain_text`] of the first `limit` pieces: what Copy takes from a reply
/// that was stopped part way.
fn plain_text_upto(blocks: &[Block], limit: usize) -> String {
    let mut n = 0usize;
    let mut line = |spans: &[Span]| {
        let mut out = String::new();
        for (piece, space) in pieces(spans) {
            n += 1;
            if n > limit {
                continue;
            }
            match piece {
                Piece::Word { text, .. } => out.push_str(&text),
                Piece::Cite(ix) => out.push_str(&format!("[{}]", ix + 1)),
            }
            if space {
                out.push(' ');
            }
        }
        out.trim_end().to_string()
    };
    let mut parts: Vec<String> = Vec::new();
    for block in blocks {
        match block {
            Block::Paragraph(spans) => {
                let text = line(spans);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
            Block::Bullets(items) => {
                let lines: Vec<String> = items
                    .iter()
                    .map(|spans| line(spans))
                    .filter(|text| !text.is_empty())
                    .map(|text| format!("- {text}"))
                    .collect();
                if !lines.is_empty() {
                    parts.push(lines.join("\n"));
                }
            }
        }
    }
    parts.join("\n\n")
}

/// One streamed unit: a word or a citation.
#[derive(Clone)]
enum Piece {
    Word { text: SharedString, strong: bool },
    Cite(usize),
}

/// A piece and whether a space follows it.
fn pieces(spans: &[Span]) -> Vec<(Piece, bool)> {
    let mut out: Vec<(Piece, bool)> = Vec::new();
    for span in spans {
        match span {
            Span::Text(s) | Span::Strong(s) => {
                let strong = matches!(span, Span::Strong(_));
                let parts: Vec<&str> = s.split(' ').collect();
                let count = parts.len();
                for (i, part) in parts.into_iter().enumerate() {
                    if part.is_empty() {
                        if i == 0
                            && let Some(last) = out.last_mut()
                        {
                            last.1 = true;
                        }
                        continue;
                    }
                    out.push((
                        Piece::Word {
                            text: part.to_string().into(),
                            strong,
                        },
                        i + 1 < count,
                    ));
                }
            }
            Span::Cite(ix) => out.push((Piece::Cite(*ix), false)),
        }
    }
    out
}

/// pink: `transcript.rs` `url_host`.
fn url_host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .to_string()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// pink: `pink_proto::view::tool_group_summary` for web tools – "Searched 1
/// time · fetched 4 pages", first segment capitalized.
fn tool_group_summary(searches: usize, fetches: usize) -> String {
    let mut segments: Vec<String> = Vec::new();
    if searches > 0 {
        segments.push(format!("searched {}", plural(searches, "time", "times")));
    }
    if fetches > 0 {
        segments.push(format!("fetched {}", plural(fetches, "page", "pages")));
    }
    let summary = segments.join(" · ");
    let mut chars = summary.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => summary,
    }
}

// ---------------------------------------------------------------------------
// Streaming veil (pink: `markdown/veil.rs`)
// ---------------------------------------------------------------------------

/// A veiled byte range and its current opacity.
type VeilSpan = (Range<usize>, f32);

/// pink: `veil_opacity` – the veil dissolves as `(1 − p)^1.6`.
fn veil_opacity(p: f32) -> f32 {
    1. - (1. - p.clamp(0., 1.)).powf(VEIL_CURVE_POW)
}

struct Chunk {
    range: Range<usize>,
    started: Instant,
    duration_ms: f32,
}

/// pink: `ElemVeil` – remembers the last rendered text of one element and
/// fades every newly appended suffix exactly once, over `clamp(ema × 3, 120,
/// 400)` ms of the stream's cadence, sped up when 3+ chunks overlap.
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
        let elapsed_ms = |c: &Chunk| now.saturating_duration_since(c.started).as_secs_f32() * 1000.;
        let b = boost(self.chunks.len());
        self.chunks.retain(|c| elapsed_ms(c) * b < c.duration_ms);
        let b = boost(self.chunks.len());
        self.chunks
            .iter()
            .map(|c| {
                let progress = (elapsed_ms(c) * b / c.duration_ms).clamp(0., 1.);
                (c.range.clone(), veil_opacity(progress))
            })
            .collect()
    }
}

/// pink: `RowVeil` – the veils of one streaming reply, keyed per text
/// element. While `seeding`, an element seen for the first time adopts its
/// text without fading (a reply already on screen when the turn mounts).
#[derive(Default)]
struct RowVeil {
    elems: HashMap<usize, ElemVeil>,
    seeding: bool,
}

impl RowVeil {
    fn advance(&mut self, elem: usize, text: &str, now: Instant) -> Vec<VeilSpan> {
        if self.seeding && !self.elems.contains_key(&elem) {
            let baseline = ElemVeil {
                prev: text.to_string(),
                ..ElemVeil::default()
            };
            self.elems.insert(elem, baseline);
            return Vec::new();
        }
        self.elems.entry(elem).or_default().advance(text, now)
    }

    fn is_fading(&self) -> bool {
        self.elems.values().any(|e| !e.chunks.is_empty())
    }
}

/// pink: `apply_veil` – multiplies the veil into the runs' colors, splitting
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
            }
            out.push(piece);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Answer text: spans → shaped runs (pink: `flatten_runs`)
// ---------------------------------------------------------------------------

/// A citation inside a text element.
#[derive(Clone)]
struct Cite {
    range: Range<usize>,
    source: usize,
    /// Its position among all the answer's citations.
    ordinal: usize,
}

/// One text element's flat text and runs.
struct TextFlat {
    elem: usize,
    text: String,
    runs: Vec<TextRun>,
    cites: Vec<Cite>,
}

enum BlockFlat {
    Paragraph(TextFlat),
    Bullets(Vec<TextFlat>),
}

fn sans(weight: FontWeight) -> Font {
    Font {
        weight,
        ..gpui::font(pink::SANS)
    }
}

/// Geist's superscript figures (OpenType `sups`), at 500. The weight is
/// load-bearing: the shaper joins runs that differ only in features, so a
/// number straight after a regular word would otherwise lose its `sups`.
fn superscript() -> Font {
    Font {
        features: FontFeatures(Arc::new(vec![("sups".into(), 1)])),
        weight: FontWeight::MEDIUM,
        ..gpui::font(pink::SANS)
    }
}

fn push_run(flat: &mut TextFlat, s: &str, font: Font, color: Hsla) {
    if s.is_empty() {
        return;
    }
    flat.text.push_str(s);
    if let Some(last) = flat.runs.last_mut()
        && last.font == font
        && last.color == color
    {
        last.len += s.len();
        return;
    }
    flat.runs.push(TextRun {
        len: s.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    });
}

/// Flattens the revealed part of one span list. Words are Geist regular, or
/// semibold when strong (pink: strong runs never drop below semibold), in
/// `text`; citations are superscript numbers in pink's list-marker tone,
/// brightening to `text` while pointed at.
#[allow(clippy::too_many_arguments)]
fn flatten(
    elem: usize,
    spans: &[Span],
    counter: &mut usize,
    cite_counter: &mut usize,
    limit: usize,
    hover: Option<usize>,
    t: &Pink,
) -> TextFlat {
    let mut flat = TextFlat {
        elem,
        text: String::new(),
        runs: Vec::new(),
        cites: Vec::new(),
    };
    let marker = t.readable_tone(t.accent, 1.);
    let mut after_cite = false;
    for (piece, space) in pieces(spans) {
        let n = *counter;
        *counter += 1;
        let ordinal = matches!(piece, Piece::Cite(_)).then(|| {
            let o = *cite_counter;
            *cite_counter += 1;
            o
        });
        if n >= limit {
            continue;
        }
        match piece {
            Piece::Word { text, strong } => {
                let weight = if strong {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                };
                push_run(&mut flat, &text, sans(weight), t.text);
                after_cite = false;
            }
            Piece::Cite(source) => {
                let ordinal = ordinal.unwrap_or_default();
                let color = if hover == Some(ordinal) {
                    t.text
                } else {
                    marker
                };
                if after_cite {
                    push_run(&mut flat, CITE_GAP, superscript(), marker);
                }
                let start = flat.text.len();
                push_run(&mut flat, &(source + 1).to_string(), superscript(), color);
                flat.cites.push(Cite {
                    range: start..flat.text.len(),
                    source,
                    ordinal,
                });
                after_cite = true;
            }
        }
        if space {
            push_run(&mut flat, " ", sans(FontWeight::NORMAL), t.text);
            after_cite = false;
        }
    }
    flat
}

/// The revealed blocks, flattened. Element keys stay stable while the reply
/// grows, so each one's veil keeps its history.
fn flatten_blocks(
    blocks: &[Block],
    limit: usize,
    hover: Option<usize>,
    t: &Pink,
) -> Vec<BlockFlat> {
    let mut counter = 0usize;
    let mut cites = 0usize;
    let mut out = Vec::new();
    for (bix, block) in blocks.iter().enumerate() {
        if counter >= limit {
            break;
        }
        match block {
            Block::Paragraph(spans) => {
                let flat = flatten(bix * 1000, spans, &mut counter, &mut cites, limit, hover, t);
                if !flat.text.is_empty() {
                    out.push(BlockFlat::Paragraph(flat));
                }
            }
            Block::Bullets(items) => {
                let mut flats = Vec::new();
                for (iix, spans) in items.iter().enumerate() {
                    if counter >= limit {
                        break;
                    }
                    let elem = bix * 1000 + 1 + iix;
                    let flat = flatten(elem, spans, &mut counter, &mut cites, limit, hover, t);
                    if !flat.text.is_empty() {
                        flats.push(flat);
                    }
                }
                if !flats.is_empty() {
                    out.push(BlockFlat::Bullets(flats));
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyTarget {
    Question,
    Answer,
}

/// pink: `ToolGroupReveal` + the group's `FoldState`.
#[derive(Default)]
struct GroupState {
    keys: Vec<SharedString>,
    starts: Vec<Option<Instant>>,
    header_started_at: Option<Instant>,
    shimmer_started_at: Option<Instant>,
    /// The user's toggle; `None` follows the auto-open rule.
    user_open: Option<bool>,
    rendered_open: Option<bool>,
    rendered_height: f32,
    fold_from: f32,
    toggled_at: Option<Instant>,
}

/// One frame of the tool group's motion.
struct GroupFrame {
    open: bool,
    /// The group is the live tail of the turn: its title shimmers.
    active: bool,
    auto_open: bool,
    header_reveal: f32,
    reveals: Vec<f32>,
    connectors: Vec<f32>,
    viewport: f32,
    body_height: f32,
    disclosure: f32,
    shimmer_start: Option<Instant>,
    moving: bool,
}

impl GroupState {
    /// pink: the reveal bookkeeping in `Transcript::sync_rows` – a new group
    /// reveals its header now and its rows from 90ms, 65ms apart; rows
    /// already on screen keep their start; a group mounted settled is
    /// history and stays still.
    fn sync(&mut self, keys: &[SharedString], historical: bool, now: Instant) {
        if keys.is_empty() {
            *self = Self::default();
            return;
        }
        let new_group = self.keys.is_empty() && !historical;
        self.shimmer_started_at.get_or_insert(now);
        if new_group {
            self.header_started_at.get_or_insert(now);
        }
        let mut arrival = 0;
        let starts = keys
            .iter()
            .enumerate()
            .map(|(ix, key)| {
                if self.keys.get(ix) == Some(key) {
                    return self.starts.get(ix).copied().flatten();
                }
                if historical {
                    return None;
                }
                let start = now + motion::tool_row_arrival_delay(arrival, new_group);
                arrival += 1;
                Some(start)
            })
            .collect();
        self.keys = keys.to_vec();
        self.starts = starts;
    }

    /// pink: `render_tool_group`'s fold – open while it is the turn's live
    /// tail (or a row is still arriving) unless the user chose; a change of
    /// openness tweens the body from its rendered height on `DISCLOSURE`.
    fn frame(&mut self, live: bool, reduced: bool, now: Instant) -> GroupFrame {
        let reveals: Vec<f32> = self
            .starts
            .iter()
            .map(|s| motion::reveal_progress(TOOL_ROW_REVEAL, *s, now, reduced))
            .collect();
        let connectors: Vec<f32> = self
            .starts
            .iter()
            .map(|s| motion::reveal_progress(TOOL_CONNECTOR_REVEAL, *s, now, reduced))
            .collect();
        let header_reveal =
            motion::reveal_progress(TOOL_ROW_REVEAL, self.header_started_at, now, reduced);
        let connector_span = TOOL_CONNECTOR_REVEAL.total().mul_f32(motion::speed_scale());
        let arrival_pending = !reduced
            && self
                .starts
                .iter()
                .flatten()
                .any(|start| now.saturating_duration_since(*start) < connector_span);
        let auto_open = live || arrival_pending;
        let open = self.user_open.unwrap_or(auto_open);
        if self.rendered_open.is_some_and(|previous| previous != open) {
            self.fold_from = self.rendered_height;
            self.toggled_at = Some(now);
        }
        self.rendered_open = Some(open);
        let viewport = CHIPS_TOP_PAD
            + reveals
                .iter()
                .map(|p| TOOL_TREE_ROW_HEIGHT * p)
                .sum::<f32>();
        let target = if open { viewport } else { 0. };
        let mut moving = header_reveal < 1.
            || reveals.iter().any(|p| *p < 1.)
            || connectors.iter().any(|p| *p < 1.);
        let (body_height, disclosure) = match self.toggled_at.filter(|_| !reduced) {
            Some(at) => {
                let t = DISCLOSURE.progress_at(now.saturating_duration_since(at));
                if t < 1. {
                    moving = true;
                } else {
                    self.toggled_at = None;
                }
                (
                    motion::lerp(self.fold_from, target, t),
                    if open { t } else { 1. - t },
                )
            }
            None => (target, if open { 1. } else { 0. }),
        };
        self.rendered_height = body_height;
        GroupFrame {
            open,
            active: live,
            auto_open,
            header_reveal,
            reveals,
            connectors,
            viewport,
            body_height,
            disclosure,
            shimmer_start: self.shimmer_started_at,
            moving,
        }
    }

    /// pink: `toggle_fold`.
    fn toggle(&mut self, open_height: f32, auto_open: bool) {
        let currently_open = self.user_open.unwrap_or(auto_open);
        self.fold_from = if currently_open { open_height } else { 0. };
        self.user_open = Some(!currently_open);
        self.toggled_at = Some(Instant::now());
    }
}

#[derive(Default)]
struct State {
    seen: bool,
    was_busy: bool,
    hover_question: bool,
    hover_answer: bool,
    hover_cite: Option<usize>,
    copied: Option<CopyTarget>,
    group: GroupState,
    veil: RowVeil,
    _copy_reset: Option<Task<()>>,
}

// ---------------------------------------------------------------------------
// The component
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct AiAnswer {
    id: ElementId,
    question: SharedString,
    query: Option<SharedString>,
    sources: Vec<Source>,
    blocks: Vec<Block>,
    status: AnswerStatus,
    reveal: Option<usize>,
    elapsed: Option<Duration>,
    asked_at: Option<SharedString>,
    answered_at: Option<SharedString>,
    width: f32,
    on_stop: Option<Handler>,
    on_regenerate: Option<Handler>,
    on_open_source: Option<SourceHandler>,
}

impl AiAnswer {
    pub fn new(id: impl Into<ElementId>, question: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            question: question.into(),
            query: None,
            sources: Vec::new(),
            blocks: Vec::new(),
            status: AnswerStatus::Writing,
            reveal: None,
            elapsed: None,
            asked_at: None,
            answered_at: None,
            width: 640.,
            on_stop: None,
            on_regenerate: None,
            on_open_source: None,
        }
    }

    /// The web search the turn ran: the tool group's first row.
    pub fn query(mut self, query: impl Into<SharedString>) -> Self {
        self.query = Some(query.into());
        self
    }

    /// The pages read so far, one fetch row each, in arrival order.
    pub fn sources(mut self, sources: Vec<Source>) -> Self {
        self.sources = sources;
        self
    }

    pub fn blocks(mut self, blocks: Vec<Block>) -> Self {
        self.blocks = blocks;
        self
    }

    pub fn status(mut self, status: AnswerStatus) -> Self {
        self.status = status;
        self
    }

    /// Shows only the first `count` words and citations (see [`piece_count`]).
    pub fn reveal(mut self, count: usize) -> Self {
        self.reveal = Some(count);
        self
    }

    /// Time since the turn began, for the working line's "· 12s".
    pub fn elapsed(mut self, elapsed: Duration) -> Self {
        self.elapsed = Some(elapsed);
        self
    }

    /// The question's timestamp in its hover lane ("Oct 23, 3:41 PM").
    pub fn asked_at(mut self, label: impl Into<SharedString>) -> Self {
        self.asked_at = Some(label.into());
        self
    }

    /// The answer's timestamp in its hover lane.
    pub fn answered_at(mut self, label: impl Into<SharedString>) -> Self {
        self.answered_at = Some(label.into());
        self
    }

    /// The column width; the question bubble caps at 80% of it.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Shows the working line's hover-only Stop.
    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }

    /// Shows Retry in the settled answer's hover lane.
    pub fn on_regenerate(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_regenerate = Some(Rc::new(handler));
        self
    }

    /// Called with the source index from a citation or a fetch row, which
    /// then wear pink's open-arrow tile.
    pub fn on_open_source(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_source = Some(Rc::new(handler));
        self
    }

    fn tool_rows(&self) -> Vec<ToolRow> {
        let mut rows = Vec::new();
        if let Some(query) = &self.query {
            rows.push(ToolRow {
                label: "Web",
                detail: query.clone(),
                key: format!("web:{query}").into(),
                source: None,
            });
        }
        for (ix, source) in self.sources.iter().enumerate() {
            let address = source.address();
            rows.push(ToolRow {
                label: "Fetch",
                key: format!("fetch:{address}").into(),
                detail: address,
                source: Some(ix),
            });
        }
        rows
    }

    /// pink: `live_status` – the newest tool in flight names the work
    /// ("Searching the web for …", "Fetching <host>"), then "Writing" once
    /// the reply streams.
    fn live_status(&self) -> (SharedString, Option<SharedString>) {
        let object = |s: &str| {
            let s = s.trim();
            (!s.is_empty())
                .then(|| SharedString::from(ellipsize_chars(s, LIVE_STATUS_OBJECT_CHARS)))
        };
        if self.status == AnswerStatus::Writing {
            return ("Writing".into(), None);
        }
        match (self.sources.last(), &self.query) {
            (Some(source), _) => ("Fetching".into(), object(&url_host(&source.address()))),
            (None, Some(query)) => ("Searching the web for".into(), object(query)),
            (None, None) => ("Searching the web".into(), None),
        }
    }
}

/// One row of the tool group (pink `tool_chip_content`: "Web" + query,
/// "Fetch" + url).
struct ToolRow {
    label: &'static str,
    detail: SharedString,
    key: SharedString,
    source: Option<usize>,
}

/// What the render helpers share.
struct Ctx<'a> {
    t: &'a Pink,
    key: &'a str,
    state: &'a Entity<State>,
    sources: Rc<Vec<Source>>,
    on_open_source: Option<SourceHandler>,
}

// ---------------------------------------------------------------------------
// Small pieces
// ---------------------------------------------------------------------------

/// pink: `meta_action` – one action in a message's hover lane: a 24px icon
/// button, transparent → `ink(0.08)` on the shared hover fade, a 14px glyph
/// in `content_muted`, a text tooltip and the inset focus ring.
fn meta_action(
    id: SharedString,
    icon: &'static str,
    label: &'static str,
    t: &Pink,
) -> Stateful<Div> {
    let ring = t.focus_ring();
    div()
        .id(id.clone())
        .role(Role::Button)
        .aria_label(label)
        .tab_index(0)
        .size(px(META_ACTION_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .cursor_pointer()
        .hover_fill(format!("tl-{id}"), gpui::transparent_black(), t.ink(0.08))
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .tooltip(pink::text_tooltip(label))
        .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
        .child(
            svg()
                .path(icon)
                .size(px(pink::icon_size::SM))
                .text_color(t.content_muted),
        )
}

/// pink: the hover metadata strip under an entry's last row – a reserved
/// 28px lane, so revealing it never moves layout, whose timestamp (12
/// `content_faint`) and actions fade in over 150ms while the entry is
/// hovered. User entries align end, under the bubble.
fn meta_lane(
    id: SharedString,
    end: bool,
    hovered: bool,
    timestamp: Option<SharedString>,
    actions: Vec<AnyElement>,
    t: &Pink,
) -> Div {
    let metadata = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.))
        .child(div().mr(px(6.)))
        .children(timestamp.map(|stamp| {
            div()
                .text_size(px(pink::text::FOOTNOTE))
                .text_color(t.content_faint)
                .child(stamp)
        }))
        .children(actions);
    div()
        .h(px(META_LANE_HEIGHT))
        .pt(px(META_LANE_HEIGHT - META_ACTION_SIZE))
        .w_full()
        .flex()
        .items_center()
        .when(end, |el| el.justify_end())
        .when(hovered, |el| el.child(motion::fade_quick(id, metadata)))
}

/// Copy in a hover lane, with pink's 1.2s "Copied" check.
fn copy_action(
    id: SharedString,
    target: CopyTarget,
    text: SharedString,
    copied: bool,
    state: &Entity<State>,
    t: &Pink,
) -> Stateful<Div> {
    let state = state.clone();
    meta_action(
        id,
        if copied { ICON_COPY_CHECK } else { ICON_COPY },
        if copied { "Copied" } else { "Copy" },
        t,
    )
    .on_click(move |_, _, cx| {
        cx.stop_propagation();
        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
        state.update(cx, |s, cx| {
            s.copied = Some(target);
            s._copy_reset = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(COPIED_FOR).await;
                this.update(cx, |s, cx| {
                    s.copied = None;
                    cx.notify();
                })
                .ok();
            }));
            cx.notify();
        });
    })
}

/// pink: the working trailer's "Stop" – a quiet way out that appears only
/// under the pointer (or on focus): 11/16 `content_muted`, `ml 4 px 6 r 6`,
/// the shared hover wash.
fn stop_button(id: SharedString, group: SharedString, on_stop: Handler, t: &Pink) -> Stateful<Div> {
    div()
        .id(id.clone())
        .role(Role::Button)
        .aria_label("Stop the current turn")
        .tab_index(0)
        .flex_none()
        .ml(px(space::XS))
        .px(px(space::S6))
        .rounded(px(radius::CONTROL))
        .cursor_pointer()
        .text_size(px(WORKING_TIMER_SIZE))
        .line_height(px(WORKING_STATUS_LINE_HEIGHT))
        .text_color(t.content_muted)
        .opacity(0.)
        .group_hover(group, |style| style.opacity(1.))
        .focus_visible(|style| style.opacity(1.))
        .hover_wash(format!("tl-{id}"), t.fill(FillTier::Tertiary))
        .child("Stop")
        .on_click(move |_: &ClickEvent, window, cx| {
            cx.stop_propagation();
            on_stop(window, cx);
        })
}

/// pink: `reveal_tool_row` – an arriving row grows to its height.
fn reveal_tool_row(row: AnyElement, height: f32, progress: f32) -> AnyElement {
    if progress >= 1. {
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

/// pink: `activity_branch_points` – the elbow from the trunk to the branch
/// tip, cut to `progress` of its length.
fn branch_points(progress: f32) -> Vec<Point<f32>> {
    let mut path: Vec<Point<f32>> = (0..=24)
        .map(|step| {
            let t = step as f32 / 24.;
            point(
                ACTIVITY_BEND_RADIUS * t * t,
                ACTIVITY_BEND_RADIUS * (2. * t - t * t),
            )
        })
        .collect();
    path.push(point(
        ACTIVITY_BRANCH_END_X - ACTIVITY_TRUNK_X,
        ACTIVITY_BEND_RADIUS,
    ));
    if progress >= 1. {
        return path;
    }
    let lengths: Vec<f32> = path
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .collect();
    let mut remaining = lengths.iter().sum::<f32>() * progress.clamp(0., 1.);
    let mut visible = vec![path[0]];
    for (pair, length) in path.windows(2).zip(lengths) {
        if remaining <= 0. {
            break;
        }
        let t = (remaining / length).min(1.);
        visible.push(point(
            motion::lerp(pair[0].x, pair[1].x, t),
            motion::lerp(pair[0].y, pair[1].y, t),
        ));
        remaining -= length;
    }
    visible
}

/// pink: `activity_ribbon` – a 1px ribbon contour; nonzero fill unions the
/// trunk and the elbow so the fork paints one alpha.
fn ribbon(path: &mut PathBuilder, points: &[Point<Pixels>]) {
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

/// pink: `activity_rail` – the 48px gutter of one row: the trunk at x 12.5
/// drawing down from the row above, a 6px elbow to x 28 and the tool glyph
/// (14, `content_muted`) at x 32, or the running row's mini spinner. The
/// ribbons are `hairline(0.12)`.
fn activity_rail(
    has_predecessor: bool,
    continues: bool,
    reveal: f32,
    continuation: f32,
    running: Option<AnyElement>,
    t: &Pink,
) -> Div {
    let color = t.hairline(0.12);
    let row_height = TOOL_TREE_ROW_HEIGHT;
    let (incoming, branch) = motion::tool_connector_parts(reveal, has_predecessor);
    let icon_top = row_height / 2. - ACTIVITY_ICON_SIZE / 2.;
    div()
        .relative()
        .w(px(ACTIVITY_GUTTER_WIDTH))
        .flex_none()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let x = bounds.origin.x + px(ACTIVITY_TRUNK_X);
                    let bend_y = bounds.origin.y + px(row_height / 2. - ACTIVITY_BEND_RADIUS);
                    let mut tree = PathBuilder::fill().with_style(PathStyle::Fill(
                        FillOptions::default().with_fill_rule(FillRule::NonZero),
                    ));
                    if incoming > 0. {
                        let mut bottom = point(
                            x,
                            bounds.origin.y
                                + px((row_height / 2. - ACTIVITY_BEND_RADIUS) * incoming),
                        );
                        if incoming >= 1. && continues && continuation > 0. {
                            let below = (f32::from(bounds.size.height)
                                - (row_height / 2. - ACTIVITY_BEND_RADIUS))
                                .max(0.);
                            bottom = point(x, bend_y + px(below * continuation));
                        }
                        ribbon(&mut tree, &[point(x, bounds.origin.y), bottom]);
                    }
                    if branch > 0. {
                        let points: Vec<Point<Pixels>> = branch_points(branch)
                            .into_iter()
                            .map(|p| point(x + px(p.x), bend_y + px(p.y)))
                            .collect();
                        ribbon(&mut tree, &points);
                    }
                    if let Ok(path) = tree.build() {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(match running {
            Some(spinner) => div()
                .absolute()
                .left(px(ACTIVITY_ICON_LEFT))
                .top(px(icon_top))
                .size(px(ACTIVITY_ICON_SIZE))
                .flex()
                .items_center()
                .justify_center()
                .opacity(branch)
                .child(spinner)
                .into_any_element(),
            None => svg()
                .path(ICON_GLOBAL)
                .absolute()
                .left(px(ACTIVITY_ICON_LEFT))
                .top(px(icon_top))
                .size(px(ACTIVITY_ICON_SIZE))
                .opacity(branch)
                .text_color(t.content_muted)
                .into_any_element(),
        })
}

/// pink: `chip_header_row` for an activity row – label and detail at 12/18
/// in `content_muted`, the detail truncating. A row that opens its page
/// (`hover` names its fade) gets the trailing open-arrow tile, half strength
/// until the row is hovered, and its text brightens to `text` with it.
fn chip_header_row(row: &ToolRow, hover: Option<&str>, t: &Pink) -> Div {
    let tone = hover.map_or(t.content_muted, |key| {
        motion::hover_blend(key, t.content_muted, t.text)
    });
    div()
        .h(px(CHIP_CARD_HEIGHT))
        .w_full()
        .min_w_0()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::SM))
        .text_size(px(TOOL_LABEL_SIZE))
        .line_height(px(TOOL_LABEL_LINE_HEIGHT))
        .text_color(tone)
        .child(
            div()
                .flex_none()
                .h(px(TOOL_LABEL_LINE_HEIGHT))
                .flex()
                .items_center()
                .child(row.label),
        )
        .child(
            div()
                .min_w_0()
                .h(px(TOOL_LABEL_LINE_HEIGHT))
                .flex()
                .items_center()
                .truncate()
                .child(div().min_w_0().truncate().child(row.detail.clone())),
        )
        .child(div().flex_1().min_w(px(4.)))
        .when_some(hover, |el, key| {
            el.child(
                div()
                    .size(px(TRAIL_TILE))
                    .flex_none()
                    .opacity(0.5 + 0.5 * motion::hover_t(key))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(ICON_OPEN)
                            .size(px(DISCLOSURE_CHEVRON_SIZE))
                            .text_color(t.readable_muted(0.8)),
                    ),
            )
        })
}

/// pink: `badges.rs` `BadgeCard` – the quoted-passage hover card, here
/// previewing a citation: a 320px plate, one inset passage row (`p 8 r 6`,
/// `ink(0.05)`, a 2px `solid @ 0.35` bar) holding the host in mono 11 with
/// the citation's number as a tag, the page title, and the passage.
struct CitationCard {
    number: usize,
    source: Source,
}

impl Render for CitationCard {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let passage = div()
            .flex()
            .flex_row()
            .gap(px(space::SM))
            .p(px(space::SM))
            .rounded(px(radius::SM))
            .bg(t.ink(0.05))
            .child(
                div()
                    .flex_none()
                    .w(px(2.))
                    .rounded(px(1.))
                    .bg(t.solid.opacity(0.35)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(space::XS))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(space::S6))
                            .font_family(pink::MONO)
                            .text_size(px(pink::text::CAPTION))
                            .line_height(px(14.))
                            .text_color(t.text_faint)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(self.source.site.clone()),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .px(px(space::XS))
                                    .rounded(px(3.))
                                    .bg(t.ink(0.10))
                                    .child(self.number.to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(pink::text::FOOTNOTE))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .child(self.source.title.clone()),
                    )
                    .when(!self.source.snippet.is_empty(), |el| {
                        el.child(
                            div()
                                .min_w_0()
                                .text_size(px(pink::text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.readable_muted(0.85))
                                .child(self.source.snippet.clone()),
                        )
                    }),
            );
        pink::popover_card(&t)
            .bg(t.tooltip_surface())
            .w(px(CARD_WIDTH))
            .p(px(space::S6))
            .flex()
            .flex_col()
            .gap(px(space::XS))
            .child(passage)
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl AiAnswer {
    /// pink: `RowKind::User` – the question in the right-aligned bubble
    /// (`user_bubble_bg`, a 1px inset edge in light, r16, `px 16 py 10`,
    /// 14/22), then its hover lane.
    fn render_question(&self, cx: &Ctx, hovered: bool, copied: bool) -> AnyElement {
        let t = cx.t;
        let bubble = div()
            .min_w_0()
            .max_w(px(self.width * USER_BUBBLE_SHARE))
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
            .px(px(space::LG))
            .py(px(space::S10))
            .text_size(px(MD_TEXT_SIZE))
            .line_height(px(USER_LINE_HEIGHT))
            .text_color(t.text)
            .child(self.question.clone());
        let state = cx.state.clone();
        let copy = copy_action(
            format!("{}-copy-q", cx.key).into(),
            CopyTarget::Question,
            self.question.clone(),
            copied,
            cx.state,
            t,
        );
        div()
            .id(SharedString::from(format!("{}-question", cx.key)))
            .w_full()
            .flex()
            .flex_col()
            .on_hover(move |hovered, _, cx| {
                let hovered = *hovered;
                state.update(cx, |s, cx| {
                    if s.hover_question != hovered {
                        s.hover_question = hovered;
                        cx.notify();
                    }
                })
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_end()
                    .items_end()
                    .child(bubble),
            )
            .child(meta_lane(
                format!("{}-meta-q", cx.key).into(),
                true,
                hovered,
                self.asked_at.clone(),
                vec![copy.into_any_element()],
                t,
            ))
            .into_any_element()
    }

    /// pink: `render_tool_group` – the summary plate (24px, r6, Quaternary →
    /// Tertiary with its text and glyph brightening, a chevron turning 90°
    /// as it opens) over the fold of rail rows.
    fn render_group(&self, cx: &Ctx, rows: &[ToolRow], frame: &GroupFrame) -> AnyElement {
        let t = cx.t;
        let key = cx.key;
        let ring = t.focus_ring();
        let searches = usize::from(self.query.is_some());
        let summary: SharedString = tool_group_summary(searches, self.sources.len()).into();
        let hover_key = format!("tl-{key}-hdr");
        let title = match frame.shimmer_start.filter(|_| frame.active) {
            Some(start) => shimmer_text(
                summary.clone(),
                TOOL_LABEL_SIZE,
                TOOL_LABEL_LINE_HEIGHT,
                t.content_muted,
                t.text,
            )
            .started_at(start)
            .flex_none()
            .into_any_element(),
            None => summary.clone().into_any_element(),
        };
        let toggle = {
            let state = cx.state.clone();
            let (viewport, auto_open) = (frame.viewport, frame.auto_open);
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                cx.stop_propagation();
                state.update(cx, |s, cx| {
                    s.group.toggle(viewport, auto_open);
                    cx.notify();
                })
            }
        };
        let header = div()
            .h(px(TOOL_GROUP_HEADER_HEIGHT))
            .flex()
            .flex_row()
            .items_center()
            .child(
                div()
                    .id(SharedString::from(format!("{key}-hdr")))
                    .rounded(px(radius::CONTROL))
                    .focus_visible(move |style| pink::focus_ring(style, ring))
                    .role(Role::Button)
                    .aria_label(SharedString::from(format!("Tool activity: {summary}")))
                    .aria_expanded(frame.open)
                    .tab_index(0)
                    .flex_none()
                    .max_w_full()
                    .min_w_0()
                    .h(px(TOOL_GROUP_HEADER_HEIGHT - 2.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(space::S6))
                    .pl(px(ACTIVITY_TRUNK_X - ACTIVITY_ICON_SIZE / 2.))
                    .pr(px(space::SM))
                    .cursor_pointer()
                    .text_size(px(TOOL_LABEL_SIZE))
                    .line_height(px(TOOL_LABEL_LINE_HEIGHT))
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
                            .path(ICON_GLOBAL)
                            .flex_none()
                            .size(px(ACTIVITY_ICON_SIZE))
                            .text_color(motion::hover_blend(&hover_key, t.content_muted, t.text)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .h(px(TOOL_LABEL_LINE_HEIGHT))
                            .flex()
                            .items_center()
                            .truncate()
                            .child(title),
                    )
                    .child(
                        svg()
                            .path(ICON_CHEVRON)
                            .flex_none()
                            .size(px(DISCLOSURE_CHEVRON_SIZE))
                            .with_transformation(Transformation::rotate(radians(
                                FRAC_PI_2 * frame.disclosure,
                            )))
                            .text_color(t.content_faint),
                    ),
            );

        let body = (frame.body_height > 0.).then(|| {
            let searching = self.status == AnswerStatus::Searching;
            let chips = div().pt(px(CHIPS_TOP_PAD)).flex().flex_col().children(
                rows.iter().enumerate().map(|(ix, row)| {
                    let connector = frame.connectors[ix];
                    let content = motion::tool_connector_parts(connector, ix > 0).1;
                    let continuation =
                        motion::tool_connector_continuation(frame.connectors.get(ix + 1).copied());
                    let running = (searching && ix + 1 == rows.len()).then(|| {
                        mini_glyph_spinner(format!("{key}-running-{ix}"), 2., t.glyph)
                            .into_any_element()
                    });
                    let rail = activity_rail(
                        ix > 0,
                        ix + 1 < rows.len(),
                        connector,
                        continuation,
                        running,
                        t,
                    );
                    let link = row.source.zip(cx.on_open_source.clone());
                    let hover_key = format!("tl-{key}-tool-{ix}");
                    let header =
                        chip_header_row(row, link.is_some().then_some(hover_key.as_str()), t);
                    let header = match link {
                        Some((source, on_open)) => div()
                            .id(SharedString::from(format!("{key}-tool-btn-{ix}")))
                            .on_hover(motion::hover_listener(hover_key.clone()))
                            .role(Role::Button)
                            .aria_label(SharedString::from(format!(
                                "{}: {}",
                                row.label, row.detail
                            )))
                            .tab_index(0)
                            .rounded(px(radius::CONTROL))
                            .focus_visible(move |style| pink::focus_ring(style, ring))
                            .h(px(CHIP_CARD_HEIGHT))
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                on_open(source, window, cx);
                            })
                            .child(header)
                            .into_any_element(),
                        None => header.into_any_element(),
                    };
                    let card = div()
                        .ml(px(ACTIVITY_TEXT_GAP))
                        .my(px((TOOL_TREE_ROW_HEIGHT - CHIP_CARD_HEIGHT) / 2.))
                        .h(px(CHIP_CARD_HEIGHT))
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .when(content < 1., |card| {
                            card.relative()
                                .top(px(4. * (1. - content)))
                                .opacity(content)
                        })
                        .child(header);
                    let row_el = div()
                        .h(px(TOOL_TREE_ROW_HEIGHT))
                        .w_full()
                        .flex_none()
                        .flex()
                        .flex_row()
                        .child(rail)
                        .child(card)
                        .into_any_element();
                    reveal_tool_row(row_el, TOOL_TREE_ROW_HEIGHT, frame.reveals[ix])
                }),
            );
            div()
                .overflow_hidden()
                .h(px(frame.body_height))
                .child(chips)
        });

        div()
            .relative()
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .child(reveal_tool_row(
                header.into_any_element(),
                TOOL_GROUP_HEADER_HEIGHT,
                frame.header_reveal,
            ))
            .children(body)
            .into_any_element()
    }

    /// One paragraph or list item: shaped runs at 14/22 (pink:
    /// `text_element`), veiled while streaming. Citations are clickable
    /// ranges with a hover card.
    fn render_text(cx: &Ctx, flat: TextFlat, spans: &[VeilSpan]) -> AnyElement {
        let key = cx.key;
        let elem = flat.elem;
        let runs = apply_veil(flat.runs, spans);
        let styled = StyledText::new(flat.text).with_runs(runs);
        if flat.cites.is_empty() {
            return div().w_full().child(styled).into_any_element();
        }
        let cites = Rc::new(flat.cites);
        let hover = {
            let cites = cites.clone();
            let state = cx.state.clone();
            move |index: Option<usize>, _: MouseMoveEvent, _: &mut Window, cx: &mut App| {
                let hovered = index.and_then(|i| {
                    cites
                        .iter()
                        .find(|c| c.range.contains(&i))
                        .map(|c| c.ordinal)
                });
                state.update(cx, |s, cx| {
                    if s.hover_cite != hovered {
                        s.hover_cite = hovered;
                        cx.notify();
                    }
                });
            }
        };
        let tooltip = {
            let cites = cites.clone();
            let sources = cx.sources.clone();
            move |index: usize, _: &mut Window, cx: &mut App| -> Option<AnyView> {
                let cite = cites.iter().find(|c| c.range.contains(&index))?;
                let source = sources.get(cite.source)?.clone();
                let number = cite.source + 1;
                Some(cx.new(|_| CitationCard { number, source }).into())
            }
        };
        let mut text = InteractiveText::new(SharedString::from(format!("{key}-md-{elem}")), styled)
            .on_hover(hover)
            .tooltip(tooltip);
        if let Some(on_open) = cx.on_open_source.clone() {
            let ranges = cites.iter().map(|c| c.range.clone()).collect();
            let cites = cites.clone();
            text = text.on_click(ranges, move |ix, window, cx| {
                if let Some(cite) = cites.get(ix) {
                    on_open(cite.source, window, cx);
                }
            });
        }
        let leave = cx.state.clone();
        div()
            .id(SharedString::from(format!("{key}-md-{elem}-box")))
            .w_full()
            .on_hover(move |hovered, _, cx| {
                if !*hovered {
                    leave.update(cx, |s, cx| {
                        if s.hover_cite.take().is_some() {
                            cx.notify();
                        }
                    });
                }
            })
            .child(text)
            .into_any_element()
    }

    /// pink: `render_block` – a paragraph, or a list of 5px accent discs in
    /// an 18px column, 8px from the text, items 4px apart.
    fn render_block(
        cx: &Ctx,
        block: BlockFlat,
        veils: &mut HashMap<usize, Vec<VeilSpan>>,
    ) -> AnyElement {
        let t = cx.t;
        let mut text = |flat: TextFlat| {
            let spans = veils.remove(&flat.elem).unwrap_or_default();
            Self::render_text(cx, flat, &spans)
        };
        let el = match block {
            BlockFlat::Paragraph(flat) => text(flat),
            BlockFlat::Bullets(items) => div()
                .flex()
                .flex_col()
                .gap(px(LIST_ITEM_GAP))
                .children(items.into_iter().map(|flat| {
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(LIST_MARKER_GAP))
                        .child(
                            div()
                                .flex_none()
                                .min_w(px(LIST_MARKER_MIN_W))
                                .h(px(MD_LINE_HEIGHT))
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .ml(px(1.))
                                        .w(px(LIST_DISC))
                                        .h(px(LIST_DISC))
                                        .rounded_full()
                                        .bg(t.accent),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap(px(LIST_ITEM_GAP))
                                .child(text(flat)),
                        )
                }))
                .into_any_element(),
        };
        div()
            .w_full()
            .text_size(px(MD_TEXT_SIZE))
            .line_height(px(MD_LINE_HEIGHT))
            .text_color(t.text)
            .child(el)
            .into_any_element()
    }

    /// pink: `render_working_trailer` – `pt 16`, `gap 8`, top-aligned: the
    /// breathing orb, the shimmering status, "· 12s" and a hover-only Stop.
    fn render_trailer(&self, cx: &Ctx) -> AnyElement {
        let t = cx.t;
        let group: SharedString = format!("{}-working", cx.key).into();
        let (verb, object) = self.live_status();
        let mut status = working_status(verb, t.content_muted, t.text);
        if let Some(object) = object {
            status = status.object(object);
        }
        div()
            .id(SharedString::from(format!("{}-working-trailer", cx.key)))
            .group(group.clone())
            .flex()
            .flex_row()
            .items_start()
            .gap(px(space::SM))
            .pt(px(space::LG))
            .min_w_0()
            .child(orb_slot(t.accent))
            .child(status)
            .when_some(self.elapsed, |el, elapsed| {
                el.child(working_timer(elapsed.as_secs() as i64, t.content_faint))
            })
            .when_some(self.on_stop.clone(), |el, on_stop| {
                el.child(stop_button(
                    format!("{}-stop", cx.key).into(),
                    group.clone(),
                    on_stop,
                    t,
                ))
            })
            .into_any_element()
    }
}

impl RenderOnce for AiAnswer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let reduced = motion::reduced_motion(cx);
        let now = Instant::now();
        let key = self.id.to_string();
        // One state per question, so a new question starts a fresh turn.
        let state = window.use_keyed_state(
            ElementId::NamedChild(
                Arc::new(self.id.clone()),
                format!("turn-{:x}", fnv1a(self.question.as_bytes())).into(),
            ),
            cx,
            |_, _: &mut Context<State>| State::default(),
        );
        let busy = self.status.busy();
        let searching = self.status == AnswerStatus::Searching;
        // pink: only a live row wears the veil; reduced motion renders none.
        let streaming = self.status == AnswerStatus::Writing && !reduced;
        let limit = self.reveal.unwrap_or(usize::MAX);

        let rows = self.tool_rows();
        let row_keys: Vec<SharedString> = rows.iter().map(|r| r.key.clone()).collect();
        let hover_cite = state.read(cx).hover_cite;
        let blocks = flatten_blocks(&self.blocks, limit, hover_cite, &t);

        let (group, mut veils, fading, hover_question, hover_answer, copied) =
            state.update(cx, |s, _| {
                // A settled turn that starts again (Retry) is a new reply.
                if s.seen && busy && !s.was_busy {
                    s.group = GroupState::default();
                    s.veil = RowVeil::default();
                    s.hover_cite = None;
                }
                let historical = !s.seen && !busy;
                if !s.seen {
                    s.veil.seeding = true;
                }
                s.seen = true;
                s.was_busy = busy;
                s.group.sync(&row_keys, historical, now);
                let group = (!rows.is_empty()).then(|| s.group.frame(searching, reduced, now));
                let mut veils: HashMap<usize, Vec<VeilSpan>> = HashMap::new();
                if streaming {
                    let mut seen = HashSet::new();
                    let mut advance = |flat: &TextFlat| {
                        seen.insert(flat.elem);
                        veils.insert(flat.elem, s.veil.advance(flat.elem, &flat.text, now));
                    };
                    for block in &blocks {
                        match block {
                            BlockFlat::Paragraph(flat) => advance(flat),
                            BlockFlat::Bullets(items) => items.iter().for_each(&mut advance),
                        }
                    }
                    s.veil.elems.retain(|elem, _| seen.contains(elem));
                } else {
                    s.veil.elems.clear();
                }
                s.veil.seeding = false;
                (
                    group,
                    veils,
                    s.veil.is_fading(),
                    s.hover_question,
                    s.hover_answer,
                    s.copied,
                )
            });
        if fading {
            motion::pulse_keep(window, cx);
        }
        if group.as_ref().is_some_and(|g| g.moving) {
            window.request_animation_frame();
        }

        let ctx = Ctx {
            t: &t,
            key: &key,
            state: &state,
            sources: Rc::new(self.sources.clone()),
            on_open_source: self.on_open_source.clone(),
        };

        let question =
            self.render_question(&ctx, hover_question, copied == Some(CopyTarget::Question));

        // ---- the assistant entry -------------------------------------------
        let has_group = group.is_some();
        let group_el = group
            .as_ref()
            .map(|frame| self.render_group(&ctx, &rows, frame));
        // pink: `top_gap_for` – 16 opens a turn, 12 follows a tool group and
        // separates markdown blocks.
        let first_gap = if has_group { space::MD } else { space::LG };
        let answer_blocks: Vec<AnyElement> = blocks
            .into_iter()
            .enumerate()
            .map(|(ix, block)| {
                div()
                    .pt(px(if ix == 0 { first_gap } else { MD_BLOCK_GAP }))
                    .child(Self::render_block(&ctx, block, &mut veils))
                    .into_any_element()
            })
            .collect();

        let lane = (!busy).then(|| {
            let copy_text = plain_text_upto(&self.blocks, limit);
            let retry = self.on_regenerate.clone().map(|on_regenerate| {
                meta_action(format!("{key}-retry").into(), ICON_RESTART, "Retry", &t)
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        on_regenerate(window, cx);
                    })
                    .on_key_down(pink::swallow_activation_key)
                    .into_any_element()
            });
            let copy = (!copy_text.is_empty()).then(|| {
                copy_action(
                    format!("{key}-copy-a").into(),
                    CopyTarget::Answer,
                    copy_text.into(),
                    copied == Some(CopyTarget::Answer),
                    &state,
                    &t,
                )
                .into_any_element()
            });
            meta_lane(
                format!("{key}-meta-a").into(),
                false,
                hover_answer,
                self.answered_at.clone(),
                retry.into_iter().chain(copy).collect(),
                &t,
            )
        });
        let trailer = busy.then(|| self.render_trailer(&ctx));

        let hover_state = state.clone();
        let answer = div()
            .id(SharedString::from(format!("{key}-answer")))
            .w_full()
            .flex()
            .flex_col()
            .on_hover(move |hovered, _, cx| {
                let hovered = *hovered;
                hover_state.update(cx, |s, cx| {
                    if s.hover_answer != hovered {
                        s.hover_answer = hovered;
                        cx.notify();
                    }
                })
            })
            .when_some(group_el, |el, group| {
                el.child(div().pt(px(space::LG)).child(group))
            })
            .children(answer_blocks)
            .children(lane)
            .children(trailer);

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(question)
            .child(answer)
    }
}

// ---------------------------------------------------------------------------
// Story: a security question answered from four pages.
// ---------------------------------------------------------------------------

const QUESTION: &str = "Is it safe to keep refresh tokens in localStorage?";
const QUERY: &str = "refresh token localStorage XSS best practice";
const ASKED_AT: &str = "Oct 23, 3:41 PM";

fn library() -> Vec<Source> {
    vec![
        Source::new("HTML5 Security Cheat Sheet", "cheatsheetseries.owasp.org")
            .url("https://cheatsheetseries.owasp.org/cheatsheets/HTML5_Security_Cheat_Sheet.html")
            .snippet("Do not store session identifiers in local storage as the data is always accessible by JavaScript. Cookies can mitigate this risk using the httpOnly flag."),
        Source::new("Set-Cookie header", "developer.mozilla.org")
            .url("https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Set-Cookie")
            .snippet("HttpOnly forbids JavaScript from accessing the cookie, for example through the Document.cookie property."),
        Source::new("RFC 9700: Best Current Practice for OAuth 2.0 Security", "datatracker.ietf.org")
            .url("https://datatracker.ietf.org/doc/html/rfc9700")
            .snippet("Refresh tokens for public clients must be sender-constrained or use refresh token rotation, and should expire when the client has been inactive for some time."),
        Source::new("OAuth 2.0 for Browser-Based Applications", "datatracker.ietf.org")
            .url("https://datatracker.ietf.org/doc/html/draft-ietf-oauth-browser-based-apps")
            .snippet("In the Backend for Frontend pattern a server-side component obtains and stores the tokens, so they never reach the browser; the frontend holds only a session cookie."),
    ]
}

fn answer() -> Vec<Block> {
    vec![
        Block::paragraph(vec![
            Span::strong("Not if you can avoid it."),
            Span::text(
                " Every script on the page can read localStorage, so one XSS bug hands an attacker a token that outlives the session",
            ),
            Span::cite(0),
            Span::text("."),
        ]),
        Block::bullets(vec![
            vec![
                Span::strong("Use a cookie:"),
                Span::text(
                    " keep the refresh token in an HttpOnly, Secure, SameSite cookie that page scripts can't read",
                ),
                Span::cite(0),
                Span::cite(1),
                Span::text("."),
            ],
            vec![
                Span::strong("Rotate it:"),
                Span::text(
                    " issue a new refresh token on every use and revoke the whole family when an old one comes back",
                ),
                Span::cite(2),
                Span::text("."),
            ],
            vec![
                Span::strong("Or keep it server-side:"),
                Span::text(
                    " a backend for frontend holds the tokens and gives the browser only a session cookie",
                ),
                Span::cite(3),
                Span::text("."),
            ],
        ]),
        Block::paragraph(vec![
            Span::text(
                "If the token has to live in the browser, pair rotation with an idle timeout so a stolen one expires on its own",
            ),
            Span::cite(2),
            Span::text("."),
        ]),
    ]
}

/// When each page comes back, in seconds after the question.
const FETCHED_AT: [f32; 4] = [0.6, 1.05, 1.45, 1.85];
/// When the reply starts streaming.
const WRITE_AT: f32 = 2.4;
/// Words and citations streamed per second.
const PIECES_PER_SEC: f32 = 28.;
/// The stream closes a beat after its last word, so that word's veil lands.
const SETTLE_AFTER: f32 = 0.4;
/// Room for the settled turn and its hover lane.
const STAGE_HEIGHT: f32 = 460.;

fn settles_at() -> f32 {
    WRITE_AT + piece_count(&answer()) as f32 / PIECES_PER_SEC + SETTLE_AFTER
}

struct Demo {
    started: Instant,
    stopped: Option<f32>,
    _ticker: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let alive = this.update(cx, |demo, cx| {
                    if demo.stopped.is_none() && demo.at() < settles_at() + 0.1 {
                        cx.notify();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Demo {
            started: Instant::now(),
            stopped: None,
            _ticker: ticker,
        }
    }

    fn replay(&mut self) {
        self.started = Instant::now();
        self.stopped = None;
    }

    fn at(&self) -> f32 {
        self.stopped
            .unwrap_or_else(|| self.started.elapsed().as_secs_f32())
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("ai-answer-demo", cx, |_, cx| Demo::new(cx));
        let d = demo.read(cx);
        let at = d.at();
        let blocks = answer();
        let total = piece_count(&blocks);
        let found = FETCHED_AT.iter().filter(|fetched| **fetched <= at).count();
        let shown = ((at - WRITE_AT).max(0.) * PIECES_PER_SEC) as usize;
        let status = if d.stopped.is_some() {
            AnswerStatus::Stopped
        } else if at < WRITE_AT {
            AnswerStatus::Searching
        } else if at < settles_at() {
            AnswerStatus::Writing
        } else {
            AnswerStatus::Done
        };
        let stop = demo.clone();
        let replay = demo.clone();
        let turn = AiAnswer::new("ai-answer", QUESTION)
            .query(QUERY)
            .sources(library().into_iter().take(found).collect())
            .blocks(blocks)
            .reveal(shown.min(total))
            .status(status)
            .elapsed(Duration::from_secs_f32(at))
            .asked_at(ASKED_AT)
            .answered_at(ASKED_AT)
            .on_stop(move |_, cx| {
                stop.update(cx, |d, cx| {
                    d.stopped = Some(d.at());
                    cx.notify();
                })
            })
            .on_regenerate(move |_, cx| {
                replay.update(cx, |d, cx| {
                    d.replay();
                    cx.notify();
                })
            })
            .on_open_source(|ix, _, cx| {
                if let Some(source) = library().get(ix) {
                    cx.open_url(&source.address());
                }
            });
        // A transcript column: the turn grows downward from its question
        // instead of re-centering as the reply streams.
        div().w(px(640.)).h(px(STAGE_HEIGHT)).child(turn)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_keep_spaces_and_glue_citations() {
        let spans = vec![
            Span::strong("Mobile Safari"),
            Span::text(" fell"),
            Span::cite(0),
            Span::text("."),
        ];
        let out = pieces(&spans);
        let spaced: Vec<bool> = out.iter().map(|(_, space)| *space).collect();
        // "Mobile" "Safari" "fell" [1] "." – a space only between the words.
        assert_eq!(spaced, vec![true, true, false, false, false]);
        assert!(matches!(out[3].0, Piece::Cite(0)));
        assert_eq!(piece_count(&[Block::paragraph(spans)]), 5);
    }

    #[test]
    fn plain_text_numbers_citations() {
        let blocks = vec![Block::bullets(vec![vec![Span::text("Up"), Span::cite(1)]])];
        assert_eq!(plain_text(&blocks), "- Up[2]");
        let stopped = vec![Block::paragraph(vec![Span::text("One two three")])];
        assert_eq!(plain_text_upto(&stopped, 2), "One two");
    }

    #[test]
    fn summary_and_status_read_like_pink() {
        assert_eq!(
            tool_group_summary(1, 4),
            "Searched 1 time · fetched 4 pages"
        );
        assert_eq!(tool_group_summary(0, 1), "Fetched 1 page");
        assert_eq!(
            url_host("https://datatracker.ietf.org/doc/html/rfc9700"),
            "datatracker.ietf.org"
        );
    }

    #[test]
    fn veil_fades_each_append_once() {
        let mut veil = ElemVeil::default();
        let t0 = Instant::now();
        assert_eq!(veil.advance("Not if", t0).len(), 1);
        let later = t0 + Duration::from_millis(500);
        assert!(veil.advance("Not if", later).is_empty());
        let spans = veil.advance("Not if you", later);
        assert_eq!(spans[0].0, 6..10);
        assert!(spans[0].1 < 0.01);
        assert_eq!(veil_opacity(1.), 1.);
    }

    #[test]
    fn citations_flatten_to_superscript_ranges() {
        let t = Pink::dark();
        let spans = vec![
            Span::text("Read it"),
            Span::cite(0),
            Span::cite(1),
            Span::text("."),
        ];
        let (mut n, mut c) = (0, 0);
        let flat = flatten(0, &spans, &mut n, &mut c, usize::MAX, None, &t);
        assert_eq!(flat.text, format!("Read it1{CITE_GAP}2."));
        assert_eq!(flat.cites.len(), 2);
        assert_eq!(&flat.text[flat.cites[1].range.clone()], "2");
        assert_eq!(
            flat.runs.iter().map(|r| r.len).sum::<usize>(),
            flat.text.len()
        );
    }
}
