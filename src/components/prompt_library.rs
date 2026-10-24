//! Prompt Library – saved prompts in a menu above the composer, filled in
//! place.
//!
//! A Pink component. The library is Pink's saved-prompts menu (Pink:
//! composer.rs `render_stash_menu`): a full-width stack popup above the
//! composer pill whose rows open a saved prompt, with the prompt's blank
//! count and a Delete button. The open prompt sits in the composer surface
//! the way Pink's question wizard does (Pink: composer.rs `composer_surface`
//! and the wizard's header and footer): a tracked title with a counter chip,
//! the prompt as a sentence whose blanks you type straight into (painted
//! like the composer's inline chips), a hint line, Copy, and one Primary
//! "Use prompt" that wakes up once nothing is missing.
//!
//! Keys: menu rows are tab stops (Enter opens one); in a blank Tab moves on,
//! Enter uses the prompt when it is complete (or jumps to the next empty
//! blank), Escape closes the menu.
//!
//! ```ignore
//! PromptLibrary::new("prompts", default_templates())
//!     .selected("follow-up")
//!     .open(true)
//!     .on_use(|template, prompt, _, _| println!("{}: {prompt}", template.title))
//! ```

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

use gpui::{
    AnyElement, AnyView, App, ClipboardItem, Context, ElementId, Entity, FontWeight, IntoElement,
    KeyDownEvent, MouseButton, Role, SharedString, Subscription, Task, TextRun, Window, deferred,
    div, font, prelude::*, px, svg,
};
use web_time::Instant;

use crate::{
    motion::{self, HOVER_FADE, MENU_OUT, MENU_RISE_UP, TRANSCRIPT_IN, TRANSCRIPT_IN_RISE},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, layout, loaders, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Prompt Library";
pub const DESCRIPTION: &str = "Saved prompts in a menu above the composer – open one, type into the blanks of its sentence and use it once nothing is missing";

const ICON_NOTEBOOK: &str = "icons/prompt-library/notebook.svg";

/// The composer's input metrics (Pink composer.rs `INPUT_TEXT_SIZE`,
/// `INPUT_LINE_HEIGHT`: `text-[14px] leading-relaxed`).
const INPUT_TEXT_SIZE: f32 = 14.0;
const INPUT_LINE_HEIGHT: f32 = 22.75;
/// The saved-prompt list's height cap (Pink `render_stash_menu`).
const STASH_LIST_MAX_H: f32 = 220.0;
/// Inline chip geometry (Pink composer.rs chip quads): the wash sits 2 px
/// inside the line box top and bottom, at radius 5.
const CHIP_INSET_Y: f32 = 2.0;
const CHIP_RADIUS: f32 = 5.0;
/// Side room inside an editable blank, so the caret never touches its edge.
const CHIP_PAD_X: f32 = 4.0;
/// How long the Copied / Sent note stays.
const NOTE_MS: u64 = 1600;
/// Room the story keeps above the pill for the popup: the list cap, the
/// card's inset and edge, and the 6 px gap to the pill.
const MENU_ROOM: f32 = STASH_LIST_MAX_H + 2.0 * layout::STACK_POPUP_INSET + 2.0 + 6.0;

type UseHandler = Rc<dyn Fn(&PromptTemplate, &str, &mut Window, &mut App)>;
type DeleteHandler = Rc<dyn Fn(&PromptTemplate, &mut Window, &mut App)>;

#[derive(Clone, Debug)]
pub struct PromptTemplate {
    pub id: SharedString,
    pub title: SharedString,
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
            body: body.into(),
            defaults: Vec::new(),
        }
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

/// One piece of the sentence: a word, or a blank.
#[derive(Clone, Debug, PartialEq)]
enum Piece {
    Word(String),
    Blank(String),
}

/// The sentence as unbreakable groups: words wrap freely, and a blank stays
/// glued to the punctuation right before or after it.
fn atoms(parts: &[Part]) -> Vec<Vec<Piece>> {
    let mut atoms: Vec<Vec<Piece>> = vec![Vec::new()];
    for part in parts {
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
                            .push(Piece::Word(chunk.to_string()));
                    }
                }
            }
            Part::Blank(var) => atoms.last_mut().unwrap().push(Piece::Blank(var.clone())),
        }
    }
    atoms.retain(|atom| !atom.is_empty());
    atoms
}

