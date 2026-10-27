//! Find Bar – a Pink component.
//!
//! Find in a conversation the way Pink's transcript does it (Pink:
//! `transcript/find.rs` `render_find_bar`, highlights from
//! `markdown/find.rs` `FindPaint`). The bar floats over the top-right of the
//! transcript (16 from the right edge) on Pink's popover card: 420 wide,
//! `px 8 / py 4`, gap 4 – a 14px magnifier in `text_muted`, the query field
//! (13/18, "Find in conversation" in `text_faint`), the status in 12px
//! `content_muted` ("3 of 7", "No results", "Searching…", "· Partial
//! history"), then the small Aa toggle (Ghost, Secondary while matching case)
//! and Previous, Next and Close icon buttons. Enter goes to the next match,
//! Shift+Enter to the previous one, Escape closes; the buttons hand the
//! keyboard straight back to the field.
//!
//! Matches paint under the transcript's glyphs as r3 washes in `warning` at
//! 0.28; the current one deepens to 0.58 with a 1px `warning` edge. Use
//! [`find_matches`] to search your rows' display text and
//! [`highlighted_text`] (or [`highlight_underlay`] under your own
//! `StyledText`) to paint them.
//!
//! ```ignore
//! FindBar::new("find")
//!     .query(query.text.clone())
//!     .case_sensitive(query.case_sensitive)
//!     .matches(total)
//!     .active(Some(current))
//!     .on_query(|query, _, cx| search(query, cx))
//!     .on_step(|forward, _, cx| step(forward, cx))
//!     .on_close(|_, cx| close_find(cx))
//! ```

use std::{ops::Range, rc::Rc, sync::Arc};
use web_time::Duration;

use gpui::{
    AnyElement, AnyView, App, BorderStyle, Bounds, BoxShadow, Context, ElementId, Entity,
    Focusable, IntoElement, MouseButton, Pixels, SharedString, StyledText, Subscription, Task,
    TextLayout, Window, canvas, div, point, prelude::*, px, quad, size, svg, transparent_black,
};

use crate::{
    pink::{self, ButtonKind, ControlSize, Pink, radius},
    primitives::{InputEvent, TextInput, text_input::SelectAll},
    story,
};

pub const NAME: &str = "Find Bar";
pub const DESCRIPTION: &str = "A floating find bar over a conversation: type to wash every match in the transcript, step through them with Enter, Shift+Enter or the arrows, match case with Aa, and read where you are as \"3 of 7\"";

/// Pink `FIND_BAR_WIDTH`.
const FIND_BAR_WIDTH: f32 = 420.;
/// Pink `FIND_BAR_HEIGHT`: what the reveal inset reserves for the bar.
pub const FIND_BAR_HEIGHT: f32 = 40.;
/// Pink: the bar floats 16 from the transcript's right edge, and revealed
/// rows land 12 below it (`find_reveal_inset`).
pub const FIND_BAR_RIGHT: f32 = 16.;
pub const FIND_REVEAL_GAP: f32 = 12.;
/// Pink `MAX_QUERY_CHARS`: a longer paste is cut at a char boundary.
const MAX_QUERY_CHARS: usize = 256;
/// The query field's metrics (Pink `with_text_metrics(13.0, 18.0)`).
const INPUT_TEXT_SIZE: f32 = 13.;
const INPUT_LINE_HEIGHT: f32 = 18.;
/// The highlight washes (Pink `FindPaint::new` / `paint_mapped`).
const MATCH_WASH: f32 = 0.28;
const ACTIVE_WASH: f32 = 0.58;
const MATCH_RADIUS: f32 = 3.;

const MAGNIFER: &str = "icons/find-bar/magnifer.svg";
const ALT_ARROW_UP: &str = "icons/find-bar/alt-arrow-up.svg";
const ALT_ARROW_DOWN: &str = "icons/find-bar/alt-arrow-down.svg";
const CLOSE: &str = "icons/find-bar/close.svg";

