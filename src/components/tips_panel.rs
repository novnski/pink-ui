//! Tips Panel – a heading over a wrapping grid of tip tiles; each tile is one
//! action, and a taken tip settles into its done status.
//!
//! The panel is Pink's Board empty state (Pink: `shell/board/page.rs`
//! `BoardPage::render_empty`): a 15 semibold heading over the
//! `settings/widgets.rs` `page_subtitle`, then the tiles in one wrapping row,
//! `gap 10`, each growing from a 180 minimum. A tile is the whole click
//! target: `p 14`, radius `LG` 10, a `separator(true)` edge on a
//! `fill(Quaternary)` plate, the edge turning accent@0.45 under the pointer
//! and the 2 px inset focus ring on keyboard focus; inside, an 18 glyph in
//! `readable_muted(0.9)`, a 13 medium title and a 12/17 description in
//! `readable_muted(0.85)`. Pink sets no line height on the two titles, so
//! they keep GPUI's `phi()`. Key caps in a description are Pink's
//! `popover::kbd_hint`.
//!
//! What Pink's tiles don't have, drawn in its language: the action's verb
//! sits at the tile's top right in the description's tone, turning `text`
//! under the pointer like a tool label, its arrow in Pink's half-strength
//! trailing tile (`transcript.rs`); a taken tip swaps them for a success
//! check and its done label over `STATUS_SWAP` and stops being a button. The
//! done count shares the heading's baseline (`settings/widgets.rs`
//! `page_header`: 10 after the title, 13 in `readable_muted(0.7)`) and, with
//! `on_close`, Pink's Sm dismiss icon button (`board-notice-dismiss`) ends
//! the heading. Tiles arrive on Pink's `fade_in` (500ms expo, 4 px rise),
//! staggered like arriving tool rows (90ms, then 65ms apart).
//!
//! ```ignore
//! TipsPanel::new("tips", vec![
//!     Tip::new("search", "Quick search", "Try it")
//!         .icon("icons/tips-panel/magnifer.svg")
//!         .description("Press ")
//!         .kbd("⌘K")
//!         .description(" to jump to any page.")
//!         .done_label("Got it"),
//! ])
//! .subtitle("Small things that make the workspace faster.")
//! .on_close(|_, _| {})
//! ```

use std::rc::Rc;
use std::sync::Arc;
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Div, ElementId, Entity, FontWeight, IntoElement, Pixels, Role,
    SharedString, Stateful, Task, TextRun, Window, div, font, phi, prelude::*, px, svg,
};

use crate::{
    motion::{self, FADE_IN, STANDARD, TOOL_FIRST_ROW_DELAY_MS, TOOL_ROW_STAGGER_MS},
    pink::{self, ControlSize, FillTier, Pink, TextStyle, TypeStyleExt, layout, radius},
    story,
};

pub const NAME: &str = "Tips Panel";
pub const DESCRIPTION: &str = "Tips laid out like Pink's Board empty state: a heading with a done count over a wrapping grid of quiet tiles, each one action with its verb in the corner, and a taken tip settling into a green check and its done label";

const ICON_CHECK: &str = "icons/tips-panel/check.svg";
const ICON_CLOSE: &str = "icons/tips-panel/close.svg";
const ICON_ARROW: &str = "icons/tips-panel/arrow-right.svg";

/// GPUI's default line height (`phi()`), which Pink's heading and tile
/// titles inherit: they set sizes but no line heights.
const PHI: f32 = 1.618_034;

