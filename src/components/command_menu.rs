//! Command Menu – one field to search and run everything.
//!
//! An original in luna's design language. App actions sit on a strip of
//! tiles you can fire with Ctrl/⌘ 1–5; everyday commands follow in groups
//! with their own shortcuts. Typing folds the strip away and filters every
//! command at once, lighting up the matched letters. Scope chips narrow the
//! list (Tab cycles them), a plate glides to the active row, the list eases
//! to its new height, and a run confirms itself in the footer.
//!
//! ```ignore
//! CommandMenu::new("palette")
//!     .quick_actions(vec![Command::new("pr", "Open a pull request").app("GitHub")])
//!     .commands(vec![Command::new("inbox", "Go to Inbox").group("Navigate")])
//!     .on_run(|command, _, _| println!("{}", command.label))
//! ```

use std::{ops::Range, rc::Rc, time::Duration};

use gpui::{
    AnimationExt, AnyView, App, Context, ElementId, Entity, Focusable, FontWeight, HighlightStyle,
    IntoElement, KeyDownEvent, ScrollHandle, SharedString, StyledText, Subscription, Task, Window,
    div, prelude::*, px, svg,
};

use crate::{
    HoverFill,
    luna::{self, ControlSize, FillTier, Luna, radius, text},
    motion::{self, EASE_OUT_QUINT, MotionSpec, ROW_IN},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Command Menu";
pub const DESCRIPTION: &str = "One field to search and run everything – app actions on numbered tiles, everyday commands in groups, scopes on Tab and matched letters lit as you type";

const ICON_SEARCH: &str = "icons/command-menu/search.svg";
const ICON_CHECK: &str = "icons/command-menu/check.svg";
const ICON_INBOX: &str = "icons/command-menu/inbox.svg";
const ICON_FOLDER: &str = "icons/command-menu/folder.svg";
const ICON_CHART: &str = "icons/command-menu/chart.svg";
const ICON_DOC: &str = "icons/command-menu/doc.svg";
const ICON_FOLDER_PLUS: &str = "icons/command-menu/folder-plus.svg";
const ICON_USER_PLUS: &str = "icons/command-menu/user-plus.svg";
const ICON_MOON: &str = "icons/command-menu/moon.svg";
const ICON_KEYBOARD: &str = "icons/command-menu/keyboard.svg";
const ICON_SLIDERS: &str = "icons/command-menu/sliders.svg";
const ICON_LOGOUT: &str = "icons/command-menu/logout.svg";
const ICON_COMPASS: &str = "icons/command-menu/compass.svg";

const WIDTH: f32 = 600.;
const ROW_H: f32 = 36.;
const HEADER_H: f32 = 28.;
const LIST_PAD: f32 = 6.;
const LIST_MAX: f32 = 216.;
const EMPTY_H: f32 = 132.;
const STRIP_H: f32 = 112.;
/// Five tiles across the menu: its width less padding and gaps.
const TILE_W: f32 = (WIDTH - 2. - 24. - 4. * 8.) / 5.;
/// The scope that shows everything.
const ALL: &str = "All";
/// The group quick actions join once you type.
const ACTIONS: &str = "Actions";

type RunHandler = Rc<dyn Fn(&Command, &mut Window, &mut App)>;

/// Shown in a command's leading slot.
#[derive(Clone, Debug)]
pub enum CommandIcon {
    /// An SVG path under `assets/`.
    Glyph(SharedString),
    /// A neutral lettered tile standing in for an app's logo.
    Monogram(SharedString),
}

#[derive(Clone)]
pub struct Command {
    pub id: SharedString,
    pub label: SharedString,
    /// Faint text after the label (an app, a count, a place).
    pub hint: Option<SharedString>,
    pub icon: CommandIcon,
    /// Keys shown on the right, space separated: `"G I"` → G, I.
    pub shortcut: Option<SharedString>,
    pub group: SharedString,
    /// Extra words the search matches.
    pub keywords: Vec<SharedString>,
    /// Footer confirmation once it ran, e.g. "Opened Inbox".
    pub done: Option<SharedString>,
    pub on_run: Option<RunHandler>,
}

impl Command {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self {
            id: id.into(),
            icon: CommandIcon::Monogram(label.chars().next().unwrap_or('?').to_string().into()),
            label,
            hint: None,
            shortcut: None,
            group: "Commands".into(),
            keywords: Vec::new(),
            done: None,
            on_run: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = CommandIcon::Glyph(path.into());
        self
    }

    /// The app a quick action belongs to: its name becomes the hint and its
    /// initial the monogram tile.
    pub fn app(mut self, name: impl Into<SharedString>) -> Self {
        let name: SharedString = name.into();
        self.icon = CommandIcon::Monogram(name.chars().next().unwrap_or('?').to_string().into());
        self.keywords.push(name.clone());
        self.hint = Some(name);
        self
    }

    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = group.into();
        self
    }

    pub fn keywords(mut self, words: impl IntoIterator<Item = &'static str>) -> Self {
        self.keywords
            .extend(words.into_iter().map(SharedString::from));
        self
    }

    pub fn done(mut self, text: impl Into<SharedString>) -> Self {
        self.done = Some(text.into());
        self
    }

    pub fn on_run(mut self, handler: impl Fn(&Command, &mut Window, &mut App) + 'static) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }

    fn done_text(&self) -> SharedString {
        self.done
            .clone()
            .unwrap_or_else(|| format!("Ran “{}”", self.label).into())
    }
}

/// Five app actions for the numbered strip.
pub fn default_quick_actions() -> Vec<Command> {
    vec![
        Command::new("pull-request", "Open a PR")
            .app("GitHub")
            .done("Pull request draft opened in GitHub"),
        Command::new("meeting", "Start a call")
            .app("Zoom")
            .keywords(["meeting", "video"])
            .done("Meeting room ready in Zoom"),
        Command::new("ticket", "File a ticket")
            .app("Jira")
            .keywords(["bug", "issue"])
            .done("New ticket started in Jira"),
        Command::new("share-file", "Share a file")
            .app("Dropbox")
            .keywords(["upload", "send"])
            .done("Share sheet opened in Dropbox"),
        Command::new("post", "Post an update")
            .app("Slack")
            .keywords(["message", "announce", "channel"])
            .done("Composer opened in Slack"),
    ]
}

/// Navigate, Create and Preferences.
pub fn default_commands() -> Vec<Command> {
    vec![
        Command::new("inbox", "Go to Inbox")
            .icon(ICON_INBOX)
            .hint("12 unread")
            .shortcut("G I")
            .group("Navigate")
            .done("Opened Inbox"),
        Command::new("projects", "Go to Projects")
            .icon(ICON_FOLDER)
            .shortcut("G P")
            .group("Navigate")
            .done("Opened Projects"),
        Command::new("reports", "Go to Reports")
            .icon(ICON_CHART)
            .hint("Updated hourly")
            .shortcut("G R")
            .group("Navigate")
            .keywords(["analytics", "metrics"])
            .done("Opened Reports"),
        Command::new("explore", "Browse templates")
            .icon(ICON_COMPASS)
            .group("Navigate")
            .done("Opened the template gallery"),
        Command::new("new-doc", "New document")
            .icon(ICON_DOC)
            .shortcut("N")
            .group("Create")
            .keywords(["page", "write"])
            .done("Created “Untitled document”"),
        Command::new("new-project", "New project")
            .icon(ICON_FOLDER_PLUS)
            .shortcut("⇧ N")
            .group("Create")
            .done("Created “Untitled project”"),
        Command::new("invite", "Invite a teammate")
            .icon(ICON_USER_PLUS)
            .hint("2 seats left")
            .group("Create")
            .keywords(["member", "people"])
            .done("Invite link copied"),
        Command::new("theme", "Switch theme")
            .icon(ICON_MOON)
            .hint("Light · Dark · System")
            .shortcut("⇧ T")
            .group("Preferences")
            .keywords(["dark", "light", "appearance"])
            .done("Theme switched"),
        Command::new("shortcuts", "Keyboard shortcuts")
            .icon(ICON_KEYBOARD)
            .shortcut("?")
            .group("Preferences")
            .done("Showing keyboard shortcuts"),
        Command::new("settings", "Workspace settings")
            .icon(ICON_SLIDERS)
            .group("Preferences")
            .keywords(["preferences", "billing"])
            .done("Opened workspace settings"),
        Command::new("logout", "Log out")
            .icon(ICON_LOGOUT)
            .group("Preferences")
            .keywords(["sign out"])
            .done("Signed out"),
    ]
}

/// The platform's primary modifier, as printed on the keycap.
fn primary_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl"
    }
}