// ---------------------------------------------------------------------------
// Matching (Pink: `FindQuery`, `push_folded`, `match_segment`, `map_folded`)
// ---------------------------------------------------------------------------

/// What to look for: the field's text and whether case must match.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FindQuery {
    pub text: SharedString,
    pub case_sensitive: bool,
}

impl FindQuery {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            case_sensitive: false,
        }
    }

    pub fn case_sensitive(mut self, on: bool) -> Self {
        self.case_sensitive = on;
        self
    }

    /// Whitespace alone matches nothing.
    pub fn is_blank(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// The literal, non-overlapping matches in `haystack` (see
    /// [`find_matches`]).
    pub fn matches_in(&self, haystack: &str) -> Vec<Range<usize>> {
        find_matches(haystack, self)
    }
}

/// Unicode lowercase per character plus the full case folds `to_lowercase`
/// lacks: `ß`/`ẞ` → `ss`, final sigma → `σ`, dotted capital `İ` → `i`.
fn push_folded(ch: char, out: &mut String) {
    match ch {
        'ß' | 'ẞ' => out.push_str("ss"),
        'ς' => out.push('σ'),
        'İ' => out.push('i'),
        _ => out.extend(ch.to_lowercase()),
    }
}

fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        push_folded(ch, &mut out);
    }
    out
}

/// Maps ascending folded byte ranges back to source byte ranges, widened to
/// whole source characters (Pink `map_folded`).
fn map_folded(source: &str, hits: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut out = Vec::with_capacity(hits.len());
    let mut folded_at = 0;
    let mut expansion = String::new();
    let mut hit = 0;
    let mut start: Option<usize> = None;
    for (offset, ch) in source.char_indices() {
        if hit == hits.len() {
            break;
        }
        expansion.clear();
        push_folded(ch, &mut expansion);
        let next = folded_at + expansion.len();
        while let Some(range) = hits.get(hit) {
            if start.is_none() {
                if range.start >= next {
                    break;
                }
                start = Some(offset);
            }
            if range.end > next {
                break;
            }
            out.extend(start.take().map(|start| start..offset + ch.len_utf8()));
            hit += 1;
        }
        folded_at = next;
    }
    out
}

/// Pink `match_segment`: the literal, non-overlapping byte ranges of `query`
/// in `haystack` – exact when matching case, else on Unicode case folds
/// mapped back to whole source characters. A blank query finds nothing.
pub fn find_matches(haystack: &str, query: &FindQuery) -> Vec<Range<usize>> {
    if query.is_blank() {
        return Vec::new();
    }
    let needle: String = query.text.chars().take(MAX_QUERY_CHARS).collect();
    if query.case_sensitive {
        return haystack
            .match_indices(needle.as_str())
            .map(|(start, hit)| start..start + hit.len())
            .collect();
    }
    let folded = fold(haystack);
    let hits: Vec<Range<usize>> = folded
        .match_indices(fold(&needle).as_str())
        .map(|(start, hit)| start..start + hit.len())
        .collect();
    if hits.is_empty() {
        return hits;
    }
    let ranges = if folded != haystack {
        map_folded(haystack, &hits)
    } else {
        hits
    };
    // Two folded hits inside one expanded character widen to the same source
    // character; keep the source ranges apart.
    let mut last_end = 0;
    ranges
        .into_iter()
        .filter(|range| {
            let keep = range.start >= last_end;
            if keep {
                last_end = range.end;
            }
            keep
        })
        .collect()
}

