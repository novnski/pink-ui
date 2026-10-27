//! Combobox – pick one or many from a searchable, grouped list.
//!
//! A Pink component. The list is Pink's searchable picker card (Pink:
//! `shell/project_picker.rs` `ProjectPicker::render`): a popover card under
//! the field with rows of a 28 px monogram tile, a medium name and an 11 px
//! subline; one highlight that the arrows and the pointer share; a check on
//! every pick; uppercase group headings; typed words lit in `code_text` on a
//! rounded `code_wash` (Pink: `popover::search_highlight`); a pinned
//! "Invite “…”" row under a full-bleed hairline when nothing fits exactly
//! (Pink: `ProjectPicker::footer_row`); and a key legend at the foot (Pink:
//! the add-project palette's footer).
//!
//! The field is pink-ui's own, drawn as a Pink dialog field
//! (`popover::dialog_field`) that holds the picks as message badges (Pink:
//! `badges::render`) ahead of the search text. Backspace on an empty field
//! first marks the last pick (Pink's selected-chip wash), then removes it.
//!
//! Keys: type to filter, ↑/↓ to move, Enter to pick or unpick, Backspace on
//! an empty field to unpick the last pick, Escape to close.
//!
//! ```ignore
//! Combobox::new("share", default_options())
//!     .selected(["design", "carla"])
//!     .creatable("Invite")
//!     .on_change(|ids, _, _| println!("{ids:?}"))
//! ```

use std::{ops::Range, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    AnimationExt, AnyElement, AnyView, App, BorderStyle, Bounds, Context, Div, ElementId, Entity,
    Focusable, FontWeight, HighlightStyle, IntoElement, MouseButton, MouseDownEvent, Pixels,
    ScrollHandle, SharedString, StyledText, Subscription, Task, TextLayout, Window, canvas,
    deferred, div, point, prelude::*, px, quad, relative, size, svg, transparent_black,
};
use web_time::Instant;

use crate::{
    HoverFill,
    motion::{self, INSERT, MENU_OUT, MENU_RISE_DOWN, QUICK},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, layout, radius},
    primitives::{InputEvent, TextInput},
    story,
};

pub const NAME: &str = "Combobox";
pub const DESCRIPTION: &str = "Pick one or many from a searchable picker card: grouped rows with monogram tiles, typed words lit, a check on every pick, picks as badges in the field and an invite row for what isn't listed";

const ICON_CLOSE: &str = "icons/combobox/close.svg";
const ICON_PLUS: &str = "icons/combobox/plus.svg";
const ICON_USERS: &str = "icons/combobox/users.svg";
const ICON_ARROW_UP: &str = "icons/combobox/arrow-up.svg";
const ICON_ARROW_DOWN: &str = "icons/combobox/arrow-down.svg";
const ICON_GLOBE: &str = "icons/combobox/globe.svg";
const ICON_COPY: &str = "icons/combobox/copy.svg";

/// A row's tile (Pink: `ProjectPicker::render`, `project_tile(.., 28.0, ..)`).
const TILE: f32 = 28.;
/// The tile inside a pick's badge in the field.
const BADGE_TILE: f32 = 16.;
/// A pick's badge: Pink's message badge at its 24 px floor.
const BADGE_H: f32 = 24.;
/// The field: the badge plus `py 6` and the 1 px edge (a dialog field's
/// height, Pink `popover::dialog_field`).
const FIELD_PAD_Y: f32 = 6.;
/// The list's scroll budget (Pink: the picker's `max_h(ui_rems(224.0))`).
const LIST_MAX: f32 = 224.;
/// Where created entries are listed.
const CREATED: &str = "Invited";

type ChangeHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;

/// What an option shows in its tile.
#[derive(Clone, Debug)]
pub enum Avatar {
    /// A monogram on one of Pink's project hues, picked from the id (Pink:
    /// `sidebar_projects::project_tile`).
    Initials,
    /// An SVG path under `assets/` on a neutral tile, for teams and groups
    /// (Pink: `sidebar_projects::all_projects_tile`).
    Icon(SharedString),
}

#[derive(Clone, Debug)]
pub struct ComboOption {
    pub id: SharedString,
    pub label: SharedString,
    /// The subline under the name.
    pub detail: Option<SharedString>,
    /// The heading the option is listed under.
    pub group: Option<SharedString>,
    pub avatar: Avatar,
}

