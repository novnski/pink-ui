//! Prompt Library – saved prompt templates you fill in place.
//!
//! An original in luna's design language. The library lists templates by
//! group; the open one reads as a sentence with blanks you type straight
//! into, each blank growing with its text. A ring and a row of variable
//! tokens track what is still missing, the assembled prompt updates below
//! with a soft flash, and "Use prompt" wakes up (with one pulse) once every
//! blank is filled.
//!
//! Keys: in the search field ↑/↓ switch templates and Enter jumps to the
//! first blank; in a blank Tab moves on, Enter uses the prompt when it is
//! complete (or jumps to the next empty blank), Escape returns to search.
//!
//! ```ignore
//! PromptLibrary::new("prompts", default_templates())
//!     .selected("follow-up")
//!     .on_use(|template, prompt, _, _| println!("{}: {prompt}", template.title))
//! ```

use std::{collections::HashMap, f32::consts::TAU, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    Animation, AnimationExt, AnyView, App, Context, ElementId, Entity, FontWeight, HighlightStyle,
    IntoElement, PathBuilder, ScrollHandle, SharedString, StyledText, Subscription, Task, TextRun,
    Window, canvas, div, font, point, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    luna::{self, ButtonKind, ControlSize, FillTier, Luna, radius, text},
    motion::{self, EASE, EASE_OUT_QUINT, MotionSpec, ROW_IN, VIEW_IN},
    primitives::{InputEvent, TextInput, ring},
    story,
};

pub const NAME: &str = "Prompt Library";
pub const DESCRIPTION: &str = "Saved prompt templates you fill in place – type into the blanks of the sentence, watch the prompt assemble below and use it once nothing is missing";

const ICON_SEARCH: &str = "icons/prompt-library/search.svg";
const ICON_COPY: &str = "icons/prompt-library/copy.svg";
const ICON_CHECK: &str = "icons/prompt-library/check.svg";
const ICON_ARROW: &str = "icons/prompt-library/arrow.svg";
const ICON_PULSE: &str = "icons/prompt-library/pulse.svg";
const ICON_BUG: &str = "icons/prompt-library/bug.svg";
const ICON_MAIL: &str = "icons/prompt-library/mail.svg";
const ICON_MEGAPHONE: &str = "icons/prompt-library/megaphone.svg";
const ICON_CHAT: &str = "icons/prompt-library/chat.svg";
const ICON_PLUS: &str = "icons/prompt-library/plus.svg";

const WIDTH: f32 = 780.;
const HEIGHT: f32 = 420.;
const SIDEBAR_W: f32 = 232.;
const ROW_H: f32 = 32.;
const HEADER_H: f32 = 26.;
const LIST_PAD: f32 = 4.;
const SENTENCE_SIZE: f32 = 15.;
const SLOT_PAD: f32 = 7.;

type UseHandler = Rc<dyn Fn(&PromptTemplate, &str, &mut Window, &mut App)>;

#[derive(Clone, Debug)]
pub struct PromptTemplate {
    pub id: SharedString,
    pub title: SharedString,
    pub group: SharedString,
    pub icon: Option<SharedString>,
    pub about: SharedString,
    /// The prompt, with `{variable}` blanks.
    pub body: SharedString,
    /// Values filled in from the start, by variable name.
    pub defaults: Vec<(SharedString, SharedString)>,
}

