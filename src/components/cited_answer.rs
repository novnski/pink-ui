//! Cited Answer – a Pink component.
//!
//! An answer beside the sources it rests on. The answer is bare markdown
//! prose at Pink's 14/22 (Pink: `markdown/render.rs` `text_element`) on the
//! canvas, with no card around it. Every sentence ends in superscript
//! citation numbers in Pink's list-marker tone. Point at one and its sentence
//! takes a soft accent wash while the rest of the answer steps back, its
//! source card lights up in the column beside it, and the source's name shows
//! in Pink's link-destination tooltip (Pink: `markdown/link_destination.rs`).
//!
//! The sources are Pink's expandable chip cards (Pink: `transcript.rs`
//! `chip_header_row`): a quiet r10 plate holding the citation number, the
//! title and the site, with a chevron that turns as the card folds open on
//! Pink's tool fold. Inside is the quoted passage in Pink's blockquote, and
//! when a citation opened it, the words that back the sentence are marked
//! one after another in Pink's match wash (Pink: `popover.rs`
//! `search_highlight`). Below the quote, the message meta lane copies it or
//! opens the original (Pink: `transcript.rs` `meta_action`).
//!
//! ```ignore
//! CitedAnswer::new("refunds", "Can customers on annual plans get a refund?")
//!     .sources(vec![CitedSource::new("Refunds and cancellations", "Help Center").passage("…")])
//!     .paragraph(vec![
//!         Sentence::new("Annual plans get a full refund within 30 days.")
//!             .cite(Citation::new(0).mark("refunded in full within 30 days")),
//!     ])
//!     .default_open(0)
//!     .on_open_source(|source, _, _| open_in_browser(source))
//! ```

use std::{cell::Cell, f32::consts::FRAC_PI_2, ops::Range, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, BorderStyle, Bounds, ClipboardItem, Context, Div, ElementId, Font,
    FontWeight, Hsla, InteractiveText, IntoElement, Pixels, Render, Role, SharedString, Stateful,
    StyledText, Task, TextLayout, TextRun, Window, canvas, div, point, prelude::*, px, quad,
    radians, size, svg, transparent_black,
};

use crate::{
    motion::{
        self, FOLD_TWEEN_WINDOW, HOVER_FADE, TOOL_BODY_FADE, TOOL_BODY_RISE, TOOL_ROW_REVEAL,
        TOOL_ROW_STAGGER_MS,
    },
    pink::{self, Pink, TextStyle, TypeStyleExt, layout, radius, space, text},
    story,
};

#[allow(dead_code)]
mod package {
    include!("../../components/cited-answer/src/lib.rs");
}
pub use package::{
    CARD_GAP, CARD_RADIUS, CITE_GAP, CardStyle, HEADER_H, HEADING_H, HeadingStyle, PREVIEW_PAD,
    PREVIEW_W, PreviewStyle, QuoteStyle, TILE, Tile, card, header, heading, marker_font,
    marker_text, preview, quote, ui_rems,
};

pub const NAME: &str = "Cited Answer";
pub const DESCRIPTION: &str = "An answer beside the sources it rests on: superscript citations that wash their sentence and light their source when pointed at, and source cards that fold open on the quoted passage with the backing words marked, ready to copy or open";

const ICON_CHEVRON: &str = "icons/cited-answer/alt-arrow-right.svg";
const ICON_COPY: &str = "icons/cited-answer/copy.svg";
const ICON_COPIED: &str = "icons/cited-answer/copy-check.svg";
const ICON_OPEN: &str = "icons/cited-answer/arrow-right-up.svg";

/// Markdown body type and block gap (Pink: `MD_TEXT_SIZE`, `MD_LINE_HEIGHT`,
/// `MD_BLOCK_GAP`): 14 on a 22 line, one line looser than the kit's Prose.
const MD_TEXT_SIZE: f32 = text::PROSE;
const MD_LINE_HEIGHT: f32 = 22.;
const MD_BLOCK_GAP: f32 = space::S12;
/// Rounded match washes under shaped glyphs (Pink: `search_highlight`):
/// 1 px wider than the glyphs, 1.5 px in from the line box, nudged 0.5 px
/// down to sit on the glyphs' optical center, radius 3.
const WASH_PAD_X: f32 = 1.;
const WASH_INSET_Y: f32 = 1.5;
const WASH_NUDGE_Y: f32 = 0.5;
const WASH_RADIUS: f32 = 3.;
/// The source column: Pink's large menu width.
const PANEL_WIDTH: f32 = layout::MENU_W_L;
/// How long "Copied" shows (Pink: `copy_ui_for`).
const COPIED_FOR: Duration = Duration::from_millis(1200);

type SourceHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// A document the answer quotes.
#[derive(Clone)]
pub struct CitedSource {
    pub title: SharedString,
    pub site: SharedString,
    /// Freshness or date ("Updated Sep 2").
    pub date: SharedString,
    /// The passage the citations point into.
    pub passage: SharedString,
}

impl CitedSource {
    pub fn new(title: impl Into<SharedString>, site: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            site: site.into(),
            date: SharedString::default(),
            passage: SharedString::default(),
        }
    }

    pub fn date(mut self, date: impl Into<SharedString>) -> Self {
        self.date = date.into();
        self
    }

    pub fn passage(mut self, passage: impl Into<SharedString>) -> Self {
        self.passage = passage.into();
        self
    }

    /// "Help Center · Updated Sep 2".
    fn meta(&self) -> SharedString {
        if self.date.is_empty() {
            self.site.clone()
        } else {
            format!("{}, {}", self.site, self.date).into()
        }
    }
}

/// A pointer from a sentence into one source, with the phrases of its
/// passage that back the sentence.
#[derive(Clone)]
pub struct Citation {
    pub source: usize,
    pub marks: Vec<SharedString>,
}

impl Citation {
    pub fn new(source: usize) -> Self {
        Self {
            source,
            marks: Vec::new(),
        }
    }

    /// A phrase, exactly as it appears in the source passage.
    pub fn mark(mut self, phrase: impl Into<SharedString>) -> Self {
        self.marks.push(phrase.into());
        self
    }
}

/// One sentence of the answer and what it cites.
#[derive(Clone)]
pub struct Sentence {
    pub text: SharedString,
    pub cites: Vec<Citation>,
}

impl Sentence {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            cites: Vec::new(),
        }
    }

    pub fn cite(mut self, citation: Citation) -> Self {
        self.cites.push(citation);
        self
    }
}

#[derive(IntoElement)]
pub struct CitedAnswer {
    id: ElementId,
    question: SharedString,
    sources: Vec<CitedSource>,
    paragraphs: Vec<Vec<Sentence>>,
    default_open: Option<usize>,
    width: f32,
    on_open_source: Option<SourceHandler>,
}

impl CitedAnswer {
    pub fn new(id: impl Into<ElementId>, question: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            question: question.into(),
            sources: Vec::new(),
            paragraphs: Vec::new(),
            default_open: None,
            width: layout::COMPOSER_MAX_WIDTH,
            on_open_source: None,
        }
    }

    pub fn sources(mut self, sources: Vec<CitedSource>) -> Self {
        self.sources = sources;
        self
    }

    pub fn paragraph(mut self, sentences: Vec<Sentence>) -> Self {
        self.paragraphs.push(sentences);
        self
    }

    /// Folds open the source of the `n`th citation (counted through the
    /// whole answer), its marks shown, on first render.
    pub fn default_open(mut self, citation: usize) -> Self {
        self.default_open = Some(citation);
        self
    }

    /// The whole component's width; the answer takes what the source column
    /// leaves. Defaults to Pink's 768 px content column.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// "Open" in a source's meta lane: called with the source's index. The
    /// action shows only when a handler is set.
    pub fn on_open_source(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_source = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// What the source column has folded open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    /// From the column: the plain passage.
    Source(usize),
    /// From a citation (its index through the answer): the passage with the
    /// citation's marks.
    Cite(usize),
}

/// The source an open state shows. `cite_sources[c]` is citation `c`'s source.
fn source_of(open: Option<Open>, cite_sources: &[usize]) -> Option<usize> {
    match open? {
        Open::Source(ix) => Some(ix),
        Open::Cite(c) => cite_sources.get(c).copied(),
    }
}

struct State {
    /// The citation under the pointer.
    hover_cite: Option<usize>,
    /// The source header under the pointer.
    hover_source: Option<usize>,
    open: Option<Open>,
    /// When each source last folded open or shut (Pink: the tool fold's
    /// `toggled_at`), so chevron, height and body fade share one clock.
    toggled: Vec<Option<Instant>>,
    /// When the open citation's marks began revealing.
    marked_at: Option<Instant>,
    copied: Option<usize>,
    heights: Vec<Rc<Cell<f32>>>,
    seeded: bool,
    _reset: Option<Task<()>>,
}

impl State {
    fn set_open(&mut self, next: Option<Open>, cite_sources: &[usize]) {
        let now = Instant::now();
        let before = source_of(self.open, cite_sources);
        let after = source_of(next, cite_sources);
        if before != after {
            for ix in [before, after].into_iter().flatten() {
                if let Some(slot) = self.toggled.get_mut(ix) {
                    *slot = Some(now);
                }
            }
        }
        self.marked_at = matches!(next, Some(Open::Cite(_))).then_some(now);
        self.open = next;
    }
}

// ---------------------------------------------------------------------------
// Answer text: sentences → shaped runs (Pink: `flatten_runs`)
// ---------------------------------------------------------------------------

