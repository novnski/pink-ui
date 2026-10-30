//! Comments Panel – a Pink component.
//!
//! The review comments on one document, in a side pane. Every comment is
//! Pink's inline comment card (Pink: comment_ui.rs `render_comment_card`,
//! drawn by the `pink-comment-card` package, [`comment_card`]): a
//! flat band with a 3px bar, the line it is pinned to in the header and the
//! note under it; here the note also names its author. A comment that starts
//! with a slash command (`/rewrite`, `/translate`, …) carries Pink's tool
//! chip card to run it as an edit: a glyph spinner and a shimmering verb
//! while it runs, then the change as diff rows with an Undo. Resolving folds
//! a comment shut into the Resolved side of Pink's segmented switch. New
//! comments are written in Pink's draft card at the bottom, where `/` opens
//! Pink's slash-command completion above it.
//!
//! ```ignore
//! CommentsPanel::new("comments", comments)
//!     .document("pricing.md")
//!     .on_submit(|text, _, cx| { /* post it */ })
//!     .on_resolve(|id, _, cx| { /* resolve or reopen it */ })
//!     .on_run(|id, _, cx| { /* apply the edit */ })
//! ```

use std::{cell::Cell, collections::HashMap, rc::Rc, sync::Arc};
use web_time::{Duration, Instant};

use gpui::{
    AnyElement, AnyView, App, Context, ElementId, Entity, FontWeight, Hsla, IntoElement, Pixels,
    Role, ScrollHandle, SharedString, Stateful, Subscription, Task, TextRun, Window, canvas, div,
    font, prelude::*, px, svg,
};

use crate::{
    components::comment_card,
    motion,
    pink::{self, ControlSize, Pink, Segment, icon_size, layout, loaders, radius, space, text},
    primitives::{InputEvent, TextInput},
    story,
};

// The pane's pieces (toolbar, heading and count, byline, note and replies
// line) are the `pink-comments-panel` package, compiled here in place; PINK
// pins the same source.
mod package {
    include!("../../components/comments-panel/src/lib.rs");
}
pub use package::{
    BYLINE_HEIGHT, ENTRY_GAP, LINE_HEIGHT, PaneStyle, SWITCH_WIDTH, TITLE_INSET, TOOLBAR_HEIGHT,
    byline, content, count, entry, entry_height, heading, list, name, note, pane, replies,
    replies_label, time, toolbar,
};

pub const NAME: &str = "Comments Panel";
pub const DESCRIPTION: &str = "A document's review comments in a side pane: flat note cards pinned to a line, slash-command notes that run as an edit and show the change as a diff with an Undo, resolving that folds a note away into Resolved, and a draft card where typing a slash opens command completion";

/// Pink `icons::COMMAND`, the slash completion's row glyph.
const ICON_COMMAND: &str = "icons/comments-panel/command.svg";
/// Pink `icons::FILE_PEN`, the edit tool's glyph.
const ICON_EDIT: &str = "icons/comments-panel/file-pen.svg";
/// Pink `icons::CHECK` and `icons::UNDO_LEFT`: resolve and reopen.
const ICON_RESOLVE: &str = "icons/comments-panel/check.svg";
const ICON_REOPEN: &str = "icons/comments-panel/undo-left.svg";

/// Pink changes.rs `ACCENT_BAR_WIDTH` and comments.rs card geometry, from
/// the `pink-comment-card` package: the bar, the vertical padding (split top
/// and bottom), a body line and the slack under the body.
const ACCENT_BAR_WIDTH: f32 = comment_card::ACCENT_BAR_WIDTH;
const CARD_PAD_V: f32 = comment_card::PAD_V;
const CARD_LINE_HEIGHT: f32 = comment_card::LINE_HEIGHT;
const CARD_GAP: f32 = comment_card::GAP;
/// Pink markdown/render.rs inline code plate.
const INLINE_CODE_RADIUS: f32 = 4.5;
const INLINE_CODE_PAD_X: f32 = 2.0;
const INLINE_CODE_INSET_Y: f32 = 2.0;
/// Pink transcript.rs chip card: 30 tall with its 28px header row and an
/// 18px glyph tile.
const CHIP_CARD_HEIGHT: f32 = 30.0;
const CHIP_HEADER_HEIGHT: f32 = CHIP_CARD_HEIGHT - 2.0;
const CHIP_TILE: f32 = 18.0;
/// Pink changes.rs diff rows: 21px lines, a 16px marker column and 12px of
/// code padding after it.
const DIFF_LINE_HEIGHT: f32 = 21.0;
const MARKER_WIDTH: f32 = 16.0;
const CODE_PAD_LEFT: f32 = 12.0;
/// Pink composer.rs `render_slash_popup_body`: the card's height budget and
/// its scrolling list; full_width_menu_above's 6px gap.
const POPUP_MAX_HEIGHT: f32 = 320.0;
const POPUP_LIST_MAX_HEIGHT: f32 = 312.0;
const POPUP_GAP: f32 = 6.0;

type IdHandler = Rc<dyn Fn(SharedString, &mut Window, &mut App)>;

/// Where an edit command is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditStatus {
    Ready,
    Running,
    Applied,
}

/// The edit a slash-command comment proposes.
#[derive(Clone)]
pub struct EditCommand {
    pub before: SharedString,
    pub after: SharedString,
    pub status: EditStatus,
}

impl EditCommand {
    pub fn new(before: impl Into<SharedString>, after: impl Into<SharedString>) -> Self {
        Self {
            before: before.into(),
            after: after.into(),
            status: EditStatus::Ready,
        }
    }