/// A command placed in the visible list, with its matched letters.
#[derive(Clone)]
struct Hit {
    command: Command,
    /// Its slot on the quick strip (for the Ctrl/⌘ digit).
    quick: Option<usize>,
    group: SharedString,
    highlights: Vec<Range<usize>>,
}

/// Case-insensitive match of `query` in `label`: a substring first, then the
/// letters in order. Returns a score (higher is better) and the byte ranges to light.
fn match_label(label: &str, query: &str) -> Option<(i32, Vec<Range<usize>>)> {
    let query: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if query.is_empty() {
        return Some((0, Vec::new()));
    }
    let chars: Vec<(usize, char)> = label.char_indices().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|(_, c)| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let end_of = |i: usize| chars.get(i).map_or(label.len(), |(b, _)| *b);

    // Contiguous run, preferring word starts.
    let mut best: Option<(i32, usize)> = None;
    for start in 0..lower.len() {
        if lower.len() - start < query.len() {
            break;
        }
        if lower[start..start + query.len()] == query[..] {
            let word_start = start == 0 || lower[start - 1] == ' ';
            let score = 200 - start as i32 + if word_start { 50 } else { 0 };
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, start));
            }
        }
    }
    if let Some((score, start)) = best {
        return Some((score, vec![chars[start].0..end_of(start + query.len())]));
    }

    // Letters in order, skipping spaces in the query.
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut at = 0;
    let mut gaps = 0;
    for q in query.iter().filter(|c| !c.is_whitespace()) {
        let found = (at..lower.len()).find(|&i| lower[i] == *q)?;
        if found > at {
            gaps += 1;
        }
        let range = chars[found].0..end_of(found + 1);
        match ranges.last_mut() {
            Some(last) if last.end == range.start => last.end = range.end,
            _ => ranges.push(range),
        }
        at = found + 1;
    }
    Some((100 - gaps * 8, ranges))
}

