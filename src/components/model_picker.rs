//! Model Picker – the composer's model chip and its model menu.
//!
//! A Pink component, ported from Pink's harness/model picker (Pink:
//! pickers.rs `trigger_chip` + `model_chip_content` and
//! `render_harness_model_popover`). The chip is a ghost pill: the
//! provider's mark, the model name and the reasoning level as a muted
//! second tone. Its menu is a flush 320 card: a strip of provider tabs led
//! by a Favorites star, a search row, a band of compact model rows (⌘1–⌘9
//! and a star each), and a pinned tray whose Reasoning and option rows open
//! nested choice menus on hover. Picking a model keeps the card open, so
//! the tray can be adjusted right after.
//!
//! Keys: ↑/↓ move through the models and on into the tray, Enter picks a
//! model or opens a tray row, → opens and ← closes a tray row's choices,
//! ⌘1–⌘9 pick a model, Escape closes the choices, then the menu.
//!
//! ```ignore
//! ModelPicker::new("model", default_providers())
//!     .selected("helix-4-pro")
//!     .on_change(|model, _, _| println!("now using {}", model.name))
//! ```

use std::{collections::HashMap, rc::Rc, sync::Arc, time::Duration};

use gpui::{
    Action, Anchor, AnyElement, AnyView, App, Bounds, Context, DispatchPhase, Div, ElementId,
    Entity, FocusHandle, Focusable, FontWeight, Hsla, IntoElement, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, Pixels, Point, Render, Role, ScrollHandle, SharedString,
    Stateful, Subscription, Task, Toggled, Window, anchored, canvas, deferred, div, point,
    prelude::*, px, svg,
};
use web_time::Instant;

use crate::{
    HoverFill,
    motion::{self, MENU_OUT, MENU_RISE_DOWN, MENU_RISE_UP},
    pink::{self, FillTier, Pink, layout, radius},
    primitives::{InputEvent, TextInput, text_input},
    story,
};

pub const NAME: &str = "Model Picker";
pub const DESCRIPTION: &str = "A composer's model chip and menu: provider tabs, a search row, compact model rows with ⌘ shortcuts and stars, and a tray whose Reasoning and option rows open nested choices";

const ICON_HELIX: &str = "icons/model-picker/helix.svg";
const ICON_ORBIT: &str = "icons/model-picker/orbit.svg";
const ICON_RELAY: &str = "icons/model-picker/relay.svg";
const ICON_MAGNIFER: &str = "icons/model-picker/magnifer.svg";
const ICON_STAR: &str = "icons/model-picker/star.svg";
const ICON_STAR_BOLD: &str = "icons/model-picker/star-bold.svg";
const ICON_BOLT: &str = "icons/model-picker/bolt-fast.svg";
const ICON_PAPERCLIP: &str = "icons/model-picker/paperclip.svg";
const ICON_ARROW_UP: &str = "icons/model-picker/arrow-up.svg";

/// The chip's brand mark and fast bolt (Pink: pickers.rs `CHIP_ICON`,
/// `CHIP_BOLT`).
const CHIP_ICON: f32 = 16.;
const CHIP_BOLT: f32 = 13.;
/// The tab strip and the search row (Pink: `render_harness_model_popover`).
const STRIP_H: f32 = 40.;
const TAB: f32 = 32.;
/// The model band: roughly seven compact rows (Pink: 216 at the 16 px
/// baseline).
const LIST_H: f32 = 216.;
const TRAY_MAX_H: f32 = 236.;
const ROW_STAR: f32 = 22.;
/// Tray rows and their nested choices (Pink: `render_traits_sections`).
const TRAY_ROW_H: f32 = 30.;
const CHOICES_W: f32 = 232.;
const CHOICES_MAX_H: f32 = 240.;
/// Room a nested choices card needs beside its row before it opens left.
const NESTED_ROOM: f32 = 244.;
/// Gap between a tray row and its nested card (Pink: `nested_menu`).
const NESTED_GAP: f32 = layout::CARD_INSET + 6.;
/// While the pointer heads for an open submenu, a sibling it crosses takes
/// over only after this pause (Pink: `hover_setting`).
const SUBMENU_GRACE: Duration = Duration::from_millis(300);
/// Tooltip details longer than this wrap (Pink: `ControlTooltip`).
const TOOLTIP_WRAP_CHARS: usize = 40;
/// Settings that carry a provider's speed, and the choices that run fast:
/// the chip then wears a bolt (Pink: `fast_speed`).
const SPEED_SETTINGS: [&str; 3] = ["serviceTier", "fastMode", "speed"];
const FAST_CHOICES: [&str; 4] = ["fast", "on", "priority", "ultrafast"];

type ChangeHandler = Rc<dyn Fn(&ModelOption, &mut Window, &mut App)>;
type ReasoningHandler = Rc<dyn Fn(ReasoningLevel, &mut Window, &mut App)>;
type SettingHandler = Rc<dyn Fn(&SharedString, &SharedString, &mut Window, &mut App)>;
type FavoriteHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;

/// How hard a model thinks before it answers (Pink: `ReasoningLevel`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReasoningLevel {
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ReasoningLevel {
    /// Pink: `reasoning_label`.
    pub fn label(self) -> &'static str {
        match self {
            ReasoningLevel::Minimal => "Minimal",
            ReasoningLevel::Low => "Low",
            ReasoningLevel::Medium => "Medium",
            ReasoningLevel::High => "High",
            ReasoningLevel::XHigh => "X-High",
            ReasoningLevel::Max => "Max",
        }
    }
}

/// The model's default level: High when it offers it, then Medium, then
/// its first (Pink: `default_reasoning_level`).
fn default_reasoning(ladder: &[ReasoningLevel]) -> Option<ReasoningLevel> {
    [ReasoningLevel::High, ReasoningLevel::Medium]
        .into_iter()
        .find(|level| ladder.contains(level))
        .or_else(|| ladder.first().copied())
}

/// A picked level the model still offers, else its default (Pink:
/// `clamp_reasoning`).
fn clamp_reasoning(
    level: Option<ReasoningLevel>,
    ladder: &[ReasoningLevel],
) -> Option<ReasoningLevel> {
    match level {
        Some(level) if ladder.contains(&level) => Some(level),
        _ => default_reasoning(ladder),
    }
}

/// One more row of the tray (Pink: a model option such as Context or
/// Speed), with its own nested choices.
#[derive(Clone, Debug)]
pub struct ModelSetting {
    pub id: SharedString,
    pub label: SharedString,
    pub choices: Vec<SharedString>,
    /// The index of the choice a fresh pick starts on.
    pub default: usize,
}

impl ModelSetting {
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        choices: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            choices: choices.into_iter().map(Into::into).collect(),
            default: 0,
        }
    }

    pub fn default_choice(mut self, ix: usize) -> Self {
        self.default = ix;
        self
    }
}

/// One model in the menu.
#[derive(Clone, Debug)]
pub struct ModelOption {
    pub id: SharedString,
    pub name: SharedString,
    /// Who serves the model when its provider routes several vendors, shown
    /// after the name (Pink: the OpenCode attribution). Rows otherwise show
    /// the name alone.
    pub attribution: Option<SharedString>,
    /// A small accent capsule after the name (Pink: the "New" badge).
    pub tag: Option<SharedString>,
    /// The reasoning ladder; empty hides the tray's Reasoning row.
    pub reasoning: Vec<ReasoningLevel>,
    /// More tray rows.
    pub settings: Vec<ModelSetting>,
}

impl ModelOption {
    /// A model with a Low, Medium and High reasoning ladder.
    pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            attribution: None,
            tag: None,
            reasoning: vec![
                ReasoningLevel::Low,
                ReasoningLevel::Medium,
                ReasoningLevel::High,
            ],
            settings: Vec::new(),
        }
    }

    pub fn attribution(mut self, attribution: impl Into<SharedString>) -> Self {
        self.attribution = Some(attribution.into());
        self
    }

    pub fn tag(mut self, tag: impl Into<SharedString>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn reasoning(mut self, ladder: impl IntoIterator<Item = ReasoningLevel>) -> Self {
        self.reasoning = ladder.into_iter().collect();
        self
    }

    /// Hides the Reasoning row for this model.
    pub fn no_reasoning(mut self) -> Self {
        self.reasoning.clear();
        self
    }

    pub fn setting(mut self, setting: ModelSetting) -> Self {
        self.settings.push(setting);
        self
    }
}

