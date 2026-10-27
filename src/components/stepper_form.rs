//! Stepper Form – a paged setup flow on Pink's question-wizard card.
//!
//! Every step is one page of Pink's question wizard (Pink: composer.rs
//! `Composer::render_wizard`): the composer pill's chrome, a tracked
//! uppercase header with an "n/4" counter chip, the question in 15/20
//! medium, then the page's controls – captioned fields (Pink: automations.rs
//! `input_frame`), numbered option rows, staged invites or a review – and the
//! Back / Next footer whose one Primary dims until the page can advance.
//!
//! Picking a region moves on by itself after Pink's 220ms beat; digits pick,
//! Enter advances and Escape goes back while no field has focus. Next checks
//! the page and says what's missing under it, the review's Edit links jump
//! back to a page, and the last page reads "Creating…" before the finished
//! project.
//!
//! The demo creates a project: name and key, data region, invites, review.
//!
//! ```ignore
//! StepperForm::new("new-project").on_complete(|project, _, _| create(project))
//! ```

use std::{rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AnyView, App, Context, Div, ElementId, Entity, FocusHandle, FontWeight,
    IntoElement, KeyDownEvent, Role, SharedString, Stateful, Subscription, Task, Toggled, Window,
    div, prelude::*, px, svg,
};

use crate::{
    HoverFill, motion,
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Stepper Form";
pub const DESCRIPTION: &str = "A multi-step setup flow on one paged card: a step counter, captioned fields, numbered choices that move on by themselves, invites you can take back, a review with edit links and a Back and Next footer that says what's missing";

const ICON_CLOSE: &str = "icons/stepper-form/close.svg";

/// A single-choice pick moves to the next page after this beat (Pink
/// `composer::AUTO_ADVANCE_MS`).
const AUTO_ADVANCE_MS: u64 = 220;
/// The demo's "creating" beat before the finished page.
const CREATE_MS: u64 = 1300;
/// The line every Pink text field lays out on (Pink
/// `composer::INPUT_LINE_HEIGHT`, `ComposerInput`'s default).
const INPUT_LINE_HEIGHT: f32 = 22.75;

/// The pages: header, question and the line under it.
const STEPS: [(&str, &str, &str); 4] = [
    (
        "Project",
        "Name your project",
        "Shown in the sidebar and in links. The key starts every issue.",
    ),
    (
        "Region",
        "Where should its data live?",
        "You can't move a project between regions later.",
    ),
    (
        "Team",
        "Who else is working on it?",
        "Invites can also be sent later from Settings.",
    ),
    (
        "Review",
        "Ready to create it?",
        "Check the details. Edit jumps back to that page.",
    ),
];

/// Data regions: name, code and a latency hint.
const REGIONS: [(&str, &str, &str); 3] = [
    ("Frankfurt", "eu-central-1", "~18 ms from you"),
    ("Virginia", "us-east-1", "~92 ms from you"),
    ("Singapore", "ap-southeast-1", "~210 ms from you"),
];

type CompleteHandler = Rc<dyn Fn(&NewProject, &mut Window, &mut App)>;

/// What the flow collects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewProject {
    pub name: SharedString,
    pub key: SharedString,
    pub region: &'static str,
    pub invites: Vec<SharedString>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Editing,
    Creating,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Field {
    Name,
    Key,
    Region,
    Email,
}

struct FormState {
    name: Entity<TextInput>,
    key: Entity<TextInput>,
    email: Entity<TextInput>,
    /// The card's own focus, so digits, Enter and Escape reach it on the
    /// pages without a field (Pink tracks `wizard_focus` on the panel).
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
    colors_dark: Option<bool>,
    key_touched: bool,
    region: Option<usize>,
    invites: Vec<SharedString>,
    step: usize,
    /// Bumped on every page change, keying the page's entrance.
    epoch: usize,
    /// The field Next found wanting, and what to say under it.
    error: Option<(Field, &'static str)>,
    phase: Phase,
    advance_task: Option<Task<()>>,
    create_task: Option<Task<()>>,
    on_complete: Option<CompleteHandler>,
}

fn derive_key(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let key: String = if words.len() >= 2 {
        words.iter().filter_map(|w| w.chars().next()).collect()
    } else {
        name.chars()
            .filter(|c| c.is_alphanumeric())
            .take(3)
            .collect()
    };
    key.to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .take(5)
        .collect()
}

fn valid_email(email: &str) -> bool {
    let Some((user, domain)) = email.split_once('@') else {
        return false;
    };
    !user.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.contains(char::is_whitespace)
}

impl FormState {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| TextInput::new(window, cx).placeholder("e.g. Northwind Ops"));
        let key = cx.new(|cx| TextInput::new(window, cx).placeholder("NWO").max_len(5));
        let email = cx.new(|cx| TextInput::new(window, cx).placeholder("name@company.com"));
        let subs = vec![
            cx.subscribe(
                &name,
                |s: &mut Self, _, event: &InputEvent, cx| match event {
                    InputEvent::Change(text) => {
                        if !s.key_touched {
                            let derived = derive_key(text);
                            s.key.update(cx, |k, cx| k.set_text(derived, cx));
                        }
                        s.clear_error(&[Field::Name, Field::Key]);
                        cx.notify();
                    }
                    InputEvent::Submit(_) | InputEvent::Tab => s.focus_field(Field::Key, cx),
                    InputEvent::Escape => s.escape_from(Field::Name, cx),
                    _ => {}
                },
            ),
            cx.subscribe(
                &key,
                |s: &mut Self, _, event: &InputEvent, cx| match event {
                    InputEvent::Change(text) => {
                        s.key_touched = !text.is_empty();
                        let upper = text.to_uppercase();
                        if upper != text.as_ref() {
                            s.key.update(cx, |k, cx| k.set_text(upper, cx));
                        }
                        s.clear_error(&[Field::Key]);
                        cx.notify();
                    }
                    InputEvent::Submit(_) => s.advance(cx),
                    InputEvent::Escape => s.escape_from(Field::Key, cx),
                    _ => {}
                },
            ),
            cx.subscribe(
                &email,
                |s: &mut Self, _, event: &InputEvent, cx| match event {
                    InputEvent::Change(_) => {
                        s.clear_error(&[Field::Email]);
                        cx.notify();
                    }
                    InputEvent::Submit(text) => {
                        if text.trim().is_empty() {
                            s.advance(cx);
                        } else {
                            s.add_invite(text.clone(), cx);
                        }
                    }
                    InputEvent::Escape => s.escape_from(Field::Email, cx),
                    _ => {}
                },
            ),
        ];
        Self {
            name,
            key,
            email,
            focus: cx.focus_handle(),
            _subscriptions: subs,
            colors_dark: None,
            key_touched: false,
            region: None,
            invites: Vec::new(),
            step: 0,
            epoch: 0,
            error: None,
            phase: Phase::Editing,
            advance_task: None,
            create_task: None,
            on_complete: None,
        }
    }

    fn input(&self, field: Field) -> Option<&Entity<TextInput>> {
        match field {
            Field::Name => Some(&self.name),
            Field::Key => Some(&self.key),
            Field::Email => Some(&self.email),
            Field::Region => None,
        }
    }

    fn clear_error(&mut self, fields: &[Field]) {
        if self.error.is_some_and(|(f, _)| fields.contains(&f)) {
            self.error = None;
        }
    }

    /// Focus moves happen on the next frame, where a window is at hand. A
    /// page without a field focuses the card itself.
    fn focus_field(&mut self, field: Field, cx: &mut Context<Self>) {
        let input = self.input(field).cloned();
        let card = self.focus.clone();
        cx.defer(move |cx| {
            if let Some(window) = cx.active_window() {
                window
                    .update(cx, |_, window, cx| match &input {
                        Some(input) => TextInput::focus(input, window, cx),
                        None => window.focus(&card, cx),
                    })
                    .ok();
            }
        });
    }

    /// Escape in an empty field goes back a page (Pink
    /// `wizard_escape_goes_back`).
    fn escape_from(&mut self, field: Field, cx: &mut Context<Self>) {
        let empty = self.input(field).is_none_or(|i| i.read(cx).is_empty());
        if empty && self.phase == Phase::Editing && self.step > 0 {
            self.go(self.step - 1, cx);
        }
    }

    /// The card's keys while no field has focus (Pink `on_wizard_key`): a
    /// bare digit picks a region, Enter advances, Escape goes back.
    fn on_key(&mut self, event: &KeyDownEvent, window: &Window, cx: &mut Context<Self>) -> bool {
        if self.phase != Phase::Editing
            || [&self.name, &self.key, &self.email]
                .iter()
                .any(|i| i.read(cx).is_focused(window))
        {
            return false;
        }
        let key = event.keystroke.key.as_str();
        if self.step == 1 && !event.keystroke.modifiers.modified() {
            if let Some(ix) = key
                .parse::<usize>()
                .ok()
                .filter(|d| (1..=REGIONS.len()).contains(d))
            {
                self.pick_region(ix - 1, cx);
                return true;
            }
        }
        match key {
            "enter" => {
                self.advance(cx);
                true
            }
            "escape" if self.step > 0 => {
                self.go(self.step - 1, cx);
                true
            }
            _ => false,
        }
    }

    /// A single-choice pick lands, then the page moves on (Pink
    /// `Wizard::select` → `WizardStep::AutoAdvance`).
    fn pick_region(&mut self, ix: usize, cx: &mut Context<Self>) {
        if self.phase != Phase::Editing {
            return;
        }
        self.region = Some(ix);
        self.clear_error(&[Field::Region]);
        self.advance_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(AUTO_ADVANCE_MS))
                .await;
            this.update(cx, |s, cx| {
                if s.step == 1 {
                    s.advance(cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    fn add_invite(&mut self, text: SharedString, cx: &mut Context<Self>) {
        let email = text.trim().to_lowercase();
        if email.is_empty() {
            return;
        }
        if !valid_email(&email) {
            self.error = Some((Field::Email, "That doesn't look like an email address."));
        } else if self.invites.iter().any(|i| i.as_ref() == email) {
            self.error = Some((Field::Email, "Already on the list."));
        } else {
            self.invites.push(email.into());
            self.email.update(cx, |e, cx| e.clear(cx));
        }
        cx.notify();
    }

    /// Whether the page has its answer; the Primary dims to 0.4 until it
    /// does (Pink `can_advance`).
    fn can_advance(&self, cx: &App) -> bool {
        match self.step {
            0 => !self.name.read(cx).is_empty() && !self.key.read(cx).is_empty(),
            1 => self.region.is_some(),
            _ => true,
        }
    }

    /// Checks the current page; returns false (and flags the field) when it isn't done.
    fn validate(&mut self, cx: &App) -> bool {
        let failure = match self.step {
            0 => {
                let name = self.name.read(cx).text();
                let key = self.key.read(cx).text();
                let len = key.chars().count();
                if name.trim().chars().count() < 3 {
                    Some((
                        Field::Name,
                        "Give the project a name of 3 characters or more.",
                    ))
                } else if !(2..=5).contains(&len) || !key.chars().all(|c| c.is_ascii_uppercase()) {
                    Some((Field::Key, "Use 2 to 5 letters, A to Z, for the key."))
                } else {
                    None
                }
            }
            1 if self.region.is_none() => {
                Some((Field::Region, "Pick where the project's data should live."))
            }
            2 => {
                let pending = self.email.read(cx).text();
                (!pending.trim().is_empty() && !valid_email(pending.trim()))
                    .then_some((Field::Email, "That doesn't look like an email address."))
            }
            _ => None,
        };
        self.error = failure;
        failure.is_none()
    }

    fn go(&mut self, step: usize, cx: &mut Context<Self>) {
        if step == self.step || self.phase != Phase::Editing {
            return;
        }
        self.advance_task = None;
        self.step = step;
        self.epoch += 1;
        self.error = None;
        self.focus_field(
            match step {
                0 => Field::Name,
                2 => Field::Email,
                _ => Field::Region,
            },
            cx,
        );
        cx.notify();
    }

    fn advance(&mut self, cx: &mut Context<Self>) {
        if self.phase != Phase::Editing {
            return;
        }
        if !self.validate(cx) {
            if let Some((field, _)) = self.error {
                self.focus_field(field, cx);
            }
            cx.notify();
            return;
        }
        if self.step == 2 {
            let pending = self.email.read(cx).text();
            if !pending.trim().is_empty() {
                self.add_invite(pending, cx);
            }
        }
        if self.step + 1 < STEPS.len() {
            self.go(self.step + 1, cx);
            return;
        }
        self.phase = Phase::Creating;
        let project = self.project(cx);
        let on_complete = self.on_complete.clone();
        self.create_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(CREATE_MS))
                .await;
            this.update(cx, |s, cx| {
                s.phase = Phase::Done;
                s.epoch += 1;
                cx.notify();
            })
            .ok();
            if let Some(f) = on_complete {
                cx.update(|cx| {
                    if let Some(window) = cx.active_window() {
                        window
                            .update(cx, |_, window, cx| f(&project, window, cx))
                            .ok();
                    }
                });
            }
        }));
        cx.notify();
    }

    fn project(&self, cx: &App) -> NewProject {
        NewProject {
            name: self.name.read(cx).text().trim().to_string().into(),
            key: self.key.read(cx).text(),
            region: self.region.map(|r| REGIONS[r].1).unwrap_or_default(),
            invites: self.invites.clone(),
        }
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.name.update(cx, |i, cx| i.clear(cx));
        self.key.update(cx, |i, cx| i.clear(cx));
        self.email.update(cx, |i, cx| i.clear(cx));
        self.key_touched = false;
        self.region = None;
        self.invites.clear();
        self.phase = Phase::Editing;
        self.step = 0;
        self.epoch += 1;
        self.error = None;
        self.focus_field(Field::Name, cx);
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct StepperForm {
    id: ElementId,
    title: SharedString,
    on_complete: Option<CompleteHandler>,
    width: gpui::Pixels,
}

impl StepperForm {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: "New project".into(),
            on_complete: None,
            width: px(640.),
        }
    }

    /// The flow's name: the card's accessible label, and its header once
    /// the project exists.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Called once the project is created, with everything the flow collected.
    pub fn on_complete(
        mut self,
        handler: impl Fn(&NewProject, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_complete = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for StepperForm {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            FormState::new,
        );
        let on_complete = self.on_complete.clone();
        state.update(cx, |s, cx| {
            s.on_complete = on_complete;
            if s.colors_dark != Some(t.dark) {
                s.colors_dark = Some(t.dark);
                for input in [&s.name, &s.key, &s.email] {
                    input.update(cx, |i, _| {
                        i.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
                    });
                }
            }
        });

        let s = state.read(cx);
        let (phase, step, epoch) = (s.phase, s.step, s.epoch);
        let focus = s.focus.clone();
        let project = s.project(cx);

        let (header, counter): (SharedString, Option<SharedString>) = if phase == Phase::Done {
            (self.title.clone(), None)
        } else {
            (
                STEPS[step].0.into(),
                Some(format!("{}/{}", step + 1, STEPS.len()).into()),
            )
        };
        let (question, description): (SharedString, SharedString) = match (phase, step) {
            (Phase::Done, _) => (
                format!("{} is ready", project.name).into(),
                format!(
                    "{} invite{} sent, data in {}",
                    project.invites.len(),
                    if project.invites.len() == 1 { "" } else { "s" },
                    project.region
                )
                .into(),
            ),
            (_, 3) => (
                format!("Create {}?", project.name).into(),
                STEPS[3].2.into(),
            ),
            (_, ix) => (STEPS[ix].1.into(), STEPS[ix].2.into()),
        };
        let content = match (phase, step) {
            (Phase::Done, _) => None,
            (_, 0) => Some(step_project(&key, &state, &t, cx)),
            (_, 1) => Some(step_region(&key, &state, &t, cx)),
            (_, 2) => Some(step_team(&key, &state, &t, cx)),
            _ => Some(step_review(&key, &state, &t, cx)),
        };

        // Pink render_wizard: the question at mt 6 in 15/20 medium, a
        // 12 px line under it (the multi-select hint's recipe), the page's
        // controls at mt 12 / pb 4 (the answer field's wrapper).
        let page = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(15.))
                    .line_height(px(20.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(question),
            )
            .child(
                div()
                    .mt(px(4.))
                    .text_size(px(12.))
                    .text_color(t.readable_muted(0.65))
                    .child(description),
            )
            .map(|el| match content {
                Some(content) => el.child(div().mt(px(12.)).pb(px(4.)).child(content)),
                // The finished page has no controls; keep their room.
                None => el.pb(px(12.)),
            });
        // Pages swap in place: Pink's in-place content swap, a 160ms fade
        // with a 4 px settle.
        let page = if epoch == 0 {
            page.into_any_element()
        } else {
            motion::rise_in(
                SharedString::from(format!("{key}-page-{epoch}")),
                motion::TRANSCRIPT_IN,
                motion::TRANSCRIPT_IN_RISE,
                page,
            )
            .into_any_element()
        };

        let keys = state.clone();
        // Pink composer_surface (opaque): the composer pill's radius, edge,
        // input plate and shadow, which the wizard reuses.
        div()
            .id(self.id.clone())
            .aria_label(self.title.clone())
            .track_focus(&focus)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if keys.update(cx, |s, cx| s.on_key(event, window, cx)) {
                    cx.stop_propagation();
                }
            })
            .w(self.width)
            .flex()
            .flex_col()
            .rounded(px(radius::COMPOSER_PILL))
            .border_1()
            .border_color(t.border)
            .bg(t.surface_bg())
            .shadow(pink::shadow_lg())
            .font_family(pink::SANS)
            .text_size(px(14.))
            .text_color(t.text)
            .child(
                div()
                    .px(px(16.))
                    .pt(px(16.))
                    .flex()
                    .flex_col()
                    .child(wizard_header(header, counter, &t))
                    .child(page),
            )
            .child(footer(&key, &state, &t, cx))
    }
}

