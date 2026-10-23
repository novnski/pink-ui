//! Notification Stack – notices stacked behind each other: dismiss the front
//! one to reveal the next.
//!
//! Every card is PINK's composer notice (Pink: notice.rs `notice_chip`): an
//! opaque plate on the stack ladder's notice step (r14), a title row with a
//! 14 px glyph, the medium label and the actions riding it, and the message on
//! the line below, all at 12/16. Warning and danger notices tint the plate
//! with their tone flattened over the canvas and set their copy in it; info
//! notices are the quiet neutral notice (Pink: composer.rs
//! `composer-recover-unsent`). Actions are Pink's 24 px capsules
//! (`notice_action`); the 20 px button in the copy button's slot dismisses the
//! card, with the board notice's close glyph (Pink: board/page.rs).
//!
//! PINK has no deck of notices, so the stacking borrows its one tucked-behind
//! pattern, the queue tray behind the composer (Pink: composer.rs
//! `QUEUE_SIDE_INSET`): each card behind is 16 px narrower per side and shows
//! a 10 px band below the one in front, its contents hidden. Dismissing the
//! front card fades it out with a short drift, above the next one gliding
//! forward and showing its contents.
//!
//! ```ignore
//! NotificationStack::new("notices", vec![
//!     Notice::new("push", "Action failed", "The remote rejected the push.")
//!         .tone(NoticeTone::Danger)
//!         .action(NoticeAction::new("Retry").on_click(|_, _| {})),
//! ])
//! .on_dismiss(|notice, _, _| println!("dismissed {}", notice.id))
//! ```

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};
use web_time::Duration;

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, Hsla, IntoElement, Role,
    SharedString, Task, Window, canvas, div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    motion::{self, MotionSpec},
    pink::{self, FillTier, Pink, TextStyle, TypeStyleExt, icon_size, layout, radius, space},
    story,
};

pub const NAME: &str = "Notification Stack";
pub const DESCRIPTION: &str = "Notices piled up above the composer – plates tinted by their tone with a glyph, a label, a capsule action and the message, the ones behind peeking out below the front card until you dismiss it";

const ICON_INFO: &str = "icons/notification-stack/info-circle.svg";
const ICON_DANGER: &str = "icons/notification-stack/danger-triangle.svg";
const ICON_CLOSE: &str = "icons/notification-stack/close.svg";

/// How far each card behind shows below the one in front.
const PEEK: f32 = space::S10;
/// How much narrower each card behind is, per side (Pink: composer.rs
/// `QUEUE_SIDE_INSET`, the tray tucked behind the composer).
const SIDE_INSET: f32 = 16.;
/// Cards behind the front one that still show; the rest wait unseen.
const MAX_BEHIND: usize = 2;
/// The notice plate's padding and row gap (Pink: notice.rs `notice_chip`).
const PAD_X: f32 = space::S10;
const PAD_Y: f32 = space::S8;
const ROW_GAP: f32 = space::S6;
/// The trailing button in the title row (Pink: `notice_chip` copy button).
const TRAILING_BUTTON: f32 = 20.;

/// Cards moving forward and the stack's height: a list reorder.
const STACK: MotionSpec = motion::RESORT;
/// The dismissed card leaving: the exit counterpart of a toast's entrance.
const LEAVE: MotionSpec = motion::exit_of(motion::STANDARD);
/// How far the dismissed card drifts as it fades (a toast's travel).
const LEAVE_DRIFT: f32 = motion::TOAST_IN_RISE;
/// Cards past the third showing or hiding.
const CARD_FADE: MotionSpec = motion::FADE_QUICK;
/// A card's contents showing as it reaches the front: an in-place swap.
const CONTENT_FADE: MotionSpec = motion::TRANSCRIPT_IN;

type Handler = Rc<dyn Fn(&mut Window, &mut App)>;
type DismissHandler = Rc<dyn Fn(&Notice, &mut Window, &mut App)>;