impl ComboOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            detail: None,
            group: None,
            avatar: Avatar::Initials,
        }
    }

    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn group(mut self, group: impl Into<SharedString>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.avatar = Avatar::Icon(path.into());
        self
    }

    /// One of Pink's six project hues, stable for the id.
    fn tint_index(&self) -> usize {
        (pink::loaders::fnv1a(self.id.as_bytes()) % 6) as usize
    }

    /// The name a pick shows in the field: a person's first name, a team's
    /// or an invite's whole label.
    fn short_label(&self) -> SharedString {
        match self.avatar {
            Avatar::Initials if self.group.as_deref() != Some(CREATED) => self
                .label
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string()
                .into(),
            _ => self.label.clone(),
        }
    }
}

/// Three teams and eight people.
pub fn default_options() -> Vec<ComboOption> {
    let team = |id: &'static str, name: &'static str, detail: &'static str| {
        ComboOption::new(id, name)
            .detail(detail)
            .group("Teams")
            .icon(ICON_USERS)
    };
    let person = |id: &'static str, name: &'static str, role: &'static str| {
        ComboOption::new(id, name).detail(role).group("People")
    };
    vec![
        team("design", "Design team", "8 members"),
        team("platform", "Platform team", "12 members"),
        team("growth", "Growth team", "5 members"),
        person("aiko", "Aiko Tanaka", "Design lead"),
        person("ben", "Ben Adeyemi", "iOS engineer"),
        person("carla", "Carla Mendes", "Product manager"),
        person("dev", "Dev Malhotra", "Data analyst"),
        person("elena", "Elena Rossi", "Marketing"),
        person("felix", "Felix Braun", "Backend engineer"),
        person("grace", "Grace Lin", "Support lead"),
        person("hugo", "Hugo Laurent", "Legal counsel"),
    ]
}

/// Pink: `popover::match_rank`, extended to the subline: `0` the name starts
/// with the query, `1` the name contains it, `2` only the subline does.
fn match_rank(query: &str, option: &ComboOption) -> Option<usize> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(1);
    }
    let label = option.label.to_lowercase();
    if label.starts_with(&query) {
        Some(0)
    } else if label.contains(&query) {
        Some(1)
    } else if option
        .detail
        .as_ref()
        .is_some_and(|detail| detail.to_lowercase().contains(&query))
    {
        Some(2)
    } else {
        None
    }
}

/// Pink: `popover::menu_step`. Steps the highlight, wrapping at both ends.
fn menu_step(active: usize, count: usize, delta: isize) -> usize {
    if count == 0 {
        return 0;
    }
    (active as isize + delta).rem_euclid(count as isize) as usize
}

