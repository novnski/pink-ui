//! Filter Menu – the Board toolbar's Filters pop-up (Pink
//! `shell/board/filters.rs` `render_filters_select` + `page.rs`
//! `render_menu_card`).
//!
//! A Secondary pop-up button reads "Filters", or "Filters · N" while N
//! filters are set, and drops a 240 pt menu card below itself. The card lists
//! each group under an uppercase heading after a full-bleed hairline: a
//! single-choice group starts with its "Any …" row, a multiple group lists
//! independent switches, and the current choices carry a trailing check.
//! "Clear all filters" closes the list, dimmed while nothing is set.
//!
//! The pointer and the arrow keys share one highlight. ↑ and ↓ move it
//! (wrapping, skipping dimmed rows), Enter or Space applies the row under
//! it, Esc closes; every pick closes the menu, the way Pink's Board does. The
//! card drops 6 pt into place over 180 ms and fades out over 120 ms.
//!
//! ```ignore
//! FilterMenu::new(
//!     "board-filters",
//!     vec![
//!         FilterGroup::new("source", "Source")
//!             .option("chat", "From a chat")
//!             .option("github", "GitHub issue"),
//!         FilterGroup::new("archive", "Archive")
//!             .multiple()
//!             .option("archived", "Show archived tickets"),
//!     ],
//! )
//! .on_change(|value, _, _| println!("{value:?}"))
//! ```

use std::{collections::BTreeMap, rc::Rc, sync::Arc};
use web_time::Instant;

use gpui::{
    Anchor, AnyView, App, ClickEvent, Context, DispatchPhase, ElementId, Entity, FocusHandle,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Role, SharedString, Task, Window,
    anchored, canvas, deferred, div, prelude::*, px, svg,
};

use crate::{
    motion::{self, MENU_OUT, MENU_RISE_DOWN},
    pink::{self, ButtonKind, ControlSize, Pink, TextStyle, TypeStyleExt, icon_size, layout},
    story,
};

pub const NAME: &str = "Filter Menu";
pub const DESCRIPTION: &str = "A Filters pop-up button with an active count that drops a menu of headed filter groups, Any rows, trailing checks and Clear all filters, driven by pointer or keyboard";

const ICON_FILTER: &str = "icons/filter-menu/filter.svg";
const ICON_ATTENTION: &str = "icons/filter-menu/danger-triangle.svg";

/// The Board menu card's height budget before it scrolls (Pink
/// `render_menu_card` `max_h 420`).
const MENU_MAX_H: f32 = 420.0;

/// Picked option ids per group id. A single-choice group holds at most one;
/// a group that is absent (or empty) reads as "Any".
pub type FilterValue = BTreeMap<SharedString, Vec<SharedString>>;

type ChangeHandler = Rc<dyn Fn(&FilterValue, &mut Window, &mut App)>;

#[derive(Clone, Debug)]
pub struct FilterOption {
    pub id: SharedString,
    pub label: SharedString,
    /// A 14 pt leading glyph (Pink's provider marks), tinted
    /// `readable_muted(0.9)`.
    pub glyph: Option<SharedString>,
}

impl FilterOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            glyph: None,
        }
    }

    pub fn glyph(mut self, path: impl Into<SharedString>) -> Self {
        self.glyph = Some(path.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct FilterGroup {
    pub id: SharedString,
    /// The group's menu heading ("Source").
    pub label: SharedString,
    pub options: Vec<FilterOption>,
    /// Independent switches (Pink's Attention and Archive rows) instead of
    /// one choice with an "Any …" row (Pink's Provider and Source).
    pub multiple: bool,
    /// The single-choice group's first row; "Any <label>" when unset.
    pub any_label: Option<SharedString>,
    /// Whether this group adds to the trigger's "Filters · N" (Pink leaves
    /// out Attention, which has its own toolbar chip).
    pub counted: bool,
}

impl FilterGroup {
    /// A single-choice group: an "Any …" row, then its options.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options: Vec::new(),
            multiple: false,
            any_label: None,
            counted: true,
        }
    }

    /// Appends one option.
    pub fn option(mut self, id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        self.options.push(FilterOption::new(id, label));
        self
    }

    pub fn options(mut self, options: Vec<FilterOption>) -> Self {
        self.options = options;
        self
    }

    /// Each option switches on and off by itself; no "Any …" row.
    pub fn multiple(mut self) -> Self {
        self.multiple = true;
        self
    }

    pub fn any_label(mut self, label: impl Into<SharedString>) -> Self {
        self.any_label = Some(label.into());
        self
    }

    pub fn counted(mut self, counted: bool) -> Self {
        self.counted = counted;
        self
    }

    fn any_row_label(&self) -> SharedString {
        self.any_label
            .clone()
            .unwrap_or_else(|| format!("Any {}", self.label.to_lowercase()).into())
    }
}