    pub fn status(mut self, status: EditStatus) -> Self {
        self.status = status;
        self
    }
}

#[derive(Clone)]
pub struct Comment {
    pub id: SharedString,
    pub author: SharedString,
    pub time: SharedString,
    /// The document line the comment is pinned to.
    pub line: Option<u32>,
    /// The note; a leading `/command` is drawn as a command.
    pub body: SharedString,
    pub edit: Option<EditCommand>,
    pub resolved: bool,
}

impl Comment {
    pub fn new(
        id: impl Into<SharedString>,
        author: impl Into<SharedString>,
        body: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            author: author.into(),
            time: SharedString::default(),
            line: None,
            body: body.into(),
            edit: None,
            resolved: false,
        }
    }

    /// When it was written, already formatted ("12m", "1h").
    pub fn time(mut self, time: impl Into<SharedString>) -> Self {
        self.time = time.into();
        self
    }

    /// Pin the comment to a line; its header reads `document:line`.
    pub fn line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    pub fn edit(mut self, edit: EditCommand) -> Self {
        self.edit = Some(edit);
        self
    }

    pub fn resolved(mut self, resolved: bool) -> Self {
        self.resolved = resolved;
        self
    }
}

/// A command the composer can complete.
#[derive(Clone)]
pub struct SlashCommand {
    pub name: SharedString,
    pub hint: SharedString,
}

impl SlashCommand {
    pub fn new(name: impl Into<SharedString>, hint: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            hint: hint.into(),
        }
    }
}

pub fn default_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand::new("rewrite", "Rewrite the line"),
        SlashCommand::new("shorten", "Make it shorter"),
        SlashCommand::new("translate", "Translate into another language"),
        SlashCommand::new("assign", "Hand the comment to someone"),
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab {
    Open,
    Resolved,
}

/// The completion as last shown, held through its exit fade.
#[derive(Clone)]
struct Popup {
    rows: Vec<SlashCommand>,
    highlight: usize,
}

struct State {
    input: Entity<TextInput>,
    text: SharedString,
    tab: Tab,
    highlight: usize,
    /// Escape closed the completion for the token being typed.
    dismissed: bool,
    scroll: ScrollHandle,
    seen: usize,
    /// Comments folding shut after being resolved or reopened, and the tab
    /// they are leaving.
    leaving: HashMap<SharedString, (Instant, Tab)>,
    heights: HashMap<SharedString, Rc<Cell<f32>>>,
    /// The comments listed when the tab came up; they reveal in a cascade,
    /// later arrivals on their own.
    batch: Option<(Tab, Vec<SharedString>)>,
    /// Each edit's status as last seen, and since when (`None` when it was
    /// already there at mount).
    edits: HashMap<SharedString, (EditStatus, Option<Instant>)>,
    diff_heights: HashMap<SharedString, Rc<Cell<f32>>>,
    popup: Option<Popup>,
    popup_closed: Option<Instant>,
    commands: Vec<SlashCommand>,
    on_submit: Option<IdHandler>,
    _input_events: Subscription,
}

impl State {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx
            // joiners: allow the slash below is the key to type, not a joiner
            .new(|cx| TextInput::new(window, cx).placeholder("Comment, or type / for a command…"));
        let sub = cx.subscribe_in(&input, window, |this: &mut State, _, ev, window, cx| {
            let matches = this.matching().map_or(0, |m| m.len());
            match ev {
                InputEvent::Change(text) => {
                    this.text = text.clone();
                    this.highlight = 0;
                    this.dismissed = false;
                    cx.notify();
                }
                InputEvent::MoveDown if matches > 0 => {
                    this.highlight = (this.highlight + 1) % matches;
                    cx.notify();
                }
                InputEvent::MoveUp if matches > 0 => {
                    this.highlight = (this.highlight + matches - 1) % matches;
                    cx.notify();
                }
                InputEvent::Tab | InputEvent::Submit(_) if matches > 0 => {
                    this.accept(this.highlight, cx);
                }
                InputEvent::Submit(_) => this.submit(window, cx),
                InputEvent::Escape if this.matching().is_some() => {
                    this.dismissed = true;
                    cx.notify();
                }
                InputEvent::Escape => this.clear(cx),
                _ => {}
            }
        });
        State {
            input,
            text: SharedString::default(),
            tab: Tab::Open,
            highlight: 0,
            dismissed: false,
            scroll: ScrollHandle::new(),
            seen: 0,
            leaving: HashMap::new(),
            heights: HashMap::new(),
            batch: None,
            edits: HashMap::new(),
            diff_heights: HashMap::new(),
            popup: None,
            popup_closed: None,
            commands: Vec::new(),
            on_submit: None,
            _input_events: sub,
        }
    }

    /// The commands matching a `/prefix` still being typed: `None` while no
    /// slash token is open (Pink shows the card, "No matching commands"
    /// included, for as long as the token is).
    fn matching(&self) -> Option<Vec<SlashCommand>> {
        let prefix = self.text.strip_prefix('/')?;
        if prefix.contains(char::is_whitespace) || self.dismissed {
            return None;
        }
        Some(
            self.commands
                .iter()
                .filter(|c| c.name.starts_with(prefix))
                .cloned()
                .collect(),
        )
    }

    /// Completes the token with the `ix`-th match (Pink `accept_slash`).
    fn accept(&mut self, ix: usize, cx: &mut Context<Self>) {
        let Some(name) = self
            .matching()
            .and_then(|m| m.get(ix).map(|c| c.name.clone()))
        else {
            return;
        };
        let text: SharedString = format!("/{name} ").into();
        self.input.update(cx, |i, cx| i.set_text(text.clone(), cx));
        self.text = text;
        self.highlight = 0;
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().trim().to_string();
        if text.is_empty() {
            return;
        }
        self.clear(cx);
        self.tab = Tab::Open;
        if let Some(f) = self.on_submit.clone() {
            f(text.into(), window, cx);
        }
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |i, cx| i.clear(cx));
        self.text = SharedString::default();
        self.highlight = 0;
        self.dismissed = false;
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct CommentsPanel {
    id: ElementId,
    comments: Vec<Comment>,
    document: SharedString,
    commands: Vec<SlashCommand>,
    width: f32,
    height: f32,
    on_submit: Option<IdHandler>,
    on_resolve: Option<IdHandler>,
    on_run: Option<IdHandler>,
    on_undo: Option<IdHandler>,
}