impl PromptTemplate {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            group: "Templates".into(),
            icon: None,
            about: SharedString::default(),
            body: body.into(),
            defaults: Vec::new(),
        }
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = group.into();
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn about(mut self, about: impl Into<SharedString>) -> Self {
        self.about = about.into();
        self
    }

    pub fn default_value(
        mut self,
        variable: impl Into<SharedString>,
        value: impl Into<SharedString>,
    ) -> Self {
        self.defaults.push((variable.into(), value.into()));
        self
    }

    /// The body split into text and blanks.
    fn parts(&self) -> Vec<Part> {
        let mut parts = Vec::new();
        let mut rest = self.body.as_ref();
        while let Some(open) = rest.find('{') {
            let Some(close) = rest[open..].find('}') else {
                break;
            };
            if open > 0 {
                parts.push(Part::Text(rest[..open].to_string()));
            }
            parts.push(Part::Blank(rest[open + 1..open + close].to_string()));
            rest = &rest[open + close + 1..];
        }
        if !rest.is_empty() {
            parts.push(Part::Text(rest.to_string()));
        }
        parts
    }

    /// Each variable once, in reading order.
    fn variables(&self) -> Vec<String> {
        let mut vars: Vec<String> = Vec::new();
        for part in self.parts() {
            if let Part::Blank(name) = part
                && !vars.contains(&name)
            {
                vars.push(name);
            }
        }
        vars
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Part {
    Text(String),
    Blank(String),
}

/// Five templates across Writing, Sales, Engineering and People.
pub fn default_templates() -> Vec<PromptTemplate> {
    vec![
        PromptTemplate::new(
            "status",
            "Weekly status update",
            "Summarize what {team} shipped this week for {audience}. Lead with {highlight} and keep it under {length} words.",
        )
        .group("Writing")
        .icon(ICON_PULSE)
        .about("A short, skimmable update for people outside the team.")
        .default_value("length", "150"),
        PromptTemplate::new(
            "launch",
            "Launch announcement",
            "Announce {feature} to {audience} as a {format}. Open with the problem it solves and close with {call to action}.",
        )
        .group("Writing")
        .icon(ICON_MEGAPHONE)
        .about("Lead with the problem, not the feature list."),
        PromptTemplate::new(
            "follow-up",
            "Customer follow-up",
            "Write a follow-up email to {name} at {company} after our {meeting} call. Mention {next step} and keep the tone {tone}.",
        )
        .group("Sales")
        .icon(ICON_MAIL)
        .about("Recap the call and agree on one clear next step.")
        .default_value("name", "Noor")
        .default_value("company", "Northwind")
        .default_value("tone", "warm"),
        PromptTemplate::new(
            "error",
            "Error explainer",
            "Explain this {language} error in plain words: {error}. Suggest {count} fixes, simplest first.",
        )
        .group("Engineering")
        .icon(ICON_BUG)
        .about("Turns a stack trace into something a teammate can act on.")
        .default_value("count", "three"),
        PromptTemplate::new(
            "interview",
            "Interview questions",
            "Draft {count} interview questions for a {role} that test {skill}, from warm-up to deep dive.",
        )
        .group("People")
        .icon(ICON_CHAT)
        .about("A structured set that gets harder as it goes."),
    ]
}

/// One blank's text field, keyed by template and variable.
struct Blank {
    input: Entity<TextInput>,
    _subscription: Subscription,
}

struct LibraryState {
    search: Entity<TextInput>,
    _subscription: Subscription,
    query: SharedString,
    selected: SharedString,
    blanks: HashMap<(SharedString, String), Blank>,
    /// Bumped on every edit, so the preview flashes.
    edits: u32,
    copied: Option<u32>,
    used: Option<u32>,
    flashes: u32,
    _timer: Option<Task<()>>,
    scroll: ScrollHandle,
    templates: Rc<Vec<PromptTemplate>>,
    on_use: Option<UseHandler>,
}

/// Visible templates grouped, in first-seen group order.
fn arranged<'a>(
    templates: &'a [PromptTemplate],
    query: &str,
) -> Vec<(SharedString, Vec<&'a PromptTemplate>)> {
    let q = query.trim().to_lowercase();
    let mut groups: Vec<(SharedString, Vec<&PromptTemplate>)> = Vec::new();
    for template in templates.iter().filter(|t| {
        q.is_empty()
            || t.title.to_lowercase().contains(&q)
            || t.group.to_lowercase().contains(&q)
            || t.body.to_lowercase().contains(&q)
    }) {
        match groups.iter_mut().find(|(g, _)| *g == template.group) {
            Some((_, rows)) => rows.push(template),
            None => groups.push((template.group.clone(), vec![template])),
        }
    }
    groups
}

impl LibraryState {
    fn visible(&self) -> Vec<PromptTemplate> {
        arranged(&self.templates, &self.query)
            .into_iter()
            .flat_map(|(_, rows)| rows.into_iter().cloned())
            .collect()
    }

    fn current(&self) -> Option<PromptTemplate> {
        self.templates
            .iter()
            .find(|t| t.id == self.selected)
            .cloned()
    }

    fn value(&self, template: &SharedString, var: &str, cx: &App) -> SharedString {
        self.blanks
            .get(&(template.clone(), var.to_string()))
            .map(|b| b.input.read(cx).text())
            .unwrap_or_default()
    }

    /// The blanks of the open template, in order.
    fn inputs(&self) -> Vec<Entity<TextInput>> {
        let Some(template) = self.current() else {
            return Vec::new();
        };
        template
            .variables()
            .iter()
            .filter_map(|v| self.blanks.get(&(template.id.clone(), v.clone())))
            .map(|b| b.input.clone())
            .collect()
    }

    fn assembled(&self, cx: &App) -> Option<(String, bool)> {
        let template = self.current()?;
        let mut out = String::new();
        let mut complete = true;
        for part in template.parts() {
            match part {
                Part::Text(text) => out.push_str(&text),
                Part::Blank(var) => {
                    let value = self.value(&template.id, &var, cx);
                    if value.trim().is_empty() {
                        complete = false;
                        out.push_str(&format!("{{{var}}}"));
                    } else {
                        out.push_str(value.trim());
                    }
                }
            }
        }
        Some((out, complete))
    }

