//! Permission Request – the agent stops and asks before it acts (pink:
//! composer.rs `Composer::render_wizard` in approval mode).
//!
//! The request takes the composer's place in the same rounded pill: a
//! tracked uppercase header, the question, the approval context (the exact
//! command, where it runs, why, and how long the request waits), one
//! numbered row per choice naming its grant scope, and a single Send.
//! Picking a row answers after a short beat; the request then folds into the
//! transcript's passive "Question" record (pink: transcript.rs `input_chip`).
//!
//! Keys while the request has focus: `1`–`9` pick a choice, `Enter` sends
//! the picked one.
//!
//! ```ignore
//! PermissionRequest::new("publish", "Codex", "publish @acme/charts 2.4.0")
//!     .header("Approve command")
//!     .commands(["npm publish --access public"])
//!     .reason("Every release check passed on main")
//!     .on_decide(|outcome, _, _| println!("{outcome:?}"))
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyView, App, ElementId, FocusHandle, FontWeight, IntoElement, KeyDownEvent, MouseButton, Role,
    SharedString, Task, Toggled, WeakEntity, Window, div, phi, prelude::*, px, svg,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink, layout, radius, space, text},
    story,
};

pub const NAME: &str = "Permission Request";
pub const DESCRIPTION: &str = "An agent stops to ask before it acts – the composer becomes the request, with the exact command, where and why, numbered choices that each name their grant scope, and a quiet Question record once you answer";

const ICON_QUESTION: &str = "icons/permission-request/chat-round-line.svg";

/// A pick answers after this beat, so the picked row reads first (pink:
/// composer.rs `AUTO_ADVANCE_MS`).
const AUTO_ADVANCE: Duration = Duration::from_millis(220);

/// The answer rows' corner (pink: composer.rs `render_wizard`, `rounded(12)`).
const OPTION_RADIUS: f32 = 12.0;

/// What the person decided (pink: proto `ApprovalDecision`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionOutcome {
    /// Allow only this request.
    AllowOnce,
    /// Allow matching requests for the rest of the agent's session.
    AllowForSession,
    /// Save a rule to the agent's policy for future matching requests.
    AllowAlways,
    /// Reject this action; the agent may continue.
    Deny,
    /// Reject this action and stop the agent's turn.
    CancelTurn,
}

/// One answer row: its label, the grant's lifetime and what it does
/// (pink: proto `ApprovalChoice`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionChoice {
    pub label: SharedString,
    /// How long the grant lives: "request", "session", "persistent policy",
    /// or "none" for a refusal.
    pub scope: SharedString,
    pub description: SharedString,
    pub outcome: PermissionOutcome,
}

impl PermissionChoice {
    pub fn new(
        label: impl Into<SharedString>,
        scope: impl Into<SharedString>,
        description: impl Into<SharedString>,
        outcome: PermissionOutcome,
    ) -> Self {
        Self {
            label: label.into(),
            scope: scope.into(),
            description: description.into(),
            outcome,
        }
    }

    // pink: harness codex/approvals.rs `command_choice`.

    pub fn allow_once() -> Self {
        Self::new(
            "Allow once",
            "request",
            "Approve only this command or terminal input.",
            PermissionOutcome::AllowOnce,
        )
    }

    pub fn allow_for_session() -> Self {
        Self::new(
            "Allow for session",
            "session",
            "Allow future prompts covered by this provider's session approval cache.",
            PermissionOutcome::AllowForSession,
        )
    }

    pub fn allow_rule() -> Self {
        Self::new(
            "Allow proposed command rule",
            "persistent policy",
            "Approve and persist the exact proposed command prefix rule for future matching commands.",
            PermissionOutcome::AllowAlways,
        )
    }

    pub fn deny() -> Self {
        Self::new(
            "Deny",
            "none",
            "Reject this action; the agent may continue.",
            PermissionOutcome::Deny,
        )
    }

    pub fn cancel_turn() -> Self {
        Self::new(
            "Cancel turn",
            "none",
            "Reject this action and interrupt the provider turn.",
            PermissionOutcome::CancelTurn,
        )
    }
}

