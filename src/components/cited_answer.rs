//! Cited Answer – an original component in luna style.
//!
//! An answer on the left and the evidence on the right. Every sentence ends
//! in numbered citations. Point at one and its sentence takes a soft accent
//! wash while the rest of the answer steps back, and the source it cites
//! lights up in the side panel. Click it and that source unfolds to the
//! quoted passage, with the words that back the sentence marked one after
//! another. Sources can also be opened from the panel, copied as a quote or
//! followed to the original.
//!
//! ```ignore
//! CitedAnswer::new("refunds", "Can customers on annual plans get a refund?")
//!     .sources(vec![CitedSource::new("Refunds and cancellations", "Help Center").passage("…")])
//!     .paragraph(vec![
//!         Sentence::new("Annual plans get a full refund within 30 days.")
//!             .cite(Citation::new(0).mark("refunded in full within 30 days")),
//!     ])
//!     .default_open(0)
//! ```

use std::{cell::Cell, f32::consts::TAU, ops::Range, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AnyView, App, ClipboardItem, Context, ElementId, Entity, FontWeight, Hsla,
    IntoElement, SharedString, Task, Transformation, Window, canvas, div, prelude::*, px, radians,
    svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, AnimationExt, EASE, EASE_OUT, GLIDE, MotionSpec},
    story,
};

pub const NAME: &str = "Cited Answer";
pub const DESCRIPTION: &str = "An answer wired to its evidence: point at a citation to light its sentence and its source, click it to unfold the quoted passage beside the answer with the supporting words marked";

const ICON_CHEVRON: &str = "icons/cited-answer/chevron.svg";
const ICON_COPY: &str = "icons/cited-answer/copy.svg";
const ICON_CHECK: &str = "icons/cited-answer/check.svg";
const ICON_OPEN: &str = "icons/cited-answer/open.svg";
const ICON_GROUNDED: &str = "icons/cited-answer/grounded.svg";

const BODY_SIZE: f32 = text::CALLOUT;
const BODY_LINE: f32 = 23.;
const WORD_GAP: f32 = BODY_SIZE * 0.25;
const QUOTE_SIZE: f32 = 12.5;
const QUOTE_GAP: f32 = QUOTE_SIZE * 0.25;
/// Sentence and source lighting: a quick ease, like a hover.
const LIGHT: MotionSpec = MotionSpec::new(200, EASE);

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
    pub tone: Tone,
}

impl CitedSource {
    pub fn new(title: impl Into<SharedString>, site: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            site: site.into(),
            date: SharedString::default(),
            passage: SharedString::default(),
            tone: Tone::Neutral,
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

    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
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

/// What the side panel has unfolded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Open {
    /// From the panel: the plain passage.
    Source(usize),
    /// From a citation (global index): the passage with its marks.
    Cite(usize),
}

struct State {
    hover: Option<usize>,
    open: Option<Open>,
    copied: Option<usize>,
    heights: Vec<Rc<Cell<f32>>>,
    seeded: bool,
    _reset: Option<Task<()>>,
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
            width: 804.,
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

    /// Unfolds the passage of the `n`th citation (counted through the whole
    /// answer) on first render.
    pub fn default_open(mut self, citation: usize) -> Self {
        self.default_open = Some(citation);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// "Open" on a source: called with its index.
    pub fn on_open_source(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_source = Some(Rc::new(handler));
        self
    }
}

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

fn tile(source: &CitedSource, size: f32, t: &Luna) -> gpui::Div {
    let color = source.tone.color(t);
    div()
        .flex_none()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .bg(color.opacity(if t.dark { 0.2 } else { 0.12 }))
        .text_size(px((size * 0.46).round()))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(
            source
                .site
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_default(),
        )
}

/// Words of `text` with their byte ranges.
fn words(text: &str) -> Vec<(Range<usize>, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, ch) in text.char_indices() {
        if ch == ' ' {
            if let Some(s) = start.take() {
                out.push((s..i, &text[s..i]));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        out.push((s..text.len(), &text[s..]));
    }
    out
}

/// Byte ranges of each mark inside the passage.
fn mark_ranges(passage: &str, marks: &[SharedString]) -> Vec<Range<usize>> {
    marks
        .iter()
        .filter_map(|m| passage.find(m.as_ref()).map(|at| at..at + m.len()))
        .collect()
}

/// The highlight wash for a lit sentence or marked words.
fn wash(t: &Luna) -> Hsla {
    t.accent.opacity(if t.dark { 0.2 } else { 0.11 })
}

impl RenderOnce for CitedAnswer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let source_count = self.sources.len();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _: &mut Context<State>| State {
                hover: None,
                open: None,
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
        });
        let s = state.read(cx);
        let (hover, open, copied) = (s.hover, s.open, s.copied);
        let heights = s.heights.clone();