/// What a notice is about (Pink: board/page.rs `NoticeTone`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoticeTone {
    /// Neutral news: the quiet card with an info glyph.
    #[default]
    Info,
    /// Something to keep an eye on: amber plate.
    Warning,
    /// Something failed: red plate.
    Danger,
}

/// A capsule button in the notice's title row.
#[derive(Clone)]
pub struct NoticeAction {
    pub label: SharedString,
    pub on_click: Option<Handler>,
}

impl NoticeAction {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            on_click: None,
        }
    }

    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

#[derive(Clone)]
pub struct Notice {
    pub id: SharedString,
    pub tone: NoticeTone,
    /// The short label beside the glyph, e.g. "Action failed".
    pub title: SharedString,
    /// The message on the line below.
    pub text: SharedString,
    pub action: Option<NoticeAction>,
}

impl Notice {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        text: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            tone: NoticeTone::default(),
            title: title.into(),
            text: text.into(),
            action: None,
        }
    }

    pub fn tone(mut self, tone: NoticeTone) -> Self {
        self.tone = tone;
        self
    }

    pub fn action(mut self, action: NoticeAction) -> Self {
        self.action = Some(action);
        self
    }
}

/// One notice's paint, by tone.
struct NoticeColors {
    plate: Hsla,
    edge: Hsla,
    glyph: Hsla,
    label: Hsla,
    copy: Hsla,
    /// The dismiss button's glyph and its hover wash.
    close: Hsla,
    close_wash: Hsla,
}

impl NoticeColors {
    fn get(tone: NoticeTone, t: &Pink) -> Self {
        let (accent, muted) = match tone {
            // Pink: composer.rs `composer-recover-unsent`: the card plate and
            // edge, a `text_muted` glyph and message under a `text` label,
            // `glass_hover` washes
            NoticeTone::Info => {
                return Self {
                    plate: t.surface_card,
                    edge: t.border,
                    glyph: t.text_muted,
                    label: t.text,
                    copy: t.text_muted,
                    close: t.readable_muted(0.8),
                    close_wash: t.fill(FillTier::Tertiary),
                };
            }
            NoticeTone::Warning => (t.warning, t.warning_muted),
            NoticeTone::Danger => (t.danger, t.danger_muted),
        };
        // Pink: notice.rs `notice_chip`: the tint flattened over the canvas,
        // so the cards behind never print through
        let copy = t.readable_tone(muted, 0.9);
        Self {
            plate: pink::flatten(accent.opacity(0.08), t.canvas()),
            edge: accent.opacity(0.22),
            glyph: copy,
            label: copy,
            copy,
            close: t.readable_tone(muted, 0.8),
            close_wash: accent.opacity(0.12),
        }
    }
}

fn tone_icon(tone: NoticeTone) -> &'static str {
    match tone {
        NoticeTone::Info => ICON_INFO,
        NoticeTone::Warning | NoticeTone::Danger => ICON_DANGER,
    }
}

/// UI state that lives across renders.
struct StackState {
    items: Vec<Notice>,
    /// Dismissed, still fading out.
    leaving: HashSet<SharedString>,
    /// Each card's natural height (border included), measured last frame.
    heights: HashMap<SharedString, f32>,
}

#[derive(IntoElement)]
pub struct NotificationStack {
    id: ElementId,
    notices: Vec<Notice>,
    on_dismiss: Option<DismissHandler>,
}

impl NotificationStack {
    /// The stack keeps its own copy of `notices` from the first render on.
    pub fn new(id: impl Into<ElementId>, notices: Vec<Notice>) -> Self {
        Self {
            id: id.into(),
            notices,
            on_dismiss: None,
        }
    }