impl CommentsPanel {
    pub fn new(id: impl Into<ElementId>, comments: Vec<Comment>) -> Self {
        Self {
            id: id.into(),
            comments,
            document: "Untitled".into(),
            commands: default_commands(),
            width: 420.,
            height: 600.,
            on_submit: None,
            on_resolve: None,
            on_run: None,
            on_undo: None,
        }
    }

    /// The document's path; the cards and the draft cite it.
    pub fn document(mut self, path: impl Into<SharedString>) -> Self {
        self.document = path.into();
        self
    }

    pub fn commands(mut self, commands: Vec<SlashCommand>) -> Self {
        self.commands = commands;
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    pub fn on_submit(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_submit = Some(Rc::new(handler));
        self
    }

    /// Resolve (or reopen) the comment with this id.
    pub fn on_resolve(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resolve = Some(Rc::new(handler));
        self
    }

    /// Run the comment's edit command.
    pub fn on_run(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_run = Some(Rc::new(handler));
        self
    }

    pub fn on_undo(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_undo = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for CommentsPanel {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let reduced = motion::reduced_motion(cx);
        let key = self.id.to_string();
        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            State::new,
        );
        let now = Instant::now();
        let fold_span = motion::DISCLOSURE.total().mul_f32(motion::speed_scale());

        // ---- mirror the props into the state ------------------------------
        let count = self.comments.len();
        let commands = self.commands.clone();
        let on_submit = self.on_submit.clone();
        let edits: Vec<(SharedString, EditStatus)> = self
            .comments
            .iter()
            .filter_map(|c| c.edit.as_ref().map(|e| (c.id.clone(), e.status)))
            .collect();
        let ids: Vec<SharedString> = self.comments.iter().map(|c| c.id.clone()).collect();
        // Pink `ComposerInput` outside the message composer: a `text_faint`
        // placeholder, the accent caret
        let colors = (t.text_faint, t.caret, t.selection);
        state.update(cx, |s, cx| {
            s.commands = commands;
            s.on_submit = on_submit;
            if count > s.seen {
                if s.seen > 0 {
                    s.scroll.scroll_to_bottom();
                }
                s.seen = count;
            }
            for id in &ids {
                s.heights.entry(id.clone()).or_default();
            }
            s.leaving
                .retain(|_, (at, _)| now.saturating_duration_since(*at) < fold_span);
            let mounted = s.batch.is_none();
            for (id, status) in edits {
                match s.edits.get(&id) {
                    Some((last, _)) if *last == status => {}
                    // A run counts from when it was first seen
                    _ if status == EditStatus::Running => {
                        s.edits.insert(id, (status, Some(now)));
                    }
                    _ => {
                        s.edits.insert(id, (status, (!mounted).then_some(now)));
                    }
                }
            }
            s.input.update(cx, |i, _| {
                i.set_colors(Some(colors.0), Some(colors.1), Some(colors.2))
            });
        });

        let s = state.read(cx);
        let tab = s.tab;
        let leaving = s.leaving.clone();
        let visible: Vec<&Comment> = self
            .comments
            .iter()
            .filter(|c| {
                let here = match tab {
                    Tab::Open => !c.resolved,
                    Tab::Resolved => c.resolved,
                };
                here || leaving.get(&c.id).is_some_and(|(_, from)| *from == tab)
            })
            .collect();
        if s.batch.as_ref().is_none_or(|(at, _)| *at != tab) {
            let batch = visible.iter().map(|c| c.id.clone()).collect();
            state.update(cx, |s, _| s.batch = Some((tab, batch)));
        }

        // ---- the completion, held through its exit -----------------------
        let open_matches = state.read(cx).matching();
        let exit = state.update(cx, |s, _| match &open_matches {
            Some(rows) => {
                s.popup = Some(Popup {
                    rows: rows.clone(),
                    highlight: s.highlight.min(rows.len().saturating_sub(1)),
                });
                s.popup_closed = None;
                None
            }
            None => {
                if s.popup.is_some() && s.popup_closed.is_none() && !reduced {
                    s.popup_closed = Some(now);
                }
                match s.popup_closed {
                    Some(at) if motion::menu_exit_progress(at) < 1. => {
                        Some((motion::menu_exit_progress(at), at))
                    }
                    _ => {
                        s.popup = None;
                        s.popup_closed = None;
                        None
                    }
                }
            }
        });

        let s = state.read(cx);
        let scroll = s.scroll.clone();
        let heights = s.heights.clone();
        let input = s.input.clone();
        let batch = s.batch.clone().map(|(_, ids)| ids).unwrap_or_default();
        let edit_seen = s.edits.clone();
        let popup = s.popup.clone();
        let mut diff_heights = s.diff_heights.clone();
        let mut moving = !leaving.is_empty() || exit.is_some();

        let open_count = self.comments.iter().filter(|c| !c.resolved).count();

        // ---- toolbar: title, count and the Open / Resolved switch ---------
        // (Pink surface_chrome `toolbar`; the count is the activity section
        // heading's trailing count; the switch is Pink `button::segmented`)
        let switch_state = state.clone();
        let toolbar = toolbar()
            .child(heading("Comments", t.text).when(open_count > 0, |d| {
                d.child(
                    package::count(format!("{open_count}"), t.readable_faint(0.7))
                        .font_features(pink::tabular_figures()),
                )
            }))
            .child(pink::segmented(
                format!("{key}-tabs"),
                vec![
                    Segment::new("open", "Open"),
                    Segment::new("resolved", "Resolved"),
                ],
                usize::from(tab == Tab::Resolved),
                SWITCH_WIDTH,
                ControlSize::Md,
                &t,
                move |ix, _, cx| {
                    let which = if *ix == 0 { Tab::Open } else { Tab::Resolved };
                    switch_state.update(cx, |s, cx| {
                        if s.tab != which {
                            s.tab = which;
                            cx.notify();
                        }
                    });
                },
            ));

        // ---- the cards ------------------------------------------------------
        let space_w = space_width(window);
        let empty = visible.is_empty();
        let mut items: Vec<AnyElement> = Vec::with_capacity(visible.len());
        for c in &visible {
            let group: SharedString = format!("{key}-card-{}", c.id).into();
            let resolved = c.resolved;
            let location: SharedString = match c.line {
                Some(line) => format!("{}:{line}", self.document).into(),
                None => self.document.clone(),
            };

            // Resolve / reopen, revealed on hover like Pink's remove
            let on_resolve = self.on_resolve.clone();
            let resolve_state = state.clone();
            let cid = c.id.clone();
            let label = if resolved { "Reopen" } else { "Resolve" };
            let ring = t.focus_ring();
            let action = comment_card::remove(
                svg().path(if resolved { ICON_REOPEN } else { ICON_RESOLVE }),
                t.text_muted,
                group.clone(),
            )
            .id(ElementId::Name(format!("{key}-resolve-{}", c.id).into()))
            .role(Role::Button)
            .aria_label(label)
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s.opacity(1.), ring))
            .tooltip(pink::text_tooltip(label))
            .tooltip_show_delay(layout::TOOLTIP_DELAY)
            .on_click(move |_, window, cx| {
                let Some(f) = &on_resolve else { return };
                let from = if resolved { Tab::Resolved } else { Tab::Open };
                resolve_state.update(cx, |s, cx| {
                    s.leaving.insert(cid.clone(), (Instant::now(), from));
                    cx.notify();
                });
                f(cid.clone(), window, cx);
            });

            // Pink: comment card header – the glyph and where it's pinned
            let header = card_header(location.clone(), &t).child(action);

            // Who wrote it and when (the sidebar's 11px tabular stamp)
            let byline =
                byline()
                    .child(name(c.author.clone(), t.text))
                    .when(!c.time.is_empty(), |d| {
                        d.child(
                            time(c.time.clone(), t.content_faint)
                                .line_height(px(pink::TextStyle::Caption.line_height()))
                                .font_features(pink::tabular_figures()),
                        )
                    });

            let edit = c.edit.clone().map(|edit| {
                let (_, since) = edit_seen.get(&c.id).copied().unwrap_or((edit.status, None));
                let cell = diff_heights.entry(c.id.clone()).or_default().clone();
                let (card, active) = edit_card(EditCard {
                    key: format!("{key}-edit-{}", c.id),
                    id: c.id.clone(),
                    command: command_of(&c.body),
                    line: c.line,
                    edit,
                    since,
                    now,
                    reduced,
                    resolved,
                    cell,
                    on_run: self.on_run.clone(),
                    on_undo: self.on_undo.clone(),
                    t: &t,
                });
                moving |= active;
                card
            });

            let cell = heights.get(&c.id).cloned().unwrap_or_default();
            let natural = cell.get();
            let band = comment_card::band(&comment_card::posted_style(&t), None)
                .group(group)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .px(px(comment_card::PAD_X))
                        .pt(px(CARD_PAD_V / 2.))
                        .pb(px(CARD_PAD_V / 2. + CARD_GAP))
                        .child(header)
                        .child(byline)
                        .child(body(&c.body, &t, space_w))
                        .children(edit),
                );