/// Pink `find_status`: position and total, progress, or coverage. Empty
/// while there is no query.
fn status_text(
    has_query: bool,
    total: usize,
    position: Option<usize>,
    searching: bool,
    complete: bool,
) -> SharedString {
    if !has_query {
        return SharedString::default();
    }
    let count = match (position.filter(|_| total > 0), total) {
        (_, 0) => None,
        (Some(position), total) => Some(format!("{} of {total}", position + 1)),
        (None, 1) => Some("1 match".to_string()),
        (None, total) => Some(format!("{total} matches")),
    };
    let text = if searching {
        count.map_or_else(|| "Searching…".to_string(), |count| format!("{count}…"))
    } else if !complete {
        count.map_or_else(
            || "Partial history".to_string(),
            |count| format!("{count}, partial history"),
        )
    } else {
        count.unwrap_or_else(|| "No results".to_string())
    };
    text.into()
}

// ---------------------------------------------------------------------------
// Highlights (Pink: `markdown/find.rs` `FindPaint`, `highlighted_text`)
// ---------------------------------------------------------------------------

/// One match to paint in a text: its byte range and whether it is the
/// current one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindHighlight {
    pub range: Range<usize>,
    pub active: bool,
}

impl FindHighlight {
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range,
            active: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// The rectangles a byte range covers on its shaped lines, one per visual
/// row (Pink: `markdown/render.rs` `range_rects`).
fn range_rects(layout: &TextLayout, range: &Range<usize>) -> Vec<Bounds<Pixels>> {
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
            rects.push(Bounds::new(p1, size(p2.x - p1.x, line_height)));
        }
        if next <= cur {
            break;
        }
        cur = next;
    }
    rects
}

/// A canvas painting the find washes under `layout`'s glyphs; put it before
/// your `StyledText` in a relative parent (Pink `FindPaint::paint`).
pub fn highlight_underlay(
    layout: TextLayout,
    highlights: Vec<FindHighlight>,
    t: &Pink,
) -> impl IntoElement {
    let wash = t.warning.opacity(MATCH_WASH);
    let active_wash = t.warning.opacity(ACTIVE_WASH);
    let active_border = t.warning;
    canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for found in &highlights {
                for rect in range_rects(&layout, &found.range) {
                    window.paint_quad(quad(
                        rect,
                        px(MATCH_RADIUS),
                        if found.active { active_wash } else { wash },
                        px(if found.active { 1. } else { 0. }),
                        if found.active {
                            active_border
                        } else {
                            transparent_black()
                        },
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

/// Plain text in the inherited style with its find highlights under it
/// (Pink `markdown/find.rs` `highlighted_text`).
pub fn highlighted_text(
    text: impl Into<SharedString>,
    highlights: Vec<FindHighlight>,
    t: &Pink,
) -> AnyElement {
    let styled = StyledText::new(text.into());
    let layout = styled.layout().clone();
    div()
        .relative()
        .child(highlight_underlay(layout, highlights, t))
        .child(styled)
        .into_any_element()
}

// ---------------------------------------------------------------------------
// The bar
// ---------------------------------------------------------------------------

type QueryHandler = Rc<dyn Fn(&FindQuery, &mut Window, &mut App)>;
type StepHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone, Default)]
struct Handlers {
    on_query: Option<QueryHandler>,
    on_step: Option<StepHandler>,
    on_close: Option<CloseHandler>,
}

struct BarState {
    input: Entity<TextInput>,
    case_sensitive: bool,
    /// The last query and case the host passed in, so a changed one wins
    /// over the field while typing in it never fights the host.
    external_query: Option<SharedString>,
    external_case: Option<bool>,
    focus_pending: bool,
    handlers: Handlers,
    _events: Subscription,
}

impl BarState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Find in conversation")
                .max_len(MAX_QUERY_CHARS)
        });
        let events = cx.subscribe_in(
            &input,
            window,
            |this: &mut Self, _, event: &InputEvent, window, cx| {
                let handlers = this.handlers.clone();
                match event {
                    InputEvent::Change(text) => {
                        let query = FindQuery {
                            text: text.clone(),
                            case_sensitive: this.case_sensitive,
                        };
                        if let Some(on_query) = handlers.on_query {
                            window.defer(cx, move |window, cx| on_query(&query, window, cx));
                        }
                        cx.notify();
                    }
                    // Enter: next; Shift+Enter: previous
                    InputEvent::Submit(_) => {
                        let forward = !window.modifiers().shift;
                        if let Some(on_step) = handlers.on_step {
                            window.defer(cx, move |window, cx| on_step(forward, window, cx));
                        }
                    }
                    InputEvent::Escape => {
                        if let Some(on_close) = handlers.on_close {
                            window.defer(cx, move |window, cx| on_close(window, cx));
                        }
                    }
                    _ => {}
                }
            },
        );
        Self {
            input,
            case_sensitive: false,
            external_query: None,
            external_case: None,
            focus_pending: true,
            handlers: Handlers::default(),
            _events: events,
        }
    }
}

