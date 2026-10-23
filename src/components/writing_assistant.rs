//! Writing Assistant – a pink component.
//!
//! A rewrite panel for a piece of selected text, laid out like pink's
//! question wizard (the composer pill that becomes a prompt): a tracked
//! uppercase header with a version counter, the selection quoted the way
//! pink's markdown quotes, the rewrite as plain prose that dissolves in
//! chunk by chunk while pink's working line ticks under it, an action lane
//! (copy, try again, version paging) in the transcript's meta-lane anatomy,
//! direction chips as composer ghost pills, an instruction field drawn as a
//! nested composer pill with the microphone and the Send disc, and a footer
//! whose one Primary button replaces the selection.
//!
//! The component owns its input and view state; the caller owns the text:
//!
//! ```ignore
//! WritingAssistant::new("rewrite", draft)
//!     .suggestions(vec!["Friendlier".into(), "Shorter".into()])
//!     .versions(versions)                       // grows as rewrites stream in
//!     .on_request(|instruction, _, cx| { /* start a rewrite */ })
//!     .on_apply(|text, _, cx| { /* replace the selection */ })
//! ```

use std::{ops::Range, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, ClipboardItem, Context, Div, ElementId, Entity, FontWeight,
    HighlightStyle, IntoElement, Role, SharedString, Stateful, StyledText, Subscription, Task,
    Toggled, Window, div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, AnimationExt, GLYPH_SWAP, TRANSCRIPT_IN, TRANSCRIPT_IN_RISE},
    pink::{self, ButtonKind, ControlSize, Pink, loaders, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Writing Assistant";
pub const DESCRIPTION: &str = "Rewrite a selection in place – pick a direction, type or dictate what to change, watch the new version fade in under a live working line, page through versions and replace the selection";

const ICON_SEND: &str = "icons/writing-assistant/arrow-up.svg";
const ICON_MIC: &str = "icons/writing-assistant/microphone.svg";
const ICON_COPY: &str = "icons/writing-assistant/copy.svg";
const ICON_COPIED: &str = "icons/writing-assistant/copy-check.svg";
const ICON_RETRY: &str = "icons/writing-assistant/refresh.svg";
const ICON_PREV: &str = "icons/writing-assistant/alt-arrow-left.svg";
const ICON_NEXT: &str = "icons/writing-assistant/alt-arrow-right.svg";

/// Prose, quoted or rewritten: pink's markdown body, 14/22
/// (`markdown/render.rs` `MD_TEXT_SIZE` / `MD_LINE_HEIGHT`).
const PROSE_SIZE: f32 = 14.;
const PROSE_LINE: f32 = 22.;
/// The instruction field: the composer input, 14 / 22.75 (`composer.rs`
/// `INPUT_TEXT_SIZE` / `INPUT_LINE_HEIGHT`).
const INPUT_SIZE: f32 = 14.;
const INPUT_LINE: f32 = 22.75;
/// Every composer-row control is a 28 pt capsule or circle (`pickers.rs`
/// `CONTROL_HEIGHT`).
const ROW_CONTROL: f32 = 28.;
/// The meta lane: 24 pt actions under a 4 pt lead (`transcript.rs`
/// `META_LANE_HEIGHT`).
const META_LANE: f32 = 28.;
const META_ACTION: f32 = 24.;
/// How long Copy shows its check (pink's code-block "Copied", ~1.2 s).
const COPIED_MS: u64 = 1200;
/// Heard words beyond this many characters scroll off the front of the
/// one-line dictation field.
const HEARD_CHARS: usize = 54;

// pink: markdown/veil.rs – the streaming fade's constants (mugen FadePainter).
const VEIL_EMA_SEED_MS: f32 = 160.;
const VEIL_MIN_FADE_MS: f32 = 120.;
const VEIL_MAX_FADE_MS: f32 = 400.;
const VEIL_CURVE_POW: f32 = 1.6;
const VEIL_GAP_CLAMP_MS: f32 = 1000.;

type TextHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;
type ToggleHandler = Rc<dyn Fn(bool, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One rewrite of the selection.
#[derive(Clone)]
pub struct Version {
    /// What was asked ("Friendlier", or the typed instruction).
    pub label: SharedString,
    pub text: SharedString,
    /// Words shown so far while it streams; `None` once complete.
    pub reveal: Option<usize>,
}

impl Version {
    pub fn new(label: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            text: text.into(),
            reveal: None,
        }
    }

    /// Still streaming: only the first `words` words have arrived.
    pub fn streaming(mut self, words: usize) -> Self {
        self.reveal = Some(words);
        self
    }

    fn writing(&self) -> bool {
        self.reveal
            .is_some_and(|n| n < self.text.split_whitespace().count())
    }

    /// The text that has arrived: the first `reveal` words, spacing kept.
    fn visible(&self) -> &str {
        match self.reveal {
            Some(words) => &self.text[..word_end(&self.text, words)],
            None => &self.text,
        }
    }
}

/// Byte offset just past the `words`-th word of `text`.
fn word_end(text: &str, words: usize) -> usize {
    if words == 0 {
        return 0;
    }
    let mut seen = 0;
    let mut in_word = false;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if in_word {
                seen += 1;
                if seen == words {
                    return i;
                }
            }
            in_word = false;
        } else {
            in_word = true;
        }
    }
    text.len()
}