            // Resolving folds the card shut (Pink `fold_progress`: the
            // DISCLOSURE tween, the body gone by 60% of it)
            let fold = leaving.get(&c.id).map(|(at, _)| {
                let elapsed = (!reduced).then(|| now.saturating_duration_since(*at));
                motion::fold_progress(false, elapsed)
            });
            let row = div()
                .id(ElementId::Name(format!("{key}-row-{}", c.id).into()))
                .w_full()
                .flex_none()
                .overflow_hidden()
                .when_some(fold.filter(|_| natural > 0.), |d, (open, alpha)| {
                    d.h(px(natural * open)).opacity(alpha)
                })
                // The gap to the next card folds with the card
                .child(
                    div()
                        .relative()
                        .flex_none()
                        .pb(px(space::S8))
                        .child(band)
                        .child(measure(cell)),
                );

            // Rows reveal like Pink's new tool rows: the ones there when the
            // tab came up cascade in, later ones arrive on their own
            let delay = batch
                .iter()
                .position(|id| *id == c.id)
                .map_or(Duration::ZERO, |ix| {
                    motion::tool_row_arrival_delay(ix, true)
                });
            items.push(
                motion::rise_in(
                    ElementId::Name(format!("{key}-in-{tab:?}-{}", c.id).into()),
                    motion::TOOL_ROW_REVEAL.with_delay(delay.as_millis() as u64),
                    motion::TRANSCRIPT_IN_RISE,
                    row,
                )
                .into_any_element(),
            );
        }
        state.update(cx, |s, _| {
            s.diff_heights = std::mem::take(&mut diff_heights)
        });

        let list = div()
            .id(ElementId::Name(format!("{key}-list").into()))
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .flex()
            .flex_col()
            .children(items)
            .when(empty, |d| {
                // Pink surface_chrome `surface_state`
                let (title, copy) = match tab {
                    Tab::Open => (
                        "All caught up",
                        format!("New comments on {} show up here.", self.document),
                    ),
                    Tab::Resolved => (
                        "Nothing resolved yet",
                        "Resolve a comment to file it here.".to_string(),
                    ),
                };
                d.child(
                    pink::surface_state(
                        svg()
                            .path(comment_card::ICON_COMMENT)
                            .size(px(28.))
                            .text_color(t.text_faint),
                        title,
                        Some(copy.into()),
                        &t,
                    )
                    .flex_1()
                    .min_h_0(),
                )
            });

        // ---- Pink: comment draft, with the slash completion above it ------
        let popup =
            popup.map(|popup| slash_popup(&key, popup, exit, &state, &t).into_any_element());
        let cancel_state = state.clone();
        let commit_state = state.clone();
        let draft = div()
            .relative()
            .h(px(comment_card::draft_height(window.rem_size().into())))
            .w_full()
            .flex_none()
            .flex()
            .flex_row()
            .bg(comment_card::draft_style(&t).background)
            // The pane's bottom corners round the band (GPUI clips square),
            // so the bar is the band's left edge here.
            .rounded_b(px(radius::PANEL - 1.))
            .border_l(px(ACCENT_BAR_WIDTH))
            .border_color(comment_card::draft_style(&t).accent)
            .child(
                comment_card::content(None)
                    .child(card_header(self.document.clone(), &t))
                    .child(comment_card::field(
                        div()
                            .text_size(px(text::FOOTNOTE))
                            .line_height(px(CARD_LINE_HEIGHT))
                            .text_color(t.text)
                            .child(input.clone()),
                    ))
                    .child(
                        comment_card::actions()
                            .child(
                                comment_action(format!("{key}-cancel"), "Cancel", false, true, &t)
                                    .on_click(move |_, _, cx| {
                                        cancel_state.update(cx, |s, cx| s.clear(cx))
                                    }),
                            )
                            .child(
                                comment_action(format!("{key}-commit"), "Comment", true, true, &t)
                                    .on_click(move |_, window, cx| {
                                        commit_state.update(cx, |s, cx| s.submit(window, cx))
                                    }),
                            ),
                    ),
            )
            .children(popup);

        if moving {
            window.request_animation_frame();
        }
        // Hover fades started in this render need their frames
        motion::keep_alive(window);

        div()
            .id(self.id.clone())
            .relative()
            .w(px(self.width))
            .h(px(self.height))
            .max_w_full()
            .flex()
            .flex_col()
            .rounded(px(radius::PANEL))
            .border_1()
            .border_color(t.border)
            .bg(t.bg)
            .overflow_hidden()
            .font_family(pink::SANS)
            .text_size(px(text::FOOTNOTE))
            .line_height(px(CARD_LINE_HEIGHT))
            .text_color(t.text)
            .child(toolbar)
            .child(list)
            .child(draft)
    }
}