/// A provider: one tab of the menu, and the mark the chip wears for its
/// models.
#[derive(Clone, Debug)]
pub struct ModelProvider {
    pub id: SharedString,
    pub name: SharedString,
    /// An SVG path under `assets/`.
    pub icon: SharedString,
    /// A brand tint for the mark; without one it paints in the text tones
    /// (Pink keeps one harness mark in its brand color).
    pub tint: Option<Hsla>,
    pub models: Vec<ModelOption>,
}

impl ModelProvider {
    pub fn new(
        id: impl Into<SharedString>,
        name: impl Into<SharedString>,
        icon: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon: icon.into(),
            tint: None,
            models: Vec::new(),
        }
    }

    pub fn tint(mut self, tint: Hsla) -> Self {
        self.tint = Some(tint);
        self
    }

    pub fn model(mut self, model: ModelOption) -> Self {
        self.models.push(model);
        self
    }
}

/// Helix, Orbit and Relay (a router that serves other vendors' models).
pub fn default_providers() -> Vec<ModelProvider> {
    use ReasoningLevel::*;
    let context = || ModelSetting::new("contextWindow", "Context", ["200K", "1M"]);
    let speed = || ModelSetting::new("speed", "Speed", ["Standard", "Fast"]);
    vec![
        ModelProvider::new("helix", "Helix", ICON_HELIX)
            .tint(pink::project_tint(5))
            .model(
                ModelOption::new("helix-4-ultra", "Helix 4 Ultra")
                    .tag("New")
                    .reasoning([Low, Medium, High, Max])
                    .setting(context()),
            )
            .model(
                ModelOption::new("helix-4-pro", "Helix 4 Pro")
                    .reasoning([Low, Medium, High, Max])
                    .setting(context()),
            )
            .model(
                ModelOption::new("helix-4-flash", "Helix 4 Flash")
                    .reasoning([Minimal, Low, Medium])
                    .setting(speed()),
            )
            .model(ModelOption::new("helix-3-5-pro", "Helix 3.5 Pro"))
            .model(ModelOption::new("helix-3-5-flash", "Helix 3.5 Flash").no_reasoning()),
        ModelProvider::new("orbit", "Orbit", ICON_ORBIT)
            .model(
                ModelOption::new("orbit-6", "Orbit-6")
                    .reasoning([Minimal, Low, Medium, High, XHigh])
                    .setting(ModelSetting::new(
                        "serviceTier",
                        "Speed",
                        ["Standard", "Priority"],
                    )),
            )
            .model(
                ModelOption::new("orbit-6-coder", "Orbit-6 Coder")
                    .tag("New")
                    .reasoning([Low, Medium, High, XHigh]),
            )
            .model(ModelOption::new("orbit-6-mini", "Orbit-6 Mini"))
            .model(ModelOption::new("orbit-5", "Orbit-5")),
        ModelProvider::new("relay", "Relay", ICON_RELAY)
            .model(
                ModelOption::new("kestrel-2-70b", "Kestrel 2 70B")
                    .attribution("Northwind")
                    .no_reasoning(),
            )
            .model(ModelOption::new("lumen-3", "Lumen 3").attribution("Fieldstone"))
            .model(
                ModelOption::new("quill-1-5", "Quill 1.5")
                    .attribution("Papermill")
                    .no_reasoning(),
            ),
    ]
}

/// Which side of the trigger the menu opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Above,
    Below,
}

#[derive(IntoElement)]
pub struct ModelPicker {
    id: ElementId,
    providers: Vec<ModelProvider>,
    selected: Option<SharedString>,
    favorites: Vec<SharedString>,
    reasoning: Option<ReasoningLevel>,
    default_open: bool,
    autofocus: bool,
    placement: Placement,
    on_change: Option<ChangeHandler>,
    on_reasoning_change: Option<ReasoningHandler>,
    on_setting_change: Option<SettingHandler>,
    on_favorite_change: Option<FavoriteHandler>,
}

impl ModelPicker {
    pub fn new(id: impl Into<ElementId>, providers: Vec<ModelProvider>) -> Self {
        Self {
            id: id.into(),
            providers,
            selected: None,
            favorites: Vec::new(),
            reasoning: None,
            default_open: false,
            autofocus: false,
            placement: Placement::Above,
            on_change: None,
            on_reasoning_change: None,
            on_setting_change: None,
            on_favorite_change: None,
        }
    }

    /// The model chosen at first (by id); defaults to the first one.
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Models starred at first (by id). With any star the menu opens on the
    /// Favorites tab, as Pink's does.
    pub fn favorites(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.favorites = ids.into_iter().map(Into::into).collect();
        self
    }

    /// The reasoning level picked at first; each model clamps it to its
    /// ladder (else its default, High when offered).
    pub fn reasoning(mut self, level: ReasoningLevel) -> Self {
        self.reasoning = Some(level);
        self
    }

    /// Starts with the menu open.
    pub fn default_open(mut self, open: bool) -> Self {
        self.default_open = open;
        self
    }

    /// Takes keyboard focus when it first renders.
    pub fn autofocus(mut self, autofocus: bool) -> Self {
        self.autofocus = autofocus;
        self
    }

