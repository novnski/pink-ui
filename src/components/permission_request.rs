//! Permission Request – an agent asks for one exact right before it acts.
//!
//! The card reads like a short form: who is asking and for what, why, the
//! exact commands it will run, what it won't touch, and how long the grant
//! lasts (once, for a time window, or always). Allowing or denying folds the
//! card into a single grant line: a live countdown with a draining bar and
//! Revoke for a timed grant, Undo for a denial.
//!
//! Keys while the card has focus: `Enter` allows, `Esc` denies, `1`–`3` or
//! the arrows pick the scope.
//!
//! ```ignore
//! PermissionRequest::new("publish", "Release bot", "publish @acme/charts 2.4.0")
//!     .reason("The release checklist for 2.4.0 is green")
//!     .commands(["npm publish --access public"])
//!     .on_decide(|outcome, _, _| println!("{outcome:?}"))
//! ```

use std::{cell::Cell, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, ElementId, Entity, FocusHandle, FontWeight, HighlightStyle, Hsla,
    IntoElement, KeyDownEvent, MouseButton, SharedString, StyledText, Task, Window, canvas, div,
    prelude::*, px, relative, svg,
};

use crate::{
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, Tone, radius, space, text},
    motion::{self, GLIDE, HOVER_FADE, LINEAR, MENU_IN, MotionSpec},
    story,
};

pub const NAME: &str = "Permission Request";
pub const DESCRIPTION: &str = "An agent asks for one exact right and spells out the commands it will run – grant it once, for an hour or for good, and it folds into a live grant line with a countdown and Revoke";

const ICON_CHECK: &str = "icons/permission-request/check.svg";
const ICON_CROSS: &str = "icons/permission-request/cross.svg";
const ICON_ALERT: &str = "icons/permission-request/alert.svg";
const ICON_CLOCK: &str = "icons/permission-request/clock.svg";
const ICON_SHIELD: &str = "icons/permission-request/shield.svg";
const ICON_PROMPT: &str = "icons/permission-request/prompt.svg";

const LABEL_W: f32 = 60.0;
const SEGMENT_W: f32 = 92.0;
const LINE_H: f32 = 60.0;

/// How long an allowed request stays allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrantScope {
    /// Just this one action.
    Once,
    /// Any matching action until the window runs out.
    For(Duration),
    /// Saved to the agent's policy until revoked.
    Always,
}

impl GrantScope {
    fn label(self) -> SharedString {
        match self {
            GrantScope::Once => "Once".into(),
            GrantScope::For(d) => duration_label(d).into(),
            GrantScope::Always => "Always".into(),
        }
    }
}

fn duration_label(d: Duration) -> String {
    let mins = d.as_secs() / 60;
    match mins {
        0 => format!("{} s", d.as_secs()),
        60 => "1 hour".into(),
        m if m % 60 == 0 => format!("{} hours", m / 60),
        m => format!("{m} min"),
    }
}

/// What the person decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allowed(GrantScope),
    Denied,
    /// A grant was taken back (or a denial undone): the request is open again.
    Reopened,
}

