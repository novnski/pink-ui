//! Stepper Form – a multi-step setup flow with a vertical step rail.
//!
//! The rail on the left numbers the steps, checks them off and fills the
//! line between them as you go; finished steps can be revisited. Each step
//! slides in from the side it comes from while the card's height glides to
//! fit it. Continue validates the step in place (the offending field shakes
//! and says why), and the last step plays a short "creating" beat before a
//! success screen.
//!
//! The demo creates a project: name and key, data region, invites, review.
//!
//! ```ignore
//! StepperForm::new("new-project").on_complete(|project, _, _| create(project))
//! ```

use std::{cell::Cell, f32::consts::PI, rc::Rc, time::Duration};

use gpui::{
    Animation, AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, IntoElement,
    SharedString, Subscription, Task, Transformation, Window, canvas, div, prelude::*, px, radians,
    svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, radius, space, text},
    motion::{self, AnimationExt, GLIDE, MotionSpec},
    primitives::{InputEvent, TextInput, ring},
    story,
};

pub const NAME: &str = "Stepper Form";
pub const DESCRIPTION: &str = "A multi-step setup flow with a vertical rail that checks steps off, steps that slide in, a card that resizes to each step, inline validation and a success screen";

const ICON_CHECK: &str = "icons/stepper-form/check.svg";
const ICON_BACK: &str = "icons/stepper-form/arrow-left.svg";
const ICON_NEXT: &str = "icons/stepper-form/arrow-right.svg";
const ICON_GLOBE: &str = "icons/stepper-form/globe.svg";
const ICON_REMOVE: &str = "icons/stepper-form/x.svg";
const ICON_MAIL: &str = "icons/stepper-form/mail.svg";
const ICON_SPARKLE: &str = "icons/stepper-form/sparkle.svg";
const ICON_SPINNER: &str = "icons/stepper-form/spinner.svg";

const STEP_IN: MotionSpec = MotionSpec::new(380, motion::EASE_OUT_QUINT);
const RAIL_W: f32 = 196.;