/// Five saved prompts for writing, sales, engineering and hiring.
pub fn default_templates() -> Vec<PromptTemplate> {
    vec![
        PromptTemplate::new(
            "status",
            "Weekly status update",
            "Summarize what {team} shipped this week for {audience}. Lead with {highlight} and keep it under {length} words.",
        )
        .default_value("length", "150"),
        PromptTemplate::new(
            "launch",
            "Launch announcement",
            "Announce {feature} to {audience} as a {format}. Open with the problem it solves and close with {call to action}.",
        ),
        PromptTemplate::new(
            "follow-up",
            "Customer follow-up",
            "Write a follow-up email to {name} at {company} after our {meeting} call. Mention {next step} and keep the tone {tone}.",
        )
        .default_value("name", "Noor")
        .default_value("company", "Northwind")
        .default_value("tone", "warm"),
        PromptTemplate::new(
            "error",
            "Error explainer",
            "Explain this {language} error in plain words: {error}. Suggest {count} fixes, simplest first.",
        )
        .default_value("count", "three"),
        PromptTemplate::new(
            "interview",
            "Interview questions",
            "Draft {count} interview questions for a {role} that test {skill}, from warm-up to deep dive.",
        ),
    ]
}

/// One blank's text field, keyed by template and variable.
struct Blank {
    input: Entity<TextInput>,
    _subscription: Subscription,
}

/// The transient line in the footer after an action.
#[derive(Clone, Copy, PartialEq)]
enum Note {
    Copied,
    Sent,
}

struct LibraryState {
    selected: SharedString,
    open: bool,
    /// Bumped on every opening, so the menu's entrance replays.
    opens: u32,
    /// When the menu began closing; it fades out over `MENU_OUT`.
    closing: Option<Instant>,
    deleted: HashSet<SharedString>,
    blanks: HashMap<(SharedString, String), Blank>,
    note: Option<Note>,
    notes: u32,
    _note_timer: Option<Task<()>>,
    _reap: Option<Task<()>>,
    templates: Rc<Vec<PromptTemplate>>,
    on_use: Option<UseHandler>,
    on_delete: Option<DeleteHandler>,
}

impl LibraryState {
    /// The templates not deleted from the menu, in order.
    fn visible(&self) -> Vec<PromptTemplate> {
        self.templates
            .iter()
            .filter(|t| !self.deleted.contains(&t.id))
            .cloned()
            .collect()
    }