/// The heading (Pink `render_empty`): 15 semibold, 4 to the subtitle.
const HEADING_SIZE: f32 = 15.0;
const HEADER_GAP: f32 = 4.0;
/// The done count (Pink `page_header`): 13, 10 after the title.
const COUNT_SIZE: f32 = 13.0;
const COUNT_GAP: f32 = 10.0;
/// The board empty state's tile grid (Pink `render_empty`): the wrap gap, a
/// tile's minimum width and padding, and its glyph.
const GRID_GAP: f32 = 10.0;
const TILE_MIN_W: f32 = 180.0;
const TILE_PAD: f32 = 14.0;
const TILE_GAP: f32 = 6.0;
const TILE_GLYPH: f32 = 18.0;
/// A tile's title: 13 medium.
const TITLE_SIZE: f32 = 13.0;
/// Heading → tiles, and the section's own vertical padding.
const SECTION_GAP: f32 = 14.0;
const SECTION_PAD_Y: f32 = 16.0;
/// A tile's description: 12 / 17.
const DESC_SIZE: f32 = 12.0;
const DESC_LINE: f32 = 17.0;
/// Pink's `fade_in` rise.
const TILE_RISE: f32 = 4.0;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One run of a tip's description: plain text or a key cap.
#[derive(Clone, Debug, PartialEq)]
pub enum TipText {
    Text(SharedString),
    Kbd(SharedString),
}

#[derive(Clone)]
pub struct Tip {
    pub id: SharedString,
    pub title: SharedString,
    /// The tile's glyph (an svg asset path), drawn at 18.
    pub icon: Option<SharedString>,
    /// Short explanation, in runs so it can hold a key cap.
    pub description: Vec<TipText>,
    /// The verb clicking the tile performs, e.g. "Turn on".
    pub action: SharedString,
    /// Text after the action was taken, e.g. "Enabled" (default "Done").
    pub done_label: Option<SharedString>,
    /// Already taken (say, in an earlier session).
    pub done: bool,
    pub on_action: Option<Handler>,
}

impl Tip {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        action: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon: None,
            description: Vec::new(),
            action: action.into(),
            done_label: None,
            done: false,
            on_action: None,
        }
    }

    /// The tile's glyph: an svg path, tinted like Pink's tile glyphs.
    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    /// Appends text to the description.
    pub fn description(mut self, text: impl Into<SharedString>) -> Self {
        self.description.push(TipText::Text(text.into()));
        self
    }

    /// Appends a key cap to the description, e.g. `⌘K`.
    pub fn kbd(mut self, key: impl Into<SharedString>) -> Self {
        self.description.push(TipText::Kbd(key.into()));
        self
    }

    pub fn done_label(mut self, label: impl Into<SharedString>) -> Self {
        self.done_label = Some(label.into());
        self
    }

    /// Marks the tip as already taken.
    pub fn done(mut self, done: bool) -> Self {
        self.done = done;
        self
    }

    pub fn on_action(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(handler));
        self
    }

    /// The description as one string (key caps inline), for the tile's
    /// accessible name.
    fn plain_description(&self) -> String {
        self.description
            .iter()
            .map(|run| match run {
                TipText::Text(text) | TipText::Kbd(text) => text.as_ref(),
            })
            .collect()
    }
}

/// The tips taken in this panel, in the order they were.
struct TipsState {
    taken: Vec<SharedString>,
}

#[derive(IntoElement)]
pub struct TipsPanel {
    id: ElementId,
    title: SharedString,
    subtitle: Option<SharedString>,
    tips: Vec<Tip>,
    on_close: Option<Handler>,
    close_label: SharedString,
}