/// What a menu row does (Pink `MenuAction`), by group and option index.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Any(usize),
    Pick(usize, usize),
    Clear,
}

/// One row of the menu (Pink `MenuEntry`).
#[derive(Clone, Debug)]
struct Entry {
    key: SharedString,
    label: SharedString,
    glyph: Option<SharedString>,
    checked: bool,
    enabled: bool,
    heading: Option<SharedString>,
    action: Action,
}

fn picked<'a>(value: &'a FilterValue, group: &str) -> &'a [SharedString] {
    value.get(group).map_or(&[], |list| list.as_slice())
}

/// The rows, top to bottom (Pink `filter_menu_entries`): per group a heading
/// on its first row, the "Any …" row for single-choice groups, the options,
/// and "Clear all filters" last.
fn entries(groups: &[FilterGroup], value: &FilterValue) -> Vec<Entry> {
    let mut rows = Vec::new();
    for (g, group) in groups.iter().enumerate() {
        let chosen = picked(value, &group.id);
        let mut heading = Some(group.label.clone());
        if !group.multiple {
            rows.push(Entry {
                key: format!("{}-any", group.id).into(),
                label: group.any_row_label(),
                glyph: None,
                checked: chosen.is_empty(),
                enabled: true,
                heading: heading.take(),
                action: Action::Any(g),
            });
        }
        for (o, option) in group.options.iter().enumerate() {
            rows.push(Entry {
                key: format!("{}-{}", group.id, option.id).into(),
                label: option.label.clone(),
                glyph: option.glyph.clone(),
                checked: chosen.contains(&option.id),
                enabled: true,
                heading: heading.take(),
                action: Action::Pick(g, o),
            });
        }
    }
    rows.push(Entry {
        key: "clear".into(),
        label: "Clear all filters".into(),
        glyph: None,
        checked: false,
        enabled: any_filter(value),
        heading: None,
        action: Action::Clear,
    });
    rows
}

/// The value after a row runs (Pink `run_menu_action`).
fn apply(groups: &[FilterGroup], action: Action, mut value: FilterValue) -> FilterValue {
    match action {
        Action::Clear => FilterValue::new(),
        Action::Any(g) => {
            value.remove(&groups[g].id);
            value
        }
        Action::Pick(g, o) => {
            let group = &groups[g];
            let option = group.options[o].id.clone();
            if group.multiple {
                let list = value.entry(group.id.clone()).or_default();
                match list.iter().position(|id| *id == option) {
                    Some(ix) => {
                        list.remove(ix);
                    }
                    None => list.push(option),
                }
                let empty = list.is_empty();
                if empty {
                    value.remove(&group.id);
                }
            } else {
                value.insert(group.id.clone(), vec![option]);
            }
            value
        }
    }
}

/// The trigger's N (Pink `active_filter_count`): one per set single-choice
/// group, one per switched-on option, counted groups only.
fn active_count(groups: &[FilterGroup], value: &FilterValue) -> usize {
    groups
        .iter()
        .filter(|g| g.counted)
        .map(|g| {
            let chosen = picked(value, &g.id);
            if g.multiple {
                chosen.len()
            } else {
                usize::from(!chosen.is_empty())
            }
        })
        .sum()
}

/// Whether "Clear all filters" has anything to clear (Pink `any_filter`).
fn any_filter(value: &FilterValue) -> bool {
    value.values().any(|list| !list.is_empty())
}

/// Steps the cursor, wrapping at both ends; `None` enters at the edge
/// matching the direction (Pink `popover::menu_step`).
fn menu_step(active: Option<usize>, count: usize, delta: isize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let count = count as isize;
    let next = match active {
        None if delta >= 0 => 0,
        None => count - 1,
        Some(at) => (at as isize + delta).rem_euclid(count),
    };
    Some(next as usize)
}