    /// Called after a notice was dismissed.
    pub fn on_dismiss(
        mut self,
        handler: impl Fn(&Notice, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

/// Where one card sits this frame.
struct Placement {
    /// Offset from the stack's top, px.
    y: f32,
    /// Horizontal drift, px (only while leaving).
    x: f32,
    /// Inset from each side, px.
    inset: f32,
    /// The plate's height, once measured.
    height: Option<f32>,
    opacity: f32,
    /// The contents' opacity: only the front card shows them.
    content: f32,
}

impl RenderOnce for NotificationStack {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();

        let state = {
            let notices = self.notices.clone();
            window.use_keyed_state(
                ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
                cx,
                move |_, _| StackState {
                    items: notices,
                    leaving: HashSet::new(),
                    heights: HashMap::new(),
                },
            )
        };
        let (items, leaving, heights) = {
            let s = state.read(cx);
            (s.items.clone(), s.leaving.clone(), s.heights.clone())
        };

        let visible: Vec<&Notice> = items.iter().filter(|n| !leaving.contains(&n.id)).collect();
        let behind = visible.len().saturating_sub(1).min(MAX_BEHIND);
        // Until every card has been measured once, place them without easing
        let ready = items.iter().all(|n| heights.contains_key(&n.id));
        let front = visible
            .first()
            .and_then(|n| heights.get(&n.id))
            .copied()
            .unwrap_or(0.);
        let stack_target = if visible.is_empty() {
            0.
        } else {
            front + behind as f32 * PEEK
        };
        let ease = |name: &str, id: &str, target: f32, spec: MotionSpec| {
            if ready {
                motion::follow(&format!("{key}-{id}-{name}"), target, spec)
            } else {
                target
            }
        };
        let stack_height = ease("stack", "", stack_target, STACK);

        let dismiss: DismissHandler = {
            let state = state.clone();
            let on_dismiss = self.on_dismiss.clone();
            Rc::new(move |notice: &Notice, window: &mut Window, cx: &mut App| {
                let id = notice.id.clone();
                state.update(cx, |s, cx| {
                    if !s.leaving.insert(id.clone()) {
                        return;
                    }
                    let delay = LEAVE.total().mul_f32(motion::speed_scale());
                    // Holds only a weak handle: harmless if the stack unmounts first
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(delay).await;
                        this.update(cx, |s, cx| {
                            s.items.retain(|n| n.id != id);
                            s.leaving.remove(&id);
                            cx.notify();
                        })
                        .ok();
                    })
                    .detach();
                    cx.notify();
                });
                if let Some(f) = &on_dismiss {
                    f(notice, window, cx);
                }
            })
        };

        let mut cards: Vec<(i32, AnyElement)> = items
            .iter()
            .map(|notice| {
                let depth = visible.iter().position(|n| n.id == notice.id);
                let gone = depth.is_none();
                let depth = depth.unwrap_or(0);
                let shown = depth.min(MAX_BEHIND) as f32;
                let natural = heights.get(&notice.id).copied();
                let id = notice.id.as_ref();
                let spec = if gone { LEAVE } else { STACK };
                // Cards behind take the front card's height, so only their
                // bottom band shows below it
                let height = if gone || depth == 0 {
                    natural
                } else {
                    Some(front)
                };
                let place = Placement {
                    y: ease("y", id, shown * PEEK, STACK),
                    x: ease("x", id, if gone { LEAVE_DRIFT } else { 0. }, spec),
                    inset: ease("inset", id, shown * SIDE_INSET, STACK),
                    height: height
                        .filter(|_| ready)
                        .map(|h| ease("height", id, h, STACK)),
                    opacity: ease(
                        "opacity",
                        id,
                        if gone || depth > MAX_BEHIND { 0. } else { 1. },
                        if gone { LEAVE } else { CARD_FADE },
                    ),
                    content: ease(
                        "content",
                        id,
                        if gone || depth == 0 { 1. } else { 0. },
                        CONTENT_FADE,
                    ),
                };
                // The leaving card fades out above the one gliding forward
                let z = if gone { 11 } else { 10 - depth as i32 };
                let card = card(CardProps {
                    key: &key,
                    notice,
                    place,
                    interactive: !gone && depth == 0,
                    state: &state,
                    dismiss: dismiss.clone(),
                    t: &t,
                });
                (z, card.into_any_element())
            })
            .collect();
        // Paint back to front: the card in front last
        cards.sort_by_key(|(z, _)| *z);

        motion::keep_alive(window);
        div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .h(px(stack_height))
            .font_family(pink::SANS)
            .type_style(TextStyle::Footnote)
            .text_color(t.text)
            .children(cards.into_iter().map(|(_, card)| card))
    }
}

