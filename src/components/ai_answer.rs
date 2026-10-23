//! AI Answer – a luna component.
//!
//! A question, the sources the assistant gathered and its streamed reply in
//! one card. While it searches, monogram tiles drop into a stacked "sources"
//! pill and skeleton lines breathe where the answer will go. The reply then
//! writes itself word by word behind a blinking caret, with small numbered
//! citations that pop in place: point at one to preview its source, click it
//! to pin the preview. The pill unfolds into the full source list. When the
//! answer settles, copy, retry and rating buttons rise in under it, followed
//! by suggested follow-up questions.
//!
//! The component renders whatever it is given, so a caller streams by passing
//! the blocks received so far (or the whole answer plus `.reveal(words)`):
//!
//! ```ignore
//! AiAnswer::new("answer", "Why did checkout conversion drop?")
//!     .sources(sources)
//!     .blocks(vec![Block::paragraph(vec![
//!         Span::strong("Mobile Safari"), Span::text(" fell 11 points"), Span::cite(0), Span::text("."),
//!     ])])
//!     .status(AnswerStatus::Writing)
//!     .on_follow_up(|question, _, _| println!("{question}"))
//! ```

use std::{cell::Cell, f32::consts::TAU, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    Anchor, Animation, AnyElement, AnyView, App, ClipboardItem, Context, ElementId, Entity,
    FontWeight, Hsla, IntoElement, SharedString, Task, Transformation, Window, anchored, canvas,
    deferred, div, percentage, point, prelude::*, px, radians, svg,
};

use crate::{
    HoverFill,
    luna::{self, FillTier, Luna, Tone, radius, space, text},
    motion::{self, AnimationExt, EASE_OUT, GLIDE, MotionSpec, ROW_IN},
    story,
};

pub const NAME: &str = "AI Answer";
pub const DESCRIPTION: &str = "A streamed answer card: sources stack up while it searches, the reply writes itself word by word with numbered citations that preview their source, then copy, retry, rating and follow-up questions settle in underneath";

const ICON_COPY: &str = "icons/ai-answer/copy.svg";
const ICON_CHECK: &str = "icons/ai-answer/check.svg";
const ICON_RETRY: &str = "icons/ai-answer/retry.svg";
const ICON_UP: &str = "icons/ai-answer/up.svg";
const ICON_DOWN: &str = "icons/ai-answer/down.svg";
const ICON_CHEVRON: &str = "icons/ai-answer/chevron.svg";
const ICON_STOP: &str = "icons/ai-answer/stop.svg";
const ICON_OPEN: &str = "icons/ai-answer/open.svg";
const ICON_FOLLOW: &str = "icons/ai-answer/follow.svg";
const ICON_SPARK: &str = "icons/ai-answer/spark.svg";
const ICON_RING: &str = "icons/ai-answer/ring.svg";
const ICON_ARC: &str = "icons/ai-answer/arc.svg";

/// Body type: callout size on a relaxed line.
const BODY_SIZE: f32 = text::CALLOUT;
const BODY_LINE: f32 = 22.;
/// A word space at the body size.
const WORD_GAP: f32 = BODY_SIZE * 0.25;
/// Horizontal step between stacked source tiles.
const TILE_STEP: f32 = 15.;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type SourceHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type TextHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type VoteHandler = Rc<dyn Fn(Option<bool>, &mut Window, &mut App)>;

/// A document the answer draws on.
#[derive(Clone)]
pub struct Source {
    pub title: SharedString,
    /// Where it lives ("Linear", "docs.acme.dev").
    pub site: SharedString,
    /// Kind and freshness ("Issue · opened Thu").
    pub meta: SharedString,
    /// A passage shown in the citation preview.
    pub snippet: SharedString,
    /// One or two letters for the tile; the site's initial when unset.
    pub monogram: Option<SharedString>,
    pub tone: Tone,
}

