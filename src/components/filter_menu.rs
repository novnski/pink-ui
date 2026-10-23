//! Filter Menu – grouped filters with removable chips, collapsible checkbox
//! groups, Clear all and full keyboard control.
//!
//! Active filters sit on top as removable chips; below, collapsible groups
//! list their options as checkboxes with a count of what's picked. Clean: one
//! flat card with hairline shadows, the chips on one line ("+N" for the rest)
//! and the keyboard hints as its last row. Classic: a white card on a gray
//! tray, chips that wrap, and the hints on the tray.
//!
//! Arrow keys move through groups and options, Enter or Space picks, ← and →
//! fold a group, Home and End jump to the ends. Heights glide when groups open
//! or chips come and go, chevrons turn and checks draw themselves in.
//!
//! ```ignore
//! FilterMenu::new(
//!     "filters",
//!     vec![FilterGroup::new("stage", "Stage").option("new", "New lead").option("won", "Won")],
//! )
//! .default_value(FilterValue::from([("stage".into(), vec!["won".into()])]))
//! .on_change(|value, _, _| println!("{value:?}"))
//! ```

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    f32::consts::FRAC_PI_2,
    rc::Rc,
    sync::Arc,
};

use gpui::{
    AnyView, App, BoxShadow, Div, ElementId, FocusHandle, FontWeight, Hsla, IntoElement,
    KeyDownEvent, MouseButton, Pixels, SharedString, TextRun, Transformation, Window, canvas, div,
    font, prelude::*, px, radians, svg,
};

use crate::{
    ActiveTheme, HoverFill, Look,
    motion::{self, AnimationExt, EASE, EASE_OUT_QUINT, MotionSpec},
    primitives::{SANS, black_a, hex, hexa, ring, shadow, white_a},
    story,
};

pub const NAME: &str = "Filter Menu";
pub const DESCRIPTION: &str = "Grouped filters with removable chips, collapsible checkbox groups, Clear all and full keyboard control";

/// Picked option ids per group id.
pub type FilterValue = BTreeMap<SharedString, Vec<SharedString>>;

type ChangeHandler = Rc<dyn Fn(&FilterValue, &mut Window, &mut App)>;

/// Gap between chips on the Clean line, in px – matches the CSS.
const CHIP_GAP: f32 = 4.;

/// `grid-template-rows` 0fr ↔ 1fr and the padding: `320ms` springy ease-out.
const PANEL: MotionSpec = MotionSpec::new(320, EASE_OUT_QUINT);
/// The options fading in and out: `opacity 220ms ease`.
const PANEL_FADE: MotionSpec = MotionSpec::new(220, EASE);
/// The chevron turning: `transform 260ms`.
const CHEVRON: MotionSpec = MotionSpec::new(260, EASE_OUT_QUINT);
/// `ui-*-chip-in`: `260ms` (scale 0.9 → 1 becomes a fade).
const CHIP_IN: MotionSpec = MotionSpec::new(260, EASE_OUT_QUINT);
/// `ui-fmc-roll`: a count rolling up into place, `300ms`.
const ROLL: MotionSpec = MotionSpec::new(300, EASE_OUT_QUINT);

#[derive(Clone, Debug)]
pub struct FilterOption {
    pub id: SharedString,
    pub label: SharedString,
}

impl FilterOption {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FilterGroup {
    pub id: SharedString,
    pub label: SharedString,
    pub options: Vec<FilterOption>,
    /// Put in front of the option on its chip, e.g. "Score: " → "Score: 80–90".
    pub chip_prefix: Option<SharedString>,
}

impl FilterGroup {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            options: Vec::new(),
            chip_prefix: None,
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

    pub fn chip_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
        self.chip_prefix = Some(prefix.into());
        self
    }
}

/// The `--ui-fmc-*` / `--ui-fm-*` tokens, by role.
struct Palette {
    tray: Option<Hsla>,
    card: Hsla,
    card_border: Option<Hsla>,
    card_shadow: Vec<BoxShadow>,
    /// Clean's hairlines between the sections.
    line: Hsla,
    title: Hsla,
    ink: Hsla,
    clear: Hsla,
    clear_hover_bg: Hsla,
    clear_off: Hsla,
    chip: Hsla,
    chip_fg: Hsla,
    chip_x: Hsla,
    chip_x_hover_bg: Hsla,
    more: Hsla,
    row_hover: Hsla,
    chevron: Hsla,
    count: Hsla,
    picked: Hsla,
    option: Hsla,
    box_bg: Hsla,
    box_edge: Hsla,
    box_on: Hsla,
    box_edge_on: Hsla,
    /// Clean lifts the empty box with `0 1px 1px rgba(0, 0, 0, .04)`.
    box_lift: Option<Hsla>,
    check: Hsla,
    hint: Hsla,
    kbd: Hsla,
    kbd_fg: Hsla,
    kbd_shadow: Vec<BoxShadow>,
    focus: Hsla,
}

