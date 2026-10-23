//! Autonomy Dial – how much an agent may do without asking.
//!
//! A rotary dial with three detents (Supervised, Balanced, Autopilot) sits
//! next to two lanes, "Unattended" and "Needs approval". Every
//! action the agent can take lives in one lane; turning the dial makes the
//! actions glide across, and a lock pins an action to "Needs approval" at
//! any setting.
//!
//! Drag the knob along the arc (it snaps to the nearest detent when you let
//! go), click a stop label, or focus the dial and use the arrow keys, Home,
//! End or 1–3.
//!
//! ```ignore
//! AutonomyDial::new("repo-agent", "Repo agent", vec![
//!     AgentAction::new("read", "Read code", Risk::Read).icon("icons/autonomy-dial/read.svg"),
//!     AgentAction::new("merge", "Merge to main", Risk::Risky),
//! ])
//! .level(1)
//! .on_change(|level, _, _| println!("level {level}"))
//! ```

use std::sync::Arc;
use std::{collections::HashSet, f32::consts::PI, rc::Rc};

use gpui::{
    AnyView, App, Bounds, DispatchPhase, ElementId, Entity, FocusHandle, FontWeight, Hsla,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels,
    Point, SharedString, Window, canvas, div, point, prelude::*, px, size, svg,
};

use crate::{
    HoverFill,
    luna::{self, FillTier, Luna, Tone, oklch, radius, text},
    motion::{self, GLIDE, MotionSpec},
    story,
};

pub const NAME: &str = "Autonomy Dial";
pub const DESCRIPTION: &str = "A rotary dial for how freely an agent may act – turn it and the agent's actions glide between an unattended lane and a needs-approval lane, with locks for actions that must always wait for you";

const ICON_LOCK: &str = "icons/autonomy-dial/lock.svg";
const ICON_UNLOCK: &str = "icons/autonomy-dial/unlock.svg";

// Dial geometry (inside a DIAL_W × DIAL_H box).
const DIAL_W: f32 = 216.;
const DIAL_H: f32 = 190.;
const CX: f32 = DIAL_W / 2.;
const CY: f32 = 114.;
const R: f32 = 72.;
const START_DEG: f32 = 150.;
const SWEEP_DEG: f32 = 240.;
const STOPS: usize = 3;

// Lanes.
const PANEL_W: f32 = 248.;
const LANE_W: f32 = 240.;
const LANE_GAP: f32 = 10.;
const LANE_HEAD: f32 = 42.;
const LANE_PAD: f32 = 6.;
const ROW_H: f32 = 34.;
const ROW_GAP: f32 = 2.;

type LevelHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type LockHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// What an action can break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    /// Looks, never changes anything.
    Read,
    /// Routine, reversible changes.
    Write,
    /// Hard or impossible to take back.
    Risky,
}

impl Risk {
    /// The lowest dial level at which this action goes ahead unattended.
    fn runs_from(self) -> usize {
        match self {
            Risk::Read => 0,
            Risk::Write => 1,
            Risk::Risky => 2,
        }
    }

    fn badge(self) -> (&'static str, Tone) {
        match self {
            Risk::Read => ("read", Tone::Neutral),
            Risk::Write => ("write", Tone::Warning),
            Risk::Risky => ("critical", Tone::Danger),
        }
    }
}

/// One thing the agent can do.
#[derive(Clone, Debug)]
pub struct AgentAction {
    pub id: SharedString,
    pub label: SharedString,
    pub risk: Risk,
    pub icon: Option<SharedString>,
    /// Always asks, whatever the dial says.
    pub locked: bool,
}

impl AgentAction {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>, risk: Risk) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            risk,
            icon: None,
            locked: false,
        }
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }
}

/// The three detents.
pub const LEVELS: [(&str, &str); STOPS] = [
    ("Supervised", "Only reads happen without you"),
    ("Balanced", "Routine writes go ahead, critical ones wait"),
    ("Autopilot", "Everything goes ahead except what you lock"),
];