type OutcomeHandler = Rc<dyn Fn(PermissionOutcome, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct PermissionRequest {
    id: ElementId,
    agent: SharedString,
    action: SharedString,
    header: SharedString,
    commands: Vec<SharedString>,
    cwd: Option<SharedString>,
    reason: Option<SharedString>,
    write_root: Option<SharedString>,
    network: Option<SharedString>,
    details: Vec<(SharedString, SharedString)>,
    expires_at: Option<SharedString>,
    choices: Vec<PermissionChoice>,
    width: f32,
    autofocus: bool,
    resolved: bool,
    on_decide: Option<OutcomeHandler>,
}

impl PermissionRequest {
    /// `agent` asks to `action`: the question reads "Codex wants to run a
    /// command."
    pub fn new(
        id: impl Into<ElementId>,
        agent: impl Into<SharedString>,
        action: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            agent: agent.into(),
            action: action.into(),
            header: "Approval".into(),
            commands: Vec::new(),
            cwd: None,
            reason: None,
            write_root: None,
            network: None,
            details: Vec::new(),
            expires_at: None,
            choices: vec![
                PermissionChoice::allow_once(),
                PermissionChoice::allow_for_session(),
                PermissionChoice::deny(),
                PermissionChoice::cancel_turn(),
            ],
            width: layout::COMPOSER_MAX_WIDTH,
            autofocus: false,
            resolved: false,
            on_decide: None,
        }
    }

    /// The small label above the question, e.g. "Approve command"
    /// (default "Approval").
    pub fn header(mut self, header: impl Into<SharedString>) -> Self {
        self.header = header.into();
        self
    }

    /// The exact commands the grant covers, one per line.
    pub fn commands<S: Into<SharedString>>(
        mut self,
        commands: impl IntoIterator<Item = S>,
    ) -> Self {
        self.commands = commands.into_iter().map(Into::into).collect();
        self
    }

    /// The working directory the commands run in.
    pub fn cwd(mut self, cwd: impl Into<SharedString>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    pub fn reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// A folder the agent asks to write outside its workspace.
    pub fn write_root(mut self, root: impl Into<SharedString>) -> Self {
        self.write_root = Some(root.into());
        self
    }

    /// A host the agent asks to reach, e.g. `("registry.npmjs.org", "https")`.
    pub fn network(
        mut self,
        host: impl Into<SharedString>,
        protocol: impl Into<SharedString>,
    ) -> Self {
        self.network = Some(format!("{} ({})", host.into(), protocol.into()).into());
        self
    }

    /// Any other context row, shown after the built-in ones.
    pub fn detail(
        mut self,
        label: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        self.details.push((label.into(), value.into()));
        self
    }

    /// When the request lapses, as a clock time ("14:32:05"). Without one it
    /// waits until answered.
    pub fn expires_at(mut self, time: impl Into<SharedString>) -> Self {
        self.expires_at = Some(time.into());
        self
    }

    /// The answer rows (default: allow once, allow for session, deny and
    /// cancel turn).
    pub fn choices(mut self, choices: impl IntoIterator<Item = PermissionChoice>) -> Self {
        self.choices = choices.into_iter().collect();
        self
    }

    /// Request width in px (default: pink's composer column, 768).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Takes keyboard focus when first shown.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Shows the request as already answered: only its transcript record.
    pub fn resolved(mut self, resolved: bool) -> Self {
        self.resolved = resolved;
        self
    }

    pub fn on_decide(
        mut self,
        handler: impl Fn(PermissionOutcome, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_decide = Some(Rc::new(handler));
        self
    }

    /// The approval context rows in pink's order (pink: proto
    /// `ApprovalDetails::display_rows`, then composer.rs
    /// `approval_expiry_row`).
    fn rows(&self) -> Vec<(SharedString, SharedString)> {
        let mut rows: Vec<(SharedString, SharedString)> = Vec::new();
        if !self.commands.is_empty() {
            let commands: Vec<&str> = self.commands.iter().map(|c| c.as_ref()).collect();
            rows.push(("Command / input".into(), commands.join("\n").into()));
        }
        for (label, value) in [
            ("Working directory", &self.cwd),
            ("Reason", &self.reason),
            ("Proposed write root", &self.write_root),
            ("Network host", &self.network),
        ] {
            if let Some(value) = value {
                rows.push((label.into(), value.clone()));
            }
        }
        rows.extend(self.details.iter().cloned());
        rows.push(match &self.expires_at {
            Some(time) => ("Approval expires".into(), time.clone()),
            None => (
                "Approval waits".into(),
                "Until you answer, the agent stops or its turn ends".into(),
            ),
        });
        rows
    }

    fn question(&self) -> SharedString {
        let action = self.action.trim_end();
        let stop = if action.ends_with(['.', '?', '!']) {
            ""
        } else {
            "."
        };
        format!("{} wants to {action}{stop}", self.agent).into()
    }
}

struct PermState {
    focus: FocusHandle,
    picked: Option<usize>,
    resolved: bool,
    /// The pending answer after a pick (pink: composer.rs
    /// `schedule_auto_advance`); a new pick replaces it.
    advance: Option<Task<()>>,
}

#[derive(Clone, Copy)]
enum Command {
    Pick(usize),
    Send,
}

impl RenderOnce for PermissionRequest {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
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
                PermState {
                    focus,
                    picked: None,
                    resolved: false,
                    advance: None,
                }
            },
        );
        let (picked, resolved, focus) = {
            let s = state.read(cx);
            (s.picked, s.resolved || self.resolved, s.focus.clone())
        };

        let root = div()
            .w_full()
            .max_w(px(self.width))
            .font_family(pink::SANS)
            .text_size(px(text::PROSE))
            .line_height(phi())
            .text_color(t.text);

        if resolved {
            return root.child(motion::fade_quick(
                SharedString::from(format!("{key}-record")),
                record(self.header.clone(), &t),
            ));
        }

        let choices: Rc<[PermissionChoice]> = self.choices.clone().into();
        let act: Rc<dyn Fn(Command, &mut Window, &mut App)> = {
            let state = state.downgrade();
            let choices = choices.clone();
            let on_decide = self.on_decide.clone();
            Rc::new(move |command, window, cx| {
                run(&state, command, &choices, &on_decide, window, cx)
            })
        };

        // ---- body: header, question, context, choices ------------------
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(space::S10))
            .child(
                div()
                    .text_size(px(text::CAPTION))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.readable_muted(0.6))
                    .child(SharedString::from(pink::tracked_upper(&self.header))),
            );
        let question = div()
            .id(SharedString::from(format!("{key}-question")))
            .max_h(px(160.))
            .overflow_y_scroll()
            .mt(px(space::S6))
            .text_size(px(text::TITLE3))
            .line_height(px(20.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.text)
            .child(self.question());
        let context = div()
            .id(SharedString::from(format!("{key}-context")))
            .mt(px(space::S8))
            .max_h(px(220.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(space::S6))
            .children(self.rows().into_iter().map(|(label, value)| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(space::S2))
                    .child(
                        div()
                            .text_size(px(text::CAPTION))
                            .text_color(t.readable_muted(0.65))
                            .child(label),
                    )
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text)
                            .child(value),
                    )
            }));
        let options = div()
            .id(SharedString::from(format!("{key}-options")))
            .max_h(px(220.))
            .overflow_y_scroll()
            .mt(px(space::S12))
            .flex()
            .flex_col()
            .gap(px(space::S4))
            .children(choices.iter().enumerate().map(|(ix, choice)| {
                option_row(&key, ix, choice, picked == Some(ix), act.clone(), &t)
            }));
        let body = div()
            .px(px(space::S16))
            .pt(px(space::S16))
            .flex()
            .flex_col()
            .child(header)
            .child(question)
            .child(context)
            .child(options);

        // ---- footer: one Primary Send ------------------------------------
        let footer = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .gap(px(space::S12))
            .px(px(space::S16))
            .pb(px(space::S16))
            .pt(px(space::S4))
            // Pink's note slot (countdowns, async notes); it also keeps Send
            // on the trailing edge.
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap(px(space::S8)),
            )
            .child({
                let act = act.clone();
                pink::button(
                    format!("{key}-send"),
                    ButtonKind::Primary,
                    ControlSize::Lg,
                    &t,
                )
                .aria_label("Send")
                .child("Send")
                .px(px(space::S16))
                .when(picked.is_none(), |el| el.opacity(0.4))
                .on_click(move |_, window, cx| act(Command::Send, window, cx))
            });

        let on_key = {
            let act = act.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let k = &event.keystroke;
                if let Some(ix) = number_shortcut(&k.key, k.modifiers.modified()) {
                    act(Command::Pick(ix), window, cx);
                    cx.stop_propagation();
                } else if k.key == "enter" {
                    act(Command::Send, window, cx);
                    cx.stop_propagation();
                } else if k.key == "escape" {
                    // Pink's Esc steps back a page; a request has one page.
                    cx.stop_propagation();
                }
            }
        };

        // pink: composer.rs `composer_surface`, opaque treatment.
        let panel = div()
            .id(SharedString::from(format!("{key}-panel")))
            .rounded(px(radius::COMPOSER))
            .border_1()
            .border_color(t.border)
            .bg(t.input_glass_bg())
            .shadow(pink::shadow_lg())
            .track_focus(&focus)
            .on_key_down(on_key)
            .on_mouse_down(MouseButton::Left, {
                let focus = focus.clone();
                move |_, window, cx| {
                    if !focus.contains_focused(window, cx) {
                        window.focus(&focus, cx);
                    }
                }
            })
            .flex()
            .flex_col()
            .child(body)
            .child(footer);

        motion::keep_alive(window);
        root.child(motion::fade_quick(
            SharedString::from(format!("{key}-wizard")),
            div().child(panel),
        ))
    }
}

