//! Reaction Bar – a pink component.
//!
//! Like, comment, repost and share as a row of pink's quiet transcript
//! actions (pink: transcript.rs `meta_action` and the hover lane's action
//! row): 24px r6 buttons 2px apart, transparent easing to `ink(0.08)` on
//! hover, 14px Solar glyphs in `content_muted`, the inset focus ring and a
//! text tooltip. A count follows its glyph in 12px tabular figures and the
//! button widens to hold it; with nothing to count the button is pink's bare
//! 24px square. Taking an action swaps the glyph for its filled twin and
//! lifts glyph and count to `text` – pink marks a toggle by swapping the
//! glyph (Copy → Copied, the model picker's star), never with a colored
//! chip. The new glyph fades in (pink's composer glyph swap) and the new
//! count fades and settles 2px (pink's status swap) while the button eases
//! to its new width. Arrow keys, Home and End move between the actions,
//! Enter or Space takes the focused one.
//!
//! ```ignore
//! ReactionBar::new("post-reactions")
//!     .reaction(ReactionKey::Like, ReactionState::new(128))
//!     .reaction(ReactionKey::Comment, ReactionState::new(24))
//!     .on_react(|key, state, _, _| println!("{key:?} → {state:?}"))
//! ```

use std::{cell::Cell, rc::Rc, sync::Arc};

use gpui::{
    AnyElement, AnyView, App, Div, ElementId, FocusHandle, Font, Hsla, IntoElement, KeyDownEvent,
    Role, SharedString, Stateful, TextRun, Toggled, Window, div, font, prelude::*, px, relative,
    svg,
};

use crate::{
    motion::{self, AnimationExt as _},
    pink::{self, ControlSize, Pink, TextStyle, TypeStyleExt, icon_size, radius, space, text},
    story,
};

pub const NAME: &str = "Reaction Bar";
pub const DESCRIPTION: &str = "Like, comment, repost and share as quiet icon actions under a post – each count in tabular figures beside its glyph, a taken action swapping to its filled glyph as the new count settles in";

/// One action's height, and its width with nothing to count (pink:
/// transcript.rs `meta_action`).
const ACTION_SIZE: f32 = 24.0;
/// Centers the 14px glyph in the 24px square, and keeps it there when a
/// count widens the button.
const GLYPH_INSET: f32 = (ACTION_SIZE - icon_size::SM) / 2.0;
/// Glyph → count, and the count's trailing pad up to pink's small-control
/// inset (`ControlSize::Sm`: gap 4, pad 8).
const COUNT_GAP: f32 = ControlSize::Sm.gap();
const COUNT_END: f32 = ControlSize::Sm.pad_x() - GLYPH_INSET;

/// One of the four actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReactionKey {
    Like,
    Comment,
    Repost,
    Share,
}

impl ReactionKey {
    pub const ALL: [ReactionKey; 4] = [
        ReactionKey::Like,
        ReactionKey::Comment,
        ReactionKey::Repost,
        ReactionKey::Share,
    ];

    fn slug(self) -> &'static str {
        match self {
            ReactionKey::Like => "like",
            ReactionKey::Comment => "comment",
            ReactionKey::Repost => "repost",
            ReactionKey::Share => "share",
        }
    }

    /// The verb and the plural noun, e.g. "Like" / "likes".
    pub fn labels(self) -> (&'static str, &'static str) {
        match self {
            ReactionKey::Like => ("Like", "likes"),
            ReactionKey::Comment => ("Comment", "comments"),
            ReactionKey::Repost => ("Repost", "reposts"),
            ReactionKey::Share => ("Share", "shares"),
        }
    }

    /// The tooltip: the verb, or what it became once taken (pink's Copy
    /// reads "Copied").
    fn tooltip(self, active: bool) -> &'static str {
        match (self, active) {
            (ReactionKey::Like, false) => "Like",
            (ReactionKey::Like, true) => "Liked",
            (ReactionKey::Comment, false) => "Comment",
            (ReactionKey::Comment, true) => "Commented",
            (ReactionKey::Repost, false) => "Repost",
            (ReactionKey::Repost, true) => "Reposted",
            (ReactionKey::Share, false) => "Share",
            (ReactionKey::Share, true) => "Shared",
        }
    }

    /// Solar Linear glyphs; once taken, Solar Bold where the glyph has a
    /// filled twin (heart, bubble, share). The bubble is pink's own
    /// `chat-round-line`.
    fn icon(self, active: bool) -> &'static str {
        match (self, active) {
            (ReactionKey::Like, false) => "icons/reaction-bar/like.svg",
            (ReactionKey::Like, true) => "icons/reaction-bar/like-fill.svg",
            (ReactionKey::Comment, false) => "icons/reaction-bar/comment.svg",
            (ReactionKey::Comment, true) => "icons/reaction-bar/comment-fill.svg",
            (ReactionKey::Repost, _) => "icons/reaction-bar/repost.svg",
            (ReactionKey::Share, false) => "icons/reaction-bar/share.svg",
            (ReactionKey::Share, true) => "icons/reaction-bar/share-fill.svg",
        }
    }
}

