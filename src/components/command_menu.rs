//! Command Menu – a ⌘K palette: one search field over everything you can run.
//!
//! Pink: `shell/command_palette.rs` `Shell::render_command_palette`. A card
//! 560 wide (radius 16, `border`, the popover plate, `shadow_lg`) that
//! arrives as one object, a 160ms fade with a 6pt rise. On top, a 44px search
//! row: the 16px palette glyph, a 14px field and a ⌘K hint over a 0.06
//! hairline. Below it the results: ungrouped actions first, then each group
//! under an 11px heading with a rule above it. A footer of key hints closes
//! the card. Rows are Pink's `menu_row` at radius 8 with a 16px glyph in
//! `text_muted`. The keyboard's row wears the selected wash and the pointer a
//! lighter one. Typed words light up in `code_text` on rounded `code_wash`
//! plates. The list stops at the window height minus 180 (100–360) and fades
//! whichever edge has more to scroll.
//!
//! Quick actions are an app's commands, named beside their label like Pink's
//! extension commands. They come first and run with ⌘1–⌘9 from anywhere in
//! the menu. A command with a `detail` gets Pink's two-line result row. Enter
//! runs the highlighted row. Esc, ⌘K or a click outside the card calls
//! `on_dismiss`. Pink closes the palette after a run, so hosts usually hide it
//! from `on_run` too. The host lays the scrim (Pink: `scrim(0.35)`) and keeps
//! the card's top fixed (Pink: 20% down the window) so results grow downward.
//!
//! ```ignore
//! CommandMenu::new("palette")
//!     .quick_actions(vec![Command::new("pr", "Open a pull request").icon(PR_ICON).app("GitHub")])
//!     .commands(vec![Command::new("inbox", "Go to Inbox").icon(INBOX_ICON).group("Navigate")])
//!     .on_run(|command, _, _| println!("{}", command.label))
//!     .on_dismiss(|_, cx| close_palette(cx))
//! ```

use std::{ops::Range, rc::Rc, sync::Arc};

use gpui::{
    AnyElement, AnyView, App, BorderStyle, Bounds, Context, ElementId, Entity, FocusHandle,
    Focusable, HighlightStyle, Hsla, IntoElement, KeyDownEvent, Pixels, ScrollHandle, SharedString,
    StyledText, Subscription, TextLayout, Window, canvas, div, fill, linear_color_stop,
    linear_gradient, point, prelude::*, px, quad, size, svg, transparent_black,
};