/// [`menu_step`] that skips rows `enabled` rejects (Pink
/// `popover::menu_step_where`).
fn menu_step_where(
    active: Option<usize>,
    count: usize,
    delta: isize,
    enabled: impl Fn(usize) -> bool,
) -> Option<usize> {
    let mut at = active;
    for _ in 0..count {
        let next = menu_step(at, count, delta)?;
        if enabled(next) {
            return Some(next);
        }
        at = Some(next);
    }
    None
}

/// The menu's lifecycle (Pink `Popup` + the Board page's `menu_cursor`).
struct MenuState {
    picked: FilterValue,
    open: bool,
    /// When the exit fade began; the card stays mounted until it ends.
    closing: Option<Instant>,
    /// The trigger press found the menu mounted, so its click leaves it
    /// closed (Pink `note_trigger_press`).
    pressed_while_open: bool,
    /// The one highlight that the arrows and the pointer share.
    cursor: Option<usize>,
    /// Focus the card on the next frame (Pink `pending_menu_focus`).
    pending_focus: bool,
    trigger: FocusHandle,
    menu: FocusHandle,
    /// Unmounts the card once the exit fade has run (Pink `reap_popup`).
    reap: Option<Task<()>>,
}

impl MenuState {
    fn mounted(&self) -> bool {
        self.open || self.closing.is_some()
    }

    fn open_menu(&mut self, cursor: Option<usize>) {
        self.open = true;
        self.closing = None;
        self.reap = None;
        self.cursor = cursor;
        self.pending_focus = true;
    }

    /// Starts the exit; `false` when the menu wasn't open.
    fn begin_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.open {
            return false;
        }
        self.open = false;
        self.cursor = None;
        self.pending_focus = false;
        let since = Instant::now();
        self.closing = Some(since);
        let exit = MENU_OUT.total().mul_f32(motion::speed_scale());
        self.reap = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(exit).await;
            this.update(cx, |s, cx| {
                if s.closing == Some(since) {
                    s.closing = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        true
    }
}

/// Closes the menu; `refocus` hands focus back to the trigger (a pick or
/// Esc), a press elsewhere leaves it where the press put it.
fn close(state: &Entity<MenuState>, refocus: bool, window: &mut Window, cx: &mut App) {
    let trigger = state.update(cx, |s, cx| {
        let closed = s.begin_close(cx);
        cx.notify();
        closed.then(|| s.trigger.clone())
    });
    if refocus && let Some(handle) = trigger {
        window.focus(&handle, cx);
    }
}

#[derive(IntoElement)]
pub struct FilterMenu {
    id: ElementId,
    groups: Vec<FilterGroup>,
    value: Option<FilterValue>,
    default_value: FilterValue,
    default_open: bool,
    title: SharedString,
    on_change: Option<ChangeHandler>,
}

impl FilterMenu {
    pub fn new(id: impl Into<ElementId>, groups: Vec<FilterGroup>) -> Self {
        Self {
            id: id.into(),
            groups,
            value: None,
            default_value: FilterValue::new(),
            default_open: false,
            title: "Filters".into(),
            on_change: None,
        }
    }

    /// Controlled value: the menu shows this and reports changes through
    /// [`on_change`](Self::on_change).
    pub fn value(mut self, value: FilterValue) -> Self {
        self.value = Some(value);
        self
    }

    /// Starting value when uncontrolled.
    pub fn default_value(mut self, value: FilterValue) -> Self {
        self.default_value = value;
        self
    }

    /// Mount with the menu already open (and focused).
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// The trigger's label (default "Filters").
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&FilterValue, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }
}

type Run = Rc<dyn Fn(Action, &mut Window, &mut App)>;

impl RenderOnce for FilterMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        let groups = Rc::new(self.groups);

        let state = {
            let default_value = self.default_value.clone();
            let default_open = self.default_open;
            window.use_keyed_state(
                ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
                cx,
                move |_, cx| MenuState {
                    picked: default_value,
                    open: default_open,
                    closing: None,
                    pressed_while_open: false,
                    cursor: None,
                    pending_focus: default_open,
                    trigger: cx.focus_handle().tab_stop(true),
                    menu: cx.focus_handle(),
                    reap: None,
                },
            )
        };