impl Palette {
    fn get(look: Look, cx: &App) -> Self {
        match (look, cx.is_dark()) {
            (Look::Clean, false) => {
                let shadow = vec![
                    ring(0.5, black_a(0.06)),
                    shadow(0., 1., 1., -1., black_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.05)),
                ];
                let accent = hexa(0x0060e2, 0.765);
                Palette {
                    tray: None,
                    card: hex(0xffffff),
                    card_border: None,
                    card_shadow: shadow.clone(),
                    line: black_a(0.06),
                    title: hex(0x000000),
                    ink: black_a(0.88),
                    clear: black_a(0.61),
                    clear_hover_bg: black_a(0.03),
                    clear_off: black_a(0.27),
                    chip: black_a(0.06),
                    chip_fg: black_a(0.88),
                    chip_x: black_a(0.447),
                    chip_x_hover_bg: black_a(0.06),
                    more: black_a(0.61),
                    row_hover: black_a(0.03),
                    chevron: black_a(0.447),
                    count: black_a(0.27),
                    picked: black_a(0.45),
                    option: black_a(0.88),
                    box_bg: hex(0xffffff),
                    box_edge: black_a(0.22),
                    box_on: accent,
                    box_edge_on: accent.opacity(0.),
                    box_lift: Some(black_a(0.04)),
                    check: hex(0xffffff),
                    hint: black_a(0.45),
                    kbd: hex(0xffffff),
                    kbd_fg: black_a(0.61),
                    kbd_shadow: shadow,
                    focus: hexa(0x0060e2, 0.35),
                }
            }
            (Look::Clean, true) => {
                let shadow = vec![
                    ring(0.5, white_a(0.1)),
                    shadow(0., 1., 2., 0., black_a(0.4)),
                ];
                let accent = hex(0x4d8dff);
                Palette {
                    tray: None,
                    card: hex(0x1c1c1e),
                    card_border: None,
                    card_shadow: shadow.clone(),
                    line: white_a(0.08),
                    title: hex(0xffffff),
                    ink: white_a(0.9),
                    clear: white_a(0.62),
                    clear_hover_bg: white_a(0.04),
                    clear_off: white_a(0.3),
                    chip: white_a(0.08),
                    chip_fg: white_a(0.9),
                    chip_x: white_a(0.5),
                    chip_x_hover_bg: white_a(0.08),
                    more: white_a(0.62),
                    row_hover: white_a(0.04),
                    chevron: white_a(0.5),
                    count: white_a(0.3),
                    picked: white_a(0.46),
                    option: white_a(0.9),
                    box_bg: hex(0x1c1c1e),
                    box_edge: white_a(0.28),
                    box_on: accent,
                    box_edge_on: accent.opacity(0.),
                    box_lift: Some(black_a(0.04)),
                    check: hex(0xffffff),
                    hint: white_a(0.46),
                    kbd: hex(0x1c1c1e),
                    kbd_fg: white_a(0.62),
                    kbd_shadow: shadow,
                    focus: hexa(0x4d8dff, 0.5),
                }
            }
            (Look::Classic, false) => Palette {
                tray: Some(hex(0xf0f0f0)),
                card: hex(0xffffff),
                card_border: Some(hex(0xe5e5e5)),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.06))],
                line: hex(0xe5e5e5),
                title: hex(0x171717),
                ink: hex(0x171717),
                clear: hex(0x8f8f8f),
                clear_hover_bg: hex(0xf7f7f7),
                clear_off: hexa(0x8f8f8f, 0.45),
                chip: hex(0xf3f3f3),
                chip_fg: hex(0x404040),
                chip_x: hex(0x8f8f8f),
                chip_x_hover_bg: hex(0xebebeb),
                more: hex(0x404040),
                row_hover: hex(0xf7f7f7),
                chevron: hex(0x404040),
                count: hex(0x8f8f8f),
                picked: hex(0x8f8f8f),
                option: hex(0x404040),
                box_bg: hex(0xf0f0f0),
                box_edge: hex(0xe3e3e3),
                box_on: hex(0x171717),
                box_edge_on: hex(0x171717),
                box_lift: None,
                check: hex(0xffffff),
                hint: hex(0x404040),
                kbd: hex(0xffffff),
                kbd_fg: hex(0x404040),
                kbd_shadow: vec![
                    ring(1., hex(0xe5e5e5)),
                    shadow(0., 1., 1., 0., black_a(0.04)),
                ],
                focus: hex(0x2f6bff),
            },
            (Look::Classic, true) => Palette {
                tray: Some(hex(0x161618)),
                card: hex(0x232326),
                card_border: Some(white_a(0.08)),
                card_shadow: vec![shadow(0., 1., 2., 0., black_a(0.4))],
                line: white_a(0.08),
                title: hex(0xededed),
                ink: hex(0xededed),
                clear: hex(0x8f8f8f),
                clear_hover_bg: white_a(0.05),
                clear_off: hexa(0x8f8f8f, 0.45),
                chip: hex(0x2a2a2d),
                chip_fg: hex(0xc4c4c4),
                chip_x: hex(0x8f8f8f),
                chip_x_hover_bg: hex(0x313135),
                more: hex(0xc4c4c4),
                row_hover: white_a(0.05),
                chevron: hex(0xc4c4c4),
                count: hex(0x8f8f8f),
                picked: hex(0x8f8f8f),
                option: hex(0xc4c4c4),
                box_bg: hex(0x2a2a2d),
                box_edge: white_a(0.1),
                box_on: hex(0xededed),
                box_edge_on: hex(0xededed),
                box_lift: None,
                check: hex(0x121213),
                hint: hex(0xc4c4c4),
                kbd: hex(0x232326),
                kbd_fg: hex(0xc4c4c4),
                kbd_shadow: vec![
                    ring(1., white_a(0.08)),
                    shadow(0., 1., 1., 0., black_a(0.04)),
                ],
                focus: hex(0x6b95ff),
            },
        }
    }
}