use crate::{
    motion,
    pink::{self, ButtonKind, ControlSize, Pink, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Command Menu";
pub const DESCRIPTION: &str = "A ⌘K palette for running anything: one search field over app actions and grouped commands, typed words lit as you type, shortcuts on every row and a footer of key hints";

const ICON_SEARCH: &str = "icons/command-menu/palette-search.svg";
/// Pink: `contributions::icon_for`'s fallback for a command without a glyph.
const ICON_DEFAULT: &str = "icons/command-menu/widget.svg";
const ICON_PULL_REQUEST: &str = "icons/command-menu/git-pull-request.svg";
const ICON_CALENDAR: &str = "icons/command-menu/calendar.svg";
const ICON_TICKET: &str = "icons/command-menu/ticket.svg";
const ICON_CLOUD: &str = "icons/command-menu/cloud.svg";
const ICON_CHAT: &str = "icons/command-menu/chat-round-line.svg";
const ICON_INBOX: &str = "icons/command-menu/inbox-in.svg";
const ICON_PROJECTS: &str = "icons/command-menu/folder-with-files.svg";
const ICON_CHART: &str = "icons/command-menu/chart.svg";
const ICON_NEW_DOC: &str = "icons/command-menu/pen-new-square.svg";
const ICON_FOLDER: &str = "icons/command-menu/folder.svg";
const ICON_ADD: &str = "icons/command-menu/add-circle.svg";
const ICON_SUN: &str = "icons/command-menu/sun.svg";
const ICON_MOON: &str = "icons/command-menu/moon.svg";
const ICON_KEYBOARD: &str = "icons/command-menu/keyboard.svg";
const ICON_SETTINGS: &str = "icons/command-menu/settings-minimalistic.svg";
const ICON_LOGOUT: &str = "icons/command-menu/logout-2.svg";
const ICON_DOCUMENT: &str = "icons/command-menu/document.svg";

/// Pink: the card's `560 × ui_scale`, kept 16px clear of each window edge.
const WIDTH: f32 = 560.;
const WINDOW_CLEARANCE: f32 = 32.;
/// Pink: the search row's `min_h`.
const FIELD_MIN_H: f32 = 44.;
/// Pink: `ComposerInput`'s default metrics, which the palette field keeps.
const FIELD_TEXT: f32 = 14.;
const FIELD_LINE_HEIGHT: f32 = 22.75;
/// Pink: the result list is the window height less 180, clamped to 100–360.
const LIST_VIEWPORT_INSET: f32 = 180.;
const LIST_MIN: f32 = 100.;
const LIST_MAX: f32 = 360.;
/// Pink: `RESULTS_FADE_BAND`.
const RESULTS_FADE_BAND: f32 = 18.;
/// Pink: the action row's `min_h`, and the two-line result row's.
const ROW_MIN_H: f32 = 32.;
const RESULT_ROW_MIN_H: f32 = 44.;
/// Quick actions with a ⌘ digit.
const QUICK_SLOTS: usize = 9;

type RunHandler = Rc<dyn Fn(&Command, &mut Window, &mut App)>;
type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// One runnable row.
#[derive(Clone)]
pub struct Command {
    pub id: SharedString,
    pub label: SharedString,
    /// A second line under the label (Pink's two-line result row), e.g.
    /// "Document · Product · Edited 2h ago".
    pub detail: Option<SharedString>,
    /// Faint text at the row's end: the app a command belongs to, a count.
    pub hint: Option<SharedString>,
    /// An SVG path under `assets/`, drawn 16px in `text_muted`.
    pub icon: SharedString,
    /// The keys on the right, as printed: `⇧⌘N`.
    pub shortcut: Option<SharedString>,
    /// The heading the row sits under; ungrouped rows lead the list.
    pub group: Option<SharedString>,
    /// Extra words the search matches.
    pub keywords: Vec<SharedString>,
    pub on_run: Option<RunHandler>,
}

impl Command {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            hint: None,
            icon: ICON_DEFAULT.into(),
            shortcut: None,
            group: None,
            keywords: Vec::new(),
            on_run: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = path.into();
        self
    }

    /// The app a command belongs to: named at the row's end (Pink
    /// `render_contributed_command_row`) and matched by the search.
    pub fn app(mut self, name: impl Into<SharedString>) -> Self {
        let name: SharedString = name.into();
        self.keywords.push(name.clone());
        self.hint = Some(name);
        self
    }

    pub fn shortcut(mut self, keys: impl Into<SharedString>) -> Self {
        self.shortcut = Some(keys.into());
        self
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn keywords(mut self, words: impl IntoIterator<Item = &'static str>) -> Self {
        self.keywords
            .extend(words.into_iter().map(SharedString::from));
        self
    }

    pub fn on_run(mut self, handler: impl Fn(&Command, &mut Window, &mut App) + 'static) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }

    /// Everything the search reads: label, detail, hint and keywords.
    fn search_text(&self) -> String {
        let mut text = self.label.to_string();
        for word in self
            .detail
            .iter()
            .chain(self.hint.iter())
            .chain(self.keywords.iter())
        {
            text.push(' ');
            text.push_str(word);
        }
        text
    }
}

/// Five app actions for the top of the list.
pub fn default_quick_actions() -> Vec<Command> {
    vec![
        Command::new("pull-request", "Open a pull request")
            .icon(ICON_PULL_REQUEST)
            .app("GitHub")
            .keywords(["pr", "review"]),
        Command::new("meeting", "Schedule a meeting")
            .icon(ICON_CALENDAR)
            .app("Calendar")
            .keywords(["call", "event", "invite"]),
        Command::new("ticket", "File a ticket")
            .icon(ICON_TICKET)
            .app("Jira")
            .keywords(["bug", "issue"]),
        Command::new("share-file", "Share a file")
            .icon(ICON_CLOUD)
            .app("Dropbox")
            .keywords(["upload", "send"]),
        Command::new("post", "Post an update")
            .icon(ICON_CHAT)
            .app("Slack")
            .keywords(["message", "announce", "channel"]),
    ]
}

/// Navigate, Create, Preferences and Recent documents. `dark` is the current
/// appearance: the theme command offers the other one (Pink: `ToggleTheme`
/// targets the opposite resolved appearance).
pub fn default_commands(dark: bool) -> Vec<Command> {
    let (theme_label, theme_icon) = if dark {
        ("Switch to light theme", ICON_SUN)
    } else {
        ("Switch to dark theme", ICON_MOON)
    };
    vec![
        Command::new("inbox", "Go to Inbox")
            .icon(ICON_INBOX)
            .hint("12 unread")
            .group("Navigate"),
        Command::new("projects", "Go to Projects")
            .icon(ICON_PROJECTS)
            .shortcut(combo(true, "P"))
            .group("Navigate"),
        Command::new("reports", "Go to Reports")
            .icon(ICON_CHART)
            .hint("Updated hourly")
            .group("Navigate")
            .keywords(["analytics", "metrics"]),
        Command::new("new-doc", "New document")
            .icon(ICON_NEW_DOC)
            .shortcut(combo(false, "N"))
            .group("Create")
            .keywords(["page", "write"]),
        Command::new("new-project", "New project")
            .icon(ICON_FOLDER)
            .shortcut(combo(true, "N"))
            .group("Create"),
        Command::new("invite", "Invite a teammate")
            .icon(ICON_ADD)
            .hint("2 seats left")
            .group("Create")
            .keywords(["member", "people"]),
        Command::new("theme", theme_label)
            .icon(theme_icon)
            .group("Preferences")
            .keywords(["appearance", "mode"]),
        Command::new("shortcuts", "Keyboard shortcuts")
            .icon(ICON_KEYBOARD)
            .shortcut(combo(false, "/"))
            .group("Preferences"),
        Command::new("settings", "Workspace settings")
            .icon(ICON_SETTINGS)
            .shortcut(combo(false, ","))
            .group("Preferences")
            .keywords(["preferences", "billing"]),
        Command::new("logout", "Log out")
            .icon(ICON_LOGOUT)
            .group("Preferences")
            .keywords(["sign out"]),
        Command::new("q3-notes", "Q3 planning notes")
            .icon(ICON_DOCUMENT)
            .detail("Product document, edited 2h ago")
            .group("Recent documents"),
        Command::new("launch", "Launch checklist")
            .icon(ICON_DOCUMENT)
            .detail("Marketing document, edited yesterday")
            .group("Recent documents"),
        Command::new("hiring", "Hiring plan 2027")
            .icon(ICON_DOCUMENT)
            .detail("People document, edited Oct 12")
            .group("Recent documents"),
    ]
}

/// A primary-modifier chord as printed on a key hint: Pink's `badge_combo`
/// (`⌘K`, `⇧⌘N`) on macOS, `Ctrl+K` / `Ctrl+Shift+N` elsewhere.
fn combo(shift: bool, key: &str) -> SharedString {
    if cfg!(target_os = "macos") {
        format!("{}⌘{key}", if shift { "⇧" } else { "" }).into()
    } else {
        format!("Ctrl+{}{key}", if shift { "Shift+" } else { "" }).into()
    }
}

/// Pink: `matches_query`: every word of the (lowercased) query appears in
/// the text.
fn matches_query(query: &str, text: &str) -> bool {
    let text = text.to_lowercase();
    query.split_whitespace().all(|word| text.contains(word))
}

/// Pink: `popover::search_match_ranges`: every occurrence of each query word
/// in `text`, case-insensitively, as merged byte ranges of the original text.
fn match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    // Fold char by char so each folded byte maps back to its source char.
    let mut folded = String::with_capacity(text.len());
    let mut original = Vec::with_capacity(text.len());
    for (start, ch) in text.char_indices() {
        let range = start..start + ch.len_utf8();
        for lower in ch.to_lowercase() {
            folded.push(lower);
            original.extend(std::iter::repeat_n(range.clone(), lower.len_utf8()));
        }
    }
    let mut ranges = Vec::new();
    for word in query.to_lowercase().split_whitespace() {
        for (start, _) in folded.match_indices(word) {
            if let (Some(first), Some(last)) =
                (original.get(start), original.get(start + word.len() - 1))
            {
                ranges.push(first.start..last.end);
            }
        }
    }
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

/// A command placed in the visible list.
#[derive(Clone)]
struct Entry {
    command: Command,
    /// Its quick-action slot (for the ⌘ digit).
    quick: Option<usize>,
}

/// Pink: `command_entries`: actions in their given order, quick actions
/// first, keeping the ones whose text holds every typed word.
fn visible(quick: &[Command], commands: &[Command], query: &str) -> Vec<Entry> {
    let query = query.trim().to_lowercase();
    quick
        .iter()
        .enumerate()
        .map(|(ix, command)| (Some(ix).filter(|ix| *ix < QUICK_SLOTS), command))
        .chain(commands.iter().map(|command| (None, command)))
        .filter(|(_, command)| matches_query(&query, &command.search_text()))
        .map(|(quick, command)| Entry {
            command: command.clone(),
            quick,
        })
        .collect()
}

struct MenuState {
    input: Entity<TextInput>,
    _subscription: Subscription,
    query: SharedString,
    /// The row the arrows bound (Pink `PaletteSelection::Bound`); `None` is
    /// the default, the first row.
    selected: Option<SharedString>,
    scroll: ScrollHandle,
    // Refreshed every render, so input events act on the current props.
    quick: Rc<Vec<Command>>,
    commands: Rc<Vec<Command>>,
    on_run: Option<RunHandler>,
    on_dismiss: Option<DismissHandler>,
}

impl MenuState {
    fn entries(&self) -> Vec<Entry> {
        visible(&self.quick, &self.commands, &self.query)
    }

    /// The highlighted row: the bound one while it is listed, else the first.
    fn active(&self, entries: &[Entry]) -> Option<usize> {
        if entries.is_empty() {
            return None;
        }
        let bound = self
            .selected
            .as_ref()
            .and_then(|id| entries.iter().position(|entry| entry.command.id == *id));
        Some(bound.unwrap_or(0))
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            // Pink: an edit resets the selection and scrolls back to the top.
            InputEvent::Change(text) => {
                self.query = text.clone();
                self.selected = None;
                self.scroll.set_offset(point(px(0.), px(0.)));
            }
            InputEvent::MoveDown | InputEvent::MoveUp => {
                let entries = self.entries();
                if entries.is_empty() {
                    return;
                }
                let down = *event == InputEvent::MoveDown;
                let len = entries.len();
                let next = match self.active(&entries) {
                    Some(active) if down => (active + 1) % len,
                    Some(active) => (active + len - 1) % len,
                    None if down => 0,
                    None => len - 1,
                };
                self.selected = Some(entries[next].command.id.clone());
                // Each entry is one child of the list, heading included.
                self.scroll.scroll_to_item(next);
            }
            InputEvent::Submit(_) => {
                let entries = self.entries();
                if let Some(entry) = self.active(&entries).and_then(|ix| entries.get(ix)) {
                    self.run(entry.command.clone(), window, cx);
                }
            }
            InputEvent::Escape => self.dismiss(window, cx),
            _ => return,
        }
        cx.notify();
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(dismiss) = self.on_dismiss.clone() {
            window.defer(cx, move |window, cx| dismiss(window, cx));
        }
    }

    fn run(&mut self, command: Command, window: &mut Window, cx: &mut App) {
        let on_run = self.on_run.clone();
        window.defer(cx, move |window, cx| {
            if let Some(run) = command.on_run.as_ref() {
                run(&command, window, cx);
            }
            if let Some(run) = on_run.as_ref() {
                run(&command, window, cx);
            }
        });
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
    on_dismiss: Option<DismissHandler>,
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

    /// App actions for the top of the list; the first nine run with ⌘1–⌘9.
    pub fn quick_actions(mut self, actions: Vec<Command>) -> Self {
        self.quick = actions;
        self
    }

    /// Everyday commands, in the order given, under their groups' headings.
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

    /// Esc, ⌘K or a click outside the card (Pink closes the palette).
    pub fn on_dismiss(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CommandMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let placeholder = self.placeholder.clone();
        let autofocus = self.autofocus;

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            move |window, cx| {
                let input = cx.new(|cx| TextInput::new(window, cx).placeholder(placeholder));
                let subscription =
                    cx.subscribe_in(&input, window, |s: &mut MenuState, _, event, window, cx| {
                        s.on_input(event, window, cx)
                    });
                if autofocus {
                    let handle = input.read(cx).focus_handle(cx);
                    window.defer(cx, move |window, cx| window.focus(&handle, cx));
                }
                MenuState {
                    input,
                    _subscription: subscription,
                    query: SharedString::default(),
                    selected: None,
                    scroll: ScrollHandle::new(),
                    quick: Rc::new(Vec::new()),
                    commands: Rc::new(Vec::new()),
                    on_run: None,
                    on_dismiss: None,
                }
            },
        );

        let quick = Rc::new(self.quick);
        let commands = Rc::new(self.commands);
        let input = state.update(cx, |s, _| {
            s.quick = quick.clone();
            s.commands = commands.clone();
            s.on_run = self.on_run.clone();
            s.on_dismiss = self.on_dismiss.clone();
            s.input.clone()
        });
        // Pink: the palette field's placeholder is `text_faint`.
        input.update(cx, |input, _| {
            input.set_colors(Some(t.text_faint), Some(t.caret), Some(t.selection))
        });

        let s = state.read(cx);
        let query = s.query.clone();
        let entries = s.entries();
        let active = s.active(&entries);
        let scroll = s.scroll.clone();

        // ⌘ + digit runs a quick action, ⌘K closes (Pink: the shortcut
        // toggles the palette).
        let on_key = {
            let state = state.clone();
            let quick = quick.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let keystroke = &event.keystroke;
                if !keystroke.modifiers.secondary() {
                    return;
                }
                if keystroke.key == "k" {
                    state.update(cx, |s, cx| s.dismiss(window, cx));
                    cx.stop_propagation();
                    return;
                }
                let slot = keystroke
                    .key
                    .parse::<usize>()
                    .ok()
                    .and_then(|digit| digit.checked_sub(1))
                    .filter(|slot| *slot < QUICK_SLOTS);
                if let Some(command) = slot.and_then(|slot| quick.get(slot)).cloned() {
                    state.update(cx, |s, cx| s.run(command, window, cx));
                    cx.stop_propagation();
                }
            }
        };
        let on_outside = {
            let state = state.clone();
            move |_: &gpui::MouseDownEvent, window: &mut Window, cx: &mut App| {
                state.update(cx, |s, cx| s.dismiss(window, cx))
            }
        };

        // Pink: the search row.
        let search_row = div()
            .min_h(px(FIELD_MIN_H))
            .flex_none()
            .px(px(16.))
            .py(px(8.))
            .flex()
            .items_center()
            .gap(px(10.))
            .border_b_1()
            .border_color(t.hairline(0.06))
            .child(palette_search_icon(&t))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(FIELD_TEXT))
                    .line_height(px(FIELD_LINE_HEIGHT))
                    .child(input),
            )
            .child(pink::kbd_hint(combo(false, "K"), &t));

        let mut rows = Vec::with_capacity(entries.len());
        let mut previous_group: Option<&SharedString> = None;
        for (ix, entry) in entries.iter().enumerate() {
            // Pink: end spacing belongs to the content, so it scrolls out of
            // the fade instead of leaving a gutter beside the chrome.
            let mut row = div()
                .id(("command-result", ix))
                .flex_none()
                .when(ix == 0, |row| row.pt(px(8.)))
                .when(ix + 1 == entries.len(), |row| row.pb(px(8.)));
            if let Some(group) = entry.command.group.as_ref()
                && previous_group != Some(group)
            {
                if ix > 0 {
                    row = row.child(group_rule(&t));
                }
                row = row.child(group_heading(group.clone(), &t));
                previous_group = Some(group);
            }
            rows.push(row.child(div().px(px(8.)).child(command_row(
                &key,
                entry,
                active == Some(ix),
                &query,
                &state,
                &t,
            ))));
        }

        let viewport = window.viewport_size();
        let list_h = (f32::from(viewport.height) - LIST_VIEWPORT_INSET).clamp(LIST_MIN, LIST_MAX);
        let body = div()
            .id("command-results")
            .min_h_0()
            .max_h(px(list_h))
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .flex()
            .flex_col()
            .children(rows)
            .when(entries.is_empty(), |el| el.child(no_results(&t)));
        let body = div()
            .relative()
            .flex()
            .flex_col()
            .min_h_0()
            .child(body)
            .child(results_fade(scroll, t.surface_bg()));

        // Pink: the footer of key hints.
        let footer = div()
            .flex_none()
            .px(px(16.))
            .py(px(7.))
            .border_t_1()
            .border_color(t.hairline(0.06))
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .child(key_hint("↑ ↓", "Navigate", &t))
            .child(key_hint("↵", "Select", &t))
            .child(key_hint("Esc", "Close", &t));

        let width = px(WIDTH).min(viewport.width - px(WINDOW_CLEARANCE));
        let card = div()
            .id(self.id)
            .w(width)
            .flex()
            .flex_col()
            .rounded(px(radius::PALETTE))
            .border_1()
            .border_color(t.border)
            .shadow(pink::shadow_lg())
            .bg(t.surface_bg())
            .font_family(pink::SANS)
            .text_color(t.text)
            .on_key_down(on_key)
            .on_mouse_down_out(on_outside)
            .child(search_row)
            .child(body)
            .child(footer);

        motion::keep_alive(window);
        // Pink: arrive as one object, a short fade and a 6pt rise.
        motion::rise_in(
            SharedString::from(format!("{key}-in")),
            motion::PALETTE_IN,
            motion::PALETTE_IN_RISE,
            card,
        )
    }
}