/// 1–9 pick a choice; modified digits belong to app shortcuts (pink:
/// composer.rs `wizard_number_shortcut`).
fn number_shortcut(key: &str, modified: bool) -> Option<usize> {
    if modified {
        return None;
    }
    let digit = key.parse::<usize>().ok()?;
    (1..=9).contains(&digit).then(|| digit - 1)
}

fn run(
    state: &WeakEntity<PermState>,
    command: Command,
    choices: &Rc<[PermissionChoice]>,
    on_decide: &Option<OutcomeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(entity) = state.upgrade() else {
        return;
    };
    match command {
        Command::Pick(ix) => {
            if ix >= choices.len() || entity.read(cx).resolved {
                return;
            }
            let advance = {
                let state = state.clone();
                let choices = choices.clone();
                let on_decide = on_decide.clone();
                window.spawn(cx, async move |cx| {
                    cx.background_executor().timer(AUTO_ADVANCE).await;
                    cx.update(|window, cx| submit(&state, &choices, &on_decide, window, cx))
                        .ok();
                })
            };
            entity.update(cx, |s, cx| {
                s.picked = Some(ix);
                s.advance = Some(advance);
                cx.notify();
            });
        }
        Command::Send => submit(state, choices, on_decide, window, cx),
    }
}

/// Answers with the picked row; without a pick Send does nothing (pink:
/// composer.rs `wizard_advance`).
fn submit(
    state: &WeakEntity<PermState>,
    choices: &Rc<[PermissionChoice]>,
    on_decide: &Option<OutcomeHandler>,
    window: &mut Window,
    cx: &mut App,
) {
    let Some(entity) = state.upgrade() else {
        return;
    };
    let mut outcome = None;
    entity.update(cx, |s, cx| {
        if s.resolved {
            return;
        }
        if let Some(choice) = s.picked.and_then(|ix| choices.get(ix)) {
            s.resolved = true;
            outcome = Some(choice.outcome);
            cx.notify();
        }
    });
    if let (Some(f), Some(outcome)) = (on_decide, outcome) {
        f(outcome, window, cx);
    }
}