/// A count and whether the viewer has taken the action.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReactionState {
    pub count: u64,
    pub active: bool,
}

impl ReactionState {
    pub fn new(count: u64) -> Self {
        Self {
            count,
            active: false,
        }
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
}

/// 1234 → "1.2K", like social apps.
pub fn short(n: u64) -> String {
    fn one_decimal(v: f64) -> String {
        let s = format!("{v:.1}");
        s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
    }
    if n >= 1_000_000 {
        format!("{}M", one_decimal(n as f64 / 1_000_000.))
    } else if n >= 10_000 {
        format!("{}K", (n as f64 / 1000.).round())
    } else if n >= 1000 {
        format!("{}K", one_decimal(n as f64 / 1000.))
    } else {
        n.to_string()
    }
}

/// Screen-reader style label, e.g. "Like, 128 likes".
pub fn aria_label(key: ReactionKey, count: u64) -> String {
    let (verb, noun) = key.labels();
    match count {
        0 => verb.to_string(),
        1 => format!("{verb}, 1 {}", &noun[..noun.len() - 1]),
        n => format!("{verb}, {n} {noun}"),
    }
}

type ReactHandler = Rc<dyn Fn(ReactionKey, ReactionState, &mut Window, &mut App)>;

struct Entry {
    key: ReactionKey,
    count: u64,
    active: bool,
    /// Toggles so far. Keys the glyph and count swaps, so only a change
    /// animates, never the first render.
    taps: u32,
    focus: FocusHandle,
}

struct BarState {
    entries: Vec<Entry>,
}

#[derive(IntoElement)]
pub struct ReactionBar {
    id: ElementId,
    reactions: Vec<(ReactionKey, ReactionState)>,
    actions: Vec<ReactionKey>,
    on_react: Option<ReactHandler>,
}

impl ReactionBar {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            reactions: Vec::new(),
            actions: ReactionKey::ALL.to_vec(),
            on_react: None,
        }
    }

    /// Starting count and whether the action is already taken (read once,
    /// when the bar first renders).
    pub fn reaction(mut self, key: ReactionKey, state: ReactionState) -> Self {
        self.reactions.retain(|(k, _)| *k != key);
        self.reactions.push((key, state));
        self
    }

    /// The actions to show, in order – hide the ones you don't need.
    pub fn actions(mut self, actions: impl IntoIterator<Item = ReactionKey>) -> Self {
        self.actions = actions.into_iter().collect();
        self
    }

    /// Called after an action was toggled, with its new state.
    pub fn on_react(
        mut self,
        handler: impl Fn(ReactionKey, ReactionState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_react = Some(Rc::new(handler));
        self
    }
}

/// The hover fade of one action (pink: `meta_action`'s `tl-{id}`).
fn fade_key(bar: &str, key: ReactionKey) -> String {
    format!("tl-{bar}-{}", key.slug())
}

/// The count's face: Geist regular with tabular figures (pink
/// `typography::tabular_figures`), so a ticking count never jitters.
fn count_font() -> Font {
    Font {
        features: pink::tabular_figures(),
        ..font(pink::SANS)
    }
}