/// Pink composer.rs `render_slash_popup_body`: a full-width stack popup card
/// above the draft, one row per command (a 14px command glyph, the name at
/// 12.5 medium, the hint at 12 `text_muted`), one highlight that the arrows
/// and the pointer share. It rises in from below (Pink `menu_motion_up`) and
/// fades out over `MENU_OUT` on close.
fn slash_popup(
    key: &str,
    popup: Popup,
    exit: Option<(f32, Instant)>,
    state: &Entity<State>,
    t: &Pink,
) -> impl IntoElement {
    let live = exit.is_none();
    let card = pink::stack_popup_card(t)
        .w_full()
        .max_h(px(POPUP_MAX_HEIGHT))
        .flex()
        .flex_col();
    let card = if popup.rows.is_empty() {
        card.child(
            div()
                .px(px(space::S12))
                .py(px(space::S10))
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_muted)
                .child("No matching commands"),
        )
    } else {
        let rows = popup.rows.iter().enumerate().map(|(ix, command)| {
            let hover_state = state.clone();
            let pick_state = state.clone();
            pink::menu_row_cursor(ix == popup.highlight, t)
                .id(ElementId::Name(format!("{key}-slash-{ix}").into()))
                .when(live, |row| {
                    row.on_mouse_move(move |_, _, cx| {
                        hover_state.update(cx, |s, cx| {
                            if s.highlight != ix {
                                s.highlight = ix;
                                cx.notify();
                            }
                        })
                    })
                    .on_click(move |_, window, cx| {
                        let input = pick_state.read(cx).input.clone();
                        pick_state.update(cx, |s, cx| s.accept(ix, cx));
                        TextInput::focus(&input, window, cx);
                    })
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(space::S8))
                        .child(
                            svg()
                                .path(ICON_COMMAND)
                                .flex_none()
                                .size(px(icon_size::SM))
                                .text_color(t.text_muted),
                        )
                        .child(
                            div()
                                .flex_none()
                                .text_size(px(text::CODE))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(t.text)
                                .child(format!("/{}", command.name)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .overflow_hidden()
                                .truncate()
                                .text_size(px(text::FOOTNOTE))
                                .text_color(t.text_muted)
                                .child(command.hint.clone()),
                        ),
                )
        });
        card.child(
            div()
                .id(ElementId::Name(format!("{key}-slash-list").into()))
                .max_h(px(POPUP_LIST_MAX_HEIGHT))
                .flex()
                .flex_col()
                .gap(px(layout::MENU_GAP))
                .overflow_y_scroll()
                .children(rows),
        )
    };
    // Inset from the pane's edges; the left offset starts inside the draft's
    // bar, which is its left border.
    let holder = div()
        .absolute()
        .bottom_full()
        .left(px(layout::WINDOW_MARGIN - ACCENT_BAR_WIDTH))
        .right(px(layout::WINDOW_MARGIN))
        .occlude()
        .pb(px(POPUP_GAP));
    match exit {
        None => holder
            .child(motion::menu_in_from(
                ElementId::Name(format!("{key}-slash-in").into()),
                motion::MENU_RISE_UP,
                card,
            ))
            .into_any_element(),
        Some((progress, at)) => holder
            .child(motion::menu_out(
                ElementId::Name(format!("{key}-slash-out-{at:?}").into()),
                progress,
                card,
            ))
            .into_any_element(),
    }
}

