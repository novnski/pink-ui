//! Decision Inbox – open decisions with two quick answers each. Answering
//! one confirms it for a moment, then folds the row away, opens the next
//! folded decision and counts down until everything is caught up.
//!
//! Clean: one flat card, the count as its heading row, decisions under
//! hairlines. Classic: the white card sits on a gray tray that carries the
//! colored count. Tab moves between the answers, Enter or Space picks one.
//!
//! ```ignore
//! DecisionInbox::new("inbox", vec![
//!     Decision::new("renewal", "Renew Figma for the design team?", "Renew", "Change seats")
//!         .detail("12 seats · $1,800 per year · renews in 5 days"),
//! ])
//! .on_decide(|decision, answer, _, _| println!("{}: {answer:?}", decision.id))
//! ```

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Arc,
};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Bounds, BoxShadow, Context, ElementId, FocusHandle, FontWeight, Hsla,
    IntoElement, KeyDownEvent, MouseButton, Pixels, SharedString, Task, Transformation, Window,
    div, linear_color_stop, linear_gradient, prelude::*, px, size, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, EASE, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, shadow, white_a},
    story,
};

pub const NAME: &str = "Decision Inbox";
pub const DESCRIPTION: &str =
    "Open decisions with two quick answers each – answered ones confirm and fold away";

/// How long the confirmation stays before the row folds away.
const CONFIRM: Duration = Duration::from_millis(1100);
/// `grid-template-rows` on an answered row: 460ms.
const ROW_FOLD: MotionSpec = MotionSpec::new(460, EASE_OUT_QUINT);
/// The answered row fades while it folds: `opacity 300ms ease`.
const ROW_FADE: MotionSpec = MotionSpec::new(300, EASE);
/// A folded decision opening: 420ms.
const BODY_FOLD: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT);
/// Classic's heading turns from orange to green: `color 300ms ease`.
const HEAD_TINT: MotionSpec = MotionSpec::new(300, EASE);
/// `ui-di*-in` on the confirmation: 360ms.
const DONE_IN: MotionSpec = MotionSpec::new(360, EASE_OUT_QUINT);
/// `ui-dic-in` on the heading text: 320ms.
const HEAD_IN: MotionSpec = MotionSpec::new(320, EASE_OUT_QUINT);
/// `ui-dic-pop` on the heading glyph: 360ms.
const GLYPH_POP: MotionSpec = MotionSpec::new(360, EASE_OUT_QUINT);
/// The empty note comes in after a beat: 420ms after 300ms.
const EMPTY_IN: MotionSpec = MotionSpec::new(420, EASE_OUT_QUINT).with_delay(300);

/// Which of the two buttons answered a decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Primary,
    Secondary,
}

/// One open question.
#[derive(Clone, Debug)]
pub struct Decision {
    pub id: SharedString,
    pub title: SharedString,
    /// Short context under the title.
    pub detail: Option<SharedString>,
    /// Main answer, shown as the dark button.
    pub primary: SharedString,
    /// Other answer, shown as the light button.
    pub secondary: SharedString,
    /// Starts folded to one muted line – click to open.
    pub collapsed: bool,
}

impl Decision {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        primary: impl Into<SharedString>,
        secondary: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            detail: None,
            primary: primary.into(),
            secondary: secondary.into(),
            collapsed: false,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }

    fn label(&self, answer: Answer) -> SharedString {
        match answer {
            Answer::Primary => self.primary.clone(),
            Answer::Secondary => self.secondary.clone(),
        }
    }
}

type DecideHandler = Rc<dyn Fn(&Decision, Answer, &mut Window, &mut App)>;
type Action = Rc<dyn Fn(&mut Window, &mut App)>;
type Heights = Rc<RefCell<HashMap<String, f32>>>;

