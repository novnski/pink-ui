//! Approval Card – an agent stops and asks before it acts.
//!
//! The card names the agent and the action, rates the risk on a three-step
//! meter, and previews exactly what will run (a file with line numbers and a
//! light syntax tint) above the facts that matter for the decision. A ring
//! timer counts down to an automatic deny and holds while the pointer is on
//! the card. Approve (Ctrl/⌘ ⏎) or Deny (Esc) morphs the card into a one-line
//! record: the preview fades, the height glides down, and a status disc pops
//! in with an Undo that stays for a few seconds.
//!
//! ```ignore
//! ApprovalCard::new("migration", ApprovalRequest::new("Release agent", "Run a schema migration")
//!     .risk(Risk::High)
//!     .file("0142_split_invoice_tax.sql", vec!["ALTER TABLE invoices …".into()]))
//!     .on_resolve(|decision, _, _| println!("{decision:?}"))
//! ```

use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{
    AnyView, App, Bounds, ElementId, Entity, FocusHandle, FontWeight, HighlightStyle, Hsla,
    KeyDownEvent, PathBuilder, Pixels, SharedString, StyledText, Task, Window, canvas, div, point,
    prelude::*, px,
};

use crate::{
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, oklch, radius, text},
    motion::{self, AnimationExt, EASE_OUT_QUINT, GLIDE, MENU_IN},
    story,
};

pub const NAME: &str = "Approval Card";
pub const DESCRIPTION: &str = "An agent pauses for sign-off before a risky action – a risk meter, a preview of exactly what will run, shortcuts, a countdown that holds while you read, and a quiet resolved record with undo";

const ICON_FILE: &str = "icons/approval-card/file.svg";
const ICON_CHECK: &str = "icons/approval-card/check.svg";
const ICON_DENY: &str = "icons/approval-card/deny.svg";
const ICON_PAUSE: &str = "icons/approval-card/pause.svg";
const ICON_UNDO: &str = "icons/approval-card/undo.svg";
const ICON_TIMER: &str = "icons/approval-card/timer.svg";
const ICON_ENTER: &str = "icons/approval-card/enter.svg";

/// Height of the resolved record.
const ROW_H: f32 = 58.;
const CARD_W: f32 = 520.;
const TICK: Duration = Duration::from_millis(100);

type DecisionHandler = Rc<dyn Fn(Decision, &mut Window, &mut App)>;
type Handler = Rc<dyn Fn(&mut Window, &mut App)>;

fn eid(s: impl Into<String>) -> ElementId {
    ElementId::Name(SharedString::from(s.into()))
}

/// How much could go wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    fn level(self) -> usize {
        match self {
            Risk::Low => 1,
            Risk::Medium => 2,
            Risk::High => 3,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Risk::Low => "Low risk",
            Risk::Medium => "Medium risk",
            Risk::High => "High risk",
        }
    }

    fn color(self, t: &Luna) -> Hsla {
        match self {
            Risk::Low => t.success,
            Risk::Medium => t.warning,
            Risk::High => t.danger,
        }
    }
}

/// How a request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Approved,
    Denied,
    /// The countdown ran out; treated as a deny.
    Expired,
}

impl Decision {
    fn verb(self) -> &'static str {
        match self {
            Decision::Approved => "approved",
            Decision::Denied => "denied",
            Decision::Expired => "expired",
        }
    }

    fn tone(self) -> Tone {
        match self {
            Decision::Approved => Tone::Success,
            Decision::Denied => Tone::Danger,
            Decision::Expired => Tone::Neutral,
        }
    }
}

/// What the agent wants to do.
#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    pub agent: SharedString,
    /// Hue (OKLCH degrees) of the agent's monogram.
    pub agent_hue: f32,
    pub title: SharedString,
    /// Short noun for the resolved record ("Schema migration").
    pub subject: SharedString,
    pub risk: Risk,
    pub file: Option<SharedString>,
    pub lines: Vec<SharedString>,
    pub added: u32,
    pub removed: u32,
    pub facts: Vec<(SharedString, SharedString)>,
    pub timeout: Duration,
}