type OutcomeHandler = Rc<dyn Fn(PermissionOutcome, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct PermissionRequest {
    id: ElementId,
    agent: SharedString,
    action: SharedString,
    monogram: Option<SharedString>,
    context: Option<SharedString>,
    reason: Option<SharedString>,
    commands: Vec<SharedString>,
    excludes: Option<SharedString>,
    risk: Option<(SharedString, Tone)>,
    warning: Option<SharedString>,
    window: Duration,
    width: f32,
    autofocus: bool,
    on_decide: Option<OutcomeHandler>,
}

impl PermissionRequest {
    /// `agent` asks to `action` ("publish @acme/charts 2.4.0").
    pub fn new(
        id: impl Into<ElementId>,
        agent: impl Into<SharedString>,
        action: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            agent: agent.into(),
            action: action.into(),
            monogram: None,
            context: None,
            reason: None,
            commands: Vec::new(),
            excludes: None,
            risk: None,
            warning: None,
            window: Duration::from_secs(60 * 60),
            width: 480.,
            autofocus: false,
            on_decide: None,
        }
    }

    /// Two letters for the agent's avatar (default: the initials of its name).
    pub fn monogram(mut self, monogram: impl Into<SharedString>) -> Self {
        self.monogram = Some(monogram.into());
        self
    }

    /// A quiet line under the title: where, and when it was asked.
    pub fn context(mut self, context: impl Into<SharedString>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// The exact commands the grant covers.
    pub fn commands<S: Into<SharedString>>(
        mut self,
        commands: impl IntoIterator<Item = S>,
    ) -> Self {
        self.commands = commands.into_iter().map(Into::into).collect();
        self
    }

    /// What the grant does not cover.
    pub fn excludes(mut self, excludes: impl Into<SharedString>) -> Self {
        self.excludes = Some(excludes.into());
        self
    }

    /// A badge beside the title, e.g. `("Permanent", Tone::Warning)`.
    pub fn risk(mut self, label: impl Into<SharedString>, tone: Tone) -> Self {
        self.risk = Some((label.into(), tone));
        self
    }

    /// A caution shown beside the buttons.
    pub fn warning(mut self, warning: impl Into<SharedString>) -> Self {
        self.warning = Some(warning.into());
        self
    }

    /// The middle scope's time window (default one hour).
    pub fn window(mut self, window: Duration) -> Self {
        self.window = window;
        self
    }

    /// Card width in px (default 480).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Takes keyboard focus when first shown.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    pub fn on_decide(
        mut self,
        handler: impl Fn(PermissionOutcome, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Status {
    Pending,
    Allowed { scope: GrantScope, at: Instant },
    Denied,
}

struct PermState {
    focus: FocusHandle,
    scope: usize,
    status: Status,
    /// The request body's laid-out height, for folding it smoothly.
    body_h: Rc<Cell<f32>>,
    /// Re-renders once a second while a timed grant counts down.
    _ticker: Task<()>,
}

impl PermState {
    fn remaining(&self) -> Option<Duration> {
        match self.status {
            Status::Allowed {
                scope: GrantScope::For(window),
                at,
            } => Some(window.saturating_sub(at.elapsed())),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Command {
    Scope(usize),
    ScopeStep(isize),
    Allow,
    Deny,
    Reopen,
}

impl RenderOnce for PermissionRequest {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let autofocus = self.autofocus;
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            |window, cx| {
                let focus = cx.focus_handle();
                if autofocus {
                    window.focus(&focus, cx);
                }
                let ticker = cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor().timer(Duration::from_secs(1)).await;
                        let alive = this.update(cx, |s: &mut PermState, cx| {
                            if let Some(left) = s.remaining() {
                                if left.is_zero() {
                                    // The window ran out: ask again next time.
                                    s.status = Status::Pending;
                                }
                                cx.notify();
                            }
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                });
                PermState {
                    focus,
                    scope: 0,
                    status: Status::Pending,
                    body_h: Rc::new(Cell::new(0.)),
                    _ticker: ticker,
                }
            },
        );

        let scopes = [
            GrantScope::Once,
            GrantScope::For(self.window),
            GrantScope::Always,
        ];
        let (scope_ix, status, remaining, body_h, focus) = {
            let s = state.read(cx);
            (
                s.scope,
                s.status,
                s.remaining(),
                s.body_h.clone(),
                s.focus.clone(),
            )
        };
        let scope = scopes[scope_ix.min(2)];

        let act: Rc<dyn Fn(Command, &mut Window, &mut App)> = {
            let state = state.clone();
            let on_decide = self.on_decide.clone();
            Rc::new(move |command, window, cx| run(&state, command, scopes, &on_decide, window, cx))
        };

        let open = motion::follow(
            &format!("{key}-open"),
            if status == Status::Pending { 1. } else { 0. },
            GLIDE,
        );

        // ---- request -------------------------------------------------------
        let monogram = self.monogram.clone().unwrap_or_else(|| {
            self.agent
                .split_whitespace()
                .filter_map(|w| w.chars().next())
                .take(2)
                .collect::<String>()
                .to_uppercase()
                .into()
        });
        let title = {
            let mid = " wants to ";
            let full = format!("{}{mid}{}", self.agent, self.action);
            let strong = HighlightStyle {
                color: Some(t.text),
                font_weight: Some(FontWeight::MEDIUM),
                ..Default::default()
            };
            let a = self.agent.len();
            let b = a + mid.len();
            StyledText::new(full.clone()).with_highlights([(0..a, strong), (b..full.len(), strong)])
        };
        let header = div()
            .flex()
            .items_start()
            .gap(px(space::S12))
            .p(px(space::S16))
            .child(
                div()
                    .size(px(36.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::MD))
                    .bg(t.accent_wash)
                    .text_size(px(text::BODY))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if t.dark { t.text } else { t.accent })
                    .child(monogram),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(text::CALLOUT))
                            .line_height(px(20.))
                            .text_color(t.text_muted)
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(space::S6))
                            .mt(px(2.))
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .when_some(self.risk.clone(), |d, (label, tone)| {
                                d.child(luna::badge(label, tone, &t))
                            })
                            .when_some(self.context.clone(), |d, c| {
                                d.child(
                                    svg()
                                        .path(ICON_SHIELD)
                                        .size(px(13.))
                                        .flex_none()
                                        .text_color(t.text_faint),
                                )
                                .child(div().truncate().child(c))
                            }),
                    ),
            );

        let field = |label: &'static str, content: AnyElement, label_top: f32| {
            div()
                .flex()
                .items_start()
                .gap(px(space::S12))
                .child(
                    div()
                        .w(px(LABEL_W))
                        .flex_none()
                        .pt(px(label_top))
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(20.))
                        .text_color(t.text_faint)
                        .child(label),
                )
                .child(div().flex_1().min_w_0().child(content))
        };

        let mut fields = div()
            .flex()
            .flex_col()
            .gap(px(space::S12))
            .px(px(space::S16))
            .pb(px(space::S16));
        if let Some(reason) = self.reason.clone() {
            fields = fields.child(field(
                "Why",
                div()
                    .text_size(px(text::BODY))
                    .line_height(px(20.))
                    .text_color(t.text)
                    .child(reason)
                    .into_any_element(),
                0.,
            ));
        }
        if !self.commands.is_empty() {
            let well = div()
                .flex()
                .flex_col()
                .py(px(space::S6))
                .px(px(space::S10))
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Quaternary))
                .border_1()
                .border_color(t.border)
                .font_family(luna::MONO)
                .text_size(px(text::FOOTNOTE))
                .line_height(px(20.))
                .children(self.commands.iter().map(|c| {
                    div()
                        .flex()
                        .items_center()
                        .gap(px(space::S6))
                        .child(
                            svg()
                                .path(ICON_PROMPT)
                                .size(px(12.))
                                .flex_none()
                                .text_color(t.text_faint),
                        )
                        .child(div().truncate().text_color(t.text).child(c.clone()))
                }));
            fields = fields.child(field("Runs", well.into_any_element(), 7.));
        }
        if let Some(excludes) = self.excludes.clone() {
            fields = fields.child(field(
                "Won't",
                div()
                    .text_size(px(text::BODY))
                    .line_height(px(20.))
                    .text_color(t.text_muted)
                    .child(excludes)
                    .into_any_element(),
                0.,
            ));
        }
        fields = fields.child(field(
            "Allow",
            div()
                .flex()
                .flex_col()
                .gap(px(space::S6))
                .child(segmented(&key, &scopes, scope_ix, act.clone(), &t))
                .child(
                    div()
                        .h(px(18.))
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(18.))
                        .text_color(t.text_muted)
                        .child(motion::rise_in(
                            SharedString::from(format!("{key}-explain-{scope_ix}")),
                            MENU_IN,
                            3.,
                            div().child(scope_explainer(scope)),
                        )),
                )
                .into_any_element(),
            5.,
        ));

        let allow_label: SharedString = match scope {
            GrantScope::Once => "Allow once".into(),
            GrantScope::For(d) => format!("Allow for {}", duration_label(d)).into(),
            GrantScope::Always => "Always allow".into(),
        };
        let footer = div()
            .flex()
            .items_center()
            .gap(px(space::S8))
            .px(px(space::S16))
            .py(px(space::S12))
            .border_t_1()
            .border_color(t.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(space::S6))
                    .when_some(self.warning.clone(), |d, w| {
                        d.child(
                            svg()
                                .path(ICON_ALERT)
                                .size(px(14.))
                                .flex_none()
                                .text_color(t.warning),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_muted)
                                .child(w),
                        )
                    }),
            )
            .child({
                let act = act.clone();
                luna::button(
                    format!("{key}-deny"),
                    ButtonKind::Secondary,
                    ControlSize::Lg,
                    &t,
                )
                .child("Deny")
                .child(key_hint("Esc", t.text))
                .on_click(move |_, window, cx| act(Command::Deny, window, cx))
            })
            .child({
                let act = act.clone();
                luna::button(
                    format!("{key}-allow"),
                    ButtonKind::Primary,
                    ControlSize::Lg,
                    &t,
                )
                .child(allow_label)
                .child(key_hint("↵", t.on_solid))
                .on_click(move |_, window, cx| act(Command::Allow, window, cx))
            });

        let request = div()
            .relative()
            .flex()
            .flex_col()
            .child(header)
            .child(fields)
            .child(footer)
            .child({
                // Measures the request so folding can glide from its height.
                let body_h = body_h.clone();
                canvas(
                    move |bounds, _, _| body_h.set(f32::from(bounds.size.height)),
                    |_, _, _, _| {},
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full()
            });

        // ---- grant line ----------------------------------------------------
        let line = (status != Status::Pending || open < 1.).then(|| {
            grant_line(
                &key,
                status,
                remaining,
                self.window,
                &self.action,
                act.clone(),
                &t,
            )
        });

        let measured = body_h.get();
        let folding = open < 0.999 && measured > 0.;
        let request_slot = div()
            .overflow_hidden()
            .when(folding, |d| d.h(px(measured * open)).opacity(open))
            .when(open > 0.001 || measured == 0., |d| d.child(request));
        let line_slot = line.map(|line| {
            div()
                .overflow_hidden()
                .h(px(LINE_H * (1. - open)))
                .opacity(1. - open)
                .child(line)
        });

        let on_key = {
            let act = act.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let k = &event.keystroke;
                if k.modifiers.platform || k.modifiers.control || k.modifiers.alt {
                    return;
                }
                let command = match k.key.as_str() {
                    "enter" => Command::Allow,
                    "escape" => Command::Deny,
                    "1" => Command::Scope(0),
                    "2" => Command::Scope(1),
                    "3" => Command::Scope(2),
                    "left" => Command::ScopeStep(-1),
                    "right" => Command::ScopeStep(1),
                    _ => return,
                };
                act(command, window, cx);
                cx.stop_propagation();
            }
        };

        motion::keep_alive(window);
        luna::card(&t)
            .id(self.id.clone())
            .track_focus(&focus)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, {
                let focus = focus.clone();
                move |_, window, cx| window.focus(&focus, cx)
            })
            .w(px(self.width))
            .overflow_hidden()
            .font_family(luna::SANS)
            .text_color(t.text)
            .child(request_slot)
            .children(line_slot)
    }
}