    pub fn placement(mut self, placement: Placement) -> Self {
        self.placement = placement;
        self
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(&ModelOption, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    /// Called with the level picked from the Reasoning row.
    pub fn on_reasoning_change(
        mut self,
        handler: impl Fn(ReasoningLevel, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_reasoning_change = Some(Rc::new(handler));
        self
    }

    /// Called with a setting's id and the choice picked from its row.
    pub fn on_setting_change(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_setting_change = Some(Rc::new(handler));
        self
    }

    /// Called with a model's id and whether it is now starred.
    pub fn on_favorite_change(
        mut self,
        handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_favorite_change = Some(Rc::new(handler));
        self
    }
}

/// The builder's data and callbacks, refreshed every render so the search
/// field's events act on the current ones.
struct Props {
    providers: Vec<ModelProvider>,
    on_change: Option<ChangeHandler>,
    on_reasoning_change: Option<ReasoningHandler>,
    on_setting_change: Option<SettingHandler>,
    on_favorite_change: Option<FavoriteHandler>,
}

/// The viewed tab (Pink: `ModelRail`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rail {
    Favorites,
    Provider(usize),
}

/// A visible model row: provider and model index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    p: usize,
    m: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GroupKind {
    Reasoning,
    Setting(usize),
}

/// One tray row and its choices (Pink: `SettingGroup`).
struct Group {
    kind: GroupKind,
    label: SharedString,
    choices: Vec<Choice>,
}

struct Choice {
    label: SharedString,
    selected: bool,
    default: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Nav {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
}

/// Match rank of `label` for `query`: 0 prefix, 1 substring (Pink:
/// `popover::match_rank`).
fn match_rank(query: &str, label: &str) -> Option<usize> {
    let label = label.to_lowercase();
    if label.starts_with(query) {
        Some(0)
    } else if label.contains(query) {
        Some(1)
    } else {
        None
    }
}

/// The rows one tab shows (Pink: `scoped_model_rows`). A query never leaves
/// the viewed tab: it ranks label prefix, then substring, then attribution
/// hits, stars and input order breaking ties. Without one, a provider tab
/// lists its stars first and the Favorites tab every star.
fn scoped_rows(
    providers: &[ModelProvider],
    rail: Rail,
    query: &str,
    starred: impl Fn(usize, usize) -> bool,
) -> Vec<Row> {
    let in_scope = |p: usize, m: usize| match rail {
        Rail::Favorites => starred(p, m),
        Rail::Provider(viewed) => p == viewed,
    };
    let query = query.trim().to_lowercase();
    if !query.is_empty() {
        let mut ranked: Vec<(usize, bool, usize, Row)> = Vec::new();
        let mut input_ix = 0;
        for (p, provider) in providers.iter().enumerate() {
            for (m, model) in provider.models.iter().enumerate() {
                if !in_scope(p, m) {
                    continue;
                }
                let by_label = match_rank(&query, &model.name);
                let haystack = format!(
                    "{} {}",
                    model.attribution.as_deref().unwrap_or(""),
                    model.name
                );
                let by_attribution = match_rank(&query, &haystack).map(|rank| rank + 2);
                if let Some(rank) = by_label.into_iter().chain(by_attribution).min() {
                    ranked.push((rank, !starred(p, m), input_ix, Row { p, m }));
                }
                input_ix += 1;
            }
        }
        ranked.sort_by_key(|&(rank, unstarred, ix, _)| (rank, unstarred, ix));
        return ranked.into_iter().map(|(.., row)| row).collect();
    }
    match rail {
        Rail::Favorites => providers
            .iter()
            .enumerate()
            .flat_map(|(p, provider)| (0..provider.models.len()).map(move |m| Row { p, m }))
            .filter(|row| starred(row.p, row.m))
            .collect(),
        Rail::Provider(viewed) => {
            let Some(provider) = providers.get(viewed) else {
                return Vec::new();
            };
            let (stars, rest): (Vec<Row>, Vec<Row>) = (0..provider.models.len())
                .map(|m| Row { p: viewed, m })
                .partition(|row| starred(row.p, row.m));
            stars.into_iter().chain(rest).collect()
        }
    }
}

/// Whether `pointer`, moving away from `origin`, is still heading into
/// `submenu` (Pink: `submenu_corridor`).
fn submenu_corridor(
    origin: Point<Pixels>,
    pointer: Point<Pixels>,
    submenu: Bounds<Pixels>,
    on_left: bool,
) -> bool {
    let edge = if on_left {
        submenu.right()
    } else {
        submenu.left()
    };
    let direction = if on_left { -1.0 } else { 1.0 };
    let distance = f32::from(edge - origin.x) * direction;
    let advance = f32::from(pointer.x - origin.x) * direction;
    if distance <= 0.0 || advance <= 0.0 || advance > distance + 8.0 {
        return false;
    }
    let fraction = (advance / distance).min(1.0);
    let top = origin.y + (submenu.top() - px(8.0) - origin.y) * fraction;
    let bottom = origin.y + (submenu.bottom() + px(8.0) - origin.y) * fraction;
    pointer.y >= top && pointer.y <= bottom
}

struct PickerState {
    /// The trigger's focus.
    focus: FocusHandle,
    search: Entity<TextInput>,
    _search_events: Subscription,
    props: Rc<Props>,
    open: bool,
    /// When the menu began its exit fade.
    closing: Option<Instant>,
    _close_task: Option<Task<()>>,
    /// Bumped on every open so the entrance replays.
    opened: u32,
    rail: Rail,
    selected: (usize, usize),
    /// The one shared highlight, over the model rows and then the tray rows.
    active: usize,
    query: SharedString,
    /// The picked reasoning level, before each model clamps it.
    reasoning: Option<ReasoningLevel>,
    /// Setting picks per model, keyed by provider, model and setting id.
    picks: HashMap<SharedString, usize>,
    favorites: Vec<(SharedString, SharedString)>,
    scroll: ScrollHandle,
    trigger: Bounds<Pixels>,
    /// The tray row whose choices are open, with their cursor and bounds.
    setting_menu: Option<usize>,
    setting_active: usize,
    setting_bounds: Option<Bounds<Pixels>>,
    setting_on_left: bool,
    setting_origin: Option<Point<Pixels>>,
    hover_pending: Option<usize>,
    hover_pointer: Option<Point<Pixels>>,
    _hover_task: Option<Task<()>>,
}

impl PickerState {
    fn provider(&self) -> &ModelProvider {
        &self.props.providers[self.selected.0]
    }

    fn model(&self) -> &ModelOption {
        &self.provider().models[self.selected.1]
    }

    fn starred(&self, p: usize, m: usize) -> bool {
        let provider = &self.props.providers[p];
        let model = &provider.models[m];
        self.favorites
            .iter()
            .any(|(pid, mid)| *pid == provider.id && *mid == model.id)
    }

    fn rows(&self) -> Vec<Row> {
        scoped_rows(&self.props.providers, self.rail, &self.query, |p, m| {
            self.starred(p, m)
        })
    }

    /// Where the selected model sits in the visible rows (Pink:
    /// `selected_model_index`).
    fn selected_index(&self) -> usize {
        let (p, m) = self.selected;
        self.rows()
            .iter()
            .position(|row| row.p == p && row.m == m)
            .unwrap_or(0)
    }

    /// Keeps indices in range after the builder's data changes.
    fn clamp(&mut self) {
        let providers = &self.props.providers;
        let (p, m) = self.selected;
        if providers
            .get(p)
            .is_none_or(|provider| m >= provider.models.len())
        {
            self.selected = providers
                .iter()
                .position(|provider| !provider.models.is_empty())
                .map_or((0, 0), |p| (p, 0));
        }
        if let Rail::Provider(viewed) = self.rail
            && viewed >= providers.len()
        {
            self.rail = Rail::Provider(self.selected.0);
        }
    }

    fn pick_key(&self, setting: &ModelSetting) -> SharedString {
        format!(
            "{}\u{1f}{}\u{1f}{}",
            self.provider().id,
            self.model().id,
            setting.id
        )
        .into()
    }

    fn effective_reasoning(&self) -> Option<ReasoningLevel> {
        clamp_reasoning(self.reasoning, &self.model().reasoning)
    }

    /// The effective choice of the selected model's setting `si`.
    fn setting_choice(&self, si: usize) -> usize {
        let setting = &self.model().settings[si];
        let last = setting.choices.len().saturating_sub(1);
        self.picks
            .get(&self.pick_key(setting))
            .copied()
            .filter(|&ix| ix <= last)
            .unwrap_or(setting.default.min(last))
    }

    /// The tray rows for the selected model (Pink: `setting_groups`).
    fn groups(&self) -> Vec<Group> {
        let model = self.model();
        let mut groups = Vec::new();
        if !model.reasoning.is_empty() {
            let selected = self.effective_reasoning();
            let default = default_reasoning(&model.reasoning);
            groups.push(Group {
                kind: GroupKind::Reasoning,
                label: "Reasoning".into(),
                choices: model
                    .reasoning
                    .iter()
                    .map(|&level| Choice {
                        label: level.label().into(),
                        selected: Some(level) == selected,
                        default: Some(level) == default,
                    })
                    .collect(),
            });
        }
        for (si, setting) in model.settings.iter().enumerate() {
            if setting.choices.is_empty() {
                continue;
            }
            let chosen = self.setting_choice(si);
            groups.push(Group {
                kind: GroupKind::Setting(si),
                label: setting.label.clone(),
                choices: setting
                    .choices
                    .iter()
                    .enumerate()
                    .map(|(ci, label)| Choice {
                        label: label.clone(),
                        selected: ci == chosen,
                        default: ci == setting.default,
                    })
                    .collect(),
            });
        }
        groups
    }

    /// Whether any trait departs from its default: the chip's suffix then
    /// brightens (Pink: `traits_customized`).
    fn customized(&self) -> bool {
        let model = self.model();
        self.effective_reasoning() != default_reasoning(&model.reasoning)
            || (0..model.settings.len())
                .filter(|&si| !model.settings[si].choices.is_empty())
                .any(|si| self.setting_choice(si) != model.settings[si].default)
    }

    /// Whether the effective speed runs faster than standard (Pink:
    /// `fast_speed`).
    fn fast(&self) -> bool {
        let model = self.model();
        model.settings.iter().enumerate().any(|(si, setting)| {
            SPEED_SETTINGS.contains(&setting.id.as_ref())
                && setting
                    .choices
                    .get(self.setting_choice(si))
                    .is_some_and(|choice| FAST_CHOICES.contains(&choice.to_lowercase().as_str()))
        })
    }

    /// "High, 1M, Fast": the reasoning level and every setting's choice
    /// (Pink: `traits_summary`).
    fn summary(&self) -> Option<SharedString> {
        let model = self.model();
        let mut parts: Vec<SharedString> = Vec::new();
        if let Some(level) = self.effective_reasoning() {
            parts.push(level.label().into());
        }
        for (si, setting) in model.settings.iter().enumerate() {
            if let Some(choice) = setting.choices.get(self.setting_choice(si)) {
                parts.push(choice.clone());
            }
        }
        (!parts.is_empty()).then(|| parts.join(", ").into())
    }

    /// Runs a callback once the current update is done.
    fn emit(
        &self,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(&Props, &mut Window, &mut App) + 'static,
    ) {
        let props = self.props.clone();
        window.defer(cx, move |window, cx| f(&props, window, cx));
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close_menu(true, window, cx);
        } else {
            self.open_menu(window, cx);
        }
    }

    /// Pink: `toggle` opening the model picker. The search clears, the
    /// Favorites tab leads when there are stars, the highlight starts on the
    /// selected row and the search takes focus.
    fn open_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open = true;
        self.closing = None;
        self._close_task = None;
        self.opened += 1;
        self.query = SharedString::default();
        self.search.update(cx, |input, cx| input.clear(cx));
        self.close_setting();
        self.rail = if self.favorites.is_empty() {
            Rail::Provider(self.selected.0)
        } else {
            Rail::Favorites
        };
        self.active = self.selected_index();
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.scroll.scroll_to_item(self.active);
        TextInput::focus(&self.search, window, cx);
        cx.notify();
    }

    /// Starts the exit fade; the menu unmounts when it ends.
    fn close_menu(&mut self, refocus: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.close_setting();
        self.open = false;
        self.closing = Some(Instant::now());
        let wait = MENU_OUT.total().mul_f32(motion::speed_scale()) + Duration::from_millis(16);
        self._close_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let _ = this.update(cx, |s, cx| {
                s.closing = None;
                cx.notify();
            });
        }));
        if refocus {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    fn cancel_setting_hover(&mut self) {
        self._hover_task = None;
        self.hover_pending = None;
        self.hover_pointer = None;
    }

    fn close_setting(&mut self) {
        self.cancel_setting_hover();
        self.setting_menu = None;
        self.setting_bounds = None;
    }

    /// Pink: `open_setting`. The choices open with their cursor on the
    /// current one.
    fn open_setting(&mut self, ix: usize, cx: &mut Context<Self>) {
        self.cancel_setting_hover();
        self.setting_origin = None;
        self.setting_active = self
            .groups()
            .get(ix)
            .and_then(|group| group.choices.iter().position(|choice| choice.selected))
            .unwrap_or(0);
        self.setting_menu = Some(ix);
        self.setting_bounds = None;
        cx.notify();
    }

    /// The pointer resting on tray row `ix` opens its choices, after a short
    /// grace when it is only crossing on its way into an open submenu
    /// (Pink: `hover_setting`).
    fn hover_setting(
        &mut self,
        ix: usize,
        active: usize,
        pointer: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.cancel_setting_hover();
        if self.setting_menu == Some(ix) {
            self.setting_origin = Some(pointer);
            return;
        }
        let toward_child = self.setting_menu.is_some()
            && self
                .setting_origin
                .zip(self.setting_bounds)
                .is_some_and(|(origin, bounds)| {
                    submenu_corridor(origin, pointer, bounds, self.setting_on_left)
                });
        if toward_child {
            let source = self.setting_menu;
            self.hover_pending = Some(ix);
            self.hover_pointer = Some(pointer);
            self._hover_task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SUBMENU_GRACE).await;
                let _ = this.update(cx, |s, cx| {
                    if s.open && s.setting_menu == source && s.hover_pending == Some(ix) {
                        s.active = active;
                        s.open_setting(ix, cx);
                        s.setting_origin = Some(pointer);
                    }
                });
            }));
        } else {
            self.active = active;
            self.open_setting(ix, cx);
            self.setting_origin = Some(pointer);
        }
    }

    /// Pink: `activate_model_index` + `pick_model`. The card stays open.
    fn pick_model(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.rows().get(ix).copied() else {
            return;
        };
        self.close_setting();
        self.selected = (row.p, row.m);
        self.active = ix;
        self.emit(window, cx, move |props, window, cx| {
            if let Some(f) = props.on_change.as_ref() {
                f(&props.providers[row.p].models[row.m], window, cx);
            }
        });
        cx.notify();
    }

    /// Pink: `toggle_model_favorite`. A star reorders the list, so the
    /// highlight goes back to the selected row.
    fn toggle_favorite(&mut self, p: usize, m: usize, window: &mut Window, cx: &mut Context<Self>) {
        let provider = &self.props.providers[p];
        let key = (provider.id.clone(), provider.models[m].id.clone());
        let starred = match self.favorites.iter().position(|fav| *fav == key) {
            Some(ix) => {
                self.favorites.remove(ix);
                false
            }
            None => {
                self.favorites.push(key.clone());
                true
            }
        };
        self.active = self.selected_index();
        self.emit(window, cx, move |props, window, cx| {
            if let Some(f) = props.on_favorite_change.as_ref() {
                f(&key.1, starred, window, cx);
            }
        });
        cx.notify();
    }

    fn view(&mut self, rail: Rail, cx: &mut Context<Self>) {
        self.close_setting();
        self.rail = rail;
        self.active = self.selected_index();
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.scroll.scroll_to_item(self.active);
        cx.notify();
    }

    /// Pink: `activate_setting_choice`.
    fn pick_choice(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(gi) = self.setting_menu else {
            return;
        };
        let groups = self.groups();
        let Some(group) = groups.get(gi) else {
            return;
        };
        let ci = self.setting_active;
        if ci >= group.choices.len() {
            return;
        }
        match group.kind {
            GroupKind::Reasoning => {
                let level = self.model().reasoning[ci];
                self.reasoning = Some(level);
                self.emit(window, cx, move |props, window, cx| {
                    if let Some(f) = props.on_reasoning_change.as_ref() {
                        f(level, window, cx);
                    }
                });
            }
            GroupKind::Setting(si) => {
                let setting = self.model().settings[si].clone();
                self.picks.insert(self.pick_key(&setting), ci);
                self.emit(window, cx, move |props, window, cx| {
                    if let Some(f) = props.on_setting_change.as_ref() {
                        f(&setting.id, &setting.choices[ci], window, cx);
                    }
                });
            }
        }
        self.close_setting();
        cx.notify();
    }

    /// The picker's keys (Pink: `on_key_down`). Returns whether it took the
    /// key.
    fn nav(&mut self, nav: Nav, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.cancel_setting_hover();
        if !self.open {
            if matches!(nav, Nav::Up | Nav::Down) {
                self.open_menu(window, cx);
                return true;
            }
            return false;
        }
        let groups = self.groups();
        if let Some(open) = self.setting_menu {
            match nav {
                Nav::Escape | Nav::Left => self.close_setting(),
                Nav::Up | Nav::Down => {
                    let count = groups.get(open).map_or(0, |group| group.choices.len());
                    self.setting_active = step(self.setting_active, count, nav == Nav::Up);
                }
                Nav::Enter => self.pick_choice(window, cx),
                Nav::Right => return false,
            }
            cx.notify();
            return true;
        }
        let rows = self.rows().len();
        match nav {
            Nav::Right if self.active >= rows && self.active < rows + groups.len() => {
                self.open_setting(self.active - rows, cx);
            }
            Nav::Escape => self.close_menu(true, window, cx),
            Nav::Up | Nav::Down => {
                self.active = step(self.active, rows + groups.len(), nav == Nav::Up);
                if self.active < rows {
                    self.scroll.scroll_to_item(self.active);
                }
            }
            Nav::Enter => {
                if self.active >= rows {
                    if self.active - rows < groups.len() {
                        self.open_setting(self.active - rows, cx);
                    }
                } else {
                    self.pick_model(self.active, window, cx);
                }
            }
            Nav::Left | Nav::Right => return false,
        }
        cx.notify();
        true
    }

    fn on_search(&mut self, event: &InputEvent, _: &mut Window, cx: &mut Context<Self>) {
        // Typing resets the highlight to the top of the fresh results.
        if let InputEvent::Change(text) = event {
            self.query = text.clone();
            self.close_setting();
            self.active = 0;
            self.scroll.set_offset(point(px(0.), px(0.)));
            cx.notify();
        }
    }
}