/// Pink's conversation Find bar (`transcript/find.rs` `render_find_bar`).
/// Position it yourself: Pink floats it absolutely over the transcript's
/// top-right corner, [`FIND_BAR_RIGHT`] from the edge.
#[derive(IntoElement)]
pub struct FindBar {
    id: ElementId,
    query: Option<SharedString>,
    case_sensitive: Option<bool>,
    total: usize,
    active: Option<usize>,
    searching: bool,
    partial: bool,
    autofocus: bool,
    handlers: Handlers,
}

impl FindBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            query: None,
            case_sensitive: None,
            total: 0,
            active: None,
            searching: false,
            partial: false,
            autofocus: true,
            handlers: Handlers::default(),
        }
    }

    /// The query in the field. Pink seeds it from a short transcript
    /// selection or the chat's last query; whenever the value you pass
    /// changes it replaces the field's text, and typing reports back through
    /// [`on_query`](Self::on_query).
    pub fn query(mut self, text: impl Into<SharedString>) -> Self {
        self.query = Some(text.into());
        self
    }

    /// Match case (the Aa toggle). A changed value wins over the toggle's
    /// own state.
    pub fn case_sensitive(mut self, on: bool) -> Self {
        self.case_sensitive = Some(on);
        self
    }

    /// How many matches the query has in the whole conversation.
    pub fn matches(mut self, total: usize) -> Self {
        self.total = total;
        self
    }

    /// The current match's place in reading order (0-based): "3 of 7".
    /// Without one the status reads "7 matches".
    pub fn active(mut self, position: Option<usize>) -> Self {
        self.active = position;
        self
    }

    /// A search pass is still running: "Searching…" or "3 of 7…".
    pub fn searching(mut self, searching: bool) -> Self {
        self.searching = searching;
        self
    }

    /// Only part of the history has arrived, so a count is not final:
    /// "Partial history" or "3 of 7, partial history".
    pub fn partial_history(mut self, partial: bool) -> Self {
        self.partial = partial;
        self
    }

    /// Focuses the field (selecting a seeded query) when the bar mounts
    /// (default on, as Pink's Cmd+F does).
    pub fn autofocus(mut self, on: bool) -> Self {
        self.autofocus = on;
        self
    }

    /// The query or its case mode changed: search again and make the first
    /// match current.
    pub fn on_query(
        mut self,
        handler: impl Fn(&FindQuery, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handlers.on_query = Some(Rc::new(handler));
        self
    }

    /// Next (`true`) or previous match, wrapping: Enter / Shift+Enter or the
    /// arrow buttons.
    pub fn on_step(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.handlers.on_step = Some(Rc::new(handler));
        self
    }

    /// Escape or the close button.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.handlers.on_close = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for FindBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            BarState::new,
        );
        let handlers = self.handlers.clone();
        let autofocus = self.autofocus;
        let (input, case_sensitive, focus_now) = state.update(cx, |s, cx| {
            s.handlers = handlers;
            if let Some(query) = &self.query
                && s.external_query.as_ref() != Some(query)
            {
                s.external_query = Some(query.clone());
                if s.input.read(cx).text() != *query {
                    let query = query.clone();
                    s.input.update(cx, |input, cx| input.set_text(query, cx));
                }
            }
            if let Some(case) = self.case_sensitive
                && s.external_case != Some(case)
            {
                s.external_case = Some(case);
                s.case_sensitive = case;
            }
            // Pink's popover field: hint in `text_faint`, the accent caret
            s.input.update(cx, |input, _| {
                input.set_colors(Some(t.text_faint), Some(t.caret), Some(t.selection))
            });
            let focus_now = std::mem::take(&mut s.focus_pending) && autofocus;
            (s.input.clone(), s.case_sensitive, focus_now)
        });
        let focus = input.read(cx).focus_handle(cx);
        if focus_now {
            window.focus(&focus, cx);
            // Select a seeded query once it is mounted, so typing replaces it
            let seeded = input.read(cx).text();
            if !seeded.is_empty() {
                let focus = focus.clone();
                let select = input.clone();
                window.on_next_frame(move |window, cx| {
                    if focus.is_focused(window) && select.read(cx).text() == seeded {
                        window.dispatch_action(Box::new(SelectAll), cx);
                    }
                });
            }
        }

        let text = input.read(cx).text();
        let has_query = !text.trim().is_empty();
        let status = status_text(
            has_query,
            self.total,
            self.active,
            self.searching,
            !self.partial,
        );
        let has_matches = self.total > 0;
        let on_step = self.handlers.on_step.clone();
        let on_close = self.handlers.on_close.clone();
        let on_query = self.handlers.on_query.clone();

        let refocus = {
            let focus = focus.clone();
            move |window: &mut Window, cx: &mut App| window.focus(&focus, cx)
        };
        let step_button = |forward: bool| {
            let (id, glyph, label, tip) = if forward {
                ("next", ALT_ARROW_DOWN, "Next match", "Next match (Enter)")
            } else {
                (
                    "previous",
                    ALT_ARROW_UP,
                    "Previous match",
                    "Previous match (Shift+Enter)",
                )
            };
            let on_step = on_step.clone();
            let refocus = refocus.clone();
            pink::icon_button_enabled(
                format!("{key}-{id}"),
                glyph,
                ControlSize::Sm,
                has_matches,
                &t,
            )
            .aria_label(label)
            .tooltip(pink::text_tooltip(tip))
            .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
            .on_click(move |_, window, cx| {
                if has_matches && let Some(on_step) = &on_step {
                    on_step(forward, window, cx);
                }
                refocus(window, cx);
            })
        };

        // Pink: a Secondary "Aa" while matching case, Ghost otherwise
        let case_button = pink::button(
            format!("{key}-case"),
            if case_sensitive {
                ButtonKind::Secondary
            } else {
                ButtonKind::Ghost
            },
            ControlSize::Sm,
            &t,
        )
        .aria_label(if case_sensitive {
            "Match case, on"
        } else {
            "Match case, off"
        })
        .tooltip(pink::text_tooltip("Match case"))
        .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
        .child("Aa")
        .on_click({
            let state = state.clone();
            let refocus = refocus.clone();
            move |_, window, cx| {
                let query = state.update(cx, |s, cx| {
                    s.case_sensitive = !s.case_sensitive;
                    cx.notify();
                    FindQuery {
                        text: s.input.read(cx).text(),
                        case_sensitive: s.case_sensitive,
                    }
                });
                if let Some(on_query) = &on_query {
                    on_query(&query, window, cx);
                }
                refocus(window, cx);
            }
        });

        let close_button = pink::icon_button(format!("{key}-close"), CLOSE, ControlSize::Sm, &t)
            .aria_label("Close Find")
            .tooltip(pink::text_tooltip("Close (Esc)"))
            .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
            .on_click({
                let on_close = on_close.clone();
                move |_, window, cx| {
                    if let Some(on_close) = &on_close {
                        on_close(window, cx);
                    }
                }
            });

        // Pink: the popover card as a 420-wide row
        let bar = pink::popover_card(&t)
            .w(px(FIND_BAR_WIDTH))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .px(px(8.))
            .py(px(4.))
            .child(
                svg()
                    .path(MAGNIFER)
                    .size(px(pink::icon_size::SM))
                    .flex_none()
                    .text_color(t.text_muted),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(INPUT_TEXT_SIZE))
                    .line_height(px(INPUT_LINE_HEIGHT))
                    .text_color(t.text)
                    .child(input),
            )
            .child(
                div()
                    .id(SharedString::from(format!("{key}-status")))
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(t.content_muted)
                    .child(status),
            )
            .child(case_button)
            .child(step_button(false))
            .child(step_button(true))
            .child(close_button);

        div()
            .id(self.id)
            .key_context("ConversationFind")
            .font_family(pink::SANS)
            .occlude()
            // Clicks on the bar never start a transcript selection
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            // Escape from anywhere in the bar (the field reports its own)
            .on_key_down(move |event, window, cx| {
                let keystroke = &event.keystroke;
                let m = keystroke.modifiers;
                if m.platform || m.control || m.alt {
                    return;
                }
                if keystroke.key == "escape" {
                    if let Some(on_close) = &on_close {
                        on_close(window, cx);
                    }
                    cx.stop_propagation();
                }
            })
            .child(bar)
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