impl ApprovalRequest {
    pub fn new(agent: impl Into<SharedString>, title: impl Into<SharedString>) -> Self {
        let title = title.into();
        Self {
            agent: agent.into(),
            agent_hue: 277.,
            subject: title.clone(),
            title,
            risk: Risk::Medium,
            file: None,
            lines: Vec::new(),
            added: 0,
            removed: 0,
            facts: Vec::new(),
            timeout: Duration::from_secs(60),
        }
    }

    pub fn agent_hue(mut self, degrees: f32) -> Self {
        self.agent_hue = degrees;
        self
    }

    pub fn subject(mut self, subject: impl Into<SharedString>) -> Self {
        self.subject = subject.into();
        self
    }

    pub fn risk(mut self, risk: Risk) -> Self {
        self.risk = risk;
        self
    }

    /// The file or command the action runs, shown with line numbers.
    pub fn file(mut self, name: impl Into<SharedString>, lines: Vec<SharedString>) -> Self {
        self.file = Some(name.into());
        self.lines = lines;
        self
    }

    /// Lines added and removed, shown next to the file name.
    pub fn diff(mut self, added: u32, removed: u32) -> Self {
        self.added = added;
        self.removed = removed;
        self
    }

    pub fn fact(mut self, label: impl Into<SharedString>, value: impl Into<SharedString>) -> Self {
        self.facts.push((label.into(), value.into()));
        self
    }