impl Source {
    pub fn new(title: impl Into<SharedString>, site: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            site: site.into(),
            meta: SharedString::default(),
            snippet: SharedString::default(),
            monogram: None,
            tone: Tone::Neutral,
        }
    }

    pub fn meta(mut self, meta: impl Into<SharedString>) -> Self {
        self.meta = meta.into();
        self
    }

    pub fn snippet(mut self, snippet: impl Into<SharedString>) -> Self {
        self.snippet = snippet.into();
        self
    }

    pub fn monogram(mut self, letters: impl Into<SharedString>) -> Self {
        self.monogram = Some(letters.into());
        self
    }

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    fn letters(&self) -> SharedString {
        self.monogram.clone().unwrap_or_else(|| {
            self.site
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default()
                .into()
        })
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

/// Where the answer is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnswerStatus {
    Searching,
    Writing,
    Done { took: Duration },
    Stopped { after: Duration },
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
    let line = |spans: &[Span]| {
        spans
            .iter()
            .map(|s| match s {
                Span::Text(t) | Span::Strong(t) => t.to_string(),
                Span::Cite(ix) => format!("[{}]", ix + 1),
            })
            .collect::<String>()
    };
    blocks
        .iter()
        .map(|b| match b {
            Block::Paragraph(spans) => line(spans),
            Block::Bullets(items) => items
                .iter()
                .map(|s| format!("- {}", line(s)))
                .collect::<Vec<_>>()
                .join("\n"),
        })
        .collect::<Vec<_>>()
        .join("\n\n")
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

struct State {
    hover_cite: Option<usize>,
    pinned_cite: Option<usize>,
    sources_open: bool,
    copied: bool,
    vote: Option<bool>,
    list_height: Rc<Cell<f32>>,
    _copy_reset: Option<Task<()>>,
}

#[derive(IntoElement)]
pub struct AiAnswer {
    id: ElementId,
    question: SharedString,
    sources: Vec<Source>,
    blocks: Vec<Block>,
    follow_ups: Vec<SharedString>,
    status: AnswerStatus,
    reveal: Option<usize>,
    width: f32,
    on_stop: Option<Handler>,
    on_regenerate: Option<Handler>,
    on_follow_up: Option<TextHandler>,
    on_open_source: Option<SourceHandler>,
    on_feedback: Option<VoteHandler>,
}

impl AiAnswer {
    pub fn new(id: impl Into<ElementId>, question: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            question: question.into(),
            sources: Vec::new(),
            blocks: Vec::new(),
            follow_ups: Vec::new(),
            status: AnswerStatus::Writing,
            reveal: None,
            width: 640.,
            on_stop: None,
            on_regenerate: None,
            on_follow_up: None,
            on_open_source: None,
            on_feedback: None,
        }
    }

    pub fn sources(mut self, sources: Vec<Source>) -> Self {
        self.sources = sources;
        self
    }

    pub fn blocks(mut self, blocks: Vec<Block>) -> Self {
        self.blocks = blocks;
        self
    }

    pub fn follow_ups(mut self, questions: Vec<SharedString>) -> Self {
        self.follow_ups = questions;
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

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn on_stop(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_stop = Some(Rc::new(handler));
        self
    }

    pub fn on_regenerate(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_regenerate = Some(Rc::new(handler));
        self
    }

    pub fn on_follow_up(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_follow_up = Some(Rc::new(handler));
        self
    }

    /// Called with the source index from a preview or the source list.
    pub fn on_open_source(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_source = Some(Rc::new(handler));
        self
    }

    /// `Some(true)` for a good rating, `Some(false)` for a bad one, `None` when cleared.
    pub fn on_feedback(
        mut self,
        handler: impl Fn(Option<bool>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_feedback = Some(Rc::new(handler));
        self
    }
}

// ---------------------------------------------------------------------------
// Small pieces
// ---------------------------------------------------------------------------

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

fn spinner(id: impl Into<ElementId>, size: f32, color: Hsla, reduced: bool) -> AnyElement {
    let arc = svg().path(ICON_ARC).size(px(size)).text_color(color);
    div()
        .relative()
        .flex_none()
        .size(px(size))
        .child(
            svg()
                .path(ICON_RING)
                .absolute()
                .size(px(size))
                .text_color(color.opacity(0.22)),
        )
        .child(if reduced {
            arc.into_any_element()
        } else {
            arc.with_animation(
                id,
                Animation::new(Duration::from_millis(850)).repeat(),
                |s, d| s.with_transformation(Transformation::rotate(percentage(d))),
            )
            .into_any_element()
        })
        .into_any_element()
}

/// The source's letter tile; opaque so stacked tiles can overlap.
fn tile(source: &Source, size: f32, ring: Option<Hsla>, t: &Luna) -> gpui::Div {
    let color = source.tone.color(t);
    let wash = motion::mix(t.surface_card, color, if t.dark { 0.22 } else { 0.13 });
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .bg(wash)
        .when_some(ring, |d, ring| d.border_2().border_color(ring))
        .text_size(px((size * 0.45).round()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(source.letters())
}

/// A square icon button whose glyph can stay lit.
fn tool_button(
    id: impl Into<SharedString>,
    icon: &'static str,
    active: bool,
    t: &Luna,
) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    let key = format!("ans-tool-{id}");
    let rest_fg = if active { t.accent } else { t.text_faint };
    let tone = motion::hover_blend(&key, rest_fg, if active { t.accent } else { t.text });
    let (rest, hover) = if active {
        (t.accent_wash, t.accent_wash)
    } else {
        (
            t.fill(FillTier::Tertiary).opacity(0.),
            t.fill(FillTier::Tertiary),
        )
    };
    div()
        .id(id)
        .flex_none()
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::MD))
        .cursor_pointer()
        .hover_fill(key, rest, hover)
        .child(svg().path(icon).size(px(15.)).text_color(tone))
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

struct Ctx<'a> {
    t: &'a Luna,
    key: &'a str,
    reduced: bool,
    state: &'a Entity<State>,
    sources: &'a [Source],
    hover: Option<usize>,
    pinned: Option<usize>,
    on_open_source: Option<SourceHandler>,
}

impl AiAnswer {
    fn render_flow(
        cx: &Ctx,
        spans: &[Span],
        counter: &mut usize,
        cite_counter: &mut usize,
        limit: usize,
        caret: bool,
    ) -> gpui::Div {
        let t = cx.t;
        let mut groups: Vec<gpui::Div> = Vec::new();
        let mut group: Vec<AnyElement> = Vec::new();
        let mut caret_placed = false;
        let all = pieces(spans);
        let total = all.len();
        for (i, (piece, space)) in all.into_iter().enumerate() {
            let n = *counter;
            *counter += 1;
            let cite_ix = if let Piece::Cite(_) = piece {
                let c = *cite_counter;
                *cite_counter += 1;
                Some(c)
            } else {
                None
            };
            if n >= limit {
                continue;
            }
            let element = match piece {
                Piece::Word { text, strong } => div()
                    .flex_none()
                    .when(strong, |d| {
                        d.font_weight(FontWeight::SEMIBOLD).text_color(t.text)
                    })
                    .child(text)
                    .with_animation(
                        ElementId::NamedInteger("ans-w".into(), n as u64),
                        MotionSpec::new(200, EASE_OUT).animation(),
                        |el, d| el.opacity(0.2 + 0.8 * d),
                    )
                    .into_any_element(),
                Piece::Cite(source) => Self::citation(cx, cite_ix.unwrap_or(0), source, n),
            };
            group.push(element);
            let last_shown = n + 1 == limit || i + 1 == total;
            if caret && n + 1 == limit {
                group.push(Self::caret(cx));
                caret_placed = true;
            }
            if space || last_shown {
                groups.push(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .when(space, |d| d.pr(px(WORD_GAP)))
                        .children(std::mem::take(&mut group)),
                );
            }
        }
        if !group.is_empty() {
            groups.push(div().flex().flex_none().items_center().children(group));
        }
        if caret && !caret_placed && limit == usize::MAX {
            groups.push(div().flex().items_center().child(Self::caret(cx)));
        }
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .line_height(px(BODY_LINE))
            .children(groups)
    }

    fn caret(cx: &Ctx) -> AnyElement {
        let bar = div()
            .flex_none()
            .ml(px(2.))
            .w(px(2.))
            .h(px(15.))
            .rounded(px(1.))
            .bg(cx.t.accent);
        if cx.reduced {
            bar.into_any_element()
        } else {
            bar.with_animation(
                "ans-caret",
                Animation::new(Duration::from_millis(1000)).repeat(),
                |el, d| el.opacity(0.6 + 0.4 * (d * TAU).cos()),
            )
            .into_any_element()
        }
    }

    /// The numbered chip, its hover/pin state and the anchored preview.
    fn citation(cx: &Ctx, cite_ix: usize, source_ix: usize, n: usize) -> AnyElement {
        let t = cx.t;
        let lit = cx.hover == Some(cite_ix) || cx.pinned == Some(cite_ix);
        let open = lit && source_ix < cx.sources.len();
        let chip_key = format!("{}-cite-{cite_ix}", cx.key);
        let (rest_bg, rest_fg) = (t.fill(FillTier::Secondary), t.text_muted);
        let (lit_bg, lit_fg) = (t.accent_wash, if t.dark { t.text } else { t.accent });
        let glow = if lit { 1. } else { motion::hover_t(&chip_key) };
        let state = cx.state.clone();
        let state_hover = cx.state.clone();
        let chip = div()
            .id(ElementId::NamedInteger("ans-cite".into(), cite_ix as u64))
            .flex_none()
            .h(px(16.))
            .min_w(px(16.))
            .px(px(4.))
            .ml(px(2.))
            .mr(px(1.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(radius::XS))
            .text_size(px(10.5))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .cursor_pointer()
            .bg(motion::mix(rest_bg, lit_bg, glow))
            .text_color(motion::mix(rest_fg, lit_fg, glow))
            .on_hover(move |hovered, window, cx| {
                motion::set_hover(&chip_key, *hovered, motion::reduced_motion(cx));
                window.refresh();
                state_hover.update(cx, |s, cx| {
                    if *hovered {
                        s.hover_cite = Some(cite_ix);
                    } else if s.hover_cite == Some(cite_ix) {
                        s.hover_cite = None;
                    }
                    cx.notify();
                })
            })
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.pinned_cite = if s.pinned_cite == Some(cite_ix) {
                        None
                    } else {
                        Some(cite_ix)
                    };
                    cx.notify();
                })
            })
            .child(format!("{}", source_ix + 1));

        let popover = open.then(|| Self::preview(cx, cite_ix, source_ix));
        div()
            .relative()
            .flex_none()
            .child(chip)
            .children(popover)
            .with_animation(
                ElementId::NamedInteger("ans-cite-in".into(), n as u64),
                MotionSpec::new(320, motion::EASE_OUT_QUINT).animation(),
                |el, d| el.opacity(d).top(px(3. * (1. - d))),
            )
            .into_any_element()
    }

    fn preview(cx: &Ctx, cite_ix: usize, source_ix: usize) -> AnyElement {
        let t = cx.t;
        let source = &cx.sources[source_ix];
        let pinned = cx.pinned == Some(cite_ix);
        let state = cx.state.clone();
        let on_open = cx.on_open_source.clone();
        let card = luna::overlay(t)
            .id(ElementId::NamedInteger(
                "ans-preview".into(),
                cite_ix as u64,
            ))
            .w(px(300.))
            .p(px(space::S12))
            .flex()
            .flex_col()
            .gap(px(space::S6))
            .font_family(luna::SANS)
            .line_height(px(18.))
            .on_mouse_down_out(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    if s.pinned_cite == Some(cite_ix) {
                        s.pinned_cite = None;
                        cx.notify();
                    }
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .child(tile(source, 18., None, t))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_muted)
                            .child(format!("{} · {}", source.site, source.meta)),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(text::CAPTION))
                            .font_family(luna::MONO)
                            .text_color(t.text_faint)
                            .child(format!("{}", source_ix + 1)),
                    ),
            )
            .child(
                div()
                    .text_size(px(text::BODY))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(source.title.clone()),
            )
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_muted)
                    .line_clamp(3)
                    .child(source.snippet.clone()),
            )
            .when(pinned, |d| {
                d.child(
                    div().flex().pt(px(space::S4)).child(
                        luna::button(
                            format!("{}-open-{cite_ix}", cx.key),
                            luna::ButtonKind::Secondary,
                            luna::ControlSize::Sm,
                            t,
                        )
                        .child(
                            luna::glyph(ICON_OPEN, luna::ControlSize::Sm).text_color(t.text_muted),
                        )
                        .child(format!("Open in {}", source.site))
                        .when_some(on_open, |b, f| {
                            b.on_click(move |_, window, cx| f(source_ix, window, cx))
                        }),
                    ),
                )
            });
        div()
            .absolute()
            .top(px(20.))
            .left(px(-6.))
            .child(deferred(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .offset(point(px(0.), px(2.)))
                    .snap_to_window_with_margin(px(8.))
                    .child(motion::pop_in(
                        ElementId::NamedInteger("ans-pop".into(), cite_ix as u64),
                        card,
                    )),
            ))
            .into_any_element()
    }
}