        // Flatten the citations: (sentence index, citation).
        let mut cites: Vec<(usize, Citation)> = Vec::new();
        let mut sentence_ix = 0;
        for paragraph in &self.paragraphs {
            for sentence in paragraph {
                for c in &sentence.cites {
                    cites.push((sentence_ix, c.clone()));
                }
                sentence_ix += 1;
            }
        }
        let open_cite = match open {
            Some(Open::Cite(c)) => Some(c),
            _ => None,
        };
        let lit_sentence = hover.or(open_cite).and_then(|c| cites.get(c)).map(|c| c.0);
        let hovered_source = hover.and_then(|c| cites.get(c)).map(|c| c.1.source);
        let open_source = match open {
            Some(Open::Source(s)) => Some(s),
            Some(Open::Cite(c)) => cites.get(c).map(|c| c.1.source),
            None => None,
        };
        let dim = motion::follow(
            &format!("{key}-dim"),
            if hover.is_some() { 1. } else { 0. },
            LIGHT,
        );

        let sentence_cited: Vec<bool> = self
            .paragraphs
            .iter()
            .flatten()
            .map(|s| !s.cites.is_empty())
            .collect();

        // ---- answer column -------------------------------------------------
        let mut sentence_ix = 0;
        let mut cite_ix = 0;
        let paragraphs: Vec<AnyElement> = self
            .paragraphs
            .iter()
            .map(|paragraph| {
                let mut groups: Vec<AnyElement> = Vec::new();
                let count = paragraph.len();
                for (k, sentence) in paragraph.iter().enumerate() {
                    let si = sentence_ix;
                    sentence_ix += 1;
                    let lit = motion::follow(
                        &format!("{key}-lit-{si}"),
                        if lit_sentence == Some(si) { 1. } else { 0. },
                        LIGHT,
                    );
                    let ink = motion::mix(t.text, t.text_faint, dim * (1. - lit));
                    let bg = wash(&t).opacity(lit);
                    let ws = words(&sentence.text);
                    let last_word = ws.len().saturating_sub(1);
                    let last_sentence = k + 1 == count;
                    for (w, (_, word)) in ws.iter().enumerate() {
                        let first = w == 0;
                        if w < last_word {
                            groups.push(
                                div()
                                    .flex_none()
                                    .pr(px(WORD_GAP))
                                    .bg(bg)
                                    .when(first, |d| {
                                        d.rounded_l(px(radius::XS)).pl(px(2.)).ml(px(-2.))
                                    })
                                    .text_color(ink)
                                    .child(SharedString::from(word.to_string()))
                                    .into_any_element(),
                            );
                            continue;
                        }
                        // The last word carries the citation chips.
                        let chips = sentence.cites.iter().map(|c| {
                            let ix = cite_ix;
                            cite_ix += 1;
                            let is_lit = hover == Some(ix) || open_cite == Some(ix);
                            citation_chip(&t, &key, &state, ix, c.source, is_lit)
                        });
                        let chips: Vec<AnyElement> = chips.collect();
                        groups.push(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .bg(bg)
                                .rounded_r(px(radius::XS))
                                .when(first, |d| d.rounded_l(px(radius::XS)))
                                .pr(px(2.))
                                .text_color(ink)
                                .child(SharedString::from(word.to_string()))
                                .children(chips)
                                .into_any_element(),
                        );
                        if !last_sentence {
                            groups.push(
                                div()
                                    .flex_none()
                                    .w(px(WORD_GAP + 2.))
                                    .h(px(BODY_LINE))
                                    .into_any_element(),
                            );
                        }
                    }
                }
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .text_size(px(BODY_SIZE))
                    .line_height(px(BODY_LINE))
                    .children(groups)
                    .into_any_element()
            })
            .collect();