    /// Time until the request denies itself.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

struct CardState {
    focus: FocusHandle,
    timeout: Duration,
    left: Duration,
    paused: bool,
    decision: Option<Decision>,
    when: SharedString,
    /// Start of the last morph (resolve or undo).
    changed: Option<Instant>,
    undo_until: Option<Instant>,
    undo_window: Duration,
    /// Height of the open card, measured while it is shown.
    height: Rc<Cell<f32>>,
    /// Bumped on every resolve / undo so entrance animations replay.
    epoch: usize,
    on_resolve: Option<DecisionHandler>,
    _tick: Task<()>,
}

impl CardState {
    fn new(
        timeout: Duration,
        resolved: Option<(Decision, SharedString)>,
        undo_window: Duration,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let tick = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                if this
                    .update_in(cx, |s, window, cx| s.tick(window, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        let (decision, when) = match resolved {
            Some((d, when)) => (Some(d), when),
            None => (None, SharedString::default()),
        };
        CardState {
            focus: cx.focus_handle(),
            timeout,
            left: timeout,
            paused: false,
            decision,
            when,
            changed: None,
            undo_until: None,
            undo_window,
            height: Rc::new(Cell::new(0.)),
            epoch: 0,
            on_resolve: None,
            _tick: tick,
        }
    }

    fn tick(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.decision.is_none() {
            if !self.paused {
                self.left = self.left.saturating_sub(TICK);
                if self.left.is_zero() {
                    self.resolve(Decision::Expired, window, cx);
                }
                cx.notify();
            }
        } else if self.undo_until.is_some_and(|until| Instant::now() >= until) {
            self.undo_until = None;
            cx.notify();
        }
    }

    fn resolve(&mut self, decision: Decision, window: &mut Window, cx: &mut gpui::Context<Self>) {
        if self.decision.is_some() {
            return;
        }
        self.decision = Some(decision);
        self.when = "just now".into();
        self.changed = Some(Instant::now());
        self.undo_until = Some(Instant::now() + self.undo_window);
        self.epoch += 1;
        if let Some(f) = self.on_resolve.clone() {
            f(decision, window, cx);
        }
        cx.notify();
    }

    fn undo(&mut self, cx: &mut gpui::Context<Self>) {
        if self.decision.take().is_none() {
            return;
        }
        // Give the reviewer time to think again.
        self.left = self.left.max(Duration::from_secs(20)).min(self.timeout);
        self.changed = Some(Instant::now());
        self.undo_until = None;
        self.epoch += 1;
        cx.notify();
    }
}

/// An agent's request for sign-off, resolving into a one-line record.
#[derive(IntoElement)]
pub struct ApprovalCard {
    id: ElementId,
    request: ApprovalRequest,
    resolved: Option<(Decision, SharedString)>,
    autofocus: bool,
    undo_window: Duration,
    on_resolve: Option<DecisionHandler>,
    on_undo: Option<Handler>,
}

impl ApprovalCard {
    pub fn new(id: impl Into<ElementId>, request: ApprovalRequest) -> Self {
        Self {
            id: id.into(),
            request,
            resolved: None,
            autofocus: false,
            undo_window: Duration::from_secs(6),
            on_resolve: None,
            on_undo: None,
        }
    }

    /// Starts as a record of an earlier decision, e.g. `(Approved, "4m ago")`.
    pub fn resolved(mut self, decision: Decision, when: impl Into<SharedString>) -> Self {
        self.resolved = Some((decision, when.into()));
        self
    }

    /// Takes keyboard focus on first render, so ⏎ and Esc work right away.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// How long Undo stays after a decision.
    pub fn undo_window(mut self, window: Duration) -> Self {
        self.undo_window = window;
        self
    }

    pub fn on_resolve(mut self, f: impl Fn(Decision, &mut Window, &mut App) + 'static) -> Self {
        self.on_resolve = Some(Rc::new(f));
        self
    }

    pub fn on_undo(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_undo = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ApprovalCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let req = self.request;
        let (timeout, resolved, undo_window, autofocus) = (
            req.timeout,
            self.resolved.clone(),
            self.undo_window,
            self.autofocus,
        );
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| {
                let state = CardState::new(timeout, resolved, undo_window, window, cx);
                if autofocus {
                    window.focus(&state.focus);
                }
                state
            },
        );
        let on_resolve = self.on_resolve.clone();
        state.update(cx, |s, _| s.on_resolve = on_resolve);

        let s = state.read(cx);
        let (decision, changed, epoch, focus) = (s.decision, s.changed, s.epoch, s.focus.clone());
        let measured = s.height.get();
        let reduced = motion::reduced_motion(cx);
        let raw = changed
            .map(|c| {
                c.elapsed().as_secs_f32()
                    / (GLIDE.duration_ms as f32 / 1000. * motion::speed_scale())
            })
            .unwrap_or(1.)
            .clamp(0., 1.);
        let raw = if reduced { 1. } else { raw };
        let e = EASE_OUT_QUINT.eval(raw);
        let ramp =
            |from: f32, to: f32| EASE_OUT_QUINT.eval(((raw - from) / (to - from)).clamp(0., 1.));
        let morphing = raw < 1. && measured > 0.;

        // (height, pending opacity, record opacity); None height = natural.
        let (height, pending_o, record_o) = match (decision.is_some(), morphing) {
            (true, true) => (
                Some(motion::lerp(measured, ROW_H, e)),
                1. - (raw * 3.).min(1.),
                ramp(0.3, 1.),
            ),
            (true, false) => (Some(ROW_H), 0., 1.),
            (false, true) => (
                Some(motion::lerp(ROW_H, measured, e)),
                ramp(0.25, 1.),
                1. - (raw * 3.).min(1.),
            ),
            (false, false) => (None, 1., 0.),
        };
        if raw < 1. {
            window.request_animation_frame();
        }

        let pending = (pending_o > 0.).then(|| pending_view(&req, &key, &state, &t, cx));
        let record = (record_o > 0.).then(|| {
            record_view(
                &req,
                decision.unwrap_or(Decision::Approved),
                epoch,
                &key,
                &state,
                self.on_undo.clone(),
                &t,
                cx,
            )
        });

        let key_state = state.clone();
        let on_key = move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            let ks = &event.keystroke;
            let decision = match ks.key.as_str() {
                "enter" if ks.modifiers.platform || ks.modifiers.control => Decision::Approved,
                "escape" => Decision::Denied,
                _ => return,
            };
            key_state.update(cx, |s, cx| s.resolve(decision, window, cx));
            cx.stop_propagation();
        };
        let hover_state = state.clone();
        let focused = focus.is_focused(window);

        let measure = state.read(cx).height.clone();
        let content = div()
            .relative()
            .overflow_hidden()
            .when_some(height, |d, h| d.h(px(h)))
            .when_some(pending, |d, pending| {
                d.child(div().relative().opacity(pending_o).child(pending).when(
                    decision.is_none() && !morphing,
                    |d| {
                        d.child(
                            canvas(
                                move |bounds: Bounds<Pixels>, _, _| {
                                    measure.set(f32::from(bounds.size.height));
                                },
                                |_, _, _, _| {},
                            )
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                        )
                    },
                ))
            })
            .when_some(record, |d, record| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .h(px(ROW_H))
                        .opacity(record_o)
                        .child(record),
                )
            });