impl TipsPanel {
    pub fn new(id: impl Into<ElementId>, tips: Vec<Tip>) -> Self {
        Self {
            id: id.into(),
            title: "Tips".into(),
            subtitle: None,
            tips,
            on_close: None,
            close_label: "Dismiss".into(),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// A line under the heading (Pink `page_subtitle`).
    pub fn subtitle(mut self, subtitle: impl Into<SharedString>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Shows the dismiss button at the heading's end.
    pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(handler));
        self
    }

    /// The dismiss button's accessible name and tooltip (default "Dismiss").
    pub fn close_label(mut self, label: impl Into<SharedString>) -> Self {
        self.close_label = label.into();
        self
    }
}

impl RenderOnce for TipsPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| TipsState { taken: Vec::new() },
        );
        let taken = state.read(cx).taken.clone();
        let is_done = |tip: &Tip| tip.done || taken.contains(&tip.id);
        let done_count = self.tips.iter().filter(|tip| is_done(tip)).count();

        let tiles: Vec<AnyElement> = self
            .tips
            .iter()
            .enumerate()
            .map(|(ix, tip)| {
                let tile = tip_tile(&key, tip, is_done(tip), &state, window, &t);
                // Pink `fade_in`, staggered like arriving tool rows
                let spec =
                    FADE_IN.with_delay(TOOL_FIRST_ROW_DELAY_MS + ix as u64 * TOOL_ROW_STAGGER_MS);
                motion::rise_in(
                    ElementId::NamedInteger("tip-in".into(), ix as u64),
                    spec,
                    TILE_RISE,
                    tile,
                )
                .into_any_element()
            })
            .collect();

        // Pink `render_empty`: one wrapping row, every tile growing from 180.
        let grid = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .gap(px(GRID_GAP))
            .children(tiles);

        // Pink `page_header`: the count shares the title's baseline.
        let title = div()
            .flex_1()
            .min_w_0()
            .flex()
            .items_baseline()
            .gap(px(COUNT_GAP))
            .child(
                div()
                    .min_w_0()
                    .text_size(px(HEADING_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.text)
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(COUNT_SIZE))
                    .font_features(pink::tabular_figures())
                    .text_color(t.readable_muted(0.7))
                    .child(format!("{done_count}/{} done", self.tips.len())),
            );
        let close = self.on_close.clone().map(|on_close| {
            // Pink `board-notice-dismiss`: the Sm close icon button, centered
            // on the title's first line.
            div()
                .flex_none()
                .h(px(HEADING_SIZE * PHI))
                .flex()
                .items_center()
                .child(
                    pink::icon_button(format!("{key}-close"), ICON_CLOSE, ControlSize::Sm, &t)
                        .aria_label(self.close_label.clone())
                        .tooltip(pink::text_tooltip(self.close_label.clone()))
                        .tooltip_show_delay(layout::TOOLTIP_DELAY)
                        .on_click(move |_, window, cx| on_close(window, cx)),
                )
        });
        let heading = div()
            .flex()
            .items_start()
            .gap(px(12.0))
            .child(title)
            .children(close);
        let header = div()
            .flex()
            .flex_col()
            .gap(px(HEADER_GAP))
            .child(heading)
            .when_some(self.subtitle.clone(), |el, copy| {
                el.child(page_subtitle(copy, &t))
            });

        // Pink's page sets no line height: the titles keep GPUI's `phi()`.
        div()
            .id(self.id.clone())
            .w_full()
            .flex()
            .flex_col()
            .gap(px(SECTION_GAP))
            .py(px(SECTION_PAD_Y))
            .font_family(pink::SANS)
            .line_height(phi())
            .text_color(t.text)
            .child(header)
            .child(grid)
    }
}

/// Pink: `settings/widgets.rs` `page_subtitle`: 13/19 `readable_muted(0.9)`,
/// at most 600 wide, 6 below the heading.
fn page_subtitle(copy: SharedString, t: &Pink) -> Div {
    div()
        .mt(px(6.0))
        .max_w(px(600.0))
        .text_size(px(13.0))
        .line_height(px(19.0))
        .text_color(t.readable_muted(0.9))
        .child(copy)
}