fn run(
    state: &Entity<PermState>,
    command: Command,
    scopes: [GrantScope; 3],
    on_decide: &Option<OutcomeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let mut outcome = None;
    state.update(cx, |s, cx| {
        let pending = s.status == Status::Pending;
        match command {
            Command::Scope(ix) if pending => s.scope = ix.min(2),
            Command::ScopeStep(d) if pending => {
                s.scope = (s.scope as isize + d).clamp(0, 2) as usize
            }
            Command::Allow if pending => {
                let scope = scopes[s.scope];
                s.status = Status::Allowed {
                    scope,
                    at: Instant::now(),
                };
                outcome = Some(PermissionOutcome::Allowed(scope));
            }
            Command::Deny if pending => {
                s.status = Status::Denied;
                outcome = Some(PermissionOutcome::Denied);
            }
            Command::Reopen if !pending => {
                s.status = Status::Pending;
                outcome = Some(PermissionOutcome::Reopened);
            }
            _ => {}
        }
        cx.notify();
    });
    if let (Some(f), Some(outcome)) = (on_decide, outcome) {
        f(outcome, window, cx);
    }
}

/// One sentence on what the chosen scope means.
fn scope_explainer(scope: GrantScope) -> String {
    match scope {
        GrantScope::Once => "Only this run. It asks again next time.".into(),
        GrantScope::For(d) => format!(
            "The commands above run without asking for the next {}.",
            if d.as_secs() == 3600 {
                "hour".to_string()
            } else {
                duration_label(d)
            }
        ),
        GrantScope::Always => "Saved to the agent's policy until you revoke it.".into(),
    }
}