        luna::card(&t)
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(on_key)
            .on_hover(move |hovered, _, cx| {
                hover_state.update(cx, |s, cx| {
                    s.paused = *hovered;
                    cx.notify();
                })
            })
            .w(px(CARD_W))
            .font_family(luna::SANS)
            .text_size(px(text::BODY))
            .text_color(t.text)
            .when(focused && decision.is_none(), |d| {
                d.border_color(t.accent.opacity(0.35))
            })
            .child(content)
    }
}

/// Agent monogram.
fn monogram(name: &str, hue: f32, size: f32, t: &Luna) -> impl IntoElement {
    let (wash, ink) = if t.dark {
        (oklch(0.30, 0.055, hue), oklch(0.86, 0.10, hue))
    } else {
        (oklch(0.945, 0.035, hue), oklch(0.47, 0.13, hue))
    };
    let initials: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(radius::MD))
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(ink)
        .child(initials)
}

/// Three rising bars, lit up to the risk level.
fn risk_meter(risk: Risk, t: &Luna) -> impl IntoElement {
    let color = risk.color(t);
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(24.))
        .pl(px(8.))
        .pr(px(10.))
        .rounded(px(radius::SM))
        .bg(color.opacity(if t.dark { 0.12 } else { 0.08 }))
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(2.))
                .h(px(11.))
                .children((1..=3).map(|i| {
                    div()
                        .w(px(3.))
                        .h(px(3. + i as f32 * 2.7))
                        .rounded(px(1.))
                        .bg(if i <= risk.level() {
                            color
                        } else {
                            color.opacity(0.25)
                        })
                })),
        )
        .child(
            div()
                .text_size(px(text::CAPTION))
                .font_weight(FontWeight::MEDIUM)
                .text_color(color)
                .child(risk.label()),
        )
}

const SQL_WORDS: &[&str] = &[
    "ALTER",
    "TABLE",
    "ADD",
    "COLUMN",
    "CREATE",
    "INDEX",
    "CONCURRENTLY",
    "ON",
    "NOT",
    "NULL",
    "DEFAULT",
    "UPDATE",
    "SET",
    "WHERE",
    "BEGIN",
    "COMMIT",
];

/// A light tint: keywords, quoted strings, numbers and flags.
fn highlight(line: &str, t: &Luna) -> Vec<(std::ops::Range<usize>, HighlightStyle)> {
    let keyword = HighlightStyle {
        color: Some(t.accent),
        ..Default::default()
    };
    let string = HighlightStyle {
        color: Some(t.success),
        ..Default::default()
    };
    let faint = HighlightStyle {
        color: Some(t.text_faint),
        ..Default::default()
    };
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\'' {
            let end = line[i + 1..].find('\'').map_or(line.len(), |e| i + e + 2);
            out.push((i..end, string));
            i = end;
        } else if c == b'-' && bytes.get(i + 1) == Some(&b'-') {
            out.push((i..line.len(), faint));
            break;
        } else if c.is_ascii_alphabetic() || c == b'-' {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'-')
            {
                i += 1;
            }
            let word = &line[start..i];
            let flag = word.starts_with('-') && word.len() > 1;
            if SQL_WORDS.contains(&word) || flag {
                out.push((start..i, keyword));
            }
        } else {
            i += 1;
        }
    }
    out
}