/// One tip (Pink: `board/page.rs` `render_empty` tile). Untaken, the whole
/// tile is a button; taken, it keeps its frame and drops the interaction.
fn tip_tile(
    key: &str,
    tip: &Tip,
    done: bool,
    state: &Entity<TipsState>,
    window: &Window,
    t: &Pink,
) -> Stateful<Div> {
    let id = format!("{key}-tip-{}", tip.id);
    let group = SharedString::from(id.clone());

    let trail = if done {
        // A success glyph and word, arriving like a status swap.
        let tone = t.readable_tone(t.success, 0.9);
        let label = tip.done_label.clone().unwrap_or_else(|| "Done".into());
        motion::status_swap(
            SharedString::from(format!("{id}-done")),
            trail_row()
                .text_color(tone)
                .child(
                    svg()
                        .path(ICON_CHECK)
                        .size(px(pink::icon_size::XS))
                        .flex_none()
                        .text_color(tone),
                )
                .child(label),
        )
        .into_any_element()
    } else {
        // The verb in the description's tone, turning `text` under the
        // pointer (Pink `transcript.rs` tool label); its arrow is Pink's
        // trailing chip tile, half strength at rest and full on hover.
        let text = t.text;
        trail_row()
            .text_color(t.readable_muted(0.85))
            .group_hover(group.clone(), move |style| style.text_color(text))
            .child(tip.action.clone())
            .child(
                div()
                    .flex_none()
                    .opacity(0.5)
                    .group_hover(group.clone(), |style| style.opacity(1.0))
                    .child(
                        svg()
                            .path(ICON_ARROW)
                            .size(px(pink::icon_size::XS))
                            .flex_none()
                            .text_color(t.readable_muted(0.8)),
                    ),
            )
            .into_any_element()
    };
    let glyph_row = div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .h(px(TILE_GLYPH))
        .children(tip.icon.clone().map(|path| {
            svg()
                .path(path)
                .size(px(TILE_GLYPH))
                .flex_none()
                .text_color(t.readable_muted(0.9))
        }))
        .child(div().flex_1().min_w_0().flex().justify_end().child(trail));

    let tile = div()
        .id(SharedString::from(id))
        .group(group)
        .aria_label(SharedString::from(format!(
            "{}: {}",
            tip.title,
            tip.plain_description()
        )))
        .flex_1()
        .min_w(px(TILE_MIN_W))
        .flex()
        .flex_col()
        .gap(px(TILE_GAP))
        .p(px(TILE_PAD))
        .rounded(px(radius::LG))
        .border_1()
        .border_color(t.separator(true))
        .bg(t.fill(FillTier::Quaternary))
        .child(glyph_row)
        .child(
            div()
                .text_size(px(TITLE_SIZE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text)
                .child(tip.title.clone()),
        )
        .child(
            description(&tip.description, window, t)
                .text_size(px(DESC_SIZE))
                .line_height(px(DESC_LINE))
                .text_color(t.readable_muted(0.85)),
        );
    if done {
        return tile;
    }

    let ring = t.focus_ring();
    let hover_edge = t.accent.opacity(0.45);
    let on_action = tip.on_action.clone();
    let tip_id = tip.id.clone();
    let state = state.clone();
    tile.role(Role::Button)
        .tab_index(0)
        .cursor_pointer()
        .hover(move |style| style.border_color(hover_edge))
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, window, cx| {
            state.update(cx, |s, cx| {
                if !s.taken.contains(&tip_id) {
                    s.taken.push(tip_id.clone());
                }
                cx.notify();
            });
            if let Some(handler) = &on_action {
                handler(window, cx);
            }
        })
}

/// The tile's trailing status line: 11/14 medium, a 12 glyph beside the word.
fn trail_row() -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .whitespace_nowrap()
        .type_style(TextStyle::Caption)
        .font_weight(FontWeight::MEDIUM)
}

/// The description: plain text wraps natively; with a key cap inside, the
/// words flow as wrapping items so the cap can sit inline. Each word gets its
/// measured width and the space after it, and the row may run one space past
/// its edge so a line's trailing space hangs as in running text.
fn description(runs: &[TipText], window: &Window, t: &Pink) -> Div {
    if runs.iter().all(|run| matches!(run, TipText::Text(_))) {
        let text: String = runs
            .iter()
            .map(|run| match run {
                TipText::Text(text) | TipText::Kbd(text) => text.as_ref(),
            })
            .collect();
        return div().child(text);
    }

    // (piece, is key cap, followed by a space)
    let mut pieces: Vec<(SharedString, bool, bool)> = Vec::new();
    for run in runs {
        match run {
            TipText::Text(text) => {
                if text.starts_with(char::is_whitespace)
                    && let Some(last) = pieces.last_mut()
                {
                    last.2 = true;
                }
                let words: Vec<&str> = text.split_whitespace().collect();
                let trailing = text.ends_with(char::is_whitespace);
                for (ix, word) in words.iter().enumerate() {
                    let last = ix + 1 == words.len();
                    pieces.push((word.to_string().into(), false, !last || trailing));
                }
            }
            TipText::Kbd(key) => pieces.push((key.clone(), true, false)),
        }
    }

    let size = px(DESC_SIZE);
    let space = text_width(window, "x x", size, t) - text_width(window, "xx", size, t);
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .content_start()
        .mr(-space)
        .children(pieces.into_iter().map(|(piece, is_kbd, gap)| {
            let item = if is_kbd {
                // Pink `popover::kbd_hint`, on the Caption line so the cap
                // stays inside the description's 17 line.
                pink::kbd_hint(piece, t).line_height(px(TextStyle::Caption.line_height()))
            } else {
                let width = text_width(window, &piece, size, t);
                div().flex_none().w(width).whitespace_nowrap().child(piece)
            };
            item.when(gap, |el| el.mr(space))
        }))
}