/// The last `max` characters of `heard`, from a word start, behind an
/// ellipsis, so the newest words stay in view.
fn heard_tail(heard: &str, max: usize) -> String {
    let count = heard.chars().count();
    if count <= max {
        return heard.to_string();
    }
    let tail: String = heard.chars().skip(count - max).collect();
    let tail = tail.split_once(' ').map_or(tail.as_str(), |(_, rest)| rest);
    format!("…{tail}")
}

/// One appended run of text mid-dissolve.
struct Chunk {
    range: Range<usize>,
    started: Instant,
    duration_ms: f32,
}

/// pink: markdown/veil.rs `ElemVeil` – streamed text commits to layout at
/// once and a cosmetic veil dissolves over each newly arrived chunk: opacity
/// only, `alpha = 1 − (1 − p)^1.6`, each chunk's duration `clamp(ema × 3,
/// 120, 400)` from the cadence of appends, a backed-up stream sped up.
struct Veil {
    version: Option<usize>,
    prev: String,
    chunks: Vec<Chunk>,
    ema_ms: f32,
    last_append: Option<Instant>,
}

impl Veil {
    fn new() -> Self {
        Veil {
            version: None,
            prev: String::new(),
            chunks: Vec::new(),
            ema_ms: VEIL_EMA_SEED_MS,
            last_append: None,
        }
    }

    /// Follows `version`'s visible `text`. A version arriving `fresh` fades
    /// in from nothing; one paged to (or on screen at mount) is adopted as
    /// already painted, like pink's seeded baseline.
    fn advance(
        &mut self,
        version: usize,
        text: &str,
        fresh: bool,
        now: Instant,
    ) -> Vec<(Range<usize>, f32)> {
        if self.version != Some(version) {
            *self = Veil::new();
            self.version = Some(version);
            if !fresh {
                self.prev.push_str(text);
            }
        }
        if text != self.prev {
            let mut p = self
                .prev
                .bytes()
                .zip(text.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            while p > 0 && !text.is_char_boundary(p) {
                p -= 1;
            }
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
        let boost = |n: usize| 1. + 0.3 * n.saturating_sub(2) as f32;
        let elapsed = |c: &Chunk| now.saturating_duration_since(c.started).as_secs_f32() * 1000.;
        let b = boost(self.chunks.len());
        self.chunks.retain(|c| elapsed(c) * b < c.duration_ms);
        let b = boost(self.chunks.len());
        self.chunks
            .iter()
            .map(|c| {
                let p = (elapsed(c) * b / c.duration_ms).clamp(0., 1.);
                (c.range.clone(), 1. - (1. - p).powf(VEIL_CURVE_POW))
            })
            .collect()
    }
}

struct State {
    input: Entity<TextInput>,
    has_text: bool,
    /// Index of the version on screen; `None` follows the newest.
    viewing: Option<usize>,
    seen_versions: usize,
    primed: bool,
    veil: Veil,
    /// When the version on screen started writing, for the working timer.
    writing_since: Option<(usize, Instant)>,
    /// The version whose Copy shows its check.
    copied: Option<usize>,
    listen_started: Option<Instant>,
    on_request: Option<TextHandler>,
    on_close: Option<Handler>,
    _reset: Option<Task<()>>,
    _input_events: Subscription,
}

impl State {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextInput::new(window, cx).placeholder("Say what to change…"));
        let sub = cx.subscribe_in(
            &input,
            window,
            |this: &mut State, input, ev, window, cx| match ev {
                InputEvent::Change(text) => {
                    this.has_text = !text.trim().is_empty();
                    cx.notify();
                }
                InputEvent::Submit(text) => {
                    let text = text.trim().to_string();
                    if text.is_empty() {
                        return;
                    }
                    input.update(cx, |i, cx| i.clear(cx));
                    this.has_text = false;
                    if let Some(f) = this.on_request.clone() {
                        f(text.into(), window, cx);
                    }
                    cx.notify();
                }
                InputEvent::Escape => {
                    if let Some(f) = this.on_close.clone() {
                        f(window, cx);
                    }
                }
                _ => {}
            },
        );
        State {
            input,
            has_text: false,
            viewing: None,
            seen_versions: 0,
            primed: false,
            veil: Veil::new(),
            writing_since: None,
            copied: None,
            listen_started: None,
            on_request: None,
            on_close: None,
            _reset: None,
            _input_events: sub,
        }
    }
}