    fn current(&self) -> Option<PromptTemplate> {
        self.visible().into_iter().find(|t| t.id == self.selected)
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

    /// Opens or closes the menu; a closing menu fades out over `MENU_OUT`
    /// before it unmounts (Pink `completion_exits`).
    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        if open == self.open {
            return;
        }
        self.open = open;
        if open {
            self.opens += 1;
            self.closing = None;
            self._reap = None;
        } else {
            self.closing = Some(Instant::now());
            let reap = MENU_OUT.total().mul_f32(motion::speed_scale());
            self._reap = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(reap).await;
                this.update(cx, |s, cx| {
                    s.closing = None;
                    cx.notify();
                })
                .ok();
            }));
        }
        cx.notify();
    }

    /// Opens a saved prompt in the composer: the menu closes and the caret
    /// lands in its first empty blank.
    fn pick(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = id;
        self.note = None;
        self.ensure_blanks(window, cx);
        self.set_open(false, cx);
        self.focus_next_empty(None, window, cx);
        cx.notify();
    }

    fn delete(&mut self, id: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        let Some(template) = self.templates.iter().find(|t| t.id == id).cloned() else {
            return;
        };
        self.deleted.insert(id.clone());
        if self.selected == id
            && let Some(first) = self.visible().first()
        {
            self.selected = first.id.clone();
            self.ensure_blanks(window, cx);
        }
        if let Some(f) = self.on_delete.clone() {
            window.defer(cx, move |window, cx| f(&template, window, cx));
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
            InputEvent::Change(_) => self.note = None,
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
            InputEvent::Escape => self.set_open(false, cx),
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
                TextInput::focus(input, window, cx);
                return;
            }
        }
        if let Some(first) = inputs.first() {
            TextInput::focus(first, window, cx);
        }
    }

    fn use_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(template), Some((prompt, true))) = (self.current(), self.assembled(cx)) else {
            return;
        };
        self.show_note(Note::Sent, cx);
        if let Some(f) = self.on_use.clone() {
            window.defer(cx, move |window, cx| f(&template, &prompt, window, cx));
        }
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let Some((prompt, _)) = self.assembled(cx) else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(prompt));
        self.show_note(Note::Copied, cx);
    }

    /// Shows `note` in the footer, then clears it after a moment.
    fn show_note(&mut self, note: Note, cx: &mut Context<Self>) {
        self.notes += 1;
        self.note = Some(note);
        let notes = self.notes;
        self._note_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(NOTE_MS))
                .await;
            this.update(cx, |s, cx| {
                if s.notes == notes {
                    s.note = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Creates the open template's blanks the first time it is shown.
    fn ensure_blanks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    open: bool,
    loading: bool,
    focus_first_blank: bool,
    on_use: Option<UseHandler>,
    on_delete: Option<DeleteHandler>,
}

impl PromptLibrary {
    pub fn new(id: impl Into<ElementId>, templates: Vec<PromptTemplate>) -> Self {
        Self {
            id: id.into(),
            templates,
            selected: None,
            open: false,
            loading: false,
            focus_first_blank: false,
            on_use: None,
            on_delete: None,
        }
    }

    /// The saved prompt open at first (by id); defaults to the first one.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Shows the saved-prompts menu when the library first renders.
    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    /// The library is still loading: an empty menu says so, and its rows
    /// take no clicks.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
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

    /// Gives every menu row a Delete button; the row leaves the menu and the
    /// handler gets the template.
    pub fn on_delete(
        mut self,
        handler: impl Fn(&PromptTemplate, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PromptLibrary {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let initial = self
            .selected
            .clone()
            .or_else(|| self.templates.first().map(|t| t.id.clone()))
            .unwrap_or_default();
        let (open, focus_first) = (self.open, self.focus_first_blank);

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx: &mut Context<LibraryState>| {
                if focus_first {
                    let this = cx.entity();
                    window.defer(cx, move |window, cx| {
                        this.update(cx, |s, cx| s.focus_next_empty(None, window, cx))
                    });
                }
                LibraryState {
                    selected: initial,
                    open,
                    opens: u32::from(open),
                    closing: None,
                    deleted: HashSet::new(),
                    blanks: HashMap::new(),
                    note: None,
                    notes: 0,
                    _note_timer: None,
                    _reap: None,
                    templates: Rc::new(Vec::new()),
                    on_use: None,
                    on_delete: None,
                }
            },
        );

        let templates = Rc::new(self.templates);
        state.update(cx, |s, cx| {
            s.templates = templates;
            s.on_use = self.on_use.clone();
            s.on_delete = self.on_delete.clone();
            s.ensure_blanks(window, cx);
            for blank in s.blanks.values() {
                blank.input.update(cx, |input, _| {
                    input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
                });
            }
        });

        let pill = composer(&key, &state, window, cx, &t);
        let menu = stash_menu(&key, &state, self.loading, cx, &t);
        let closer = state.clone();

        motion::keep_alive(window);
        div()
            .id(self.id)
            .relative()
            .w_full()
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    closer.update(cx, |s, cx| s.set_open(false, cx));
                }
            })
            .child(pill)
            .children(menu)
    }
}