struct DialState {
    level: usize,
    /// Continuous dial position (0..=2) while dragging.
    value: f32,
    dragging: bool,
    locked: HashSet<SharedString>,
    focus: FocusHandle,
}

impl DialState {
    /// Moves the dial; returns the new level when it crossed a detent.
    fn set_value(&mut self, value: f32) -> Option<usize> {
        self.value = value.clamp(0., (STOPS - 1) as f32);
        self.dragging = true;
        self.set_level(self.value.round() as usize)
    }

    fn set_level(&mut self, level: usize) -> Option<usize> {
        let level = level.min(STOPS - 1);
        if level == self.level {
            return None;
        }
        self.level = level;
        Some(level)
    }
}

/// A dial for an agent's autonomy, with its actions sorted into two lanes.
#[derive(IntoElement)]
pub struct AutonomyDial {
    id: ElementId,
    agent: SharedString,
    detail: SharedString,
    hue: f32,
    actions: Vec<AgentAction>,
    level: usize,
    on_change: Option<LevelHandler>,
    on_lock: Option<LockHandler>,
}

impl AutonomyDial {
    pub fn new(
        id: impl Into<ElementId>,
        agent: impl Into<SharedString>,
        actions: Vec<AgentAction>,
    ) -> Self {
        Self {
            id: id.into(),
            agent: agent.into(),
            detail: SharedString::default(),
            hue: 277.,
            actions,
            level: 1,
            on_change: None,
            on_lock: None,
        }
    }

    /// A line under the agent's name (model, scope).
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = detail.into();
        self
    }

    /// Hue (OKLCH degrees) of the agent's monogram.
    pub fn hue(mut self, degrees: f32) -> Self {
        self.hue = degrees;
        self
    }

    /// Initial detent, 0 (Supervised) to 2 (Autopilot).
    pub fn level(mut self, level: usize) -> Self {
        self.level = level.min(STOPS - 1);
        self
    }

    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// A lock was toggled: `(action id, locked)`.
    pub fn on_lock(
        mut self,
        f: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_lock = Some(Rc::new(f));
        self
    }
}

/// Screen angle (degrees, clockwise from +x) of dial position `v`.
fn angle_of(v: f32) -> f32 {
    START_DEG + v / (STOPS - 1) as f32 * SWEEP_DEG
}

fn on_arc(center: Point<Pixels>, radius: f32, deg: f32) -> Point<Pixels> {
    let a = deg.to_radians();
    point(
        center.x + px(radius * a.cos()),
        center.y + px(radius * a.sin()),
    )
}

/// Dial position under the pointer; the gap at the bottom snaps to the
/// nearer end.
fn value_at(pos: Point<Pixels>, center: Point<Pixels>) -> f32 {
    let dx = f32::from(pos.x - center.x);
    let dy = f32::from(pos.y - center.y);
    let deg = dy.atan2(dx) * 180. / PI;
    let rel = (deg - START_DEG).rem_euclid(360.);
    let max = (STOPS - 1) as f32;
    if rel <= SWEEP_DEG {
        rel / SWEEP_DEG * max
    } else if rel - SWEEP_DEG < (360. - SWEEP_DEG) / 2. {
        max
    } else {
        0.
    }
}

fn arc_path(
    center: Point<Pixels>,
    radius: f32,
    from: f32,
    to: f32,
    width: f32,
) -> Option<gpui::Path<Pixels>> {
    let sweep = to - from;
    if sweep.abs() < 0.01 {
        return None;
    }
    let steps = ((sweep.abs() / 3.).ceil() as usize).max(2);
    let mut b = PathBuilder::stroke(px(width));
    for i in 0..=steps {
        let p = on_arc(center, radius, from + sweep * i as f32 / steps as f32);
        if i == 0 {
            b.move_to(p);
        } else {
            b.line_to(p);
        }
    }
    b.build().ok()
}