    fn on_search(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                self.query = text.clone();
                let visible = self.visible();
                if !visible.iter().any(|t| t.id == self.selected)
                    && let Some(first) = visible.first()
                {
                    self.selected = first.id.clone();
                }
            }
            InputEvent::MoveDown | InputEvent::MoveUp => {
                let visible = self.visible();
                let len = visible.len();
                if len > 0 {
                    let at = visible.iter().position(|t| t.id == self.selected);
                    let next = match (at, *event == InputEvent::MoveDown) {
                        (None, _) => 0,
                        (Some(at), true) => (at + 1) % len,
                        (Some(at), false) => (at + len - 1) % len,
                    };
                    self.selected = visible[next].id.clone();
                }
            }
            InputEvent::Submit(_) => self.focus_next_empty(None, window, cx),
            InputEvent::Escape => {
                self.search.update(cx, |input, cx| input.clear(cx));
                self.query = SharedString::default();
            }
            _ => return,
        }
        cx.notify();
    }

    fn on_blank(
        &mut self,
        input: &Entity<TextInput>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputEvent::Change(_) => self.edits += 1,
            InputEvent::Tab => {
                let inputs = self.inputs();
                if let Some(at) = inputs.iter().position(|i| i == input) {
                    TextInput::focus(&inputs[(at + 1) % inputs.len()], window, cx);
                }
            }
            InputEvent::Submit(_) => {
                if self.assembled(cx).is_some_and(|(_, done)| done) {
                    self.use_prompt(window, cx);
                } else {
                    self.focus_next_empty(Some(input), window, cx);
                }
            }
            InputEvent::Escape => TextInput::focus(&self.search, window, cx),
            InputEvent::Focus | InputEvent::Blur => {}
            _ => return,
        }
        cx.notify();
    }

    /// Focuses the first empty blank after `from` (wrapping).
    fn focus_next_empty(
        &mut self,
        from: Option<&Entity<TextInput>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let inputs = self.inputs();
        let start = from
            .and_then(|f| inputs.iter().position(|i| i == f))
            .map_or(0, |at| at + 1);
        let len = inputs.len();
        for step in 0..len {
            let input = &inputs[(start + step) % len];
            if input.read(cx).text().trim().is_empty() {
                TextInput::focus(&input, window, cx);
                return;
            }
        }
        if let Some(first) = inputs.first() {
            TextInput::focus(&first, window, cx);
        }
    }

    fn use_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(template), Some((prompt, true))) = (self.current(), self.assembled(cx)) else {
            return;
        };
        self.flashes += 1;
        self.used = Some(self.flashes);
        self.reset_later(cx);
        if let Some(f) = self.on_use.clone() {
            window.defer(cx, move |window, cx| f(&template, &prompt, window, cx));
        }
        cx.notify();
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let Some((prompt, _)) = self.assembled(cx) else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(prompt));
        self.flashes += 1;
        self.copied = Some(self.flashes);
        self.reset_later(cx);
        cx.notify();
    }

    /// Clears the Copied / Sent confirmations after a moment.
    fn reset_later(&mut self, cx: &mut Context<Self>) {
        let flashes = self.flashes;
        self._timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(1600))
                .await;
            this.update(cx, |s, cx| {
                if s.flashes == flashes {
                    s.copied = None;
                    s.used = None;
                    cx.notify();
                }
            })
            .ok();
        }));
    }

    /// Creates the open template's blanks the first time it is shown.
    fn ensure_blanks(&mut self, window: &mut Window, cx: &mut Context<Self>, t: &Luna) {
        let Some(template) = self.current() else {
            return;
        };
        for var in template.variables() {
            let key = (template.id.clone(), var.clone());
            if self.blanks.contains_key(&key) {
                continue;
            }
            let default = template
                .defaults
                .iter()
                .find(|(name, _)| name.as_ref() == var)
                .map(|(_, value)| value.clone())
                .unwrap_or_default();
            let placeholder = var.clone();
            let input = cx.new(|cx| {
                TextInput::new(window, cx)
                    .placeholder(placeholder)
                    .default_text(default)
            });
            input.update(cx, |input, _| {
                input.set_colors(Some(t.text_faint), Some(t.accent), Some(t.selection))
            });
            let subscription =
                cx.subscribe_in(&input, window, |s: &mut Self, input, event, window, cx| {
                    s.on_blank(input, event, window, cx)
                });
            self.blanks.insert(
                key,
                Blank {
                    input,
                    _subscription: subscription,
                },
            );
        }
    }
}