/// The wizard header (Pink render_wizard): the label tracked uppercase in
/// 11 medium, and the "1/3" counter chip (h 20, px 6, radius 6, `ink(0.06)`).
fn wizard_header(label: SharedString, counter: Option<SharedString>, t: &Pink) -> Div {
    let tone = t.readable_muted(0.6);
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(tone)
                .child(pink::tracked_upper(&label)),
        )
        .when_some(counter, |el, counter| {
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
                    .text_color(tone)
                    .child(counter),
            )
        })
}

/// The line under a page's controls: what Next found missing, in the error
/// copy tone (Pink accounts.rs login dialog, `mt 8`, 12), or a hint in the
/// wizard footer note's tone (`readable_muted(0.7)`).
fn note(copy: impl Into<SharedString>, error: bool, t: &Pink) -> Div {
    div()
        .mt(px(8.))
        .text_size(px(12.))
        .text_color(if error {
            t.readable_tone(t.danger_muted, 0.9)
        } else {
            t.readable_muted(0.7)
        })
        .child(copy.into())
}

/// A captioned field (Pink automations.rs `render_editor` `field`): an
/// 11/14 `readable_muted(0.85)` caption over `input_frame`, gap 4, growing
/// from `min`. Pink frames don't change on focus; the caret says it.
fn field(
    key: &str,
    label: &'static str,
    input: &Entity<TextInput>,
    min: f32,
    mono: bool,
    t: &Pink,
) -> Div {
    let focus = input.clone();
    div()
        .flex_1()
        .min_w(px(min))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(px(11.))
                .line_height(px(14.))
                .text_color(t.readable_muted(0.85))
                .child(label),
        )
        .child(
            pink::input_frame(t)
                .id(SharedString::from(format!("{key}-field-{label}")))
                .line_height(px(INPUT_LINE_HEIGHT))
                .text_color(t.text)
                .cursor_text()
                .when(mono, |el| el.font_family(pink::MONO))
                .on_click(move |_, window, cx| TextInput::focus(&focus, window, cx))
                .child(input.clone()),
        )
}