/// The four steps: title and the line under it in the rail.
const STEPS: [(&str, &str); 4] = [
    ("Project", "Name and key"),
    ("Region", "Where data lives"),
    ("Team", "Invite people"),
    ("Review", "Check and create"),
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
    _subscriptions: Vec<Subscription>,
    colors_dark: Option<bool>,
    key_touched: bool,
    region: Option<usize>,
    invites: Vec<SharedString>,
    step: usize,
    /// Furthest step reached, for the rail.
    reached: usize,
    dir: f32,
    epoch: usize,
    /// Field with an error after the last Continue, and how often it shook.
    error: Option<(Field, &'static str)>,
    shakes: usize,
    phase: Phase,
    task: Option<Task<()>>,
    height: Rc<Cell<f32>>,
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
                    InputEvent::Submit(_) => s.advance_from_event(cx),
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
                        s.add_invite(text.clone(), cx);
                    }
                    _ => {}
                },
            ),
        ];
        Self {
            name,
            key,
            email,
            _subscriptions: subs,
            colors_dark: None,
            key_touched: false,
            region: None,
            invites: Vec::new(),
            step: 0,
            reached: 0,
            dir: 1.,
            epoch: 0,
            error: None,
            shakes: 0,
            phase: Phase::Editing,
            task: None,
            height: Rc::default(),
            on_complete: None,
        }
    }

    fn clear_error(&mut self, fields: &[Field]) {
        if self.error.is_some_and(|(f, _)| fields.contains(&f)) {
            self.error = None;
        }
    }

    /// Focus moves happen on the next frame, where a window is at hand.
    fn focus_field(&mut self, field: Field, cx: &mut Context<Self>) {
        let input = match field {
            Field::Name => self.name.clone(),
            Field::Key => self.key.clone(),
            Field::Email => self.email.clone(),
            Field::Region => return,
        };
        cx.defer(move |cx| {
            if let Some(window) = cx.active_window() {
                window
                    .update(cx, |_, window, cx| input.read(cx).focus(window))
                    .ok();
            }
        });
    }

    fn add_invite(&mut self, text: SharedString, cx: &mut Context<Self>) {
        let email = text.trim().to_lowercase();
        if email.is_empty() {
            return;
        }
        if !valid_email(&email) {
            self.fail(Field::Email, "That doesn't look like an email address");
        } else if self.invites.iter().any(|i| i.as_ref() == email) {
            self.fail(Field::Email, "Already on the list");
        } else {
            self.invites.push(email.into());
            self.email.update(cx, |e, cx| e.clear(cx));
        }
        cx.notify();
    }

    fn fail(&mut self, field: Field, message: &'static str) {
        self.error = Some((field, message));
        self.shakes += 1;
    }

    /// Checks the current step; returns false (and flags the field) when it isn't done.
    fn validate(&mut self, cx: &App) -> bool {
        match self.step {
            0 => {
                let name = self.name.read(cx).text();
                let key = self.key.read(cx).text();
                if name.trim().chars().count() < 3 {
                    self.fail(
                        Field::Name,
                        "Give the project a name of 3 characters or more",
                    );
                    return false;
                }
                let len = key.chars().count();
                if !(2..=5).contains(&len) || !key.chars().all(|c| c.is_ascii_uppercase()) {
                    self.fail(Field::Key, "Use 2 to 5 letters, A to Z");
                    return false;
                }
                true
            }
            1 if self.region.is_none() => {
                self.fail(Field::Region, "Pick where the project's data should live");
                false
            }
            2 => {
                let pending = self.email.read(cx).text();
                if !pending.trim().is_empty() {
                    // Typed but not added yet: add it on the way out.
                    if !valid_email(pending.trim()) {
                        self.fail(Field::Email, "That doesn't look like an email address");
                        return false;
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn go(&mut self, step: usize, cx: &mut Context<Self>) {
        if step == self.step {
            return;
        }
        self.dir = if step > self.step { 1. } else { -1. };
        self.step = step;
        self.reached = self.reached.max(step);
        self.epoch += 1;
        self.error = None;
        match step {
            0 => self.focus_field(Field::Name, cx),
            2 => self.focus_field(Field::Email, cx),
            _ => {}
        }
        cx.notify();
    }

    fn advance_from_event(&mut self, cx: &mut Context<Self>) {
        self.advance(cx);
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
        self.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1300))
                .await;
            this.update(cx, |s, cx| {
                s.phase = Phase::Done;
                s.reached = STEPS.len();
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
                })
                .ok();
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
        self.reached = 0;
        self.phase = Phase::Editing;
        self.dir = -1.;
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

    /// The heading above the rail.
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
        let element = self.build(window, cx);
        // Followers started while building need frames: the story's own
        // keep_alive ran before this component rendered.
        motion::keep_alive(window);
        element
    }
}

impl StepperForm {
    fn build(self, window: &mut Window, cx: &mut App) -> AnyElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
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
                        i.set_colors(Some(t.text_faint), Some(t.accent), Some(t.selection))
                    });
                }
            }
        });

        let s = state.read(cx);
        let phase = s.phase;
        let rail = rail(&key, &state, &self.title, &t, cx);

        let content: AnyElement = if phase == Phase::Done {
            success(&key, &state, &t, cx)
        } else {
            let body = match s.step {
                0 => step_project(&key, &state, &t, window, cx),
                1 => step_region(&key, &state, &t, cx),
                2 => step_team(&key, &state, &t, window, cx),
                _ => step_review(&key, &state, &t, cx),
            };
            let (step, dir, epoch) = (s.step, s.dir, s.epoch);
            let heading = div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .mb(px(space::S16))
                .child(
                    div()
                        .text_size(px(text::CAPTION))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(t.text_faint)
                        .child(format!("Step {} of {}", step + 1, STEPS.len())),
                )
                .child(
                    div()
                        .text_size(px(text::TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(match step {
                            0 => "Name your project",
                            1 => "Choose a data region",
                            2 => "Invite your team",
                            _ => "Review and create",
                        }),
                );
            let page = div().flex().flex_col().child(heading).child(body);
            if epoch == 0 {
                page.into_any_element()
            } else {
                page.with_animation(
                    SharedString::from(format!("{key}-page-{epoch}")),
                    STEP_IN.animation(),
                    move |el, p| el.relative().left(px(28. * dir * (1. - p))).opacity(p),
                )
                .into_any_element()
            }
        };

        // The content box glides to each step's natural height.
        let measured = s.height.clone();
        let natural = measured.get();
        let height =
            (natural > 0.).then(|| motion::follow(&format!("{key}-height"), natural, GLIDE));

        let footer = (phase != Phase::Done).then(|| footer(&key, &state, &t, cx));

        luna::card(&t)
            .id(self.id.clone())
            .w(self.width)
            .flex()
            .font_family(luna::SANS)
            .text_color(t.text)
            .text_size(px(text::BODY))
            .overflow_hidden()
            .child(rail)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .overflow_hidden()
                            .when_some(height, |d, h| d.h(px(h)))
                            .child(
                                div()
                                    .relative()
                                    .px(px(space::S24))
                                    .pt(px(space::S24))
                                    .pb(px(space::S20))
                                    .child(content)
                                    .child(measure(measured)),
                            ),
                    )
                    .children(footer),
            )
            .into_any_element()
    }
}