#[derive(IntoElement)]
pub struct PromptLibrary {
    id: ElementId,
    templates: Vec<PromptTemplate>,
    selected: Option<SharedString>,
    focus_first_blank: bool,
    on_use: Option<UseHandler>,
}

impl PromptLibrary {
    pub fn new(id: impl Into<ElementId>, templates: Vec<PromptTemplate>) -> Self {
        Self {
            id: id.into(),
            templates,
            selected: None,
            focus_first_blank: false,
            on_use: None,
        }
    }

    /// The template open at first (by id); defaults to the first one.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Puts the caret in the first empty blank when the library first renders.
    pub fn focus_first_blank(mut self, focus: bool) -> Self {
        self.focus_first_blank = focus;
        self
    }

    /// Called with the template and the finished prompt.
    pub fn on_use(
        mut self,
        handler: impl Fn(&PromptTemplate, &str, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_use = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PromptLibrary {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let initial = self
            .selected
            .clone()
            .or_else(|| self.templates.first().map(|t| t.id.clone()))
            .unwrap_or_default();
        let focus_first = self.focus_first_blank;

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| {
                let search =
                    cx.new(|cx| TextInput::new(window, cx).placeholder("Search templates"));
                let subscription = cx.subscribe_in(
                    &search,
                    window,
                    |s: &mut LibraryState, _, event, window, cx| s.on_search(event, window, cx),
                );
                if focus_first {
                    let this = cx.entity();
                    window.defer(cx, move |window, cx| {
                        this.update(cx, |s, cx| s.focus_next_empty(None, window, cx))
                    });
                }
                LibraryState {
                    search,
                    _subscription: subscription,
                    query: SharedString::default(),
                    selected: initial,
                    blanks: HashMap::new(),
                    edits: 0,
                    copied: None,
                    used: None,
                    flashes: 0,
                    _timer: None,
                    scroll: ScrollHandle::new(),
                    templates: Rc::new(Vec::new()),
                    on_use: None,
                }
            },
        );

        let templates = Rc::new(self.templates);
        state.update(cx, |s, cx| {
            s.templates = templates.clone();
            s.on_use = self.on_use.clone();
            s.ensure_blanks(window, cx, &t);
            s.search.update(cx, |input, _| {
                input.set_colors(Some(t.text_faint), Some(t.accent), Some(t.selection))
            });
            for blank in s.blanks.values() {
                blank.input.update(cx, |input, _| {
                    input.set_colors(Some(t.text_faint), Some(t.accent), Some(t.selection))
                });
            }
        });

        let s = state.read(cx);
        let groups = arranged(&templates, &s.query);
        let current = s.current();
        let search = s.search.clone();
        let scroll = s.scroll.clone();
        let selected = s.selected.clone();

        let sidebar = sidebar(&key, &groups, &selected, &search, &state, &scroll, &t);
        let main = match current {
            Some(template) => editor(&key, &template, &state, window, cx, &t).into_any_element(),
            None => div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_color(t.text_faint)
                .child("No templates yet")
                .into_any_element(),
        };

        motion::keep_alive(window);
        div()
            .id(self.id)
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .flex()
            .font_family(luna::SANS)
            .text_color(t.text)
            .bg(t.surface_card)
            .border_1()
            .border_color(t.border)
            .rounded(px(radius::XL))
            .shadow(t.menu_shadow())
            .overflow_hidden()
            .child(sidebar)
            .child(main)
            .with_animation("prompt-library-in", ROW_IN.animation(), |el, k| {
                el.opacity(k).mt(px(8. * (1. - k)))
            })
    }
}

fn sidebar(
    key: &str,
    groups: &[(SharedString, Vec<&PromptTemplate>)],
    selected: &SharedString,
    search: &Entity<TextInput>,
    state: &Entity<LibraryState>,
    scroll: &ScrollHandle,
    t: &Luna,
) -> impl IntoElement {
    let count: usize = groups.iter().map(|(_, rows)| rows.len()).sum();
    let mut y = LIST_PAD;
    let mut plate: Option<f32> = None;
    for (_, rows) in groups {
        y += HEADER_H;
        for row in rows {
            if row.id == *selected {
                plate = Some(y);
            }
            y += ROW_H;
        }
    }
    let plate_top = motion::follow(
        &format!("{key}-plate"),
        plate.unwrap_or(LIST_PAD + HEADER_H),
        MotionSpec::new(240, EASE_OUT_QUINT),
    );
    let plate_bg = if t.dark {
        t.fill(FillTier::Secondary)
    } else {
        t.surface_card
    };

    let mut list = div()
        .id("templates")
        .relative()
        .flex_1()
        .min_h_0()
        .px(px(8.))
        .py(px(LIST_PAD))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(
            div()
                .absolute()
                .left(px(8.))
                .right(px(8.))
                .top(px(plate_top))
                .h(px(ROW_H))
                .rounded(px(radius::MD))
                .bg(plate_bg)
                .when(!t.dark, |el| {
                    el.border_1().border_color(t.border).shadow(t.card_shadow())
                })
                .when(plate.is_none(), |el| el.opacity(0.)),
        );
    for (group, rows) in groups {
        list = list.child(
            div()
                .h(px(HEADER_H))
                .px(px(8.))
                .pt(px(6.))
                .flex()
                .items_center()
                .text_size(px(text::CAPTION))
                .font_weight(FontWeight::MEDIUM)
                .text_color(t.text_faint)
                .child(group.clone()),
        );
        for template in rows {
            let picker = state.clone();
            let id = template.id.clone();
            let on = template.id == *selected;
            let lit = motion::follow(
                &format!("{key}-lit-{}", template.id),
                if on { 1. } else { 0. },
                MotionSpec::new(160, EASE),
            );
            list = list.child(
                div()
                    .id(SharedString::from(format!("template-{}", template.id)))
                    .relative()
                    .h(px(ROW_H))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .rounded(px(radius::MD))
                    .cursor_pointer()
                    .hover_fill(
                        format!("{key}-row-{}", template.id),
                        t.fill(FillTier::Tertiary).opacity(0.),
                        if on {
                            t.fill(FillTier::Tertiary).opacity(0.)
                        } else {
                            t.fill(FillTier::Tertiary)
                        },
                    )
                    .on_click(move |_, _, cx| {
                        picker.update(cx, |s, cx| {
                            s.selected = id.clone();
                            cx.notify();
                        })
                    })
                    .when_some(template.icon.clone(), |el, icon| {
                        el.child(
                            svg()
                                .path(icon)
                                .size(px(15.))
                                .flex_none()
                                .text_color(motion::mix(t.text_faint, t.accent, lit)),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::BODY))
                            .text_color(motion::mix(t.text_muted, t.text, lit))
                            .font_weight(if on {
                                FontWeight::MEDIUM
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(template.title.clone()),
                    ),
            );
        }
    }
    if groups.is_empty() {
        list = list.child(
            div()
                .pt(px(20.))
                .flex()
                .justify_center()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child("No templates match"),
        );
    }

    div()
        .w(px(SIDEBAR_W))
        .flex_none()
        .h_full()
        .flex()
        .flex_col()
        .bg(t.surface)
        .border_r_1()
        .border_color(t.border)
        .child(
            div()
                .h(px(48.))
                .pl(px(16.))
                .pr(px(10.))
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(px(text::CALLOUT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Library"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_faint)
                        .child(count.to_string()),
                )
                .child(luna::icon_button("pl-new", ICON_PLUS, ControlSize::Md, t)),
        )
        .child(
            div().px(px(12.)).pb(px(6.)).child(
                div()
                    .h(px(30.))
                    .px(px(9.))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .rounded(px(radius::MD))
                    .bg(t.input_bg)
                    .border_1()
                    .border_color(t.border)
                    .child(
                        svg()
                            .path(ICON_SEARCH)
                            .size(px(14.))
                            .flex_none()
                            .text_color(t.text_faint),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(text::BODY))
                            .line_height(px(18.))
                            .child(search.clone()),
                    ),
            ),
        )
        .child(list)
}

/// Text width at the sentence's size, for sizing a blank to its contents.
fn measure(text: &str, window: &mut Window) -> f32 {
    if text.is_empty() {
        return 0.;
    }
    let run = TextRun {
        len: text.len(),
        font: gpui::Font {
            weight: FontWeight::MEDIUM,
            ..font(luna::SANS)
        },
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line =
        window
            .text_system()
            .shape_line(text.to_string().into(), px(SENTENCE_SIZE), &[run], None);
    f32::from(line.width)
}

fn editor(
    key: &str,
    template: &PromptTemplate,
    state: &Entity<LibraryState>,
    window: &mut Window,
    cx: &mut App,
    t: &Luna,
) -> impl IntoElement {
    let s = state.read(cx);
    let vars = template.variables();
    let values: Vec<(String, SharedString)> = vars
        .iter()
        .map(|v| (v.clone(), s.value(&template.id, v, cx)))
        .collect();
    let filled = values.iter().filter(|(_, v)| !v.trim().is_empty()).count();
    let total = vars.len();
    let complete = filled == total;
    let (prompt, _) = s.assembled(cx).unwrap_or_default();
    let tokens = (prompt.chars().count() as f32 / 4.).round() as usize;
    let edits = s.edits;
    let copied = s.copied;
    let used = s.used;
    let inputs: HashMap<String, Entity<TextInput>> = vars
        .iter()
        .filter_map(|v| {
            s.blanks
                .get(&(template.id.clone(), v.clone()))
                .map(|b| (v.clone(), b.input.clone()))
        })
        .collect();

    // Header: title, about, and the fill ring.
    let progress = motion::follow(
        &format!("{key}-progress-{}", template.id),
        if total == 0 {
            1.
        } else {
            filled as f32 / total as f32
        },
        MotionSpec::new(420, EASE_OUT_QUINT),
    );
    let header = div()
        .flex()
        .items_start()
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
                        .text_size(px(text::TITLE))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(template.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(text::FOOTNOTE))
                        .text_color(t.text_muted)
                        .child(template.about.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(px(text::FOOTNOTE))
                .text_color(if complete { t.success } else { t.text_muted })
                .child(fill_ring(progress, complete, t))
                .child(format!("{filled} of {total}")),
        );

    // Variable tokens: a checklist that also jumps to its blank.
    let tokens_row = div()
        .flex()
        .flex_wrap()
        .gap(px(6.))
        .children(values.iter().map(|(var, value)| {
            let done = !value.trim().is_empty();
            let target = inputs.get(var).cloned();
            let k = motion::follow(
                &format!("{key}-token-{}-{var}", template.id),
                if done { 1. } else { 0. },
                MotionSpec::new(220, EASE_OUT_QUINT),
            );
            div()
                .id(SharedString::from(format!("token-{var}")))
                .h(px(22.))
                .px(px(7.))
                .flex()
                .items_center()
                .gap(px(4.))
                .rounded(px(radius::SM))
                .border_1()
                .border_color(motion::mix(t.border_strong, t.accent.opacity(0.0), k))
                .when(!done, |el| el.border_dashed())
                .bg(motion::mix(
                    t.fill(FillTier::Quaternary).opacity(0.),
                    t.accent_wash,
                    k,
                ))
                .text_size(px(text::CAPTION))
                .font_family(luna::MONO)
                .text_color(motion::mix(
                    t.text_muted,
                    if t.dark { t.text } else { t.accent },
                    k,
                ))
                .cursor_pointer()
                .when_some(target, |el, input| {
                    el.on_click(move |_, window, cx| TextInput::focus(&input, window, cx))
                })
                .when(done, |el| {
                    el.child(svg().path(ICON_CHECK).size(px(11.)).text_color(if t.dark {
                        t.text
                    } else {
                        t.accent
                    }))
                })
                .child(var.clone())
        }));

    // The sentence: words wrap freely; a blank stays glued to the
    // punctuation right after it.
    let mut atoms: Vec<Vec<gpui::AnyElement>> = vec![Vec::new()];
    let mut seen: Vec<String> = Vec::new();
    for part in template.parts() {
        match part {
            Part::Text(text) => {
                for (i, chunk) in text.split(' ').enumerate() {
                    if i > 0 {
                        atoms.push(Vec::new());
                    }
                    if !chunk.is_empty() {
                        atoms
                            .last_mut()
                            .unwrap()
                            .push(div().child(chunk.to_string()).into_any_element());
                    }
                }
            }
            Part::Blank(var) => {
                let first = !seen.contains(&var);
                seen.push(var.clone());
                let value = values
                    .iter()
                    .find(|(v, _)| *v == var)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default();
                let element = match inputs.get(&var).filter(|_| first) {
                    Some(input) => {
                        blank(key, &var, &value, input, window, cx, t).into_any_element()
                    }
                    None => div()
                        .text_color(t.accent)
                        .child(if value.is_empty() {
                            format!("{{{var}}}")
                        } else {
                            value.to_string()
                        })
                        .into_any_element(),
                };
                atoms.last_mut().unwrap().push(element);
            }
        }
    }
    let sentence = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_x(px(4.5))
        .gap_y(px(6.))
        .text_size(px(SENTENCE_SIZE))
        .line_height(px(28.))
        .text_color(t.text)
        .children(
            atoms
                .into_iter()
                .filter(|a| !a.is_empty())
                .map(|pieces| div().flex().items_center().children(pieces)),
        );

    // The assembled prompt: values bright, open blanks faint.
    let mut highlights: Vec<(std::ops::Range<usize>, HighlightStyle)> = Vec::new();
    let mut preview = String::new();
    for part in template.parts() {
        match part {
            Part::Text(text) => preview.push_str(&text),
            Part::Blank(var) => {
                let value = values
                    .iter()
                    .find(|(v, _)| *v == var)
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default();
                let start = preview.len();
                let style = if value.is_empty() {
                    // Underscores keep a blank's name on one line when it wraps.
                    preview.push_str(&var.replace(' ', "_"));
                    HighlightStyle {
                        color: Some(t.text_faint),
                        underline: Some(gpui::UnderlineStyle {
                            thickness: px(1.),
                            color: Some(t.text_faint),
                            wavy: false,
                        }),
                        ..Default::default()
                    }
                } else {
                    preview.push_str(&value);
                    HighlightStyle {
                        color: Some(t.text),
                        font_weight: Some(FontWeight::SEMIBOLD),
                        ..Default::default()
                    }
                };
                highlights.push((start..preview.len(), style));
            }
        }
    }
    let preview_box = div()
        .p(px(12.))
        .rounded(px(radius::MD))
        .bg(t.fill(FillTier::Quaternary))
        .border_1()
        .border_color(t.border)
        .font_family(luna::MONO)
        .text_size(px(12.5))
        .line_height(px(20.))
        .text_color(t.text_muted)
        .child(StyledText::new(preview).with_highlights(highlights))
        .with_animation(
            ElementId::NamedInteger(format!("{key}-preview-flash").into(), u64::from(edits)),
            MotionSpec::new(700, EASE).animation(),
            {
                let (accent, border) = (t.accent.opacity(0.55), t.border);
                move |el, k| {
                    el.border_color(motion::mix(accent, border, if edits == 0 { 1. } else { k }))
                }
            },
        );

    let footer = footer(
        key,
        template,
        complete,
        total - filled,
        tokens,
        copied,
        used,
        state,
        t,
    );

    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .child(
            div()
                .id("editor")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(px(22.))
                .pt(px(18.))
                .pb(px(14.))
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(header)
                .child(tokens_row)
                .child(sentence)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .text_size(px(text::CAPTION))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text_faint)
                                .child("Assembled prompt")
                                .child(format!("≈ {tokens} tokens")),
                        )
                        .child(preview_box),
                )
                .with_animation(
                    SharedString::from(format!("{key}-editor-{}", template.id)),
                    VIEW_IN.animation(),
                    |el, k| el.opacity(k).mt(px(6. * (1. - k))),
                ),
        )
        .child(footer)
}

/// One blank in the sentence: a text field as wide as its text.
fn blank(
    key: &str,
    var: &str,
    value: &SharedString,
    input: &Entity<TextInput>,
    window: &mut Window,
    cx: &mut App,
    t: &Luna,
) -> impl IntoElement {
    let focused = input.read(cx).is_focused(window);
    let filled = !value.trim().is_empty();
    let shown = if value.is_empty() {
        var
    } else {
        value.as_ref()
    };
    // Room for the caret after the last letter.
    let width = measure(shown, window) + 2. * SLOT_PAD + 5.;
    let k = motion::follow(
        &format!("{key}-blank-{var}"),
        if filled { 1. } else { 0. },
        MotionSpec::new(220, EASE_OUT_QUINT),
    );
    let f = motion::follow(
        &format!("{key}-blank-focus-{var}"),
        if focused { 1. } else { 0. },
        MotionSpec::new(160, EASE),
    );
    let ink = if t.dark { t.text } else { t.accent };
    div()
        .h(px(28.))
        .w(px(width))
        .px(px(SLOT_PAD))
        .flex()
        .items_center()
        .rounded(px(radius::SM))
        .border_1()
        .when(!filled && !focused, |el| el.border_dashed())
        .border_color(motion::mix(
            motion::mix(t.border_strong, t.accent.opacity(0.0), k),
            t.accent,
            f,
        ))
        .shadow(vec![ring(3. * f, t.focus_ring().opacity(0.3 * f))])
        .bg(motion::mix(t.fill(FillTier::Quaternary), t.accent_wash, k))
        .font_weight(FontWeight::MEDIUM)
        .line_height(px(20.))
        .text_color(motion::mix(t.text, ink, k))
        .child(input.clone())
}

fn fill_ring(progress: f32, complete: bool, t: &Luna) -> impl IntoElement {
    let track = t.fill(FillTier::Primary);
    let color = if complete { t.success } else { t.accent };
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let center = bounds.center();
            let r = f32::from(bounds.size.width) / 2. - 1.5;
            let arc = |from: f32, to: f32| {
                let mut path = PathBuilder::stroke(px(2.5));
                let steps = ((to - from) * 64.).ceil().max(1.) as usize;
                for i in 0..=steps {
                    let a = (from + (to - from) * i as f32 / steps as f32) * TAU - TAU / 4.;
                    let p = point(center.x + px(r * a.cos()), center.y + px(r * a.sin()));
                    if i == 0 {
                        path.move_to(p);
                    } else {
                        path.line_to(p);
                    }
                }
                path.build().ok()
            };
            if let Some(path) = arc(0., 1.) {
                window.paint_path(path, track);
            }
            if progress > 0.005
                && let Some(path) = arc(0., progress.min(1.))
            {
                window.paint_path(path, color);
            }
        },
    )
    .size(px(18.))
    .flex_none()
}