        // Pink `render`: focus the card once it has opened.
        let focus_menu = state.update(cx, |s, _| {
            let due = s.pending_focus && s.open;
            if due {
                s.pending_focus = false;
            }
            due.then(|| s.menu.clone())
        });
        if let Some(handle) = focus_menu {
            window.focus(&handle, cx);
        }

        let (value, open, closing, cursor, trigger_focus, menu_focus) = {
            let s = state.read(cx);
            (
                self.value.clone().unwrap_or_else(|| s.picked.clone()),
                s.open,
                s.closing,
                s.cursor,
                s.trigger.clone(),
                s.menu.clone(),
            )
        };

        // Changes go through here: kept inside when uncontrolled, always reported.
        let controlled = self.value.clone();
        let run: Run = {
            let state = state.clone();
            let groups = groups.clone();
            let on_change = self.on_change.clone();
            Rc::new(move |action, window, cx| {
                // Pink `run_menu_action`: close first, then apply.
                close(&state, true, window, cx);
                let current = controlled
                    .clone()
                    .unwrap_or_else(|| state.read(cx).picked.clone());
                let next = apply(&groups, action, current);
                if controlled.is_none() {
                    state.update(cx, |s, cx| {
                        s.picked = next.clone();
                        cx.notify();
                    });
                }
                if let Some(f) = &on_change {
                    f(&next, window, cx);
                }
            })
        };

        let rows = Rc::new(entries(&groups, &value));
        let first_enabled = rows.iter().position(|e| e.enabled);

        // ---------- Trigger (Pink `render_filters_select`) ----------
        let active = active_count(&groups, &value);
        let label: SharedString = if active == 0 {
            self.title.clone()
        } else {
            format!("{} · {active}", self.title).into()
        };
        let aria: SharedString = if active == 0 {
            self.title.clone()
        } else {
            format!("{}: {active} active", self.title).into()
        };
        let mut trigger =
            pink::select_frame(format!("{key}-trigger"), ControlSize::Md, open, true, &t)
                .track_focus(&trigger_focus)
                .aria_label(aria)
                .relative()
                .font_family(pink::SANS)
                .on_mouse_down(MouseButton::Left, {
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, _| s.pressed_while_open = s.mounted());
                    }
                })
                .on_click({
                    let state = state.clone();
                    move |event: &ClickEvent, _, cx| {
                        let was_open =
                            state.update(cx, |s, _| std::mem::take(&mut s.pressed_while_open));
                        if was_open {
                            return;
                        }
                        // Pink `open_menu`: the keyboard lands on the first live
                        // row, the pointer on none.
                        let cursor = if event.is_keyboard() {
                            first_enabled
                        } else {
                            None
                        };
                        state.update(cx, |s, cx| {
                            s.open_menu(cursor);
                            cx.notify();
                        });
                    }
                })
                .child(pink::glyph(ICON_FILTER, ControlSize::Md))
                .child(div().flex_none().child(label))
                .child(pink::select_chevron(&t));

        if open || closing.is_some() {
            let card = menu_card(&key, &t, &rows, cursor, &state, &menu_focus, run);
            // Reduced motion drops the fade; the card still unmounts on the
            // exit's clock, so a press on the trigger keeps it closed.
            let reduced = motion::reduced_motion(cx);
            let exit = closing.map(|since| {
                if reduced {
                    1.0
                } else {
                    motion::menu_exit_progress(since)
                }
            });
            trigger = trigger.child(anchored_menu_below(
                format!("{key}-menu").into(),
                exit,
                card,
            ));
        }

        motion::keep_alive(window);
        trigger
    }
}