/// A citation's superscript number inside a paragraph.
#[derive(Clone, Debug, PartialEq)]
struct CiteHit {
    range: Range<usize>,
    /// Its position among all the answer's citations.
    ordinal: usize,
    source: usize,
}

/// One paragraph's flat text, runs, sentence spans and citations.
#[derive(Default)]
struct ParagraphFlat {
    text: String,
    runs: Vec<TextRun>,
    /// (sentence index through the answer, its span with its citations).
    sentences: Vec<(usize, Range<usize>)>,
    cites: Vec<CiteHit>,
}

fn sans() -> Font {
    gpui::font(pink::SANS)
}

/// Geist's superscript figures (the package's `marker_font`).
fn superscript() -> Font {
    marker_font(pink::SANS)
}

fn push_run(flat: &mut ParagraphFlat, s: &str, font: Font, color: Hsla) {
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

/// Flattens one paragraph: sentences joined by a space, each followed by its
/// citations as superscript numbers (thin-spaced when there are several).
/// `sentence_color(si)` and `cite_color(ordinal, source)` paint them.
fn flatten_paragraph(
    sentences: &[Sentence],
    first_sentence: usize,
    first_cite: usize,
    sentence_color: &dyn Fn(usize) -> Hsla,
    cite_color: &dyn Fn(usize, usize) -> Hsla,
) -> ParagraphFlat {
    let mut flat = ParagraphFlat::default();
    let mut ordinal = first_cite;
    for (k, sentence) in sentences.iter().enumerate() {
        let si = first_sentence + k;
        if k > 0 {
            push_run(&mut flat, " ", sans(), sentence_color(si));
        }
        let start = flat.text.len();
        push_run(&mut flat, &sentence.text, sans(), sentence_color(si));
        for (n, cite) in sentence.cites.iter().enumerate() {
            let color = cite_color(ordinal, cite.source);
            if n > 0 {
                push_run(&mut flat, CITE_GAP, superscript(), color);
            }
            let at = flat.text.len();
            push_run(
                &mut flat,
                &(cite.source + 1).to_string(),
                superscript(),
                color,
            );
            flat.cites.push(CiteHit {
                range: at..flat.text.len(),
                ordinal,
                source: cite.source,
            });
            ordinal += 1;
        }
        flat.sentences.push((si, start..flat.text.len()));
    }
    flat
}

/// The rectangles a byte range covers on its shaped lines, one per visual
/// row (Pink: `markdown/render.rs` `range_rects`).
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

/// A canvas painting rounded washes under `layout`'s glyphs; place it before
/// the text in a relative parent (Pink: `search_highlight`'s underlay).
fn wash_underlay(layout: TextLayout, washes: Vec<(Range<usize>, Hsla)>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for (range, color) in &washes {
                for mut rect in range_rects(&layout, range, WASH_PAD_X, WASH_INSET_Y) {
                    rect.origin.y += px(WASH_NUDGE_Y);
                    window.paint_quad(quad(
                        rect,
                        px(WASH_RADIUS),
                        *color,
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
    .size_full()
}

/// Byte ranges of each mark inside the passage.
fn mark_ranges(passage: &str, marks: &[SharedString]) -> Vec<Range<usize>> {
    marks
        .iter()
        .filter_map(|m| passage.find(m.as_ref()).map(|at| at..at + m.len()))
        .collect()
}

/// Records an element's laid-out height, refreshing once when it changes.
fn measure(cell: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            if (cell.get() - h).abs() > 0.5 {
                cell.set(h);
                window.on_next_frame(|window, _| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

// ---------------------------------------------------------------------------
// Pieces
// ---------------------------------------------------------------------------

/// Pink: `markdown/link_destination.rs` `destination_card` – where a citation
/// leads, shown while it is pointed at: `px 6 py 4`, radius 4, a
/// `border_strong` edge on the opaque tooltip surface, `shadow_sm`, 11/14.
struct CiteDestination(SharedString);

impl Render for CiteDestination {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        div()
            .px(px(6.))
            .py(px(4.))
            .rounded(px(radius::XS))
            .border_1()
            .border_color(t.border_strong)
            .bg(t.tooltip_surface())
            .shadow(pink::shadow_sm())
            .font_family(pink::SANS)
            .type_style(TextStyle::Caption)
            .text_color(t.text)
            .whitespace_nowrap()
            .child(self.0.clone())
    }
}

/// Pink: `transcript.rs` `meta_action`, drawn by the `pink-message-actions`
/// package: a 24 pt icon button with the shared hover fade to `ink(0.08)`,
/// a text tooltip, an accessible name and the inset focus ring. `live` is
/// false while its card is folded shut, so a hidden action is neither a tab
/// stop nor a tooltip.
fn meta_action(
    id: SharedString,
    icon: &'static str,
    label: SharedString,
    live: bool,
    t: &Pink,
) -> Stateful<Div> {
    use crate::components::message_actions as lane;
    let fade_key = SharedString::from(format!("{id}-fade"));
    let ring = t.focus_ring();
    lane::action(
        id,
        svg()
            .path(icon)
            .size(lane::ui_rems(lane::ACTION_ICON))
            .flex_none()
            .text_color(t.content_muted)
            .into_any_element(),
        motion::hover_blend(&fade_key, transparent_black(), t.ink(0.08)),
    )
    .flex_none()
    .role(Role::Button)
    .aria_label(label.clone())
    .when(live, |el| {
        el.tab_index(0)
            .tooltip(pink::text_tooltip(label))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
    })
    .on_hover(motion::hover_listener(fade_key))
    .focus_visible(move |style| pink::focus_ring(style, ring))
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl RenderOnce for CitedAnswer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let reduced = motion::reduced_motion(cx);
        let now = Instant::now();
        let source_count = self.sources.len();

        // Citations through the whole answer: (sentence, citation).
        let mut cites: Vec<(usize, &Citation)> = Vec::new();
        let mut sentence_sources: Vec<Vec<usize>> = Vec::new();
        for sentence in self.paragraphs.iter().flatten() {
            let si = sentence_sources.len();
            for c in &sentence.cites {
                cites.push((si, c));
            }
            sentence_sources.push(sentence.cites.iter().map(|c| c.source).collect());
        }
        let cite_sources: Rc<Vec<usize>> = Rc::new(cites.iter().map(|c| c.1.source).collect());

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _: &mut Context<State>| State {
                hover_cite: None,
                hover_source: None,
                open: None,
                toggled: Vec::new(),
                marked_at: None,
                copied: None,
                heights: Vec::new(),
                seeded: false,
                _reset: None,
            },
        );
        let default_open = self.default_open;
        state.update(cx, |s, _| {
            if !s.seeded {
                s.seeded = true;
                s.open = default_open.map(Open::Cite);
            }
            while s.heights.len() < source_count {
                s.heights.push(Rc::new(Cell::new(0.)));
            }
            s.toggled.resize(source_count.max(s.toggled.len()), None);
        });
        let s = state.read(cx);
        let (hover_cite, hover_source, open, copied, marked_at) =
            (s.hover_cite, s.hover_source, s.open, s.copied, s.marked_at);
        let toggled = s.toggled.clone();
        let heights = s.heights.clone();

        let open_cite = match open {
            Some(Open::Cite(c)) if c < cites.len() => Some(c),
            _ => None,
        };
        let open_source = source_of(open, &cite_sources).filter(|ix| *ix < source_count);
        // The source a pointed-at citation lights in the column.
        let hot_source = hover_cite.and_then(|c| cite_sources.get(c).copied());
        let pointing = hover_cite.is_some() || hover_source.is_some();

        // A sentence is lit while its citation is pointed at, while the
        // pointer is on a source it cites, or (at rest) while its citation
        // holds the column open.
        let lit_target = |si: usize| -> bool {
            if let Some(c) = hover_cite {
                cites.get(c).map(|c| c.0) == Some(si)
            } else if let Some(src) = hover_source {
                sentence_sources[si].contains(&src)
            } else if let Some(c) = open_cite {
                cites[c].0 == si
            } else {
                false
            }
        };
        let dim = motion::follow(&format!("{key}-dim"), pointing as u8 as f32, HOVER_FADE);
        let lit: Vec<f32> = (0..sentence_sources.len())
            .map(|si| {
                motion::follow(
                    &format!("{key}-lit-{si}"),
                    lit_target(si) as u8 as f32,
                    HOVER_FADE,
                )
            })
            .collect();
        let glow: Vec<f32> = (0..cites.len())
            .map(|c| {
                let src = cite_sources[c];
                let on = hover_cite == Some(c)
                    || open_cite == Some(c)
                    || hover_source == Some(src)
                    || open == Some(Open::Source(src));
                motion::follow(&format!("{key}-glow-{c}"), on as u8 as f32, HOVER_FADE)
            })
            .collect();

        // ---- answer --------------------------------------------------------
        let sentence_wash = t.tab_focused_fill();
        let marker = t.readable_tone(t.accent, 1.);
        let labels: Rc<Vec<SharedString>> = Rc::new(
            self.sources
                .iter()
                .map(|s| SharedString::from(format!("{}: {}", s.site, s.title)))
                .collect(),
        );
        let mut first_sentence = 0;
        let mut first_cite = 0;
        let paragraphs: Vec<AnyElement> = self
            .paragraphs
            .iter()
            .enumerate()
            .map(|(pix, sentences)| {
                let sentence_color =
                    |si: usize| motion::mix(t.text, t.content_muted, dim * (1. - lit[si]));
                let cite_color = |c: usize, _: usize| motion::mix(marker, t.text, glow[c]);
                let flat = flatten_paragraph(
                    sentences,
                    first_sentence,
                    first_cite,
                    &sentence_color,
                    &cite_color,
                );
                first_sentence += sentences.len();
                first_cite += flat.cites.len();
                paragraph(
                    pix,
                    flat,
                    &lit,
                    sentence_wash,
                    &labels,
                    &state,
                    &cite_sources,
                )
            })
            .collect();

        let total = sentence_sources.len();
        let backed = sentence_sources.iter().filter(|c| !c.is_empty()).count();
        let answer = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                // Pink: the question wizard's question, 15/20 medium.
                div()
                    .type_style(TextStyle::Title3)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(self.question.clone()),
            )
            .child(
                div()
                    .mt(px(space::S2))
                    .type_style(TextStyle::Footnote)
                    .font_features(pink::tabular_figures())
                    .text_color(t.content_faint)
                    .child(format!(
                        "{source_count} source{}, {backed} of {total} sentences cited",
                        if source_count == 1 { "" } else { "s" }
                    )),
            )
            .child(
                div()
                    .mt(px(space::S16))
                    .flex()
                    .flex_col()
                    .gap(px(MD_BLOCK_GAP))
                    .children(paragraphs),
            );

        // ---- sources -------------------------------------------------------
        let mut animating = false;
        let cards: Vec<AnyElement> = self
            .sources
            .iter()
            .enumerate()
            .map(|(ix, source)| {
                let is_open = open_source == Some(ix);
                let elapsed = toggled[ix]
                    .map(|at| now.saturating_duration_since(at))
                    .filter(|e| !reduced && *e < FOLD_TWEEN_WINDOW);
                animating |= elapsed.is_some();
                let (progress, body_alpha) = motion::fold_progress(is_open, elapsed);
                let marks: Vec<SharedString> = match open_cite {
                    Some(c) if is_open => cites[c].1.marks.clone(),
                    _ => Vec::new(),
                };
                let reveal = |n: usize| match marked_at {
                    Some(at) if !reduced => TOOL_ROW_REVEAL
                        .with_delay(TOOL_BODY_FADE.duration_ms + n as u64 * TOOL_ROW_STAGGER_MS)
                        .progress_at(now.saturating_duration_since(at)),
                    _ => 1.,
                };
                let mark_progress: Vec<f32> = (0..marks.len()).map(reveal).collect();
                animating |= mark_progress.iter().any(|p| *p < 1.);
                SourceCard {
                    key: &key,
                    ix,
                    source,
                    is_open,
                    hot: hot_source == Some(ix),
                    hovered: hover_source == Some(ix),
                    progress,
                    body_alpha,
                    marks,
                    mark_progress,
                    copied: copied == Some(ix),
                    height: heights[ix].clone(),
                    on_open: self.on_open_source.clone(),
                }
                .render(&t, &state, &cite_sources)
            })
            .collect();
        if animating {
            window.request_animation_frame();
        }

        let panel = div()
            .w(px(PANEL_WIDTH))
            .flex_none()
            .flex()
            .flex_col()
            .child(
                // Pink: `activity_center.rs` `activity_section_heading`, from
                // the package: the 11 px semibold name and its count.
                heading(
                    &HeadingStyle {
                        title: t.readable_faint(0.9),
                        count: t.readable_faint(0.7),
                    },
                    "Sources",
                    source_count.to_string(),
                )
                .font_features(pink::tabular_figures())
                .id("ca-sources-heading")
                .role(Role::Heading)
                .aria_label(SharedString::from(format!(
                    "Sources, {source_count} source{}",
                    if source_count == 1 { "" } else { "s" }
                ))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(ui_rems(CARD_GAP))
                    .children(cards),
            );

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .flex()
            .items_start()
            .gap(px(space::S32))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(answer)
            .child(panel)
    }
}

/// One paragraph: shaped runs at 14/22 over the lit sentences' washes, its
/// citations hoverable (lighting their sentence and source), clickable
/// (folding their source open) and previewing where they lead.
fn paragraph(
    pix: usize,
    flat: ParagraphFlat,
    lit: &[f32],
    wash: Hsla,
    labels: &Rc<Vec<SharedString>>,
    state: &gpui::Entity<State>,
    cite_sources: &Rc<Vec<usize>>,
) -> AnyElement {
    let washes: Vec<(Range<usize>, Hsla)> = flat
        .sentences
        .iter()
        .filter(|(si, _)| lit[*si] > 0.001)
        .map(|(si, range)| (range.clone(), wash.opacity(lit[*si])))
        .collect();
    let styled = StyledText::new(flat.text).with_runs(flat.runs);
    let underlay = wash_underlay(styled.layout().clone(), washes);
    let hits = Rc::new(flat.cites);
    let ranges = hits.iter().map(|h| h.range.clone()).collect();

    let hover_hits = hits.clone();
    let hover_state = state.clone();
    let tip_hits = hits.clone();
    let tip_labels = labels.clone();
    let click_hits = hits.clone();
    let click_state = state.clone();
    let click_sources = cite_sources.clone();
    let text = InteractiveText::new(
        ElementId::NamedInteger("ca-text".into(), pix as u64),
        styled,
    )
    .on_hover(move |ix, _, _, cx| {
        let hovered = ix.and_then(|i| {
            hover_hits
                .iter()
                .find(|h| h.range.contains(&i))
                .map(|h| h.ordinal)
        });
        hover_state.update(cx, |s, cx| {
            if s.hover_cite != hovered {
                s.hover_cite = hovered;
                cx.notify();
            }
        });
    })
    .tooltip(move |ix, _, cx| -> Option<AnyView> {
        let hit = tip_hits.iter().find(|h| h.range.contains(&ix))?;
        let label = tip_labels.get(hit.source)?.clone();
        Some(cx.new(|_| CiteDestination(label)).into())
    })
    .on_click(ranges, move |range_ix, _, cx| {
        let Some(ordinal) = click_hits.get(range_ix).map(|h| h.ordinal) else {
            return;
        };
        click_state.update(cx, |s, cx| {
            let next = if s.open == Some(Open::Cite(ordinal)) {
                None
            } else {
                Some(Open::Cite(ordinal))
            };
            s.set_open(next, &click_sources);
            cx.notify();
        });
    });

    let leave = state.clone();
    div()
        .id(ElementId::NamedInteger("ca-para".into(), pix as u64))
        .relative()
        .w_full()
        .text_size(px(MD_TEXT_SIZE))
        .line_height(px(MD_LINE_HEIGHT))
        .on_hover(move |hovered, _, cx| {
            if !*hovered {
                leave.update(cx, |s, cx| {
                    if s.hover_cite.take().is_some() {
                        cx.notify();
                    }
                });
            }
        })
        .child(underlay)
        .child(text)
        .into_any_element()
}

/// One source in the column.
struct SourceCard<'a> {
    key: &'a str,
    ix: usize,
    source: &'a CitedSource,
    is_open: bool,
    /// A citation of this source is pointed at.
    hot: bool,
    /// The pointer is on this source's header.
    hovered: bool,
    /// The fold: 0 shut, 1 open, and the body's fade.
    progress: f32,
    body_alpha: f32,
    marks: Vec<SharedString>,
    mark_progress: Vec<f32>,
    copied: bool,
    height: Rc<Cell<f32>>,
    on_open: Option<SourceHandler>,
}

impl SourceCard<'_> {
    /// Pink: `transcript.rs` – the expandable chip card (r10,
    /// `hairline(0.07)`, `ink(0.03)`) with a two-line header and a body that
    /// folds open under a 1 px `hairline(0.06)` rule, fading in with a 6 px
    /// settle.
    fn render(
        self,
        t: &Pink,
        state: &gpui::Entity<State>,
        cite_sources: &Rc<Vec<usize>>,
    ) -> AnyElement {
        let Self {
            key,
            ix,
            source,
            is_open,
            hot,
            progress,
            body_alpha,
            ..
        } = self;
        let hot_t = motion::follow(&format!("{key}-hot-{ix}"), hot as u8 as f32, HOVER_FADE);
        let lit = motion::follow(
            &format!("{key}-tile-{ix}"),
            (hot || is_open) as u8 as f32,
            HOVER_FADE,
        );
        let hover_t = motion::follow(
            &format!("{key}-head-{ix}"),
            self.hovered as u8 as f32,
            HOVER_FADE,
        );
        let ring = t.focus_ring();

        // The package's chip card: the 18 px tile (r6, `ink(0.08)`) holding
        // the citation number takes the accent wash while the source is in
        // play; the title brightens and the half-strength chevron comes up
        // under the pointer, turning 90 degrees with the fold.
        let style = CardStyle {
            border: motion::mix(t.hairline(0.07), t.accent.opacity(0.35), hot_t),
            background: t.ink(0.03),
            tile: motion::mix(t.ink(0.08), t.accent_wash, lit),
            tile_text: motion::mix(t.content_muted, t.readable_tone(t.accent, 1.), lit),
            title: motion::mix(t.text.opacity(0.85), t.text, hover_t),
            meta: t.content_faint,
            chevron: motion::mix(t.content_faint.opacity(0.5), t.text, hover_t),
        };

        let hover_state = state.clone();
        let toggle_state = state.clone();
        let toggle_sources = cite_sources.clone();
        let header = header(
            &style,
            Tile::Number((ix + 1).to_string().into()),
            source.title.clone(),
            source.meta(),
            Some((ICON_CHEVRON.into(), radians(FRAC_PI_2 * progress))),
        )
        .font_features(pink::tabular_figures())
        .id(ElementId::NamedInteger("ca-src-head".into(), ix as u64))
        .role(Role::Button)
        .aria_label(source.title.clone())
        .aria_expanded(is_open)
        .tab_index(0)
        .rounded(px(radius::SM))
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .cursor_pointer()
        .on_hover(move |hovered, _, cx| {
            hover_state.update(cx, |s, cx| {
                let next = if *hovered {
                    Some(ix)
                } else if s.hover_source == Some(ix) {
                    None
                } else {
                    s.hover_source
                };
                if s.hover_source != next {
                    s.hover_source = next;
                    cx.notify();
                }
            })
        })
        .on_click(move |_, _, cx| {
            toggle_state.update(cx, |s, cx| {
                let next = if source_of(s.open, &toggle_sources) == Some(ix) {
                    None
                } else {
                    Some(Open::Source(ix))
                };
                s.set_open(next, &toggle_sources);
                cx.notify();
            })
        });

        let body = self.body(t, state);
        let natural = self.height.get();
        card(&style)
            .id(ElementId::NamedInteger("ca-src".into(), ix as u64))
            .child(header)
            .child(
                div()
                    .flex_none()
                    .overflow_hidden()
                    .when(natural > 0. || !is_open, |d| d.h(px(natural * progress)))
                    .child(
                        div()
                            .relative()
                            .top(px(TOOL_BODY_RISE * (1. - body_alpha)))
                            .opacity(body_alpha)
                            .child(body),
                    ),
            )
            .into_any_element()
    }

    /// The folded part: the passage in Pink's blockquote (a 2 px accent@0.6
    /// rail on accent@0.05, right corners 6, `pl 12 pr 10 py 6`, muted text),
    /// set at Pink's 12/18 tool-detail size for the narrow column, with its
    /// marks in Pink's match wash, then the meta lane.
    fn body(&self, t: &Pink, state: &gpui::Entity<State>) -> Div {
        let ix = self.ix;
        let key = self.key;
        let passage = &self.source.passage;

        // Marked words: `code_text` over a rounded `code_wash`, each mark
        // arriving on its own beat.
        let mut ranges: Vec<(Range<usize>, f32)> = mark_ranges(passage, &self.marks)
            .into_iter()
            .zip(self.mark_progress.iter().copied())
            .collect();
        ranges.sort_by_key(|(r, _)| r.start);
        let mut runs = Vec::new();
        let mut at = 0;
        let run = |len: usize, color: Hsla| TextRun {
            len,
            font: sans(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        for (range, p) in &ranges {
            if range.start < at {
                continue;
            }
            if range.start > at {
                runs.push(run(range.start - at, t.content_muted));
            }
            runs.push(run(
                range.len(),
                motion::mix(t.content_muted, t.code_text, *p),
            ));
            at = range.end;
        }
        if at < passage.len() {
            runs.push(run(passage.len() - at, t.content_muted));
        }
        let styled = StyledText::new(passage.clone()).with_runs(runs);
        let washes = ranges
            .iter()
            .filter(|(_, p)| *p > 0.001)
            .map(|(r, p)| (r.clone(), t.code_wash.opacity(*p)))
            .collect();
        let quote = quote(
            &QuoteStyle {
                rule: t.hairline(0.06),
                rail: t.accent.opacity(0.6),
                background: t.accent.opacity(0.05),
                text: t.content_muted,
            },
            div()
                .relative()
                .child(wash_underlay(styled.layout().clone(), washes))
                .child(styled),
        );

        // Pink: the meta lane – `pt 4`, 24 px actions 2 px apart.
        let live = self.is_open;
        let copy_state = state.clone();
        let quoted = SharedString::from(format!(
            "“{}”\n{}, {}",
            passage, self.source.title, self.source.site
        ));
        let copy = meta_action(
            format!("{key}-copy-{ix}").into(),
            if self.copied { ICON_COPIED } else { ICON_COPY },
            if self.copied { "Copied" } else { "Copy quote" }.into(),
            live,
            t,
        )
        .when(live, |el| {
            el.on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(quoted.to_string()));
                let entity = copy_state.clone();
                copy_state.update(cx, |s, cx| {
                    s.copied = Some(ix);
                    s._reset = Some(cx.spawn(async move |_, cx| {
                        cx.background_executor().timer(COPIED_FOR).await;
                        let _ = entity.update(cx, |s, cx| {
                            s.copied = None;
                            cx.notify();
                        });
                    }));
                    cx.notify();
                });
            })
        });
        let open = self.on_open.clone().map(|handler| {
            meta_action(
                format!("{key}-goto-{ix}").into(),
                ICON_OPEN,
                format!("Open in {}", self.source.site).into(),
                live,
                t,
            )
            .when(live, |el| {
                el.on_click(move |_, window, cx| handler(ix, window, cx))
            })
        });

        div()
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .child(quote)
            .child(
                div()
                    .px(px(space::S4))
                    .pt(px(space::S4))
                    .pb(px(space::S4))
                    .flex()
                    .items_center()
                    .gap(px(space::S2))
                    .child(copy)
                    .children(open),
            )
            .child(measure(self.height.clone()))
    }
}

// ---------------------------------------------------------------------------
// Story: a support knowledge-base answer about refunds.
// ---------------------------------------------------------------------------

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        CitedAnswer::new("cited-answer", "Can customers on annual plans get a refund?")
            .sources(vec![
                CitedSource::new("Refunds and cancellations", "Help Center")
                    .date("Updated Sep 2")
                    .passage("Annual subscriptions can be refunded in full within 30 days of purchase or renewal. After 30 days we refund the unused months on a prorated basis, minus one month's fee. Monthly plans are not refunded but can be cancelled at any time."),
                CitedSource::new("Billing terms, section 4.2", "Legal")
                    .date("Effective Jan 1")
                    .passage("Add-ons bought during a subscription term, including extra seats and storage, are refundable under the same conditions as the plan they are attached to, measured from the add-on's own purchase date."),
                CitedSource::new("Issuing refunds", "Support playbook")
                    .date("Edited 3 days ago")
                    .passage("Agents can issue refunds of up to $5,000 directly from the billing console. Anything larger needs a finance approver on the ticket. Customers who pay by invoice receive a credit note rather than a card refund."),
            ])
            .paragraph(vec![
                Sentence::new("Yes. Annual plans get a full refund within 30 days of purchase or renewal.")
                    .cite(Citation::new(0).mark("refunded in full within 30 days of purchase or renewal")),
                Sentence::new("After that, the unused months are refunded pro rata, minus a one-month fee.")
                    .cite(
                        Citation::new(0)
                            .mark("refund the unused months on a prorated basis")
                            .mark("minus one month's fee"),
                    ),
            ])
            .paragraph(vec![
                Sentence::new("Add-ons such as extra seats follow the same rule, counted from the day they were bought.")
                    .cite(
                        Citation::new(1)
                            .mark("same conditions as the plan")
                            .mark("measured from the add-on's own purchase date"),
                    )
                    .cite(Citation::new(0).mark("refunded in full within 30 days")),
                Sentence::new("Support can refund up to $5,000 from the billing console; larger amounts need a finance approver.")
                    .cite(
                        Citation::new(2)
                            .mark("up to $5,000 directly from the billing console")
                            .mark("needs a finance approver"),
                    ),
                Sentence::new("Customers who pay by invoice get a credit note instead.")
                    .cite(Citation::new(2).mark("receive a credit note rather than a card refund")),
            ])
            .default_open(1)
            .on_open_source(|_, _, _| {})
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_found_in_passage() {
        let ranges = mark_ranges(
            "refund the unused months",
            &["unused months".into(), "absent".into()],
        );
        assert_eq!(ranges, vec![11..24]);
    }

    #[test]
    fn sentences_flatten_with_superscript_citations() {
        let sentences = vec![
            Sentence::new("One.").cite(Citation::new(0)),
            Sentence::new("Two.")
                .cite(Citation::new(1))
                .cite(Citation::new(2)),
        ];
        let color = |_: usize| Hsla::default();
        let cite = |_: usize, _: usize| Hsla::default();
        let flat = flatten_paragraph(&sentences, 3, 5, &color, &cite);
        assert_eq!(flat.text, format!("One.1 Two.2{CITE_GAP}3"));
        assert_eq!(flat.sentences[0], (3, 0..5));
        assert_eq!(flat.sentences[1].0, 4);
        assert_eq!(
            &flat.text[flat.sentences[1].1.clone()],
            format!("Two.2{CITE_GAP}3")
        );
        let ordinals: Vec<_> = flat.cites.iter().map(|c| (c.ordinal, c.source)).collect();
        assert_eq!(ordinals, [(5, 0), (6, 1), (7, 2)]);
        assert_eq!(&flat.text[flat.cites[2].range.clone()], "3");
        assert_eq!(
            flat.runs.iter().map(|r| r.len).sum::<usize>(),
            flat.text.len()
        );
    }

    #[test]
    fn open_state_names_its_source() {
        let cite_sources = [0, 0, 2];
        assert_eq!(source_of(Some(Open::Cite(2)), &cite_sources), Some(2));
        assert_eq!(source_of(Some(Open::Source(1)), &cite_sources), Some(1));
        assert_eq!(source_of(Some(Open::Cite(9)), &cite_sources), None);
        assert_eq!(source_of(None, &cite_sources), None);
    }
}