/// The count's laid-out width, rounded up like a text box.
fn count_width(window: &Window, label: &str) -> f32 {
    let run = TextRun {
        len: label.len(),
        font: count_font(),
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let width = window
        .text_system()
        .shape_line(
            SharedString::from(label.to_string()),
            px(TextStyle::Footnote.size()),
            &[run],
            None,
        )
        .width;
    f32::from(width).ceil()
}

/// The count after the glyph: 12/16 tabular figures in the action's tone.
/// Its box eases to the new width over `QUICK` (0 when there is nothing to
/// count), and a changed count fades in and settles 2px over `STATUS_SWAP`
/// (pink: transcript.rs working-status swap).
fn count_view(window: &Window, id: &str, count: u64, taps: u32, tone: Hsla) -> Div {
    let label = (count > 0).then(|| short(count));
    let natural = label.as_deref().map_or(0., |label| {
        COUNT_GAP + count_width(window, label) + COUNT_END
    });
    let width = motion::follow(&format!("{id}-w"), natural, motion::QUICK);
    let digits = label.map(|label| -> AnyElement {
        let digits = div().flex_none().whitespace_nowrap().child(label);
        if taps == 0 {
            return digits.into_any_element();
        }
        digits
            .with_animation(
                ElementId::Name(format!("{id}-count-{taps}").into()),
                motion::STATUS_SWAP.animation(),
                |el, p| el.relative().opacity(p).top(px(2.0 * (1.0 - p))),
            )
            .into_any_element()
    });
    div()
        .flex_none()
        .w(px(width))
        .h(px(TextStyle::Footnote.line_height()))
        .overflow_hidden()
        .child(
            div()
                .flex()
                .flex_none()
                .pl(px(COUNT_GAP))
                .pr(px(COUNT_END))
                .type_style(TextStyle::Footnote)
                .font_features(pink::tabular_figures())
                .text_color(tone)
                .children(digits),
        )
}

/// One action (pink: transcript.rs `meta_action`): 24px tall, r6,
/// transparent easing to `ink(0.08)` on hover, a 14px glyph, the inset
/// focus ring and a text tooltip. Pink's square at rest; a count widens it.
/// A taken action shows its filled glyph and lifts glyph and count from
/// `content_muted` to `text`; a fresh swap fades the glyph in over
/// `GLYPH_SWAP` (pink: composer.rs `composer-primary-glyph`).
#[allow(clippy::too_many_arguments)]
fn reaction_action(
    window: &Window,
    bar: &str,
    key: ReactionKey,
    count: u64,
    active: bool,
    taps: u32,
    focus: &FocusHandle,
    t: &Pink,
) -> Stateful<Div> {
    let id = format!("{bar}-{}", key.slug());
    let fade = fade_key(bar, key);
    let ring = t.focus_ring();
    let tone = if active { t.text } else { t.content_muted };

    let glyph = svg()
        .path(key.icon(active))
        .flex_none()
        .size(px(icon_size::SM))
        .text_color(tone);
    let glyph: AnyElement = if taps == 0 {
        glyph.into_any_element()
    } else {
        div()
            .flex_none()
            .child(glyph)
            .with_animation(
                ElementId::Name(format!("{id}-glyph-{taps}").into()),
                motion::GLYPH_SWAP.animation(),
                |el, p| el.opacity(p),
            )
            .into_any_element()
    };

    div()
        .id(ElementId::Name(id.clone().into()))
        .track_focus(focus)
        .role(Role::Button)
        .aria_label(aria_label(key, count))
        .aria_toggled(if active {
            Toggled::True
        } else {
            Toggled::False
        })
        .flex_none()
        .h(px(ACTION_SIZE))
        .px(px(GLYPH_INSET))
        .flex()
        .flex_row()
        .items_center()
        .rounded(px(radius::SM))
        .cursor_pointer()
        .bg(motion::hover_blend(
            &fade,
            gpui::transparent_black(),
            t.ink(0.08),
        ))
        .on_hover(motion::hover_listener(fade))
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .tooltip(pink::text_tooltip(key.tooltip(active)))
        .child(glyph)
        .child(count_view(window, &id, count, taps, tone))
}

impl RenderOnce for ReactionBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let bar = self.id.to_string();

        let reactions = self.reactions.clone();
        let actions = self.actions.clone();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |_, cx| BarState {
                entries: actions
                    .iter()
                    .map(|&key| {
                        let start = reactions
                            .iter()
                            .find(|(r, _)| *r == key)
                            .map(|(_, s)| *s)
                            .unwrap_or_default();
                        Entry {
                            key,
                            count: start.count,
                            active: start.active,
                            taps: 0,
                            focus: cx.focus_handle().tab_stop(true),
                        }
                    })
                    .collect(),
            },
        );

        let snapshot: Vec<_> = state
            .read(cx)
            .entries
            .iter()
            .map(|e| (e.key, e.count, e.active, e.taps, e.focus.clone()))
            .collect();
        let handles: Vec<FocusHandle> = snapshot.iter().map(|e| e.4.clone()).collect();

        let buttons: Vec<_> = snapshot
            .into_iter()
            .enumerate()
            .map(|(ix, (key, count, active, taps, focus))| {
                let on_react = self.on_react.clone();
                let click = state.clone();
                // Enter and Space on a focused action click it too (GPUI);
                // like pink's meta actions, the key stops there.
                reaction_action(window, &bar, key, count, active, taps, &focus, &t)
                    .on_click(move |_, window, cx| {
                        toggle(&click, ix, on_react.as_ref(), window, cx)
                    })
                    .on_key_down(pink::swallow_activation_key)
            })
            .collect();

        // A toolbar: arrows, Home and End move focus between the actions.
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let len = handles.len();
            if len == 0 {
                return;
            }
            let current = handles.iter().position(|f| f.is_focused(window));
            let next = match event.keystroke.key.as_str() {
                "right" | "down" => current.map_or(0, |i| (i + 1) % len),
                "left" | "up" => current.map_or(len - 1, |i| (i + len - 1) % len),
                "home" => 0,
                "end" => len - 1,
                _ => return,
            };
            window.focus(&handles[next], cx);
            cx.stop_propagation();
        };

        // Followers and hover fades started in this render need frames.
        motion::keep_alive(window);

        div()
            .id(self.id.clone())
            .flex()
            .flex_row()
            .items_center()
            // pink: the hover lane's 2px action rhythm
            .gap(px(space::S2))
            .max_w_full()
            .font_family(pink::SANS)
            .on_key_down(on_key)
            .children(buttons)
    }
}