fn search(quick: &[Command], commands: &[Command], query: &str, scope: &str) -> Vec<Hit> {
    let query = query.trim();
    let in_scope = |group: &str| scope == ALL || scope == group;
    let mut hits: Vec<(i32, usize, Hit)> = Vec::new();
    let quick_hits = quick
        .iter()
        .enumerate()
        .map(|(ix, c)| (Some(ix), ACTIONS.into(), c));
    let other_hits = commands.iter().map(|c| (None, c.group.clone(), c));
    for (order, (slot, group, command)) in quick_hits.chain(other_hits).enumerate() {
        if !in_scope(&group) {
            continue;
        }
        // With an empty query in "All", the actions live on the strip instead.
        if query.is_empty() && slot.is_some() && scope == ALL {
            continue;
        }
        let found = match_label(&command.label, query).or_else(|| {
            let q = query.to_lowercase();
            let extra = command.keywords.iter().chain(command.hint.iter());
            extra
                .into_iter()
                .any(|w| w.to_lowercase().contains(&q))
                .then_some((10, Vec::new()))
        });
        if let Some((score, highlights)) = found {
            hits.push((
                score,
                order,
                Hit {
                    command: command.clone(),
                    quick: slot,
                    group,
                    highlights,
                },
            ));
        }
    }
    if !query.is_empty() {
        // Best group first (by its best hit), then best hits within each group.
        let mut group_best: Vec<(SharedString, i32, usize)> = Vec::new();
        for (score, order, hit) in &hits {
            match group_best.iter_mut().find(|(g, _, _)| *g == hit.group) {
                Some(entry) => entry.1 = entry.1.max(*score),
                None => group_best.push((hit.group.clone(), *score, *order)),
            }
        }
        let rank = |group: &SharedString| {
            group_best
                .iter()
                .find(|(g, _, _)| g == group)
                .map_or((0, 0), |(_, s, o)| (*s, *o))
        };
        hits.sort_by(|a, b| {
            let (ga, gb) = (rank(&a.2.group), rank(&b.2.group));
            gb.0.cmp(&ga.0)
                .then(ga.1.cmp(&gb.1))
                .then(b.0.cmp(&a.0))
                .then(a.1.cmp(&b.1))
        });
    }
    hits.into_iter().map(|(_, _, hit)| hit).collect()
}

/// Groups in first-seen order, as `(group, hits)`.
fn grouped(hits: &[Hit]) -> Vec<(SharedString, Vec<(usize, &Hit)>)> {
    let mut groups: Vec<(SharedString, Vec<(usize, &Hit)>)> = Vec::new();
    for (ix, hit) in hits.iter().enumerate() {
        match groups.iter_mut().find(|(g, _)| *g == hit.group) {
            Some((_, rows)) => rows.push((ix, hit)),
            None => groups.push((hit.group.clone(), vec![(ix, hit)])),
        }
    }
    groups
}

struct Ran {
    id: SharedString,
    text: SharedString,
    count: u32,
}