/// One step through `count` rows, wrapping (Pink: `popover::menu_step`).
fn step(at: usize, count: usize, up: bool) -> usize {
    if count == 0 {
        return 0;
    }
    if up {
        (at.min(count) + count - 1) % count
    } else {
        (at + 1) % count
    }
}

/// A capture listener for one of the search field's keys, so the picker
/// sees them before the field does.
fn nav_action<A: Action>(
    state: &Entity<PickerState>,
    nav: Nav,
) -> impl Fn(&A, &mut Window, &mut App) + 'static {
    let state = state.clone();
    move |_, window, cx| {
        if state.update(cx, |s, cx| s.nav(nav, window, cx)) {
            cx.stop_propagation();
        }
    }
}

impl RenderOnce for ModelPicker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = Pink::get(cx);
        let key = self.id.to_string();
        if self
            .providers
            .iter()
            .all(|provider| provider.models.is_empty())
        {
            return div().id(self.id);
        }
        let initial = self
            .selected
            .as_ref()
            .and_then(|id| {
                self.providers.iter().enumerate().find_map(|(p, provider)| {
                    provider
                        .models
                        .iter()
                        .position(|m| &m.id == id)
                        .map(|m| (p, m))
                })
            })
            .unwrap_or_else(|| {
                let p = self
                    .providers
                    .iter()
                    .position(|provider| !provider.models.is_empty())
                    .unwrap_or(0);
                (p, 0)
            });
        let favorites: Vec<(SharedString, SharedString)> = self
            .favorites
            .iter()
            .filter_map(|id| {
                self.providers.iter().find_map(|provider| {
                    provider
                        .models
                        .iter()
                        .any(|m| &m.id == id)
                        .then(|| (provider.id.clone(), id.clone()))
                })
            })
            .collect();
        let props = Rc::new(Props {
            providers: self.providers,
            on_change: self.on_change,
            on_reasoning_change: self.on_reasoning_change,
            on_setting_change: self.on_setting_change,
            on_favorite_change: self.on_favorite_change,
        });
        let (default_open, autofocus, reasoning) =
            (self.default_open, self.autofocus, self.reasoning);

        let state = window.use_keyed_state(
            ElementId::NamedChild(Arc::new(self.id.clone()), "state".into()),
            cx,
            {
                let props = props.clone();
                move |window, cx| {
                    let focus = cx.focus_handle().tab_stop(true);
                    let search =
                        cx.new(|cx| TextInput::new(window, cx).placeholder("Search models…"));
                    let search_events = cx.subscribe_in(
                        &search,
                        window,
                        |s: &mut PickerState, _, event, window, cx| s.on_search(event, window, cx),
                    );
                    let mut s = PickerState {
                        focus,
                        search,
                        _search_events: search_events,
                        props,
                        open: false,
                        closing: None,
                        _close_task: None,
                        opened: 0,
                        rail: Rail::Provider(initial.0),
                        selected: initial,
                        active: 0,
                        query: SharedString::default(),
                        reasoning,
                        picks: HashMap::new(),
                        favorites,
                        scroll: ScrollHandle::new(),
                        trigger: Bounds::default(),
                        setting_menu: None,
                        setting_active: 0,
                        setting_bounds: None,
                        setting_on_left: false,
                        setting_origin: None,
                        hover_pending: None,
                        hover_pointer: None,
                        _hover_task: None,
                    };
                    if default_open {
                        s.open = true;
                        s.opened = 1;
                        s.rail = if s.favorites.is_empty() {
                            Rail::Provider(initial.0)
                        } else {
                            Rail::Favorites
                        };
                        s.active = s.selected_index();
                        s.scroll.scroll_to_item(s.active);
                    }
                    if autofocus {
                        let handle = if default_open {
                            s.search.read(cx).focus_handle(cx)
                        } else {
                            s.focus.clone()
                        };
                        window.defer(cx, move |window, cx| window.focus(&handle, cx));
                    }
                    s
                }
            },
        );
        state.update(cx, |s, cx| {
            s.props = props;
            s.clamp();
            s.search.update(cx, |input, _| {
                input.set_colors(Some(t.placeholder), Some(t.caret), Some(t.selection))
            });
        });

        let placement = self.placement;
        let s = state.read(cx);
        let trigger = trigger(&key, s, &t, &state);
        let layer = (s.open || s.closing.is_some()).then(|| {
            let exit = (!s.open)
                .then(|| s.closing.map(motion::menu_exit_progress))
                .flatten();
            let above = placement == Placement::Above;
            let inner = div()
                .occlude()
                .when(above, |el| el.pb(px(layout::POPOVER_GAP)))
                .when(!above, |el| el.pt(px(layout::POPOVER_GAP)))
                .child(menu(&key, s, &t, &state));
            let inner: AnyElement = match exit {
                // A dying menu takes no clicks (Pink: `menu_motion_from`).
                Some(progress) => motion::menu_out(
                    ElementId::NamedInteger("menu-out".into(), u64::from(s.opened)),
                    progress,
                    inner.relative().child(div().absolute().inset_0().occlude()),
                )
                .into_any_element(),
                None => motion::menu_in_from(
                    ElementId::NamedInteger("menu-in".into(), u64::from(s.opened)),
                    if above { MENU_RISE_UP } else { MENU_RISE_DOWN },
                    inner,
                )
                .into_any_element(),
            };
            // Pink: `anchored_menu_above_leading` / `_below_leading`. The card
            // starts one inset left of the chip so its rows line up with the
            // chip's mark.
            div()
                .absolute()
                .left_0()
                .when(above, |el| el.top_0())
                .when(!above, |el| el.bottom_0())
                .size_0()
                .child(
                    deferred(
                        anchored()
                            .anchor(if above {
                                Anchor::BottomLeft
                            } else {
                                Anchor::TopLeft
                            })
                            .offset(point(px(-layout::CARD_INSET), px(0.)))
                            .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                            .child(inner),
                    )
                    .with_priority(1),
                )
        });

        let on_key = {
            let state = state.clone();
            move |event: &KeyDownEvent, window: &mut Window, cx: &mut App| {
                let keystroke = &event.keystroke;
                // ⌘1–⌘9 pick the Nth visible row (Pink: the rows' hints).
                if keystroke.modifiers.secondary()
                    && let Ok(n) = keystroke.key.parse::<usize>()
                    && (1..=9).contains(&n)
                {
                    let handled = state.update(cx, |s, cx| {
                        s.open && {
                            s.pick_model(n - 1, window, cx);
                            true
                        }
                    });
                    if handled {
                        cx.stop_propagation();
                    }
                    return;
                }
                if keystroke.modifiers.modified() {
                    return;
                }
                let nav = match keystroke.key.as_str() {
                    "up" => Nav::Up,
                    "down" => Nav::Down,
                    "left" => Nav::Left,
                    "right" => Nav::Right,
                    "enter" => Nav::Enter,
                    "escape" => Nav::Escape,
                    _ => return,
                };
                if state.update(cx, |s, cx| s.nav(nav, window, cx)) {
                    cx.stop_propagation();
                }
            }
        };

        motion::keep_alive(window);
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .font_family(pink::SANS)
            .on_key_down(on_key)
            // The search field binds these keys; the picker reads them first.
            .capture_action(nav_action::<text_input::Up>(&state, Nav::Up))
            .capture_action(nav_action::<text_input::Down>(&state, Nav::Down))
            .capture_action(nav_action::<text_input::Left>(&state, Nav::Left))
            .capture_action(nav_action::<text_input::Right>(&state, Nav::Right))
            .capture_action(nav_action::<text_input::Enter>(&state, Nav::Enter))
            .capture_action(nav_action::<text_input::Escape>(&state, Nav::Escape))
            .child(trigger)
            .children(layer)
    }
}

