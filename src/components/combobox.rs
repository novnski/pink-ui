//! Combobox – pick one or many from a searchable, grouped list.
//!
//! A pink component. Picks sit in the field as chips
//! with a small avatar; the list below groups the options, lights up the
//! letters that match what you typed and offers to create the entry when
//! nothing fits. A plate glides to the active row, checks pop in as rows are
//! picked, and Backspace on an empty field first marks the last chip, then
//! removes it.
//!
//! Keys: type to filter, ↑/↓ to move, Enter to pick or unpick, Backspace on
//! an empty field to unpick the last chip, Escape to clear or close.
//!
//! ```ignore
//! Combobox::new("share", default_options())
//!     .selected(["design", "carla"])
//!     .creatable("Invite")
//!     .on_change(|ids, _, _| println!("{ids:?}"))
//! ```

use std::{ops::Range, rc::Rc, sync::Arc};

use gpui::{
    AnimationExt, AnyView, App, Bounds, Context, ElementId, Entity, Focusable, FontWeight,
    HighlightStyle, Hsla, IntoElement, MouseDownEvent, Pixels, ScrollHandle, SharedString,
    StyledText, Subscription, Transformation, Window, canvas, deferred, div, percentage,
    prelude::*, px, relative, svg,
};

use crate::{
    HoverFill,
    motion::{self, EASE, EASE_OUT_BACK, EASE_OUT_QUINT, MENU_IN, MotionSpec},
    pink::{self, ButtonKind, ControlSize, FillTier, Pink, oklch, radius, text},
    primitives::{InputEvent, TextInput, ring},
    story,
};

pub const NAME: &str = "Combobox";
pub const DESCRIPTION: &str = "Pick one or many from a searchable, grouped list – avatar chips in the field, matched letters lit, Backspace to unpick and new entries created as you type";

const ICON_CHEVRON: &str = "icons/combobox/chevron.svg";
const ICON_X: &str = "icons/combobox/x.svg";
const ICON_CHECK: &str = "icons/combobox/check.svg";
const ICON_PLUS: &str = "icons/combobox/plus.svg";
const ICON_MAIL: &str = "icons/combobox/mail.svg";
const ICON_USERS: &str = "icons/combobox/users.svg";
const ICON_LINK: &str = "icons/combobox/link.svg";
const ICON_GLOBE: &str = "icons/combobox/globe.svg";

const ROW_H: f32 = 34.;
const HEADER_H: f32 = 26.;
const LIST_PAD: f32 = 4.;
const LIST_MAX: f32 = 236.;
/// Where created entries are listed.
const CREATED: &str = "Invited";

type ChangeHandler = Rc<dyn Fn(&[SharedString], &mut Window, &mut App)>;

/// What an option shows in its avatar slot.
#[derive(Clone, Debug)]
pub enum Avatar {
    /// Initials on a soft tint picked from the label.
    Initials,
    /// An SVG path under `assets/`, on a rounded tile (teams, groups).
    Icon(SharedString),
}

#[derive(Clone, Debug)]
pub struct ComboOption {
    pub id: SharedString,
    pub label: SharedString,
    pub detail: Option<SharedString>,
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

    fn initials(&self) -> String {
        self.label
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase()
    }