struct ComboState {
    input: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
    query: SharedString,
    open: bool,
    /// When the card began closing: it fades out over `MENU_OUT`, then
    /// unmounts.
    closing: Option<Instant>,
    _close_task: Option<Task<()>>,
    /// Bumped on every open so the card replays its entrance.
    opened: u32,
    /// The highlight: an index into [`Self::rows`], or one past it on the
    /// pinned invite row.
    active: usize,
    selected: Vec<SharedString>,
    created: Vec<ComboOption>,
    /// Backspace on an empty field marked the last pick.
    armed: bool,
    field: Bounds<Pixels>,
    scroll: ScrollHandle,
    options: Rc<Vec<ComboOption>>,
    multiple: bool,
    creatable: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl ComboState {
    fn all(&self) -> impl Iterator<Item = &ComboOption> {
        self.options.iter().chain(self.created.iter())
    }

    fn find_option(&self, id: &SharedString) -> Option<ComboOption> {
        self.all().find(|o| o.id == *id).cloned()
    }

    /// The listed options: those matching the query, groups kept together in
    /// the order they first appear, ranked within each group (Pink:
    /// `popover::filter_indices`).
    fn rows(&self) -> Vec<ComboOption> {
        let mut groups: Vec<Option<SharedString>> = Vec::new();
        for option in self.all() {
            if !groups.contains(&option.group) {
                groups.push(option.group.clone());
            }
        }
        let mut ranked: Vec<(usize, usize, usize, &ComboOption)> = self
            .all()
            .enumerate()
            .filter_map(|(ix, option)| {
                let rank = match_rank(&self.query, option)?;
                let group = groups.iter().position(|g| *g == option.group)?;
                Some((group, rank, ix, option))
            })
            .collect();
        ranked.sort_by_key(|&(group, rank, ix, _)| (group, rank, ix));
        ranked.into_iter().map(|(.., o)| o.clone()).collect()
    }

    /// The query, when the invite row offers it: something typed that no
    /// label matches exactly.
    fn create_label(&self) -> Option<SharedString> {
        let query = self.query.trim();
        let exact = self.all().any(|o| o.label.eq_ignore_ascii_case(query));
        (self.creatable.is_some() && !query.is_empty() && !exact).then(|| query.to_string().into())
    }

    fn nav_count(&self) -> usize {
        self.rows().len() + usize::from(self.create_label().is_some())
    }

    /// Pink: the highlight starts on the current choice.
    fn home(&self) -> usize {
        if self.multiple {
            return 0;
        }
        let rows = self.rows();
        self.selected
            .first()
            .and_then(|id| rows.iter().position(|o| o.id == *id))
            .unwrap_or(0)
    }

    fn open_list(&mut self) {
        if self.open {
            return;
        }
        self.open = true;
        self.closing = None;
        self._close_task = None;
        self.opened += 1;
        let home = self.home();
        self.set_active(home);
    }

    /// Starts the exit fade; the card unmounts when it ends. `clear` empties
    /// the field's search text once the card is gone, so the dying card
    /// keeps its rows.
    fn close_list(&mut self, clear: bool, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.armed = false;
        self.closing = Some(Instant::now());
        let wait = MENU_OUT.total().mul_f32(motion::speed_scale()) + Duration::from_millis(16);
        self._close_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |s, cx| {
                s.closing = None;
                if clear {
                    s.clear_query(cx);
                }
                cx.notify();
            });
        }));
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                if text.is_empty() && self.query.is_empty() {
                    // Backspace on an empty field: mark the last pick, then remove it.
                    if self.armed {
                        self.armed = false;
                        if let Some(id) = self.selected.last().cloned() {
                            self.toggle(&id, window, cx);
                        }
                    } else {
                        self.armed = !self.selected.is_empty();
                    }
                } else {
                    self.query = text.clone();
                    self.armed = false;
                    self.open_list();
                    self.set_active(0);
                }
            }
            InputEvent::MoveDown | InputEvent::MoveUp => {
                self.armed = false;
                if !self.open {
                    self.open_list();
                } else {
                    let delta = if *event == InputEvent::MoveDown {
                        1
                    } else {
                        -1
                    };
                    let next = menu_step(self.active, self.nav_count(), delta);
                    self.set_active(next);
                }
            }
            InputEvent::Submit(_) => {
                self.armed = false;
                if !self.open {
                    self.open_list();
                } else {
                    self.activate(self.active, window, cx);
                }
            }
            InputEvent::Escape => {
                self.armed = false;
                if self.open {
                    self.close_list(true, cx);
                } else if !self.query.is_empty() {
                    self.clear_query(cx);
                }
            }
            InputEvent::Tab => self.close_list(false, cx),
            // A focus that lands while the card fades out (a refocus after a
            // pick) must not reopen it.
            InputEvent::Focus if self.closing.is_none() => self.open_list(),
            InputEvent::Blur => self.armed = false,
            _ => return,
        }
        cx.notify();
    }

    fn clear_query(&mut self, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.clear(cx));
        self.query = SharedString::default();
        self.set_active(0);
    }

    fn set_active(&mut self, ix: usize) {
        self.active = ix;
        let rows = self.rows();
        // The pinned invite row sits below the scroller.
        if ix >= rows.len() {
            return;
        }
        if ix == 0 {
            // Show the first group's heading too.
            self.scroll.set_offset(point(px(0.), px(0.)));
            return;
        }
        // The scroller's children: a heading before each group, then its rows.
        let mut child = 0;
        let mut group: Option<Option<SharedString>> = None;
        for (i, option) in rows.iter().enumerate() {
            if group.as_ref() != Some(&option.group) {
                if option.group.is_some() {
                    child += 1;
                }
                group = Some(option.group.clone());
            }
            if i == ix {
                self.scroll.scroll_to_item(child);
                return;
            }
            child += 1;
        }
    }

    /// Enter or a click on row `ix`: pick or unpick it, or invite the query.
    fn activate(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows();
        let id = if let Some(option) = rows.get(ix) {
            option.id.clone()
        } else if let Some(label) = self.create_label() {
            let id: SharedString = format!("new:{label}").into();
            self.created.push(
                ComboOption::new(id.clone(), label)
                    .detail("Pending invite")
                    .group(CREATED),
            );
            id
        } else {
            return;
        };
        self.toggle(&id, window, cx);
        if !self.multiple {
            self.close_list(true, cx);
        } else if !self.query.is_empty() {
            self.clear_query(cx);
        }
    }

    fn toggle(&mut self, id: &SharedString, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected.contains(id) {
            self.selected.retain(|s| s != id);
        } else if self.multiple {
            self.selected.push(id.clone());
        } else {
            self.selected = vec![id.clone()];
        }
        self.armed = false;
        if let Some(f) = self.on_change.clone() {
            let ids = self.selected.clone();
            window.defer(cx, move |window, cx| f(&ids, window, cx));
        }
        cx.notify();
    }
}