fn pending_view(
    req: &ApprovalRequest,
    key: &str,
    state: &Entity<CardState>,
    t: &Luna,
    cx: &App,
) -> impl IntoElement {
    let s = state.read(cx);
    let (left, paused) = (s.left, s.paused);
    let frac = if req.timeout.is_zero() {
        0.
    } else {
        left.as_secs_f32() / req.timeout.as_secs_f32()
    };
    let frac = motion::follow(
        &format!("{key}-ring"),
        frac,
        motion::MotionSpec::new(TICK.as_millis() as u64 + 20, motion::LINEAR),
    );
    let secs = left.as_secs_f32().ceil() as u32;
    let urgency = if paused {
        t.text_muted
    } else if secs <= 5 {
        t.danger
    } else if secs <= 10 {
        t.warning
    } else {
        t.text_muted
    };

    let header = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(monogram(&req.agent, req.agent_hue, 34., t))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .text_color(t.text_faint)
                        .child(format!("{} wants to", req.agent)),
                )
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(21.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(req.title.clone()),
                ),
        )
        .child(risk_meter(req.risk, t));

    let preview = req.file.clone().map(|file| {
        let lines = req.lines.iter().enumerate().map(|(i, line)| {
            div()
                .flex()
                .h(px(20.))
                .items_center()
                .child(
                    div()
                        .flex_none()
                        .w(px(28.))
                        .pr(px(10.))
                        .flex()
                        .justify_end()
                        .text_color(t.text_faint.opacity(0.7))
                        .child((i + 1).to_string()),
                )
                .child(
                    div()
                        .whitespace_nowrap()
                        .text_color(t.text)
                        .child(StyledText::new(line.clone()).with_highlights(highlight(line, t))),
                )
        });
        div()
            .mt(px(14.))
            .rounded(px(radius::MD))
            .border_1()
            .border_color(t.border)
            .bg(t.fill(FillTier::Quaternary))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .h(px(32.))
                    .px(px(10.))
                    .border_b_1()
                    .border_color(t.border)
                    .child(
                        gpui::svg()
                            .path(ICON_FILE)
                            .size(px(14.))
                            .text_color(t.text_faint),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(luna::MONO)
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_muted)
                            .child(file),
                    )
                    .when(req.added + req.removed > 0, |d| {
                        d.child(
                            div()
                                .flex()
                                .gap(px(6.))
                                .font_family(luna::MONO)
                                .text_size(px(text::CAPTION))
                                .child(div().text_color(t.success).child(format!("+{}", req.added)))
                                .child(
                                    div()
                                        .text_color(t.danger)
                                        .child(format!("−{}", req.removed)),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .py(px(6.))
                    .font_family(luna::MONO)
                    .text_size(px(text::FOOTNOTE))
                    .children(lines),
            )
    });

    let facts = (!req.facts.is_empty()).then(|| {
        div()
            .mt(px(14.))
            .flex()
            .gap(px(12.))
            .children(req.facts.iter().map(|(label, value)| {
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(text::CAPTION))
                            .text_color(t.text_faint)
                            .child(label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(text::BODY))
                            .text_color(t.text)
                            .truncate()
                            .child(value.clone()),
                    )
            }))
    });

    let timer = div()
        .flex()
        .items_center()
        .gap(px(8.))
        .flex_1()
        .min_w_0()
        .child(ring_timer(frac, urgency, t.fill(FillTier::Primary), paused))
        .child(
            div()
                .text_size(px(text::FOOTNOTE))
                .text_color(urgency)
                .child(if paused {
                    "Paused while you review".to_string()
                } else {
                    format!("Auto-deny in {}:{:02}", secs / 60, secs % 60)
                }),
        );

    let (approve_state, deny_state) = (state.clone(), state.clone());
    let approve_mod = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl"
    };
    let approve_kbd = div()
        .flex_none()
        .h(px(18.))
        .px(px(5.))
        .ml(px(2.))
        .flex()
        .items_center()
        .gap(px(3.))
        .rounded(px(radius::XS))
        .bg(t.on_solid.opacity(0.14))
        .text_size(px(text::CAPTION))
        .font_family(luna::MONO)
        .text_color(t.on_solid.opacity(0.75))
        .child(approve_mod)
        .child(
            gpui::svg()
                .path(ICON_ENTER)
                .size(px(11.))
                .text_color(t.on_solid.opacity(0.75)),
        );
    let footer = div()
        .mt(px(16.))
        .pt(px(14.))
        .border_t_1()
        .border_color(t.border)
        .flex()
        .items_center()
        .gap(px(8.))
        .child(timer)
        .child(
            luna::button(
                format!("{key}-deny"),
                ButtonKind::Secondary,
                ControlSize::Lg,
                t,
            )
            .pr(px(6.))
            .child("Deny")
            .child(luna::kbd("Esc", t).ml(px(2.)))
            .on_click(move |_, window, cx| {
                deny_state.update(cx, |s, cx| s.resolve(Decision::Denied, window, cx))
            }),
        )
        .child(
            luna::button(
                format!("{key}-approve"),
                ButtonKind::Primary,
                ControlSize::Lg,
                t,
            )
            .pr(px(6.))
            .child("Approve")
            .child(approve_kbd)
            .on_click(move |_, window, cx| {
                approve_state.update(cx, |s, cx| s.resolve(Decision::Approved, window, cx))
            }),
        );

    div()
        .p(px(16.))
        .flex()
        .flex_col()
        .child(header)
        .children(preview)
        .children(facts)
        .child(footer)
}