/// The model chip (Pink: pickers.rs `trigger_chip` with
/// `model_chip_content`): the provider's mark, the model name, a bolt for a
/// fast speed and the reasoning level as the muted second tone, brighter
/// when the configuration departs from the model's defaults.
fn trigger(key: &str, s: &PickerState, t: &Pink, state: &Entity<PickerState>) -> Stateful<Div> {
    let provider = s.provider();
    let model = s.model();
    let id = format!("{key}-trigger");
    // The kit chip's own hover key: the bolt brightens with the label.
    let label_color =
        motion::hover_blend(&format!("pink-trigger-{id}"), t.text.opacity(0.9), t.text);
    let suffix = s.effective_reasoning().map(|level| {
        let tone = if s.customized() {
            t.text.opacity(0.85)
        } else {
            t.readable_muted(0.7)
        };
        (level.label(), tone)
    });
    let (title, detail) = (model.name.clone(), s.summary());
    let toggler = state.clone();
    let measurer = state.clone();
    pink::trigger_chip(id, true, s.open, t)
        .relative()
        .track_focus(&s.focus)
        .aria_label(model.name.clone())
        .on_click(move |_, window, cx| toggler.update(cx, |s, cx| s.toggle(window, cx)))
        // The open menu already says everything the tooltip would.
        .when(!s.open, |el| {
            el.tooltip(move |_, cx| ControlTooltip::view(title.clone(), detail.clone(), cx))
                .tooltip_show_delay(layout::TOOLTIP_DELAY)
        })
        .child(
            svg()
                .path(provider.icon.clone())
                .size(px(CHIP_ICON))
                .flex_none()
                .text_color(provider.tint.unwrap_or(t.text_muted)),
        )
        .child(div().min_w_0().truncate().child(model.name.clone()))
        .when(s.fast(), |el| {
            el.child(
                svg()
                    .path(ICON_BOLT)
                    .size(px(CHIP_BOLT))
                    .flex_none()
                    .text_color(label_color),
            )
        })
        .when_some(suffix, |el, (label, tone)| {
            el.child(div().flex_none().text_color(tone).child(label))
        })
        .child(
            canvas(
                move |bounds, _, cx| measurer.update(cx, |s, _| s.trigger = bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
}

/// The menu card (Pink: `popover_frame_flush` at `MENU_W_L` around
/// `render_harness_model_popover`): tabs, search, the model band and the
/// traits tray.
fn menu(key: &str, s: &PickerState, t: &Pink, state: &Entity<PickerState>) -> Div {
    let rows = s.rows();
    let groups = s.groups();
    let refocus = s.search.clone();
    let closer = state.clone();
    pink::popover_card_flush(t)
        .w(px(layout::MENU_W_L))
        .flex()
        .flex_col()
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            if !refocus.read(cx).is_focused(window) {
                TextInput::focus(&refocus, window, cx);
            }
        })
        .on_mouse_down_out(move |event: &MouseDownEvent, window, cx| {
            closer.update(cx, |s, cx| {
                if !s.open
                    || s.trigger.contains(&event.position)
                    || (s.setting_menu.is_some()
                        && s.setting_bounds
                            .is_some_and(|b| b.contains(&event.position)))
                {
                    return;
                }
                let focused = s.search.read(cx).is_focused(window);
                s.close_menu(false, window, cx);
                if focused {
                    window.blur(cx);
                }
            })
        })
        .child(
            div()
                .flex()
                .flex_col()
                .child(tabs(key, s, t, state))
                .child(search_row(s, t))
                .child(list(key, s, &rows, t, state))
                .children(tray(key, s, &groups, rows.len(), t, state)),
        )
}

/// A provider tab (Pink: `model_tab`): an announced tab that keeps focus in
/// the search field when pressed.
fn tab(id: impl Into<ElementId>, label: SharedString, viewed: bool, t: &Pink) -> Stateful<Div> {
    let ring = t.accent;
    div()
        .id(id)
        .role(Role::Tab)
        .aria_label(label)
        .aria_selected(viewed)
        .tab_index(0)
        .focus_visible(move |style| pink::focus_ring(style, ring))
        .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
        .on_key_down(pink::swallow_activation_key)
        .relative()
        .w(px(TAB))
        .h(px(TAB))
        .rounded(px(radius::MD))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
}

/// The viewed tab's 2 px accent bar, sitting on the strip's bottom hairline
/// (Pink: `tab_indicator`).
fn tab_indicator(t: &Pink) -> Div {
    div()
        .absolute()
        .bottom(px(-4.))
        .left(px(6.))
        .right(px(6.))
        .h(px(2.))
        .rounded(px(1.))
        .bg(t.accent)
}

/// The tab strip: the Favorites star, then one mark per provider.
fn tabs(key: &str, s: &PickerState, t: &Pink, state: &Entity<PickerState>) -> Stateful<Div> {
    let favorites = s.rail == Rail::Favorites;
    let viewer = state.clone();
    let star = tab("tab-favorites", "Favorites".into(), favorites, t)
        .when(!favorites, |el| {
            el.hover_wash(format!("{key}-tab-favorites"), t.fill(FillTier::Tertiary))
        })
        .on_click(move |_, _, cx| viewer.update(cx, |s, cx| s.view(Rail::Favorites, cx)))
        .child(
            svg()
                .path(ICON_STAR_BOLD)
                .size(px(15.))
                .text_color(if favorites { t.text } else { t.text_muted }),
        )
        .when(favorites, |el| el.child(tab_indicator(t)));
    let providers = s.props.providers.iter().enumerate().map(|(ix, provider)| {
        let viewed = s.rail == Rail::Provider(ix);
        let viewer = state.clone();
        tab(("tab-provider", ix), provider.name.clone(), viewed, t)
            .when(!viewed, |el| {
                el.hover_wash(format!("{key}-tab-{ix}"), t.fill(FillTier::Tertiary))
            })
            .on_click(move |_, _, cx| viewer.update(cx, |s, cx| s.view(Rail::Provider(ix), cx)))
            .child(
                svg().path(provider.icon.clone()).size(px(16.)).text_color(
                    provider
                        .tint
                        .unwrap_or(if viewed { t.text } else { t.text_muted }),
                ),
            )
            .when(viewed, |el| el.child(tab_indicator(t)))
    });
    div()
        .id("model-provider-tabs")
        .role(Role::TabList)
        .aria_label("Providers")
        .flex_none()
        .h(px(STRIP_H))
        .px(px(layout::CARD_INSET))
        .border_b_1()
        .border_color(t.hairline(0.08))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.))
        .child(star)
        .children(providers)
}