/// The `--ui-di-*` / `--ui-dic-*` tokens, plus the button and card looks.
struct Palette {
    tray: Option<Hsla>,
    card: Hsla,
    card_border: Option<Hsla>,
    card_shadow: Vec<BoxShadow>,
    line: Hsla,
    title: Hsla,
    ink: Hsla,
    detail: Hsla,
    muted: Hsla,
    /// A folded title, and its hover color.
    folded: Hsla,
    folded_hover: Hsla,
    /// Clean washes the folded row on hover.
    wash: Option<Hsla>,
    alert: Hsla,
    ok: Hsla,
    ok_bg: Option<Hsla>,
    /// The cut-out mark inside the glyphs.
    mark: Hsla,
    btn: Hsla,
    btn_hover: Hsla,
    btn_fg: Hsla,
    btn_shadow: Vec<BoxShadow>,
    btn_shadow_hover: Vec<BoxShadow>,
    /// Gradient top and bottom; Clean's solid chip has both the same.
    primary: (Hsla, Hsla),
    primary_hover: (Hsla, Hsla),
    primary_fg: Hsla,
    primary_shadow: Vec<BoxShadow>,
    /// `inset 0 1px 0` highlight on Classic's primary button.
    primary_inset: Option<Hsla>,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => {
                let solid = black_a(0.88);
                let solid_hover = black_a(0.8);
                Palette {
                    tray: None,
                    card: hex(0xffffff),
                    card_border: None,
                    card_shadow: clean_shadow(false, false),
                    line: black_a(0.06),
                    title: hex(0x000000),
                    ink: black_a(0.88),
                    detail: black_a(0.61),
                    muted: black_a(0.45),
                    folded: black_a(0.45),
                    folded_hover: black_a(0.88),
                    wash: Some(black_a(0.03)),
                    alert: hex(0xe8a20c),
                    ok: hex(0x1a9a52),
                    ok_bg: Some(hexa(0x1a9a52, 0.08)),
                    mark: hex(0xffffff),
                    btn: hex(0xffffff),
                    btn_hover: hex(0xffffff),
                    btn_fg: hex(0x000000),
                    btn_shadow: clean_shadow(false, false),
                    btn_shadow_hover: clean_shadow(false, true),
                    primary: (solid, solid),
                    primary_hover: (solid_hover, solid_hover),
                    primary_fg: hex(0xfcfcfc),
                    primary_shadow: vec![],
                    primary_inset: None,
                    focus: hexa(0x0060e2, 0.35),
                }
            }
            (Look::Clean, true) => {
                let solid = white_a(0.9);
                let solid_hover = white_a(0.82);
                Palette {
                    tray: None,
                    card: hex(0x1c1c1e),
                    card_border: None,
                    card_shadow: clean_shadow(true, false),
                    line: white_a(0.08),
                    title: hex(0xffffff),
                    ink: white_a(0.9),
                    detail: white_a(0.62),
                    muted: white_a(0.46),
                    folded: white_a(0.46),
                    folded_hover: white_a(0.9),
                    wash: Some(white_a(0.04)),
                    alert: hex(0xf0b232),
                    ok: hex(0x3ec27a),
                    ok_bg: Some(hexa(0x1a9a52, 0.14)),
                    mark: hex(0x1c1c1e),
                    btn: hex(0x1c1c1e),
                    btn_hover: hex(0x1c1c1e),
                    btn_fg: hex(0xffffff),
                    btn_shadow: clean_shadow(true, false),
                    btn_shadow_hover: clean_shadow(true, true),
                    primary: (solid, solid),
                    primary_hover: (solid_hover, solid_hover),
                    primary_fg: hex(0x161618),
                    primary_shadow: vec![],
                    primary_inset: None,
                    focus: hexa(0x4d8dff, 0.5),
                }
            }
            (Look::Classic, false) => Palette {
                tray: Some(hex(0xf0f0f0)),
                card: hex(0xffffff),
                card_border: Some(hex(0xe5e5e5)),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
                line: hex(0xf0f0f0),
                title: hex(0x171717),
                ink: hex(0x171717),
                detail: hex(0x8f8f8f),
                muted: hex(0x8f8f8f),
                folded: hex(0xb5b5b5),
                folded_hover: hex(0x8f8f8f),
                wash: None,
                alert: hex(0xe8590c),
                ok: hex(0x1f8a4c),
                ok_bg: None,
                mark: hex(0xffffff),
                btn: hex(0xffffff),
                btn_hover: hex(0xf7f7f7),
                btn_fg: hex(0x171717),
                btn_shadow: vec![
                    ring(1., hex(0xe5e5e5)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ],
                btn_shadow_hover: vec![
                    ring(1., hex(0xe5e5e5)),
                    shadow(0., 1., 2., 0., black_a(0.04)),
                ],
                primary: (hex(0x323137), hex(0x201e25)),
                primary_hover: (hex(0x3b3a41), hex(0x26242b)),
                primary_fg: hex(0xffffff),
                primary_shadow: vec![
                    ring(1., hex(0x0d0d0d)),
                    shadow(0., 2., 4., 0., black_a(0.1)),
                ],
                primary_inset: Some(white_a(0.12)),
                focus: hex(0x2f6bff),
            },
            (Look::Classic, true) => Palette {
                tray: Some(hex(0x161618)),
                card: hex(0x232326),
                card_border: Some(white_a(0.08)),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                line: white_a(0.08),
                title: hex(0xededed),
                ink: hex(0xededed),
                detail: hex(0x8f8f8f),
                muted: hex(0x8f8f8f),
                folded: hex(0x6b6b70),
                folded_hover: hex(0x8f8f8f),
                wash: None,
                alert: hex(0xfb8a3c),
                ok: hex(0x3fbf74),
                ok_bg: None,
                mark: hex(0xffffff),
                btn: hex(0x2a2a2d),
                btn_hover: hex(0x313135),
                btn_fg: hex(0xededed),
                btn_shadow: vec![ring(1., white_a(0.1)), shadow(0., 1., 2., 0., black_a(0.4))],
                btn_shadow_hover: vec![
                    ring(1., white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ],
                primary: (hex(0xf4f4f5), hex(0xe4e4e7)),
                primary_hover: (hex(0xffffff), hex(0xececef)),
                primary_fg: hex(0x121213),
                primary_shadow: vec![ring(1., black_a(0.6)), shadow(0., 2., 4., 0., black_a(0.3))],
                primary_inset: Some(hex(0xffffff)),
                focus: hex(0x2f6bff),
            },
        }
    }
}