struct MenuState {
    input: Entity<TextInput>,
    _subscription: Subscription,
    query: SharedString,
    scope: usize,
    active: usize,
    scroll: ScrollHandle,
    ran: Option<Ran>,
    runs: u32,
    _ran_timer: Option<Task<()>>,
    // Refreshed every render, so input events can act on the current props.
    quick: Rc<Vec<Command>>,
    commands: Rc<Vec<Command>>,
    scopes: Rc<Vec<SharedString>>,
    on_run: Option<RunHandler>,
    on_dismiss: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl MenuState {
    fn hits(&self) -> Vec<Hit> {
        let scope = self.scopes.get(self.scope).map_or(ALL, |s| s.as_ref());
        search(&self.quick, &self.commands, &self.query, scope)
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                self.query = text.clone();
                self.set_active(0);
            }
            InputEvent::MoveDown | InputEvent::MoveUp => {
                let len = self.hits().len();
                if len > 0 {
                    let next = if *event == InputEvent::MoveDown {
                        (self.active + 1) % len
                    } else {
                        (self.active + len - 1) % len
                    };
                    self.set_active(next);
                }
            }
            InputEvent::Tab => {
                self.scope = (self.scope + 1) % self.scopes.len().max(1);
                self.set_active(0);
            }
            InputEvent::Submit(_) => {
                if let Some(hit) = self.hits().get(self.active).cloned() {
                    self.run(hit.command, window, cx);
                }
            }
            InputEvent::Escape => {
                if !self.query.is_empty() {
                    self.input.update(cx, |input, cx| input.clear(cx));
                    self.query = SharedString::default();
                    self.set_active(0);
                } else if let Some(f) = self.on_dismiss.clone() {
                    window.defer(cx, move |window, cx| f(window, cx));
                }
            }
            _ => return,
        }
        cx.notify();
    }

    fn set_active(&mut self, ix: usize) {
        self.active = ix;
        if ix == 0 {
            // Show the first group's header too.
            self.scroll.set_offset(gpui::point(px(0.), px(0.)));
            return;
        }
        // Child 0 of the list is the plate; headers and rows follow in order.
        let hits = self.hits();
        let mut child = 1;
        for (_, rows) in grouped(&hits) {
            child += 1;
            for (row, _) in rows {
                if row == ix {
                    self.scroll.scroll_to_item(child);
                    return;
                }
                child += 1;
            }
        }
    }

    fn run(&mut self, command: Command, window: &mut Window, cx: &mut Context<Self>) {
        self.runs += 1;
        self.ran = Some(Ran {
            id: command.id.clone(),
            text: command.done_text(),
            count: self.runs,
        });
        let runs = self.runs;
        self._ran_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(2200))
                .await;
            this.update(cx, |s, cx| {
                if s.runs == runs {
                    s.ran = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        let on_run = self.on_run.clone();
        window.defer(cx, move |window, cx| {
            if let Some(f) = command.on_run.as_ref() {
                f(&command, window, cx);
            }
            if let Some(f) = on_run.as_ref() {
                f(&command, window, cx);
            }
        });
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct CommandMenu {
    id: ElementId,
    quick: Vec<Command>,
    commands: Vec<Command>,
    placeholder: SharedString,
    autofocus: bool,
    on_run: Option<RunHandler>,
    on_dismiss: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
}

impl CommandMenu {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            quick: Vec::new(),
            commands: Vec::new(),
            placeholder: "Search commands, pages and apps…".into(),
            autofocus: true,
            on_run: None,
            on_dismiss: None,
        }
    }

    /// Up to five app actions for the numbered strip (Ctrl/⌘ 1–5).
    pub fn quick_actions(mut self, actions: Vec<Command>) -> Self {
        self.quick = actions;
        self
    }

    /// Everyday commands, shown by group in the order given.
    pub fn commands(mut self, commands: Vec<Command>) -> Self {
        self.commands = commands;
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// Focuses the search field when the menu first renders (on by default).
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Called for every command that runs, after the command's own handler.
    pub fn on_run(mut self, handler: impl Fn(&Command, &mut Window, &mut App) + 'static) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }

    /// Escape on an empty field.
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CommandMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Luna::get(cx);
        let key = self.id.to_string();
        let placeholder = self.placeholder.clone();
        let autofocus = self.autofocus;

        let state = window.use_keyed_state(
            ElementId::NamedChild(Box::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| {
                let input = cx.new(|cx| TextInput::new(window, cx).placeholder(placeholder));
                let subscription =
                    cx.subscribe_in(&input, window, |s: &mut MenuState, _, event, window, cx| {
                        s.on_input(event, window, cx)
                    });
                if autofocus {
                    let handle = input.read(cx).focus_handle(cx);
                    window.defer(cx, move |window, _| window.focus(&handle));
                }
                MenuState {
                    input,
                    _subscription: subscription,
                    query: SharedString::default(),
                    scope: 0,
                    active: 0,
                    scroll: ScrollHandle::new(),
                    ran: None,
                    runs: 0,
                    _ran_timer: None,
                    quick: Rc::new(Vec::new()),
                    commands: Rc::new(Vec::new()),
                    scopes: Rc::new(Vec::new()),
                    on_run: None,
                    on_dismiss: None,
                }
            },
        );

        // Scopes: All, Actions (when there are quick actions), then each group.
        let mut scopes: Vec<SharedString> = vec![ALL.into()];
        if !self.quick.is_empty() {
            scopes.push(ACTIONS.into());
        }
        for command in &self.commands {
            if !scopes.contains(&command.group) {
                scopes.push(command.group.clone());
            }
        }
        let quick = Rc::new(self.quick);
        let commands = Rc::new(self.commands);
        let scopes = Rc::new(scopes);
        let input = state.update(cx, |s, _| {
            s.quick = quick.clone();
            s.commands = commands.clone();
            s.scopes = scopes.clone();
            s.on_run = self.on_run.clone();
            s.on_dismiss = self.on_dismiss.clone();
            s.scope = s.scope.min(scopes.len() - 1);
            s.input.clone()
        });
        input.update(cx, |input, _| {
            input.set_colors(Some(t.text_faint), Some(t.accent), Some(t.selection))
        });

        let s = state.read(cx);
        let query = s.query.clone();
        let scope = s.scope;
        let hits = s.hits();
        let active = s.active.min(hits.len().saturating_sub(1));
        let scroll = s.scroll.clone();
        let ran = s
            .ran
            .as_ref()
            .map(|r| (r.id.clone(), r.text.clone(), r.count));
        let scope_name = scopes[scope].clone();

        let show_strip = query.trim().is_empty() && scope_name.as_ref() == ALL && !quick.is_empty();
        let strip_t = motion::follow(
            &format!("{key}-strip"),
            if show_strip { 1. } else { 0. },
            MotionSpec::new(320, EASE_OUT_QUINT),
        );

        // Ctrl/⌘ + digit runs a quick action from anywhere in the menu.
        let on_key = {
            let state = state.clone();
            let quick = quick.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let keystroke = &event.keystroke;
                if !keystroke.modifiers.secondary() {
                    return;
                }
                let Some(slot) = keystroke
                    .key
                    .parse::<usize>()
                    .ok()
                    .and_then(|d| d.checked_sub(1))
                else {
                    return;
                };
                if let Some(command) = quick.get(slot).cloned() {
                    state.update(cx, |s, cx| s.run(command, window, cx));
                    cx.stop_propagation();
                }
            }
        };

        let search_row = div()
            .h(px(52.))
            .px(px(16.))
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                svg()
                    .path(ICON_SEARCH)
                    .size(px(18.))
                    .flex_none()
                    .text_color(t.text_faint),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(15.))
                    .line_height(px(22.))
                    .text_color(t.text)
                    .child(input.clone()),
            )
            .when(!query.is_empty(), |el| {
                let clearer = state.clone();
                el.child(
                    div()
                        .id("clear")
                        .flex_none()
                        .h(px(22.))
                        .px(px(7.))
                        .flex()
                        .items_center()
                        .rounded(px(radius::SM))
                        .cursor_pointer()
                        .text_size(px(text::CAPTION))
                        .hover_fill_fg(
                            format!("{key}-clear"),
                            t.fill(FillTier::Quaternary),
                            t.fill(FillTier::Secondary),
                            t.text_muted,
                            t.text,
                        )
                        .on_click(move |_, _, cx| {
                            clearer.update(cx, |s, cx| {
                                s.input.update(cx, |input, cx| input.clear(cx));
                                s.query = SharedString::default();
                                s.set_active(0);
                                cx.notify();
                            })
                        })
                        .child("Clear"),
                )
            });

        let scope_row = div()
            .px(px(12.))
            .pb(px(10.))
            .flex()
            .items_center()
            .gap(px(4.))
            .children(scopes.iter().enumerate().map(|(ix, name)| {
                let picker = state.clone();
                luna::chip(
                    SharedString::from(format!("{key}-scope-{name}")),
                    ix == scope,
                    ControlSize::Sm,
                    &t,
                )
                .on_click(move |_, _, cx| {
                    picker.update(cx, |s, cx| {
                        s.scope = ix;
                        s.set_active(0);
                        cx.notify();
                    })
                })
                .child(name.clone())
            }))
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(text::CAPTION))
                    .text_color(t.text_faint)
                    .child(luna::kbd("Tab", &t))
                    .child("next scope"),
            );

        let strip = div()
            .h(px(STRIP_H * strip_t))
            .overflow_hidden()
            .opacity(strip_t)
            .child(quick_strip(&key, &quick, &state, ran.as_ref(), &t));

        let body = results(
            &key,
            &hits,
            active,
            &query,
            &state,
            &scroll,
            ran.as_ref(),
            &t,
        );

        let footer = footer(&key, ran.as_ref(), hits.len(), &query, &t);

        motion::keep_alive(window);
        div()
            .id(self.id)
            .w(px(WIDTH))
            .font_family(luna::SANS)
            .text_color(t.text)
            .bg(t.surface_overlay)
            .border_1()
            .border_color(t.border)
            .rounded(px(radius::XL))
            .shadow(t.menu_shadow())
            .overflow_hidden()
            .on_key_down(on_key)
            .child(search_row)
            .child(scope_row)
            .child(luna::divider(&t))
            .child(strip)
            .child(body)
            .child(footer)
            .with_animation("command-menu-in", ROW_IN.animation(), |el, k| {
                el.opacity(k).mt(px(8. * (1. - k)))
            })
    }
}