/// Pink comment_ui.rs card header: the comment glyph and the path in mono.
fn card_header(location: SharedString, t: &Pink) -> gpui::Div {
    comment_card::header(svg().path(comment_card::ICON_COMMENT), t.text_faint)
        .child(comment_card::location(location, t.text_faint, pink::MONO))
}

/// Pink comment_ui.rs `comment_action`: the primary is the `solid` plate;
/// the other is a ghost whose label and `element_hover` wash ease in under
/// the pointer. Disabled is Pink's 0.45.
fn comment_action(
    id: String,
    label: &'static str,
    primary: bool,
    enabled: bool,
    t: &Pink,
) -> Stateful<gpui::Div> {
    let ring = t.focus_ring();
    let style = if primary || enabled {
        comment_card::action_style(&id, primary, t)
    } else {
        comment_card::ActionStyle {
            background: t.element_hover.opacity(0.),
            text: t.text_muted,
        }
    };
    let action = comment_card::action(&style, label)
        .id(ElementId::Name(id.clone().into()))
        .role(Role::Button)
        .aria_label(label)
        .flex_none()
        .on_key_down(pink::swallow_activation_key);
    let action = if enabled {
        action
            .tab_index(0)
            .focus_visible(move |s| pink::focus_ring(s, ring))
    } else {
        action.cursor_default().opacity(0.45)
    };
    if primary || !enabled {
        action
    } else {
        action.on_hover(motion::hover_listener(id))
    }
}

/// What an edit chip card needs.
struct EditCard<'a> {
    key: String,
    id: SharedString,
    command: Option<SharedString>,
    line: Option<u32>,
    edit: EditCommand,
    /// When the edit reached its status (`None`: it was there at mount).
    since: Option<Instant>,
    now: Instant,
    reduced: bool,
    resolved: bool,
    cell: Rc<Cell<f32>>,
    on_run: Option<IdHandler>,
    on_undo: Option<IdHandler>,
    t: &'a Pink,
}