/// The saved-prompts menu (Pink: composer.rs `render_stash_menu` on
/// popover.rs `full_width_menu_above`): a full-width `stack_popup_card`
/// 6 px above the pill, rising 6 px in over `MENU_IN` and fading out over
/// `MENU_OUT`. Rows are menu rows on the popup's 14 pt corner: the title
/// truncating, an 11 `text_muted` blank count and a Ghost Sm Delete.
fn stash_menu(
    key: &str,
    state: &Entity<LibraryState>,
    loading: bool,
    cx: &App,
    t: &Pink,
) -> Option<AnyElement> {
    let s = state.read(cx);
    let exit = match (s.open, s.closing) {
        (true, _) => None,
        (false, Some(since)) => Some(motion::menu_exit_progress(since)),
        (false, None) => return None,
    };
    let busy = loading || exit.is_some();
    let visible = s.visible();
    let ring = t.focus_ring();

    let mut rows: Vec<AnyElement> = Vec::new();
    if visible.is_empty() {
        rows.push(
            div()
                .px(px(8.))
                .py(px(8.))
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child(if loading {
                    "Loading saved prompts…"
                } else {
                    "No saved prompts yet"
                })
                .into_any_element(),
        );
    }
    for template in visible {
        let blanks = template.variables().len();
        let active = template.id == s.selected;
        let (picker, deleter) = (state.clone(), state.clone());
        let (pick_id, delete_id) = (template.id.clone(), template.id.clone());
        let label = template.title.clone();
        rows.push(
            pink::menu_row(format!("{key}-stash-row-{}", template.id), active, t)
                .rounded(px(radius::STACK_POPUP_ROW))
                .role(Role::Button)
                .aria_label(format!("Open saved prompt: {label}"))
                .when(!busy, |row| {
                    row.tab_index(0)
                        .focus_visible(move |style| pink::focus_ring(style, ring))
                        .on_click(move |_, window, cx| {
                            picker.update(cx, |s, cx| s.pick(pick_id.clone(), window, cx))
                        })
                        .on_key_down(pink::swallow_activation_key)
                })
                .when(busy, |row| row.cursor_default())
                .child(div().flex_1().min_w_0().truncate().child(label.clone()))
                .when(blanks > 0, |row| {
                    row.child(
                        div()
                            .flex_none()
                            .text_size(px(11.))
                            .text_color(t.text_muted)
                            .child(format!(
                                "{blanks} blank{}",
                                if blanks == 1 { "" } else { "s" }
                            )),
                    )
                })
                .when(s.on_delete.is_some(), |row| {
                    row.child(
                        pink::button_enabled(
                            format!("{key}-stash-delete-{}", template.id),
                            ButtonKind::Ghost,
                            ControlSize::Sm,
                            !busy,
                            t,
                        )
                        .aria_label(format!("Delete saved prompt: {label}"))
                        .when(!busy, |button| {
                            button.on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                deleter.update(cx, |s, cx| s.delete(delete_id.clone(), window, cx))
                            })
                        })
                        .child("Delete"),
                    )
                })
                .into_any_element(),
        );
    }

    let card = pink::stack_popup_card(t).w_full().child(
        div()
            .id(SharedString::from(format!("{key}-stash-list")))
            .max_h(px(STASH_LIST_MAX_H))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(layout::MENU_GAP))
            .children(rows),
    );
    let inner = div().occlude().pb(px(layout::POPOVER_GAP)).child(card);
    let layer = match exit {
        // A dying menu takes no clicks.
        Some(progress) => motion::menu_out(
            SharedString::from(format!("{key}-stash-out")),
            progress,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(
            ElementId::NamedInteger(format!("{key}-stash-in").into(), u64::from(s.opens)),
            MENU_RISE_UP,
            inner,
        )
        .into_any_element(),
    };
    Some(
        div()
            .absolute()
            .bottom_full()
            .left_0()
            .right_0()
            .child(deferred(layer).priority(1))
            .into_any_element(),
    )
}