fn rail(
    key: &str,
    state: &Entity<FormState>,
    title: &SharedString,
    t: &Luna,
    cx: &App,
) -> impl IntoElement {
    let s = state.read(cx);
    let done_all = s.phase == Phase::Done;
    let editing = s.phase == Phase::Editing;
    let steps = STEPS.iter().enumerate().map(|(ix, (name, hint))| {
        let current = ix == s.step && !done_all;
        let done = done_all || ix < s.step || (ix <= s.reached && ix != s.step && ix < s.reached);
        let reachable = editing && ix <= s.reached && ix != s.step;
        let fill = motion::follow(
            &format!("{key}-line-{ix}"),
            if done_all || ix < s.step { 1. } else { 0. },
            MotionSpec::new(420, motion::EASE_OUT_QUINT).with_delay(60),
        );
        let state = state.clone();
        let hover = format!("{key}-rail-{ix}");
        let dot = div()
            .size(px(24.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .text_size(px(text::CAPTION))
            .font_weight(FontWeight::SEMIBOLD)
            .map(|d| {
                if done {
                    d.bg(t.accent_strong).child(
                        svg()
                            .path(ICON_CHECK)
                            .size(px(13.))
                            .text_color(t.on_accent)
                            .with_animation(
                                SharedString::from(format!("{key}-check-{ix}-{}", s.epoch)),
                                MotionSpec::new(300, motion::EASE_OUT_QUINT).animation(),
                                |el, p| el.size(px(5. + 8. * p)),
                            ),
                    )
                } else if current {
                    d.border_1()
                        .border_color(t.accent)
                        .bg(t.accent_wash)
                        .shadow(vec![ring(3., t.accent.opacity(0.12))])
                        .text_color(if t.dark { t.text } else { t.accent })
                        .child((ix + 1).to_string())
                } else {
                    d.border_1()
                        .border_color(t.border_strong)
                        .text_color(t.text_faint)
                        .child((ix + 1).to_string())
                }
            });
        let connector = (ix + 1 < STEPS.len()).then(|| {
            div()
                .absolute()
                .left(px(11.5))
                .top(px(28.))
                .w(px(1.5))
                .h(px(28.))
                .rounded(px(1.))
                .bg(t.border_strong)
                .child(
                    div()
                        .w_full()
                        .h(gpui::relative(fill))
                        .rounded(px(1.))
                        .bg(t.accent),
                )
        });
        div()
            .id(SharedString::from(hover.clone()))
            .relative()
            .flex()
            .gap(px(space::S10))
            .h(px(56.))
            .when(reachable, |d| d.cursor_pointer())
            .on_hover(motion::hover_listener(hover.clone()))
            .on_click(move |_, _, cx| {
                if reachable {
                    state.update(cx, |s, cx| s.go(ix, cx));
                }
            })
            .child(dot)
            .children(connector)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .pt(px(2.))
                    .child(
                        div()
                            .text_size(px(text::BODY))
                            .line_height(px(20.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(if current || done {
                                t.text
                            } else if reachable {
                                motion::hover_blend(&hover, t.text_muted, t.text)
                            } else {
                                t.text_muted
                            })
                            .child(*name),
                    )
                    .child(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .text_color(t.text_faint)
                            .child(*hint),
                    ),
            )
    });

    div()
        .w(px(RAIL_W))
        .flex_none()
        .flex()
        .flex_col()
        .p(px(space::S20))
        .bg(t.fill(FillTier::Quaternary))
        .border_r_1()
        .border_color(t.border)
        .child(
            div()
                .mb(px(space::S20))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(text::CALLOUT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.text)
                        .child(title.clone()),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child("About two minutes"),
                ),
        )
        .children(steps)
}

/// A labelled field around a text input, with its error under it.
#[allow(clippy::too_many_arguments)]
fn text_field(
    key: &str,
    label: &'static str,
    input: &Entity<TextInput>,
    prefix: Option<&'static str>,
    hint: Option<SharedString>,
    error: Option<&'static str>,
    shakes: usize,
    t: &Luna,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let focused = input.read(cx).is_focused(window);
    let edge = if error.is_some() {
        t.danger.opacity(0.7)
    } else if focused {
        t.accent.opacity(0.6)
    } else {
        t.border
    };
    let focus = input.clone();
    let shell = div()
        .id(SharedString::from(format!("{key}-{label}")))
        .flex()
        .items_center()
        .h(px(34.))
        .px(px(space::S10))
        .rounded(px(radius::MD))
        .border_1()
        .border_color(edge)
        .bg(t.input_bg)
        .cursor_text()
        .when(focused, |d| {
            d.shadow(vec![ring(
                3.,
                if error.is_some() { t.danger } else { t.accent }.opacity(0.14),
            )])
        })
        .on_click(move |_, window, cx| focus.read(cx).focus(window))
        .when_some(prefix, |d, p| {
            d.child(div().text_color(t.text_faint).child(p))
        })
        .child(div().flex_1().min_w_0().child(input.clone()));
    let shell = shake(&format!("{key}-{label}"), error.is_some(), shakes, shell);

    div()
        .flex()
        .flex_col()
        .gap(px(space::S6))
        .child(
            div()
                .text_size(px(text::FOOTNOTE))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_muted)
                .child(label),
        )
        .child(shell)
        .child(match error {
            Some(message) => div()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.danger)
                .child(message)
                .into_any_element(),
            None => div()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .children(hint)
                .into_any_element(),
        })
        .into_any_element()
}