/// The edit a slash-command comment runs, as Pink's tool chip card
/// (transcript.rs: r10, `hairline(0.07)` edge over `ink(0.03)`, a 28px header
/// with an 18px glyph tile, the label in 12/18 medium `content_muted` and the
/// detail truncating). Running swaps the glyph for Pink's mini glyph spinner
/// and shimmers the verb with a tabular timer; applied, the card folds open
/// (DISCLOSURE) onto the change as Pink's diff rows. Returns whether it is
/// still animating.
fn edit_card(card: EditCard) -> (AnyElement, bool) {
    let EditCard {
        key,
        id,
        command,
        line,
        edit,
        since,
        now,
        reduced,
        resolved,
        cell,
        on_run,
        on_undo,
        t,
    } = card;
    let label: SharedString = command
        .as_deref()
        .map_or_else(|| "Edit".to_string(), title_case)
        .into();
    let tile = div()
        .size(px(CHIP_TILE))
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(radius::SM))
        .bg(t.ink(0.08));
    let tile = if edit.status == EditStatus::Running {
        tile.child(loaders::mini_glyph_spinner(
            format!("{key}-spin"),
            2.0,
            t.glyph,
        ))
    } else {
        tile.child(
            svg()
                .path(ICON_EDIT)
                .size(px(icon_size::XS))
                .text_color(t.content_muted),
        )
    };
    let label_el: AnyElement = match edit.status {
        EditStatus::Running => {
            let verb: SharedString = match &command {
                Some(name) => participle(name).into(),
                None => "Editing".into(),
            };
            let mut shimmer = loaders::shimmer_text(
                verb,
                loaders::TOOL_LABEL_SIZE,
                loaders::TOOL_LABEL_LINE_HEIGHT,
                t.content_muted,
                t.text,
            )
            .live(!reduced)
            .flex_none()
            .font_weight(FontWeight::MEDIUM);
            if let Some(since) = since {
                shimmer = shimmer.started_at(since);
            }
            shimmer.into_any_element()
        }
        _ => div()
            .flex_none()
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.content_muted)
            .child(label)
            .into_any_element(),
    };
    let detail: SharedString = match (edit.status, line) {
        (EditStatus::Applied, Some(line)) => format!("Applied to line {line}").into(),
        (EditStatus::Applied, None) => "Applied".into(),
        _ => edit.before.clone(),
    };
    let trailing: Option<AnyElement> = match edit.status {
        EditStatus::Ready => {
            let enabled = !resolved && on_run.is_some();
            let id = id.clone();
            Some(
                comment_action(format!("{key}-run"), "Run", true, enabled, t)
                    .when_some(on_run.filter(|_| enabled), |b, f| {
                        b.on_click(move |_, window, cx| f(id.clone(), window, cx))
                    })
                    .into_any_element(),
            )
        }
        EditStatus::Running => since.map(|since| {
            let secs = now.saturating_duration_since(since).as_secs() as i64;
            div()
                .flex_none()
                .pr(px(space::S4))
                .text_size(px(text::CAPTION))
                .font_features(pink::tabular_figures())
                .text_color(t.content_faint)
                .child(loaders::format_elapsed(secs))
                .into_any_element()
        }),
        EditStatus::Applied => {
            let enabled = on_undo.is_some();
            let id = id.clone();
            Some(
                comment_action(format!("{key}-undo"), "Undo", false, enabled, t)
                    .when_some(on_undo, |b, f| {
                        b.on_click(move |_, window, cx| f(id.clone(), window, cx))
                    })
                    .into_any_element(),
            )
        }
    };
    let header = div()
        .h(px(CHIP_HEADER_HEIGHT))
        .w_full()
        .min_w_0()
        .flex_none()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(space::S8))
        .pl(px(space::S8))
        .pr(px(space::S4))
        .text_size(px(loaders::TOOL_LABEL_SIZE))
        .line_height(px(loaders::TOOL_LABEL_LINE_HEIGHT))
        .child(tile)
        .child(label_el)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(if edit.status == EditStatus::Applied {
                    t.content_muted
                } else {
                    t.text.opacity(0.85)
                })
                .child(detail),
        )
        .children(trailing);

    let mut active = false;
    let diff = (edit.status == EditStatus::Applied).then(|| {
        let elapsed = since
            .filter(|_| !reduced)
            .map(|at| now.saturating_duration_since(at));
        let (open, alpha) = motion::fold_progress(true, elapsed);
        active = open < 1. || alpha < 1.;
        let natural = cell.get();
        let rows = div()
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .py(px(space::S4))
            .border_t_1()
            .border_color(t.hairline(0.07))
            .opacity(alpha)
            .child(diff_row(false, edit.before.clone(), t))
            .child(diff_row(true, edit.after.clone(), t))
            .child(measure(cell));
        div()
            .w_full()
            .flex_none()
            .overflow_hidden()
            .when(open < 1., |d| d.h(px(natural * open)))
            .child(rows)
    });

    let card = div()
        .mt(px(space::S8))
        .min_h(px(CHIP_CARD_HEIGHT))
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded(px(radius::LG))
        .border_1()
        .border_color(t.hairline(0.07))
        .bg(t.ink(0.03))
        .child(header)
        .children(diff)
        .into_any_element();
    (card, active)
}