const DEMO_WIDTH: f32 = 640.;
/// The demo transcript's line metrics (Pink `MD_TEXT_SIZE` / `MD_LINE_HEIGHT`).
const TEXT_SIZE: f32 = 14.;
const LINE_HEIGHT: f32 = 22.;

/// A short conversation: who said it, and its display text.
const DEMO_ROWS: [(bool, &str); 6] = [
    (
        true,
        "Why did the deploy token stop working after last night's rotation?",
    ),
    (
        false,
        "The rotation job issued a new token at 02:00, but the deploy runner still reads the old one from its cache. Cached tokens are never refreshed, so every push after the rotation was rejected with a 401.",
    ),
    (
        false,
        "Deleting the cache and restarting the runner makes it fetch a fresh Token on its next start.",
    ),
    (true, "Can it pick up new tokens without a restart?"),
    (
        false,
        "Yes. Point token_path at the secret mount instead of the cache file. The mount updates in place whenever the token rotates, and the runner reads it before every job.",
    ),
    (
        false,
        "I opened #482 with that change and a check that fails the job when the token is older than a day.",
    ),
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Beat {
    Open,
    Type(&'static str),
    Step(bool),
    Case(bool),
    Close,
}

/// The demo: open, type, step, match case, narrow to nothing, close, again.
const SCRIPT: [(Beat, u64); 16] = [
    (Beat::Open, 700),
    (Beat::Type("t"), 140),
    (Beat::Type("to"), 140),
    (Beat::Type("tok"), 140),
    (Beat::Type("toke"), 140),
    (Beat::Type("token"), 1300),
    (Beat::Step(true), 900),
    (Beat::Step(true), 900),
    (Beat::Step(true), 1100),
    (Beat::Case(true), 1500),
    (Beat::Step(false), 1700),
    (Beat::Type("token r"), 1700),
    (Beat::Type("token rx"), 1500),
    (Beat::Type(""), 600),
    (Beat::Case(false), 500),
    (Beat::Close, 1600),
];

struct Demo {
    open: bool,
    query: FindQuery,
    /// The current match, in reading order.
    active: Option<usize>,
    /// Bumped on every reopen, so the bar mounts fresh and takes focus.
    session: u64,
    beat: usize,
    _timer: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            open: false,
            query: FindQuery::default(),
            active: None,
            session: 0,
            beat: 0,
            _timer: Self::play(cx),
        }
    }

    fn play(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |demo, cx| {
                    let (beat, wait) = SCRIPT[demo.beat];
                    demo.apply(beat);
                    demo.beat = (demo.beat + 1) % SCRIPT.len();
                    cx.notify();
                    wait
                }) else {
                    break;
                };
                cx.background_executor()
                    .timer(Duration::from_millis(wait))
                    .await;
            }
        })
    }

    fn apply(&mut self, beat: Beat) {
        match beat {
            Beat::Open => self.open_bar(),
            Beat::Type(text) => self.set_query(FindQuery {
                text: text.into(),
                case_sensitive: self.query.case_sensitive,
            }),
            Beat::Step(forward) => self.step(forward),
            Beat::Case(on) => self.set_query(self.query.clone().case_sensitive(on)),
            Beat::Close => self.open = false,
        }
    }

    /// The reader took over: the script stops so it never types over them.
    fn take_over(&mut self) {
        self._timer = Task::ready(());
    }

    fn open_bar(&mut self) {
        if !self.open {
            self.open = true;
            self.session += 1;
        }
    }

    /// Every match in reading order: (row, range).
    fn matches(&self) -> Vec<(usize, Range<usize>)> {
        DEMO_ROWS
            .iter()
            .enumerate()
            .flat_map(|(row, (_, text))| {
                find_matches(text, &self.query)
                    .into_iter()
                    .map(move |range| (row, range))
            })
            .collect()
    }

    /// Pink: a new query (or case mode) makes the first match current.
    fn set_query(&mut self, query: FindQuery) {
        self.query = query;
        self.active = (!self.matches().is_empty()).then_some(0);
    }

    /// Pink `find_step`: next or previous, wrapping.
    fn step(&mut self, forward: bool) {
        let len = self.matches().len();
        if len == 0 {
            return;
        }
        self.active = Some(match self.active {
            Some(position) if forward => (position + 1) % len,
            Some(position) => (position + len - 1) % len,
            None if forward => 0,
            None => len - 1,
        });
    }
}