#[derive(IntoElement)]
pub struct Combobox {
    id: ElementId,
    options: Vec<ComboOption>,
    selected: Vec<SharedString>,
    multiple: bool,
    placeholder: SharedString,
    empty: SharedString,
    creatable: Option<SharedString>,
    default_open: bool,
    autofocus: bool,
    on_change: Option<ChangeHandler>,
}

impl Combobox {
    pub fn new(id: impl Into<ElementId>, options: Vec<ComboOption>) -> Self {
        Self {
            id: id.into(),
            options,
            selected: Vec::new(),
            multiple: true,
            placeholder: "Search…".into(),
            empty: "No matches".into(),
            creatable: None,
            default_open: false,
            autofocus: false,
            on_change: None,
        }
    }

    /// The ids picked at first.
    pub fn selected(mut self, ids: impl IntoIterator<Item = &'static str>) -> Self {
        self.selected = ids.into_iter().map(SharedString::from).collect();
        self
    }

    /// One pick at a time (the card closes once you pick). Multiple is the default.
    pub fn single(mut self) -> Self {
        self.multiple = false;
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    /// The line the card shows when nothing matches (Pink: "No matching
    /// projects").
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty = text.into();
        self
    }

    /// Offers "<verb> “query”" when nothing matches exactly; picking it adds the entry.
    pub fn creatable(mut self, verb: impl Into<SharedString>) -> Self {
        self.creatable = Some(verb.into());
        self
    }

    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Focuses the field when it first renders.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    /// Called with every picked id whenever the picks change.
    pub fn on_change(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Combobox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let options = Rc::new(self.options);
        let multiple = self.multiple;

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            {
                let placeholder = self.placeholder.clone();
                let selected = self.selected.clone();
                let options = options.clone();
                let creatable = self.creatable.clone();
                let (default_open, autofocus) = (self.default_open, self.autofocus);
                move |window, cx| {
                    let input = cx.new(|cx| TextInput::new(window, cx).placeholder(placeholder));
                    let subscription = cx.subscribe_in(
                        &input,
                        window,
                        |s: &mut ComboState, _, event, window, cx| s.on_input(event, window, cx),
                    );
                    if autofocus {
                        let handle = input.read(cx).focus_handle(cx);
                        window.defer(cx, move |window, cx| window.focus(&handle, cx));
                    }
                    let mut state = ComboState {
                        input,
                        _subscriptions: vec![subscription],
                        query: SharedString::default(),
                        open: false,
                        closing: None,
                        _close_task: None,
                        opened: 0,
                        active: 0,
                        selected,
                        created: Vec::new(),
                        armed: false,
                        field: Bounds::default(),
                        scroll: ScrollHandle::new(),
                        options,
                        multiple,
                        creatable,
                        on_change: None,
                    };
                    if default_open {
                        state.open_list();
                    }
                    state
                }
            },
        );

        let input = state.update(cx, |s, _| {
            s.options = options.clone();
            s.multiple = multiple;
            s.creatable = self.creatable.clone();
            s.on_change = self.on_change.clone();
            s.input.clone()
        });
        input.update(cx, |input, _| {
            input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
        });