/// One answer row (pink: composer.rs `render_wizard` "wizard-option"): r12
/// on an ink wash that deepens on hover, a deeper wash and a faint ink edge
/// when picked, the 13.5 label over its "scope · description" line, and a
/// 22 px number chip. Keyboard focus recolors the resting transparent edge
/// in the accent, as pink does for these rows.
fn option_row(
    key: &str,
    ix: usize,
    choice: &PermissionChoice,
    picked: bool,
    act: Rc<dyn Fn(Command, &mut Window, &mut App)>,
    t: &Pink,
) -> impl IntoElement {
    let hover_key = format!("{key}-option-{ix}");
    let accent = t.accent;
    div()
        .id(SharedString::from(hover_key.clone()))
        .role(Role::RadioButton)
        .aria_label(choice.label.clone())
        .aria_toggled(if picked {
            Toggled::True
        } else {
            Toggled::False
        })
        .tab_index(0)
        .focus_visible(move |style| style.border_color(accent))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S12))
        .px(px(14.))
        .py(px(space::S10))
        .rounded(px(OPTION_RADIUS))
        .border_1()
        .border_color(if picked {
            t.ink(0.16)
        } else {
            gpui::transparent_black()
        })
        .bg(if picked {
            t.ink(0.09)
        } else {
            motion::hover_blend(&hover_key, t.ink(0.025), t.ink(0.06))
        })
        .on_hover(motion::hover_listener(hover_key))
        .cursor_pointer()
        // The click rides GPUI's key-up keyboard click; the key-down must not
        // also reach the request's Enter.
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, window, cx| act(Command::Pick(ix), window, cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if picked { t.text } else { t.text.opacity(0.9) })
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(choice.label.clone())
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(t.readable_muted(0.65))
                        .child(SharedString::from(format!(
                            "{} · {}",
                            choice.scope, choice.description
                        ))),
                ),
        )
        .when(ix < 9, |el| {
            el.child(
                div()
                    .flex_none()
                    .size(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::SM))
                    .bg(if picked { t.ink(0.16) } else { t.ink(0.05) })
                    .text_size(px(text::CAPTION))
                    .text_color(if picked {
                        t.text
                    } else {
                        t.readable_muted(0.6)
                    })
                    .child(SharedString::from(format!("{}", ix + 1))),
            )
        })
}