/// `--ui-dic-shadow` / `--ui-dic-shadow-hover`: a 0.5px hairline plus a soft lift.
fn clean_shadow(dark: bool, hover: bool) -> Vec<BoxShadow> {
    match (dark, hover) {
        (false, false) => vec![
            ring(0.5, black_a(0.06)),
            shadow(0., 1., 1., -1., black_a(0.1)),
            shadow(0., 1., 2., 0., black_a(0.05)),
        ],
        (false, true) => vec![
            ring(0.5, black_a(0.1)),
            shadow(0., 1., 1., -1., black_a(0.12)),
            shadow(0., 1., 3., 0., black_a(0.07)),
        ],
        (true, false) => vec![
            ring(0.5, white_a(0.1)),
            shadow(0., 1., 2., 0., black_a(0.4)),
        ],
        (true, true) => vec![
            ring(0.5, white_a(0.16)),
            shadow(0., 1., 2., 0., black_a(0.4)),
        ],
    }
}

/// Eases a list of shadows toward another of the same shape (hover lifts).
fn mix_shadows(rest: &[BoxShadow], hover: &[BoxShadow], t: f32) -> Vec<BoxShadow> {
    rest.iter()
        .zip(hover)
        .map(|(a, b)| BoxShadow {
            color: motion::mix(a.color, b.color, t),
            offset: a.offset + (b.offset - a.offset) * t,
            blur_radius: a.blur_radius + (b.blur_radius - a.blur_radius) * t,
            spread_radius: a.spread_radius + (b.spread_radius - a.spread_radius) * t,
            inset: a.inset,
        })
        .collect()
}

/// A `0.5px` border the way browsers draw it: at least one device pixel
/// (GPUI clips thinner top borders away at 1x).
fn hairline(window: &Window) -> Pixels {
    let scale = window.scale_factor();
    px((0.5 * scale).round().max(1.) / scale)
}

/// `box-shadow: 0 0 0 <width> color`. GPUI's shadow shader draws nothing for
/// a zero blur (`primitives::ring`), so the edge gets a hair of blur – about
/// the anti-aliasing a browser gives it.
fn ring(width: f32, color: Hsla) -> BoxShadow {
    shadow(0., 0., 0.35, width, color)
}