/// The five numbered app tiles.
fn quick_strip(
    key: &str,
    quick: &Rc<Vec<Command>>,
    state: &Entity<MenuState>,
    ran: Option<&(SharedString, SharedString, u32)>,
    t: &Luna,
) -> impl IntoElement {
    let tiles = quick.iter().take(9).enumerate().map(|(slot, command)| {
        let hover_key = format!("{key}-tile-{}", command.id);
        let lift = motion::hover_t(&hover_key);
        let runner = state.clone();
        let run_command = command.clone();
        let just_ran = ran.is_some_and(|(id, _, _)| *id == command.id);
        div()
            .id(SharedString::from(format!("tile-{}", command.id)))
            .flex_none()
            .w(px(TILE_W))
            .h(px(78.))
            .p(px(10.))
            .flex()
            .flex_col()
            .justify_between()
            .rounded(px(radius::LG))
            .border_1()
            .border_color(motion::mix(t.border, t.border_strong, lift))
            .cursor_pointer()
            .hover_fill(
                hover_key,
                t.fill(FillTier::Quaternary),
                t.fill(FillTier::Tertiary),
            )
            .on_click(move |_, window, cx| {
                let command = run_command.clone();
                runner.update(cx, |s, cx| s.run(command, window, cx));
            })
            .child(
                div()
                    .flex()
                    .items_start()
                    .justify_between()
                    .child(if just_ran {
                        check_tile(t).into_any_element()
                    } else {
                        monogram(&command.icon, 24., t).into_any_element()
                    })
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family(luna::MONO)
                            .text_color(t.text_faint)
                            .child(chord(slot + 1)),
                    ),
            )
            .child(
                div()
                    // Explicit, so the labels truncate against the tile's inner width.
                    .w(px(TILE_W - 22.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(command.label.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(text::CAPTION))
                            .line_height(px(14.))
                            .text_color(t.text_faint)
                            .child(command.hint.clone().unwrap_or_default()),
                    ),
            )
    });
    div()
        .px(px(12.))
        .pt(px(10.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(section_label("Quick actions", t).px(px(2.)).h(px(18.)))
        .child(div().flex().gap(px(8.)).children(tiles))
}

fn check_tile(t: &Luna) -> impl IntoElement {
    div()
        .size(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .bg(t.success.opacity(if t.dark { 0.18 } else { 0.12 }))
        .child(svg().path(ICON_CHECK).size(px(14.)).text_color(t.success))
        .with_animation(
            "tile-check",
            MotionSpec::new(260, EASE_OUT_QUINT).animation(),
            |el, k| el.opacity(k),
        )
}

/// `⌘1` on macOS, `Ctrl+1` elsewhere.
fn chord(digit: usize) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{digit}")
    } else {
        format!("Ctrl+{digit}")
    }
}

/// A command's leading glyph, or a neutral lettered tile for an app.
fn monogram(icon: &CommandIcon, size: f32, t: &Luna) -> gpui::Div {
    match icon {
        CommandIcon::Glyph(path) => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(
                svg()
                    .path(path.clone())
                    .size(px(16.))
                    .text_color(t.text_muted),
            ),
        CommandIcon::Monogram(letter) => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(if size >= 24. { radius::SM } else { radius::XS }))
            .bg(if t.dark {
                t.surface_raised
            } else {
                t.surface_overlay
            })
            .border_1()
            .border_color(t.border_strong)
            .text_size(px(if size >= 24. { 12. } else { 10. }))
            .font_weight(FontWeight::BOLD)
            .text_color(t.text)
            .child(letter.clone()),
    }
}