fn dot(window: &mut Window, at: Point<Pixels>, diameter: f32, color: Hsla) {
    let r = diameter / 2.;
    window.paint_quad(
        gpui::fill(
            Bounds::new(
                point(at.x - px(r), at.y - px(r)),
                size(px(diameter), px(diameter)),
            ),
            color,
        )
        .corner_radii(px(r)),
    );
}

fn emit(handler: &Option<LevelHandler>, level: Option<usize>, window: &mut Window, cx: &mut App) {
    if let (Some(f), Some(level)) = (handler, level) {
        f(level, window, cx);
    }
}

impl RenderOnce for AutonomyDial {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let (initial, initial_locked) = (
            self.level,
            self.actions
                .iter()
                .filter(|a| a.locked)
                .map(|a| a.id.clone())
                .collect::<HashSet<_>>(),
        );
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |_, cx| DialState {
                level: initial,
                value: initial as f32,
                dragging: false,
                locked: initial_locked,
                focus: cx.focus_handle(),
            },
        );
        let s = state.read(cx);
        let (level, value, dragging, focus) = (s.level, s.value, s.dragging, s.focus.clone());
        let locked = s.locked.clone();
        let focused = focus.is_focused(window);

        // While dragging the knob tracks the pointer; afterwards it glides
        // from wherever it was let go to the detent.
        let shown = if dragging {
            motion::follow(
                &format!("{key}-knob"),
                value,
                MotionSpec::new(0, motion::LINEAR),
            )
        } else {
            motion::follow(&format!("{key}-knob"), level as f32, GLIDE)
        };

        let run_count = self
            .actions
            .iter()
            .filter(|a| !locked.contains(&a.id) && a.risk.runs_from() <= level)
            .count();
        let dial = dial_view(
            &key,
            shown,
            level,
            run_count,
            self.actions.len(),
            focused,
            &state,
            self.on_change.clone(),
            &t,
        );

        let key_state = state.clone();
        let key_change = self.on_change.clone();
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let current = key_state.read(cx).level;
            let next = match event.keystroke.key.as_str() {
                "left" | "down" => current.saturating_sub(1),
                "right" | "up" => (current + 1).min(STOPS - 1),
                "home" => 0,
                "end" => STOPS - 1,
                "1" => 0,
                "2" => 1,
                "3" => 2,
                _ => return,
            };
            let changed = key_state.update(cx, |s, cx| {
                s.dragging = false;
                let changed = s.set_level(next);
                s.value = s.level as f32;
                cx.notify();
                changed
            });
            emit(&key_change, changed, window, cx);
            cx.stop_propagation();
        };

        let (wash, ink) = if t.dark {
            (oklch(0.30, 0.055, self.hue), oklch(0.86, 0.10, self.hue))
        } else {
            (oklch(0.945, 0.035, self.hue), oklch(0.47, 0.13, self.hue))
        };
        let initials: String = self
            .agent
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .map(|c| c.to_ascii_uppercase())
            .collect();

        let panel = div()
            .id("dial-panel")
            .track_focus(&focus)
            .on_key_down(on_key)
            .flex_none()
            .w(px(PANEL_W))
            .p(px(16.))
            .border_r_1()
            .border_color(t.border)
            .bg(t.fill(FillTier::Quaternary))
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex_none()
                            .size(px(32.))
                            .rounded(px(radius::MD))
                            .bg(wash)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(ink)
                            .child(initials),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(text::CALLOUT))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .line_height(px(18.))
                                    .child(self.agent.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(text::FOOTNOTE))
                                    .line_height(px(16.))
                                    .text_color(t.text_faint)
                                    .truncate()
                                    .child(self.detail.clone()),
                            ),
                    ),
            )
            .child(div().mt(px(14.)).child(dial))
            .child(
                div()
                    .mt(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .text_size(px(text::CAPTION))
                    .text_color(t.text_faint)
                    .child(luna::kbd("←", &t))
                    .child(luna::kbd("→", &t))
                    .child(div().ml(px(2.)).child("or drag the knob")),
            );

        let lanes = lanes_view(
            &key,
            &self.actions,
            level,
            &locked,
            &state,
            self.on_lock.clone(),
            &t,
        );

        let right = div()
            .flex_1()
            .p(px(16.))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .mb(px(12.))
                    .child(
                        div()
                            .text_size(px(text::CALLOUT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("What it may do"),
                    )
                    .child(motion::fade_in(
                        eid(format!("{key}-blurb-{level}")),
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_muted)
                            .child(LEVELS[level].1),
                    )),
            )
            .child(lanes);

        let el = luna::card(&t)
            .id(self.id.clone())
            .font_family(luna::SANS)
            .text_size(px(text::BODY))
            .text_color(t.text)
            .overflow_hidden()
            .flex()
            .child(panel)
            .child(right);
        motion::keep_alive(window);
        el
    }
}