fn eid(id: String) -> ElementId {
    ElementId::Name(id.into())
}

/// Eased progress of a timeline that started at `since`.
fn progress(since: Instant, now: Instant, spec: MotionSpec) -> f32 {
    let total = spec.total().mul_f32(motion::speed_scale()).as_secs_f32();
    if total <= 0. {
        return 1.;
    }
    let raw = now.saturating_duration_since(since).as_secs_f32() / total;
    spec.progress(raw)
}

/// Stores the natural height of the measured element's first child under `key`.
fn measure(
    heights: &Heights,
    key: String,
) -> impl Fn(Vec<Bounds<Pixels>>, &mut Window, &mut App) + use<> {
    let heights = heights.clone();
    move |bounds, window, _| {
        let Some(bounds) = bounds.first() else {
            return;
        };
        let h = f32::from(bounds.size.height);
        let old = heights.borrow_mut().insert(key.clone(), h);
        if old.is_none_or(|old| (old - h).abs() > 0.01) {
            window.request_animation_frame();
        }
    }
}

#[derive(Clone, Default)]
struct RowState {
    open: bool,
    answered: Option<SharedString>,
    gone_at: Option<Instant>,
}

struct InboxState {
    rows: HashMap<SharedString, RowState>,
    /// Decision order, for opening the next folded one.
    order: Vec<(SharedString, bool)>,
    heights: Heights,
    focus: HashMap<String, FocusHandle>,
    /// Focus came from the keyboard: show the focus ring (`:focus-visible`).
    keyboard: Rc<Cell<bool>>,
    timers: Vec<Task<()>>,
}

impl InboxState {
    fn new(decisions: &[Decision]) -> Self {
        Self {
            rows: decisions
                .iter()
                .map(|d| {
                    let row = RowState {
                        open: !d.collapsed,
                        ..Default::default()
                    };
                    (d.id.clone(), row)
                })
                .collect(),
            order: decisions
                .iter()
                .map(|d| (d.id.clone(), d.collapsed))
                .collect(),
            heights: Rc::default(),
            focus: HashMap::new(),
            keyboard: Rc::default(),
            timers: Vec::new(),
        }
    }

    fn row(&mut self, id: &SharedString) -> &mut RowState {
        let collapsed = self
            .order
            .iter()
            .find(|(x, _)| x == id)
            .is_some_and(|(_, c)| *c);
        self.rows.entry(id.clone()).or_insert_with(|| RowState {
            open: !collapsed,
            ..Default::default()
        })
    }

    fn focus_handle(&mut self, key: String, cx: &mut App) -> FocusHandle {
        self.focus
            .entry(key)
            .or_insert_with(|| cx.focus_handle().tab_stop(true))
            .clone()
    }