fn field_error(s: &FormState, field: Field) -> Option<&'static str> {
    s.error.filter(|(f, _)| *f == field).map(|(_, m)| m)
}

fn step_project(key: &str, state: &Entity<FormState>, t: &Pink, cx: &App) -> AnyElement {
    let s = state.read(cx);
    let project_key = s.key.read(cx).text();
    let error = field_error(s, Field::Name).or(field_error(s, Field::Key));
    let hint = (!project_key.is_empty())
        .then(|| format!("Issues will read {project_key}-1, {project_key}-2, …"));
    div()
        .flex()
        .flex_col()
        .child(
            pink::row_block()
                .items_start()
                .child(field(key, "Project name", &s.name, 160., false, t))
                .child(
                    field(key, "Key", &s.key, 96., true, t)
                        .flex_none()
                        .w(px(160.)),
                ),
        )
        .map(|el| match (error, hint) {
            (Some(message), _) => el.child(note(message, true, t)),
            (None, Some(hint)) => el.child(note(hint, false, t)),
            (None, None) => el,
        })
        .into_any_element()
}

/// One choice (Pink render_wizard option rows): px 14 / py 10, gap 12,
/// radius 12; picked rows take an `ink(0.16)` edge on `ink(0.09)`, others
/// ease `ink(0.025)` → `ink(0.06)` under the pointer; the label 13.5 medium
/// over an 11.5 description, and the 22 px number chip its digit picks.
fn option_row(
    key: &str,
    ix: usize,
    label: &'static str,
    description: SharedString,
    picked: bool,
    state: &Entity<FormState>,
    t: &Pink,
) -> Stateful<Div> {
    let hover = format!("{key}-option-{ix}");
    let accent = t.accent;
    let state = state.clone();
    div()
        .id(SharedString::from(hover.clone()))
        .role(Role::RadioButton)
        .aria_label(label)
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
        .gap(px(12.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(12.))
        .border_1()
        .border_color(if picked {
            t.ink(0.16)
        } else {
            gpui::transparent_black()
        })
        .bg(if picked {
            t.ink(0.09)
        } else {
            motion::hover_blend(&hover, t.ink(0.025), t.ink(0.06))
        })
        .on_hover(motion::hover_listener(hover))
        .cursor_pointer()
        .on_key_down(pink::swallow_activation_key)
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.pick_region(ix, cx)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .text_size(px(13.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(if picked { t.text } else { t.text.opacity(0.9) })
                .child(label)
                .child(
                    div()
                        .text_size(px(11.5))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(t.readable_muted(0.65))
                        .child(description),
                ),
        )
        .child(
            div()
                .flex_none()
                .size(px(22.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::SM))
                .bg(if picked { t.ink(0.16) } else { t.ink(0.05) })
                .text_size(px(11.))
                .text_color(if picked {
                    t.text
                } else {
                    t.readable_muted(0.6)
                })
                .child(SharedString::from((ix + 1).to_string())),
        )
}

fn step_region(key: &str, state: &Entity<FormState>, t: &Pink, cx: &App) -> AnyElement {
    let s = state.read(cx);
    let rows = REGIONS
        .iter()
        .enumerate()
        .map(|(ix, (city, code, latency))| {
            option_row(
                key,
                ix,
                city,
                format!("{code}, {latency}").into(),
                s.region == Some(ix),
                state,
                t,
            )
        });
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .id(SharedString::from(format!("{key}-options")))
                .role(Role::RadioGroup)
                .max_h(px(220.))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(rows),
        )
        .when_some(field_error(s, Field::Region), |el, message| {
            el.child(note(message, true, t))
        })
        .into_any_element()
}

/// A staged invite: Pink's message badge (min-h 24, px 8, radius 8,
/// `ink(0.06)`, 12 medium) holding the address and a 20 px remove control
/// (Pink's sidebar row action: `hover_wash`, glyph `text_muted` → `text`).
/// It arrives like a staged attachment (Pink `ATTACHMENT_IN`).
fn invite_chip(key: &str, email: &SharedString, state: &Entity<FormState>, t: &Pink) -> AnyElement {
    let remove = format!("{key}-invite-remove-{email}");
    let ring = t.focus_ring();
    let state = state.clone();
    let address = email.clone();
    motion::rise_in(
        SharedString::from(format!("{key}-invite-{email}")),
        motion::ATTACHMENT_IN,
        8.,
        pink::message_badge(t)
            .pr(px(2.))
            .child(email.clone())
            .child(
                div()
                    .id(SharedString::from(remove.clone()))
                    .role(Role::Button)
                    .aria_label(SharedString::from(format!("Remove {email}")))
                    .tab_index(0)
                    .focus_visible(move |style| pink::focus_ring(style, ring))
                    .size(px(pink::chrome::MIN_HIT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::inner(radius::MD, 2.)))
                    .cursor_pointer()
                    .hover_wash(remove.clone(), t.fill(FillTier::Tertiary))
                    .on_key_down(pink::swallow_activation_key)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.invites.retain(|e| *e != address);
                            cx.notify();
                        })
                    })
                    .child(
                        svg()
                            .path(ICON_CLOSE)
                            .size(px(10.))
                            .text_color(motion::hover_blend(&remove, t.text_muted, t.text)),
                    ),
            ),
    )
    .into_any_element()
}