/// The arc, its ticks, the knob, the stop labels and the readout.
#[allow(clippy::too_many_arguments)]
fn dial_view(
    key: &str,
    shown: f32,
    level: usize,
    runs: usize,
    total: usize,
    focused: bool,
    state: &Entity<DialState>,
    on_change: Option<LevelHandler>,
    t: &Luna,
) -> impl IntoElement {
    let (track, fill, tick, tick_on) = (
        t.fill(FillTier::Primary),
        t.accent,
        t.text_faint.opacity(0.35),
        t.accent.opacity(0.7),
    );
    let paint_state = state.clone();
    let paint_change = on_change.clone();
    let arc = canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let center = point(bounds.origin.x + px(CX), bounds.origin.y + px(CY));
            let end = START_DEG + SWEEP_DEG;
            let at = angle_of(shown);
            if let Some(p) = arc_path(center, R, START_DEG, end, 6.) {
                window.paint_path(p, track);
            }
            dot(window, on_arc(center, R, START_DEG), 6., fill);
            dot(window, on_arc(center, R, end), 6., track);
            if let Some(p) = arc_path(center, R, START_DEG, at, 6.) {
                window.paint_path(p, fill);
            }
            // Minute ticks outside the arc, longer at the detents.
            let mut lit = PathBuilder::stroke(px(1.25));
            let mut dim = PathBuilder::stroke(px(1.25));
            let count = 24;
            for i in 0..=count {
                let deg = START_DEG + SWEEP_DEG * i as f32 / count as f32;
                let detent = i % (count / (STOPS - 1)) == 0;
                let (r0, r1) = if detent {
                    (R + 9., R + 16.)
                } else {
                    (R + 10., R + 13.)
                };
                let b = if deg <= at + 0.5 { &mut lit } else { &mut dim };
                b.move_to(on_arc(center, r0, deg));
                b.line_to(on_arc(center, r1, deg));
            }
            if let Ok(p) = lit.build() {
                window.paint_path(p, tick_on);
            }
            if let Ok(p) = dim.build() {
                window.paint_path(p, tick);
            }

            // Pointer: press on the ring band, drag along it, let go to snap.
            let down_state = paint_state.clone();
            let down_change = paint_change.clone();
            window.on_mouse_event(move |ev: &MouseDownEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || ev.button != MouseButton::Left {
                    return;
                }
                let dx = f32::from(ev.position.x - center.x);
                let dy = f32::from(ev.position.y - center.y);
                let d = (dx * dx + dy * dy).sqrt();
                if !(R - 26. ..=R + 22.).contains(&d) || !bounds.contains(&ev.position) {
                    return;
                }
                let v = value_at(ev.position, center);
                let changed = down_state.update(cx, |s, cx| {
                    window.focus(&s.focus, cx);
                    let changed = s.set_value(v);
                    cx.notify();
                    changed
                });
                emit(&down_change, changed, window, cx);
            });
            let move_state = paint_state.clone();
            let move_change = paint_change.clone();
            window.on_mouse_event(move |ev: &MouseMoveEvent, phase, window, cx| {
                if phase != DispatchPhase::Bubble || !move_state.read(cx).dragging {
                    return;
                }
                if ev.pressed_button != Some(MouseButton::Left) {
                    move_state.update(cx, |s, cx| {
                        s.dragging = false;
                        s.value = s.level as f32;
                        cx.notify();
                    });
                    return;
                }
                let v = value_at(ev.position, center);
                let changed = move_state.update(cx, |s, cx| {
                    let changed = s.set_value(v);
                    cx.notify();
                    changed
                });
                emit(&move_change, changed, window, cx);
            });
            let up_state = paint_state.clone();
            window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                if phase != DispatchPhase::Bubble || !up_state.read(cx).dragging {
                    return;
                }
                up_state.update(cx, |s, cx| {
                    s.dragging = false;
                    s.value = s.level as f32;
                    cx.notify();
                });
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full();

    let knob_at = on_arc(point(px(CX), px(CY)), R, angle_of(shown));
    let knob = div()
        .absolute()
        .left(knob_at.x - px(11.))
        .top(knob_at.y - px(11.))
        .size(px(22.))
        .rounded_full()
        .bg(if t.dark {
            t.surface_raised
        } else {
            t.surface_card
        })
        .border_1()
        .border_color(if focused { t.accent } else { t.border_strong })
        .shadow(if focused {
            vec![
                crate::primitives::ring(3., t.focus_ring().opacity(0.35)),
                crate::primitives::shadow(0., 1., 3., 0., gpui::hsla(0., 0., 0., 0.25)),
            ]
        } else {
            vec![crate::primitives::shadow(
                0.,
                1.,
                3.,
                0.,
                gpui::hsla(0., 0., 0., if t.dark { 0.5 } else { 0.18 }),
            )]
        })
        .cursor_grab()
        .flex()
        .items_center()
        .justify_center()
        .child(div().size(px(8.)).rounded_full().bg(t.accent));

    // Stop labels: the top one above the arc, the ends below them.
    let labels = (0..STOPS).map(|i| {
        let deg = angle_of(i as f32);
        let anchor = on_arc(point(px(CX), px(CY)), R + 22., deg);
        let top = if i == STOPS / 2 {
            anchor.y - px(16.)
        } else {
            anchor.y + px(6.)
        };
        let label_key = format!("{key}-stop-{i}");
        let state = state.clone();
        let on_change = on_change.clone();
        let active = i == level;
        div()
            .id(eid(format!("stop-{i}")))
            .absolute()
            .left(anchor.x - px(44.))
            .top(top)
            .w(px(88.))
            .h(px(18.))
            .flex()
            .justify_center()
            .text_size(px(text::CAPTION))
            .font_weight(if active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::MEDIUM
            })
            .cursor_pointer()
            .when(active, |d| d.text_color(t.text))
            .when(!active, |d| d.hover_fg(label_key, t.text_faint, t.text))
            .on_click(move |_, window, cx| {
                let changed = state.update(cx, |s, cx| {
                    window.focus(&s.focus, cx);
                    s.dragging = false;
                    let changed = s.set_level(i);
                    s.value = s.level as f32;
                    cx.notify();
                    changed
                });
                emit(&on_change, changed, window, cx);
            })
            .child(LEVELS[i].0)
    });

    let readout = div()
        .absolute()
        .left(px(CX - 60.))
        .top(px(CY - 24.))
        .w(px(120.))
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(26.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(px(30.))
                        .child(runs.to_string()),
                )
                .child(
                    div()
                        .text_size(px(text::CALLOUT))
                        .text_color(t.text_faint)
                        .child(format!("/{total}")),
                ),
        )
        .child(
            div()
                .text_size(px(text::CAPTION))
                .line_height(px(15.))
                .text_color(t.text_muted)
                .child("act unattended"),
        );

    div()
        .relative()
        .w(px(DIAL_W))
        .h(px(DIAL_H))
        .child(arc)
        .child(readout)
        .children(labels)
        .child(knob)
}