/// Icon paths for a look: Clean draws on a 16 grid with thinner strokes.
fn icon(look: Look, name: &str) -> SharedString {
    match look {
        Look::Clean => format!("icons/filter-menu/clean/{name}.svg").into(),
        Look::Classic => format!("icons/filter-menu/{name}.svg").into(),
    }
}

/// The check's polyline in its viewBox, the dash length and the box it's drawn in.
struct CheckGeometry {
    points: [(f32, f32); 3],
    dash: f32,
    cap: f32,
    view_box: f32,
    size: f32,
}

impl CheckGeometry {
    fn get(look: Look) -> Self {
        match look {
            Look::Clean => CheckGeometry {
                points: [(4.25, 8.25), (6.75, 10.75), (11.75, 5.25)],
                dash: 12.,
                cap: 0.8,
                view_box: 16.,
                size: 12.,
            },
            Look::Classic => CheckGeometry {
                points: [(6., 12.5), (10., 16.5), (18., 7.5)],
                dash: 20.,
                cap: 1.1,
                view_box: 24.,
                size: 14.,
            },
        }
    }

    /// How far the stroke reaches to the right (in px of the icon) once `t`
    /// of the dash is drawn. The check runs left to right, so revealing the
    /// icon up to here stands in for the `stroke-dashoffset` draw-in.
    fn reach(&self, t: f32) -> f32 {
        let mut left = self.dash * t;
        if left <= 0. {
            return 0.;
        }
        let mut x = self.points[0].0;
        for w in self.points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let seg = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            if left <= seg {
                x = a.0 + (b.0 - a.0) * left / seg;
                break;
            }
            left -= seg;
            x = b.0;
        }
        ((x + self.cap) / self.view_box).min(1.) * self.size
    }
}

/// UI state the web version keeps in `useState`.
struct FilterState {
    picked: FilterValue,
    open: HashSet<SharedString>,
    /// One focus handle per row the arrow keys can reach, by row key.
    handles: HashMap<SharedString, FocusHandle>,
    /// The group list itself, so it can take the keys before a row has focus.
    list: FocusHandle,
    /// Focus came from the keyboard: show the focus ring (`:focus-visible`).
    keyboard: bool,
    /// The menu's width from the last frame, to fit the chips.
    width: Option<Pixels>,
}

/// A row the arrow keys can reach: a group head or one of its options.
#[derive(Clone)]
struct Row {
    key: SharedString,
    group: SharedString,
}

fn group_key(group: &str) -> SharedString {
    format!("g:{group}").into()
}

fn option_key(group: &str, option: &str) -> SharedString {
    format!("o:{group}:{option}").into()
}

fn is_on(picked: &FilterValue, group: &str, option: &str) -> bool {
    picked
        .get(group)
        .is_some_and(|list| list.iter().any(|o| o == option))
}