/// Pink: `popover::palette_search_icon`, the search glyph in the same 16px
/// slot as the row glyphs.
fn palette_search_icon(t: &Pink) -> gpui::Div {
    div()
        .size(px(16.))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path(ICON_SEARCH)
                .size(px(16.))
                .text_color(t.text_muted),
        )
}

/// Pink: `spaces::sidebar_separator` at `my 7`, the rule above a group.
fn group_rule(t: &Pink) -> gpui::Div {
    div()
        .h(px(1.))
        .w_full()
        .my(px(7.))
        .bg(t.border.opacity(0.6))
}

/// Pink: a result group's heading, 11 medium `text_faint`, sentence case.
fn group_heading(label: SharedString, t: &Pink) -> gpui::Div {
    div()
        .px(px(10.))
        .pb(px(4.))
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(t.text_faint)
        .child(label)
}

/// One row. Pink: the action row of `render_command_palette` (a 16px glyph,
/// the highlighted label and a `kbd_hint`), `render_contributed_command_row`
/// for the faint name at its end, and `render_command_history_row` for a
/// command with a detail line.
fn command_row(
    key: &str,
    entry: &Entry,
    active: bool,
    query: &str,
    state: &Entity<MenuState>,
    t: &Pink,
) -> impl IntoElement {
    let command = &entry.command;
    let keys = match (entry.quick, &command.shortcut) {
        (Some(slot), _) => Some(combo(false, &(slot + 1).to_string())),
        (None, shortcut) => shortcut.clone(),
    };
    let runner = state.clone();
    let run = command.clone();
    let label = search_highlight(command.label.clone(), query, t);
    let two_line = command.detail.is_some();

    pink::menu_row(format!("{key}-row-{}", command.id), active, t)
        .rounded(px(radius::PALETTE_ITEM))
        .min_h(px(if two_line {
            RESULT_ROW_MIN_H
        } else {
            ROW_MIN_H
        }))
        .py(px(if two_line { 5. } else { 4. }))
        .on_click(move |_, window, cx| {
            let command = run.clone();
            runner.update(cx, |s, cx| s.run(command, window, cx));
        })
        .child(
            svg()
                .path(command.icon.clone())
                .size(px(16.))
                .flex_none()
                .text_color(t.text_muted),
        )
        .child(match &command.detail {
            None => div().flex_1().min_w_0().child(label),
            Some(detail) => div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .truncate()
                        .text_size(px(12.))
                        .text_color(t.text)
                        .child(label),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(t.text_faint)
                        .child(detail.clone()),
                ),
        })
        .when_some(command.hint.clone(), |row, hint| {
            row.child(
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(t.text_faint)
                    .child(hint),
            )
        })
        .when_some(keys, |row, keys| row.child(pink::kbd_hint(keys, t)))
}