fn section_label(label: impl Into<SharedString>, t: &Luna) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .text_size(px(text::CAPTION))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text_faint)
        .child(label.into())
}

#[allow(clippy::too_many_arguments)]
fn results(
    key: &str,
    hits: &[Hit],
    active: usize,
    query: &SharedString,
    state: &Entity<MenuState>,
    scroll: &ScrollHandle,
    ran: Option<&(SharedString, SharedString, u32)>,
    t: &Luna,
) -> impl IntoElement {
    let groups = grouped(hits);

    // Row tops, for the plate, and the content height, for the glide.
    let mut y = LIST_PAD;
    let mut active_top = LIST_PAD + HEADER_H;
    for (_, rows) in &groups {
        y += HEADER_H;
        for (ix, _) in rows {
            if *ix == active {
                active_top = y;
            }
            y += ROW_H;
        }
    }
    let content = if hits.is_empty() {
        EMPTY_H
    } else {
        y + LIST_PAD
    };
    let height = motion::follow(
        &format!("{key}-height"),
        content.min(LIST_MAX),
        MotionSpec::new(300, EASE_OUT_QUINT),
    );
    let plate_top = motion::follow(
        &format!("{key}-plate"),
        active_top,
        MotionSpec::new(180, EASE_OUT_QUINT),
    );

    let mut list = div()
        .id("results")
        .relative()
        .h(px(height))
        .px(px(LIST_PAD))
        .py(px(LIST_PAD))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(
            div()
                .absolute()
                .left(px(LIST_PAD))
                .right(px(LIST_PAD))
                .top(px(plate_top))
                .h(px(ROW_H))
                .rounded(px(radius::MD))
                .bg(t.fill(FillTier::Tertiary))
                .when(hits.is_empty(), |el| el.opacity(0.)),
        );

    if hits.is_empty() {
        return list.child(empty_state(query, t));
    }

    for (group, rows) in groups {
        list = list.child(
            section_label(group.clone(), t)
                .h(px(HEADER_H))
                .px(px(10.))
                .pt(px(4.)),
        );
        for (ix, hit) in rows {
            list = list.child(row(key, ix, hit, ix == active, state, ran, t));
        }
    }
    list
}