/// Text width at the composer's input size, for sizing a blank to its text.
fn measure(text: &str, window: &mut Window) -> f32 {
    if text.is_empty() {
        return 0.;
    }
    let run = TextRun {
        len: text.len(),
        font: font(pink::SANS),
        color: gpui::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line =
        window
            .text_system()
            .shape_line(text.to_string().into(), px(INPUT_TEXT_SIZE), &[run], None);
    f32::from(line.width)
}

/// A filled blank's colors: Pink's inline folder chip (composer.rs
/// `composer_chip_palette`): `accent` text on accent at 0.17 dark / 0.10
/// light.
fn chip_palette(t: &Pink) -> (gpui::Hsla, gpui::Hsla) {
    (t.accent, t.accent.opacity(if t.dark { 0.17 } else { 0.10 }))
}

/// The pill holding the open prompt (Pink: composer.rs `composer_surface`
/// with the question wizard's layout): radius 26, a 1 px `border`, the
/// input plate and `shadow_lg`; `px 16 / pt 16` body with a tracked header
/// and counter chip; a footer of quiet controls and notes on the left and
/// one Primary on the right.
fn composer(
    key: &str,
    state: &Entity<LibraryState>,
    window: &mut Window,
    cx: &App,
    t: &Pink,
) -> AnyElement {
    let s = state.read(cx);
    let current = s.current();
    let open = s.open;
    let note = s.note;
    let (values, inputs): (
        Vec<(String, SharedString)>,
        HashMap<String, Entity<TextInput>>,
    ) = match &current {
        Some(template) => {
            let vars = template.variables();
            let values = vars
                .iter()
                .map(|v| (v.clone(), s.value(&template.id, v, cx)))
                .collect();
            let inputs = vars
                .iter()
                .filter_map(|v| {
                    s.blanks
                        .get(&(template.id.clone(), v.clone()))
                        .map(|b| (v.clone(), b.input.clone()))
                })
                .collect();
            (values, inputs)
        }
        None => (Vec::new(), HashMap::new()),
    };
    let filled = values.iter().filter(|(_, v)| !v.trim().is_empty()).count();
    let total = values.len();
    let complete = current.is_some() && filled == total;
    let muted = t.readable_muted(0.6);

    // Header (Pink wizard): tracked uppercase 11 medium + a counter chip.
    let title = current
        .as_ref()
        .map_or("Saved prompts".into(), |c| c.title.clone());
    let header = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(muted)
                .child(SharedString::from(pink::tracked_upper(&title))),
        )
        .when(total > 0, |el| {
            el.child(
                div()
                    .h(px(20.))
                    .px(px(6.))
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .bg(t.ink(0.06))
                    .text_size(px(11.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child(format!("{filled} of {total} filled")),
            )
        });

    let sentence = match &current {
        Some(template) => {
            let space = measure(" ", window);
            let mut seen: Vec<String> = Vec::new();
            let mut groups: Vec<AnyElement> = Vec::new();
            for atom in atoms(&template.parts()) {
                let mut group = div()
                    .min_h(px(INPUT_LINE_HEIGHT))
                    .flex()
                    .flex_row()
                    .items_center();
                for piece in atom {
                    group = match piece {
                        Piece::Word(word) => group.child(div().child(word)),
                        Piece::Blank(var) => {
                            let value = values
                                .iter()
                                .find(|(v, _)| *v == var)
                                .map(|(_, value)| value.clone())
                                .unwrap_or_default();
                            let first = !seen.contains(&var);
                            seen.push(var.clone());
                            match inputs.get(&var).filter(|_| first) {
                                Some(input) => group.child(blank(
                                    &format!("{key}-blank-{}-{var}", template.id),
                                    &var,
                                    &value,
                                    input,
                                    window,
                                    cx,
                                    t,
                                )),
                                None => group.child(echo(&var, &value, t)),
                            }
                        }
                    };
                }
                groups.push(group.into_any_element());
            }
            motion::rise_in(
                SharedString::from(format!("{key}-sentence-{}", template.id)),
                TRANSCRIPT_IN,
                TRANSCRIPT_IN_RISE,
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_x(px(space))
                    .text_color(t.text)
                    .children(groups),
            )
            .into_any_element()
        }
        None => div()
            .text_color(t.placeholder)
            .child("Pick a saved prompt to fill in")
            .into_any_element(),
    };

    let body = div()
        .px(px(16.))
        .pt(px(16.))
        .flex()
        .flex_col()
        .child(header)
        .child(
            div()
                .mt(px(6.))
                .text_size(px(INPUT_TEXT_SIZE))
                .line_height(px(INPUT_LINE_HEIGHT))
                .child(sentence),
        );

    div()
        .id(SharedString::from(format!("{key}-pill")))
        .w_full()
        .flex()
        .flex_col()
        .rounded(px(radius::COMPOSER))
        .border_1()
        .border_color(t.border)
        .bg(t.surface_bg())
        .shadow(pink::shadow_lg())
        .child(body)
        .child(footer(
            key,
            state,
            open,
            complete,
            current.is_some(),
            note,
            t,
        ))
        .into_any_element()
}

/// One editable blank: a text field as wide as its text, painted as one of
/// the composer's inline chips (radius 5, 2 px inside the line box). Empty
/// it shows the variable's name in `placeholder` on a neutral fill, which
/// deepens one tier while the caret is in it; filled it takes the accent
/// chip's colors. Both changes ease over `HOVER_FADE`.
fn blank(
    key: &str,
    var: &str,
    value: &SharedString,
    input: &Entity<TextInput>,
    window: &mut Window,
    cx: &App,
    t: &Pink,
) -> impl IntoElement {
    let focused = input.read(cx).is_focused(window);
    let filled = !value.trim().is_empty();
    let shown = if value.is_empty() {
        var
    } else {
        value.as_ref()
    };
    let width = measure(shown, window) + 2. * CHIP_PAD_X + layout::CARET_WIDTH;
    let k = motion::follow(key, if filled { 1. } else { 0. }, HOVER_FADE);
    let f = motion::follow(
        &format!("{key}-focus"),
        if focused { 1. } else { 0. },
        HOVER_FADE,
    );
    let (ink, wash) = chip_palette(t);
    let empty = motion::mix(t.fill(FillTier::Tertiary), t.fill(FillTier::Primary), f);
    let target = input.clone();
    div()
        .flex_none()
        .w(px(width))
        .h(px(INPUT_LINE_HEIGHT - 2. * CHIP_INSET_Y))
        .px(px(CHIP_PAD_X))
        .flex()
        .items_center()
        .rounded(px(CHIP_RADIUS))
        .bg(motion::mix(empty, wash, k))
        .text_color(motion::mix(t.text, ink, k))
        .line_height(px(INPUT_LINE_HEIGHT - 2. * CHIP_INSET_Y))
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            TextInput::focus(&target, window, cx)
        })
        .child(input.clone())
}