/// The menu card: headed groups of rows with one shared keyboard/pointer
/// highlight (Pink `page.rs` `render_menu_card`).
fn menu_card(
    key: &str,
    t: &Pink,
    rows: &Rc<Vec<Entry>>,
    cursor: Option<usize>,
    state: &Entity<MenuState>,
    focus: &FocusHandle,
    run: Run,
) -> impl IntoElement {
    let glyph_tone = t.readable_muted(0.9);
    let mut children = Vec::new();
    for (ix, entry) in rows.iter().enumerate() {
        if let Some(heading) = &entry.heading {
            if ix > 0 {
                children.push(pink::menu_separator(t).into_any_element());
            }
            children.push(pink::menu_heading(heading, t).into_any_element());
        }
        let enabled = entry.enabled;
        let action = entry.action;
        let run = run.clone();
        children.push(
            pink::menu_row_nav(entry.checked, cursor == Some(ix), t)
                .id(SharedString::from(format!("{key}-row-{}", entry.key)))
                .role(Role::MenuItem)
                .aria_label(entry.label.clone())
                .when(!enabled, |row| row.opacity(0.45).cursor_default())
                .on_mouse_move({
                    let state = state.clone();
                    move |_, _, cx| {
                        state.update(cx, |s, cx| {
                            if s.open && s.cursor != Some(ix) {
                                s.cursor = Some(ix);
                                cx.notify();
                            }
                        })
                    }
                })
                .when(enabled, |row| {
                    row.on_click(move |_, window, cx| run(action, window, cx))
                })
                .when_some(entry.glyph.clone(), |row, glyph| {
                    row.child(
                        svg()
                            .path(glyph)
                            .size(px(icon_size::SM))
                            .flex_none()
                            .text_color(glyph_tone),
                    )
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(entry.label.clone()),
                )
                .into_any_element(),
        );
    }

    // Pink `on_menu_key`: Up/Down/Enter/Space/Escape inside the open menu;
    // Tab closes it and moves on.
    let on_key = {
        let state = state.clone();
        let rows = rows.clone();
        move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
            if !state.read(cx).open {
                return;
            }
            let handled = match event.keystroke.key.as_str() {
                "up" | "down" => {
                    let delta = if event.keystroke.key == "up" { -1 } else { 1 };
                    state.update(cx, |s, cx| {
                        s.cursor =
                            menu_step_where(s.cursor, rows.len(), delta, |ix| rows[ix].enabled);
                        cx.notify();
                    });
                    true
                }
                "enter" | "space" => {
                    let cursor = state.read(cx).cursor;
                    if let Some(entry) = cursor.and_then(|ix| rows.get(ix))
                        && entry.enabled
                    {
                        run(entry.action, window, cx);
                    }
                    true
                }
                "escape" => {
                    close(&state, true, window, cx);
                    true
                }
                "tab" => {
                    close(&state, false, window, cx);
                    false
                }
                _ => false,
            };
            if handled {
                cx.stop_propagation();
            }
        }
    };

    pink::popover_card(t)
        .id(SharedString::from(format!("{key}-card")))
        .role(Role::Menu)
        .track_focus(focus)
        .w(px(layout::MENU_W_S))
        .max_h(px(MENU_MAX_H))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .on_mouse_down_out({
            let state = state.clone();
            move |_, window, cx| close(&state, false, window, cx)
        })
        .on_key_down(on_key)
        .children(children)
}

/// A dropdown hung from the trigger's bottom-left corner, 6 pt below it,
/// kept 8 pt inside the window (Pink `popover::anchored_menu_below`). It
/// drops 6 pt into place on [`motion::MENU_IN`]; while `exit` runs it fades
/// on [`MENU_OUT`] under an occluder so the dying rows take no clicks
/// (Pink `menu_motion_from`).
fn anchored_menu_below(
    id: SharedString,
    exit: Option<f32>,
    card: impl IntoElement,
) -> impl IntoElement {
    let inner = div()
        .occlude()
        .pt(px(layout::POPOVER_GAP))
        .child(div().relative().child(press_guard()).child(card));
    let layer = match exit {
        Some(t) => motion::menu_out(
            SharedString::from(format!("{id}-out")),
            t,
            inner.relative().child(div().absolute().inset_0().occlude()),
        )
        .into_any_element(),
        None => motion::menu_in_from(id, MENU_RISE_DOWN, inner).into_any_element(),
    };
    div().absolute().bottom_0().left_0().size_0().child(
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                .child(layer),
        )
        .priority(1),
    )
}