fn row(
    key: &str,
    ix: usize,
    hit: &Hit,
    active: bool,
    state: &Entity<MenuState>,
    ran: Option<&(SharedString, SharedString, u32)>,
    t: &Luna,
) -> impl IntoElement {
    let command = &hit.command;
    let lit = motion::follow(
        &format!("{key}-lit-{}", command.id),
        if active { 1. } else { 0. },
        MotionSpec::new(160, EASE_OUT_QUINT),
    );
    let pointer = state.clone();
    let runner = state.clone();
    let run_command = command.clone();
    let just_ran = ran.filter(|(id, _, _)| *id == command.id);

    let label = StyledText::new(command.label.clone()).with_highlights(hit.highlights.iter().map(
        |range| {
            (
                range.clone(),
                HighlightStyle {
                    color: Some(t.accent),
                    font_weight: Some(FontWeight::SEMIBOLD),
                    ..Default::default()
                },
            )
        },
    ));

    let keys: Vec<SharedString> = match (&hit.quick, &command.shortcut) {
        (Some(slot), _) => vec![primary_key().into(), (slot + 1).to_string().into()],
        (None, Some(shortcut)) => shortcut
            .split_whitespace()
            .map(|k| SharedString::from(k.to_string()))
            .collect(),
        _ => Vec::new(),
    };

    let trailing = match just_ran {
        Some((_, _, count)) => div()
            .flex()
            .items_center()
            .gap(px(4.))
            .text_size(px(text::CAPTION))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.success)
            .child(svg().path(ICON_CHECK).size(px(14.)).text_color(t.success))
            .child("Done")
            .with_animation(
                ElementId::NamedInteger("done".into(), u64::from(*count)),
                MotionSpec::new(280, EASE_OUT_QUINT).animation(),
                |el, k| el.opacity(k).ml(px(6. * (1. - k))),
            )
            .into_any_element(),
        None => div()
            .flex()
            .items_center()
            .gap(px(3.))
            .children(keys.into_iter().map(|k| luna::kbd(k, t)))
            .into_any_element(),
    };

    div()
        .id(SharedString::from(format!("row-{}", command.id)))
        .relative()
        .h(px(ROW_H))
        .px(px(10.))
        .flex()
        .items_center()
        .gap(px(10.))
        .cursor_pointer()
        .on_mouse_move(move |_, _, cx| {
            pointer.update(cx, |s, cx| {
                if s.active != ix {
                    s.active = ix;
                    cx.notify();
                }
            })
        })
        .on_click(move |_, window, cx| {
            let command = run_command.clone();
            runner.update(cx, |s, cx| {
                s.active = ix;
                s.run(command, window, cx)
            });
        })
        .child(
            div()
                .w(px(20.))
                .flex_none()
                .flex()
                .justify_center()
                .child(match &command.icon {
                    CommandIcon::Glyph(path) => svg()
                        .path(path.clone())
                        .size(px(16.))
                        .text_color(motion::mix(t.text_faint, t.text, lit))
                        .into_any_element(),
                    icon => monogram(icon, 20., t).into_any_element(),
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(px(8.))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(
                    div()
                        .flex_none()
                        .text_size(px(text::BODY))
                        .text_color(t.text)
                        .child(label),
                )
                .when_some(command.hint.clone(), |el, hint| {
                    el.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(hint),
                    )
                }),
        )
        .child(trailing)
        .with_animation(
            SharedString::from(format!("row-in-{}", command.id)),
            MotionSpec::new(260, EASE_OUT_QUINT).animation(),
            |el, k| el.opacity(k).top(px(3. * (1. - k))),
        )
}