#[derive(IntoElement)]
pub struct WritingAssistant {
    id: ElementId,
    selection: SharedString,
    context: Option<SharedString>,
    suggestions: Vec<SharedString>,
    versions: Vec<Version>,
    listening: Option<SharedString>,
    width: f32,
    on_request: Option<TextHandler>,
    on_apply: Option<TextHandler>,
    on_listen: Option<ToggleHandler>,
    on_close: Option<Handler>,
}

impl WritingAssistant {
    pub fn new(id: impl Into<ElementId>, selection: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            selection: selection.into(),
            context: None,
            suggestions: Vec::new(),
            versions: Vec::new(),
            listening: None,
            width: 580.,
            on_request: None,
            on_apply: None,
            on_listen: None,
            on_close: None,
        }
    }

    /// Where the selection comes from ("Release email · draft").
    pub fn context(mut self, context: impl Into<SharedString>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn suggestions(mut self, suggestions: Vec<SharedString>) -> Self {
        self.suggestions = suggestions;
        self
    }

    pub fn versions(mut self, versions: Vec<Version>) -> Self {
        self.versions = versions;
        self
    }

    /// Dictation in progress, with the words heard so far.
    pub fn listening(mut self, transcript: Option<SharedString>) -> Self {
        self.listening = transcript;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// A suggestion, a typed instruction or a finished dictation.
    pub fn on_request(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_request = Some(Rc::new(handler));
        self
    }

    /// "Replace selection" with the version on screen.
    pub fn on_apply(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_apply = Some(Rc::new(handler));
        self
    }

    /// The microphone: `true` to start listening, `false` to stop. Stopping
    /// with words heard also sends them through `on_request`.
    pub fn on_listen(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_listen = Some(Rc::new(handler));
        self
    }

    /// "Keep original" (or Escape in the field): dismiss without replacing.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }
}

/// pink: composer.rs `composer_surface` – the pill chrome the question
/// wizard shares with the composer: radius 26, a 1 px `border`, the opaque
/// `input_glass_bg` plate. `floating` adds the opaque treatment's
/// `shadow_lg`; the nested field stays flat.
fn composer_surface(floating: bool, t: &Pink) -> Div {
    div()
        .rounded(px(radius::COMPOSER))
        .border_1()
        .border_color(t.border)
        .bg(t.input_glass_bg())
        .when(floating, |el| el.shadow(pink::shadow_lg()))
}

/// pink: transcript.rs `meta_action` – a 24 pt square, radius 6,
/// transparent → `ink(0.08)`, a 14 pt glyph in `content_muted`, its name
/// as the tooltip. Disabled (the ends of the version list) it rests at
/// pink's disabled 45%.
fn meta_action(
    id: impl Into<SharedString>,
    icon: &'static str,
    label: &'static str,
    enabled: bool,
    t: &Pink,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let key = format!("wa-meta-{id}");
    let ring = t.focus_ring();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .size(px(META_ACTION))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .map(|el| {
            if enabled {
                el.tab_index(0)
                    .cursor_pointer()
                    .focus_visible(move |s| pink::focus_ring(s, ring))
                    .on_key_down(pink::swallow_activation_key)
                    .hover_fill(key, t.ink(0.), t.ink(0.08))
                    .tooltip(pink::text_tooltip(label))
                    .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
            } else {
                el.opacity(0.45)
            }
        })
        .child(
            svg()
                .path(icon)
                .size(px(14.))
                .flex_none()
                .text_color(t.content_muted),
        )
}

/// pink: composer/goals.rs `render_goal_mode_chip` – a ghost pill that is
/// on: the same 28 pt capsule at 12 medium, washed in `accent @ 0.14`
/// (0.2 under the pointer) with the label in `readable_tone(accent)`. The
/// direction of the version on screen wears it.
fn chip_on(id: impl Into<SharedString>, t: &Pink) -> Stateful<Div> {
    let id: SharedString = id.into();
    let ring = t.focus_ring();
    div()
        .id(id.clone())
        .role(Role::Button)
        .aria_toggled(Toggled::True)
        .tab_index(0)
        .focus_visible(move |s| pink::focus_ring(s, ring))
        .on_key_down(pink::swallow_activation_key)
        .flex_none()
        .h(px(ROW_CONTROL))
        .px(px(10.))
        .flex()
        .items_center()
        .rounded_full()
        .text_size(px(12.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.readable_tone(t.accent, 0.95))
        .cursor_pointer()
        .hover_fill(
            format!("wa-chip-on-{id}"),
            t.accent.opacity(0.14),
            t.accent.opacity(0.2),
        )
}

/// An approval-context label (pink: composer.rs question wizard): 11 pt in
/// `readable_muted(0.65)`, one line.
fn context_label(copy: impl Into<SharedString>, t: &Pink) -> Div {
    div()
        .flex()
        .min_w_0()
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(t.readable_muted(0.65))
        .child(div().flex_1().min_w_0().truncate().child(copy.into()))
}

impl RenderOnce for WritingAssistant {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            State::new,
        );

        let now = Instant::now();
        let listening = self.listening.is_some();
        let version_count = self.versions.len();
        let input_colors = (t.placeholder, t.caret, t.selection);
        let (shown_ix, veil, writing_secs) = state.update(cx, |s, cx| {
            s.on_request = self.on_request.clone();
            s.on_close = self.on_close.clone();
            // A new version takes the stage and dissolves in from nothing.
            let arrived = s.primed && version_count > s.seen_versions;
            if version_count != s.seen_versions {
                s.seen_versions = version_count;
                s.viewing = None;
            }
            s.primed = true;
            match (listening, s.listen_started) {
                (true, None) => s.listen_started = Some(now),
                (false, Some(_)) => s.listen_started = None,
                _ => {}
            }
            s.input.update(cx, |i, _| {
                i.set_colors(
                    Some(input_colors.0),
                    Some(input_colors.1),
                    Some(input_colors.2),
                )
            });
            let shown_ix = s
                .viewing
                .filter(|ix| *ix < version_count)
                .or(version_count.checked_sub(1));
            let Some(ix) = shown_ix else {
                s.veil = Veil::new();
                return (None, Vec::new(), 0);
            };
            let version = &self.versions[ix];
            let veil = s.veil.advance(ix, version.visible(), arrived, now);
            let secs = if version.writing() {
                let since = match s.writing_since {
                    Some((i, since)) if i == ix => since,
                    _ => {
                        s.writing_since = Some((ix, now));
                        now
                    }
                };
                now.saturating_duration_since(since).as_secs() as i64
            } else {
                0
            };
            (Some(ix), veil, secs)
        });
        if !veil.is_empty() {
            window.request_animation_frame();
        }
        let s = state.read(cx);
        let input = s.input.clone();
        let has_text = s.has_text;
        let copied = s.copied;
        let listen_started = s.listen_started;
        let input_focused = input.read(cx).is_focused(window);
        let shown = shown_ix.map(|ix| self.versions[ix].clone());
        let writing = shown.as_ref().is_some_and(|v| v.writing());

        // ---- header: pink's wizard header -------------------------------
        // Tracked uppercase 11 medium in `readable_muted(0.6)`, gap 10, and
        // the counter chip once there is more than one version (h 20, px 6,
        // radius 6, `ink(0.06)`).
        let header = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .h(px(20.))
            .child(
                div()
                    .text_size(px(11.))
                    .line_height(px(14.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.readable_muted(0.6))
                    .child(pink::tracked_upper("Rewrite")),
            )
            .when_some(shown_ix.filter(|_| version_count > 1), |el, ix| {
                el.child(
                    div()
                        .h(px(20.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::SM))
                        .bg(t.ink(0.06))
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.readable_muted(0.6))
                        .child(format!("{}/{version_count}", ix + 1)),
                )
            });

        // ---- the selection: pink's markdown blockquote --------------------
        // A 2 px `accent @ 0.6` rail, `accent @ 0.05` behind, right corners
        // 6, `pl 12 pr 10 py 6`, prose in `content_muted`. Replacing it swaps
        // the quote in place (pink TRANSCRIPT_IN, 160 ms + 4 pt).
        let quote_id = loaders::label_id("wa-quote", &self.selection);
        let original = div()
            .mt(px(8.))
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(context_label(
                match &self.context {
                    Some(c) => format!("Selected text · {c}"),
                    None => "Selected text".into(),
                },
                &t,
            ))
            .child(motion::rise_in(
                quote_id,
                TRANSCRIPT_IN,
                TRANSCRIPT_IN_RISE,
                div()
                    .border_l_2()
                    .border_color(t.accent.opacity(0.6))
                    .bg(t.accent.opacity(0.05))
                    .rounded_tr(px(radius::SM))
                    .rounded_br(px(radius::SM))
                    .pl(px(12.))
                    .pr(px(10.))
                    .py(px(6.))
                    .text_size(px(PROSE_SIZE))
                    .line_height(px(PROSE_LINE))
                    .text_color(t.content_muted)
                    .line_clamp(3)
                    .child(self.selection.clone()),
            ));

        // ---- the rewrite ------------------------------------------------
        let rewrite: AnyElement = match (&shown, shown_ix) {
            (Some(version), Some(ix)) => {
                // The prose: transcript text, bare on the plate, the veil's
                // chunks multiplied into the text color.
                let prose_text: SharedString = version.visible().to_string().into();
                let highlights = veil.iter().map(|(range, alpha)| {
                    (
                        range.clone(),
                        HighlightStyle {
                            color: Some(t.text.opacity(*alpha)),
                            ..Default::default()
                        },
                    )
                });
                let prose = div()
                    .min_h(px(PROSE_LINE))
                    .text_size(px(PROSE_SIZE))
                    .line_height(px(PROSE_LINE))
                    .text_color(t.text)
                    .child(StyledText::new(prose_text).with_highlights(highlights));

                // The lane under it: pink's working line while it writes,
                // then the meta lane's actions (both fit the reserved 28).
                let lane_body: AnyElement = if writing {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(loaders::orb_slot(t.accent))
                        .child(loaders::working_status("Rewriting", t.content_muted, t.text))
                        .child(loaders::working_timer(writing_secs, t.content_faint))
                        .into_any_element()
                } else {
                    let delta = version.text.split_whitespace().count() as i64
                        - self.selection.split_whitespace().count() as i64;
                    let note = match delta {
                        _ if version.text == self.selection => "In your text".to_string(),
                        0 => "Same length".to_string(),
                        1 => "1 word longer".to_string(),
                        -1 => "1 word shorter".to_string(),
                        d if d < 0 => format!("{} words shorter", -d),
                        d => format!("{d} words longer"),
                    };
                    let is_copied = copied == Some(ix);
                    let copy_text = version.text.clone();
                    let copy_state = state.clone();
                    let retry_label = version.label.clone();
                    let on_request = self.on_request.clone();
                    let (prev_state, next_state) = (state.clone(), state.clone());
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap(px(2.))
                        .child(
                            div()
                                .mr(px(6.))
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(t.content_faint)
                                .child(note),
                        )
                        .child(
                            meta_action(
                                format!("{key}-copy"),
                                if is_copied { ICON_COPIED } else { ICON_COPY },
                                if is_copied { "Copied" } else { "Copy" },
                                true,
                                &t,
                            )
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    copy_text.to_string(),
                                ));
                                let entity = copy_state.clone();
                                copy_state.update(cx, |s, cx| {
                                    s.copied = Some(ix);
                                    cx.notify();
                                    s._reset = Some(cx.spawn(async move |_, cx| {
                                        cx.background_executor()
                                            .timer(Duration::from_millis(COPIED_MS))
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
                            meta_action(format!("{key}-retry"), ICON_RETRY, "Try again", true, &t)
                                .when_some(on_request, |b, f| {
                                    b.on_click(move |_, window, cx| {
                                        f(retry_label.clone(), window, cx)
                                    })
                                }),
                        )
                        .child(div().flex_1())
                        .when(version_count > 1, |el| {
                            el.child(
                                meta_action(
                                    format!("{key}-prev"),
                                    ICON_PREV,
                                    "Previous version",
                                    ix > 0,
                                    &t,
                                )
                                .on_click(move |_, _, cx| {
                                    prev_state.update(cx, |s, cx| {
                                        s.viewing = Some(ix.saturating_sub(1));
                                        cx.notify();
                                    })
                                }),
                            )
                            .child(
                                meta_action(
                                    format!("{key}-next"),
                                    ICON_NEXT,
                                    "Next version",
                                    ix + 1 < version_count,
                                    &t,
                                )
                                .on_click(move |_, _, cx| {
                                    next_state.update(cx, |s, cx| {
                                        s.viewing = Some((ix + 1).min(version_count - 1));
                                        cx.notify();
                                    })
                                }),
                            )
                        })
                        .into_any_element()
                };
                // The lane swaps like pink's status line (STATUS_SWAP:
                // opacity + 2 pt settle, 180 ms quint).
                let lane = div()
                    .h(px(META_LANE))
                    .pt(px(META_LANE - META_ACTION))
                    .flex()
                    .items_center()
                    .child(motion::status_swap(
                        loaders::label_id(
                            "wa-lane",
                            &format!("{ix}-{}", if writing { "writing" } else { "done" }),
                        ),
                        div().flex_1().h(px(META_ACTION)).flex().items_center().child(lane_body),
                    ));

                // Paging swaps the version in place (TRANSCRIPT_IN).
                motion::rise_in(
                    ElementId::NamedInteger("wa-version".into(), ix as u64),
                    TRANSCRIPT_IN,
                    TRANSCRIPT_IN_RISE,
                    // The lane sits right under the prose, its own 4 pt
                    // lead the only space, as under a transcript message.
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(context_label(version.label.clone(), &t))
                                .child(prose),
                        )
                        .child(lane),
                )
                .into_any_element()
            }
            // Nothing asked yet: the wizard's question, 15/20 medium, and a
            // 12 pt note in `readable_muted(0.65)`.
            _ => div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child("How should it read?"),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(t.readable_muted(0.65))
                        .child(
                            "Pick a direction or say what to change. Your text stays as it is until you replace it.",
                        ),
                )
                .into_any_element(),
        };

        // ---- directions: composer ghost pills ------------------------------
        // pink: pickers.rs `trigger_chip` (28 pt, `px 10`, 12 medium,
        // transparent → `element_hover`), chips 4 apart as in the composer's
        // row; the direction on screen is on (`chip_on`).
        let active_label = shown.as_ref().map(|v| v.label.clone());
        let chips = self.suggestions.iter().enumerate().map(|(i, label)| {
            let id = format!("{key}-chip-{i}");
            let chip = if active_label.as_ref() == Some(label) {
                chip_on(id, &t)
            } else {
                pink::trigger_chip(id, false, false, &t).flex_none()
            };
            let on_request = self.on_request.clone();
            let ask = label.clone();
            chip.aria_label(label.clone())
                .when_some(on_request, |c, f| {
                    c.on_click(move |_, window, cx| f(ask.clone(), window, cx))
                })
                .child(label.clone())
        });
        let directions = div()
            .mt(px(12.))
            .flex()
            .flex_wrap()
            .gap(px(4.))
            .children(chips);

        // ---- the instruction field: a nested composer pill -----------------
        // The question wizard's free answer (a `composer_surface` inside the
        // panel, `mt 12 pb 4 px 4`) with the composer's controls inside it: the input
        // at 14 / 22.75, a 28 pt microphone circle and the 28 pt Send disc
        // 8 apart, 10 from the edge like the composer's controls row.
        let heard = self.listening.clone().unwrap_or_default();
        let has_heard = !heard.trim().is_empty();
        let send_ready = if listening { has_heard } else { has_text };

        let on_listen = self.on_listen.clone();
        let on_request_mic = self.on_request.clone();
        let heard_mic = heard.clone();
        // pink: composer.rs Attach – a 28 pt circle, transparent →
        // `ink(0.10)`, a 16 pt glyph in `text_muted`. While listening it holds
        // the wash with the glyph in `text`, the composer row's open state.
        let mic_key = format!("{key}-mic");
        let mic_label = if listening {
            "Stop dictation"
        } else {
            "Dictate"
        };
        let ring = t.focus_ring();
        let mic = div()
            .id(SharedString::from(mic_key.clone()))
            .role(Role::Button)
            .aria_label(mic_label)
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
            .on_key_down(pink::swallow_activation_key)
            .flex_none()
            .size(px(ROW_CONTROL))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .tooltip(pink::text_tooltip(mic_label))
            .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
            .map(|d| {
                if listening {
                    d.bg(t.ink(0.10))
                } else {
                    d.hover_fill(mic_key.clone(), t.ink(0.), t.ink(0.10))
                }
            })
            .child(
                svg()
                    .path(ICON_MIC)
                    .size(px(16.))
                    .flex_none()
                    .text_color(if listening { t.text } else { t.text_muted }),
            )
            .on_click(move |_, window, cx| {
                if let Some(f) = &on_listen {
                    f(!listening, window, cx);
                }
                if listening
                    && has_heard
                    && let Some(f) = &on_request_mic
                {
                    f(heard_mic.clone(), window, cx);
                }
            });

        // pink: composer.rs `render_send_button` – one stable disc: solid
        // `text` (→ `text @ 0.88` under the pointer, 0.76 pressed) with the
        // arrow in `bg` when it can send; recessed `ink(0.08)` with the arrow
        // at `text @ 0.35` when empty, the arrow turning accent while the
        // field is focused. The glyph fades in on each change (180 ms quint).
        let ready_glyph = !send_ready && input_focused && !listening;
        let glyph_color = if send_ready {
            t.bg
        } else if ready_glyph {
            t.readable_tone(t.accent, 1.0)
        } else {
            t.text.opacity(0.35)
        };
        let glyph_state = if send_ready {
            "send"
        } else if ready_glyph {
            "ready"
        } else {
            "rest"
        };
        let send_key = format!("{key}-send");
        let send_input = input.clone();
        let on_listen_send = self.on_listen.clone();
        let heard_send = heard.clone();
        let send = div()
            .id(SharedString::from(send_key.clone()))
            .role(Role::Button)
            .aria_label("Send")
            .flex_none()
            .size(px(ROW_CONTROL))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .tooltip(pink::text_tooltip("Send (⏎)"))
            .tooltip_show_delay(pink::layout::TOOLTIP_DELAY)
            .child(
                svg()
                    .path(ICON_SEND)
                    .size(px(14.))
                    .flex_none()
                    .text_color(glyph_color)
                    .with_animation(
                        SharedString::from(format!("{send_key}-glyph-{glyph_state}")),
                        GLYPH_SWAP.animation(),
                        |el, d| el.opacity(d),
                    ),
            )
            .map(|d| {
                if !send_ready {
                    return d.bg(t.ink(0.08));
                }
                let state = state.clone();
                d.cursor_pointer()
                    .hover_fill(send_key.clone(), t.text, t.text.opacity(0.88))
                    .active(|s| s.opacity(0.76))
                    .on_click(move |_, window, cx| {
                        if listening {
                            if let Some(f) = &on_listen_send {
                                f(false, window, cx);
                            }
                            let f = state.read(cx).on_request.clone();
                            if let Some(f) = f {
                                f(heard_send.clone(), window, cx);
                            }
                            return;
                        }
                        let text = send_input.read(cx).text();
                        if text.trim().is_empty() {
                            return;
                        }
                        send_input.update(cx, |i, cx| i.clear(cx));
                        let f = state.update(cx, |s, _| {
                            s.has_text = false;
                            s.on_request.clone()
                        });
                        if let Some(f) = f {
                            f(text.trim().to_string().into(), window, cx);
                        }
                    })
            });

        // While dictating, the field is a live line: the breathing orb, the
        // words as they are heard (the newest kept in view; a shimmering
        // "Listening…" until the first one) and pink's elapsed timer.
        let field: AnyElement = match (listening, listen_started) {
            (true, Some(started)) => div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(loaders::orb_slot(t.accent))
                .child(if has_heard {
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_color(t.text)
                        .child(heard_tail(heard.trim(), HEARD_CHARS))
                        .into_any_element()
                } else {
                    loaders::shimmer_text(
                        "Listening…",
                        INPUT_SIZE,
                        INPUT_LINE,
                        t.placeholder,
                        t.text,
                    )
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .into_any_element()
                })
                .child(loaders::working_timer(
                    now.saturating_duration_since(started).as_secs() as i64,
                    t.content_faint,
                ))
                .into_any_element(),
            _ => div()
                .flex_1()
                .min_w_0()
                .text_color(t.text)
                .child(input.clone())
                .into_any_element(),
        };
        let field = div().mt(px(12.)).pb(px(4.)).px(px(4.)).child(
            composer_surface(false, &t)
                .w_full()
                .overflow_hidden()
                .pl(px(16.))
                .pr(px(10.))
                .py(px(10.))
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(INPUT_SIZE))
                .line_height(px(INPUT_LINE))
                .child(field)
                .child(mic)
                .child(send),
        );

        // ---- footer: pink's wizard footer -----------------------------------
        // `px 16 pb 16 pt 4`, gap 12, ghost buttons on the left and the one
        // Primary on the right (`Lg`, `px 16`), resting at pink's disabled
        // opacity until a finished version differs from the selection.
        let applied = shown.as_ref().is_some_and(|v| v.text == self.selection);
        let can_apply = shown.is_some() && !writing && !applied;
        let on_apply = self.on_apply.clone();
        let apply_text = shown.as_ref().map(|v| v.text.clone());
        let replace = pink::button_enabled(
            format!("{key}-apply"),
            ButtonKind::Primary,
            ControlSize::Lg,
            can_apply,
            &t,
        )
        .px(px(16.))
        .aria_label("Replace selection")
        .child(if applied {
            "Replaced"
        } else {
            "Replace selection"
        })
        .when_some(
            on_apply.zip(apply_text).filter(|_| can_apply),
            |b, (f, text)| b.on_click(move |_, window, cx| f(text.clone(), window, cx)),
        );
        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(16.))
            .pb(px(16.))
            .pt(px(4.))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(8.))
                    .when_some(self.on_close.clone(), |el, f| {
                        el.child(
                            pink::button(
                                format!("{key}-close"),
                                ButtonKind::Ghost,
                                ControlSize::Lg,
                                &t,
                            )
                            .aria_label("Keep original")
                            .child("Keep original")
                            .on_click(move |_, window, cx| f(window, cx)),
                        )
                    }),
            )
            .child(replace);

        div()
            .id(self.id.clone())
            .w(px(self.width))
            .max_w_full()
            .font_family(pink::SANS)
            .child(
                composer_surface(true, &t)
                    .flex()
                    .flex_col()
                    .text_color(t.text)
                    .child(
                        div()
                            .px(px(16.))
                            .pt(px(16.))
                            .flex()
                            .flex_col()
                            .child(header)
                            .child(original)
                            .child(div().mt(px(12.)).child(rewrite))
                            .child(directions)
                            .child(field),
                    )
                    .child(footer),
            )
    }
}