/// Pink: the palette's "No results" line and its muted hint.
fn no_results(t: &Pink) -> impl IntoElement {
    div()
        .w_full()
        .py(px(24.))
        .px(px(16.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.))
        .text_size(px(13.))
        .child("No results")
        .child(
            div()
                .text_color(t.text_muted)
                .child("Try a command, a page or an app."),
        )
}

/// Pink: `command_key_hint`, a `kbd_hint` and its 11px verb.
fn key_hint(keys: &'static str, label: &'static str, t: &Pink) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(px(5.))
        .child(pink::kbd_hint(keys, t))
        .child(
            div()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child(label),
        )
}

/// Pink: `popover::search_highlight`. Matched words are recolored
/// `code_text` over rounded (r3) `code_wash` plates painted beneath the
/// shaped glyphs; the row keeps its font and spacing.
fn search_highlight(text: SharedString, query: &str, t: &Pink) -> AnyElement {
    if query.trim().is_empty() {
        return text.into_any_element();
    }
    let ranges = match_ranges(&text, query);
    if ranges.is_empty() {
        return text.into_any_element();
    }
    let styled = StyledText::new(text).with_highlights(ranges.iter().cloned().map(|range| {
        (
            range,
            HighlightStyle {
                color: Some(t.code_text),
                ..Default::default()
            },
        )
    }));
    let layout = styled.layout().clone();
    let wash = t.code_wash;
    let underlay = canvas(
        |_, _, _| (),
        move |_, _, window, _| {
            for range in &ranges {
                for mut bounds in range_rects(&layout, range, 1.0, 1.5) {
                    // Glyphs sit below the line box's optical center.
                    bounds.origin.y += px(0.5);
                    window.paint_quad(quad(
                        bounds,
                        px(3.),
                        wash,
                        px(0.),
                        transparent_black(),
                        BorderStyle::default(),
                    ));
                }
            }
        },
    )
    .absolute()
    .size_full();
    div()
        .relative()
        .child(underlay)
        .child(styled)
        .into_any_element()
}