/// The search row: a magnifier and a borderless field over a full-bleed
/// hairline (a strip, not a well).
fn search_row(s: &PickerState, t: &Pink) -> Div {
    div()
        .flex_none()
        .h(px(STRIP_H))
        .px(px(10.))
        .border_b_1()
        .border_color(t.hairline(0.08))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(8.))
        .child(
            svg()
                .path(ICON_MAGNIFER)
                .size(px(14.))
                .flex_none()
                .text_color(t.text_muted),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(13.))
                .child(s.search.clone()),
        )
}

/// The model band: a fixed height on a whisper of wash between the pinned
/// chrome above and the tray below.
fn list(
    key: &str,
    s: &PickerState,
    rows: &[Row],
    t: &Pink,
    state: &Entity<PickerState>,
) -> Stateful<Div> {
    let body = if rows.is_empty() {
        let note = if !s.query.trim().is_empty() {
            "No models found"
        } else {
            "No starred models yet. Star a row to keep it here"
        };
        div()
            .id("model-list")
            .size_full()
            .px(px(layout::CARD_INSET))
            .child(pink::menu_empty(note, t))
    } else {
        div()
            .id("model-list")
            .role(Role::ListBox)
            .aria_label("Models")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&s.scroll)
            .px(px(layout::CARD_INSET))
            .children(
                rows.iter()
                    .enumerate()
                    .map(|(ix, &row)| model_row(key, ix, row, rows.len(), s, t, state)),
            )
    };
    div()
        .id("model-list-host")
        .relative()
        .flex_none()
        .h(px(LIST_H))
        .py(px(layout::CARD_INSET))
        .bg(t.ink(0.02))
        .child(body)
}