fn empty_state(query: &SharedString, t: &Luna) -> impl IntoElement {
    div()
        .h(px(EMPTY_H - 2. * LIST_PAD))
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .child(
            div()
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::LG))
                .bg(t.fill(FillTier::Tertiary))
                .child(
                    svg()
                        .path(ICON_SEARCH)
                        .size(px(16.))
                        .text_color(t.text_muted),
                ),
        )
        .child(
            div()
                .text_size(px(text::BODY))
                .font_weight(FontWeight::MEDIUM)
                .child(format!("Nothing matches “{}”", query.trim())),
        )
        .child(
            div()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child("Try a page, an app or an action, or press Tab to change scope"),
        )
}

fn footer(
    key: &str,
    ran: Option<&(SharedString, SharedString, u32)>,
    count: usize,
    query: &SharedString,
    t: &Luna,
) -> impl IntoElement {
    let hint = |keys: &[&'static str], label: &'static str| {
        div()
            .flex()
            .items_center()
            .gap(px(5.))
            .child(
                div()
                    .flex()
                    .gap(px(2.))
                    .children(keys.iter().map(|k| luna::kbd(*k, t))),
            )
            .child(label)
    };
    let status = match ran {
        Some((_, text, count)) => div()
            .flex()
            .items_center()
            .gap(px(6.))
            .text_color(t.text)
            .child(
                div()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(t.success.opacity(if t.dark { 0.2 } else { 0.14 }))
                    .child(svg().path(ICON_CHECK).size(px(11.)).text_color(t.success)),
            )
            .child(text.clone())
            .with_animation(
                ElementId::NamedInteger(format!("{key}-ran").into(), u64::from(*count)),
                MotionSpec::new(320, EASE_OUT_QUINT).animation(),
                |el, k| el.relative().opacity(k).top(px(6. * (1. - k))),
            )
            .into_any_element(),
        None => div()
            .flex()
            .items_center()
            .gap(px(14.))
            .child(hint(&["↑", "↓"], "move"))
            .child(hint(&["↵"], "run"))
            .with_animation(
                SharedString::from(format!("{key}-hints")),
                MotionSpec::new(320, EASE_OUT_QUINT).animation(),
                |el, k| el.relative().opacity(k).top(px(6. * (1. - k))),
            )
            .into_any_element(),
    };
    let right = if query.trim().is_empty() {
        hint(&["Esc"], "close").into_any_element()
    } else if count == 1 {
        div().child("1 result").into_any_element()
    } else {
        div().child(format!("{count} results")).into_any_element()
    };
    div()
        .h(px(38.))
        .px(px(14.))
        .flex()
        .items_center()
        .justify_between()
        .overflow_hidden()
        .border_t_1()
        .border_color(t.border)
        .bg(t.fill(FillTier::Quaternary))
        .text_size(px(text::FOOTNOTE))
        .text_color(t.text_faint)
        .child(status)
        .child(right)
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        CommandMenu::new("command-menu")
            .quick_actions(default_quick_actions())
            .commands(default_commands())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_runs_then_letters() {
        let (_, ranges) = match_label("Go to Reports", "re").unwrap();
        assert_eq!(ranges, vec![6..8]);
        let (_, ranges) = match_label("Go to Reports", "gtr").unwrap();
        assert_eq!(ranges, vec![0..1, 3..4, 6..7]);
        assert!(match_label("Go to Reports", "xyz").is_none());
    }

    #[test]
    fn word_starts_score_higher() {
        let (start, _) = match_label("Projects", "pro").unwrap();
        let (inside, _) = match_label("Improve", "pro").unwrap();
        assert!(start > inside);
    }

    #[test]
    fn quick_actions_join_the_list_once_searching() {
        let quick = default_quick_actions();
        let commands = default_commands();
        let idle = search(&quick, &commands, "", ALL);
        assert!(idle.iter().all(|hit| hit.quick.is_none()));
        let typed = search(&quick, &commands, "ticket", ALL);
        assert_eq!(typed.first().map(|hit| hit.quick), Some(Some(2)));
        let scoped = search(&quick, &commands, "", ACTIONS);
        assert_eq!(scoped.len(), quick.len());
    }
}