    /// Records the answer; false when the decision was already answered.
    fn decide(&mut self, id: SharedString, label: SharedString, cx: &mut Context<Self>) -> bool {
        let row = self.row(&id);
        if row.answered.is_some() {
            return false;
        }
        row.answered = Some(label);
        // Show the confirmation for a moment, then fold the row away and open the next folded one
        let delay = CONFIRM.mul_f32(motion::speed_scale());
        self.timers.push(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |s, cx| {
                s.row(&id).gone_at = Some(Instant::now());
                let next = s.order.iter().map(|(x, _)| x.clone()).find(|x| {
                    let row = s.rows.get(x).cloned().unwrap_or_default();
                    *x != id && !row.open && row.answered.is_none()
                });
                if let Some(next) = next {
                    s.row(&next).open = true;
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
        true
    }
}

#[derive(IntoElement)]
pub struct DecisionInbox {
    id: ElementId,
    decisions: Vec<Decision>,
    on_decide: Option<DecideHandler>,
    look: Option<Look>,
}

impl DecisionInbox {
    pub fn new(id: impl Into<ElementId>, decisions: Vec<Decision>) -> Self {
        Self {
            id: id.into(),
            decisions,
            on_decide: None,
            look: None,
        }
    }

    /// Called with the decision and the chosen answer.
    pub fn on_decide(
        mut self,
        handler: impl Fn(&Decision, Answer, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }

    /// Overrides the global Clean/Classic look for this inbox.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

impl RenderOnce for DecisionInbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx);
        let key = self.id.to_string();
        let decisions = self.decisions;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, _| InboxState::new(&decisions),
        );

        // One focus handle per control, kept across frames.
        let handles: Vec<[FocusHandle; 3]> = state.update(cx, |s, cx| {
            decisions
                .iter()
                .map(|d| {
                    ["title", "primary", "secondary"]
                        .map(|part| s.focus_handle(format!("{}-{part}", d.id), cx))
                })
                .collect()
        });
        // The card itself takes focus on click, so Tab can enter its controls
        let root_focus = state.update(cx, |s, cx| {
            s.focus
                .entry("root".into())
                .or_insert_with(|| cx.focus_handle())
                .clone()
        });
        let (rows, heights, keyboard) = {
            let s = state.read(cx);
            (s.rows.clone(), s.heights.clone(), s.keyboard.clone())
        };
        let row_of = |d: &Decision| {
            rows.get(&d.id).cloned().unwrap_or(RowState {
                open: !d.collapsed,
                ..Default::default()
            })
        };
        let open_count = decisions
            .iter()
            .filter(|d| row_of(d).answered.is_none())
            .count();
        let clear = open_count == 0;
        let hair = hairline(window);
        let now = Instant::now();
        let mut moving = false;
        // Controls in tab order (Enter / Space click the focused one – GPUI)
        let mut tab_order: Vec<FocusHandle> = Vec::new();

        let mut list = div().flex().flex_col();
        for (ix, d) in decisions.iter().enumerate() {
            let s = row_of(d);
            if let Some(gone_at) = s.gone_at
                && progress(gone_at, now, ROW_FOLD) >= 1.
                && progress(gone_at, now, ROW_FADE) >= 1.
            {
                continue;
            }
            let prev_gone = ix > 0 && row_of(&decisions[ix - 1]).gone_at.is_some();
            let [title_focus, primary_focus, secondary_focus] = handles[ix].clone();
            let focus_ring = |handle: &FocusHandle| keyboard.get() && handle.is_focused(window);

            // Title: plain text while open, a muted button while folded
            let title_size = px(if clean { 13. } else { 14. });
            let title: AnyElement = if s.open {
                div()
                    .text_size(title_size)
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.title)
                    .child(d.title.clone())
                    .into_any_element()
            } else {
                let hover_key = format!("{key}-fold-{}", d.id);
                let open: Action = {
                    let state = state.clone();
                    let id = d.id.clone();
                    Rc::new(move |_, cx| {
                        state.update(cx, |s, cx| {
                            s.row(&id).open = true;
                            cx.notify();
                        })
                    })
                };
                tab_order.push(title_focus.clone());
                let ring_on = focus_ring(&title_focus);
                div()
                    .id(eid(hover_key.clone()))
                    .track_focus(&title_focus)
                    .w_full()
                    .cursor_pointer()
                    .text_size(title_size)
                    .line_height(px(20.))
                    .map(|b| match p.wash {
                        // Clean: one muted line with its own padding and a wash on hover;
                        // the focus ring sits inside it
                        Some(wash) => b
                            .when(ring_on, |b| b.border_2().border_color(p.focus))
                            .py(px(if ring_on { 12. } else { 14. }))
                            .px(px(if ring_on { 14. } else { 16. }))
                            .font_weight(FontWeight::NORMAL)
                            .hover_fill_fg(
                                hover_key,
                                wash.opacity(0.),
                                wash,
                                p.folded,
                                p.folded_hover,
                            ),
                        None => b
                            .font_weight(FontWeight::MEDIUM)
                            .rounded(px(4.))
                            .when(ring_on, |b| b.shadow(vec![ring(2., p.focus)]))
                            .hover_fg(hover_key, p.folded, p.folded_hover),
                    })
                    .on_click(move |_, window, cx| open(window, cx))
                    .child(d.title.clone())
                    .into_any_element()
            };

            // Answers, or the confirmation in their place
            let actions = div()
                .flex()
                .items_center()
                .gap(px(8.))
                .min_h(px(if clean { 28. } else { 30. }))
                .mt(px(12.));
            let actions = match s.answered.clone() {
                Some(label) => {
                    let done = div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(p.ok)
                        .when_some(p.ok_bg, |d, bg| {
                            d.h(px(28.)).pl(px(8.)).pr(px(10.)).rounded(px(6.)).bg(bg)
                        })
                        .child(glyph(look, Glyph::Done, p.ok, p.mark, 1.))
                        .child(label);
                    actions.child(motion::rise_in(
                        eid(format!("{key}-done-{}", d.id)),
                        DONE_IN,
                        3.,
                        done,
                    ))
                }
                None => {
                    let mut buttons = Vec::new();
                    for (answer, handle) in [
                        (Answer::Primary, primary_focus),
                        (Answer::Secondary, secondary_focus),
                    ] {
                        let pick: Action = {
                            let state = state.clone();
                            let decision = d.clone();
                            let on_decide = self.on_decide.clone();
                            Rc::new(move |window, cx| {
                                let label = decision.label(answer);
                                let id = decision.id.clone();
                                let fresh = state.update(cx, |s, cx| s.decide(id, label, cx));
                                if let Some(f) = on_decide.as_ref().filter(|_| fresh) {
                                    f(&decision, answer, window, cx);
                                }
                            })
                        };
                        tab_order.push(handle.clone());
                        let hover_key = format!("{key}-{}-{answer:?}", d.id);
                        let ring_on = focus_ring(&handle);
                        buttons.push(
                            button(&p, look, answer, &hover_key, d.label(answer))
                                .track_focus(&handle)
                                // `outline: 2px; outline-offset: 1px` (Clean) / 2px (Classic)
                                .when(ring_on, |b| {
                                    let gap = if clean { 1. } else { 2. };
                                    b.shadow(vec![ring(gap + 2., p.focus), ring(gap, p.card)])
                                })
                                .on_click(move |_, window, cx| pick(window, cx)),
                        );
                    }
                    actions.children(buttons)
                }
            };

            // The body folds open from the title (420ms) and keeps 4px of room
            // around the buttons so the clip never cuts their shadows
            let body_key = format!("{key}-body-{}", d.id);
            let natural = heights.borrow().get(&body_key).copied();
            let target = if s.open { natural } else { Some(0.) };
            let body_h = target.map(|t| motion::follow(&body_key, t, BODY_FOLD));
            // Settled open: let the body size itself
            let body_h = body_h.filter(|h| !(s.open && Some(*h) == natural));
            let content = div()
                .flex_none()
                .flex()
                .flex_col()
                .when_some(d.detail.clone(), |c, detail| {
                    c.child(
                        div()
                            .mt(px(if clean { 2. } else { 4. }))
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(p.detail)
                            .child(detail),
                    )
                })
                .child(actions);
            let body = div().map(|b| match body_h {
                Some(h) => b.h(px(h)),
                None => b,
            });
            let clip = div()
                .mx(px(-4.))
                .mb(px(-4.))
                .px(px(4.))
                .pb(px(4.))
                .overflow_hidden()
                .map(|c| match body_h {
                    Some(h) => c.h(px(h + 4.)),
                    None => c,
                })
                .on_children_prepainted(measure(&heights, body_key))
                .child(content);
            let body = body.child(clip);

            let item = div()
                .flex_none()
                .flex()
                .flex_col()
                .when(ix > 0, |i| {
                    i.border_t(if clean { hair } else { px(1.) })
                        .border_color(if prev_gone {
                            p.line.opacity(0.)
                        } else {
                            p.line
                        })
                })
                .map(|i| match (clean, s.open) {
                    (true, true) => i.pt(px(14.)).px(px(16.)).pb(px(16.)),
                    (true, false) => i,
                    (false, open) => i
                        .pt(px(16.))
                        .px(px(20.))
                        .pb(px(if open { 14. } else { 16. })),
                })
                .child(title)
                .child(body);

            // Answered rows fold away (460ms) and fade (300ms)
            let row_key = format!("{key}-row-{}", d.id);
            let mut row = div().on_children_prepainted(measure(&heights, row_key.clone()));
            if let Some(gone_at) = s.gone_at {
                let natural = heights.borrow().get(&row_key).copied().unwrap_or(0.);
                let fold = progress(gone_at, now, ROW_FOLD);
                let fade = progress(gone_at, now, ROW_FADE);
                row = row
                    .overflow_hidden()
                    .h(px(natural * (1. - fold)))
                    .opacity(1. - fade);
                moving = true;
            }
            list = list.child(row.child(item));
        }

        if clear {
            list = list.child(motion::rise_in(
                eid(format!("{key}-empty")),
                EMPTY_IN,
                3.,
                div()
                    .px(px(if clean { 16. } else { 20. }))
                    .py(px(if clean { 14. } else { 18. }))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(p.muted)
                    .child("Nothing waiting on you – new decisions show up here."),
            ));
        }

        // Heading: an amber mark while something waits, a green one when all is done
        let label: SharedString = if clear {
            "All caught up".into()
        } else {
            format!(
                "{open_count} {} your input",
                if open_count == 1 { "needs" } else { "need" }
            )
            .into()
        };
        let tint = motion::follow(
            &format!("{key}-tint"),
            if clear { 1. } else { 0. },
            HEAD_TINT,
        );
        let head_color = motion::mix(p.alert, p.ok, tint);
        let mark = if clear { Glyph::Done } else { Glyph::Alert };
        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .font_weight(FontWeight::MEDIUM)
            .line_height(px(20.));
        let head = if clean {
            let color = if clear { p.ok } else { p.alert };
            // `ui-dic-pop`: the glyph grows in from 60% whenever it changes
            let pop = motion::follow_from(
                &format!("{key}-pop-{}", if clear { "done" } else { "alert" }),
                0.,
                1.,
                GLYPH_POP,
            );
            head.h(px(44.))
                .px(px(16.))
                .border_b(hair)
                .border_color(p.line)
                .text_size(px(13.))
                .text_color(p.ink)
                .child(glyph(
                    look,
                    mark,
                    color.opacity(color.a * pop),
                    p.mark.opacity(p.mark.a * pop),
                    motion::lerp(0.6, 1., pop),
                ))
                .child(motion::rise_in(
                    eid(format!("{key}-head-text")),
                    HEAD_IN,
                    3.,
                    div().child(label),
                ))
        } else {
            head.pt(px(11.))
                .px(px(18.))
                .pb(px(10.))
                .text_size(px(14.))
                .text_color(head_color)
                .child(glyph(look, mark, head_color, p.mark, 1.))
                .child(label)
        };

        if moving {
            window.request_animation_frame();
        }
        // Followers and hover fades started in this render need their frames
        motion::keep_alive(window);

        let on_key = {
            let keyboard = keyboard.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                if tab_order.is_empty() || event.keystroke.key != "tab" {
                    return;
                }
                let current = tab_order.iter().position(|f| f.is_focused(window));
                let len = tab_order.len();
                let next = match (current, event.keystroke.modifiers.shift) {
                    (Some(i), false) => (i + 1) % len,
                    (Some(i), true) => (i + len - 1) % len,
                    (None, false) => 0,
                    (None, true) => len - 1,
                };
                keyboard.set(true);
                window.focus(&tab_order[next], cx);
                cx.stop_propagation();
            }
        };