/// One model row (Pink: `render_model_row`). Hovering moves the shared
/// highlight; the selected row wears the stronger wash and ring. A
/// provider tab shows the name alone (plus an attribution), the Favorites
/// tab adds the provider's mark and name underneath.
fn model_row(
    key: &str,
    ix: usize,
    row: Row,
    count: usize,
    s: &PickerState,
    t: &Pink,
    state: &Entity<PickerState>,
) -> AnyElement {
    let provider = &s.props.providers[row.p];
    let model = &provider.models[row.m];
    let selected = s.selected == (row.p, row.m);
    let active = ix == s.active;
    let starred = s.starred(row.p, row.m);
    let compact = s.rail != Rail::Favorites;
    let name = model.name.clone();

    let label = div()
        .text_size(px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(t.text)
        .child(name.clone());
    let badge = model.tag.clone().map(|tag| new_badge(tag, t));
    let attribution = model.attribution.clone();
    let body = if compact {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .child(label.flex_none().max_w_full().truncate())
            .children(badge)
            .when_some(attribution, |el, attribution| {
                el.child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child(attribution),
                )
            })
    } else {
        let meta = |copy: SharedString| {
            div()
                .flex_none()
                .text_size(px(11.))
                .text_color(t.text_muted)
                .child(copy)
        };
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(label.min_w_0().truncate())
                    .children(badge),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        svg()
                            .path(provider.icon.clone())
                            .size(px(11.))
                            .flex_none()
                            .text_color(provider.tint.unwrap_or(t.text_muted)),
                    )
                    .child(meta(provider.name.clone()))
                    .when_some(attribution, |el, attribution| {
                        el.child(meta(attribution).flex_initial().min_w_0().truncate())
                    }),
            )
    };

    let hover = state.clone();
    let chooser = state.clone();
    let star_state = state.clone();
    let star = div()
        .id("star")
        .role(Role::Button)
        .aria_label(SharedString::from(format!("Favorite {name}")))
        .aria_toggled(if starred {
            Toggled::True
        } else {
            Toggled::False
        })
        .flex_none()
        .size(px(ROW_STAR))
        .rounded(px(radius::MENU_ITEM))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover_wash(
            format!("{key}-star-{}-{}", provider.id, model.id),
            t.fill(FillTier::Tertiary),
        )
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            star_state.update(cx, |s, cx| s.toggle_favorite(row.p, row.m, window, cx));
        })
        .child(
            svg()
                .path(if starred { ICON_STAR_BOLD } else { ICON_STAR })
                .size(px(13.))
                .text_color(if starred { t.warning } else { t.text_muted }),
        );

    let el = div()
        .id(ElementId::Name(
            format!("model-row-{}-{}", provider.id, model.id).into(),
        ))
        .role(Role::ListBoxOption)
        .aria_label(name)
        .aria_selected(selected)
        .aria_position_in_set(ix + 1)
        .aria_size_of_set(count)
        .px(px(8.))
        .py(px(if compact { 5. } else { 6. }))
        .rounded(px(radius::MENU_ITEM))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(10.))
        .cursor_pointer()
        .when(selected, |el| {
            el.bg(t.card_selected_bg())
                .shadow(t.card_selected_shadows())
        })
        .when(!selected && active, |el| el.bg(t.ink(0.05)))
        .on_hover(move |hovered, _, cx| {
            if *hovered {
                hover.update(cx, |s, cx| {
                    if s.active != ix {
                        s.active = ix;
                        cx.notify();
                    }
                });
            }
        })
        .on_click(move |_, window, cx| chooser.update(cx, |s, cx| s.pick_model(ix, window, cx)))
        .child(body)
        .when(ix < 9, |el| {
            el.child(pink::kbd_hint(format!("⌘{}", ix + 1), t))
        })
        .child(star);
    div().pb(px(layout::MENU_GAP)).child(el).into_any_element()
}

/// The "New" capsule beside a model's name (Pink: `new_model_badge`).
fn new_badge(label: SharedString, t: &Pink) -> Div {
    div()
        .flex_none()
        .h(px(16.))
        .px(px(5.))
        .flex()
        .items_center()
        .rounded(px(radius::XS))
        .bg(t.accent.opacity(0.14))
        .text_size(px(10.5))
        .line_height(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.readable_tone(t.accent, 1.0))
        .child(label)
}

/// The "Default" marker beside a row's default choice: bare muted text
/// (Pink: `default_badge`).
fn default_badge(t: &Pink) -> Div {
    div()
        .flex_none()
        .text_size(px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.text_muted)
        .child("Default")
}

/// The traits tray pinned under the list (Pink: the tray around
/// `render_traits_sections`): the Reasoning row and the model's settings,
/// each naming its current choice. Hidden when the model has neither.
fn tray(
    key: &str,
    s: &PickerState,
    groups: &[Group],
    base: usize,
    t: &Pink,
    state: &Entity<PickerState>,
) -> Option<Stateful<Div>> {
    if groups.is_empty() {
        return None;
    }
    let rows = groups
        .iter()
        .enumerate()
        .map(|(ix, group)| tray_row(key, ix, group, base, s, t, state));
    Some(
        div()
            .id("model-traits-tray")
            .flex_none()
            .border_t_1()
            .border_color(t.hairline(0.08))
            .max_h(px(TRAY_MAX_H))
            .overflow_y_scroll()
            .px(px(layout::CARD_INSET))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(layout::MENU_GAP))
                    .py(px(layout::CARD_INSET))
                    .children(rows),
            ),
    )
}

/// One tray row: a 30 tall menu row with the label, the muted value and a
/// chevron. The pointer opens its choices by resting on it; a click only
/// closes them.
fn tray_row(
    key: &str,
    ix: usize,
    group: &Group,
    base: usize,
    s: &PickerState,
    t: &Pink,
    state: &Entity<PickerState>,
) -> Stateful<Div> {
    let open = s.setting_menu == Some(ix);
    let value = group
        .choices
        .iter()
        .find(|choice| choice.selected)
        .map(|choice| choice.label.clone())
        .unwrap_or_default();
    let clicker = state.clone();
    let outside = state.clone();
    let side = state.clone();
    let exit = state.clone();
    let row = pink::menu_row(
        format!("{key}-setting-{ix}"),
        open || s.active == base + ix,
        t,
    )
    .role(Role::ComboBox)
    .aria_label(group.label.clone())
    .aria_value(value.clone())
    .aria_expanded(open)
    .relative()
    .h(px(TRAY_ROW_H))
    .py(px(0.))
    .on_click(move |_, window, cx| {
        cx.stop_propagation();
        clicker.update(cx, |s, cx| {
            s.active = base + ix;
            s.close_setting();
            TextInput::focus(&s.search, window, cx);
            cx.notify();
        });
    })
    // Elsewhere in the card, a press closes these choices and still
    // reaches its target; inside the choices it stays local.
    .on_mouse_down_out(move |event: &MouseDownEvent, _, cx| {
        outside.update(cx, |s, cx| {
            if s.setting_menu == Some(ix)
                && !s
                    .setting_bounds
                    .is_some_and(|b| b.contains(&event.position))
            {
                s.close_setting();
                cx.notify();
            }
        })
    })
    .child(
        canvas(
            move |bounds, window, cx| {
                let left = bounds.right() + px(NESTED_ROOM) > window.viewport_size().width;
                side.update(cx, |s, cx| {
                    if s.setting_on_left != left {
                        s.setting_on_left = left;
                        cx.notify();
                    }
                });
            },
            move |trigger, _, window, _| {
                if !open {
                    return;
                }
                // Leaving both the row and its choices (and not heading
                // for them) closes them.
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    exit.update(cx, |s, cx| {
                        if s.setting_menu != Some(ix) {
                            return;
                        }
                        let pointer = event.position;
                        if trigger.contains(&pointer) {
                            s.setting_origin = Some(pointer);
                            return;
                        }
                        let Some(bounds) = s.setting_bounds else {
                            return;
                        };
                        if bounds.contains(&pointer)
                            || s.setting_origin.is_some_and(|origin| {
                                submenu_corridor(origin, pointer, bounds, s.setting_on_left)
                            })
                        {
                            return;
                        }
                        s.close_setting();
                        s.setting_origin = None;
                        // No keyboard-style highlight is left on the row.
                        s.active = 0;
                        cx.notify();
                    });
                });
            },
        )
        .absolute()
        .inset_0(),
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .truncate()
            .child(group.label.clone()),
    )
    .child(
        div()
            .max_w(px(100.))
            .truncate()
            .text_color(t.text_muted)
            .child(value),
    )
    .child(pink::line_glyph(
        pink::glyphs::ALT_ARROW_RIGHT,
        12.,
        t.text_muted,
    ))
    .when(open, |el| el.child(choices(key, ix, group, s, t, state)));

    // Hover lives on a stable wrapper: the row owns its own hover fade.
    let hover = state.clone();
    let mover = state.clone();
    div()
        .id(("model-setting-hover", ix))
        .on_hover(move |hovered, window, cx| {
            let pointer = window.mouse_position();
            hover.update(cx, |s, cx| {
                if *hovered {
                    s.hover_setting(ix, base + ix, pointer, cx);
                } else if s.hover_pending == Some(ix) {
                    s.cancel_setting_hover();
                }
            })
        })
        .on_mouse_move(move |event: &MouseMoveEvent, _, cx| {
            mover.update(cx, |s, cx| {
                if s.setting_menu == Some(ix) {
                    s.setting_origin = Some(event.position);
                } else if s.hover_pending == Some(ix) {
                    let in_corridor =
                        s.setting_origin
                            .zip(s.setting_bounds)
                            .is_some_and(|(origin, bounds)| {
                                submenu_corridor(origin, event.position, bounds, s.setting_on_left)
                            });
                    let forward = s.hover_pointer.map_or(0.0, |previous| {
                        f32::from(event.position.x - previous.x)
                            * if s.setting_on_left { -1.0 } else { 1.0 }
                    });
                    if !in_corridor || forward <= -2.0 {
                        s.active = base + ix;
                        s.open_setting(ix, cx);
                        s.setting_origin = Some(event.position);
                    } else if forward >= 2.0 {
                        // Slow but continuing progress renews the grace.
                        s.hover_setting(ix, base + ix, event.position, cx);
                    }
                }
            })
        })
        .child(row)
}