/// A key hint inside a button, tinted with the button's label color.
fn key_hint(keys: &'static str, fg: Hsla) -> impl IntoElement {
    div()
        .h(px(16.))
        .min_w(px(16.))
        .px(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::XS))
        .bg(fg.opacity(0.10))
        .font_family(luna::MONO)
        .text_size(px(10.))
        .text_color(fg.opacity(0.62))
        .child(keys)
}

/// The scope picker: three segments on a well with a thumb that glides.
fn segmented(
    key: &str,
    scopes: &[GrantScope; 3],
    selected: usize,
    act: Rc<dyn Fn(Command, &mut Window, &mut App)>,
    t: &Luna,
) -> impl IntoElement {
    let x = motion::follow(&format!("{key}-thumb"), selected as f32 * SEGMENT_W, GLIDE);
    let thumb_bg = if t.dark {
        t.surface_raised
    } else {
        t.surface_card
    };
    div()
        .relative()
        .flex()
        .w(px(SEGMENT_W * 3. + 4.))
        .h(px(30.))
        .p(px(2.))
        .rounded(px(radius::MD))
        .bg(t.fill(FillTier::Tertiary))
        .child(
            div()
                .absolute()
                .top(px(2.))
                .left(px(2. + x))
                .w(px(SEGMENT_W))
                .h(px(26.))
                .rounded(px(radius::SM))
                .bg(thumb_bg)
                .shadow(t.card_shadow())
                .border_1()
                .border_color(t.border),
        )
        .children(scopes.iter().enumerate().map(|(ix, scope)| {
            let act = act.clone();
            let hover_key = format!("{key}-seg-{ix}");
            let on = motion::follow(
                &format!("{hover_key}-on"),
                if ix == selected { 1. } else { 0. },
                HOVER_FADE,
            );
            let rest = motion::mix(t.text_muted, t.text, on);
            div()
                .id(SharedString::from(hover_key.clone()))
                .relative()
                .w(px(SEGMENT_W))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(space::S4))
                .cursor_pointer()
                .text_size(px(text::FOOTNOTE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(motion::hover_blend(&hover_key, rest, t.text))
                .on_hover(motion::hover_listener(hover_key))
                .on_click(move |_, window, cx| act(Command::Scope(ix), window, cx))
                .when(matches!(scope, GrantScope::For(_)), |d| {
                    d.child(svg().path(ICON_CLOCK).size(px(12.)).text_color(rest))
                })
                .child(scope.label())
        }))
}

/// The folded state: what was decided, the time left and the way back.
fn grant_line(
    key: &str,
    status: Status,
    remaining: Option<Duration>,
    window_len: Duration,
    action: &SharedString,
    act: Rc<dyn Fn(Command, &mut Window, &mut App)>,
    t: &Luna,
) -> impl IntoElement {
    let (allowed, title, detail, back): (bool, SharedString, SharedString, &'static str) =
        match status {
            Status::Allowed { scope, .. } => (
                true,
                match scope {
                    GrantScope::Once => "Allowed once".into(),
                    GrantScope::For(d) => format!("Allowed for {}", duration_label(d)).into(),
                    GrantScope::Always => "Always allowed".into(),
                },
                match scope {
                    GrantScope::Once => format!("{action} · this run only").into(),
                    GrantScope::For(_) => action.clone(),
                    GrantScope::Always => format!("{action} · saved to policy").into(),
                },
                "Revoke",
            ),
            _ => (
                false,
                "Denied".into(),
                format!("{action} · the agent was told no").into(),
                "Undo",
            ),
        };
    let tone = if allowed { t.success } else { t.danger };
    let fraction = remaining.map(|left| {
        let target = left.as_secs_f32() / window_len.as_secs_f32().max(1.);
        // Ease each one-second step so the bar drains continuously.
        motion::follow(
            &format!("{key}-drain"),
            target,
            MotionSpec::new(1000, LINEAR),
        )
    });
    let clock = remaining.map(|left| {
        let s = left.as_secs();
        if s >= 3600 {
            format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
        } else {
            format!("{:02}:{:02}", s / 60, s % 60)
        }
    });

    div()
        .relative()
        .h(px(LINE_H))
        .flex()
        .items_center()
        .gap(px(space::S12))
        .px(px(space::S16))
        .child(motion::rise_in(
            SharedString::from(format!("{key}-badge-{allowed}")),
            MENU_IN.with_delay(120),
            2.,
            div()
                .size(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(tone.opacity(if t.dark { 0.16 } else { 0.12 }))
                .child(
                    svg()
                        .path(if allowed { ICON_CHECK } else { ICON_CROSS })
                        .size(px(16.))
                        .text_color(tone),
                ),
        ))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(text::BODY))
                        .line_height(px(18.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text)
                        .child(title),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(text::FOOTNOTE))
                        .line_height(px(16.))
                        .text_color(t.text_faint)
                        .child(detail),
                ),
        )
        .when_some(clock, |d, clock| {
            d.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(space::S4))
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(
                        svg()
                            .path(ICON_CLOCK)
                            .size(px(13.))
                            .text_color(t.text_faint),
                    )
                    .child(
                        div()
                            .font_family(luna::MONO)
                            .text_color(t.text_muted)
                            .child(clock),
                    )
                    .child("left"),
            )
        })
        .child(
            luna::button(
                format!("{key}-back"),
                ButtonKind::Secondary,
                ControlSize::Md,
                t,
            )
            .child(back)
            .on_click(move |_, window, cx| act(Command::Reopen, window, cx)),
        )
        .when_some(fraction, |d, f| {
            d.child(
                div()
                    .absolute()
                    .left_0()
                    .bottom_0()
                    .h(px(2.))
                    .w(relative(f.clamp(0., 1.)))
                    .bg(t.success.opacity(0.8)),
            )
        })
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        PermissionRequest::new(
            "permission-request",
            "Release bot",
            "publish @acme/charts 2.4.0",
        )
        .context("npm registry · asked a few seconds ago")
        .reason("PR #2184 bumped the version and every release check passed on main.")
        .commands([
            "npm publish --access public",
            "git tag v2.4.0",
            "git push origin v2.4.0",
        ])
        .excludes("Unpublishing, or any other package in the org.")
        .risk("Permanent", Tone::Warning)
        .warning("Publishing can't be undone")
        .autofocus(true)
    })
}