/// Pink's user bubble (`transcript.rs` user row): right-aligned, at most 80%
/// wide, r16, `px 16 / py 10` on the bubble wash.
fn user_row(text: AnyElement, t: &Pink) -> AnyElement {
    let edge = t.user_bubble_edge();
    div()
        .w_full()
        .flex()
        .justify_end()
        .child(
            div()
                .min_w_0()
                .max_w(px(DEMO_WIDTH * 0.8))
                .bg(t.user_bubble_bg())
                .when_some(edge, |el, edge| {
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
                .child(text),
        )
        .into_any_element()
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("find-bar-demo", cx, |_, cx| Demo::new(cx));
        let (open, query, active, session, matches) = {
            let d = demo.read(cx);
            (d.open, d.query.clone(), d.active, d.session, d.matches())
        };

        // The transcript, each row with its matches washed in
        let mut rows = div().flex().flex_col().gap(px(16.));
        for (row, (user, text)) in DEMO_ROWS.iter().enumerate() {
            let highlights = if open {
                matches
                    .iter()
                    .enumerate()
                    .filter(|(_, (r, _))| *r == row)
                    .map(|(ix, (_, range))| {
                        FindHighlight::new(range.clone()).active(active == Some(ix))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let text = highlighted_text(*text, highlights, &t);
            rows = rows.child(if *user {
                user_row(text, &t)
            } else {
                div().child(text).into_any_element()
            });
        }

        let bar = open.then(|| {
            FindBar::new(ElementId::NamedInteger("find".into(), session))
                .query(query.text.clone())
                .case_sensitive(query.case_sensitive)
                .matches(matches.len())
                .active(active)
                .on_query({
                    let demo = demo.clone();
                    move |query, _, cx| {
                        demo.update(cx, |d, cx| {
                            d.take_over();
                            d.set_query(query.clone());
                            cx.notify();
                        })
                    }
                })
                .on_step({
                    let demo = demo.clone();
                    move |forward, _, cx| {
                        demo.update(cx, |d, cx| {
                            d.take_over();
                            d.step(forward);
                            cx.notify();
                        })
                    }
                })
                .on_close({
                    let demo = demo.clone();
                    move |_, cx| {
                        demo.update(cx, |d, cx| {
                            d.take_over();
                            d.open = false;
                            cx.notify();
                        })
                    }
                })
        });

        // Closed: a way back in, as Cmd+F would
        let reopen = (!open).then(|| {
            pink::button("find-bar-reopen", ButtonKind::Ghost, ControlSize::Sm, &t)
                .child(pink::glyph(MAGNIFER, ControlSize::Sm).text_color(t.readable_muted(0.95)))
                .child("Find in conversation")
                .on_click({
                    let demo = demo.clone();
                    move |_, _, cx| {
                        demo.update(cx, |d, cx| {
                            d.open_bar();
                            cx.notify();
                        })
                    }
                })
        });

        div()
            .w(px(DEMO_WIDTH + 2. * FIND_BAR_RIGHT))
            .relative()
            .font_family(pink::SANS)
            .text_color(t.text)
            .text_size(px(TEXT_SIZE))
            .line_height(px(LINE_HEIGHT))
            // Revealed rows land below the bar (Pink `find_reveal_inset`)
            .pt(px(8. + FIND_BAR_HEIGHT + FIND_REVEAL_GAP))
            .px(px(FIND_BAR_RIGHT))
            .child(rows)
            .children(bar.map(|bar| {
                div()
                    .absolute()
                    .top(px(8.))
                    .right(px(FIND_BAR_RIGHT))
                    .child(bar)
            }))
            .children(reopen.map(|button| {
                div()
                    .absolute()
                    .top(px(12.))
                    .right(px(FIND_BAR_RIGHT))
                    .child(button)
            }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_fold_case_unless_asked_not_to() {
        let text = "Token, token and TOKEN";
        let any = FindQuery::new("token");
        assert_eq!(find_matches(text, &any).len(), 3);
        let exact = FindQuery::new("token").case_sensitive(true);
        assert_eq!(find_matches(text, &exact), vec![7..12]);
        assert!(find_matches(text, &FindQuery::new("   ")).is_empty());
    }

    #[test]
    fn folded_matches_map_back_to_whole_characters() {
        // ß folds to "ss": a match inside it widens to the whole character
        let text = "Straße and STRASSE";
        let ranges = find_matches(text, &FindQuery::new("strasse"));
        assert_eq!(ranges.len(), 2);
        assert_eq!(&text[ranges[0].clone()], "Straße");
        assert_eq!(&text[ranges[1].clone()], "STRASSE");
    }

    #[test]
    fn status_reads_like_pink() {
        assert_eq!(status_text(false, 3, Some(0), false, true), "");
        assert_eq!(status_text(true, 7, Some(2), false, true), "3 of 7");
        assert_eq!(status_text(true, 7, None, false, true), "7 matches");
        assert_eq!(status_text(true, 1, None, false, true), "1 match");
        assert_eq!(status_text(true, 0, None, false, true), "No results");
        assert_eq!(status_text(true, 0, None, true, true), "Searching…");
        assert_eq!(status_text(true, 7, Some(2), true, true), "3 of 7…");
        assert_eq!(
            status_text(true, 7, Some(2), false, false),
            "3 of 7, partial history"
        );
        assert_eq!(status_text(true, 0, None, false, false), "Partial history");
    }

    #[test]
    fn demo_steps_wrap_and_case_narrows() {
        let mut demo_query = FindQuery::new("token");
        let count = |q: &FindQuery| {
            DEMO_ROWS
                .iter()
                .map(|(_, text)| find_matches(text, q).len())
                .sum::<usize>()
        };
        let all = count(&demo_query);
        demo_query.case_sensitive = true;
        let exact = count(&demo_query);
        assert!(exact < all && exact > 0);
        assert_eq!(count(&FindQuery::new("token rx")), 0);
    }
}