/// A tray row's nested choices (Pink: the `popover_card` that
/// `render_traits_sections` hands `nested_menu`): a 232 card beside the
/// row with an uppercase heading, the Default marker and a check.
fn choices(
    key: &str,
    ix: usize,
    group: &Group,
    s: &PickerState,
    t: &Pink,
    state: &Entity<PickerState>,
) -> AnyElement {
    let rows = group.choices.iter().enumerate().map(|(ci, choice)| {
        let picker = state.clone();
        pink::menu_row(format!("{key}-choice-{ix}-{ci}"), ci == s.setting_active, t)
            .role(Role::ListBoxOption)
            .aria_label(choice.label.clone())
            .aria_selected(choice.selected)
            .h(px(TRAY_ROW_H))
            .py(px(0.))
            .flex_none()
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                picker.update(cx, |s, cx| {
                    s.setting_active = ci;
                    s.pick_choice(window, cx);
                });
            })
            .child(choice.label.clone())
            .child(div().flex_1())
            .when(choice.default, |el| el.child(default_badge(t)))
            .when(choice.selected, |el| {
                el.child(pink::line_glyph(pink::glyphs::CHECK, 14., t.text))
            })
    });
    let measurer = state.clone();
    let card = pink::popover_card(t)
        .w(px(CHOICES_W))
        .relative()
        .child(pink::menu_heading(&group.label, t))
        .child(
            div()
                .id("model-setting-choices")
                .role(Role::ListBox)
                .aria_label(group.label.clone())
                .max_h(px(CHOICES_MAX_H))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(layout::MENU_GAP))
                .children(rows),
        )
        .child(
            canvas(
                move |bounds, _, cx| measurer.update(cx, |s, _| s.setting_bounds = Some(bounds)),
                |_, _, _, _| {},
            )
            .absolute()
            .inset_0(),
        );
    let left = s.setting_on_left;
    div()
        .absolute()
        .top_0()
        .size_0()
        .when(left, |el| el.left(px(-NESTED_GAP)))
        .when(!left, |el| el.right(px(-NESTED_GAP)))
        .child(
            deferred(
                anchored()
                    .anchor(if left {
                        Anchor::TopRight
                    } else {
                        Anchor::TopLeft
                    })
                    .snap_to_window_with_margin(px(layout::WINDOW_MARGIN))
                    .child(motion::menu_in_from(
                        SharedString::from(format!("{key}-setting-menu-{ix}")),
                        MENU_RISE_DOWN,
                        div().occlude().child(card),
                    )),
            )
            .with_priority(2),
        )
        .into_any_element()
}

/// The chip's tooltip: the model over its configuration (Pink: pickers.rs
/// `ControlTooltip`).
struct ControlTooltip {
    title: SharedString,
    detail: Option<SharedString>,
}

impl ControlTooltip {
    fn view(title: SharedString, detail: Option<SharedString>, cx: &mut App) -> AnyView {
        cx.new(|_| Self { title, detail }).into()
    }
}

impl Render for ControlTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Pink::get(cx);
        let wraps = self.detail.as_ref().is_some_and(|detail| {
            detail
                .split('\n')
                .any(|line| line.chars().count() > TOOLTIP_WRAP_CHARS)
        });
        pink::tooltip_card(&t)
            .when(wraps, |card| card.w(px(layout::TOOLTIP_MAX_WIDTH)))
            .when(!wraps, |card| card.whitespace_nowrap())
            .px(px(10.))
            .py(px(7.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .text_size(px(12.))
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(t.text)
                    .child(self.title.clone()),
            )
            .when_some(self.detail.clone(), |card, detail| {
                card.children(detail.split('\n').map(|line| {
                    div()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child(SharedString::from(line.to_string()))
                }))
            })
    }
}

pub fn story(_window: &mut Window, cx: &mut App) -> AnyView {
    story::view(cx, |_, cx| {
        let t = Pink::get(cx);
        // Pink composer.rs: the opaque composer pill, its textarea and the
        // controls row (attach, the model chip, the recessed Send disc).
        let attach = div()
            .id("mp-attach")
            .size(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .hover_wash("mp-story-attach", t.ink(0.10))
            .child(
                svg()
                    .path(ICON_PAPERCLIP)
                    .size(px(16.))
                    .relative()
                    .left(px(1.))
                    .text_color(t.text_muted),
            );
        let send = div()
            .size(px(28.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(t.ink(0.08))
            .child(
                svg()
                    .path(ICON_ARROW_UP)
                    .size(px(14.))
                    .text_color(t.text.opacity(0.35)),
            );
        div()
            .w(px(640.))
            .h(px(520.))
            .flex()
            .flex_col()
            .justify_end()
            .font_family(pink::SANS)
            .child(
                div()
                    .w_full()
                    .rounded(px(radius::COMPOSER_PILL))
                    .border_1()
                    .border_color(t.border)
                    .bg(t.surface_bg())
                    .shadow(pink::shadow_lg())
                    .child(
                        div()
                            .h(px(60.))
                            .px(px(16.))
                            .pt(px(16.))
                            .text_size(px(14.))
                            .line_height(px(22.75))
                            .text_color(t.placeholder)
                            .child("Ask for a change, a review or a plan…"),
                    )
                    .child(
                        div()
                            .h(px(46.))
                            .px(px(10.))
                            .pt(px(2.))
                            .pb(px(12.))
                            .flex()
                            .items_center()
                            .child(
                                div().flex().items_center().gap(px(2.)).child(attach).child(
                                    ModelPicker::new("model-picker", default_providers())
                                        .selected("helix-4-pro")
                                        .default_open(true)
                                        .autofocus(true),
                                ),
                            )
                            .child(div().flex_1())
                            .child(send),
                    ),
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reasoning_defaults_to_high_then_medium() {
        use ReasoningLevel::*;
        assert_eq!(default_reasoning(&[Low, Medium, High, Max]), Some(High));
        assert_eq!(default_reasoning(&[Minimal, Low, Medium]), Some(Medium));
        assert_eq!(default_reasoning(&[Minimal, Low]), Some(Minimal));
        assert_eq!(default_reasoning(&[]), None);
        assert_eq!(clamp_reasoning(Some(Max), &[Low, Medium, High]), Some(High));
        assert_eq!(clamp_reasoning(Some(Low), &[Low, Medium, High]), Some(Low));
    }

    #[test]
    fn rows_stay_in_the_viewed_tab() {
        let providers = default_providers();
        let none = |_: usize, _: usize| false;
        let helix = scoped_rows(&providers, Rail::Provider(0), "", none);
        assert_eq!(helix.len(), providers[0].models.len());
        // Stars lead a provider tab.
        let star = |p: usize, m: usize| p == 0 && m == 3;
        assert_eq!(
            scoped_rows(&providers, Rail::Provider(0), "", star)[0],
            Row { p: 0, m: 3 }
        );
        assert_eq!(
            scoped_rows(&providers, Rail::Favorites, "", star),
            vec![Row { p: 0, m: 3 }]
        );
        // A query ranks prefix hits first and never leaves the tab.
        let flash = scoped_rows(&providers, Rail::Provider(0), "flash", none);
        assert_eq!(flash.len(), 2);
        assert!(scoped_rows(&providers, Rail::Provider(1), "flash", none).is_empty());
        // Attribution is searchable too.
        assert_eq!(
            scoped_rows(&providers, Rail::Provider(2), "north", none),
            vec![Row { p: 2, m: 0 }]
        );
    }

    #[test]
    fn steps_wrap() {
        assert_eq!(step(0, 3, true), 2);
        assert_eq!(step(2, 3, false), 0);
        assert_eq!(step(5, 3, true), 2);
        assert_eq!(step(0, 0, false), 0);
    }
}