struct CardProps<'a> {
    key: &'a str,
    notice: &'a Notice,
    place: Placement,
    /// Only the front card takes clicks and focus.
    interactive: bool,
    state: &'a Entity<StackState>,
    dismiss: DismissHandler,
    t: &'a Pink,
}

/// One notice card, placed in the stack (Pink: notice.rs `notice_chip`).
fn card(props: CardProps) -> impl IntoElement {
    let CardProps {
        key,
        notice,
        place,
        interactive,
        state,
        dismiss,
        t,
    } = props;
    let c = NoticeColors::get(notice.tone, t);
    let ring = t.focus_ring();

    // Records the card's natural height for the next frame
    let measure = {
        let state = state.clone();
        let id = notice.id.clone();
        canvas(
            move |bounds, window, cx| {
                // The plate's 1 px border, top and bottom
                let h = f32::from(bounds.size.height) + 2.;
                let changed = state.update(cx, |s, _| {
                    let changed = s.heights.get(&id).is_none_or(|old| (old - h).abs() > 0.5);
                    if changed {
                        s.heights.insert(id.clone(), h);
                    }
                    changed
                });
                if changed {
                    window.request_animation_frame();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    };

    // Pink: board/page.rs `notice_strip` actions take a Ghost button; the
    // composer notice takes the capsule
    let action = notice.action.clone().map(|action| {
        pink::notice_action(format!("{key}-{}-action", notice.id), t)
            .aria_label(action.label.clone())
            .child(action.label.clone())
            .when(!interactive, |d| d.tab_stop(false))
            .when(interactive, |d| {
                d.when_some(action.on_click.clone(), |d, f| {
                    d.on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        f(window, cx)
                    })
                })
            })
    });

    // The 20 px button in `notice_chip`'s copy slot, dismissing the card
    let close_key = format!("{key}-{}-dismiss", notice.id);
    let close = div()
        .id("dismiss")
        .flex_none()
        .size(px(TRAILING_BUTTON))
        .rounded(px(radius::SM))
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(ICON_CLOSE)
                .size(px(icon_size::XS))
                .flex_none()
                .text_color(c.close),
        )
        .when(interactive, |d| {
            let notice = notice.clone();
            d.role(Role::Button)
                .aria_label("Dismiss")
                .tab_index(0)
                .cursor_pointer()
                .hover_fill(close_key, c.close_wash.opacity(0.), c.close_wash)
                .focus_visible(move |s| pink::focus_ring(s, ring))
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    dismiss(&notice, window, cx);
                })
                .on_key_down(pink::swallow_activation_key)
        });

    let title_row = div()
        .flex()
        // A narrow stack wraps the actions under the label instead of
        // pushing them past the plate
        .flex_wrap()
        .items_center()
        .gap(px(space::S8))
        .child(
            svg()
                .path(tone_icon(notice.tone))
                .size(px(icon_size::SM))
                .flex_none()
                .text_color(c.glyph),
        )
        .child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(c.label)
                .child(notice.title.clone()),
        )
        .child(div().flex_1())
        .children(action)
        .child(close);

    let body = div()
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(ROW_GAP))
        .px(px(PAD_X))
        .py(px(PAD_Y))
        .text_color(c.copy)
        .child(measure)
        .child(title_row)
        .child(div().min_w_0().w_full().child(notice.text.clone()));

    div()
        .id(ElementId::Name(format!("card-{}", notice.id).into()))
        .absolute()
        .top(px(place.y))
        .left(px(place.inset + place.x))
        .right(px(place.inset - place.x))
        .when_some(place.height, |d, h| d.h(px(h)))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(radius::STACK_NOTICE))
        .border_1()
        .border_color(c.edge)
        .bg(c.plate)
        .opacity(place.opacity)
        .when(interactive, |d| {
            d.occlude()
                .role(Role::Status)
                .aria_label(notice.title.clone())
        })
        .child(body.opacity(place.content))
}