impl RenderOnce for AiAnswer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let reduced = motion::reduced_motion(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _: &mut Context<State>| State {
                hover_cite: None,
                pinned_cite: None,
                sources_open: false,
                copied: false,
                vote: None,
                list_height: Rc::new(Cell::new(0.)),
                _copy_reset: None,
            },
        );
        let s = state.read(cx);
        let (hover, pinned, sources_open, copied, vote) = (
            s.hover_cite,
            s.pinned_cite,
            s.sources_open,
            s.copied,
            s.vote,
        );
        let list_height = s.list_height.clone();
        let busy = self.status.busy();
        let searching = self.status == AnswerStatus::Searching;
        let settled = !busy;

        // ---- sources pill + status -----------------------------------------
        let pill_key = format!("{key}-sources");
        let lit_source = hover.or(pinned).and_then(|c| {
            self.blocks
                .iter()
                .flat_map(|b| match b {
                    Block::Paragraph(s) => s.clone(),
                    Block::Bullets(items) => items.concat(),
                })
                .filter_map(|s| match s {
                    Span::Cite(ix) => Some(ix),
                    _ => None,
                })
                .nth(c)
        });
        let tiles = self.sources.iter().enumerate().map(|(ix, source)| {
            let ring = if lit_source == Some(ix) {
                t.accent
            } else {
                t.surface_card
            };
            div()
                .absolute()
                .top_0()
                .left(px(ix as f32 * TILE_STEP))
                .child(motion::rise_in(
                    ElementId::NamedInteger("ans-tile".into(), ix as u64),
                    MotionSpec::new(360, motion::EASE_OUT_QUINT),
                    -6.,
                    tile(source, 22., Some(ring), &t),
                ))
        });
        let count = self.sources.len();
        let stack_width = motion::follow(
            &format!("{key}-stack"),
            if count == 0 {
                0.
            } else {
                22. + TILE_STEP * (count - 1) as f32
            },
            GLIDE,
        );
        let pill_label: SharedString = if searching {
            match count {
                0 => "Searching your workspace".into(),
                1 => "Found 1 source".into(),
                n => format!("Found {n} sources").into(),
            }
        } else {
            match count {
                1 => "1 source".into(),
                n => format!("{n} sources").into(),
            }
        };
        let chevron_turn = motion::follow(
            &format!("{key}-chev"),
            if sources_open { 0.5 } else { 0. },
            GLIDE,
        );
        let pill = div()
            .id("ans-sources")
            .h(px(30.))
            .pl(px(4.))
            .pr(px(space::S8))
            .flex()
            .items_center()
            .gap(px(space::S8))
            .rounded(px(radius::MD))
            .cursor_pointer()
            .hover_fill(
                pill_key.clone(),
                t.fill(FillTier::Quaternary),
                t.fill(FillTier::Tertiary),
            )
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.sources_open = !s.sources_open;
                        cx.notify();
                    })
                }
            })
            .when(count > 0, |d| {
                d.child(
                    div()
                        .relative()
                        .flex_none()
                        .h(px(22.))
                        .w(px(stack_width))
                        .children(tiles),
                )
            })
            .when(count == 0, |d| d.pl(px(space::S8)))
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(motion::hover_blend(&pill_key, t.text_muted, t.text))
                    .child(pill_label),
            )
            .when(count > 0, |d| {
                d.child(
                    svg()
                        .path(ICON_CHEVRON)
                        .size(px(14.))
                        .text_color(t.text_faint)
                        .with_transformation(Transformation::rotate(radians(chevron_turn * TAU))),
                )
            });

        let status = match self.status {
            AnswerStatus::Searching | AnswerStatus::Writing => {
                let label = if searching { "Searching" } else { "Writing" };
                let on_stop = self.on_stop.clone();
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S8))
                    .child(spinner("ans-spin", 12., t.accent, reduced))
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_muted)
                            .child(label),
                    )
                    .child(
                        luna::button(
                            format!("{key}-stop"),
                            luna::ButtonKind::Secondary,
                            luna::ControlSize::Sm,
                            &t,
                        )
                        .child(
                            luna::glyph(ICON_STOP, luna::ControlSize::Sm).text_color(t.text_muted),
                        )
                        .child("Stop")
                        .when_some(on_stop, |b, f| {
                            b.on_click(move |_, window, cx| f(window, cx))
                        }),
                    )
                    .into_any_element()
            }
            AnswerStatus::Done { took } | AnswerStatus::Stopped { after: took } => {
                let stopped = matches!(self.status, AnswerStatus::Stopped { .. });
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(
                        svg()
                            .path(if stopped { ICON_STOP } else { ICON_SPARK })
                            .size(px(13.))
                            .text_color(if stopped { t.text_faint } else { t.accent }),
                    )
                    .child(if stopped {
                        format!("Stopped after {:.1}s", took.as_secs_f32())
                    } else {
                        format!("Answered in {:.1}s", took.as_secs_f32())
                    })
                    .into_any_element()
            }
        };

        // ---- source list (unfolds under the pill) --------------------------
        let on_open_source = self.on_open_source.clone();
        let rows = self.sources.iter().enumerate().map(|(ix, source)| {
            let row_key = format!("{key}-src-{ix}");
            let on_open = on_open_source.clone();
            div()
                .id(ElementId::NamedInteger("ans-src".into(), ix as u64))
                .h(px(44.))
                .px(px(space::S8))
                .flex()
                .items_center()
                .gap(px(space::S10))
                .rounded(px(radius::MD))
                .cursor_pointer()
                .hover_fill(
                    row_key.clone(),
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Tertiary),
                )
                .when_some(on_open, |d, f| {
                    d.on_click(move |_, window, cx| f(ix, window, cx))
                })
                .child(
                    div()
                        .w(px(14.))
                        .text_size(px(text::CAPTION))
                        .font_family(luna::MONO)
                        .text_color(t.text_faint)
                        .child(format!("{}", ix + 1)),
                )
                .child(tile(source, 24., None, &t))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::BODY))
                                .line_height(px(18.))
                                .text_color(t.text)
                                .child(source.title.clone()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(format!("{} · {}", source.site, source.meta)),
                        ),
                )
                .child(
                    svg()
                        .path(ICON_OPEN)
                        .size(px(14.))
                        .text_color(motion::hover_blend(
                            &row_key,
                            t.text_faint.opacity(0.),
                            t.text_muted,
                        )),
                )
        });
        let natural = list_height.get();
        let list_open = motion::follow(
            &format!("{key}-list"),
            if sources_open { 1. } else { 0. },
            GLIDE,
        );
        let source_list = div()
            .overflow_hidden()
            .h(px(natural * list_open))
            .opacity(list_open)
            .child(
                div()
                    .relative()
                    .flex_none()
                    .pt(px(space::S8))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .children(rows)
                    .child(measure(list_height)),
            );

        // ---- body -------------------------------------------------------------
        let ctx = Ctx {
            t: &t,
            key: &key,
            reduced,
            state: &state,
            sources: &self.sources,
            hover,
            pinned,
            on_open_source: self.on_open_source.clone(),
        };
        let limit = self.reveal.unwrap_or(usize::MAX);
        let streaming = self.status == AnswerStatus::Writing;
        let total_pieces = piece_count(&self.blocks);
        let frontier = limit.min(total_pieces);
        let mut counter = 0usize;
        let mut cites = 0usize;
        let mut blocks: Vec<AnyElement> = Vec::new();
        for block in &self.blocks {
            if counter >= limit {
                break;
            }
            match block {
                Block::Paragraph(spans) => {
                    let start = counter;
                    let len = pieces(spans).len();
                    let caret = streaming && frontier > start && frontier <= start + len;
                    blocks.push(
                        Self::render_flow(&ctx, spans, &mut counter, &mut cites, frontier, caret)
                            .into_any_element(),
                    );
                }
                Block::Bullets(items) => {
                    let mut list = div().flex().flex_col().gap(px(space::S4));
                    for spans in items {
                        if counter >= limit {
                            break;
                        }
                        let start = counter;
                        let len = pieces(spans).len();
                        let caret = streaming && frontier > start && frontier <= start + len;
                        let flow = Self::render_flow(
                            &ctx,
                            spans,
                            &mut counter,
                            &mut cites,
                            frontier,
                            caret,
                        );
                        list = list.child(
                            div()
                                .flex()
                                .gap(px(space::S8))
                                .child(
                                    div()
                                        .flex_none()
                                        .w(px(12.))
                                        .h(px(BODY_LINE))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(div().size(px(4.)).rounded_full().bg(t.text_faint)),
                                )
                                .child(div().flex_1().min_w_0().child(flow)),
                        );
                    }
                    blocks.push(list.into_any_element());
                }
            }
        }

        let skeleton = (searching || (streaming && frontier == 0)).then(|| {
            let bars = [0.94f32, 0.82, 0.58];
            div().flex().flex_col().gap(px(10.)).py(px(4.)).children(
                bars.into_iter().enumerate().map(|(i, w)| {
                    let bar = div()
                        .h(px(10.))
                        .w(gpui::relative(w))
                        .rounded(px(radius::XS))
                        .bg(t.fill(FillTier::Secondary));
                    if reduced {
                        bar.into_any_element()
                    } else {
                        bar.with_animation(
                            ElementId::NamedInteger("ans-skel".into(), i as u64),
                            Animation::new(Duration::from_millis(1400)).repeat(),
                            move |el, d| {
                                let phase = d * TAU - i as f32 * 0.9;
                                el.opacity(0.55 + 0.45 * phase.cos())
                            },
                        )
                        .into_any_element()
                    }
                }),
            )
        });

        // ---- actions + follow-ups ------------------------------------------
        let copy_text = plain_text(&self.blocks);
        let actions = settled.then(|| {
            let on_regenerate = self.on_regenerate.clone();
            let on_feedback = self.on_feedback.clone();
            let vote_button = |up: bool| {
                let state = state.clone();
                let on_feedback = on_feedback.clone();
                tool_button(
                    format!("{key}-{}", if up { "up" } else { "down" }),
                    if up { ICON_UP } else { ICON_DOWN },
                    vote == Some(up),
                    &t,
                )
                .on_click(move |_, window, cx| {
                    let next = state.update(cx, |s, cx| {
                        s.vote = if s.vote == Some(up) { None } else { Some(up) };
                        cx.notify();
                        s.vote
                    });
                    if let Some(f) = &on_feedback {
                        f(next, window, cx);
                    }
                })
            };
            let copy = tool_button(
                format!("{key}-copy"),
                if copied { ICON_CHECK } else { ICON_COPY },
                copied,
                &t,
            )
            .on_click({
                let state = state.clone();
                move |_, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
                    let entity = state.clone();
                    state.update(cx, |s, cx| {
                        s.copied = true;
                        cx.notify();
                        s._copy_reset = Some(cx.spawn(async move |_, cx| {
                            cx.background_executor()
                                .timer(Duration::from_millis(1400))
                                .await;
                            let _ = entity.update(cx, |s, cx| {
                                s.copied = false;
                                cx.notify();
                            });
                        }));
                    });
                }
            });
            let retry = tool_button(format!("{key}-retry"), ICON_RETRY, false, &t)
                .when_some(on_regenerate, |b, f| {
                    b.on_click(move |_, window, cx| f(window, cx))
                });
            let buttons: Vec<AnyElement> = vec![
                copy.into_any_element(),
                retry.into_any_element(),
                div()
                    .w(px(1.))
                    .h(px(14.))
                    .mx(px(space::S4))
                    .bg(t.border)
                    .into_any_element(),
                vote_button(true).into_any_element(),
                vote_button(false).into_any_element(),
            ];
            div()
                .flex()
                .items_center()
                .gap(px(2.))
                .mt(px(space::S2))
                .ml(px(-6.))
                .children(buttons.into_iter().enumerate().map(|(i, b)| {
                    motion::rise_in(
                        ElementId::NamedInteger("ans-act".into(), i as u64),
                        ROW_IN.with_delay(40 * i as u64),
                        4.,
                        div().child(b),
                    )
                }))
                .when(copied, |d| {
                    d.child(motion::fade_in(
                        "ans-copied",
                        div()
                            .ml(px(space::S6))
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child("Copied"),
                    ))
                })
        });

        let follow_ups =
            (settled && !self.follow_ups.is_empty()).then(|| {
                let on_follow_up = self.on_follow_up.clone();
                div()
                    .border_t_1()
                    .border_color(t.border)
                    .px(px(space::S20))
                    .pt(px(space::S12))
                    .pb(px(space::S16))
                    .flex()
                    .flex_col()
                    .gap(px(space::S8))
                    .child(motion::fade_in(
                        "ans-next-label",
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child("Ask next"),
                    ))
                    .child(div().flex().flex_wrap().gap(px(space::S6)).children(
                        self.follow_ups.iter().enumerate().map(|(i, q)| {
                            let fkey = format!("{key}-next-{i}");
                            let on_follow_up = on_follow_up.clone();
                            let q2 = q.clone();
                            motion::rise_in(
                                ElementId::NamedInteger("ans-next".into(), i as u64),
                                ROW_IN.with_delay(160 + 70 * i as u64),
                                5.,
                                div()
                                    .id(ElementId::NamedInteger("ans-next-btn".into(), i as u64))
                                    .h(px(30.))
                                    .pl(px(space::S8))
                                    .pr(px(space::S10))
                                    .flex()
                                    .items_center()
                                    .gap(px(space::S6))
                                    .rounded(px(radius::MD))
                                    .border_1()
                                    .border_color(t.border)
                                    .cursor_pointer()
                                    .text_size(px(text::BODY))
                                    .hover_fill_fg(
                                        fkey.clone(),
                                        t.fill(FillTier::Quaternary),
                                        t.fill(FillTier::Tertiary),
                                        t.text_muted,
                                        t.text,
                                    )
                                    .when_some(on_follow_up, |d, f| {
                                        d.on_click(move |_, window, cx| f(q2.clone(), window, cx))
                                    })
                                    .child(svg().path(ICON_FOLLOW).size(px(14.)).text_color(
                                        motion::hover_blend(&fkey, t.text_faint, t.accent),
                                    ))
                                    .child(q.clone()),
                            )
                        }),
                    ))
            });

        let body = div()
            .id(SharedString::from(format!("ans-body-{}", self.question)))
            .px(px(space::S20))
            .pt(px(space::S16))
            .pb(px(if settled { space::S12 } else { space::S20 }))
            .flex()
            .flex_col()
            .gap(px(space::S10))
            .text_size(px(BODY_SIZE))
            .line_height(px(BODY_LINE))
            .text_color(t.text)
            .children(skeleton)
            .children(blocks)
            .children(actions);

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .font_family(luna::SANS)
            .child(
                luna::card(&t)
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px(px(space::S20))
                            .pt(px(space::S16 + 2.))
                            .child(
                                div()
                                    .text_size(px(text::TITLE))
                                    .line_height(px(24.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(t.text)
                                    .child(self.question.clone()),
                            )
                            .child(
                                div()
                                    .mt(px(space::S10))
                                    .ml(px(-4.))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(space::S12))
                                    .child(pill)
                                    .child(status),
                            )
                            .child(source_list),
                    )
                    .child(
                        div()
                            .mx(px(space::S20))
                            .mt(px(space::S12))
                            .child(luna::divider(&t)),
                    )
                    .child(body)
                    .children(follow_ups),
            )
    }
}