/// Swallows the press that dismisses the menu: the card closes on it during
/// capture, and this stops it in the bubble phase so nothing behind the menu
/// (the trigger included) acts on it too (Pink `frosted_menu_rounded`'s
/// guard).
fn press_guard() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        |bounds, _, window, _| {
            window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && !bounds.contains(&event.position) {
                    cx.stop_propagation();
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

/// The Board's filters (Pink `filter_menu_entries`): Attention, Source and
/// Archive.
pub fn demo_groups() -> Vec<FilterGroup> {
    vec![
        FilterGroup::new("attention", "Attention")
            .multiple()
            .counted(false)
            .option("needs-attention", "Needs attention only"),
        FilterGroup::new("source", "Source")
            .option("local", "Written manually")
            .option("chat", "From a chat")
            .option("authoring", "Discuss with AI")
            .option("github", "GitHub issue"),
        FilterGroup::new("archive", "Archive")
            .multiple()
            .option("archived", "Show archived tickets"),
    ]
}

/// The demo's tickets: (source, needs attention, archived).
const TICKETS: [(&str, bool, bool); 12] = [
    ("local", false, false),
    ("chat", true, false),
    ("github", false, false),
    ("chat", false, false),
    ("authoring", false, false),
    ("github", true, false),
    ("local", false, true),
    ("chat", true, false),
    ("github", false, true),
    ("local", true, false),
    ("chat", false, true),
    ("github", false, false),
];

fn matching(value: &FilterValue) -> usize {
    let attention = !picked(value, "attention").is_empty();
    let source = picked(value, "source").first();
    let archived = !picked(value, "archive").is_empty();
    TICKETS
        .iter()
        .filter(|(s, a, arch)| {
            (!attention || *a) && source.is_none_or(|x| x.as_ref() == *s) && (archived || !arch)
        })
        .count()
}

/// The Board header and toolbar around the Filters pop-up (Pink
/// `render_header` / `render_toolbar`): the "Needs attention" chip drives
/// the same Attention filter, and nothing matching shows Pink's
/// `render_no_matches`.
pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |window, cx| {
        let value = window.use_keyed_state("filter-menu-demo", cx, |_, _| {
            FilterValue::from([
                ("attention".into(), vec!["needs-attention".into()]),
                ("source".into(), vec!["chat".into()]),
            ])
        });
        let t = Pink::get(cx);
        let current = value.read(cx).clone();
        let count = matching(&current);
        let attention = !picked(&current, "attention").is_empty();
        let set = {
            let value = value.clone();
            Rc::new(move |next: FilterValue, cx: &mut App| {
                value.update(cx, |v, cx| {
                    *v = next;
                    cx.notify();
                })
            })
        };

        let header = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .gap(px(10.))
                    .child(
                        div()
                            .type_style(TextStyle::Title1)
                            .text_color(t.text)
                            .child("Board"),
                    )
                    .child(
                        div()
                            .text_size(px(13.))
                            .text_color(t.readable_muted(0.7))
                            .child(count.to_string()),
                    ),
            )
            .child(subtitle(
                "Move a ticket to Ready to start it; move it back to Backlog to stop it. \
                 Done is set when checks and the review pass.",
                &t,
            ));

        let chip = pink::chip("filter-menu-attention", attention, ControlSize::Md, &t)
            .aria_label("Needs attention only")
            .gap(px(6.))
            .child(pink::glyph(ICON_ATTENTION, ControlSize::Md))
            .child("Needs attention")
            .on_click({
                let set = set.clone();
                let current = current.clone();
                move |_, _, cx| {
                    let mut next = current.clone();
                    if attention {
                        next.remove("attention");
                    } else {
                        next.insert("attention".into(), vec!["needs-attention".into()]);
                    }
                    set(next, cx);
                }
            });
        let menu = FilterMenu::new("filter-menu", demo_groups())
            .value(current)
            .default_open(true)
            .on_change({
                let set = set.clone();
                move |next, _, cx| set(next.clone(), cx)
            });
        let toolbar = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .child(chip)
            .child(menu);
        let no_matches = (count == 0).then(|| {
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(8.))
                .py(px(24.))
                .child(subtitle("No tickets match these filters.", &t))
                .child(
                    pink::button(
                        "filter-menu-clear",
                        ButtonKind::Secondary,
                        ControlSize::Md,
                        &t,
                    )
                    .aria_label("Clear filters")
                    .child("Clear filters")
                    .on_click(move |_, _, cx| set(FilterValue::new(), cx)),
                )
        });

        div()
            .w(px(560.))
            .h(px(480.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .font_family(pink::SANS)
            .child(header)
            .child(toolbar)
            .children(no_matches)
    })
}