        let s = state.read(cx);
        let rows = s.rows();
        let create = s.create_label();
        let nav = rows.len() + usize::from(create.is_some());
        let active = s.active.min(nav.saturating_sub(1));
        let picks: Vec<ComboOption> = s
            .selected
            .iter()
            .filter_map(|id| s.find_option(id))
            .collect();
        let selected = s.selected.clone();
        let (open, closing, opened, armed) = (s.open, s.closing, s.opened, s.armed);
        let scroll = s.scroll.clone();
        let query = s.query.clone();
        let verb = self.creatable.clone().unwrap_or_else(|| "Create".into());

        let pick_count = picks.len();
        let badges = picks.into_iter().enumerate().map(|(ix, option)| {
            let marked = armed && ix + 1 == pick_count;
            pick_badge(&key, &option, marked, &state, &t)
        });

        let opener = state.clone();
        let measurer = state.clone();
        // pink-ui's field, on Pink's dialog field: a separator edge over the
        // Quaternary plate, 14 px text. Pink frames don't change on focus;
        // the caret shows it.
        let field = pink::dialog_field(&t)
            .id("field")
            .relative()
            .min_h(px(BADGE_H + 2. * (FIELD_PAD_Y + 1.)))
            .pl(px(FIELD_PAD_Y))
            .pr(px(10.))
            .py(px(FIELD_PAD_Y))
            .flex()
            .items_center()
            .gap(px(8.))
            .cursor_text()
            .on_click(move |_, window, cx| {
                opener.update(cx, |s, cx| {
                    TextInput::focus(&s.input, window, cx);
                    s.open_list();
                    cx.notify();
                })
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(4.))
                    .children(badges)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(96.))
                            .h(px(BADGE_H))
                            .pl(px(6.))
                            .flex()
                            .items_center()
                            .text_color(t.text)
                            .child(input.clone()),
                    ),
            )
            .child(pink::select_chevron(&t))
            .child(
                canvas(
                    move |bounds, _, cx| measurer.update(cx, |s, _| s.field = bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );

        let layer = (open || closing.is_some()).then(|| {
            let exit = (!open)
                .then(|| closing.map(motion::menu_exit_progress))
                .flatten();
            let card = card(Card {
                rows: &rows,
                create: create.clone(),
                active,
                selected: &selected,
                query: &query,
                verb: &verb,
                empty: &self.empty,
                multiple,
                state: &state,
                scroll: &scroll,
                t: &t,
            });
            let inner = div().pt(px(layout::POPOVER_GAP)).child(card);
            let inner: AnyElement = match exit {
                // A dying card takes no clicks (Pink: `menu_motion_from`).
                Some(progress) => motion::menu_out(
                    ElementId::NamedInteger("listbox-out".into(), u64::from(opened)),
                    progress,
                    inner.relative().child(div().absolute().inset_0().occlude()),
                )
                .into_any_element(),
                None => motion::menu_in_from(
                    ElementId::NamedInteger("listbox-in".into(), u64::from(opened)),
                    MENU_RISE_DOWN,
                    inner,
                )
                .into_any_element(),
            };
            let closer = state.clone();
            deferred(
                div()
                    .id("listbox")
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(relative(1.))
                    .occlude()
                    .on_mouse_down_out(move |event: &MouseDownEvent, _, cx| {
                        closer.update(cx, |s, cx| {
                            if s.open && !s.field.contains(&event.position) {
                                s.close_list(false, cx);
                                cx.notify();
                            }
                        })
                    })
                    .child(inner),
            )
            .with_priority(1)
        });

        motion::keep_alive(window);
        div()
            .id(self.id)
            .relative()
            .w_full()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(field)
            .children(layer)
    }
}