#[allow(clippy::too_many_arguments)]
fn footer(
    key: &str,
    template: &PromptTemplate,
    complete: bool,
    missing: usize,
    tokens: usize,
    copied: Option<u32>,
    used: Option<u32>,
    state: &Entity<LibraryState>,
    t: &Luna,
) -> impl IntoElement {
    let copier = state.clone();
    let user = state.clone();
    let ready = motion::follow(
        &format!("{key}-ready-{}", template.id),
        if complete { 1. } else { 0. },
        MotionSpec::new(240, EASE_OUT_QUINT),
    );
    let status: SharedString = if complete {
        "Ready to use".into()
    } else if missing == 1 {
        "1 blank left".into()
    } else {
        format!("{missing} blanks left").into()
    };

    let copy = luna::button("pl-copy", ButtonKind::Secondary, ControlSize::Md, t)
        .on_click(move |_, _, cx| copier.update(cx, |s, cx| s.copy(cx)))
        .child(
            svg()
                .path(if copied.is_some() {
                    ICON_CHECK
                } else {
                    ICON_COPY
                })
                .size(px(14.))
                .text_color(if copied.is_some() {
                    t.success
                } else {
                    t.text_muted
                }),
        )
        .child(if copied.is_some() { "Copied" } else { "Copy" });

    let arrow_key = format!("{key}-use");
    let nudge = motion::hover_t(&arrow_key) * 2. * ready;
    let use_button = luna::button("pl-use", ButtonKind::Primary, ControlSize::Md, t)
        .opacity(0.4 + 0.6 * ready)
        .when(!complete, |el| el.cursor_default())
        .when(complete, |el| {
            el.on_click(move |_, window, cx| user.update(cx, |s, cx| s.use_prompt(window, cx)))
        })
        .child(if used.is_some() {
            "Sent to chat"
        } else {
            "Use prompt"
        })
        .child(
            div().pl(px(nudge)).child(
                svg()
                    .path(if used.is_some() {
                        ICON_CHECK
                    } else {
                        ICON_ARROW
                    })
                    .size(px(14.))
                    .text_color(t.on_solid),
            ),
        );
    // One soft pulse when the prompt becomes complete.
    let use_button = div()
        .id(SharedString::from(arrow_key.clone()))
        .rounded(px(radius::MD))
        .on_hover(motion::hover_listener(arrow_key))
        .child(use_button);
    let use_button = if complete {
        let glow = t.accent;
        use_button
            .with_animation(
                SharedString::from(format!("{key}-pulse-{}", template.id)),
                Animation::new(Duration::from_millis(1100)).with_easing(EASE_OUT_QUINT.easing()),
                move |el, k| el.shadow(vec![ring(6. * k, glow.opacity(0.35 * (1. - k)))]),
            )
            .into_any_element()
    } else {
        use_button.into_any_element()
    };

    div()
        .h(px(52.))
        .px(px(16.))
        .flex()
        .items_center()
        .gap(px(8.))
        .border_t_1()
        .border_color(t.border)
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(text::FOOTNOTE))
                .text_color(if complete { t.success } else { t.text_faint })
                .when(complete, |el| {
                    el.child(svg().path(ICON_CHECK).size(px(13.)).text_color(t.success))
                })
                .child(status)
                .child(
                    div()
                        .text_color(t.text_faint)
                        .child(format!("· {tokens} tokens")),
                ),
        )
        .child(copy)
        .child(use_button)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        PromptLibrary::new("prompt-library", default_templates())
            .selected("follow-up")
            .focus_first_blank(true)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_text_and_blanks() {
        let template = PromptTemplate::new("t", "T", "Hi {name}, meet {next step}. Bye {name}");
        assert_eq!(
            template.parts(),
            vec![
                Part::Text("Hi ".into()),
                Part::Blank("name".into()),
                Part::Text(", meet ".into()),
                Part::Blank("next step".into()),
                Part::Text(". Bye ".into()),
                Part::Blank("name".into()),
            ]
        );
        assert_eq!(template.variables(), vec!["name", "next step"]);
    }

    #[test]
    fn groups_keep_first_seen_order() {
        let templates = default_templates();
        let groups: Vec<_> = arranged(&templates, "")
            .into_iter()
            .map(|(group, rows)| (group, rows.len()))
            .collect();
        assert_eq!(
            groups.first().map(|(g, n)| (g.as_ref(), *n)),
            Some(("Writing", 2))
        );
        assert_eq!(arranged(&templates, "stack trace").len(), 0);
        assert_eq!(arranged(&templates, "error").len(), 1);
    }
}