        let answer = div()
            .flex_1()
            .min_w_0()
            .px(px(space::S24))
            .pt(px(space::S20))
            .pb(px(space::S24))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .text_size(px(text::FOOTNOTE))
                    .child(svg().path(ICON_GROUNDED).size(px(14.)).text_color(t.accent))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text_muted)
                            .child("Answer"),
                    )
                    .child(div().text_color(t.text_faint).child(format!(
                        "· grounded in {source_count} source{}",
                        if source_count == 1 { "" } else { "s" }
                    ))),
            )
            .child(
                div()
                    .mt(px(space::S8))
                    .mb(px(space::S12))
                    .text_size(px(text::TITLE))
                    .line_height(px(24.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(self.question.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S12))
                    .children(paragraphs),
            )
            .child(coverage(&t, &key, &sentence_cited, lit_sentence));

        // ---- source panel --------------------------------------------------
        let rows = self.sources.iter().enumerate().map(|(ix, source)| {
            let is_open = open_source == Some(ix);
            let is_hot = hovered_source == Some(ix);
            let row_key = format!("{key}-row-{ix}");
            let openness = motion::follow(
                &format!("{key}-open-{ix}"),
                if is_open { 1. } else { 0. },
                GLIDE,
            );
            let hot = motion::follow(
                &format!("{key}-hot-{ix}"),
                if is_hot { 1. } else { 0. },
                LIGHT,
            );
            let raised = openness.max(hot);
            let marks: Vec<SharedString> = match open {
                Some(Open::Cite(c)) if is_open => {
                    cites.get(c).map(|c| c.1.marks.clone()).unwrap_or_default()
                }
                _ => Vec::new(),
            };
            let passage_key = match open {
                Some(Open::Cite(c)) if is_open => c + 1,
                _ => 0,
            };

            let toggle_state = state.clone();
            let head = div()
                .id(ElementId::NamedInteger("ca-row".into(), ix as u64))
                .h(px(52.))
                .px(px(space::S10))
                .flex()
                .items_center()
                .gap(px(space::S10))
                .cursor_pointer()
                .rounded(px(radius::MD))
                .hover_fill(
                    row_key.clone(),
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Quaternary),
                )
                .on_click(move |_, _, cx| {
                    toggle_state.update(cx, |s, cx| {
                        s.open = if is_open {
                            None
                        } else {
                            Some(Open::Source(ix))
                        };
                        cx.notify();
                    })
                })
                .child(number_chip(&t, ix, hot.max(openness)))
                .child(tile(source, 26., &t))
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
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(source.title.clone()),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .line_height(px(16.))
                                .text_color(t.text_faint)
                                .child(format!("{} · {}", source.site, source.date)),
                        ),
                )
                .child(
                    svg()
                        .path(ICON_CHEVRON)
                        .size(px(14.))
                        .flex_none()
                        .text_color(motion::hover_blend(&row_key, t.text_faint, t.text_muted))
                        .with_transformation(Transformation::rotate(radians(openness * TAU / 2.))),
                );

            // The passage, always laid out so its height is known before it opens.
            let ranges = mark_ranges(&source.passage, &marks);
            let mut mark_n = 0u64;
            let passage_words = words(&source.passage);
            let last = passage_words.len().saturating_sub(1);
            let quote_groups = passage_words
                .into_iter()
                .enumerate()
                .map(|(w, (range, word))| {
                    let marked = ranges
                        .iter()
                        .any(|m| m.start < range.end && range.start < m.end);
                    // Marks join across the gap when the next word is marked too.
                    let joins = marked
                        && ranges
                            .iter()
                            .any(|m| m.start <= range.end + 1 && range.end + 1 < m.end);
                    let el = div()
                        .flex_none()
                        .when(w < last, |d| d.pr(px(QUOTE_GAP)))
                        .text_color(if marked { t.text } else { t.text_muted })
                        .child(SharedString::from(word.to_string()));
                    if marked {
                        let n = mark_n;
                        mark_n += 1;
                        let wash = wash(&t);
                        el.when(!joins, |d| d.pr(px(0.)).mr(px(QUOTE_GAP)))
                            .with_animation(
                                ElementId::NamedInteger(format!("ca-mark-{passage_key}").into(), n),
                                MotionSpec::new(360, EASE_OUT)
                                    .with_delay(220 + 45 * n)
                                    .animation(),
                                move |el, d| el.bg(wash.opacity(d)),
                            )
                            .into_any_element()
                    } else {
                        el.into_any_element()
                    }
                });
            let copy_state = state.clone();
            let passage_text = source.passage.clone();
            let on_open = self.on_open_source.clone();
            let is_copied = copied == Some(ix);
            let body = div()
                .relative()
                .flex_none()
                .px(px(space::S10))
                .pb(px(space::S10))
                .child(
                    div()
                        .flex()
                        .gap(px(space::S10))
                        .child(
                            div()
                                .flex_none()
                                .w(px(2.))
                                .rounded(px(1.))
                                .bg(source.tone.color(&t).opacity(0.55)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_wrap()
                                .text_size(px(QUOTE_SIZE))
                                .line_height(px(19.))
                                .children(quote_groups),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(space::S6))
                        .mt(px(space::S10))
                        .child(
                            luna::button(
                                format!("{key}-copy-{ix}"),
                                ButtonKind::Secondary,
                                ControlSize::Sm,
                                &t,
                            )
                            .child(
                                luna::glyph(
                                    if is_copied { ICON_CHECK } else { ICON_COPY },
                                    ControlSize::Sm,
                                )
                                .text_color(if is_copied {
                                    t.success
                                } else {
                                    t.text_muted
                                }),
                            )
                            .child(if is_copied { "Copied" } else { "Copy quote" })
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(format!(
                                    "“{passage_text}”"
                                )));
                                let entity = copy_state.clone();
                                copy_state.update(cx, |s, cx| {
                                    s.copied = Some(ix);
                                    cx.notify();
                                    s._reset = Some(cx.spawn(async move |_, cx| {
                                        cx.background_executor()
                                            .timer(Duration::from_millis(1400))
                                            .await;
                                        let _ = entity.update(cx, |s, cx| {
                                            s.copied = None;
                                            cx.notify();
                                        });
                                    }));
                                });
                            }),
                        )
                        .child(
                            luna::button(
                                format!("{key}-goto-{ix}"),
                                ButtonKind::Ghost,
                                ControlSize::Sm,
                                &t,
                            )
                            .child(luna::glyph(ICON_OPEN, ControlSize::Sm).text_color(t.text_muted))
                            .child(format!("Open in {}", source.site))
                            .when_some(on_open, |b, f| {
                                b.on_click(move |_, window, cx| f(ix, window, cx))
                            }),
                        ),
                )
                .child(measure(heights[ix].clone()));
            let natural = heights[ix].get();

            div()
                .id(ElementId::NamedInteger("ca-src".into(), ix as u64))
                .flex()
                .flex_col()
                .rounded(px(radius::LG))
                .border_1()
                .border_color(motion::mix(
                    t.border.opacity(0.),
                    if is_hot {
                        t.accent.opacity(0.55)
                    } else {
                        t.border
                    },
                    raised,
                ))
                .bg(motion::mix(
                    t.surface_card.opacity(0.),
                    t.surface_card,
                    raised,
                ))
                .when(raised > 0.5, |d| d.shadow(t.card_shadow()))
                .child(head)
                .child(
                    div()
                        .overflow_hidden()
                        .when(natural > 0. || !is_open, |d| d.h(px(natural * openness)))
                        .opacity(openness)
                        .child(div().id(("ca-passage", passage_key)).child(body)),
                )
        });

        let panel = div()
            .w(px(312.))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(space::S4))
            .p(px(space::S10))
            .border_l_1()
            .border_color(t.border)
            .bg(t.fill(FillTier::Quaternary))
            .rounded_r(px(radius::LG - 1.))
            .child(
                div()
                    .px(px(space::S10))
                    .pt(px(space::S6))
                    .pb(px(space::S6))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(text::FOOTNOTE))
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text_muted)
                            .child("Sources"),
                    )
                    .when(open_source.is_none(), |d| {
                        d.child(
                            div()
                                .text_color(t.text_faint)
                                .child("Click a number to read"),
                        )
                    }),
            )
            .children(rows);

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .font_family(luna::SANS)
            .child(luna::card(&t).flex().child(answer).child(panel))
    }
}