/// Shakes `el` sideways once per failed Continue.
fn shake<E: IntoElement + Styled + 'static>(
    key: &str,
    active: bool,
    shakes: usize,
    el: E,
) -> AnyElement {
    if !active || shakes == 0 {
        return el.into_any_element();
    }
    el.with_animation(
        SharedString::from(format!("{key}-shake-{shakes}")),
        Animation::new(Duration::from_millis(360)),
        |el, p| el.relative().left(px((p * PI * 5.).sin() * 5. * (1. - p))),
    )
    .into_any_element()
}

fn step_project(
    key: &str,
    state: &Entity<FormState>,
    t: &Luna,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let s = state.read(cx);
    let error = |f: Field| s.error.filter(|(e, _)| *e == f).map(|(_, m)| m);
    let project_key = s.key.read(cx).text();
    let key_hint: SharedString = if project_key.is_empty() {
        "Prefixes every issue in the project".into()
    } else {
        format!("Issues will read {project_key}-1, {project_key}-2, …").into()
    };
    div()
        .flex()
        .flex_col()
        .gap(px(space::S16))
        .child(text_field(
            key,
            "Project name",
            &s.name,
            None,
            Some("Shown in the sidebar and in links".into()),
            error(Field::Name),
            s.shakes,
            t,
            window,
            cx,
        ))
        .child(div().w(px(200.)).child(text_field(
            key,
            "Project key",
            &s.key,
            None,
            Some(key_hint),
            error(Field::Key),
            s.shakes,
            t,
            window,
            cx,
        )))
        .into_any_element()
}