/// A small ring that drains clockwise from twelve o'clock.
fn ring_timer(frac: f32, color: Hsla, track: Hsla, paused: bool) -> impl IntoElement {
    div()
        .flex_none()
        .relative()
        .size(px(18.))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let center = bounds.center();
                    let r = f32::from(bounds.size.width) / 2. - 1.5;
                    if let Some(path) = arc(center, r, 0., 360., 1.5) {
                        window.paint_path(path, track);
                    }
                    if frac > 0.002
                        && let Some(path) = arc(center, r, 0., 360. * frac.min(1.), 1.5)
                    {
                        window.paint_path(path, color);
                    }
                },
            )
            .absolute()
            .size_full(),
        )
        .when(paused, |d| {
            d.child(
                div()
                    .absolute()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(gpui::svg().path(ICON_PAUSE).size(px(10.)).text_color(color)),
            )
        })
}

/// A stroked arc as a polyline; angles in degrees, clockwise from 12 o'clock.
fn arc(
    center: gpui::Point<Pixels>,
    r: f32,
    start: f32,
    sweep: f32,
    width: f32,
) -> Option<gpui::Path<Pixels>> {
    let steps = ((sweep.abs() / 4.).ceil() as usize).max(2);
    let mut b = PathBuilder::stroke(px(width));
    for i in 0..=steps {
        let a = (start + sweep * i as f32 / steps as f32 - 90.).to_radians();
        let p = point(center.x + px(r * a.cos()), center.y + px(r * a.sin()));
        if i == 0 {
            b.move_to(p);
        } else {
            b.line_to(p);
        }
    }
    b.build().ok()
}

#[allow(clippy::too_many_arguments)]
fn record_view(
    req: &ApprovalRequest,
    decision: Decision,
    epoch: usize,
    key: &str,
    state: &Entity<CardState>,
    on_undo: Option<Handler>,
    t: &Luna,
    cx: &App,
) -> impl IntoElement {
    let s = state.read(cx);
    let can_undo = s.undo_until.is_some();
    let when = s.when.clone();
    let color = decision.tone().color(t);
    let wash = if decision == Decision::Expired {
        t.fill(FillTier::Secondary)
    } else {
        color.opacity(if t.dark { 0.16 } else { 0.10 })
    };
    let icon = match decision {
        Decision::Approved => ICON_CHECK,
        Decision::Denied => ICON_DENY,
        Decision::Expired => ICON_TIMER,
    };
    let disc = div()
        .flex_none()
        .size(px(28.))
        .rounded_full()
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .child(gpui::svg().path(icon).size(px(15.)).text_color(color))
        .with_animation(
            eid(format!("{key}-disc-{epoch}")),
            MENU_IN.with_duration(320).with_delay(120).animation(),
            |el, p| {
                let s = 0.5 + 0.5 * motion::EASE_OUT_BACK.eval(p);
                el.opacity(p).size(px(28. * s))
            },
        );
    let detail = match decision {
        Decision::Expired => format!("No answer in time · {when}"),
        _ => format!("{} · by you · {when}", req.agent),
    };
    let undo_state = state.clone();
    div()
        .h(px(ROW_H))
        .px(px(14.))
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .flex_none()
                .w(px(28.))
                .flex()
                .justify_center()
                .child(disc),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(px(text::BODY))
                        .line_height(px(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(format!("{} {}", req.subject, decision.verb())),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .text_color(t.text_faint)
                        .child(detail),
                ),
        )
        .when(can_undo, |d| {
            d.child(motion::fade_in(
                eid(format!("{key}-undo-{epoch}")),
                luna::button(format!("{key}-undo"), ButtonKind::Ghost, ControlSize::Md, t)
                    .child(luna::glyph(ICON_UNDO, ControlSize::Md).text_color(t.text_muted))
                    .child("Undo")
                    .on_click(move |_, window, cx| {
                        undo_state.update(cx, |s, cx| s.undo(cx));
                        if let Some(f) = on_undo.as_ref() {
                            f(window, cx);
                        }
                    }),
            ))
        })
        .when(!can_undo, |d| {
            d.child(luna::badge(
                match decision {
                    Decision::Approved => "Approved",
                    Decision::Denied => "Denied",
                    Decision::Expired => "Expired",
                },
                decision.tone(),
                t,
            ))
        })
}