// ---------------------------------------------------------------------------
// Story: a product analytics question with follow-ups that play their own
// answers.
// ---------------------------------------------------------------------------

struct Script {
    question: &'static str,
    sources: Vec<usize>,
    blocks: fn() -> Vec<Block>,
    follow_ups: [&'static str; 2],
}

fn library() -> Vec<Source> {
    vec![
        Source::new("Checkout funnel, week 41", "Amplitude")
            .meta("Dashboard · updated 2h ago")
            .snippet("Mobile Safari: cart → payment 61.2% (previous week 72.8%). Desktop and Android flat week over week.")
            .tone(Tone::Accent),
        Source::new("WEB-2214 Pay button hidden behind consent banner", "Linear")
            .meta("Issue · opened Thursday")
            .snippet("On iPhone SE and mini the consent banner covers the Apple Pay button until it is dismissed. Regression from 4.12.")
            .tone(Tone::Danger),
        Source::new("Release 4.12", "GitHub")
            .meta("Release · Oct 8")
            .snippet("New EU consent banner, sticky order summary on mobile, wallet buttons now lazy-load on the payment step.")
            .tone(Tone::Neutral),
        Source::new("#payments: drop-off on iOS", "Slack")
            .meta("Thread · 14 replies")
            .snippet("Support has 23 tickets since Wednesday saying they can't find the pay button. Every one of them is on iOS.")
            .tone(Tone::Warning),
        Source::new("Revenue by platform", "Looker")
            .meta("Report · daily")
            .snippet("iOS web revenue down €3.6k–€3.9k a day against the four-week baseline from Wed 9 Oct. Average order value unchanged.")
            .tone(Tone::Success),
    ]
}

const QUESTIONS: [&str; 3] = [
    "Why did checkout conversion drop last week?",
    "How much revenue did this cost us?",
    "Draft a status update for #payments",
];

fn scripts() -> Vec<Script> {
    vec![
        Script {
            question: QUESTIONS[0],
            sources: vec![0, 1, 2, 3],
            blocks: || {
                vec![
                    Block::paragraph(vec![
                        Span::strong("Mobile Safari"),
                        Span::text(
                            " accounts for almost all of it: its cart-to-payment rate fell from 72.8% to 61.2% while desktop and Android held steady",
                        ),
                        Span::cite(0),
                        Span::text("."),
                    ]),
                    Block::bullets(vec![
                        vec![
                            Span::strong("Cause:"),
                            Span::text(
                                " release 4.12 added a consent banner that covers the Apple Pay button on smaller iPhones",
                            ),
                            Span::cite(1),
                            Span::cite(2),
                            Span::text("."),
                        ],
                        vec![
                            Span::strong("Timing:"),
                            Span::text(
                                " the drop starts Wednesday morning, when 4.12 reached all iOS traffic",
                            ),
                            Span::cite(2),
                            Span::text("."),
                        ],
                        vec![
                            Span::strong("Support:"),
                            Span::text(
                                " 23 tickets say the pay button is missing, every one from iOS",
                            ),
                            Span::cite(3),
                            Span::text("."),
                        ],
                    ]),
                    Block::paragraph(vec![
                        Span::text(
                            "Collapsing the banner on the payment step is a quick win until the fix in WEB-2214 ships",
                        ),
                        Span::cite(1),
                        Span::text("."),
                    ]),
                ]
            },
            follow_ups: [QUESTIONS[1], QUESTIONS[2]],
        },
        Script {
            question: QUESTIONS[1],
            sources: vec![4, 0],
            blocks: || {
                vec![
                    Block::paragraph(vec![
                        Span::text("About "),
                        Span::strong("€18,700"),
                        Span::text(" over five days, all of it from iOS web"),
                        Span::cite(0),
                        Span::text("."),
                    ]),
                    Block::bullets(vec![
                        vec![
                            Span::strong("Volume:"),
                            Span::text(
                                " roughly 390 fewer orders, 78 a day below the four-week baseline",
                            ),
                            Span::cite(1),
                            Span::text("."),
                        ],
                        vec![
                            Span::strong("Basket:"),
                            Span::text(
                                " average order value held at €47.90, so this is lost checkouts, not smaller ones",
                            ),
                            Span::cite(0),
                            Span::text("."),
                        ],
                    ]),
                    Block::paragraph(vec![Span::text(
                        "Most of it comes back once the banner stops covering the button, ideally before the weekend peak.",
                    )]),
                ]
            },
            follow_ups: [QUESTIONS[2], QUESTIONS[0]],
        },
        Script {
            question: QUESTIONS[2],
            sources: vec![1, 0, 4],
            blocks: || {
                vec![
                    Block::paragraph(vec![Span::text("Here is a short update you can post:")]),
                    Block::bullets(vec![
                        vec![
                            Span::strong("What happened:"),
                            Span::text(
                                " since Wednesday a new consent banner hid the Apple Pay button on smaller iPhones",
                            ),
                            Span::cite(0),
                            Span::text("."),
                        ],
                        vec![
                            Span::strong("Impact:"),
                            Span::text(
                                " mobile Safari checkout fell 11.6 points, about €18.7k in lost orders",
                            ),
                            Span::cite(1),
                            Span::cite(2),
                            Span::text("."),
                        ],
                        vec![
                            Span::strong("Next:"),
                            Span::text(
                                " the banner collapses on the payment step today; the full fix ships with WEB-2214",
                            ),
                            Span::cite(0),
                            Span::text("."),
                        ],
                    ]),
                ]
            },
            follow_ups: [QUESTIONS[0], QUESTIONS[1]],
        },
    ]
}

/// Seconds spent searching; sources land evenly across it.
const SEARCH_SECS: f32 = 1.8;
/// Words and citations streamed per second.
const PIECES_PER_SEC: f32 = 26.;

struct Demo {
    script: usize,
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
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        Demo {
            script: 0,
            started: Instant::now(),
            stopped: None,
            _ticker: ticker,
        }
    }