fn step_region(key: &str, state: &Entity<FormState>, t: &Luna, cx: &App) -> AnyElement {
    let s = state.read(cx);
    let error = s.error.filter(|(f, _)| *f == Field::Region).map(|(_, m)| m);
    let cards = REGIONS
        .iter()
        .enumerate()
        .map(|(ix, (city, code, latency))| {
            let selected = s.region == Some(ix);
            let state = state.clone();
            let hover = format!("{key}-region-{ix}");
            let pick = motion::follow(
                &format!("{hover}-pick"),
                if selected { 1. } else { 0. },
                MotionSpec::new(220, motion::EASE_OUT_QUINT),
            );
            div()
                .id(SharedString::from(hover.clone()))
                .flex()
                .items_center()
                .gap(px(space::S12))
                .px(px(space::S12))
                .h(px(52.))
                .rounded(px(radius::LG))
                .border_1()
                .cursor_pointer()
                .map(|d| {
                    if selected {
                        d.border_color(t.accent.opacity(0.7))
                            .bg(t.accent_wash.opacity(if t.dark { 0.5 } else { 0.6 }))
                            .shadow(vec![ring(3., t.accent.opacity(0.10))])
                    } else {
                        d.hover_colors(
                            hover.clone(),
                            crate::interaction::HoverColors {
                                bg: t.input_bg,
                                border: if error.is_some() {
                                    t.danger.opacity(0.5)
                                } else {
                                    t.border
                                },
                                fg: t.text,
                            },
                            crate::interaction::HoverColors {
                                bg: t.fill(FillTier::Quaternary),
                                border: t.border_strong,
                                fg: t.text,
                            },
                        )
                    }
                })
                .on_click(move |_, _, cx| {
                    state.update(cx, |s, cx| {
                        s.region = Some(ix);
                        s.clear_error(&[Field::Region]);
                        cx.notify();
                    })
                })
                .child(
                    div()
                        .size(px(30.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(radius::MD))
                        .bg(if selected {
                            t.accent_wash
                        } else {
                            t.fill(FillTier::Tertiary)
                        })
                        .child(
                            svg()
                                .path(ICON_GLOBE)
                                .size(px(15.))
                                .text_color(if selected { t.accent } else { t.text_muted }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(*city),
                        )
                        .child(
                            div()
                                .text_size(px(text::FOOTNOTE))
                                .font_family(luna::MONO)
                                .text_color(t.text_faint)
                                .child(*code),
                        ),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child(*latency),
                )
                // Radio dot.
                .child(
                    div()
                        .size(px(16.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .border_1()
                        .border_color(motion::mix(t.border_strong, t.accent, pick))
                        .child(div().size(px(8. * pick)).rounded_full().bg(t.accent)),
                )
        });
    let list = div().flex().flex_col().gap(px(space::S8)).children(cards);
    div()
        .flex()
        .flex_col()
        .gap(px(space::S8))
        .child(shake(
            &format!("{key}-regions"),
            error.is_some(),
            s.shakes,
            list,
        ))
        .child(match error {
            Some(m) => div()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.danger)
                .child(m),
            None => div()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child("You can't move a project between regions later."),
        })
        .into_any_element()
}

fn step_team(
    key: &str,
    state: &Entity<FormState>,
    t: &Luna,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let s = state.read(cx);
    let error = s.error.filter(|(f, _)| *f == Field::Email).map(|(_, m)| m);
    let chips = s.invites.iter().enumerate().map(|(ix, email)| {
        let state = state.clone();
        let email_for_remove = email.clone();
        motion::rise_in(
            SharedString::from(format!("{key}-invite-{email}")),
            motion::MENU_IN,
            3.,
            div()
                .flex()
                .items_center()
                .gap(px(space::S6))
                .h(px(26.))
                .pl(px(space::S8))
                .pr(px(2.))
                .rounded(px(radius::SM))
                .bg(t.fill(FillTier::Tertiary))
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text)
                .child(svg().path(ICON_MAIL).size(px(12.)).text_color(t.text_faint))
                .child(email.clone())
                .child(
                    luna::icon_button(
                        format!("{key}-remove-{ix}"),
                        ICON_REMOVE,
                        ControlSize::Sm,
                        t,
                    )
                    .size(px(22.))
                    .on_click(move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            s.invites.retain(|e| *e != email_for_remove);
                            cx.notify();
                        })
                    }),
                ),
        )
    });
    let count = s.invites.len();
    div()
        .flex()
        .flex_col()
        .gap(px(space::S12))
        .child(text_field(
            key,
            "Email address",
            &s.email,
            None,
            Some("Press Enter to add · invites can also be sent later".into()),
            error,
            s.shakes,
            t,
            window,
            cx,
        ))
        .child(if count == 0 {
            div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(56.))
                .rounded(px(radius::MD))
                .border_1()
                .border_dashed()
                .border_color(t.border)
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child("No one invited yet")
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .gap(px(space::S6))
                .children(chips)
                .into_any_element()
        })
        .into_any_element()
}