/// The advance of `text` in Geist regular at `size`.
fn text_width(window: &Window, text: &str, size: Pixels, t: &Pink) -> Pixels {
    let run = TextRun {
        len: text.len(),
        font: font(pink::SANS),
        color: t.text,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(SharedString::from(text.to_string()), size, &[run], None)
        .width
}

/// The demo's tips; two were taken in an earlier session.
pub fn demo_tips() -> Vec<Tip> {
    vec![
        Tip::new("search", "Quick search", "Try it")
            .icon("icons/tips-panel/magnifer.svg")
            .description("Press ")
            .kbd("⌘K")
            .description(" to jump to any page, project or setting.")
            .done_label("Got it"),
        Tip::new("digest", "Weekly digest", "Turn on")
            .icon("icons/tips-panel/bell.svg")
            .description("Get a short summary of what changed every Monday morning.")
            .done_label("Turned on"),
        Tip::new("theme", "Dark mode", "Switch")
            .icon("icons/tips-panel/moon.svg")
            .description("Switch the theme or follow your system setting automatically.")
            .done_label("Switched")
            .done(true),
        Tip::new("webhook", "Channel alerts", "Connect")
            .icon("icons/tips-panel/webhook.svg")
            .description("Post an update to a channel when something needs your attention.")
            .done_label("Connected"),
        Tip::new("mobile", "Mobile app", "Get the app")
            .icon("icons/tips-panel/smartphone.svg")
            .description("Check activity and approve requests while you're on the go.")
            .done_label("Link sent")
            .done(true),
        Tip::new("api", "API keys", "Create key")
            .icon("icons/tips-panel/key-minimalistic.svg")
            .description("Automate your workflow and pull data into your own tools.")
            .done_label("Key created"),
    ]
}

/// The demo's close: the panel fades out (Pink's exit of `STANDARD`), then a
/// fresh one mounts 900ms later.
struct DemoState {
    generation: u32,
    closing: Option<Instant>,
    _reopen: Option<Task<()>>,
}

impl DemoState {
    fn close(&mut self, cx: &mut gpui::Context<Self>) {
        if self.closing.is_some() {
            return;
        }
        self.closing = Some(Instant::now());
        self._reopen = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(900))
                .await;
            this.update(cx, |s, cx| {
                s.generation += 1;
                s.closing = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }
}

#[derive(IntoElement)]
struct TipsDemo;

impl RenderOnce for TipsDemo {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let demo = window.use_keyed_state("tips-demo", cx, |_, _| DemoState {
            generation: 0,
            closing: None,
            _reopen: None,
        });
        let (generation, closing) = {
            let s = demo.read(cx);
            (s.generation, s.closing)
        };
        let opacity = closing.map_or(1.0, |since| {
            let progress = motion::exit_of(STANDARD).progress_at(since.elapsed());
            if progress < 1.0 {
                window.request_animation_frame();
            }
            1.0 - progress
        });
        div().w(px(layout::PAGE_MAX_WIDTH)).opacity(opacity).child(
            TipsPanel::new(
                SharedString::from(format!("tips-panel-{generation}")),
                demo_tips(),
            )
            .title("Get more out of your workspace")
            .subtitle(
                "A few small things that save time every day. Each takes under a minute; \
                 pick any of them.",
            )
            .on_close(move |_, cx| demo.update(cx, |s, cx| s.close(cx))),
        )
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| TipsDemo)
}