/// Width of one line of Inter at `size` and `weight`.
fn text_width(window: &Window, text: &str, size: f32, weight: FontWeight) -> f32 {
    let mut f = font(SANS);
    f.weight = weight;
    let run = TextRun {
        len: text.len(),
        font: f,
        color: Hsla::default(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.to_string().into(), px(size), &[run], None);
    f32::from(line.width)
}

#[derive(IntoElement)]
pub struct FilterMenu {
    id: ElementId,
    groups: Vec<FilterGroup>,
    value: Option<FilterValue>,
    default_value: FilterValue,
    default_open: Option<Vec<SharedString>>,
    title: SharedString,
    hints: bool,
    on_change: Option<ChangeHandler>,
    look: Option<Look>,
}

impl FilterMenu {
    pub fn new(id: impl Into<ElementId>, groups: Vec<FilterGroup>) -> Self {
        Self {
            id: id.into(),
            groups,
            value: None,
            default_value: FilterValue::new(),
            default_open: None,
            title: "Filters".into(),
            hints: true,
            on_change: None,
            look: None,
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

    /// Groups that start open – by default the first one.
    pub fn default_open(mut self, groups: Vec<SharedString>) -> Self {
        self.default_open = Some(groups);
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Show the keyboard hints (default on).
    pub fn hints(mut self, hints: bool) -> Self {
        self.hints = hints;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&FilterValue, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Overrides the global Clean/Classic look for this menu.
    pub fn look(mut self, look: Look) -> Self {
        self.look = Some(look);
        self
    }
}

/// A chip on the active line.
struct Chip {
    group: SharedString,
    option: SharedString,
    label: SharedString,
}

type Toggle = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type ToggleGroup = Rc<dyn Fn(&SharedString, &mut App)>;

impl RenderOnce for FilterMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let look = self.look.unwrap_or(cx.look());
        let clean = look == Look::Clean;
        let p = Palette::get(look, cx);
        let key = self.id.to_string();
        let groups = Rc::new(self.groups);

        let state = {
            let default_value = self.default_value.clone();
            let default_open: HashSet<SharedString> = match &self.default_open {
                Some(open) => open.iter().cloned().collect(),
                None => groups.iter().take(1).map(|g| g.id.clone()).collect(),
            };
            window.use_keyed_state(
                ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
                cx,
                move |_, cx| FilterState {
                    picked: default_value,
                    open: default_open,
                    handles: HashMap::new(),
                    list: cx.focus_handle(),
                    keyboard: false,
                    width: None,
                },
            )
        };

        // One focus handle per group head and option, kept across frames.
        state.update(cx, |s, cx| {
            for g in groups.iter() {
                s.handles
                    .entry(group_key(&g.id))
                    .or_insert_with(|| cx.focus_handle());
                for o in &g.options {
                    s.handles
                        .entry(option_key(&g.id, &o.id))
                        .or_insert_with(|| cx.focus_handle());
                }
            }
        });

        let (picked, open, handles, list_handle, keyboard, width) = {
            let s = state.read(cx);
            (
                self.value.clone().unwrap_or_else(|| s.picked.clone()),
                s.open.clone(),
                s.handles.clone(),
                s.list.clone(),
                s.keyboard,
                s.width,
            )
        };
        let width = width.map_or(if clean { 400. } else { 420. }, f32::from);

        // Changes go through here: kept inside when uncontrolled, always reported.
        let controlled = self.value.clone();
        let commit: Rc<dyn Fn(FilterValue, &mut Window, &mut App)> = {
            let state = state.clone();
            let is_controlled = controlled.is_some();
            let on_change = self.on_change.clone();
            Rc::new(move |next, window, cx| {
                if !is_controlled {
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
        let toggle: Toggle = {
            let state = state.clone();
            let commit = commit.clone();
            Rc::new(move |group, option, window, cx| {
                let mut next = controlled
                    .clone()
                    .unwrap_or_else(|| state.read(cx).picked.clone());
                let list = next.entry(group.clone()).or_default();
                match list.iter().position(|o| o == option) {
                    Some(ix) => {
                        list.remove(ix);
                    }
                    None => list.push(option.clone()),
                }
                commit(next, window, cx);
            })
        };
        let toggle_group: ToggleGroup = {
            let state = state.clone();
            Rc::new(move |group, cx| {
                state.update(cx, |s, cx| {
                    if !s.open.remove(group) {
                        s.open.insert(group.clone());
                    }
                    cx.notify();
                })
            })
        };

        // Chips in the order of the groups and their options
        let chips: Vec<Chip> = groups
            .iter()
            .flat_map(|g| {
                let picked = &picked;
                g.options
                    .iter()
                    .filter(move |o| is_on(picked, &g.id, &o.id))
                    .map(move |o| Chip {
                        group: g.id.clone(),
                        option: o.id.clone(),
                        label: format!("{}{}", g.chip_prefix.clone().unwrap_or_default(), o.label)
                            .into(),
                    })
            })
            .collect();

        // Everything the arrow keys can reach, top to bottom
        let rows: Rc<Vec<Row>> = Rc::new(
            groups
                .iter()
                .flat_map(|g| {
                    let head = Row {
                        key: group_key(&g.id),
                        group: g.id.clone(),
                    };
                    let options = open
                        .contains(&g.id)
                        .then(|| {
                            g.options.iter().map(|o| Row {
                                key: option_key(&g.id, &o.id),
                                group: g.id.clone(),
                            })
                        })
                        .into_iter()
                        .flatten();
                    std::iter::once(head).chain(options)
                })
                .collect(),
        );

        let on_key = {
            let rows = rows.clone();
            let handles = handles.clone();
            let state = state.clone();
            let toggle_group = toggle_group.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let len = rows.len();
                if len == 0 {
                    return;
                }
                let current = rows
                    .iter()
                    .position(|r| handles.get(&r.key).is_some_and(|h| h.is_focused(window)));
                let focus_row = |key: &SharedString, window: &mut Window, cx: &mut App| {
                    if let Some(handle) = handles.get(key) {
                        window.focus(handle, cx);
                    }
                };
                match event.keystroke.key.as_str() {
                    "down" | "up" => {
                        let next = match current {
                            Some(i) if event.keystroke.key == "down" => (i + 1).min(len - 1),
                            Some(i) => i.saturating_sub(1),
                            None => 0,
                        };
                        focus_row(&rows[next].key, window, cx);
                    }
                    "home" => focus_row(&rows[0].key, window, cx),
                    "end" => focus_row(&rows[len - 1].key, window, cx),
                    "right" | "left" => {
                        let Some(i) = current else { return };
                        let group = rows[i].group.clone();
                        let is_open = state.read(cx).open.contains(&group);
                        if event.keystroke.key == "right" {
                            if !is_open {
                                toggle_group(&group, cx);
                            }
                        } else {
                            if is_open {
                                toggle_group(&group, cx);
                            }
                            focus_row(&group_key(&group), window, cx);
                        }
                    }
                    // GPUI clicks the focused row when Enter or Space comes
                    // back up, which picks the option or folds the group
                    "enter" | "space" if current.is_some() => {}
                    _ => return,
                }
                state.update(cx, |s, cx| {
                    s.keyboard = true;
                    cx.notify();
                });
                cx.stop_propagation();
            }
        };

        let ctx = Ctx {
            key: key.clone(),
            look,
            clean,
            keyboard,
        };

        // ---------- Header ----------
        let enabled = !chips.is_empty();
        let clear_key = format!("{key}-clear");
        let on = motion::follow(
            &format!("{clear_key}-on"),
            if enabled { 1. } else { 0. },
            MotionSpec::new(if clean { 160 } else { 200 }, EASE),
        );
        let hover = if enabled {
            motion::hover_t(&clear_key)
        } else {
            0.
        };
        let clear_fg = motion::mix(motion::mix(p.clear_off, p.clear, on), p.ink, hover);
        let clear = div()
            .id("clear")
            .flex()
            .items_center()
            .text_size(px(13.))
            .text_color(clear_fg)
            .bg(motion::mix(
                p.clear_hover_bg.opacity(0.),
                p.clear_hover_bg,
                hover,
            ))
            .on_hover(motion::hover_listener(clear_key))
            .when(clean, |d| {
                d.h(px(28.))
                    .px(px(8.))
                    .rounded(px(6.))
                    .font_weight(FontWeight::MEDIUM)
            })
            .when(!clean, |d| {
                d.py(px(4.)).px(px(6.)).mr(px(-6.)).rounded(px(8.))
            })
            .when(enabled, |d| {
                let commit = commit.clone();
                d.cursor_pointer()
                    .on_click(move |_, window, cx| commit(FilterValue::new(), window, cx))
            })
            .child("Clear all");
        let title = div()
            .text_size(px(15.))
            .text_color(p.title)
            .when(clean, |d| {
                d.line_height(px(24.)).font_weight(FontWeight::MEDIUM)
            })
            .when(!clean, |d| d.font_weight(FontWeight::SEMIBOLD))
            .child(self.title.clone());
        let head = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .when(clean, |d| d.pt(px(12.)).pr(px(10.)).pb(px(8.)).pl(px(16.)))
            .when(!clean, |d| d.px(px(2.)))
            .child(title)
            .child(clear);

        // ---------- Chips ----------
        let chips_row = chip_line(&ctx, &p, window, &chips, width, toggle.clone());

        // ---------- Groups ----------
        let sections: Vec<_> = groups
            .iter()
            .map(|g| {
                group_section(
                    &ctx,
                    &p,
                    g,
                    &picked,
                    open.contains(&g.id),
                    &handles,
                    window,
                    toggle.clone(),
                    toggle_group.clone(),
                )
            })
            .collect();
        let list = div()
            .id("groups")
            .track_focus(&list_handle)
            .flex()
            .flex_col()
            .when(clean, |d| {
                d.p(px(4.)).border_t(px(0.5)).border_color(p.line)
            })
            .when(!clean, |d| d.pt(px(6.)))
            .on_key_down(on_key)
            .children(sections);

        // Records the menu's width for the next frame's chip fitting.
        let measure = {
            let state = state.clone();
            canvas(
                move |bounds, window, cx| {
                    let w = bounds.size.width;
                    let changed = state.update(cx, |s, _| {
                        let changed = s
                            .width
                            .is_none_or(|old| (f32::from(old) - f32::from(w)).abs() > 0.5);
                        if changed {
                            s.width = Some(w);
                        }
                        changed
                    });
                    if changed {
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };

        let on_pointer = {
            let state = state.clone();
            move |_: &gpui::MouseDownEvent, _: &mut Window, cx: &mut App| {
                if state.read(cx).keyboard {
                    state.update(cx, |s, cx| {
                        s.keyboard = false;
                        cx.notify();
                    });
                }
            }
        };

        let hints = self.hints.then(|| hint_row(&ctx, &p));
        let root = div()
            .id(self.id.clone())
            .relative()
            .w_full()
            .font_family(SANS)
            .text_size(px(13.))
            .text_color(p.ink)
            .on_mouse_down(MouseButton::Left, on_pointer)
            .child(measure);
        let root = match look {
            Look::Clean => root
                .max_w(px(400.))
                .overflow_hidden()
                .rounded(px(10.))
                .bg(p.card)
                .shadow(p.card_shadow.clone())
                .line_height(px(20.))
                .child(head)
                .child(chips_row)
                .child(list)
                .children(hints),
            Look::Classic => root
                .max_w(px(420.))
                .p(px(6.))
                .rounded(px(24.))
                .when_some(p.tray, |d, tray| d.bg(tray))
                // The site's body line height, which the classic CSS inherits
                .line_height(px(18.571))
                .child(
                    div()
                        .pt(px(16.))
                        .px(px(16.))
                        .pb(px(10.))
                        .rounded(px(20.))
                        .bg(p.card)
                        .shadow(p.card_shadow.clone())
                        .when_some(p.card_border, |d, c| d.border_1().border_color(c))
                        .child(head)
                        .child(chips_row)
                        .child(list),
                )
                .children(hints),
        };
        motion::keep_alive(window);
        root
    }
}

/// What the pieces below need to know about the menu.
struct Ctx {
    key: String,
    look: Look,
    clean: bool,
    keyboard: bool,
}

/// The active chips. Clean keeps them on one line and folds the rest into
/// "+N"; Classic wraps them. The box glides to its new height.
fn chip_line(
    ctx: &Ctx,
    p: &Palette,
    window: &Window,
    chips: &[Chip],
    width: f32,
    toggle: Toggle,
) -> impl IntoElement {
    let clean = ctx.clean;
    let (fit, target) = if clean {
        // As many chips as fit, leaving room for a "+N" if any are left over
        let widths: Vec<f32> = chips
            .iter()
            .map(|c| {
                8. + text_width(window, &c.label, 12., FontWeight::MEDIUM).min(180.)
                    + 2.
                    + 18.
                    + 3.
                    + CHIP_GAP
            })
            .collect();
        let more = 16. + text_width(window, "+99", 12., FontWeight::MEDIUM) + CHIP_GAP;
        let room = width - 32. + CHIP_GAP;
        let mut used = 0.;
        let mut n = 0;
        while n < widths.len() {
            let rest = if n + 1 < widths.len() { more } else { 0. };
            if used + widths[n] + rest > room {
                break;
            }
            used += widths[n];
            n += 1;
        }
        (n, if chips.is_empty() { 0. } else { 36. })
    } else {
        // Lines the wrapping chips take inside the card
        let room = width - 12. - 2. - 32.;
        let mut lines = 0;
        let mut x = 0.;
        for c in chips {
            let w = 11. + text_width(window, &c.label, 13., FontWeight::NORMAL) + 2. + 22. + 4.;
            if lines == 0 || x + 6. + w > room {
                lines += 1;
                x = w;
            } else {
                x += 6. + w;
            }
        }
        let height = if lines == 0 {
            0.
        } else {
            12. + lines as f32 * 30. + (lines - 1) as f32 * 6.
        };
        (chips.len(), height)
    };
    let height = motion::follow(&format!("{}-chips-h", ctx.key), target, PANEL);

    let shown = chips[..fit].iter().map(|c| {
        let toggle = toggle.clone();
        let (group, option) = (c.group.clone(), c.option.clone());
        let chip_key = format!("{}-chip-{}:{}", ctx.key, c.group, c.option);
        chip(ctx, p, &chip_key, c.label.clone(), move |window, cx| {
            toggle(&group, &option, window, cx)
        })
        .with_animation(
            SharedString::from(chip_key.clone()),
            CHIP_IN.animation(),
            |el, t| el.opacity(t),
        )
    });
    let hidden = &chips[fit..];
    let more = (!hidden.is_empty()).then(|| {
        let count = hidden.len();
        div()
            .flex()
            .flex_none()
            .items_center()
            .h(px(24.))
            .px(px(8.))
            .rounded(px(6.))
            .overflow_hidden()
            .bg(p.chip)
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.more)
            .child(roll(
                SharedString::from(format!("{}-more-{count}", ctx.key)),
                format!("+{count}"),
            ))
    });

    let inner = div()
        .flex()
        .gap(px(if clean { CHIP_GAP } else { 6. }))
        .when(clean, |d| {
            d.flex_nowrap()
                .overflow_hidden()
                .px(px(16.))
                .when(!chips.is_empty(), |d| d.pb(px(12.)))
        })
        .when(!clean, |d| {
            d.flex_wrap().when(!chips.is_empty(), |d| d.pt(px(12.)))
        })
        .children(shown)
        .children(more);
    div()
        .flex_none()
        .h(px(height))
        .overflow_hidden()
        .child(inner)
}

/// One removable chip.
fn chip(
    ctx: &Ctx,
    p: &Palette,
    chip_key: &str,
    label: SharedString,
    on_remove: impl Fn(&mut Window, &mut App) + 'static,
) -> Div {
    let clean = ctx.clean;
    let x_key = format!("{chip_key}-x");
    let x = div()
        .id("x")
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(if clean { 18. } else { 22. }))
        .rounded(px(if clean { 4. } else { 6. }))
        .cursor_pointer()
        .hover_fill(
            x_key.clone(),
            p.chip_x_hover_bg.opacity(0.),
            p.chip_x_hover_bg,
        )
        .on_click(move |_, window, cx| on_remove(window, cx))
        .child(
            svg()
                .path(icon(ctx.look, "x"))
                .size(px(if clean { 12. } else { 14. }))
                .flex_none()
                .text_color(motion::hover_blend(&x_key, p.chip_x, p.ink)),
        );
    let base = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(2.))
        .bg(p.chip)
        .text_color(p.chip_fg)
        .whitespace_nowrap();
    if clean {
        base.h(px(24.))
            .pl(px(8.))
            .pr(px(3.))
            .rounded(px(6.))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(FontWeight::MEDIUM)
            .child(div().max_w(px(180.)).truncate().child(label))
            .child(x)
    } else {
        base.h(px(30.))
            .pl(px(11.))
            .pr(px(4.))
            .rounded(px(10.))
            .text_size(px(13.))
            .child(label)
            .child(x)
    }
}

/// Text that rolls up into place whenever `id` changes (`ui-fmc-roll`).
fn roll(id: SharedString, text: String) -> impl IntoElement {
    div()
        .child(text)
        .with_animation(id, ROLL.animation(), |el, t| {
            el.relative().top(px(16. * 0.6 * (1. - t))).opacity(t)
        })
}

/// A group: its head, and the options that fold out beneath it.
#[allow(clippy::too_many_arguments)]
fn group_section(
    ctx: &Ctx,
    p: &Palette,
    g: &FilterGroup,
    picked: &FilterValue,
    is_open: bool,
    handles: &HashMap<SharedString, FocusHandle>,
    window: &Window,
    toggle: Toggle,
    toggle_group: ToggleGroup,
) -> impl IntoElement {
    let clean = ctx.clean;
    let key = &ctx.key;
    let target = if is_open { 1. } else { 0. };
    let turn = motion::follow(&format!("{key}-chevron-{}", g.id), target, CHEVRON);
    let reveal = motion::follow(&format!("{key}-panel-{}", g.id), target, PANEL);
    let fade = motion::follow(&format!("{key}-fade-{}", g.id), target, PANEL_FADE);
    let count = picked.get(&g.id).map_or(0, |list| list.len());

    let head_key = group_key(&g.id);
    let head_handle = &handles[&head_key];
    let head_ring = ctx.keyboard && head_handle.is_focused(window);
    let picked_label =
        (count > 0).then(|| format!("{count} filter{}", if count == 1 { "" } else { "s" }));
    let head = div()
        .id(head_key.clone())
        .track_focus(head_handle)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .cursor_pointer()
        .hover_fill(
            format!("{key}-head-{}", g.id),
            p.row_hover.opacity(0.),
            p.row_hover,
        )
        .when(clean, |d| {
            d.gap(px(8.)).h(px(32.)).px(px(8.)).rounded(px(6.))
        })
        .when(!clean, |d| {
            d.gap(px(7.))
                .h(px(36.))
                .mt(px(2.))
                .mx(px(-6.))
                .pl(px(6.))
                .pr(px(8.))
                .rounded(px(10.))
        })
        .on_click({
            let group = g.id.clone();
            move |_, _, cx| toggle_group(&group, cx)
        })
        .child(
            svg()
                .path(icon(ctx.look, "chevron"))
                .size(px(16.))
                .flex_none()
                .text_color(p.chevron)
                .with_transformation(Transformation::rotate(radians(-FRAC_PI_2 * (1. - turn)))),
        )
        .child(
            div()
                .whitespace_nowrap()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.ink)
                .when(!clean, |d| d.text_size(px(14.)))
                .child(g.label.clone()),
        )
        .child(
            div()
                .text_color(p.count)
                .when(clean, |d| d.text_size(px(12.)).line_height(px(16.)))
                .when(!clean, |d| d.text_size(px(12.5)))
                .child(g.options.len().to_string()),
        )
        .child(
            // `margin-left: auto`: takes the free space and sits on the right
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .justify_end()
                .whitespace_nowrap()
                .text_color(p.picked)
                .when(clean, |d| {
                    d.overflow_hidden()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .children(picked_label.clone().map(|label| {
                            roll(
                                SharedString::from(format!("{key}-picked-{}-{count}", g.id)),
                                label,
                            )
                        }))
                })
                .when(!clean, |d| d.text_size(px(12.5)).children(picked_label)),
        )
        .when(head_ring, |d| {
            d.child(focus_ring(p, if clean { 6. } else { 10. }, clean))
        });

    // Opens and closes by gliding its height from 0 to the options' height
    let n = g.options.len() as f32;
    let full = if clean {
        n * 32. + 4.
    } else {
        2. + n * 34. + (n - 1.).max(0.) + 6.
    };
    let panel = (is_open || reveal > 0.001).then(|| {
        let options = g.options.iter().map(|o| {
            let row_key = option_key(&g.id, &o.id);
            let handle = &handles[&row_key];
            let ring = ctx.keyboard && handle.is_focused(window);
            let on = is_on(picked, &g.id, &o.id);
            let toggle = toggle.clone();
            let (group, option) = (g.id.clone(), o.id.clone());
            div()
                .id(row_key.clone())
                .track_focus(handle)
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .cursor_pointer()
                .hover_fill(
                    format!("{key}-option-{}-{}", g.id, o.id),
                    p.row_hover.opacity(0.),
                    p.row_hover,
                )
                .text_color(p.option)
                .when(clean, |d| {
                    d.gap(px(8.))
                        .h(px(32.))
                        .pl(px(32.))
                        .pr(px(8.))
                        .rounded(px(6.))
                })
                .when(!clean, |d| {
                    d.gap(px(12.))
                        .h(px(34.))
                        .mx(px(-6.))
                        .px(px(6.))
                        .rounded(px(10.))
                })
                .on_click(move |_, window, cx| toggle(&group, &option, window, cx))
                .child(checkbox(
                    ctx,
                    p,
                    &format!("{key}-box-{}-{}", g.id, o.id),
                    on,
                ))
                .child(div().min_w_0().truncate().child(o.label.clone()))
                .when(ring, |d| {
                    d.child(focus_ring(p, if clean { 6. } else { 10. }, true))
                })
        });
        div()
            .flex_none()
            .h(px(full * reveal))
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .opacity(fade)
                    .when(!clean, |d| d.gap(px(1.)).pt(px(2. * reveal)))
                    .children(options),
            )
    });

    div().flex().flex_col().child(head).children(panel)
}

/// `outline: 2px solid` for keyboard focus – drawn inside the row (offset
/// -2px), or around it for Classic's group heads.
fn focus_ring(p: &Palette, radius: f32, inside: bool) -> impl IntoElement {
    let ring = div().absolute().border_2().border_color(p.focus);
    if inside {
        ring.inset_0().rounded(px(radius))
    } else {
        ring.top(px(-4.))
            .left(px(-4.))
            .right(px(-4.))
            .bottom(px(-4.))
            .rounded(px(radius + 4.))
    }
}

/// The checkbox: fills with the accent and draws its check in.
fn checkbox(ctx: &Ctx, p: &Palette, box_key: &str, on: bool) -> impl IntoElement {
    let clean = ctx.clean;
    let target = if on { 1. } else { 0. };
    let fill = motion::follow(
        &format!("{box_key}-fill"),
        target,
        MotionSpec::new(if clean { 160 } else { 180 }, EASE),
    );
    let draw = motion::follow(
        &format!("{box_key}-draw"),
        target,
        MotionSpec::new(if clean { 240 } else { 260 }, EASE_OUT_QUINT),
    );
    let geometry = CheckGeometry::get(ctx.look);
    let reach = geometry.reach(draw);
    let size = geometry.size;
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(if clean { 14. } else { 20. }))
        .rounded(px(if clean { 4. } else { 6. }))
        .bg(motion::mix(p.box_bg, p.box_on, fill))
        .border(px(if clean { 0.5 } else { 1. }))
        .border_color(motion::mix(p.box_edge, p.box_edge_on, fill))
        .when_some(p.box_lift, |d, lift| {
            d.shadow(vec![shadow(0., 1., 1., 0., lift.opacity(1. - fill))])
        })
        .child(
            div().size(px(size)).flex_none().child(
                div().w(px(reach)).h(px(size)).overflow_hidden().child(
                    svg()
                        .path(icon(ctx.look, "check"))
                        .size(px(size))
                        .flex_none()
                        .text_color(p.check),
                ),
            ),
        )
}

/// The keyboard hints: the card's last row (Clean) or a line on the tray.
fn hint_row(ctx: &Ctx, p: &Palette) -> impl IntoElement {
    if ctx.clean {
        let kbd = |glyph: &'static str| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .min_w(px(16.))
                .h(px(16.))
                .px(px(4.))
                .rounded(px(4.))
                .bg(p.kbd)
                .shadow(p.kbd_shadow.clone())
                .text_size(px(10.))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.kbd_fg)
                .child(glyph)
        };
        let hint = |keys: Vec<Div>, text: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(2.))
                .whitespace_nowrap()
                .children(keys)
                .child(div().ml(px(4.)).child(text))
        };
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .h(px(32.))
            .px(px(12.))
            .border_t(px(0.5))
            .border_color(p.line)
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(p.hint)
            .child(hint(vec![kbd("↑"), kbd("↓")], "to navigate"))
            .child(hint(vec![kbd("↵")], "to select"))
    } else {
        let kbd = |name: &str| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(22.))
                .rounded(px(6.))
                .bg(p.kbd)
                .shadow(p.kbd_shadow.clone())
                .child(
                    svg()
                        .path(icon(ctx.look, name))
                        .size(px(13.))
                        .flex_none()
                        .text_color(p.kbd_fg),
                )
        };
        let hint = |keys: Vec<Div>, text: &'static str| {
            div()
                .flex()
                .items_center()
                .gap(px(5.))
                .children(keys)
                .child(div().ml(px(4.)).child(text))
        };
        div()
            .flex()
            .justify_between()
            .gap(px(12.))
            .pt(px(10.))
            .px(px(8.))
            .pb(px(4.))
            .text_size(px(12.5))
            .text_color(p.hint)
            .child(hint(
                vec![kbd("arrow-down"), kbd("arrow-up")],
                "To navigate",
            ))
            .child(hint(vec![kbd("enter")], "To select"))
    }
}