/// One tick per sentence: cited ones in the accent, the lit one solid.
fn coverage(t: &Luna, key: &str, cited: &[bool], lit: Option<usize>) -> impl IntoElement {
    let total = cited.len();
    let backed = cited.iter().filter(|c| **c).count();
    div()
        .mt_auto()
        .pt(px(space::S20))
        .flex()
        .items_center()
        .gap(px(space::S10))
        .child(
            div()
                .flex()
                .gap(px(3.))
                .children(cited.iter().enumerate().map(|(i, &c)| {
                    let on = motion::follow(
                        &format!("{key}-tick-{i}"),
                        if lit == Some(i) { 1. } else { 0. },
                        LIGHT,
                    );
                    let rest = if c {
                        t.accent.opacity(0.35)
                    } else {
                        t.fill(FillTier::Primary)
                    };
                    div()
                        .w(px(18.))
                        .h(px(4.))
                        .rounded(px(2.))
                        .bg(motion::mix(rest, t.accent, on))
                })),
        )
        .child(
            div()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child(format!("{backed} of {total} sentences cited")),
        )
}

/// The source number in the panel, lit while its source is in play.
fn number_chip(t: &Luna, ix: usize, lit: f32) -> impl IntoElement {
    div()
        .flex_none()
        .h(px(16.))
        .min_w(px(16.))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::XS))
        .bg(motion::mix(t.fill(FillTier::Secondary), t.accent_wash, lit))
        .text_size(px(10.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(motion::mix(
            t.text_muted,
            if t.dark { t.text } else { t.accent },
            lit,
        ))
        .child(format!("{}", ix + 1))
}

fn citation_chip(
    t: &Luna,
    key: &str,
    state: &Entity<State>,
    ix: usize,
    source: usize,
    lit: bool,
) -> AnyElement {
    let chip_key = format!("{key}-cite-{ix}");
    let glow = if lit { 1. } else { motion::hover_t(&chip_key) };
    let hover_state = state.clone();
    let click_state = state.clone();
    div()
        .id(ElementId::NamedInteger("ca-cite".into(), ix as u64))
        .flex_none()
        .h(px(16.))
        .min_w(px(16.))
        .px(px(4.))
        .ml(px(3.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::XS))
        .cursor_pointer()
        .text_size(px(10.5))
        .line_height(px(16.))
        .font_weight(FontWeight::MEDIUM)
        .bg(motion::mix(
            t.fill(FillTier::Secondary),
            t.accent_wash,
            glow,
        ))
        .text_color(motion::mix(
            t.text_muted,
            if t.dark { t.text } else { t.accent },
            glow,
        ))
        .on_hover(move |hovered, window, cx| {
            motion::set_hover(&chip_key, *hovered, motion::reduced_motion(cx));
            window.refresh();
            hover_state.update(cx, |s, cx| {
                if *hovered {
                    s.hover = Some(ix);
                } else if s.hover == Some(ix) {
                    s.hover = None;
                }
                cx.notify();
            })
        })
        .on_click(move |_, _, cx| {
            click_state.update(cx, |s, cx| {
                s.open = if s.open == Some(Open::Cite(ix)) {
                    None
                } else {
                    Some(Open::Cite(ix))
                };
                cx.notify();
            })
        })
        .child(format!("{}", source + 1))
        .into_any_element()
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
                    .passage("Annual subscriptions can be refunded in full within 30 days of purchase or renewal. After 30 days we refund the unused months on a prorated basis, minus one month's fee. Monthly plans are not refunded but can be cancelled at any time.")
                    .tone(Tone::Accent),
                CitedSource::new("Billing terms, section 4.2", "Legal")
                    .date("Effective Jan 1")
                    .passage("Add-ons bought during a subscription term, including extra seats and storage, are refundable under the same conditions as the plan they are attached to, measured from the add-on's own purchase date.")
                    .tone(Tone::Warning),
                CitedSource::new("Issuing refunds", "Support playbook")
                    .date("Edited 3 days ago")
                    .passage("Agents can issue refunds of up to $5,000 directly from the billing console. Anything larger needs a finance approver on the ticket. Customers who pay by invoice receive a credit note rather than a card refund.")
                    .tone(Tone::Success),
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
                    ),
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_carry_byte_ranges() {
        let w = words("a  bc d");
        assert_eq!(
            w.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
            ["a", "bc", "d"]
        );
        assert_eq!(w[1].0, 3..5);
    }

    #[test]
    fn marks_found_in_passage() {
        let ranges = mark_ranges(
            "refund the unused months",
            &["unused months".into(), "absent".into()],
        );
        assert_eq!(ranges, vec![11..24]);
    }
}