        // Classic's white card; Clean's rows lie right in the root card (a
        // fill here would square off its rounded corners – GPUI clips to rectangles)
        let card = div()
            .when_some(p.card_border, |c, border| {
                c.bg(p.card)
                    .overflow_hidden()
                    .border_1()
                    .border_color(border)
                    .rounded(px(16.))
                    .shadow(p.card_shadow.clone())
            })
            .child(list);

        let root = div()
            .id(self.id.clone())
            .track_focus(&root_focus)
            .w_full()
            .max_w(px(440.))
            .font_family(SANS)
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(p.ink)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, move |_, _, _| keyboard.set(false));
        match p.tray {
            // One white card – the edge is a shadow, not a border
            None => root
                .overflow_hidden()
                .rounded(px(10.))
                .bg(p.card)
                .shadow(p.card_shadow.clone())
                .child(head)
                .child(card),
            // The gray tray frames the white card on every side
            Some(tray) => root
                .p(px(6.))
                .rounded(px(22.))
                .bg(tray)
                .child(head)
                .child(card),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Glyph {
    Alert,
    Done,
}

/// The solid 16px glyph with its cut-out mark drawn on top in `mark`,
/// scaled by `scale` around its center.
fn glyph(look: Look, glyph: Glyph, color: Hsla, mark: Hsla, scale: f32) -> gpui::Div {
    let dir = match look {
        Look::Clean => "icons/decision-inbox/clean",
        Look::Classic => "icons/decision-inbox",
    };
    let name = match glyph {
        Glyph::Alert => "alert",
        Glyph::Done => "done",
    };
    div()
        .relative()
        .flex_none()
        .size(px(16.))
        .child(
            svg()
                .path(SharedString::from(format!("{dir}/{name}.svg")))
                .size(px(16.))
                .with_transformation(Transformation::scale(size(scale, scale)))
                .text_color(color),
        )
        .child(
            svg()
                .path(SharedString::from(format!("{dir}/{name}-mark.svg")))
                .absolute()
                .top_0()
                .left_0()
                .size(px(16.))
                .with_transformation(Transformation::scale(size(scale, scale)))
                .text_color(mark),
        )
}

/// The two answer buttons: Clean's dark chip and hairline button, Classic's
/// gradient button and white pill.
fn button(
    p: &Palette,
    look: Look,
    answer: Answer,
    hover_key: &str,
    label: SharedString,
) -> gpui::Stateful<gpui::Div> {
    let clean = look == Look::Clean;
    let t = motion::hover_t(hover_key);
    let base = div()
        .id(eid(hover_key.to_string()))
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(if clean { 28. } else { 30. }))
        .px(px(if clean { 10. } else { 12. }))
        .rounded(px(if clean { 6. } else { 9. }))
        .whitespace_nowrap()
        .text_size(px(13.))
        .line_height(px(20.))
        // Classic's button reset (`font: inherit`) wins over its weight
        .font_weight(if clean {
            FontWeight::MEDIUM
        } else {
            FontWeight::NORMAL
        })
        .cursor_pointer()
        .on_hover(motion::hover_listener(SharedString::from(
            hover_key.to_string(),
        )));
    match answer {
        Answer::Primary => {
            let top = motion::mix(p.primary.0, p.primary_hover.0, t);
            let bottom = motion::mix(p.primary.1, p.primary_hover.1, t);
            base.bg(linear_gradient(
                180.,
                linear_color_stop(top, 0.),
                linear_color_stop(bottom, 1.),
            ))
            .shadow(p.primary_shadow.clone())
            .text_color(p.primary_fg)
            // `inset 0 1px 0`: a top edge that follows the rounded corners
            .when_some(p.primary_inset, |b, inset| {
                b.border_t_1().border_color(inset)
            })
            .child(label)
        }
        Answer::Secondary => base
            .bg(motion::mix(p.btn, p.btn_hover, t))
            .shadow(mix_shadows(&p.btn_shadow, &p.btn_shadow_hover, t))
            .text_color(p.btn_fg)
            .child(label),
    }
}