// ---------------------------------------------------------------------------
// Story: polishing a release announcement.
// ---------------------------------------------------------------------------

const DRAFT: &str = "hey all, the new release is out and it has a lot of stuff in it like faster sync and the new sharing settings. please update when you get a chance and let us know if anything breaks";

const SUGGESTIONS: [&str; 4] = ["Friendlier", "Shorter", "Clearer", "Fix grammar"];

fn rewrite_for(label: &str) -> &'static str {
    match label {
        "Friendlier" => {
            "Hi everyone! Version 3.2 is out today, with faster sync and the new sharing settings you asked for. Update whenever it suits you, and tell us if anything feels off."
        }
        "Shorter" => {
            "Version 3.2 is out: faster sync and new sharing settings. Please update, and tell us if anything breaks."
        }
        "Clearer" => {
            "Version 3.2 is now available. Sync is faster, and all sharing settings now live in one place. Please update when you can and report any problems in #support."
        }
        "Fix grammar" => {
            "Hey all, the new release is out, and it includes a lot: faster sync and the new sharing settings. Please update when you get a chance, and let us know if anything breaks."
        }
        _ => {
            "Big news: version 3.2 just landed! Sync is faster and sharing settings are all new. Update today, then join Thursday's walkthrough webinar to see it in action."
        }
    }
}