/// A pick in the field: Pink's message badge (`badges::render`: 24 tall,
/// radius 8, `ink(0.06)`, 12 medium `text_muted`) holding a 16 px tile, the
/// name and a remove button. Marked for removal it takes Pink's selected
/// chip look (`button::chip`: `accent_wash`, an `accent @ 0.55` edge, the
/// accent label), eased over `QUICK`; it fades in over `INSERT`.
fn pick_badge(
    key: &str,
    option: &ComboOption,
    marked: bool,
    state: &Entity<ComboState>,
    t: &Pink,
) -> impl IntoElement + use<> {
    let remover = state.clone();
    let id = option.id.clone();
    let mark = motion::follow(
        &format!("{key}-mark-{}", option.id),
        if marked { 1. } else { 0. },
        QUICK,
    );
    let unpick = format!("{key}-unpick-{}", option.id);
    pink::message_badge(t)
        .id(SharedString::from(format!("pick-{}", option.id)))
        .h(px(BADGE_H))
        .pl(px(3.))
        .pr(px(3.))
        .border_1()
        .border_color(motion::mix(
            t.accent.opacity(0.),
            t.accent.opacity(0.55),
            mark,
        ))
        .bg(motion::mix(t.ink(0.06), t.accent_wash, mark))
        .text_color(motion::mix(
            t.text_muted,
            t.readable_tone(t.accent, 1.0),
            mark,
        ))
        .child(tile(option, BADGE_TILE, t))
        .child(div().max_w(px(140.)).truncate().child(option.short_label()))
        .child(
            // Pink: the history search's close (a quiet `hover_wash` box, a
            // small CLOSE glyph that brightens).
            div()
                .id(SharedString::from(format!("unpick-{}", option.id)))
                .size(px(pink::chrome::MIN_HIT))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::XS))
                .cursor_pointer()
                .hover_wash(unpick.clone(), t.fill(FillTier::Tertiary))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    remover.update(cx, |s, cx| s.toggle(&id, window, cx))
                })
                .child(
                    svg()
                        .path(ICON_CLOSE)
                        .size(px(10.))
                        .flex_none()
                        .text_color(motion::hover_blend(&unpick, t.readable_faint(0.9), t.text)),
                ),
        )
        .with_animation(
            SharedString::from(format!("{key}-pick-in-{}", option.id)),
            INSERT.animation(),
            |el, k| el.opacity(k),
        )
}

/// An option's tile: a monogram on its hue (Pink:
/// `sidebar_projects::project_tile`) or a glyph on the neutral tile (Pink:
/// `all_projects_tile`: `text @ 0.07`, glyph `round(size × 0.58)` in
/// `text_muted`), both radius `round(size × 0.28)`.
fn tile(option: &ComboOption, size: f32, t: &Pink) -> Div {
    match &option.avatar {
        Avatar::Initials => pink::monogram_tile(
            pink::project_tint(option.tint_index()),
            &option.label,
            size,
            t,
        ),
        Avatar::Icon(path) => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px((size * 0.28).round()))
            .bg(t.text.opacity(0.07))
            .child(
                svg()
                    .path(path.clone())
                    .size(px((size * 0.58).round()))
                    .flex_none()
                    .text_color(t.text_muted),
            ),
    }
}

/// What the card shows, gathered for [`card`].
struct Card<'a> {
    rows: &'a [ComboOption],
    create: Option<SharedString>,
    active: usize,
    selected: &'a [SharedString],
    query: &'a SharedString,
    verb: &'a SharedString,
    empty: &'a SharedString,
    multiple: bool,
    state: &'a Entity<ComboState>,
    scroll: &'a ScrollHandle,
    t: &'a Pink,
}