fn lanes_view(
    key: &str,
    actions: &[AgentAction],
    level: usize,
    locked: &HashSet<SharedString>,
    state: &Entity<DialState>,
    on_lock: Option<LockHandler>,
    t: &Luna,
) -> impl IntoElement {
    let runs_on_own = |a: &AgentAction| !locked.contains(&a.id) && a.risk.runs_from() <= level;
    let counts = [
        actions.iter().filter(|a| runs_on_own(a)).count(),
        actions.iter().filter(|a| !runs_on_own(a)).count(),
    ];
    // Tall enough for the fuller lane at any setting, so turning the dial
    // never resizes the card; only a lock can.
    let fullest = (0..STOPS)
        .map(|lvl| {
            let runs = actions
                .iter()
                .filter(|a| !locked.contains(&a.id) && a.risk.runs_from() <= lvl)
                .count();
            runs.max(actions.len() - runs)
        })
        .max()
        .unwrap_or(0)
        .max(3);
    let height = motion::follow(
        &format!("{key}-lanes-h"),
        LANE_HEAD + fullest as f32 * (ROW_H + ROW_GAP) + LANE_PAD,
        GLIDE,
    );

    let lane = |i: usize, title: &'static str, color: Hsla| {
        div()
            .absolute()
            .top_0()
            .left(px(i as f32 * (LANE_W + LANE_GAP)))
            .w(px(LANE_W))
            .h(px(height))
            .rounded(px(radius::LG))
            .border_1()
            .border_color(t.border)
            .bg(t.fill(FillTier::Quaternary))
            .child(
                div()
                    .h(px(LANE_HEAD - 4.))
                    .px(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(div().size(px(6.)).rounded_full().bg(color))
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(text::FOOTNOTE))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text_muted)
                            .child(title),
                    )
                    .child(
                        div()
                            .min_w(px(20.))
                            .h(px(18.))
                            .px(px(5.))
                            .rounded(px(radius::XS))
                            .bg(t.fill(FillTier::Tertiary))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_family(luna::MONO)
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_muted)
                            .child(counts[i].to_string()),
                    ),
            )
            .when(counts[i] == 0, |d| {
                d.child(
                    div()
                        .mt(px(12.))
                        .flex()
                        .justify_center()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_faint)
                        .child(if i == 0 {
                            "Every action waits for you"
                        } else {
                            "Nothing needs approval"
                        }),
                )
            })
    };

    let mut slot = [0usize; 2];
    let rows = actions.iter().enumerate().map(|(ix, action)| {
        let lane_ix = usize::from(!runs_on_own(action));
        let pos = slot[lane_ix];
        slot[lane_ix] += 1;
        // A gentle cascade: later rows take a little longer to arrive.
        let spec = GLIDE.with_duration(380 + ix as u64 * 35);
        let x = motion::follow(
            &format!("{key}-x-{}", action.id),
            lane_ix as f32 * (LANE_W + LANE_GAP) + LANE_PAD,
            spec,
        );
        let y = motion::follow(
            &format!("{key}-y-{}", action.id),
            LANE_HEAD + pos as f32 * (ROW_H + ROW_GAP),
            spec,
        );
        action_row(
            key,
            action,
            x,
            y,
            locked.contains(&action.id),
            state,
            on_lock.clone(),
            t,
        )
    });

    div()
        .relative()
        .w(px(LANE_W * 2. + LANE_GAP))
        .h(px(height))
        .child(lane(0, "Unattended", t.success))
        .child(lane(1, "Needs approval", t.warning))
        .children(rows)
}