/// The answered request as the transcript keeps it (pink: transcript.rs
/// `input_chip`): a 34 px neutral chip with the question glyph, "Question"
/// and the request's header. Resolution never recolors it.
fn record(header: SharedString, t: &Pink) -> gpui::Div {
    div().py(px(space::S4)).w_full().child(
        div()
            .h(px(34.))
            .w_full()
            .flex()
            .items_center()
            .gap(px(space::S8))
            .overflow_hidden()
            .rounded(px(radius::LG))
            .border_1()
            .border_color(t.hairline(0.08))
            .bg(t.ink(0.045))
            .px(px(space::S8))
            .text_size(px(text::FOOTNOTE))
            .child(
                div()
                    .flex_none()
                    .size(px(20.))
                    .rounded(px(radius::SM))
                    .bg(t.ink(0.09))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        svg()
                            .path(ICON_QUESTION)
                            .size(px(12.))
                            .text_color(t.content_muted),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.content_muted)
                    .child("Question"),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .truncate()
                    .text_color(t.text.opacity(0.9))
                    .child(header),
            ),
    )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        div()
            .w_full()
            .max_w(px(layout::COMPOSER_MAX_WIDTH))
            .px(px(space::S16))
            .flex()
            .flex_col()
            .gap(px(space::S8))
            // An earlier request, already answered: its transcript record.
            .child(
                PermissionRequest::new("permission-request-earlier", "Codex", "modify files")
                    .header("Approve file change")
                    .resolved(true),
            )
            .child(
                PermissionRequest::new(
                    "permission-request",
                    "Codex",
                    "publish @acme/charts 2.4.0 to npm",
                )
                .header("Approve command")
                .commands(["npm publish --access public", "git push origin v2.4.0"])
                .cwd("~/code/acme/charts")
                .reason("PR #2184 bumped the version and every release check passed on main.")
                .choices([
                    PermissionChoice::allow_once(),
                    PermissionChoice::allow_for_session(),
                    PermissionChoice::deny(),
                ])
                .autofocus(true),
            )
    })
}