    /// A stable hue from the label, for the avatar tint.
    fn hue(&self) -> f32 {
        let hash = self
            .label
            .bytes()
            .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
        (hash % 360) as f32
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

/// A visible row: an option (with matched ranges) or the create entry.
#[derive(Clone)]
enum Row {
    Option {
        option: ComboOption,
        highlights: Vec<Range<usize>>,
    },
    Create(SharedString),
}

/// The byte range where `query` appears in `label` (case-insensitive),
/// preferring a word start. `None` when it doesn't appear.
fn find(label: &str, query: &str) -> Option<Vec<Range<usize>>> {
    let q: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if q.is_empty() {
        return Some(Vec::new());
    }
    let chars: Vec<(usize, char)> = label.char_indices().collect();
    let lower: Vec<char> = chars
        .iter()
        .map(|(_, c)| c.to_lowercase().next().unwrap_or(*c))
        .collect();
    let end = |i: usize| chars.get(i).map_or(label.len(), |(b, _)| *b);
    // Prefer a run at a word start, then any run.
    let runs: Vec<usize> = (0..lower.len().saturating_sub(q.len() - 1))
        .filter(|&s| lower[s..s + q.len()] == q[..])
        .collect();
    let start = runs
        .iter()
        .find(|&&s| s == 0 || lower[s - 1] == ' ')
        .or(runs.first())?;
    Some(vec![chars[*start].0..end(start + q.len())])
}

/// The group a row is listed under (the create row stands alone).
fn row_group(row: &Row) -> Option<SharedString> {
    match row {
        Row::Option { option, .. } => option.group.clone(),
        Row::Create(_) => None,
    }
}

struct ComboState {
    input: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
    query: SharedString,
    open: bool,
    /// Bumped on every open so the list replays its entrance.
    opened: u32,
    active: usize,
    selected: Vec<SharedString>,
    created: Vec<ComboOption>,
    /// Backspace on an empty field marked the last chip.
    armed: bool,
    focused: bool,
    field: Bounds<Pixels>,
    scroll: ScrollHandle,
    options: Rc<Vec<ComboOption>>,
    multiple: bool,
    creatable: Option<SharedString>,
    on_change: Option<ChangeHandler>,
}

impl ComboState {
    fn all(&self) -> Vec<ComboOption> {
        self.options
            .iter()
            .chain(self.created.iter())
            .cloned()
            .collect()
    }

    fn find_option(&self, id: &SharedString) -> Option<ComboOption> {
        self.options
            .iter()
            .chain(self.created.iter())
            .find(|o| o.id == *id)
            .cloned()
    }

    fn rows(&self) -> Vec<Row> {
        let query = self.query.trim();
        let mut rows: Vec<Row> = self
            .all()
            .into_iter()
            .filter_map(|option| {
                let highlights = find(&option.label, query).or_else(|| {
                    let detail = option.detail.as_ref()?;
                    detail
                        .to_lowercase()
                        .contains(&query.to_lowercase())
                        .then(Vec::new)
                })?;
                Some(Row::Option { option, highlights })
            })
            .collect();
        // Keep groups together, in the order they first appear.
        let mut order: Vec<Option<SharedString>> = Vec::new();
        for row in &rows {
            if let Row::Option { option, .. } = row
                && !order.contains(&option.group)
            {
                order.push(option.group.clone());
            }
        }
        rows.sort_by_key(|row| match row {
            Row::Option { option, .. } => order.iter().position(|g| *g == option.group),
            Row::Create(_) => None,
        });
        let exact = self
            .all()
            .iter()
            .any(|o| o.label.eq_ignore_ascii_case(query));
        if self.creatable.is_some() && !query.is_empty() && !exact {
            rows.push(Row::Create(query.to_string().into()));
        }
        rows
    }

    fn set_open(&mut self, open: bool) {
        if open && !self.open {
            self.opened += 1;
            self.active = 0;
        }
        self.open = open;
    }

    fn on_input(&mut self, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change(text) => {
                if text.is_empty() && self.query.is_empty() {
                    // Backspace on an empty field: mark the last chip, then remove it.
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
                    self.set_open(true);
                    self.set_active(0);
                }
            }
            InputEvent::MoveDown | InputEvent::MoveUp => {
                self.armed = false;
                if !self.open {
                    self.set_open(true);
                } else {
                    let len = self.rows().len();
                    if len > 0 {
                        let next = if *event == InputEvent::MoveDown {
                            (self.active + 1) % len
                        } else {
                            (self.active + len - 1) % len
                        };
                        self.set_active(next);
                    }
                }
            }
            InputEvent::Submit(_) => {
                self.armed = false;
                if !self.open {
                    self.set_open(true);
                } else if let Some(row) = self.rows().get(self.active).cloned() {
                    self.pick(row, window, cx);
                }
            }
            InputEvent::Escape => {
                self.armed = false;
                if !self.query.is_empty() {
                    self.clear_query(cx);
                } else {
                    self.open = false;
                }
            }
            InputEvent::Focus => {
                self.focused = true;
                self.set_open(true);
            }
            InputEvent::Blur => {
                self.focused = false;
                self.armed = false;
            }
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
        if ix == 0 {
            // Show the first group's header too.
            self.scroll.set_offset(gpui::point(px(0.), px(0.)));
            return;
        }
        // Child 0 is the plate; then a header before each group's first row.
        let mut child = 1;
        let mut group: Option<Option<SharedString>> = None;
        for (i, row) in self.rows().iter().enumerate() {
            let g = row_group(row);
            if group.as_ref() != Some(&g) && g.is_some() {
                child += 1;
            }
            group = Some(g);
            if i == ix {
                self.scroll.scroll_to_item(child);
                return;
            }
            child += 1;
        }
    }

    fn pick(&mut self, row: Row, window: &mut Window, cx: &mut Context<Self>) {
        let id = match row {
            Row::Option { option, .. } => option.id,
            Row::Create(label) => {
                let id: SharedString = format!("new:{label}").into();
                let mut option = ComboOption::new(id.clone(), label)
                    .detail("Invite by email")
                    .group(CREATED);
                option.avatar = Avatar::Icon(ICON_MAIL.into());
                self.created.push(option);
                id
            }
        };
        self.toggle(&id, window, cx);
        if self.multiple {
            if !self.query.is_empty() {
                self.clear_query(cx);
            }
        } else {
            self.clear_query(cx);
            self.open = false;
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

    /// One pick at a time (the list closes once you pick). Multiple is the default.
    pub fn single(mut self) -> Self {
        self.multiple = false;
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
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
        let placeholder = self.placeholder.clone();
        let initial = self.selected.clone();
        let (default_open, autofocus) = (self.default_open, self.autofocus);

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
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
                ComboState {
                    input,
                    _subscriptions: vec![subscription],
                    query: SharedString::default(),
                    open: default_open,
                    opened: u32::from(default_open),
                    active: 0,
                    selected: initial,
                    created: Vec::new(),
                    armed: false,
                    focused: false,
                    field: Bounds::default(),
                    scroll: ScrollHandle::new(),
                    options: Rc::new(Vec::new()),
                    multiple: true,
                    creatable: None,
                    on_change: None,
                }
            },
        );

        let options = Rc::new(self.options);
        let input = state.update(cx, |s, _| {
            s.options = options.clone();
            s.multiple = self.multiple;
            s.creatable = self.creatable.clone();
            s.on_change = self.on_change.clone();
            s.input.clone()
        });
        input.update(cx, |input, _| {
            input.set_colors(Some(t.text_faint), Some(t.accent), Some(t.selection))
        });

        let s = state.read(cx);
        let rows = s.rows();
        let active = s.active.min(rows.len().saturating_sub(1));
        let chips: Vec<ComboOption> = s
            .selected
            .iter()
            .filter_map(|id| s.find_option(id))
            .collect();
        let selected = s.selected.clone();
        let (open, opened, armed, focused) = (s.open, s.opened, s.armed, s.focused);
        let scroll = s.scroll.clone();
        let query = s.query.clone();
        let multiple = self.multiple;
        let verb = self.creatable.clone().unwrap_or_else(|| "Create".into());

        let turn = motion::follow(
            &format!("{key}-chevron"),
            if open { 0.5 } else { 0. },
            MotionSpec::new(280, EASE_OUT_QUINT),
        );
        let ring_t = motion::follow(
            &format!("{key}-ring"),
            if focused { 1. } else { 0. },
            MotionSpec::new(160, EASE),
        );

        let chip_count = chips.len();
        let chip_elements = chips.into_iter().enumerate().map(|(ix, option)| {
            let marked = armed && ix + 1 == chip_count;
            chip(&key, &option, marked, &state, &t)
        });

        let opener = state.clone();
        let measurer = state.clone();
        let field = div()
            .id("field")
            .relative()
            .min_h(px(40.))
            .w_full()
            .pl(px(6.))
            .pr(px(8.))
            .py(px(7.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(px(radius::MD))
            .bg(t.input_bg)
            .border_1()
            .border_color(motion::mix(t.border_strong, t.accent, ring_t))
            .shadow(vec![ring(
                3. * ring_t,
                t.focus_ring().opacity(0.35 * ring_t),
            )])
            .cursor_text()
            .on_click(move |_, window, cx| {
                opener.update(cx, |s, cx| {
                    TextInput::focus(&s.input, window, cx);
                    s.set_open(true);
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
                    .children(chip_elements)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(96.))
                            .h(px(24.))
                            .pl(px(4.))
                            .flex()
                            .items_center()
                            .text_size(px(text::BODY))
                            .line_height(px(20.))
                            .text_color(t.text)
                            .child(input.clone()),
                    ),
            )
            .when(!selected.is_empty(), |el| {
                el.child(
                    div()
                        .flex_none()
                        .text_size(px(text::CAPTION))
                        .font_family(pink::MONO)
                        .text_color(t.text_faint)
                        .child(roll(&key, selected.len())),
                )
            })
            .child(
                svg()
                    .path(ICON_CHEVRON)
                    .size(px(16.))
                    .flex_none()
                    .text_color(t.text_faint)
                    .with_transformation(Transformation::rotate(percentage(turn))),
            )
            .child(
                canvas(
                    move |bounds, _, cx| measurer.update(cx, |s, _| s.field = bounds),
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );

        let popover = open.then(|| {
            let list = list(
                &key, &rows, active, &selected, &query, &verb, multiple, &state, &scroll, &t,
            );
            let closer = state.clone();
            let clearer = state.clone();
            let count = selected.len();
            let popover = div()
                .id("listbox")
                .absolute()
                .left_0()
                .right_0()
                .top(relative(1.))
                .occlude()
                .on_mouse_down_out(move |event: &MouseDownEvent, _, cx| {
                    closer.update(cx, |s, cx| {
                        if s.open && !s.field.contains(&event.position) {
                            s.open = false;
                            cx.notify();
                        }
                    })
                })
                .child(
                    pink::overlay(&t).overflow_hidden().child(list).child(
                        div()
                            .h(px(34.))
                            .px(px(12.))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .border_t_1()
                            .border_color(t.border)
                            .bg(t.fill(FillTier::Quaternary))
                            .text_size(px(text::FOOTNOTE))
                            .text_color(t.text_faint)
                            .child(format!("{count} selected"))
                            .when(count > 0 && multiple, |el| {
                                el.child(
                                    div()
                                        .id("clear-all")
                                        .px(px(6.))
                                        .h(px(20.))
                                        .flex()
                                        .items_center()
                                        .rounded(px(radius::XS))
                                        .cursor_pointer()
                                        .hover_fill_fg(
                                            format!("{key}-clear"),
                                            t.fill(FillTier::Tertiary).opacity(0.),
                                            t.fill(FillTier::Tertiary),
                                            t.text_muted,
                                            t.text,
                                        )
                                        .on_click(move |_, window, cx| {
                                            clearer.update(cx, |s, cx| {
                                                for id in s.selected.clone() {
                                                    s.toggle(&id, window, cx);
                                                }
                                            })
                                        })
                                        .child("Clear"),
                                )
                            })
                            .child(div().flex_1())
                            .child(hint(&["↑", "↓"], "move", &t))
                            .child(hint(&["↵"], if multiple { "toggle" } else { "pick" }, &t))
                            .when(multiple, |el| el.child(hint(&["⌫"], "unpick", &t))),
                    ),
                )
                .with_animation(
                    ElementId::NamedInteger("listbox-in".into(), u64::from(opened)),
                    MENU_IN.with_duration(220).animation(),
                    |el, k| el.opacity(k).mt(px(6. - 4. * (1. - k))),
                );
            deferred(popover).with_priority(1)
        });

        motion::keep_alive(window);
        div()
            .id(self.id)
            .relative()
            .w_full()
            .font_family(pink::SANS)
            .text_color(t.text)
            .child(field)
            .children(popover)
    }
}

fn hint(keys: &[&'static str], label: &'static str, t: &Pink) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .gap(px(2.))
                .children(keys.iter().map(|k| pink::kbd(*k, t))),
        )
        .child(label)
}

/// The pick count, rolling to each new value.
fn roll(key: &str, count: usize) -> impl IntoElement {
    div()
        .h(px(16.))
        .overflow_hidden()
        .child(div().child(count.to_string()).with_animation(
            SharedString::from(format!("{key}-count-{count}")),
            MotionSpec::new(280, EASE_OUT_QUINT).animation(),
            |el, k| el.relative().top(px(8. * (1. - k))).opacity(k),
        ))
}

/// The tint behind an option's initials, and the ink on it.
fn tint(option: &ComboOption, t: &Pink) -> (Hsla, Hsla) {
    let hue = option.hue();
    if t.dark {
        (oklch(0.36, 0.06, hue), oklch(0.88, 0.07, hue))
    } else {
        (oklch(0.93, 0.04, hue), oklch(0.45, 0.11, hue))
    }
}

fn avatar(option: &ComboOption, size: f32, t: &Pink) -> impl IntoElement {
    match &option.avatar {
        Avatar::Initials => {
            let (bg, ink) = tint(option, t);
            div()
                .size(px(size))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(bg)
                .text_size(px((size * 0.42).round()))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(ink)
                .child(option.initials())
        }
        Avatar::Icon(path) => div()
            .size(px(size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(if size > 18. { radius::SM } else { radius::XS }))
            .bg(t.fill(FillTier::Secondary))
            .child(
                svg()
                    .path(path.clone())
                    .size(px((size * 0.62).round()))
                    .text_color(t.text_muted),
            ),
    }
}

fn chip(
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
        MotionSpec::new(160, EASE),
    );
    let first = option
        .label
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string();
    let label = match option.avatar {
        Avatar::Initials if option.group.as_ref().map(|g| g.as_ref()) != Some(CREATED) => first,
        _ => option.label.to_string(),
    };
    div()
        .id(SharedString::from(format!("chip-{}", option.id)))
        .flex_none()
        .h(px(24.))
        .pl(px(3.))
        .pr(px(2.))
        .flex()
        .items_center()
        .gap(px(5.))
        .rounded(px(radius::SM))
        .border_1()
        .border_color(motion::mix(t.border, t.accent.opacity(0.6), mark))
        .bg(motion::mix(t.fill(FillTier::Tertiary), t.accent_wash, mark))
        .text_size(px(text::FOOTNOTE))
        .font_weight(FontWeight::MEDIUM)
        .child(avatar(option, 16., t))
        .child(div().max_w(px(140.)).truncate().child(label))
        .child(
            div()
                .id(SharedString::from(format!("unpick-{}", option.id)))
                .size(px(18.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(radius::XS))
                .cursor_pointer()
                .hover_fill_fg(
                    format!("{key}-unpick-{}", option.id),
                    t.fill(FillTier::Secondary).opacity(0.),
                    t.fill(FillTier::Secondary),
                    t.text_faint,
                    t.text,
                )
                .on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    remover.update(cx, |s, cx| s.toggle(&id, window, cx))
                })
                .child(
                    svg()
                        .path(ICON_X)
                        .size(px(12.))
                        .text_color(motion::hover_blend(
                            &format!("{key}-unpick-{}", option.id),
                            t.text_faint,
                            t.text,
                        )),
                ),
        )
        .with_animation(
            SharedString::from(format!("{key}-chip-in-{}", option.id)),
            MotionSpec::new(240, EASE_OUT_QUINT).animation(),
            |el, k| el.opacity(k).ml(px(-4. * (1. - k))),
        )
}

#[allow(clippy::too_many_arguments)]
fn list(
    key: &str,
    rows: &[Row],
    active: usize,
    selected: &[SharedString],
    query: &SharedString,
    verb: &SharedString,
    multiple: bool,
    state: &Entity<ComboState>,
    scroll: &ScrollHandle,
    t: &Pink,
) -> impl IntoElement {
    // Lay out the rows with their group headers, tracking the active row's top.
    let mut y = LIST_PAD;
    let mut active_top = LIST_PAD;
    let mut items: Vec<gpui::AnyElement> = Vec::new();
    let mut group: Option<Option<SharedString>> = None;
    for (ix, row) in rows.iter().enumerate() {
        let g = row_group(row);
        if group.as_ref() != Some(&g)
            && let Some(name) = g.clone()
        {
            items.push(
                div()
                    .h(px(HEADER_H))
                    .px(px(8.))
                    .pt(px(4.))
                    .flex()
                    .items_center()
                    .text_size(px(text::CAPTION))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text_faint)
                    .child(name)
                    .into_any_element(),
            );
            y += HEADER_H;
        }
        group = Some(g);
        if ix == active {
            active_top = y;
        }
        y += ROW_H;
        let picked = match row {
            Row::Option { option, .. } => selected.contains(&option.id),
            Row::Create(_) => false,
        };
        items.push(list_row(key, ix, row, picked, multiple, verb, state, t).into_any_element());
    }
    let content = if rows.is_empty() { 64. } else { y + LIST_PAD };
    let height = motion::follow(
        &format!("{key}-height"),
        content.min(LIST_MAX),
        MotionSpec::new(280, EASE_OUT_QUINT),
    );
    let plate_top = motion::follow(
        &format!("{key}-plate"),
        active_top,
        MotionSpec::new(180, EASE_OUT_QUINT),
    );

    let mut list = div()
        .id("options")
        .relative()
        .h(px(height))
        .p(px(LIST_PAD))
        .overflow_y_scroll()
        .track_scroll(scroll)
        .child(
            div()
                .absolute()
                .left(px(LIST_PAD))
                .right(px(LIST_PAD))
                .top(px(plate_top))
                .h(px(ROW_H))
                .rounded(px(radius::SM))
                .bg(t.fill(FillTier::Tertiary))
                .when(rows.is_empty(), |el| el.opacity(0.)),
        )
        .children(items);
    if rows.is_empty() {
        list = list.child(
            div()
                .h(px(56.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(text::FOOTNOTE))
                .text_color(t.text_faint)
                .child(format!("No one matches “{}”", query.trim())),
        );
    }
    list
}

#[allow(clippy::too_many_arguments)]
fn list_row(
    key: &str,
    ix: usize,
    row: &Row,
    picked: bool,
    multiple: bool,
    verb: &SharedString,
    state: &Entity<ComboState>,
    t: &Pink,
) -> gpui::AnyElement {
    let pointer = state.clone();
    let picker = state.clone();
    let pick_row = row.clone();
    let base = div()
        .id(("option", ix))
        .relative()
        .h(px(ROW_H))
        .px(px(8.))
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
            let row = pick_row.clone();
            picker.update(cx, |s, cx| {
                s.active = ix;
                s.pick(row, window, cx);
                TextInput::focus(&s.input, window, cx);
            })
        });

    match row {
        Row::Option { option, highlights } => {
            let label = StyledText::new(option.label.clone()).with_highlights(
                highlights.iter().map(|range| {
                    (
                        range.clone(),
                        HighlightStyle {
                            color: Some(t.accent),
                            font_weight: Some(FontWeight::SEMIBOLD),
                            ..Default::default()
                        },
                    )
                }),
            );
            let check = motion::follow(
                &format!("{key}-check-{}", option.id),
                if picked { 1. } else { 0. },
                MotionSpec::new(260, EASE_OUT_BACK),
            );
            base.child(avatar(option, 22., t))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .items_baseline()
                        .gap(px(8.))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .child(div().flex_none().text_size(px(text::BODY)).child(label))
                        .when_some(option.detail.clone(), |el, detail| {
                            el.child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_faint)
                                    .child(detail),
                            )
                        }),
                )
                .child(
                    div()
                        .size(px(18.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(multiple, |el| {
                            el.rounded(px(radius::XS))
                                .border_1()
                                .border_color(motion::mix(
                                    t.border_strong,
                                    t.accent_strong,
                                    check.min(1.),
                                ))
                                .bg(motion::mix(
                                    t.input_bg.opacity(0.),
                                    t.accent_strong,
                                    check.min(1.),
                                ))
                        })
                        .child(
                            svg()
                                .path(ICON_CHECK)
                                .size(px(12. * check.max(0.)))
                                .text_color(if multiple { t.on_accent } else { t.accent }),
                        ),
                )
                .into_any_element()
        }
        Row::Create(label) => base
            .child(
                div()
                    .size(px(22.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(radius::SM))
                    .border_1()
                    .border_color(t.border_strong)
                    .border_dashed()
                    .child(svg().path(ICON_PLUS).size(px(14.)).text_color(t.text_muted)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(text::BODY))
                    .text_color(t.text_muted)
                    .child(format!("{verb} “{label}”")),
            )
            .child(
                div()
                    .text_size(px(text::FOOTNOTE))
                    .text_color(t.text_faint)
                    .child(if label.contains('@') {
                        "by email"
                    } else {
                        "as new"
                    }),
            )
            .with_animation(
                "create-in",
                MotionSpec::new(240, EASE_OUT_QUINT).animation(),
                |el, k| el.opacity(k).top(px(3. * (1. - k))),
            )
            .into_any_element(),
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        div()
            .h(px(430.))
            .w(px(480.))
            .flex()
            .flex_col()
            .font_family(pink::SANS)
            .child(
                pink::card(&t)
                    .w_full()
                    .rounded(px(radius::XL))
                    .p(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(14.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(text::TITLE))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.text)
                                    .child("Share “Q4 launch plan”"),
                            )
                            .child(
                                div()
                                    .text_size(px(text::BODY))
                                    .text_color(t.text_muted)
                                    .child("People you add can view, comment and suggest edits."),
                            ),
                    )
                    .child(
                        Combobox::new("combobox", default_options())
                            .selected(["design", "carla"])
                            .placeholder("Add people or teams…")
                            .creatable("Invite")
                            .default_open(true)
                            .autofocus(true),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                svg()
                                    .path(ICON_GLOBE)
                                    .size(px(14.))
                                    .text_color(t.text_faint),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(text::FOOTNOTE))
                                    .text_color(t.text_faint)
                                    .child("Anyone at Northwind with the link can view"),
                            )
                            .child(
                                pink::button("cb-copy", ButtonKind::Ghost, ControlSize::Md, &t)
                                    .child(
                                        pink::glyph(ICON_LINK, ControlSize::Md)
                                            .text_color(t.text_muted),
                                    )
                                    .child("Copy link"),
                            )
                            .child(
                                pink::button("cb-share", ButtonKind::Primary, ControlSize::Md, &t)
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
    fn finds_substrings_preferring_word_starts() {
        assert_eq!(find("Grace Lin", "gr"), Some(vec![0..2]));
        assert_eq!(find("Hugo Laurent", "gr"), None);
        assert_eq!(find("Ana Tanaka", "ta"), Some(vec![4..6]));
        assert_eq!(find("Aiko Tanaka", "an"), Some(vec![6..8]));
        assert_eq!(find("Anyone", ""), Some(vec![]));
    }

    #[test]
    fn initials_and_hue_are_stable() {
        let option = ComboOption::new("aiko", "Aiko Tanaka");
        assert_eq!(option.initials(), "AT");
        assert_eq!(option.hue(), ComboOption::new("x", "Aiko Tanaka").hue());
    }
}