/// The site demo's two decisions.
pub fn demo_decisions() -> Vec<Decision> {
    vec![
        Decision::new(
            "rollout",
            "Roll out the new onboarding to everyone?",
            "Roll out",
            "Keep testing",
        )
        .detail("Test won with +8% activation · 2 weeks of data"),
        Decision::new(
            "renewal",
            "Renew Figma for the design team?",
            "Renew",
            "Change seats",
        )
        .detail("12 seats · $1,800 per year · renews in 5 days"),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    // Like the site: once every decision is answered, the inbox starts over
    // 3.2s later with fresh state.
    let round = Rc::new(Cell::new(0usize));
    let left = Rc::new(Cell::new(demo_decisions().len()));
    story::view(cx, move |_, _| {
        let (round_, left_) = (round.clone(), left.clone());
        DecisionInbox::new(
            ElementId::named_usize("decision-inbox", round.get()),
            demo_decisions(),
        )
        .on_decide(move |_, _, _, cx| {
            if left_.get() > 1 {
                left_.set(left_.get() - 1);
                return;
            }
            let (round, left) = (round_.clone(), left_.clone());
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(3200))
                    .await;
                left.set(demo_decisions().len());
                round.set(round.get() + 1);
                cx.refresh();
            })
            .detach();
        })
    })
}