fn toggle(
    state: &gpui::Entity<BarState>,
    ix: usize,
    on_react: Option<&ReactHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let next = state.update(cx, |s, cx| {
        let entry = &mut s.entries[ix];
        let was = entry.active;
        entry.active = !was;
        entry.count = if was {
            entry.count.saturating_sub(1)
        } else {
            entry.count + 1
        };
        entry.taps += 1;
        cx.notify();
        (
            entry.key,
            ReactionState {
                count: entry.count,
                active: entry.active,
            },
        )
    });
    if let Some(handler) = on_react {
        handler(next.0, next.1, window, cx);
    }
}

// ---------------------------------------------------------------------------
// Story
// ---------------------------------------------------------------------------

/// The demo's content column.
const COLUMN: f32 = 560.;
/// Avatar plus its gap: the body lines up with the name.
const BODY_INSET: f32 = 20. + space::S8;
/// The bar sits a glyph inset further out, so the first glyph (not its
/// hover plate) lines up with the text, like pink's leading-edge menus.
const BAR_INSET: f32 = BODY_INSET - GLYPH_INSET;

struct Post {
    initial: &'static str,
    author: &'static str,
    when: &'static str,
    body: &'static str,
    reactions: [ReactionState; 4],
}

const POSTS: [Post; 3] = [
    Post {
        initial: "M",
        author: "Mara Lindqvist",
        when: "2h",
        body: "Offline mode shipped today. Drafts sync the moment you're back online, and conflicting edits open side by side instead of quietly overwriting each other.",
        reactions: [
            ReactionState {
                count: 128,
                active: true,
            },
            ReactionState {
                count: 24,
                active: false,
            },
            ReactionState {
                count: 9,
                active: false,
            },
            ReactionState {
                count: 0,
                active: false,
            },
        ],
    },
    Post {
        initial: "T",
        author: "Theo Okafor",
        when: "5h",
        body: "Small thing I love: the command palette remembers what you ran last, so ⌘K then Enter repeats it.",
        reactions: [
            ReactionState {
                count: 1240,
                active: false,
            },
            ReactionState {
                count: 86,
                active: false,
            },
            ReactionState {
                count: 31,
                active: true,
            },
            ReactionState {
                count: 12,
                active: false,
            },
        ],
    },
    Post {
        initial: "I",
        author: "Ines Duarte",
        when: "Just now",
        body: "Does anyone have a checklist for reviewing database migrations before they reach staging?",
        reactions: [
            ReactionState {
                count: 0,
                active: false,
            },
            ReactionState {
                count: 2,
                active: false,
            },
            ReactionState {
                count: 0,
                active: false,
            },
            ReactionState {
                count: 0,
                active: false,
            },
        ],
    },
];