/// The listbox (Pink: `ProjectPicker::render`): the popover card over the
/// scrolling rows or the empty line, the pinned invite row and the legend.
fn card(c: Card<'_>) -> Div {
    let t = c.t;
    let mut items: Vec<AnyElement> = Vec::new();
    let mut group: Option<Option<SharedString>> = None;
    for (ix, option) in c.rows.iter().enumerate() {
        if group.as_ref() != Some(&option.group) {
            if let Some(name) = &option.group {
                items.push(pink::menu_heading(name, t).into_any_element());
            }
            group = Some(option.group.clone());
        }
        let picked = c.selected.contains(&option.id);
        items.push(option_row(
            ix,
            option,
            picked,
            ix == c.active,
            c.query,
            c.state,
            t,
        ));
    }

    let mut card = pink::popover_card(t).w_full().flex().flex_col();
    card = if c.rows.is_empty() {
        // Pink: "No matching projects".
        card.child(
            div()
                .px(px(8.))
                .py(px(6.))
                .text_size(px(12.))
                .text_color(t.readable_faint(0.9))
                .child(c.empty.clone()),
        )
    } else {
        // Pink: `popover::menu_scroll_host` / `menu_scroll_list`. The
        // scroller bleeds through the card's inset and re-pads, so the rows
        // stay put and scroll under the card's edge.
        card.child(
            div().relative().mx(px(-layout::CARD_INSET)).child(
                div()
                    .id("options")
                    .px(px(layout::CARD_INSET))
                    .overflow_y_scroll()
                    .track_scroll(c.scroll)
                    .max_h(px(LIST_MAX))
                    .flex()
                    .flex_col()
                    .gap(px(layout::MENU_GAP))
                    .children(items),
            ),
        )
    };
    if let Some(label) = c.create {
        let index = c.rows.len();
        card = card.child(full_bleed_rule(t)).child(create_row(
            index,
            c.active == index,
            &label,
            c.verb,
            c.state,
            t,
        ));
    }
    let count = c.selected.len();
    card.child(full_bleed_rule(t)).child(
        // Pink: the add-project palette's footer legends, `gap 12` as in the
        // command palette.
        div()
            .px(px(8.))
            .py(px(2.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(pink::key_hint_pair(
                ICON_ARROW_UP,
                ICON_ARROW_DOWN,
                "Navigate",
                t,
            ))
            .child(pink::key_hint_text(
                "↵",
                if c.multiple { "Toggle" } else { "Select" },
                t,
            ))
            .child(pink::key_hint_text("esc", "Close", t))
            .child(div().flex_1())
            .when(c.multiple && count > 0, |el| {
                el.child(
                    div()
                        .flex_none()
                        .text_size(px(11.))
                        .font_features(pink::tabular_figures())
                        .text_color(t.readable_faint(0.9))
                        .child(format!("{count} selected")),
                )
            }),
    )
}

/// Pink: the picker's divider over its pinned rows. Full-bleed through the
/// card's inset (one stopping short of the edges reads as a mistake),
/// `border @ 0.6`, `my 2`.
fn full_bleed_rule(t: &Pink) -> Div {
    div()
        .my(px(layout::MENU_GAP))
        .mx(px(-layout::CARD_INSET))
        .h(px(1.))
        .flex_none()
        .bg(t.border.opacity(0.6))
}

/// One option (Pink: `ProjectPicker::render` rows): `menu_row_nav` with the
/// tile, the name in medium and the 11 px subline in `readable_faint(0.9)`,
/// typed words lit in both. The pointer moves the shared highlight.
fn option_row(
    ix: usize,
    option: &ComboOption,
    picked: bool,
    highlighted: bool,
    query: &SharedString,
    state: &Entity<ComboState>,
    t: &Pink,
) -> AnyElement {
    let pointer = state.clone();
    let picker = state.clone();
    pink::menu_row_nav(picked, highlighted, t)
        .id(("option", ix))
        .on_mouse_move(move |_, _, cx| {
            pointer.update(cx, |s, cx| {
                if s.active != ix {
                    s.active = ix;
                    cx.notify();
                }
            })
        })
        .on_click(move |_, window, cx| {
            picker.update(cx, |s, cx| {
                s.active = ix;
                s.activate(ix, window, cx);
                TextInput::focus(&s.input, window, cx);
            })
        })
        .child(tile(option, TILE, t))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .font_weight(FontWeight::MEDIUM)
                        .child(search_highlight(option.label.clone(), query, t)),
                )
                .when_some(option.detail.clone(), |el, detail| {
                    el.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.))
                            .text_color(t.readable_faint(0.9))
                            .child(search_highlight(detail, query, t)),
                    )
                }),
        )
        .into_any_element()
}

/// The pinned invite row (Pink: `ProjectPicker::footer_row` "New project…"):
/// `menu_row_nav` with a 12 px plus in `text_muted` and the label.
fn create_row(
    index: usize,
    highlighted: bool,
    label: &SharedString,
    verb: &SharedString,
    state: &Entity<ComboState>,
    t: &Pink,
) -> impl IntoElement + use<> {
    let pointer = state.clone();
    let creator = state.clone();
    pink::menu_row_nav(false, highlighted, t)
        .id("create")
        .on_mouse_move(move |_, _, cx| {
            pointer.update(cx, |s, cx| {
                if s.active != index {
                    s.active = index;
                    cx.notify();
                }
            })
        })
        .on_click(move |_, window, cx| {
            creator.update(cx, |s, cx| {
                s.activate(index, window, cx);
                TextInput::focus(&s.input, window, cx);
            })
        })
        .child(
            svg()
                .path(ICON_PLUS)
                .size(px(12.))
                .flex_none()
                .text_color(t.text_muted),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(format!("{verb} “{label}”")),
        )
}