/// Pink: `markdown::render::range_rects`: one rectangle per visual row of
/// `range`, widened by `pad_x` and inset by `inset_y` from the line box.
fn range_rects(
    layout: &TextLayout,
    range: &Range<usize>,
    pad_x: f32,
    inset_y: f32,
) -> Vec<Bounds<Pixels>> {
    let bounds = layout.bounds();
    let line_height = layout.line_height();
    let position = |ix: usize| layout.position_for_index(ix);
    let mut rects = Vec::new();
    let mut cur = range.start;
    let mut guard = 0;
    while cur < range.end && guard < 256 {
        guard += 1;
        let Some(mut start) = position(cur) else {
            break;
        };
        // A soft-wrap boundary reports the end of the row before it.
        if let Some(after) = position(cur + 1)
            && after.y > start.y
        {
            start = point(bounds.left(), after.y);
        }
        let (seg_end, next) = match position(range.end) {
            Some(end) if end.y == start.y => (range.end, range.end),
            _ => {
                let (mut lo, mut hi) = (cur, range.end);
                while hi - lo > 1 {
                    let mid = lo + (hi - lo) / 2;
                    match position(mid) {
                        Some(at) if at.y == start.y => lo = mid,
                        _ => hi = mid,
                    }
                }
                (lo, lo)
            }
        };
        if let Some(end) = position(seg_end)
            && end.x > start.x
        {
            rects.push(Bounds::new(
                point(start.x - px(pad_x), start.y + px(inset_y)),
                size(
                    end.x - start.x + px(2. * pad_x),
                    line_height - px(2. * inset_y),
                ),
            ));
        }
        if next <= cur {
            break;
        }
        cur = next;
    }
    rects
}