fn step_review(key: &str, state: &Entity<FormState>, t: &Luna, cx: &App) -> AnyElement {
    let s = state.read(cx);
    let project = s.project(cx);
    let creating = s.phase == Phase::Creating;
    let region = s
        .region
        .map(|r| format!("{} · {}", REGIONS[r].0, REGIONS[r].1))
        .unwrap_or_else(|| "–".into());
    let invites = match project.invites.len() {
        0 => "Nobody yet".to_string(),
        1 => project.invites[0].to_string(),
        n => format!("{} and {} more", project.invites[0], n - 1),
    };
    let rows = [
        ("Project", project.name.to_string(), 0usize),
        ("Key", project.key.to_string(), 0),
        ("Region", region, 1),
        ("Invites", invites, 2),
    ];
    let items = rows
        .into_iter()
        .enumerate()
        .map(|(ix, (label, value, step))| {
            let state = state.clone();
            let link = format!("{key}-edit-{ix}");
            div()
                .flex()
                .items_center()
                .gap(px(space::S12))
                .h(px(40.))
                .px(px(space::S12))
                .when(ix > 0, |d| d.border_t_1().border_color(t.border))
                .child(
                    div()
                        .w(px(72.))
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .when(label == "Key", |d| d.font_family(luna::MONO))
                        .child(value),
                )
                .when(!creating, |d| {
                    d.child(
                        div()
                            .id(SharedString::from(link.clone()))
                            .text_size(px(text::FOOTNOTE))
                            .font_weight(FontWeight::MEDIUM)
                            .cursor_pointer()
                            .hover_fg(link, t.accent.opacity(0.8), t.accent)
                            .child("Edit")
                            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.go(step, cx))),
                    )
                })
        });
    div()
        .flex()
        .flex_col()
        .rounded(px(radius::LG))
        .border_1()
        .border_color(t.border)
        .bg(t.input_bg)
        .children(items)
        .into_any_element()
}