fn bar_id(ix: usize) -> ElementId {
    ElementId::NamedInteger("rb-post".into(), ix as u64)
}

/// One post: avatar, author and age (pink: the lane's 12px `content_faint`
/// timestamp), the text in 14/22 prose, then the reactions under it.
fn post(ix: usize, post: &Post, t: &Pink) -> Div {
    let bar = ReactionKey::ALL
        .iter()
        .zip(post.reactions)
        .fold(ReactionBar::new(bar_id(ix)), |bar, (key, state)| {
            bar.reaction(*key, state)
        });
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(space::S4))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(space::S8))
                .child(pink::avatar(post.initial, t))
                .child(
                    div()
                        .type_style(TextStyle::Body)
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(post.author),
                )
                .child(
                    div()
                        .type_style(TextStyle::Footnote)
                        .text_color(t.content_faint)
                        .child(post.when),
                ),
        )
        .child(
            div()
                .pl(px(BODY_INSET))
                .text_size(px(text::PROSE))
                .line_height(px(22.))
                .text_color(t.text)
                .child(post.body),
        )
        .child(div().pl(px(BAR_INSET)).child(bar))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    // A pointer rests on Theo's Comment until the real one moves, so the
    // stage shows the hover wash.
    let resting = fade_key(&bar_id(1).to_string(), ReactionKey::Comment);
    let parked = Rc::new(Cell::new(true));
    story::view(cx, move |_, cx| {
        let t = Pink::get(cx);
        // Set once the fade entry is missing (first frame, or pruned).
        if parked.get() && motion::hover_t_or(&resting, -1.) < 0. {
            motion::set_hover(&resting, true, motion::reduced_motion(cx));
        }
        let (parked, resting) = (parked.clone(), resting.clone());
        let mut feed = div()
            .id("rb-feed")
            .w(px(COLUMN))
            .max_w(relative(1.))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_mouse_move(move |_, window, cx| {
                if parked.replace(false) {
                    motion::set_hover(&resting, false, motion::reduced_motion(cx));
                    window.refresh();
                }
            });
        for (ix, p) in POSTS.iter().enumerate() {
            if ix > 0 {
                feed = feed.child(div().py(px(space::S16)).child(pink::divider(&t)));
            }
            feed = feed.child(post(ix, p, &t));
        }
        feed
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_counts() {
        assert_eq!(short(0), "0");
        assert_eq!(short(999), "999");
        assert_eq!(short(1000), "1K");
        assert_eq!(short(1234), "1.2K");
        assert_eq!(short(12_345), "12K");
        assert_eq!(short(1_500_000), "1.5M");
        assert_eq!(short(2_000_000), "2M");
    }

    #[test]
    fn labels() {
        assert_eq!(aria_label(ReactionKey::Like, 0), "Like");
        assert_eq!(aria_label(ReactionKey::Like, 1), "Like, 1 like");
        assert_eq!(aria_label(ReactionKey::Repost, 9), "Repost, 9 reposts");
    }

    #[test]
    fn bare_action_is_pinks_square() {
        assert_eq!(GLYPH_INSET * 2. + icon_size::SM, ACTION_SIZE);
        assert_eq!(GLYPH_INSET + COUNT_END, ControlSize::Sm.pad_x());
    }
}