/// Stage, Deal size and Region – the website demo's CRM filters.
pub fn demo_groups() -> Vec<FilterGroup> {
    vec![
        FilterGroup::new("stage", "Stage")
            .option("new", "New lead")
            .option("qualified", "Qualified")
            .option("proposal", "Proposal sent")
            .option("negotiation", "Negotiation"),
        FilterGroup::new("value", "Deal size")
            .chip_prefix("Deal: ")
            .option("s", "Under $5k")
            .option("m", "$5k–20k")
            .option("l", "$20k–50k")
            .option("xl", "Over $50k"),
        FilterGroup::new("region", "Region")
            .option("dach", "DACH")
            .option("nordics", "Nordics")
            .option("uk", "UK & Ireland")
            .option("na", "North America")
            .option("apac", "APAC"),
    ]
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, _| {
        let value = FilterValue::from([
            ("stage".into(), vec!["qualified".into()]),
            ("value".into(), vec!["m".into()]),
            ("region".into(), vec!["dach".into()]),
        ]);
        FilterMenu::new("filter-menu", demo_groups()).default_value(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_reach_grows_to_the_box() {
        for look in [Look::Clean, Look::Classic] {
            let g = CheckGeometry::get(look);
            assert_eq!(g.reach(0.), 0.);
            let mut last = 0.;
            for i in 1..=20 {
                let r = g.reach(i as f32 / 20.);
                assert!(r >= last);
                last = r;
            }
            assert!(last <= g.size && last > g.size * 0.7);
        }
    }

    #[test]
    fn row_keys() {
        assert_eq!(group_key("stage").as_ref(), "g:stage");
        assert_eq!(option_key("stage", "new").as_ref(), "o:stage:new");
        let value = FilterValue::from([("stage".into(), vec!["new".into()])]);
        assert!(is_on(&value, "stage", "new"));
        assert!(!is_on(&value, "stage", "won"));
        assert!(!is_on(&value, "region", "new"));
    }
}