/// Pink: `edge_fade::edge_faded(RESULTS_FADE_BAND, true, true, ..)
/// .fade_overflow_y(..)`. GPUI here has no per-pixel edge fade, so the
/// opaque plate is painted over each edge that has more to scroll, from
/// solid at the edge to clear, gated on the handle at paint time.
fn results_fade(scroll: ScrollHandle, plate: Hsla) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let scrolled = -f32::from(scroll.offset().y);
            let max = f32::from(scroll.max_offset().y);
            let band = px(RESULTS_FADE_BAND).min(bounds.size.height / 2.);
            let clear = plate.opacity(0.);
            if scrolled > 0.5 {
                window.paint_quad(fill(
                    Bounds::new(bounds.origin, size(bounds.size.width, band)),
                    linear_gradient(
                        180.,
                        linear_color_stop(plate, 0.),
                        linear_color_stop(clear, 1.),
                    ),
                ));
            }
            if scrolled < max - 0.5 {
                window.paint_quad(fill(
                    Bounds::new(
                        point(bounds.left(), bounds.bottom() - band),
                        size(bounds.size.width, band),
                    ),
                    linear_gradient(
                        0.,
                        linear_color_stop(plate, 0.),
                        linear_color_stop(clear, 1.),
                    ),
                ));
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// The story's host: the palette while it is open, and a quiet way back once
/// a run or a dismissal closes it (Pink closes the palette on both).
struct Stage {
    open: bool,
    ran: Option<SharedString>,
    focus: FocusHandle,
}

/// The stage keeps the card's top fixed, as Pink anchors it, so the results
/// grow and shrink below the field.
const STAGE_W: f32 = 600.;
const STAGE_H: f32 = 480.;

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let t = Pink::get(cx);
        let stage = window.use_keyed_state("command-menu-stage", cx, |_, cx| Stage {
            open: true,
            ran: None,
            focus: cx.focus_handle(),
        });
        let (open, ran, focus) = {
            let s = stage.read(cx);
            (s.open, s.ran.clone(), s.focus.clone())
        };
        let set_open = {
            let stage = stage.clone();
            let focus = focus.clone();
            Rc::new(
                move |open: bool, ran: Option<SharedString>, window: &mut Window, cx: &mut App| {
                    if !open {
                        // Keep ⌘K working once the palette's field is gone.
                        window.focus(&focus, cx);
                    }
                    stage.update(cx, |s, cx| {
                        s.open = open;
                        if ran.is_some() {
                            s.ran = ran;
                        }
                        cx.notify();
                    })
                },
            )
        };

        let content = if open {
            let on_run = set_open.clone();
            let on_dismiss = set_open.clone();
            CommandMenu::new("command-menu")
                .quick_actions(default_quick_actions())
                .commands(default_commands(t.dark))
                .on_run(move |command, window, cx| {
                    on_run(false, Some(command.label.clone()), window, cx)
                })
                .on_dismiss(move |window, cx| on_dismiss(false, None, window, cx))
                .into_any_element()
        } else {
            let reopen = set_open.clone();
            let title: SharedString = match &ran {
                Some(label) => format!("Ran “{label}”").into(),
                None => "Command palette closed".into(),
            };
            div()
                .pt(px(120.))
                .child(
                    pink::surface_state(
                        svg()
                            .path(ICON_SEARCH)
                            .size(px(28.))
                            .text_color(t.text_faint),
                        title,
                        Some(format!("Press {} to search again.", combo(false, "K")).into()),
                        &t,
                    )
                    .child(
                        div().mt(px(6.)).child(
                            pink::button(
                                "command-menu-reopen",
                                ButtonKind::Secondary,
                                ControlSize::Md,
                                &t,
                            )
                            .on_click(move |_, window, cx| reopen(true, None, window, cx))
                            .child("Open command palette"),
                        ),
                    ),
                )
                .into_any_element()
        };

        let reopen = set_open.clone();
        div()
            .w(px(STAGE_W))
            .h(px(STAGE_H))
            .flex()
            .flex_col()
            .items_center()
            .font_family(pink::SANS)
            .text_color(t.text)
            .track_focus(&focus)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let keystroke = &event.keystroke;
                if !open && keystroke.modifiers.secondary() && keystroke.key == "k" {
                    reopen(true, None, window, cx);
                    cx.stop_propagation();
                }
            })
            .child(content)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_word_must_appear() {
        assert!(matches_query("go rep", "Go to Reports"));
        assert!(matches_query("", "Go to Reports"));
        assert!(!matches_query("gtr", "Go to Reports"));
    }

    #[test]
    fn highlights_every_case_insensitive_word() {
        assert_eq!(
            match_ranges("New chat, new project", "NEW"),
            vec![0..3, 10..13]
        );
        assert_eq!(
            match_ranges("authentication", "auth authentication"),
            vec![0..14]
        );
        assert!(match_ranges("New chat", "  ").is_empty());
        assert!(match_ranges("New chat", "settings").is_empty());
        assert_eq!(match_ranges("İstanbul café", "i CAFÉ"), vec![0..2, 10..15]);
    }

    #[test]
    fn quick_actions_lead_and_filter_with_the_rest() {
        let quick = default_quick_actions();
        let commands = default_commands(true);
        let idle = visible(&quick, &commands, "");
        assert_eq!(idle.len(), quick.len() + commands.len());
        assert_eq!(idle[0].quick, Some(0));
        let typed = visible(&quick, &commands, "ticket");
        assert_eq!(typed.len(), 1);
        assert_eq!(typed[0].quick, Some(2));
        // Keywords and the app name match too.
        assert_eq!(visible(&quick, &commands, "slack")[0].command.id, "post");
        assert!(visible(&quick, &commands, "zzz").is_empty());
    }
}