/// One Pink diff row (changes.rs `diff_row_paint`): the add/delete tint at
/// 0.10 dark / 0.08 light and the marker, both floored on the tint; the text
/// in Pink's plain diff tone, mono 12 at 21.
fn diff_row(added: bool, line: SharedString, t: &Pink) -> gpui::Div {
    let tone = if added { t.diff_add } else { t.diff_del };
    let wash = tone.opacity(if t.dark { 0.10 } else { 0.08 });
    div()
        .flex()
        .flex_row()
        .bg(wash)
        .font_family(pink::MONO)
        .text_size(px(text::FOOTNOTE))
        .line_height(px(DIFF_LINE_HEIGHT))
        .child(
            div()
                .w(px(MARKER_WIDTH))
                .flex_none()
                .flex()
                .justify_center()
                .text_color(t.readable_on(tone, wash, pink::TEXT_CONTRAST_TARGET))
                .child(if added { "+" } else { "−" }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .pl(px(CODE_PAD_LEFT))
                .pr(px(space::S8))
                .text_color(t.readable_on(t.text.opacity(0.92), wash, pink::TEXT_CONTRAST_TARGET))
                .child(line),
        )
}

/// A word of the note: plain, or the leading `/command`.
enum Word<'a> {
    Plain(&'a str),
    Command(&'a str),
}

/// The leading `/command` of a note, without the slash.
fn command_of(body: &str) -> Option<SharedString> {
    let word = body.split_whitespace().next()?;
    let name = word.strip_prefix('/')?;
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_'))
    .then(|| SharedString::from(name.to_string()))
}

fn title_case(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// "rewrite" → "Rewriting": the verb Pink's working line shimmers.
fn participle(name: &str) -> String {
    let stem = match name.strip_suffix('e') {
        Some(stem) if !name.ends_with("ee") => stem,
        _ => name,
    };
    title_case(&format!("{stem}ing"))
}

/// The note at Pink's card body (12/18 `text_dim`). A leading command reads
/// as Pink's inline code (`code_text` on a rounded `code_wash`, mono), laid
/// out word by word (a GPUI text run can't hold a rounded plate) with the
/// space as the gap.
fn body(text: &SharedString, t: &Pink, space: Pixels) -> AnyElement {
    let base = div()
        .text_size(px(text::FOOTNOTE))
        .line_height(px(CARD_LINE_HEIGHT))
        .text_color(t.text_dim);
    if command_of(text).is_none() {
        return base.child(text.clone()).into_any_element();
    }
    let words = text.split_whitespace().enumerate().map(|(ix, w)| {
        if ix == 0 {
            Word::Command(w)
        } else {
            Word::Plain(w)
        }
    });
    base.flex()
        .flex_wrap()
        .items_center()
        .gap_x(space)
        .children(words.map(|word| {
            match word {
                Word::Plain(w) => div().whitespace_nowrap().child(w.to_string()),
                Word::Command(w) => div().h(px(CARD_LINE_HEIGHT)).flex().items_center().child(
                    div()
                        .h(px(CARD_LINE_HEIGHT - 2. * INLINE_CODE_INSET_Y))
                        .flex()
                        .items_center()
                        .px(px(INLINE_CODE_PAD_X))
                        .rounded(px(INLINE_CODE_RADIUS))
                        .bg(t.code_wash)
                        .text_color(t.code_text)
                        .font_family(pink::MONO)
                        .child(w.to_string()),
                ),
            }
        }))
        .into_any_element()
}

/// Width of a space in the body text, for laying out tagged text word by word.
fn space_width(window: &Window) -> Pixels {
    let run = TextRun {
        len: 1,
        font: font(pink::SANS),
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(" ".into(), px(text::FOOTNOTE), &[run], None)
        .width
}

/// Records an element's laid-out height, for folding it.
fn measure(cell: Rc<Cell<f32>>) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let h = f32::from(bounds.size.height);
            if (cell.get() - h).abs() > 0.5 {
                cell.set(h);
                window.on_next_frame(|window, _| window.refresh());
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

// ---------------------------------------------------------------------------
// Story: review comments on a pricing page draft.
// ---------------------------------------------------------------------------

/// How long the demo's edits take to run.
const DEMO_RUN: Duration = Duration::from_millis(1600);

struct Demo {
    comments: Vec<Comment>,
    next_id: usize,
    tasks: Vec<Task<()>>,
}

impl Demo {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut demo = Demo {
            comments: vec![
                Comment::new(
                    "c1",
                    "Priya Natarajan",
                    "/rewrite make the headline shorter, under six words",
                )
                .time("12m")
                .line(3)
                .edit(EditCommand::new(
                    "Simple pricing that scales with your whole team",
                    "Pricing that grows with you",
                )),
                Comment::new(
                    "c2",
                    "Tomás Ortega",
                    "Do we still offer the nonprofit discount? It's missing from the FAQ.",
                )
                .time("25m")
                .line(41),
                Comment::new(
                    "c3",
                    "Ada Kim",
                    "/translate the plan names into German for the DE page",
                )
                .time("1h")
                .line(12)
                .edit(EditCommand::new(
                    "Starter, Team and Enterprise",
                    "Starter, Team und Unternehmen",
                )),
                Comment::new(
                    "c4",
                    "Felix Braun",
                    "Swapped the screenshots for the new dashboard.",
                )
                .time("2h")
                .line(20)
                .resolved(true),
            ],
            next_id: 5,
            tasks: Vec::new(),
        };
        // Someone ran the translation just before the pane opened
        demo.run("c3".into(), cx);
        demo
    }

    fn set_status(&mut self, id: &str, status: EditStatus) {
        if let Some(edit) = self
            .comments
            .iter_mut()
            .find(|c| c.id.as_ref() == id)
            .and_then(|c| c.edit.as_mut())
        {
            edit.status = status;
        }
    }

    fn run(&mut self, id: SharedString, cx: &mut Context<Self>) {
        self.set_status(&id, EditStatus::Running);
        self.tasks.push(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEMO_RUN).await;
            let _ = this.update(cx, |d, cx| {
                d.set_status(&id, EditStatus::Applied);
                cx.notify();
            });
        }));
        cx.notify();
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let demo = window.use_keyed_state("comments-panel-demo", cx, |_, cx| Demo::new(cx));
        let (on_submit, on_resolve, on_run, on_undo) =
            (demo.clone(), demo.clone(), demo.clone(), demo.clone());
        CommentsPanel::new("comments-panel", demo.read(cx).comments.clone())
            .document("pricing.md")
            .size(420., 680.)
            .on_submit(move |text, _, cx| {
                on_submit.update(cx, |d, cx| {
                    let id: SharedString = format!("c{}", d.next_id).into();
                    d.next_id += 1;
                    let mut comment = Comment::new(id, "Jun Park", text.clone()).time("now");
                    if text.starts_with("/rewrite") || text.starts_with("/shorten") {
                        comment = comment.line(57).edit(EditCommand::new(
                            "Cancel anytime, no questions asked",
                            "Cancel whenever you like",
                        ));
                    }
                    d.comments.push(comment);
                    cx.notify();
                })
            })
            .on_resolve(move |id, _, cx| {
                on_resolve.update(cx, |d, cx| {
                    if let Some(c) = d.comments.iter_mut().find(|c| c.id == id) {
                        c.resolved = !c.resolved;
                    }
                    cx.notify();
                })
            })
            .on_run(move |id, _, cx| on_run.update(cx, |d, cx| d.run(id, cx)))
            .on_undo(move |id, _, cx| {
                on_undo.update(cx, |d, cx| {
                    d.set_status(&id, EditStatus::Ready);
                    cx.notify();
                })
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_read_from_the_leading_word() {
        assert_eq!(command_of("/rewrite it").as_deref(), Some("rewrite"));
        assert_eq!(command_of("rewrite it"), None);
        assert_eq!(command_of("/ alone"), None);
    }

    #[test]
    fn verbs_shimmer_as_participles() {
        assert_eq!(participle("rewrite"), "Rewriting");
        assert_eq!(participle("translate"), "Translating");
        assert_eq!(participle("shorten"), "Shortening");
        assert_eq!(participle("assign"), "Assigning");
    }
}