#[allow(clippy::too_many_arguments)]
fn action_row(
    key: &str,
    action: &AgentAction,
    x: f32,
    y: f32,
    is_locked: bool,
    state: &Entity<DialState>,
    on_lock: Option<LockHandler>,
    t: &Luna,
) -> impl IntoElement {
    let row_key = format!("{key}-row-{}", action.id);
    let lock_key = format!("{key}-lock-{}", action.id);
    let row_t = motion::hover_t(&row_key);
    let (badge, tone) = action.risk.badge();
    let id = action.id.clone();
    let state = state.clone();
    let lock_color = if is_locked {
        t.text
    } else {
        motion::hover_blend(&lock_key, t.text_faint, t.text)
    };
    div()
        .id(eid(format!("action-{}", action.id)))
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(LANE_W - LANE_PAD * 2.))
        .h(px(ROW_H))
        .pl(px(8.))
        .pr(px(4.))
        .flex()
        .items_center()
        .gap(px(8.))
        .rounded(px(radius::MD))
        .hover_fill(
            row_key,
            t.fill(FillTier::Tertiary).opacity(0.),
            t.fill(FillTier::Tertiary),
        )
        .when_some(action.icon.clone(), |d, icon| {
            d.child(
                svg()
                    .path(icon)
                    .size(px(15.))
                    .flex_none()
                    .text_color(t.text_muted),
            )
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(text::BODY))
                .child(action.label.clone()),
        )
        .child(luna::badge(badge, tone, t))
        .child(
            div()
                .id(eid(format!("lock-{}", action.id)))
                .flex_none()
                .size(px(24.))
                .rounded(px(radius::SM))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .opacity(if is_locked {
                    1.
                } else {
                    motion::lerp(0.45, 1., row_t)
                })
                .hover_fill(
                    lock_key,
                    t.fill(FillTier::Tertiary).opacity(0.),
                    t.fill(FillTier::Secondary),
                )
                .when(is_locked, |d| d.bg(t.fill(FillTier::Secondary)))
                .child(
                    svg()
                        .path(if is_locked { ICON_LOCK } else { ICON_UNLOCK })
                        .size(px(14.))
                        .text_color(lock_color),
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    let now_locked = state.update(cx, |s, cx| {
                        let now_locked = if s.locked.remove(&id) {
                            false
                        } else {
                            s.locked.insert(id.clone());
                            true
                        };
                        cx.notify();
                        now_locked
                    });
                    if let Some(f) = on_lock.as_ref() {
                        f(&id, now_locked, window, cx);
                    }
                }),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    let icon = |name: &str| SharedString::from(format!("icons/autonomy-dial/{name}.svg"));
    let actions = vec![
        AgentAction::new("read", "Read code", Risk::Read).icon(icon("read")),
        AgentAction::new("tests", "Run tests", Risk::Read).icon(icon("test")),
        AgentAction::new("comment", "Comment on PRs", Risk::Write).icon(icon("comment")),
        AgentAction::new("push", "Push branches", Risk::Write).icon(icon("push")),
        AgentAction::new("merge", "Merge to main", Risk::Risky).icon(icon("merge")),
        AgentAction::new("prune", "Delete branches", Risk::Risky).icon(icon("delete")),
        AgentAction::new("keys", "Rotate keys", Risk::Risky)
            .icon(icon("key"))
            .locked(true),
    ];
    story::view(cx, move |_, _| {
        AutonomyDial::new("autonomy-dial", "Repo agent", actions.clone())
            .detail("lumen-4 · acme/billing-api")
            .hue(200.)
            .level(1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(deg: f32) -> Point<Pixels> {
        on_arc(point(px(100.), px(100.)), 50., deg)
    }

    #[test]
    fn pointer_angle_maps_to_detents() {
        let center = point(px(100.), px(100.));
        assert!(value_at(at(START_DEG + 0.01), center).abs() < 1e-3);
        assert!((value_at(at(270.), center) - 1.).abs() < 1e-3);
        assert!((value_at(at(START_DEG + SWEEP_DEG - 0.01), center) - 2.).abs() < 1e-3);
        // The gap at the bottom snaps to the nearer end.
        assert_eq!(value_at(at(120.), center), 0.);
        assert_eq!(value_at(at(60.), center), 2.);
    }

    #[test]
    fn detents_unlock_by_risk() {
        assert_eq!(Risk::Read.runs_from(), 0);
        assert_eq!(Risk::Write.runs_from(), 1);
        assert_eq!(Risk::Risky.runs_from(), 2);
    }
}