    fn play(&mut self, script: usize) {
        self.script = script;
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
        let scripts = scripts();
        let script = &scripts[d.script];
        let library = library();
        let at = d.at();
        let all_sources: Vec<Source> = script
            .sources
            .iter()
            .map(|&ix| library[ix].clone())
            .collect();
        let blocks = (script.blocks)();
        let total = piece_count(&blocks);
        let shown = (((at - SEARCH_SECS).max(0.)) * PIECES_PER_SEC) as usize;
        let write_secs = total as f32 / PIECES_PER_SEC;

        let (status, found) = if let Some(after) = d.stopped {
            (
                AnswerStatus::Stopped {
                    after: Duration::from_secs_f32(after),
                },
                all_sources.len(),
            )
        } else if at < SEARCH_SECS {
            let per = SEARCH_SECS / (all_sources.len() as f32 + 1.);
            (
                AnswerStatus::Searching,
                ((at / per) as usize).min(all_sources.len()),
            )
        } else if shown < total {
            (AnswerStatus::Writing, all_sources.len())
        } else {
            (
                AnswerStatus::Done {
                    took: Duration::from_secs_f32(SEARCH_SECS + write_secs),
                },
                all_sources.len(),
            )
        };

        let stop = demo.clone();
        let regen = demo.clone();
        let follow = demo.clone();
        AiAnswer::new("ai-answer", script.question)
            .sources(all_sources.into_iter().take(found).collect())
            .blocks(blocks)
            .reveal(if status == AnswerStatus::Searching {
                0
            } else {
                shown
            })
            .status(status)
            .follow_ups(
                script
                    .follow_ups
                    .iter()
                    .map(|q| SharedString::from(*q))
                    .collect(),
            )
            .on_stop(move |_, cx| {
                stop.update(cx, |d, cx| {
                    d.stopped = Some(d.at());
                    cx.notify();
                })
            })
            .on_regenerate(move |_, cx| {
                regen.update(cx, |d, cx| {
                    let s = d.script;
                    d.play(s);
                    cx.notify();
                })
            })
            .on_follow_up(move |question, _, cx| {
                follow.update(cx, |d, cx| {
                    if let Some(ix) = QUESTIONS.iter().position(|q| *q == question.as_ref()) {
                        d.play(ix);
                        cx.notify();
                    }
                })
            })
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
    }
}