/// A repeated blank: the same chip, showing what the first one holds.
fn echo(var: &str, value: &SharedString, t: &Pink) -> impl IntoElement {
    let filled = !value.trim().is_empty();
    let (ink, wash) = chip_palette(t);
    div()
        .flex_none()
        .h(px(INPUT_LINE_HEIGHT - 2. * CHIP_INSET_Y))
        .px(px(CHIP_PAD_X))
        .flex()
        .items_center()
        .rounded(px(CHIP_RADIUS))
        .bg(if filled {
            wash
        } else {
            t.fill(FillTier::Tertiary)
        })
        .text_color(if filled { ink } else { t.placeholder })
        .child(if filled {
            value.trim().to_string()
        } else {
            var.to_string()
        })
}

/// The pill's footer (Pink wizard footer: `px 16 / pb 16`, gap 12,
/// `justify_between`): the Saved prompts trigger chip (the composer's ghost
/// pill, washed while the menu is open) and a 12 `readable_muted(0.7)` note
/// on the left; Ghost Copy and the Primary on the right, both Lg.
fn footer(
    key: &str,
    state: &Entity<LibraryState>,
    open: bool,
    complete: bool,
    has_prompt: bool,
    note: Option<Note>,
    t: &Pink,
) -> impl IntoElement {
    let (toggler, copier, user) = (state.clone(), state.clone(), state.clone());
    let trigger = pink::trigger_chip(format!("{key}-saved"), true, open, t)
        .on_click(move |_, _, cx| toggler.update(cx, |s, cx| s.set_open(!s.open, cx)))
        .child(
            svg()
                .path(ICON_NOTEBOOK)
                .size(px(16.))
                .flex_none()
                .text_color(t.text_muted),
        )
        .child("Saved prompts");

    let hint: SharedString = match (note, complete) {
        (Some(Note::Copied), _) => "Copied to clipboard".into(),
        (Some(Note::Sent), _) => "Sent to chat".into(),
        (None, true) => "Enter uses the prompt".into(),
        (None, false) if has_prompt => "Tab moves to the next blank".into(),
        (None, false) => "Open the menu to pick one".into(),
    };
    let hint = motion::status_swap(
        loaders::label_id(&format!("{key}-note"), &hint),
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .text_size(px(12.))
            .text_color(t.readable_muted(0.7))
            .child(hint),
    );

    let copy = pink::button_enabled(
        format!("{key}-copy"),
        ButtonKind::Ghost,
        ControlSize::Lg,
        has_prompt,
        t,
    )
    .when(has_prompt, |el| {
        el.on_click(move |_, _, cx| copier.update(cx, |s, cx| s.copy(cx)))
    })
    .child("Copy");
    let use_prompt = pink::button_enabled(
        format!("{key}-use"),
        ButtonKind::Primary,
        ControlSize::Lg,
        complete,
        t,
    )
    .px(px(16.))
    .when(complete, |el| {
        el.on_click(move |_, window, cx| user.update(cx, |s, cx| s.use_prompt(window, cx)))
    })
    .child("Use prompt");

    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .gap(px(12.))
        .px(px(16.))
        .pt(px(12.))
        .pb(px(16.))
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(8.))
                .child(trigger)
                .child(hint),
        )
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(8.))
                .child(copy)
                .child(use_prompt),
        )
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        // A composer column with the transcript's room above it, where the
        // saved-prompts menu opens.
        div().w(px(680.)).pt(px(MENU_ROOM)).child(
            PromptLibrary::new("prompt-library", default_templates())
                .selected("follow-up")
                .open(true)
                .focus_first_blank(true)
                .on_delete(|_, _, _| {}),
        )
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
    fn blanks_stay_glued_to_their_punctuation() {
        let template = PromptTemplate::new("t", "T", "Close with {call to action}. Thanks");
        let word = |w: &str| Piece::Word(w.into());
        assert_eq!(
            atoms(&template.parts()),
            vec![
                vec![word("Close")],
                vec![word("with")],
                vec![Piece::Blank("call to action".into()), word(".")],
                vec![word("Thanks")],
            ]
        );
    }
}