fn footer(key: &str, state: &Entity<FormState>, t: &Luna, cx: &App) -> impl IntoElement {
    let s = state.read(cx);
    let step = s.step;
    let creating = s.phase == Phase::Creating;
    let last = step + 1 == STEPS.len();
    let back = (step > 0 && !creating).then(|| {
        let state = state.clone();
        luna::button(format!("{key}-back"), ButtonKind::Ghost, ControlSize::Lg, t)
            .child(luna::glyph(ICON_BACK, ControlSize::Lg).text_color(t.text_muted))
            .child("Back")
            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.go(s.step - 1, cx)))
    });
    let skip = (step == 2 && s.invites.is_empty()).then(|| {
        let state = state.clone();
        luna::button(format!("{key}-skip"), ButtonKind::Ghost, ControlSize::Lg, t)
            .child("Skip for now")
            .on_click(move |_, _, cx| {
                state.update(cx, |s, cx| {
                    s.email.update(cx, |e, cx| e.clear(cx));
                    s.go(3, cx);
                })
            })
    });
    let next = {
        let state = state.clone();
        let arrow_key = format!("{key}-next");
        let shift = motion::hover_t(&format!("luna-btn-{arrow_key}")) * 2.;
        luna::button(arrow_key, ButtonKind::Primary, ControlSize::Lg, t)
            .min_w(px(112.))
            .map(|b| {
                if creating {
                    b.child(
                        svg()
                            .path(ICON_SPINNER)
                            .size(px(14.))
                            .text_color(t.on_solid)
                            .with_animation(
                                SharedString::from(format!("{key}-spin")),
                                Animation::new(Duration::from_millis(760)).repeat(),
                                |svg, p| {
                                    svg.with_transformation(Transformation::rotate(radians(
                                        p * 2. * PI,
                                    )))
                                },
                            ),
                    )
                    .child("Creating…")
                } else if last {
                    b.child("Create project")
                } else {
                    b.child("Continue").child(
                        div()
                            .w(px(16.))
                            .pl(px(shift))
                            .child(luna::glyph(ICON_NEXT, ControlSize::Lg).text_color(t.on_solid)),
                    )
                }
            })
            .on_click(move |_, _, cx| state.update(cx, |s, cx| s.advance(cx)))
    };
    div()
        .flex()
        .items_center()
        .gap(px(space::S8))
        .px(px(space::S24))
        .py(px(space::S16))
        .border_t_1()
        .border_color(t.border)
        .children(back)
        .child(div().flex_1())
        .children(skip)
        .child(next)
}

fn success(key: &str, state: &Entity<FormState>, t: &Luna, cx: &App) -> AnyElement {
    let s = state.read(cx);
    let project = s.project(cx);
    let epoch = s.epoch;
    let restart = state.clone();
    let badge = div()
        .size(px(56.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(t.success.opacity(if t.dark { 0.16 } else { 0.10 }))
        .child(
            svg()
                .path(ICON_CHECK)
                .size(px(26.))
                .text_color(t.success)
                .with_animation(
                    SharedString::from(format!("{key}-done-check-{epoch}")),
                    MotionSpec::new(460, motion::EASE_OUT_QUINT)
                        .with_delay(120)
                        .animation(),
                    |el, p| el.size(px(8. + 18. * p)).opacity(p),
                ),
        )
        .with_animation(
            SharedString::from(format!("{key}-done-badge-{epoch}")),
            MotionSpec::new(420, motion::EASE_OUT_QUINT).animation(),
            |el, p| el.opacity(p),
        );
    let line = |delay: u64, el: gpui::Div| {
        el.with_animation(
            SharedString::from(format!("{key}-done-line-{delay}-{epoch}")),
            MotionSpec::new(380, motion::EASE_OUT_QUINT)
                .with_delay(delay)
                .animation(),
            |el, p| el.relative().top(px(6. * (1. - p))).opacity(p),
        )
    };
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(space::S12))
        .py(px(space::S20))
        .child(badge)
        .child(line(
            180,
            div()
                .text_size(px(text::TITLE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(t.text)
                .child(format!("{} is ready", project.name)),
        ))
        .child(line(
            240,
            div()
                .text_size(px(text::BODY))
                .text_color(t.text_muted)
                .child(format!(
                    "{} invite{} sent · data in {}",
                    project.invites.len(),
                    if project.invites.len() == 1 { "" } else { "s" },
                    project.region
                )),
        ))
        .child(line(
            300,
            div()
                .flex()
                .gap(px(space::S8))
                .mt(px(space::S8))
                .child(
                    luna::button(
                        format!("{key}-again"),
                        ButtonKind::Ghost,
                        ControlSize::Lg,
                        t,
                    )
                    .child("Start over")
                    .on_click(move |_, _, cx| restart.update(cx, |s, cx| s.restart(cx))),
                )
                .child(
                    luna::button(
                        format!("{key}-open"),
                        ButtonKind::Primary,
                        ControlSize::Lg,
                        t,
                    )
                    .child(luna::glyph(ICON_SPARKLE, ControlSize::Lg).text_color(t.on_solid))
                    .child("Open project"),
                ),
        ))
        .into_any_element()
}

/// Records its parent's height in `cell` (and asks for a frame when it changes).
fn measure(cell: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            if (cell.get() - h).abs() > 0.5 {
                cell.set(h);
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
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