/// Pink `widgets::page_subtitle`: body copy in the secondary tone.
fn subtitle(copy: &'static str, t: &Pink) -> gpui::Div {
    div()
        .mt(px(6.))
        .max_w(px(600.))
        .text_size(px(13.))
        .line_height(px(19.))
        .text_color(t.readable_muted(0.9))
        .child(copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(pairs: &[(&str, &[&str])]) -> FilterValue {
        pairs
            .iter()
            .map(|(g, ids)| {
                (
                    SharedString::from(g.to_string()),
                    ids.iter()
                        .map(|id| SharedString::from(id.to_string()))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn rows_follow_pinks_board_menu() {
        let groups = demo_groups();
        let rows = entries(&groups, &FilterValue::new());
        let labels: Vec<&str> = rows.iter().map(|r| r.label.as_ref()).collect();
        assert_eq!(
            labels,
            [
                "Needs attention only",
                "Any source",
                "Written manually",
                "From a chat",
                "Discuss with AI",
                "GitHub issue",
                "Show archived tickets",
                "Clear all filters",
            ]
        );
        let headings: Vec<Option<&str>> = rows.iter().map(|r| r.heading.as_deref()).collect();
        assert_eq!(headings[0], Some("Attention"));
        assert_eq!(headings[1], Some("Source"));
        assert_eq!(headings[2], None);
        assert_eq!(headings[6], Some("Archive"));
        assert_eq!(headings[7], None);
        // Nothing set: Any is checked and Clear all is dimmed.
        assert!(rows[1].checked);
        assert!(!rows[7].enabled);
    }

    #[test]
    fn picks_set_switch_and_clear() {
        let groups = demo_groups();
        let v = apply(&groups, Action::Pick(1, 1), FilterValue::new());
        assert_eq!(v, value(&[("source", &["chat"])]));
        let v = apply(&groups, Action::Pick(1, 3), v);
        assert_eq!(v, value(&[("source", &["github"])]));
        let v = apply(&groups, Action::Pick(2, 0), v);
        let v = apply(&groups, Action::Pick(0, 0), v);
        assert_eq!(active_count(&groups, &v), 2, "Attention isn't counted");
        let v = apply(&groups, Action::Pick(2, 0), v);
        assert!(!v.contains_key("archive"));
        let v = apply(&groups, Action::Any(1), v);
        assert_eq!(v, value(&[("attention", &["needs-attention"])]));
        assert!(entries(&groups, &v).last().unwrap().enabled);
        assert!(apply(&groups, Action::Clear, v).is_empty());
    }

    #[test]
    fn cursor_wraps_and_skips_dimmed_rows() {
        assert_eq!(menu_step(None, 3, 1), Some(0));
        assert_eq!(menu_step(None, 3, -1), Some(2));
        assert_eq!(menu_step(Some(2), 3, 1), Some(0));
        assert_eq!(menu_step(None, 0, 1), None);
        let enabled = [true, false, true];
        assert_eq!(menu_step_where(Some(0), 3, 1, |ix| enabled[ix]), Some(2));
        assert_eq!(menu_step_where(Some(2), 3, -1, |ix| enabled[ix]), Some(0));
        assert_eq!(menu_step_where(None, 3, 1, |_| false), None);
    }

    /// Hosts a controlled menu, opened on mount, and records its changes.
    #[derive(Default)]
    struct Host {
        value: FilterValue,
        changes: usize,
    }

    impl gpui::Render for Host {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let host = cx.entity().downgrade();
            div().size_full().child(
                FilterMenu::new("fm", demo_groups())
                    .value(self.value.clone())
                    .default_open(true)
                    .on_change(move |next, _, cx| {
                        host.update(cx, |h, cx| {
                            h.value = next.clone();
                            h.changes += 1;
                            cx.notify();
                        })
                        .ok();
                    }),
            )
        }
    }

    #[gpui::test]
    fn keyboard_drives_the_menu(cx: &mut gpui::TestAppContext) {
        use gpui::{KeyUpEvent, Keystroke};
        cx.update(crate::init);
        let (host, cx) = cx.add_window_view(|_, _| Host::default());
        let settle = |cx: &mut gpui::VisualTestContext| {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        };
        // Enter on the focused trigger: GPUI clicks on the key coming back up.
        let press_trigger = |cx: &mut gpui::VisualTestContext| {
            cx.simulate_keystrokes("enter");
            cx.simulate_event(KeyUpEvent {
                keystroke: Keystroke::parse("enter").unwrap(),
            });
            settle(cx);
        };
        settle(cx);

        // Open on mount and focused: the pointer cursor is unset, so Down
        // enters at the top; the third row is "Written manually".
        cx.simulate_keystrokes("down down down enter");
        settle(cx);
        host.read_with(cx, |h, _| {
            assert_eq!(h.changes, 1);
            assert_eq!(h.value, value(&[("source", &["local"])]));
        });

        // The pick closed the menu and handed focus back to the trigger,
        // whose keyboard click opens it on the first row.
        press_trigger(cx);
        cx.simulate_keystrokes("enter");
        settle(cx);
        host.read_with(cx, |h, _| {
            assert_eq!(h.changes, 2);
            assert_eq!(
                h.value,
                value(&[("attention", &["needs-attention"]), ("source", &["local"])])
            );
        });

        // Esc closes without a change; arrows on the trigger do nothing.
        press_trigger(cx);
        cx.simulate_keystrokes("escape");
        settle(cx);
        cx.simulate_keystrokes("down enter");
        settle(cx);
        host.read_with(cx, |h, _| assert_eq!(h.changes, 2));

        // Up from the first row wraps to "Clear all filters".
        press_trigger(cx);
        cx.simulate_keystrokes("up enter");
        settle(cx);
        host.read_with(cx, |h, _| {
            assert_eq!(h.changes, 3);
            assert!(h.value.is_empty());
        });
    }

    #[gpui::test]
    fn pointer_picks_and_dismisses(cx: &mut gpui::TestAppContext) {
        use gpui::{Modifiers, point};
        cx.update(crate::init);
        let (host, cx) = cx.add_window_view(|_, _| Host::default());
        let settle = |cx: &mut gpui::VisualTestContext| {
            cx.executor()
                .advance_clock(std::time::Duration::from_millis(200));
            cx.update(|window, _| window.refresh());
            cx.run_until_parked();
        };
        let click = |cx: &mut gpui::VisualTestContext, x: f32, y: f32| {
            cx.simulate_mouse_move(point(px(x), px(y)), None, Modifiers::none());
            settle(cx);
            cx.simulate_click(point(px(x), px(y)), Modifiers::none());
            settle(cx);
        };
        // The trigger sits at the window's corner; the card hangs 6 pt below
        // its 28 pt, and its first row follows the Attention heading.
        let (trigger, row) = ((20.0, 14.0), (60.0, 34.0 + 1.0 + 4.0 + 26.0 + 14.0));
        settle(cx);

        click(cx, row.0, row.1);
        host.read_with(cx, |h, _| {
            assert_eq!(h.changes, 1);
            assert_eq!(h.value, value(&[("attention", &["needs-attention"])]));
        });

        // Open, then press the trigger again: the press only dismisses.
        click(cx, trigger.0, trigger.1);
        click(cx, trigger.0, trigger.1);
        // Nothing is under the row's spot any more.
        click(cx, row.0, row.1);
        host.read_with(cx, |h, _| assert_eq!(h.changes, 1));

        // Reopened, the same row switches Attention back off.
        click(cx, trigger.0, trigger.1);
        click(cx, row.0, row.1);
        host.read_with(cx, |h, _| {
            assert_eq!(h.changes, 2);
            assert!(h.value.is_empty());
        });
    }

    #[test]
    fn demo_count_follows_the_filters() {
        assert_eq!(matching(&FilterValue::new()), 9);
        assert_eq!(
            matching(&value(&[
                ("attention", &["needs-attention"]),
                ("source", &["chat"])
            ])),
            2
        );
        assert_eq!(
            matching(&value(&[
                ("attention", &["needs-attention"]),
                ("source", &["authoring"])
            ])),
            0
        );
    }
}