/// Pink: `popover::search_highlight`. Typed words are recolored `code_text`
/// over rounded (r3) `code_wash` plates painted beneath the shaped glyphs;
/// the text keeps its font, weight and spacing.
fn search_highlight(text: SharedString, query: &str, t: &Pink) -> AnyElement {
    if query.trim().is_empty() {
        return text.into_any_element();
    }
    let ranges = search_match_ranges(&text, query);
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

/// Pink: `popover::search_match_ranges`. Every case-insensitive occurrence
/// of each typed word, as byte ranges of `text`, merged where they touch.
fn search_match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let folded = text.to_lowercase();
    let mut original = Vec::with_capacity(folded.len());
    for (start, ch) in text.char_indices() {
        let range = start..start + ch.len_utf8();
        for lower in ch.to_lowercase() {
            original.extend(std::iter::repeat_n(range.clone(), lower.len_utf8()));
        }
    }
    let mut ranges = Vec::new();
    for word in query.to_lowercase().split_whitespace() {
        for (start, _) in folded.match_indices(word) {
            ranges.push(original[start].start..original[start + word.len() - 1].end);
        }
    }
    ranges.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.start <= last.end
        {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}

/// Pink: `markdown::render::range_rects`. One rectangle per visual row of
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

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div().h(px(470.)).flex().flex_col().child(
            // A share dialog (Pink: `popover::dialog_card`, title, body,
            // controls at `mt 16`, Lg buttons at the end).
            pink::dialog_card(&t)
                .w(px(440.))
                .child(pink::dialog_title("Share “Q4 launch plan”", &t))
                .child(
                    pink::dialog_body("People you add can view, comment and suggest edits.", &t)
                        .mt(px(6.)),
                )
                .child(
                    div().mt(px(16.)).child(
                        Combobox::new("combobox", default_options())
                            .selected(["platform", "carla"])
                            .placeholder("Add people…")
                            .empty_text("No matching people or teams")
                            .creatable("Invite")
                            .default_open(true)
                            .autofocus(true),
                    ),
                )
                .child(
                    div()
                        .mt(px(12.))
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            svg()
                                .path(ICON_GLOBE)
                                .size(px(14.))
                                .flex_none()
                                .text_color(t.readable_muted(0.85)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.readable_muted(0.85))
                                .child("Anyone at Northwind with the link can view"),
                        ),
                )
                .child(
                    div()
                        .mt(px(16.))
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            pink::button("cb-copy", ButtonKind::Ghost, ControlSize::Lg, &t)
                                .child(pink::glyph(ICON_COPY, ControlSize::Lg))
                                .child("Copy link"),
                        )
                        .child(
                            pink::button("cb-share", ButtonKind::Primary, ControlSize::Lg, &t)
                                .child("Share"),
                        ),
                ),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_name_prefixes_before_substrings_and_sublines() {
        let grace = ComboOption::new("grace", "Grace Lin").detail("Support lead");
        assert_eq!(match_rank("gr", &grace), Some(0));
        assert_eq!(match_rank("lin", &grace), Some(1));
        assert_eq!(match_rank("support", &grace), Some(2));
        assert_eq!(match_rank("hugo", &grace), None);
        assert_eq!(match_rank("  ", &grace), Some(1));
    }

    #[test]
    fn highlights_every_typed_word_case_insensitively() {
        assert_eq!(search_match_ranges("Aiko Tanaka", "an"), vec![6..8]);
        assert_eq!(
            search_match_ranges("New chat, new project", "NEW"),
            vec![0..3, 10..13]
        );
        assert!(search_match_ranges("Grace Lin", "  ").is_empty());
        assert!(search_match_ranges("Grace Lin", "hugo").is_empty());
    }

    #[test]
    fn steps_wrap_at_both_ends() {
        assert_eq!(menu_step(0, 3, -1), 2);
        assert_eq!(menu_step(2, 3, 1), 0);
        assert_eq!(menu_step(0, 0, 1), 0);
    }

    #[test]
    fn tints_are_stable_per_id() {
        let a = ComboOption::new("aiko", "Aiko Tanaka");
        assert_eq!(
            a.tint_index(),
            ComboOption::new("aiko", "Someone").tint_index()
        );
        assert_eq!(a.short_label(), "Aiko");
    }
}