/// A failed turn, a dropped connection, a moved ticket and an unsaved draft,
/// piling up above the composer – the demo deck.
pub fn demo_notices() -> Vec<Notice> {
    vec![
        Notice::new(
            "turn-failed",
            "Action failed",
            "The engine stopped before the turn finished. Your message is back in the draft.",
        )
        .tone(NoticeTone::Danger)
        .action(NoticeAction::new("Retry")),
        Notice::new(
            "connection",
            "Connection",
            "Engine not connected. Messages send once it's back.",
        )
        .tone(NoticeTone::Warning),
        Notice::new(
            "ticket-moved",
            "Ticket moved",
            "PINK-42 → In progress · started a new chat",
        )
        .action(NoticeAction::new("Open ticket")),
        Notice::new(
            "draft",
            "Draft not saved",
            "PINK couldn't write the draft to disk. It stays here until it saves.",
        )
        .tone(NoticeTone::Danger)
        .action(NoticeAction::new("Retry")),
    ]
}

/// One notice of each tone, shown on its own.
fn tone_samples() -> Vec<Notice> {
    vec![
        Notice::new(
            "not-delivered",
            "Not delivered",
            "The message never reached the engine.",
        )
        .tone(NoticeTone::Danger)
        .action(NoticeAction::new("Retry")),
        Notice::new(
            "checking-delivery",
            "Checking delivery",
            "The engine hasn't confirmed this message yet.",
        )
        .tone(NoticeTone::Warning),
        Notice::new(
            "unsent",
            "Unsent message saved",
            "An earlier message was not sent. Its saved copy is available.",
        )
        .action(NoticeAction::new("Restore")),
    ]
}

/// How long an emptied demo stack stays empty before it fills again.
const REFILL: Duration = Duration::from_millis(1200);

/// The demo refills a stack shortly after its last notice is dismissed.
struct Demo {
    /// Bumped to remount the deck, then the tone samples, full again.
    deck: usize,
    samples: usize,
    /// Notices still in the deck.
    remaining: usize,
    _refill_deck: Option<Task<()>>,
    _refill_samples: Option<Task<()>>,
}

/// Runs `fill` on the demo after [`REFILL`].
fn refill(cx: &mut Context<Demo>, fill: fn(&mut Demo)) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(REFILL).await;
        this.update(cx, |d, cx| {
            fill(d);
            cx.notify();
        })
        .ok();
    })
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let demo = window.use_keyed_state("notification-stack-demo", cx, |_, _| Demo {
            deck: 0,
            samples: 0,
            remaining: demo_notices().len(),
            _refill_deck: None,
            _refill_samples: None,
        });
        let (deck, samples) = {
            let d = demo.read(cx);
            (d.deck, d.samples)
        };

        let stack = NotificationStack::new(("notification-stack", deck), demo_notices())
            .on_dismiss({
                let demo = demo.clone();
                move |_, _, cx| {
                    demo.update(cx, |d, cx| {
                        d.remaining = d.remaining.saturating_sub(1);
                        if d.remaining == 0 {
                            d._refill_deck = Some(refill(cx, |d| {
                                d.remaining = demo_notices().len();
                                d.deck += 1;
                            }));
                        }
                    })
                }
            });

        let tones = tone_samples().into_iter().enumerate().map(|(ix, notice)| {
            let demo = demo.clone();
            NotificationStack::new(
                ElementId::NamedInteger(format!("tone-{ix}").into(), samples as u64),
                vec![notice],
            )
            .on_dismiss(move |_, _, cx| {
                demo.update(cx, |d, cx| {
                    d._refill_samples = Some(refill(cx, |d| d.samples += 1));
                })
            })
        });

        // A composer-side column, as wide as a dialog
        div()
            .w(px(layout::DIALOG_WIDTH))
            .flex()
            .flex_col()
            .gap(px(space::S32))
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(stack)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S8))
                    .child(
                        div()
                            .type_style(TextStyle::Footnote)
                            .text_color(t.readable_muted(0.85))
                            .child("Each tone"),
                    )
                    .children(tones),
            )
    })
}