const DICTATION: &str = "make it sound more excited and mention the webinar on Thursday";

const WORDS_PER_SEC: f32 = 16.;
/// Dictation "hears" a word this often (seconds).
const WORD_HEARD: f32 = 0.24;

struct Demo {
    selection: SharedString,
    /// (label, text, started)
    versions: Vec<(SharedString, &'static str, Instant)>,
    listening: Option<Instant>,
    /// The scripted tour: a first rewrite, a shorter one, then dictation.
    /// It stops as soon as the reader takes over.
    step: u8,
    touched: bool,
    _ticker: Task<()>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let mounted = Instant::now();
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let alive = this.update(cx, |demo, cx| {
                    let at = mounted.elapsed().as_secs_f32();
                    if !demo.touched {
                        match demo.step {
                            0 if at > 1.4 => {
                                demo.step = 1;
                                demo.request("Friendlier".into());
                            }
                            1 if at > 3.9 => {
                                demo.step = 2;
                                demo.request("Shorter".into());
                            }
                            2 if at > 5.2 => {
                                demo.step = 3;
                                demo.listening = Some(Instant::now());
                            }
                            _ => {}
                        }
                    }
                    // Dictation sends itself once the sentence is heard.
                    if let Some(started) = demo.listening {
                        let total = DICTATION.split(' ').count() as f32;
                        if started.elapsed().as_secs_f32() > total * WORD_HEARD + 0.9 {
                            demo.listening = None;
                            demo.request(DICTATION.into());
                        }
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        });
        Demo {
            selection: DRAFT.into(),
            versions: Vec::new(),
            listening: None,
            step: 0,
            touched: false,
            _ticker: ticker,
        }
    }

    fn request(&mut self, label: SharedString) {
        let text = rewrite_for(&label);
        let label = if SUGGESTIONS.contains(&label.as_ref()) {
            label
        } else {
            let mut l = label.to_string();
            if let Some(c) = l.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            l.into()
        };
        self.versions.push((label, text, Instant::now()));
    }

    fn transcript(&self) -> Option<SharedString> {
        self.listening.map(|started| {
            let n = (started.elapsed().as_secs_f32() / WORD_HEARD) as usize;
            DICTATION
                .split(' ')
                .take(n)
                .collect::<Vec<_>>()
                .join(" ")
                .into()
        })
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("writing-assistant-demo", cx, |_, cx| Demo::new(cx));
        let d = demo.read(cx);
        let versions = d
            .versions
            .iter()
            .map(|(label, text, started)| {
                let n = (started.elapsed().as_secs_f32() * WORDS_PER_SEC) as usize;
                let v = Version::new(label.clone(), *text);
                if n < text.split_whitespace().count() {
                    v.streaming(n)
                } else {
                    v
                }
            })
            .collect();
        let (on_req, on_apply, on_listen, on_close) =
            (demo.clone(), demo.clone(), demo.clone(), demo.clone());
        WritingAssistant::new("writing-assistant", d.selection.clone())
            .context("Release announcement · #general draft")
            .suggestions(SUGGESTIONS.iter().map(|s| SharedString::from(*s)).collect())
            .versions(versions)
            .listening(d.transcript())
            .on_request(move |label, _, cx| {
                on_req.update(cx, |d, cx| {
                    d.touched = true;
                    d.request(label);
                    cx.notify();
                })
            })
            .on_apply(move |text, _, cx| {
                on_apply.update(cx, |d, cx| {
                    d.touched = true;
                    d.selection = text;
                    cx.notify();
                })
            })
            .on_listen(move |on, _, cx| {
                on_listen.update(cx, |d, cx| {
                    d.touched = true;
                    d.listening = on.then(Instant::now);
                    cx.notify();
                })
            })
            .on_close(move |_, cx| {
                on_close.update(cx, |d, cx| {
                    d.touched = true;
                    d.versions.clear();
                    d.listening = None;
                    cx.notify();
                })
            })
    })
}