fn step_team(key: &str, state: &Entity<FormState>, t: &Pink, cx: &App) -> AnyElement {
    let s = state.read(cx);
    let chips: Vec<AnyElement> = s
        .invites
        .iter()
        .map(|email| invite_chip(key, email, state, t))
        .collect();
    div()
        .flex()
        .flex_col()
        .child(pink::row_block().child(field(key, "Email address", &s.email, 160., false, t)))
        .child(match field_error(s, Field::Email) {
            Some(message) => note(message, true, t),
            None => note("Press Enter to add each address.", false, t),
        })
        .when(!chips.is_empty(), |el| {
            el.child(
                div()
                    .mt(px(10.))
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .children(chips),
            )
        })
        .into_any_element()
}

/// The review (Pink render_wizard approval context): label 11
/// `readable_muted(0.65)` over its value in 12 `text`, gap 2, rows gap 6;
/// each with an Edit text link (Pink accounts.rs `url_link`: 12,
/// `text_muted` → `text`).
fn step_review(key: &str, state: &Entity<FormState>, t: &Pink, cx: &App) -> AnyElement {
    let s = state.read(cx);
    let project = s.project(cx);
    let creating = s.phase == Phase::Creating;
    let region = s
        .region
        .map(|r| format!("{} ({})", REGIONS[r].0, REGIONS[r].1))
        .unwrap_or_else(|| "Not chosen".into());
    let invites = match project.invites.len() {
        0 => "Nobody yet".to_string(),
        1 => project.invites[0].to_string(),
        n => format!("{} and {} more", project.invites[0], n - 1),
    };
    // The key rides beside the name in mono: both are edited on page 1.
    let rows = [
        (
            "Project",
            project.name.to_string(),
            Some(project.key.clone()),
            0usize,
        ),
        ("Region", region, None, 1),
        ("Invites", invites, None, 2),
    ];
    let items = rows
        .into_iter()
        .enumerate()
        .map(|(ix, (label, value, code, step))| {
            let state = state.clone();
            let link = format!("{key}-edit-{ix}");
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(t.readable_muted(0.65))
                                .child(label),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(6.))
                                .text_size(px(12.))
                                .text_color(t.text)
                                .child(value)
                                .when_some(code, |el, code| {
                                    el.child(
                                        div()
                                            .font_family(pink::MONO)
                                            .text_color(t.readable_muted(0.85))
                                            .child(code),
                                    )
                                }),
                        ),
                )
                .when(!creating, |el| {
                    el.child(
                        div()
                            .id(SharedString::from(link.clone()))
                            .flex_none()
                            .text_size(px(12.))
                            .cursor_pointer()
                            .hover_fg(link, t.text_muted, t.text)
                            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.go(step, cx)))
                            .child("Edit"),
                    )
                })
        });
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .children(items)
        .into_any_element()
}