// ---------------------------------------------------------------------------
// Story: a release agent asking to touch production
// ---------------------------------------------------------------------------

struct Queue {
    next_visible: bool,
    _reveal: Option<Task<()>>,
}

fn migration() -> ApprovalRequest {
    ApprovalRequest::new("Release agent", "Run a schema migration on prod-eu-1")
        .subject("Schema migration")
        .agent_hue(32.)
        .risk(Risk::High)
        .file(
            "migrations/0142_split_invoice_tax.sql",
            vec![
                "ALTER TABLE invoices".into(),
                "  ADD COLUMN tax_region text NOT NULL DEFAULT 'eu';".into(),
                "CREATE INDEX CONCURRENTLY invoices_tax_region_idx".into(),
                "  ON invoices (tax_region);".into(),
            ],
        )
        .diff(18, 4)
        .fact("Target", "prod-eu-1")
        .fact("Rows touched", "≈ 1.2M")
        .fact("Rollback", "0142_down.sql")
        .timeout(Duration::from_secs(90))
}

fn key_rotation() -> ApprovalRequest {
    ApprovalRequest::new("Release agent", "Rotate the staging API keys")
        .subject("Key rotation")
        .agent_hue(32.)
        .risk(Risk::Medium)
        .file(
            "scripts/rotate-keys.sh",
            vec![
                "vault write -f auth/staging/rotate-root".into(),
                "kubectl rollout restart deploy/api -n staging".into(),
            ],
        )
        .fact("Target", "staging")
        .fact("Downtime", "None expected")
        .fact("Notifies", "#release")
        .timeout(Duration::from_secs(60))
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let queue = window.use_keyed_state("approval-card-queue", cx, |_, _| Queue {
            next_visible: false,
            _reveal: None,
        });
        let next_visible = queue.read(cx).next_visible;
        let reveal = queue.clone();
        div()
            .w(px(CARD_W))
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                ApprovalCard::new(
                    "approval-earlier",
                    ApprovalRequest::new("Release agent", "Open a release pull request")
                        .subject("Release PR #482")
                        .agent_hue(32.),
                )
                .resolved(Decision::Approved, "4m ago"),
            )
            .child(
                ApprovalCard::new("approval-migration", migration())
                    .autofocus(true)
                    .on_resolve(move |_, _, cx| {
                        reveal.update(cx, |q, cx| {
                            q._reveal = Some(cx.spawn(async move |this, cx| {
                                cx.background_executor()
                                    .timer(Duration::from_millis(900))
                                    .await;
                                this.update(cx, |q, cx| {
                                    q.next_visible = true;
                                    cx.notify();
                                })
                                .ok();
                            }));
                        })
                    }),
            )
            .when(next_visible, |d| {
                d.child(motion::rise_in(
                    "approval-next-in",
                    motion::VIEW_IN,
                    8.,
                    div().child(ApprovalCard::new("approval-keys", key_rotation())),
                ))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans<'a>(line: &'a str) -> Vec<&'a str> {
        highlight(line, &Luna::light())
            .into_iter()
            .map(|(r, _)| &line[r])
            .collect()
    }

    #[test]
    fn tints_keywords_strings_and_flags() {
        assert_eq!(
            spans("  ADD COLUMN region text DEFAULT 'eu';"),
            ["ADD", "COLUMN", "DEFAULT", "'eu'"]
        );
        assert_eq!(
            spans("kubectl rollout restart deploy/api -n staging"),
            ["-n"]
        );
    }
}