/// The wizard footer (Pink render_wizard): no hairline, px 16 / pb 16 /
/// pt 4, gap 12; quiet ghost actions on the left, one Primary (px 16) on
/// the right at 0.4 opacity until the page can advance, reading "Next",
/// "Create project" and "Creating…" (Pink "Next" / "Send" / "Sending…").
fn footer(key: &str, state: &Entity<FormState>, t: &Pink, cx: &App) -> impl IntoElement {
    let s = state.read(cx);
    let (phase, step) = (s.phase, s.step);
    let editing = phase == Phase::Editing;
    let ghost = |id: &str, label: &'static str| {
        pink::button(format!("{key}-{id}"), ButtonKind::Ghost, ControlSize::Lg, t)
            .aria_label(label)
            .child(label)
    };
    let back = (editing && step > 0).then(|| {
        let state = state.clone();
        ghost("back", "Back")
            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.go(s.step - 1, cx)))
    });
    let skip = (editing && step == 2 && s.invites.is_empty()).then(|| {
        let state = state.clone();
        ghost("skip", "Skip for now").on_click(move |_, _, cx| {
            state.update(cx, |s, cx| {
                s.email.update(cx, |e, cx| e.clear(cx));
                s.go(3, cx);
            })
        })
    });
    let restart = (phase == Phase::Done).then(|| {
        let state = state.clone();
        ghost("restart", "Start over")
            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.restart(cx)))
    });
    let label = match phase {
        Phase::Done => "Open project",
        Phase::Creating => "Creating…",
        Phase::Editing if step + 1 == STEPS.len() => "Create project",
        Phase::Editing => "Next",
    };
    let ready = phase == Phase::Done || (editing && s.can_advance(cx));
    let submit = {
        let state = state.clone();
        pink::button(
            format!("{key}-submit"),
            ButtonKind::Primary,
            ControlSize::Lg,
            t,
        )
        .aria_label(label)
        .px(px(16.))
        .when(!ready, |el| el.opacity(0.4))
        .on_click(move |_, _, cx| state.update(cx, |s, cx| s.advance(cx)))
        .child(label)
    };
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
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
                .children(back)
                .children(skip)
                .children(restart),
        )
        .child(submit)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| StepperForm::new("stepper-form"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_and_emails() {
        assert_eq!(derive_key("Northwind Ops"), "NO");
        assert_eq!(derive_key("atlas"), "ATL");
        assert!(valid_email("mia@northwind.dev"));
        assert!(!valid_email("mia@northwind"));
        assert!(!valid_email("@x.io"));
    }
}
